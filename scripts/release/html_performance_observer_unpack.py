"""Validate and unpack a pinned HTML observer archive into a new directory."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
from typing import IO

import html_performance_observer_artifact as artifact


def _object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate manifest key")
        result[key] = value
    return result


def _path(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("member path is not text")
    path = PurePosixPath(value)
    if (not value or "\\" in value or path.is_absolute() or path.as_posix() != value
            or any(part in ("", ".", "..") for part in path.parts)):
        raise ValueError("noncanonical member path")
    return value


def _sha(value: object) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ValueError("invalid SHA256")
    return value


def _manifest(data: bytes, expected: str) -> dict[str, tuple[str, int, int]]:
    if hashlib.sha256(data).hexdigest() != _sha(expected):
        raise ValueError("manifest SHA differs")
    value = json.loads(data, object_pairs_hook=_object)
    if (not isinstance(value, dict) or set(value) != {"schema", "files"}
            or value["schema"] != artifact._SCHEMA or not isinstance(value["files"], list)):
        raise ValueError("manifest schema differs")
    records: dict[str, tuple[str, int, int]] = {}
    for row in value["files"]:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size", "mode"}:
            raise ValueError("manifest row differs")
        path = _path(row["path"])
        mode, size = row["mode"], row["size"]
        if (path == artifact._MANIFEST_NAME or path in records or type(size) is not int or size < 0
                or not isinstance(mode, str) or re.fullmatch(r"[0-7]{4}", mode) is None):
            raise ValueError("manifest row invalid")
        records[path] = (_sha(row["sha256"]), size, int(mode, 8))
    if not records or "headless_shell" not in records or artifact._LICENSE_NAME not in records:
        raise ValueError("required observer files missing")
    return records


def _members(archive: tarfile.TarFile) -> dict[str, tarfile.TarInfo]:
    result: dict[str, tarfile.TarInfo] = {}
    for item in archive.getmembers():
        name = _path(item.name)
        if (name in result or item.type not in (tarfile.REGTYPE, tarfile.AREGTYPE)
                or item.sparse is not None or item.linkname or item.devmajor or item.devminor
                or item.uid != 0 or item.gid != 0 or item.uname != "" or item.gname != "" or item.mtime != 0
                or set(item.pax_headers) - {"path"}
                or ("path" in item.pax_headers and item.pax_headers["path"] != name)):
            raise ValueError("archive member or metadata invalid")
        result[name] = item
    return result


def _write_member(root_fd: int, name: str, source: IO[bytes], mode: int) -> tuple[str, int]:
    parts = PurePosixPath(name).parts
    directory = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            try:
                os.mkdir(part, 0o755, dir_fd=directory)
            except FileExistsError:
                pass
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory)
            os.close(directory)
            directory = child
        descriptor = os.open(parts[-1], os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=directory)
        with os.fdopen(descriptor, "wb") as target:
            digest = hashlib.sha256()
            count = 0
            while chunk := source.read(artifact._BUFFER_SIZE):
                target.write(chunk)
                digest.update(chunk)
                count += len(chunk)
            target.flush()
            os.fchmod(target.fileno(), mode)
            return digest.hexdigest(), count
    finally:
        os.close(directory)


def _verify_inventory(root_fd: int, records: dict[str, tuple[str, int, int]]) -> None:
    directories: set[str] = set()
    for name in records:
        parts = PurePosixPath(name).parts
        directories.update("/".join(parts[:index]) for index in range(1, len(parts)))
    seen_files: set[str] = set()
    seen_directories: set[str] = set()

    def visit(directory_fd: int, prefix: str) -> None:
        with os.scandir(directory_fd) as entries:
            for entry in entries:
                relative = f"{prefix}/{entry.name}" if prefix else entry.name
                info = entry.stat(follow_symlinks=False)
                if stat.S_ISDIR(info.st_mode) and relative in directories:
                    seen_directories.add(relative)
                    child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                    dir_fd=directory_fd)
                    try:
                        visit(child, relative)
                    finally:
                        os.close(child)
                elif stat.S_ISREG(info.st_mode) and relative in records:
                    seen_files.add(relative)
                else:
                    raise ValueError("unexpected extracted member")

    visit(root_fd, "")
    if seen_files != set(records) or seen_directories != directories:
        raise ValueError("extracted inventory differs")


def unpack(archive_path: Path, expected_archive: str, expected_manifest: str,
           expected_executable: str, destination: Path) -> int:
    if (not hasattr(os, "O_NOFOLLOW") or not hasattr(os, "O_DIRECTORY")
            or not hasattr(os, "fchmod") or os.open not in os.supports_dir_fd
            or os.mkdir not in os.supports_dir_fd):
        raise RuntimeError("secure relative file access is unavailable")
    artifact._canonical_file(archive_path)
    _sha(expected_archive)
    _sha(expected_manifest)
    _sha(expected_executable)
    if (not destination.is_absolute() or destination.parent.resolve(strict=True) != destination.parent
            or destination.resolve(strict=False) != destination or not destination.parent.is_dir()):
        raise ValueError("destination is not canonical")
    descriptor = os.open(archive_path, os.O_RDONLY | os.O_NOFOLLOW)
    created: tuple[int, int] | None = None
    with os.fdopen(descriptor, "rb") as stream:
        before = artifact._state(os.fstat(stream.fileno()))
        checksum, _ = artifact._hash_descriptor(stream.fileno())
        if checksum != expected_archive or artifact._state(archive_path.lstat()) != before:
            raise ValueError("archive identity differs")
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode="r:gz") as archive:
            members = _members(archive)
            manifest = members.get(artifact._MANIFEST_NAME)
            if manifest is None or manifest.mode != 0o444:
                raise ValueError("manifest member missing")
            manifest_file = archive.extractfile(manifest)
            if manifest_file is None:
                raise ValueError("manifest bytes missing")
            with manifest_file:
                records = _manifest(manifest_file.read(), expected_manifest)
            if set(members) != set(records) | {artifact._MANIFEST_NAME}:
                raise ValueError("archive inventory differs")
            executable = records["headless_shell"]
            if executable[0] != expected_executable or not executable[2] & 0o111:
                raise ValueError("observer executable differs")
            for name, (_, size, mode) in records.items():
                item = members[name]
                if item.size != size or item.mode != mode:
                    raise ValueError("archive member metadata differs")
            os.mkdir(destination, 0o700)
            root_stat = destination.lstat()
            created = (root_stat.st_dev, root_stat.st_ino)
            root_fd = os.open(destination, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                opened = os.fstat(root_fd)
                if (opened.st_dev, opened.st_ino) != created:
                    raise ValueError("destination changed before extraction")
                for name, (digest, size, mode) in records.items():
                    data = archive.extractfile(members[name])
                    if data is None:
                        raise ValueError("archive member bytes missing")
                    with data:
                        actual, count = _write_member(root_fd, name, data, mode)
                    if actual != digest or count != size:
                        raise ValueError("archive member digest differs")
                for name, (digest, size, mode) in records.items():
                    descriptor = artifact._open_relative(destination, name)
                    try:
                        before_file = os.fstat(descriptor)
                        actual, count = artifact._hash_descriptor(descriptor)
                        after_file = os.fstat(descriptor)
                        if (not stat.S_ISREG(before_file.st_mode)
                                or stat.S_IMODE(before_file.st_mode) != mode
                                or artifact._state(before_file) != artifact._state(after_file)
                                or artifact._relative_state(destination, name) != artifact._state(after_file)
                                or actual != digest or count != size):
                            raise ValueError("extracted member changed")
                    finally:
                        os.close(descriptor)
                _verify_inventory(root_fd, records)
            finally:
                os.close(root_fd)
        final_digest, _ = artifact._hash_descriptor(stream.fileno())
        if (final_digest != expected_archive or artifact._state(os.fstat(stream.fileno())) != before
                or artifact._state(archive_path.lstat()) != before):
            raise ValueError("archive changed during extraction")
        current = destination.lstat()
        if created != (current.st_dev, current.st_ino) or not stat.S_ISDIR(current.st_mode):
            raise ValueError("destination changed during extraction")
        return len(records)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--archive-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--executable-sha256", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    try:
        count = unpack(args.archive, args.archive_sha256, args.manifest_sha256,
                       args.executable_sha256, args.output_dir)
    except (OSError, ValueError, TypeError, KeyError, RuntimeError, tarfile.TarError):
        print("observer archive extraction failed", file=sys.stderr)
        return 1
    print(json.dumps({"fileCount": count}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
