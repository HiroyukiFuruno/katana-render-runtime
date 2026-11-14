"""Hash directory inventories with bounded file-read concurrency."""
from __future__ import annotations

import os
import stat
from concurrent.futures import ThreadPoolExecutor
from functools import partial
from pathlib import Path

import verify_html_performance_evidence as gate

MAX_HASH_WORKERS = 8
FileStamp = tuple[int, int, int, int, int, int]


def _stamp(path: Path) -> FileStamp:
    metadata = path.lstat()
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _link_row(path: Path, root: Path) -> gate.JsonObject:
    before = _stamp(path)
    if not stat.S_ISLNK(before[2]):
        raise ValueError("inventory member changed from symlink")
    target = os.readlink(path)
    if before != _stamp(path):
        raise ValueError("inventory symlink changed during read")
    return {"path": str(path.relative_to(root)), "link": target}


def _file_row(path: Path, root: Path) -> gate.JsonObject:
    before = _stamp(path)
    if not stat.S_ISREG(before[2]):
        raise ValueError("inventory member changed from regular file")
    digest = gate.sha256(path)
    if before != _stamp(path):
        raise ValueError("inventory file changed during hash")
    return {
        "path": str(path.relative_to(root)),
        "sha256": digest,
        "size": before[3],
    }


def inventory(path: Path) -> gate.JsonObject:
    """Return the legacy inventory shape while hashing at most eight files at once."""
    if not path.exists():
        return {"root": str(path), "exists": False, "files": []}

    members = sorted(path.rglob("*"))
    rows_by_member: dict[Path, gate.JsonObject] = {}
    regular: list[Path] = []
    for member in members:
        mode = _stamp(member)[2]
        if stat.S_ISLNK(mode):
            rows_by_member[member] = _link_row(member, path)
        elif stat.S_ISREG(mode):
            regular.append(member)
    with ThreadPoolExecutor(max_workers=MAX_HASH_WORKERS) as executor:
        for offset in range(0, len(regular), MAX_HASH_WORKERS):
            batch = regular[offset : offset + MAX_HASH_WORKERS]
            worker = partial(_file_row, root=path)
            for member, row in zip(batch, executor.map(worker, batch), strict=True):
                rows_by_member[member] = row
    rows = [rows_by_member[member] for member in members if member in rows_by_member]
    return {"root": str(path), "exists": True, "files": rows}
