"""Owned, recoverable scratch space for HTML performance transport."""
from __future__ import annotations

import os
import re
import shutil
import signal
import stat
import uuid
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from pathlib import Path
from types import FrameType

_NAMESPACE = "html-performance-transport-scratch"
_WORK = re.compile(r"work-(\d+)-(\d+)-([0-9a-f]{32})\Z")
_BOOTSTRAP = re.compile(r"bootstrap-[0-9a-f]{32}\Z")


def _private_directory(path: Path) -> os.stat_result:
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or path.is_junction():
        raise ValueError("transport scratch directory identity differs")
    if os.name != "nt" and (stat.S_IMODE(info.st_mode) != 0o700 or info.st_uid != os.getuid()):
        raise ValueError("transport scratch directory identity differs")
    return info


def _namespace(tmp: Path) -> Path:
    if not tmp.is_absolute() or tmp.resolve(strict=True) != tmp or not tmp.is_dir():
        raise ValueError("transport scratch parent is not a physical directory")
    directory = tmp / _NAMESPACE
    directory.mkdir(mode=0o700, exist_ok=True)
    _private_directory(directory)
    return directory


def _safe_contents(directory: Path) -> bool:
    for entry in directory.iterdir():
        info = entry.lstat()
        if stat.S_ISLNK(info.st_mode) or entry.is_junction():
            return False
        if os.name != "nt" and info.st_uid != os.getuid():
            return False
        if stat.S_ISDIR(info.st_mode):
            if not _safe_contents(entry):
                return False
        elif not stat.S_ISREG(info.st_mode):
            return False
    return True


def _remove_owned(directory: Path, expected: tuple[int, int], parent: tuple[int, int],
                  *, empty: bool = False) -> None:
    try:
        parent_info = _private_directory(directory.parent)
        if (parent_info.st_dev, parent_info.st_ino) != parent:
            return
        info = _private_directory(directory)
        if (info.st_dev, info.st_ino) != expected:
            return
        if empty:
            directory.rmdir()
        elif _safe_contents(directory):
            parent_info = _private_directory(directory.parent)
            info = _private_directory(directory)
            if ((parent_info.st_dev, parent_info.st_ino) == parent
                    and (info.st_dev, info.st_ino) == expected):
                shutil.rmtree(directory)
    except FileNotFoundError:
        return
    except OSError:
        if empty:
            return
        raise


def recover_transport_scratch(tmp: Path) -> None:
    if not os.path.lexists(tmp / _NAMESPACE):
        return
    namespace = _namespace(tmp)
    parent_info = _private_directory(namespace)
    parent = (parent_info.st_dev, parent_info.st_ino)
    for directory in namespace.iterdir():
        match = _WORK.fullmatch(directory.name)
        try:
            info = _private_directory(directory)
        except (FileNotFoundError, ValueError):
            continue
        if match is not None:
            _remove_owned(directory, (int(match[1]), int(match[2])), parent)
        elif _BOOTSTRAP.fullmatch(directory.name):
            _remove_owned(directory, (info.st_dev, info.st_ino), parent, empty=True)


@contextmanager
def transport_workspace(tmp: Path) -> Iterator[Path]:
    namespace = _namespace(tmp)
    parent_info = _private_directory(namespace)
    parent = (parent_info.st_dev, parent_info.st_ino)
    previous: dict[int, Callable[[int, FrameType | None], object] | int | None] = {}
    stop_signals = {signal.SIGTERM} if os.name == "nt" else {signal.SIGTERM, signal.SIGHUP}
    bootstrap: Path | None = None
    workspace: Path | None = None
    identity: tuple[int, int] | None = None

    def interrupted(signum: int, frame: FrameType | None) -> None:
        del frame
        raise InterruptedError(f"transport interrupted by signal {signum}")

    try:
        for signum in stop_signals:
            old = signal.getsignal(signum)
            if old != signal.SIG_IGN:
                previous[signum] = signal.signal(signum, interrupted)
        token = uuid.uuid4().hex
        bootstrap = namespace / f"bootstrap-{token}"
        bootstrap.mkdir(mode=0o700)
        info = _private_directory(bootstrap)
        identity = (info.st_dev, info.st_ino)
        workspace = namespace / f"work-{info.st_dev}-{info.st_ino}-{token}"
        bootstrap.rename(workspace)
        yield workspace
    finally:
        blocked = (signal.pthread_sigmask(signal.SIG_BLOCK, stop_signals | {signal.SIGINT})
                   if os.name != "nt" else None)
        try:
            if workspace is not None and identity is not None:
                _remove_owned(workspace, identity, parent)
            if bootstrap is not None and identity is not None:
                _remove_owned(bootstrap, identity, parent, empty=True)
        finally:
            try:
                for restored_signum, handler in previous.items():
                    if handler is not None:
                        signal.signal(restored_signum, handler)
            finally:
                if blocked is not None:
                    signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
