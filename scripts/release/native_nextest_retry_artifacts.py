#!/usr/bin/env python3
"""Bind reusable Nextest records to the complete local artifact trees."""

from __future__ import annotations

import os
import re
import stat
import uuid
from pathlib import Path
from typing import cast

from native_coverage_local_context import (
    LocalCoverageContextError,
    _dependency_tree_digest,
    _file_identity,
)

SCHEMA = "krr-native-nextest-retry-artifacts-v1"
TREE_PATHS = {
    "nextest": Path("target/nextest-root-state"),
    "llvmCov": Path("target/llvm-cov-target"),
}
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


def _canonical_uuid(value: object) -> bool:
    if not isinstance(value, str):
        return False
    try:
        return str(uuid.UUID(value)) == value
    except ValueError:
        return False


def _real_directory(path: Path) -> Path:
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("artifact directory path must be canonical and absolute")
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        try:
            metadata = current.lstat()
        except OSError as error:
            raise ValueError(f"artifact directory is unavailable: {current}") from error
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ValueError(
                f"artifact directory has a symlink or non-directory ancestor: {current}"
            )
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"artifact directory is unavailable: {path}") from error
    if resolved != path:
        raise ValueError("artifact directory is not canonical")
    return path


def _open_directory(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        return os.open(path, flags)
    except OSError as error:
        raise ValueError(
            f"artifact directory cannot be opened safely: {path}"
        ) from error


def _scan_run_directories(directory_fd: int, found: set[str]) -> None:
    before_directory = os.fstat(directory_fd)
    scan_fd = os.dup(directory_fd)
    try:
        with os.scandir(scan_fd) as iterator:
            names = sorted((entry.name for entry in iterator), key=os.fsencode)
    except OSError as error:
        raise ValueError("Nextest record tree cannot be read safely") from error
    finally:
        os.close(scan_fd)

    for name in names:
        try:
            before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except OSError as error:
            raise ValueError("Nextest record tree changed while scanning") from error
        if stat.S_ISLNK(before.st_mode):
            raise ValueError("Nextest record tree contains a symlink")
        if stat.S_ISDIR(before.st_mode):
            flags = (
                os.O_RDONLY
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0)
            )
            try:
                child_fd = os.open(name, flags, dir_fd=directory_fd)
            except OSError as error:
                raise ValueError(
                    "Nextest record tree changed while scanning"
                ) from error
            try:
                if _file_identity(os.fstat(child_fd)) != _file_identity(before):
                    raise ValueError("Nextest record tree changed while scanning")
                if _canonical_uuid(name) and _has_regular_store(child_fd):
                    found.add(name)
                _scan_run_directories(child_fd, found)
                if _file_identity(os.fstat(child_fd)) != _file_identity(before):
                    raise ValueError("Nextest record tree changed while scanning")
            finally:
                os.close(child_fd)
        elif not stat.S_ISREG(before.st_mode):
            raise ValueError("Nextest record tree contains an unsupported entry")
    if _file_identity(os.fstat(directory_fd)) != _file_identity(before_directory):
        raise ValueError("Nextest record tree changed while scanning")


def _has_regular_store(run_fd: int) -> bool:
    try:
        metadata = os.stat("store.zip", dir_fd=run_fd, follow_symlinks=False)
    except FileNotFoundError:
        return False
    except OSError as error:
        raise ValueError("Nextest store.zip cannot be inspected safely") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise ValueError("Nextest store.zip is a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError("Nextest store.zip is not a regular file")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        file_fd = os.open("store.zip", flags, dir_fd=run_fd)
    except OSError as error:
        raise ValueError("Nextest store.zip changed while scanning") from error
    try:
        if _file_identity(os.fstat(file_fd)) != _file_identity(metadata):
            raise ValueError("Nextest store.zip changed while scanning")
    finally:
        os.close(file_fd)
    return True


def recorded_run_ids(repo: Path) -> set[str]:
    root = _real_directory(repo / TREE_PATHS["nextest"])
    root_fd = _open_directory(root)
    found: set[str] = set()
    try:
        _scan_run_directories(root_fd, found)
    finally:
        os.close(root_fd)
    return found


def _tree_snapshot(repo: Path, relative: Path) -> dict[str, object]:
    root = _real_directory(repo / relative)
    try:
        digest, entry_count, total_bytes = _dependency_tree_digest(root)
    except LocalCoverageContextError as error:
        raise ValueError(f"artifact tree is unsupported: {relative}") from error
    _real_directory(root)
    return {"sha256": digest, "entryCount": entry_count, "totalBytes": total_bytes}


def snapshot(repo: Path, run_id: str) -> dict[str, object]:
    if not _canonical_uuid(run_id):
        raise ValueError("runId must be a canonical full UUID")
    if run_id not in recorded_run_ids(repo):
        raise ValueError("runId has no regular Nextest store.zip")
    trees = {
        name: _tree_snapshot(repo, relative) for name, relative in TREE_PATHS.items()
    }
    if run_id not in recorded_run_ids(repo):
        raise ValueError("runId changed while artifact snapshot was collected")
    return {"schema": SCHEMA, "runId": run_id, "trees": trees}


def verify_snapshot(repo: Path, value: object) -> bool:
    if not isinstance(value, dict) or set(value) != {"schema", "runId", "trees"}:
        return False
    if value.get("schema") != SCHEMA or not _canonical_uuid(value.get("runId")):
        return False
    trees = value.get("trees")
    if not isinstance(trees, dict) or set(trees) != set(TREE_PATHS):
        return False
    for entry in trees.values():
        if not isinstance(entry, dict) or set(entry) != {
            "sha256",
            "entryCount",
            "totalBytes",
        }:
            return False
        digest = entry.get("sha256")
        entry_count = entry.get("entryCount")
        total_bytes = entry.get("totalBytes")
        if (
            not isinstance(digest, str)
            or _SHA256.fullmatch(digest) is None
            or not isinstance(entry_count, int)
            or isinstance(entry_count, bool)
            or entry_count < 1
            or not isinstance(total_bytes, int)
            or isinstance(total_bytes, bool)
            or total_bytes < 0
        ):
            return False
    try:
        current = snapshot(repo, cast(str, value["runId"]))
    except (OSError, ValueError):
        return False
    return current == value
