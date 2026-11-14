from __future__ import annotations

from contextlib import contextmanager
import errno
import os
from pathlib import Path
from collections.abc import Iterator
from typing import Protocol, runtime_checkable

from local_review_state import ReviewError


@runtime_checkable
class _WindowsLockApi(Protocol):
    LK_NBLCK: int

    def locking(self, descriptor: int, mode: int, count: int) -> None: ...


@contextmanager
def review_lock(path: Path) -> Iterator[None]:
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        try:
            if os.name == "nt":
                import msvcrt

                windows_lock: object = msvcrt
                if (not isinstance(windows_lock, _WindowsLockApi)
                        or type(windows_lock.LK_NBLCK) is not int
                        or not callable(windows_lock.locking)):
                    raise OSError("Windows file-lock API is unavailable")
                windows_lock.locking(descriptor, windows_lock.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            if error.errno in (errno.EACCES, errno.EAGAIN):
                raise ReviewError("another local review is active; do not start recursive reviews") from error
            raise
        yield
    finally:
        # 同じinodeを維持し、待機者と別のファイルへ排他が分裂することを防ぐ。
        # OSはclose・異常終了のどちらでもロックを解放する。
        os.close(descriptor)
