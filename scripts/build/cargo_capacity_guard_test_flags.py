from __future__ import annotations

from collections.abc import Iterator, Sequence
from contextlib import contextmanager
from pathlib import Path
import os
import sys


@contextmanager
def native_file_flags(paths: Sequence[Path], flags: int) -> Iterator[None]:
    if sys.platform == "linux" or sys.platform == "win32":
        raise RuntimeError("native BSD file flags are unavailable")
    changed: list[Path] = []
    try:
        for path in paths:
            os.chflags(path, flags)
            changed.append(path)
        yield
    finally:
        for path in reversed(changed):
            os.chflags(path, 0)
