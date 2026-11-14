"""Create an exclusive archive of a GN runtime_deps set and its license."""

from __future__ import annotations

import argparse
from contextlib import ExitStack
from collections.abc import Sequence
from dataclasses import dataclass
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile
from typing import BinaryIO

_SCHEMA = "krr-html-performance-observer-artifact-v1"
_MANIFEST_NAME = "manifest.json"
_LICENSE_NAME = "LICENSE.headless_shell"
_BUFFER_SIZE = 1024 * 1024
_NOFOLLOW = getattr(os, "O_NOFOLLOW", 0)
_DIRECTORY = getattr(os, "O_DIRECTORY", 0)


@dataclass(frozen=True)
class StableInput:
    path: Path
    sha256: str
    state: tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class FileRecord:
    root: Path
    source_path: str
    archive_path: str
    sha256: str
    size: int
    mode: int
    state: tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class VerifiedOutput:
    path: Path
    stream: BinaryIO
    sha256: str
    state: tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class ArtifactSummary:
    archive_sha256: str
    manifest_sha256: str
    file_count: int


class DigestingReader:
    def __init__(self, source: BinaryIO) -> None:
        self.source = source
        self.digest = hashlib.sha256()
        self.bytes_read = 0

    def read(self, size: int = -1) -> bytes:
        data = self.source.read(size)
        self.digest.update(data)
        self.bytes_read += len(data)
        return data


def _state(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
        value.st_mode,
    )


def _canonical_directory(path: Path) -> Path:
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("input directory is not canonical")
    value = path.lstat()
    if stat.S_ISLNK(value.st_mode) or not stat.S_ISDIR(value.st_mode):
        raise ValueError("input root is not a regular directory")
    return path


def _canonical_file(path: Path) -> Path:
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("input file is not canonical")
    value = path.lstat()
    if stat.S_ISLNK(value.st_mode) or not stat.S_ISREG(value.st_mode):
        raise ValueError("input is not a regular file")
    return path


def _open_relative(root: Path, relative: str) -> int:
    if not _NOFOLLOW or not _DIRECTORY:
        raise RuntimeError("secure no-follow file access is unavailable")
    parts = PurePosixPath(relative).parts
    directory_fd = os.open(root, os.O_RDONLY | _DIRECTORY | _NOFOLLOW)
    try:
        for part in parts[:-1]:
            next_fd = os.open(
                part,
                os.O_RDONLY | _DIRECTORY | _NOFOLLOW,
                dir_fd=directory_fd,
            )
            os.close(directory_fd)
            directory_fd = next_fd
        return os.open(
            parts[-1],
            os.O_RDONLY | _NOFOLLOW,
            dir_fd=directory_fd,
        )
    finally:
        os.close(directory_fd)


def _relative_state(root: Path, relative: str) -> tuple[int, int, int, int, int, int]:
    descriptor = _open_relative(root, relative)
    try:
        value = os.fstat(descriptor)
        if not stat.S_ISREG(value.st_mode):
            raise ValueError("runtime dependency is not a regular file")
        return _state(value)
    finally:
        os.close(descriptor)


def _read_stable(path: Path) -> tuple[bytes, StableInput]:
    _canonical_file(path)
    initial = _state(path.lstat())
    descriptor = os.open(path, os.O_RDONLY | _NOFOLLOW)
    try:
        if _state(os.fstat(descriptor)) != initial:
            raise ValueError("input changed before reading")
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, _BUFFER_SIZE):
            chunks.append(chunk)
        if _state(os.fstat(descriptor)) != initial or _state(path.lstat()) != initial:
            raise ValueError("input changed while reading")
        data = b"".join(chunks)
        return data, StableInput(path, hashlib.sha256(data).hexdigest(), initial)
    finally:
        os.close(descriptor)


def _relative_path(value: str) -> str:
    normalized = value.strip()
    parts = normalized.split("/")
    if (
        not normalized
        or "\\" in normalized
        or normalized.startswith("/")
        or any(part in ("", "..") for part in parts)
    ):
        raise ValueError("runtime dependency path is not canonical and relative")
    canonical = "/".join(part for part in parts if part != ".")
    if not canonical:
        raise ValueError("runtime dependency path is empty after normalization")
    return canonical


def _runtime_paths(runtime_deps: bytes) -> list[str]:
    text = runtime_deps.decode("utf-8")
    values = [_relative_path(line) for line in text.splitlines()]
    if not values:
        raise ValueError("runtime dependency list is empty")
    return sorted(set(values))


