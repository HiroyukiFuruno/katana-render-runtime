from __future__ import annotations

from contextlib import contextmanager
import errno
import os
from pathlib import Path
from typing import Iterator

from local_review_state import ReviewError


@contextmanager
def review_lock(path: Path) -> Iterator[None]:
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        try:
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(descriptor, msvcrt.LK_NBLCK, 1)
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
