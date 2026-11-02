"""測定済みsourceと文書のみ異なる現sourceの対応を検証する。"""
from __future__ import annotations

import hashlib
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

ALLOWED = frozenset({
    "docs/performance/html-performance-history.json",
    "docs/performance/README.md",
    "docs/quality-gate-improvement-tasks.md",
})
Records = dict[str, tuple[str, int]]
GitBytes = Callable[[Sequence[str]], bytes]
GitText = Callable[[Sequence[str]], str]


@dataclass(frozen=True)
class SourceReuseContext:
    measured_head: str
    eligible_head: str
    measured_records: Mapping[str, tuple[str, int]]
    changed_paths: tuple[str, ...]


def _modes(raw: bytes) -> dict[str, str]:
    result: dict[str, str] = {}
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, name = entry.split(b"\t", 1)
        mode, kind, _oid = metadata.decode("ascii").split()
        path = name.decode("utf8")
        if kind != "blob" or mode not in ("100644", "100755") or path in result:
            raise ValueError("unsupported source tree entry")
        result[path] = mode
    return result


def validate_reuse_source(root: Path, measured_head: str, current: Records,
                          measured: Records, git_text: GitText,
                          git_bytes: GitBytes) -> SourceReuseContext:
    eligible = git_text(("rev-parse", "HEAD")).strip()
    if git_text(("merge-base", measured_head, eligible)).strip() != measured_head:
        raise ValueError("measured HEAD is not an ancestor")
    if git_text(("status", "--porcelain=v1", "--untracked-files=all")):
        raise ValueError("release source is dirty or untracked")
    old_modes = _modes(git_bytes(("ls-tree", "-r", "-z", measured_head)))
    new_modes = _modes(git_bytes(("ls-tree", "-r", "-z", eligible)))
    if old_modes != new_modes or set(current) != set(measured) or set(current) != set(new_modes):
        raise ValueError("source membership or mode differs")
    changed = tuple(sorted(path for path in current if current[path] != measured[path]))
    if not set(changed).issubset(ALLOWED):
        raise ValueError("source changes exceed documentation allowance")
    if any(old_modes.get(path) != "100644" for path in changed):
        raise ValueError("changed documentation is not a regular non-executable file")
    for name, (checksum, size) in current.items():
        path = root / name
        if (not path.is_file() or path.is_symlink() or path.resolve() != path
                or path.stat().st_size != size
                or hashlib.sha256(path.read_bytes()).hexdigest() != checksum):
            raise ValueError("working source differs from current committed tree: " + name)
    return SourceReuseContext(measured_head, eligible, dict(measured), changed)
