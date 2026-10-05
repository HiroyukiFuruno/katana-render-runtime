#!/usr/bin/env python3
"""公開前アセットを含むパッケージ済みランタイムをオフラインで検査する。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys
import tarfile
import tomllib
from collections.abc import Callable, Sequence
from pathlib import Path, PurePosixPath
from typing import Union


ROOT = Path(__file__).resolve().parents[2]
ASSET_CRATE = "katana-render-runtime-assets"
RUNTIME_CRATE = "katana-render-runtime"
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
CommandRunner = Callable[
    ..., Union[subprocess.CompletedProcess[bytes], subprocess.CompletedProcess[str]]
]


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def _safe_relative_path(path: PurePosixPath, original: str) -> None:
    if (
        path.is_absolute()
        or ".." in path.parts
        or not path.parts
        or "\\" in original
        or any(":" in part for part in path.parts)
    ):
        raise ValueError(f"unsafe asset archive path: {original}")


def _safe_archive_path(name: str, crate_dir: str) -> PurePosixPath:
    path = PurePosixPath(name)
    _safe_relative_path(path, name)
    if path.parts[0] != crate_dir:
        raise ValueError(f"unsafe asset archive path: {name}")
    return path


def asset_archive_members(archive: Path, version: str) -> dict[str, bytes]:
    """展開時に危険な経路や特殊ファイルを含まない通常ファイルだけを読む。"""

    crate_dir = f"{ASSET_CRATE}-{version}"
    files: dict[str, bytes] = {}
    with tarfile.open(archive, mode="r:gz") as tar:
        for member in tar.getmembers():
            path = _safe_archive_path(member.name, crate_dir)
            if member.isdir():
                continue
            if not member.isfile():
                raise ValueError(f"unsupported asset archive member type: {member.name}")
            if len(path.parts) == 1:
                raise ValueError(
                    f"asset archive member lacks a package-relative path: {member.name}"
                )
            relative = PurePosixPath(*path.parts[1:]).as_posix()
            if relative in files:
                raise ValueError(f"duplicate asset archive path: {relative}")
            source = tar.extractfile(member)
            if source is None:
                raise ValueError(f"could not read asset archive member: {member.name}")
            files[relative] = source.read()
    if not files:
        raise ValueError("asset archive contains no regular files")
    file_paths = set(files)
    for relative in file_paths:
        parts = PurePosixPath(relative).parts
        conflicts = (
            PurePosixPath(*parts[:index]).as_posix() in file_paths
            for index in range(1, len(parts))
        )
        if any(conflicts):
            raise ValueError(f"conflicting asset archive file paths: {relative}")
    return files


def verify_asset_lock_entry(lock_path: Path, archive: Path, version: str) -> str:
    """パッケージ済みロックが同じ公開アーカイブを固定していることを確認する。"""

    lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    entries = [
        package
        for package in lock.get("package", [])
        if package.get("name") == ASSET_CRATE and package.get("version") == version
    ]
    if len(entries) != 1:
        raise ValueError(f"expected one {ASSET_CRATE} {version} entry in {lock_path}")
    entry = entries[0]
    if entry.get("source") != REGISTRY_SOURCE:
        raise ValueError(f"{ASSET_CRATE} lock entry must use the crates.io registry source")
    archive_sha = sha256_bytes(archive.read_bytes())
    if entry.get("checksum") != archive_sha:
        raise ValueError(
            f"{ASSET_CRATE} Cargo.lock checksum does not match packaged archive: "
            f"expected {entry.get('checksum')}, actual {archive_sha}"
        )
    return archive_sha


def _read_owned_vendor_manifest(destination: Path) -> dict[str, object] | None:
    if not destination.exists():
        return None
    if destination.is_symlink() or not destination.is_dir():
        raise ValueError(f"asset vendor destination is not a regular directory: {destination}")
    manifest_path = destination / ".cargo-checksum.json"
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise ValueError(f"refusing to replace unowned vendor directory: {destination}")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"invalid existing vendor checksum manifest: {manifest_path}") from error
    if not isinstance(manifest, dict) or not isinstance(manifest.get("files"), dict):
        raise ValueError(f"invalid existing vendor checksum manifest: {manifest_path}")
    if not isinstance(manifest.get("package"), str):
        raise ValueError(f"invalid existing vendor checksum manifest: {manifest_path}")

    expected_files = set(manifest["files"])
    if any(not isinstance(relative, str) for relative in expected_files):
        raise ValueError(f"invalid existing vendor checksum manifest: {manifest_path}")
    actual_files: set[str] = set()
    for path in destination.rglob("*"):
        relative = path.relative_to(destination).as_posix()
        if path.is_symlink():
            raise ValueError(f"refusing to replace vendor directory containing symlink: {path}")
        if path.is_file() and relative != ".cargo-checksum.json":
            actual_files.add(relative)
        elif not path.is_file() and not path.is_dir():
            raise ValueError(f"unsupported file in existing vendor directory: {path}")
    if actual_files != expected_files:
        raise ValueError(
            "existing vendor directory has files outside its checksum manifest: "
            f"{destination}"
        )
    for relative, expected in manifest["files"].items():
        relative_path = PurePosixPath(relative)
        try:
            _safe_relative_path(relative_path, relative)
        except ValueError as error:
            raise ValueError(f"unsafe existing vendor manifest path: {relative}") from error
        path = destination.joinpath(*relative_path.parts)
        if not isinstance(expected, str) or sha256_bytes(path.read_bytes()) != expected:
            raise ValueError(f"existing vendor file does not match its checksum manifest: {path}")
    return manifest


def stage_asset_archive(archive: Path, destination: Path, version: str) -> str:
    """Cargoのdirectory source向けに公開アーカイブのバイト列をそのまま配置する。"""

    archive_sha = sha256_bytes(archive.read_bytes())
    files = asset_archive_members(archive, version)
    old_manifest = _read_owned_vendor_manifest(destination)
    if old_manifest is not None:
        for relative in old_manifest["files"]:
            path = destination.joinpath(*PurePosixPath(relative).parts)
            path.unlink()
        (destination / ".cargo-checksum.json").unlink()

    destination.mkdir(parents=True, exist_ok=True)
    file_checksums: dict[str, str] = {}
    for relative, content in files.items():
        path = destination.joinpath(*PurePosixPath(relative).parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        file_checksums[relative] = sha256_bytes(content)
    manifest = {"files": file_checksums, "package": archive_sha}
    (destination / ".cargo-checksum.json").write_text(
        json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8"
    )
    return archive_sha


def parse_cargo_command(value: str) -> list[str]:
    if "\0" in value:
        raise ValueError("--cargo must not contain a NUL byte")
    try:
        command = shlex.split(value, posix=True)
    except ValueError as error:
        raise ValueError(f"invalid --cargo command: {error}") from error
    if not command:
        raise ValueError("--cargo must not be empty")
    return command


def packaged_test_command(
    cargo: Sequence[str], manifest: Path, vendor: Path, test_threads: int
) -> list[str]:
    return [
        *cargo,
        "test",
        "--manifest-path",
        str(manifest),
        "--lib",
        "--locked",
        "--offline",
        "--config",
        'source.crates-io.replace-with="packaged-runtime-test-vendor"',
        "--config",
        "source.packaged-runtime-test-vendor.directory=" + json.dumps(str(vendor)),
        "--",
        "--test-threads",
        str(test_threads),
    ]


def run_packaged_test(
    version: str,
    cargo: Sequence[str],
    test_threads: int = 1,
    runner: CommandRunner = subprocess.run,
) -> int:
    version_pattern = (
        r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\."
        r"(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"
    )
    if re.fullmatch(version_pattern, version) is None:
        raise ValueError(f"invalid bare crate version: {version}")
    package_dir = ROOT / "target" / "package"
    asset_archive = package_dir / f"{ASSET_CRATE}-{version}.crate"
    runtime_dir = package_dir / f"{RUNTIME_CRATE}-{version}"
    runtime_manifest = runtime_dir / "Cargo.toml"
    runtime_lock = runtime_dir / "Cargo.lock"
    vendor_dir = package_dir / "packaged-test-vendor"
    if not asset_archive.is_file():
        raise ValueError(f"packaged asset crate not found: {asset_archive}")
    if not runtime_manifest.is_file() or not runtime_lock.is_file():
        raise ValueError(f"packaged runtime manifest or lockfile not found under {runtime_dir}")

    verify_asset_lock_entry(runtime_lock, asset_archive, version)
    lock_before = runtime_lock.read_bytes()

    vendor_command = [
        *cargo,
        "vendor",
        "--locked",
        "--versioned-dirs",
        str(vendor_dir),
    ]
    vendor_result = runner(vendor_command, cwd=ROOT, check=False)
    if vendor_result.returncode:
        return vendor_result.returncode

    staged_asset = vendor_dir / f"{ASSET_CRATE}-{version}"
    stage_asset_archive(asset_archive, staged_asset, version)
    test_command = packaged_test_command(cargo, runtime_manifest, vendor_dir, test_threads)
    test_result = runner(test_command, cwd=ROOT, check=False)
    if runtime_lock.read_bytes() != lock_before:
        print(f"packaged runtime Cargo.lock changed during test: {runtime_lock}", file=sys.stderr)
        return test_result.returncode or 1
    return test_result.returncode


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="bare crate version, for example 0.4.23")
    parser.add_argument(
        "--cargo", default="cargo", help="configured cargo command, optionally with a wrapper"
    )
    parser.add_argument(
        "--test-threads", type=int, default=1, help="Rust test harness worker count"
    )
    args = parser.parse_args(argv)
    if args.test_threads < 1:
        parser.error("--test-threads must be at least 1")
    try:
        cargo = parse_cargo_command(args.cargo)
        return run_packaged_test(args.version, cargo, args.test_threads)
    except (OSError, tarfile.TarError, ValueError) as error:
        print(f"packaged runtime test setup failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
