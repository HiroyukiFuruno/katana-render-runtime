"""HTML検証用の実行ファイルを、同一性を確認して読み取り専用に固定する。"""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat


def file_identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns


def _require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def _require_canonical_file(path: Path, label: str) -> None:
    _require(path.is_absolute() and path.resolve() == path and not path.is_symlink()
             and path.is_file(), label + " must be a canonical regular file")


def freeze_binary(source: Path, destination: Path) -> str:
    _require_canonical_file(source, "selected binary")
    _require(destination.is_absolute() and destination.parent.resolve() == destination.parent,
             "frozen destination must be canonical")
    no_follow: object = getattr(os, "O_NOFOLLOW", None)
    if isinstance(no_follow, bool) or not isinstance(no_follow, int) or no_follow <= 0:
        raise ValueError("binary freeze requires a no-follow file API")
    descriptor = os.open(source, os.O_RDONLY | no_follow)
    try:
        with os.fdopen(descriptor, "rb") as reader:
            descriptor = -1
            before = os.fstat(reader.fileno())
            _require(stat.S_ISREG(before.st_mode), "selected binary must be regular")
            destination.parent.mkdir(parents=True, exist_ok=False)
            checksum = hashlib.sha256()
            with destination.open("xb") as writer:
                while block := reader.read(1024 * 1024):
                    checksum.update(block)
                    writer.write(block)
            _require_canonical_file(destination, "frozen binary")
            frozen_identity = file_identity(destination.lstat())
            after = os.fstat(reader.fileno())
            _require(file_identity(before) == file_identity(after) == file_identity(source.lstat()),
                     "selected binary identity changed during freeze")
            reader.seek(0)
            original_hash = hashlib.sha256()
            while block := reader.read(1024 * 1024):
                original_hash.update(block)
            original_checksum = original_hash.hexdigest()
            _require(file_identity(before) == file_identity(os.fstat(reader.fileno()))
                     == file_identity(source.lstat()), "selected binary identity changed during freeze")
            with destination.open("rb") as frozen:
                frozen_checksum = hashlib.file_digest(frozen, "sha256").hexdigest()
            _require(original_checksum == checksum.hexdigest() == frozen_checksum, "frozen binary SHA differs")
            _require(file_identity(destination.lstat()) == frozen_identity, "frozen binary identity changed")
            destination.chmod(0o555)
            return checksum.hexdigest()
    finally:
        if descriptor >= 0:
            os.close(descriptor)
