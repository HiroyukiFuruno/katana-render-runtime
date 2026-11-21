from __future__ import annotations

import os
import posixpath
import stat
from pathlib import Path, PurePosixPath


class LocalCoverageContextError(ValueError):
    """Raised internally when the supported local host cannot be proven."""


def _file_identity(metadata: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _internal_file_symlink_entry(
    root: Path,
    directory_fd: int,
    name: str,
    relative: str,
    before: os.stat_result,
) -> tuple[dict[str, object], str]:
    try:
        target = os.readlink(name, dir_fd=directory_fd)
        resolved_relative = _resolve_internal_regular_target(root, relative, target)
        _require_stable_symlink(directory_fd, name, before, target)
    except LocalCoverageContextError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
    return (
        {
            "path": relative,
            "type": "symlink-file",
            "target": target,
            "resolvedTarget": resolved_relative,
        },
        resolved_relative,
    )


def _resolve_internal_regular_target(root: Path, relative: str, target: str) -> str:
    parent = posixpath.dirname(relative)
    normalized = posixpath.normpath(posixpath.join(parent, target))
    if (
        not target
        or PurePosixPath(target).is_absolute()
        or normalized in {".", ".."}
        or normalized.startswith("../")
    ):
        raise LocalCoverageContextError("cargo_dependency_source_symlink")
    target_path = root.joinpath(*PurePosixPath(normalized).parts)
    resolved = target_path.resolve(strict=True)
    try:
        resolved_relative = resolved.relative_to(root).as_posix()
    except ValueError as error:
        raise LocalCoverageContextError("cargo_dependency_source_symlink") from error
    if resolved_relative != normalized or not stat.S_ISREG(target_path.lstat().st_mode):
        raise LocalCoverageContextError("cargo_dependency_source_symlink")
    return resolved_relative


def _require_stable_symlink(
    directory_fd: int,
    name: str,
    before: os.stat_result,
    target: str,
) -> None:
    after = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if _file_identity(after) != _file_identity(before):
        raise LocalCoverageContextError("cargo_dependency_source_changed")
    if os.readlink(name, dir_fd=directory_fd) != target:
        raise LocalCoverageContextError("cargo_dependency_source_changed")
    after_readlink = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if _file_identity(after_readlink) != _file_identity(before):
        raise LocalCoverageContextError("cargo_dependency_source_changed")