def _verify_stable_input(expected: StableInput) -> None:
    _data, current = _read_stable(expected.path)
    if current != expected:
        raise ValueError("input changed after inventory")


def _hash_descriptor(descriptor: int) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    os.lseek(descriptor, 0, os.SEEK_SET)
    while chunk := os.read(descriptor, _BUFFER_SIZE):
        digest.update(chunk)
        size += len(chunk)
    return digest.hexdigest(), size


def _collect_file(root: Path, source_path: str, archive_path: str) -> FileRecord:
    descriptor = _open_relative(root, source_path)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise ValueError("archive input is not a regular file")
        checksum, size = _hash_descriptor(descriptor)
        after = os.fstat(descriptor)
        current = _relative_state(root, source_path)
        if _state(before) != _state(after) or _state(after) != current:
            raise ValueError("archive input changed while hashing")
        return FileRecord(
            root=root,
            source_path=source_path,
            archive_path=archive_path,
            sha256=checksum,
            size=size,
            mode=stat.S_IMODE(before.st_mode),
            state=_state(before),
        )
    finally:
        os.close(descriptor)


def _verify_record(expected: FileRecord) -> None:
    current = _collect_file(expected.root, expected.source_path, expected.archive_path)
    if current != expected:
        raise ValueError("archive input changed after writing")


def _verify_inputs(runtime_deps: StableInput, records: list[FileRecord]) -> None:
    _verify_stable_input(runtime_deps)
    for record in records:
        _verify_record(record)


def _manifest_bytes(records: list[FileRecord]) -> bytes:
    value = {
        "schema": _SCHEMA,
        "files": [
            {
                "path": record.archive_path,
                "sha256": record.sha256,
                "size": record.size,
                "mode": f"{record.mode:04o}",
            }
            for record in records
        ],
    }
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def _tar_info(name: str, size: int, mode: int) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.size = size
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    return info


def _add_file(archive: tarfile.TarFile, record: FileRecord) -> None:
    descriptor = _open_relative(record.root, record.source_path)
    with os.fdopen(descriptor, "rb") as source:
        before = os.fstat(source.fileno())
        if _state(before) != record.state:
            raise ValueError("archive input changed after inventory")
        reader = DigestingReader(source)
        archive.addfile(
            _tar_info(record.archive_path, record.size, record.mode),
            reader,
        )
        after = os.fstat(source.fileno())
        current = _relative_state(record.root, record.source_path)
        if (
            _state(after) != record.state
            or current != record.state
            or reader.bytes_read != record.size
            or reader.digest.hexdigest() != record.sha256
        ):
            raise ValueError("archive input changed while archiving")


def _add_bytes(archive: tarfile.TarFile, name: str, value: bytes) -> None:
    archive.addfile(_tar_info(name, len(value), 0o444), io.BytesIO(value))


def _hash_stream(stream: BinaryIO) -> str:
    position = stream.tell()
    os.lseek(stream.fileno(), 0, os.SEEK_SET)
    digest = hashlib.sha256()
    while chunk := os.read(stream.fileno(), _BUFFER_SIZE):
        digest.update(chunk)
    stream.seek(position)
    return digest.hexdigest()


def _verify_output(
    path: Path,
    stream: BinaryIO,
    created_state: tuple[int, int, int, int, int, int],
) -> VerifiedOutput:
    stream.flush()
    os.fsync(stream.fileno())
    before = _state(os.fstat(stream.fileno()))
    checksum = _hash_stream(stream)
    after = _state(os.fstat(stream.fileno()))
    path_info = path.lstat()
    path_state = _state(path_info)
    if (
        not stat.S_ISREG(path_info.st_mode)
        or before[:2] != created_state[:2]
        or before != after
        or path_state != after
    ):
        raise ValueError("generated output identity changed")
    return VerifiedOutput(path, stream, checksum, after)


def _verify_output_identity(output: VerifiedOutput) -> None:
    path_info = output.path.lstat()
    if (
        not stat.S_ISREG(path_info.st_mode)
        or _state(path_info) != output.state
        or _state(os.fstat(output.stream.fileno())) != output.state
    ):
        raise ValueError("generated output changed before completion")


