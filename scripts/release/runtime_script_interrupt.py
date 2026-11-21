"""Defer CLI interruption until a producer child has been reaped."""

from __future__ import annotations

import signal
import subprocess
from contextlib import contextmanager
from collections.abc import Iterator

_pending: int | None = None


def _interrupt(signum: int, _frame: object) -> None:
    global _pending
    _pending = signum


def wait_for_child(child: subprocess.Popen[bytes]) -> int:
    while True:
        if _pending is not None:
            try:
                child.terminate()
            except ProcessLookupError:
                pass
            try:
                return child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    child.kill()
                except ProcessLookupError:
                    pass
                return child.wait()
        try:
            return child.wait(timeout=0.1)
        except subprocess.TimeoutExpired:
            continue


def raise_if_interrupted() -> None:
    if _pending is not None:
        raise SystemExit(128 + _pending)


@contextmanager
def interruptible() -> Iterator[None]:
    global _pending
    _pending = None
    previous = {sig: signal.signal(sig, _interrupt) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        yield
        raise_if_interrupted()
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        _pending = None