def _remove_owned_outputs(outputs: list[tuple[Path, tuple[int, int, int, int, int, int]]]) -> None:
    for path, created_state in outputs:
        try:
            if _state(path.lstat())[:2] == created_state[:2]:
                path.unlink()
        except FileNotFoundError:
            continue


def _write_outputs(
    archive_path: Path,
    manifest_path: Path,
    runtime_deps: StableInput,
    records: list[FileRecord],
    manifest: bytes,
) -> tuple[str, str]:
    created: list[tuple[Path, tuple[int, int, int, int, int, int]]] = []
    try:
        with ExitStack() as stack:
            streams: list[BinaryIO] = []
            for path in (archive_path, manifest_path):
                descriptor = os.open(
                    path,
                    os.O_RDWR | os.O_CREAT | os.O_EXCL,
                    0o644,
                )
                created.append((path, _state(os.fstat(descriptor))))
                streams.append(stack.enter_context(os.fdopen(descriptor, "w+b")))
            archive_stream, manifest_stream = streams
            manifest_stream.write(manifest)
            manifest_stream.flush()
            with gzip.GzipFile(
                filename="",
                mode="wb",
                fileobj=archive_stream,
                mtime=0,
            ) as compressed:
                with tarfile.open(
                    fileobj=compressed,
                    mode="w",
                    format=tarfile.PAX_FORMAT,
                ) as archive:
                    for record in records:
                        _add_file(archive, record)
                    _add_bytes(archive, _MANIFEST_NAME, manifest)
            archive_output = _verify_output(archive_path, archive_stream, created[0][1])
            manifest_output = _verify_output(manifest_path, manifest_stream, created[1][1])
            _verify_inputs(runtime_deps, records)
            _verify_output_identity(archive_output)
            _verify_output_identity(manifest_output)
            return archive_output.sha256, manifest_output.sha256
    except BaseException:
        _remove_owned_outputs(created)
        raise


def _output_path(path: Path) -> Path:
    if (
        not path.is_absolute()
        or path.parent.resolve(strict=True) != path.parent
        or path.resolve(strict=False) != path
        or not path.parent.is_dir()
    ):
        raise ValueError("output path is not canonical")
    return path


def create_artifact(
    runtime_deps: Path,
    gn_out: Path,
    license_file: Path,
    archive_path: Path,
    manifest_path: Path,
) -> ArtifactSummary:
    runtime_deps = _canonical_file(runtime_deps)
    gn_out = _canonical_directory(gn_out)
    license_file = _canonical_file(license_file)
    if license_file.name != _LICENSE_NAME:
        raise ValueError("license input has an unexpected name")
    archive_path = _output_path(archive_path)
    manifest_path = _output_path(manifest_path)
    if archive_path == manifest_path:
        raise ValueError("archive and manifest outputs must differ")

    runtime_data, runtime_snapshot = _read_stable(runtime_deps)
    paths = _runtime_paths(runtime_data)
    if _LICENSE_NAME in paths or _MANIFEST_NAME in paths:
        raise ValueError("runtime dependency collides with a fixed archive member")
    records = [_collect_file(gn_out, value, value) for value in paths]
    license_root = _canonical_directory(license_file.parent)
    records.append(_collect_file(license_root, license_file.name, _LICENSE_NAME))
    records.sort(key=lambda record: record.archive_path)
    manifest = _manifest_bytes(records)
    archive_sha256, manifest_sha256 = _write_outputs(
        archive_path,
        manifest_path,
        runtime_snapshot,
        records,
        manifest,
    )
    return ArtifactSummary(
        archive_sha256=archive_sha256,
        manifest_sha256=manifest_sha256,
        file_count=len(records),
    )


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-deps", type=Path, required=True)
    parser.add_argument("--gn-out", type=Path, required=True)
    parser.add_argument("--license-file", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args(arguments)
    try:
        result = create_artifact(
            args.runtime_deps,
            args.gn_out,
            args.license_file,
            args.archive,
            args.manifest,
        )
    except (OSError, ValueError, RuntimeError, tarfile.TarError) as error:
        reason = (
            error.strerror or "filesystem operation failed"
            if isinstance(error, OSError)
            else str(error)
        )
        print(
            f"observer artifact creation failed: {type(error).__name__}: {reason}",
            file=sys.stderr,
        )
        return 1
    print(
        json.dumps(
            {
                "archiveSha256": result.archive_sha256,
                "manifestSha256": result.manifest_sha256,
                "fileCount": result.file_count,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
