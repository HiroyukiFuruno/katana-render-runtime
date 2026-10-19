"""Cross-platform locks and conservative active-process checks.

On Windows, process inspection is intentionally unavailable and cleanup is
skipped safely rather than attempting an unverified automatic sweep.
"""
from __future__ import annotations

import os
import re
import stat
import sys
import time
import uuid
from collections import Counter, defaultdict
import subprocess
from pathlib import Path
from collections.abc import Callable, Iterable, Iterator, Sequence
from contextlib import ExitStack, contextmanager
from typing import TYPE_CHECKING, Protocol, TextIO, TypedDict, runtime_checkable
from cargo_capacity_guard_build_script_ownership import (
    BuildScriptOwnership, OrphanBuildScript, file_stamp, orphan_build_scripts, verify_orphan_build_scripts,
)
from cargo_capacity_guard_selected_cache_ownership import (
    SelectedCacheOwnership, binary_cache_members,
)
from cargo_capacity_guard_library_alias_ownership import LibraryAliasOwnership, library_alias_groups

if TYPE_CHECKING:
    _Identity = tuple[int, int, int]
    _Inode = tuple[int, int]
    _Stamp = tuple[_Identity, int, int, int, int, int]
    _File = tuple[Path, os.stat_result]
    _Candidate = tuple[Path, Path, os.stat_result]
    _Directory = tuple[Path, _Stamp, list[str]]
    _Held = tuple[Path, int, _Identity]
    _Fingerprint = tuple[Path, Path, list[_File], list[_Directory]]
    _PathValue = str | os.PathLike[str]

KNOWN_PROFILE_NAMES = frozenset({"debug", "release", "krr-coverage"})


class _EvictionReportFields(TypedDict):
    candidate_paths: list[str]
    staged_paths: list[str]
    invalidated_profiles: list[str]
    retained_unknown_alias_inodes: int
    retained_protected_inodes: int
    estimated_candidate_bytes: int
    deleted_files: int
    retained_unknown_entries: int
    retained_unknown_alias_diagnostics: list[UnknownAliasDiagnostic]


class UnknownAliasDiagnostic(TypedDict):
    device: int
    inode: int
    st_nlink: int
    candidate_row_count: int
    unique_path_count: int
    paths: list[str]


class EvictionReport(_EvictionReportFields, total=False):
    retained_profiles: list[str]
    recovery_required_paths: list[str]
    recovery_required_profile: str
    retained_active_closures: int
    removed_incremental_sessions: int
    trimmed_incremental_sessions: int
    error: str


@runtime_checkable
class _PosixLockApi(Protocol):
    LOCK_EX: int
    LOCK_NB: int
    LOCK_UN: int

    def flock(self, descriptor: int, operation: int) -> None: ...

    def lockf(self, descriptor: int, operation: int, length: int = 0,
              start: int = 0, whence: int = 0) -> None: ...


@runtime_checkable
class _WindowsLockApi(Protocol):
    LK_LOCK: int
    LK_NBLCK: int
    LK_UNLCK: int

    def locking(self, descriptor: int, operation: int, count: int) -> None: ...


def _windows_lock_api() -> _WindowsLockApi:
    import msvcrt

    value: object = msvcrt
    if (not isinstance(value, _WindowsLockApi) or not callable(value.locking)
            or any(type(mode) is not int for mode in (value.LK_LOCK, value.LK_NBLCK, value.LK_UNLCK))):
        raise RuntimeError("Windows file-lock API is unavailable")
    return value


fcntl: _PosixLockApi | None

if os.name != "nt":
    import fcntl as _fcntl_module

    if not isinstance(_fcntl_module, _PosixLockApi):
        raise RuntimeError("POSIX file-lock API is unavailable")
    fcntl = _fcntl_module
else:  # pragma: no cover - exercised on Windows
    fcntl = None


class GuardLock:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.handle: TextIO | None = None

    def __enter__(self) -> GuardLock:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.handle = self.path.open("a+")
        self.handle.seek(0)
        try:
            if os.name == "nt":
                windows_lock = _windows_lock_api()
                windows_lock.locking(self.handle.fileno(), windows_lock.LK_LOCK, 1)
            else:
                if fcntl is None:
                    raise RuntimeError("POSIX file-lock API is unavailable")
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_EX)
        except BaseException:
            self.handle.close()
            self.handle = None
            raise
        return self

    def __exit__(self, *_: object) -> None:
        if self.handle is None:
            return
        try:
            self.handle.seek(0)
            if os.name == "nt":
                windows_lock = _windows_lock_api()
                windows_lock.locking(self.handle.fileno(), windows_lock.LK_UNLCK, 1)
            else:
                if fcntl is None:
                    raise RuntimeError("POSIX file-lock API is unavailable")
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_UN)
        finally:
            self.handle.close()
            self.handle = None


class ProfileLocks:
    def __init__(self, target: Path, input_check: Callable[[], None] | None = None) -> None:
        names = (".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock")
        paths = [target / name for name in names]
        if target.exists():
            paths.extend(path for path in target.rglob(".*-lock") if path.name in names)
        self.paths = list(dict.fromkeys(paths))
        self.handles: list[TextIO] = []
        self.identities: list[tuple[Path, _Identity]] = []
        self.target = target
        self.target_identity: _Identity | None = None
        self.ancestors: list[_Held] = []
        self.input_check = input_check

    def __enter__(self) -> ProfileLocks:
        try:
            cursor = Path(os.path.abspath(self.target))
            if os.name == "nt":
                self.target.mkdir(parents=True, exist_ok=True)
                for path in [*reversed(cursor.parents), cursor]:
                    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                    self.ancestors.append((path, descriptor, _identity(os.fstat(descriptor))))
            else:
                # clean直後のtargetは、検証済みparentのdir_fdから安全に作成する。
                current = Path(cursor.anchor)
                parent_fd = os.open(current, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                self.ancestors.append((current, parent_fd, _identity(os.fstat(parent_fd))))
                for name in cursor.parts[1:]:
                    current /= name
                    try:
                        descriptor = os.open(
                            name,
                            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=parent_fd,
                        )
                    except FileNotFoundError:
                        try:
                            os.mkdir(name, 0o755, dir_fd=parent_fd)
                        except FileExistsError:
                            pass
                        descriptor = os.open(
                            name,
                            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=parent_fd,
                        )
                    self.ancestors.append((current, descriptor, _identity(os.fstat(descriptor))))
                    parent_fd = descriptor
            self.target_identity = _identity(self.target.lstat())
            for profile in _profiles(self.target):
                self.paths.append(profile / ".cargo-lock")
            for path in dict.fromkeys(self.paths):
                if not path.exists() and path.name != ".cargo-lock":
                    continue
                descriptor = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
                handle = os.fdopen(descriptor, "a+")
                if not stat.S_ISREG(os.fstat(descriptor).st_mode):
                    handle.close()
                    raise BlockingIOError(str(path))
                try:
                    handle.seek(0)
                    if os.name == "nt":
                        windows_lock = _windows_lock_api()
                        windows_lock.locking(handle.fileno(), windows_lock.LK_NBLCK, 1)
                    else:
                        if fcntl is None:
                            raise RuntimeError("POSIX file-lock API is unavailable")
                        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                except OSError as error:
                    handle.close()
                    raise BlockingIOError(str(path)) from error
                except BaseException:
                    handle.close()
                    raise
                self.handles.append(handle)
                self.identities.append((path, _identity(os.fstat(handle.fileno()))))
        except BaseException:
            self._release()
            raise
        return self

    def verify(self) -> None:
        if self.input_check is not None:
            self.input_check()
        self._verify_identities()

    def _verify_identities(self) -> None:
        for path, descriptor, identity in self.ancestors:
            if (_identity(path.lstat()) != identity
                    or _identity(os.fstat(descriptor)) != identity):
                raise RuntimeError("target ancestor changed during cleanup")
        if _identity(self.target.lstat()) != self.target_identity:
            raise RuntimeError("target identity changed during cleanup")
        for index in range(min(len(self.handles), len(self.identities))):
            handle = self.handles[index]
            path, expected = self.identities[index]
            if (_identity(os.fstat(handle.fileno())) != expected
                    or _identity(path.lstat()) != expected):
                raise RuntimeError("native lock identity changed during cleanup")

    def _release(self) -> None:
        @contextmanager
        def release_handle(handle: TextIO) -> Iterator[None]:
            try:
                yield
            finally:
                try:
                    handle.seek(0)
                    if os.name == "nt":
                        windows_lock = _windows_lock_api()
                        windows_lock.locking(handle.fileno(), windows_lock.LK_UNLCK, 1)
                    else:
                        if fcntl is None:
                            raise RuntimeError("POSIX file-lock API is unavailable")
                        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
                finally:
                    handle.close()

        @contextmanager
        def release_ancestor(descriptor: int) -> Iterator[None]:
            try:
                yield
            finally:
                os.close(descriptor)
        # 一つのunlockが失敗しても、残handleとancestor FDを既存の逆順で全て解放する。
        with ExitStack() as cleanup:
            cleanup.callback(self.ancestors.clear)
            for _, descriptor, _ in self.ancestors:
                cleanup.enter_context(release_ancestor(descriptor))
            cleanup.callback(self.identities.clear)
            cleanup.callback(self.handles.clear)
            for handle in self.handles:
                cleanup.enter_context(release_handle(handle))

    def __exit__(self, *_: object) -> None:
        self._release()


def busy_processes(target: Path, root: Path) -> list[str]:
    """Return users of the target, failing closed when inspection is unavailable."""
    if os.name == "nt":
        return ["process inspection unavailable on Windows"]
    try:
        result = subprocess.run(["ps", "-axo", "pid=,ppid=,command="], text=True,
                                capture_output=True, check=False)
    except OSError as error:
        return [f"ps unavailable: {error}"]
    if result.returncode != 0:
        return [f"ps failed with exit code {result.returncode}"]
    needle = str(target)
    target_text = str(target)
    own = str(os.getpid())
    ancestry = {own}
    parent_by_pid = {}
    for line in result.stdout.splitlines():
        fields = line.strip().split(maxsplit=2)
        if len(fields) >= 2 and fields[0].isdigit() and fields[1].isdigit():
            parent_by_pid[fields[0]] = fields[1]
    cursor = own
    while cursor in parent_by_pid and parent_by_pid[cursor] not in ancestry:
        cursor = parent_by_pid[cursor]
        ancestry.add(cursor)
    busy = []
    for line in result.stdout.splitlines():
        fields = line.strip().split(maxsplit=2)
        pid = fields[0] if fields else ""
        if pid in ancestry:
            continue
        if needle in line:
            busy.append(f"pid {pid}: target in command")
            continue
        if len(fields) < 3:
            continue
        lower = line.lower()
        if not pid.isdigit() or not any(name in lower for name in ("cargo", "rustc", "rustdoc", "cargo-sweep")):
            continue
        try:
            location = subprocess.run(["lsof", "-a", "-p", pid, "-d", "cwd", "-Fn"],
                                      text=True, capture_output=True, check=False)
        except OSError as error:
            busy.append(f"pid {pid}: lsof unavailable: {error}")
            continue
        if location.returncode != 0:
            try:
                os.kill(int(pid), 0)
            except ProcessLookupError:
                # psの取得後に終了したプロセスだけは、lsof失敗を安全に無視できる。
                continue
            except OSError:
                pass
            busy.append(f"pid {pid}: lsof failed")
            continue
        cwd = next((item[1:] for item in location.stdout.splitlines() if item.startswith("n")), "")
        if needle in line or cwd == target_text or cwd.startswith(target_text + os.sep) or cwd == str(root):
            busy.append(f"pid {pid}: build target in use")
    return busy


def _identity(value: os.stat_result) -> _Identity:
    return value.st_dev, value.st_ino, value.st_mode


def _stamp(value: os.stat_result) -> _Stamp:
    return (_identity(value), value.st_nlink, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns, value.st_blocks)


def _is_profile(path: Path) -> bool:
    return (path / ".fingerprint").exists() or (path / "deps").is_dir() or bool(list(path.glob(".capacity-invalidated-*")))


def _checked_profile(path: Path, target: Path) -> bool:
    device = target.lstat().st_dev
    cursor = target
    for part in path.relative_to(target).parts:
        cursor /= part
        try:
            value = cursor.lstat()
        except FileNotFoundError:
            return False
        if not stat.S_ISDIR(value.st_mode) or value.st_dev != device:
            raise RuntimeError("profile directory kind or device boundary")
    return _is_profile(path)


def _profiles(target: Path) -> list[Path]:
    if not target.exists():
        return []
    profiles = []
    for child in target.iterdir():
        if not stat.S_ISDIR(child.lstat().st_mode):
            continue
        if _checked_profile(child, target):
            profiles.append(child)
        else:
            for suffix in ("debug", "release", "krr-coverage", "build/debug", "build/release"):
                profile = child / suffix
                if _checked_profile(profile, target):
                    profiles.append(profile)
    return profiles


def retained_profile_paths(target: Path, values: Iterable[_PathValue]) -> tuple[Path, ...]:
    profiles = []
    for value in values:
        path = Path(value)
        if not path.is_absolute():
            raise RuntimeError("retained profile must be an absolute path")
        try:
            parts = path.relative_to(target).parts
        except ValueError as exc:
            raise RuntimeError("retained profile escapes target") from exc
        if (not parts or parts[-1] not in KNOWN_PROFILE_NAMES
                or len(parts) > 3 or ".." in parts
                or (len(parts) == 3 and (parts[1] != "build" or parts[-1] not in {"debug", "release"}))):
            raise RuntimeError("retained profile is outside known profile scope")
        cursor = target
        for part in parts:
            cursor /= part
            try:
                info = cursor.lstat()
            except FileNotFoundError as exc:
                raise RuntimeError("retained profile does not exist") from exc
            if not stat.S_ISDIR(info.st_mode) or info.st_dev != target.lstat().st_dev:
                raise RuntimeError("retained profile directory kind or device boundary")
        profiles.append(path)
    return tuple(dict.fromkeys(profiles))


def _metadata_at(descriptor: int, name: str, device: int) -> os.stat_result:
    value = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
    if not stat.S_ISREG(value.st_mode) or value.st_dev != device:
        raise RuntimeError("artifact kind or device changed")
    return value


def _reference_metadata(path: Path, device: int) -> os.stat_result:
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        return _metadata_at(descriptor, path.name, device)
    finally:
        os.close(descriptor)


def _regular(path: Path, device: int) -> os.stat_result:
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        value = os.fstat(descriptor)
        if (not stat.S_ISREG(value.st_mode) or value.st_dev != device
                or _stamp(value) != _stamp(path.lstat())):
            raise RuntimeError("artifact kind, device or identity changed")
        return value
    finally:
        os.close(descriptor)


def _fingerprint_files(root: Path, device: int) -> tuple[list[_File], list[_Directory]]:
    files: list[_File] = []
    directories: list[_Directory] = []
    def visit(path: Path) -> None:
        descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            value = os.fstat(descriptor)
            if value.st_dev != device or _identity(value) != _identity(path.lstat()):
                raise RuntimeError("fingerprint directory identity changed")
            names = sorted(os.listdir(descriptor))
            directories.append((path, _stamp(value), names))
            for name in names:
                child = path / name
                child_stat = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                kind = child_stat.st_mode
                if stat.S_ISDIR(kind):
                    visit(child)
                elif stat.S_ISREG(kind):
                    files.append((child, _metadata_at(descriptor, name, device)))
                else:
                    raise RuntimeError("unknown fingerprint kind; artifacts retained")
            if _stamp(value) != _stamp(os.fstat(descriptor)) or names != sorted(os.listdir(descriptor)):
                raise RuntimeError("fingerprint changed during selection")
        finally:
            os.close(descriptor)
    visit(root)
    return files, directories


INCREMENTAL_INDEX_NAMES = {"dep-graph.bin", "query-cache.bin", "work-products.bin"}
_RUST_SESSION_NAME = re.compile(r"s-([0-9a-z]{10})-([0-9a-z]{7})-([0-9a-z]{25}|working)")
_RUST_SESSION_LOCK_PREFIX = re.compile(r"^(s-[0-9a-z]{10}-[0-9a-z]{7})-")
_WORKING_SESSION_FILE = "dep-graph.part.bin"


def _rust_session_parts(name: str) -> tuple[int, str] | None:
    match = _RUST_SESSION_NAME.fullmatch(name)
    if match is None:
        return None
    try:
        return int(match.group(1), 36), match.group(3)
    except ValueError:
        return None


def _rust_session_lock_name(name: str) -> str | None:
    match = _RUST_SESSION_LOCK_PREFIX.match(name)
    return None if match is None else f"{match.group(1)}.lock"


def _acquire_incremental_session_lock(
    owner: Path, owner_fd: int, session_name: str, device: int, protected: set[_Inode],
) -> tuple[int, Path, _Identity] | None:
    parts = _rust_session_parts(session_name)
    if parts is None or os.name == "nt" or fcntl is None:
        return None
    match = _RUST_SESSION_NAME.fullmatch(session_name)
    if match is None:
        return None
    lock_name = f"s-{match.group(1)}-{match.group(2)}.lock"
    descriptor: int | None = None
    try:
        before = os.stat(lock_name, dir_fd=owner_fd, follow_symlinks=False)
        if (not stat.S_ISREG(before.st_mode) or before.st_dev != device
                or before.st_nlink != 1):
            return None
        if (before.st_dev, before.st_ino) in protected:
            return None
        descriptor = os.open(lock_name, os.O_RDWR | os.O_NOFOLLOW, dir_fd=owner_fd)
        value = os.fstat(descriptor)
        if _identity(value) != _identity(before):
            os.close(descriptor)
            return None
        if sys.platform.startswith("linux"):
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        else:
            # rustc uses F_SETLK record locks on macOS and other non-Linux Unix.
            fcntl.lockf(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        after = os.stat(lock_name, dir_fd=owner_fd, follow_symlinks=False)
        if _identity(after) != _identity(value) or after.st_nlink != 1:
            os.close(descriptor)
            return None
        return descriptor, owner / lock_name, _identity(value)
    except (OSError, RuntimeError):
        if descriptor is not None:
            try:
                os.close(descriptor)
            except OSError:
                pass
        return None


def _known_session_members(
    session_fd: int, names: list[str], finalized: bool, cache_leaves_only: bool = False,
) -> bool:
    if finalized:
        indexes = INCREMENTAL_INDEX_NAMES.intersection(names)
        if cache_leaves_only:
            # graph欠落後の残cacheも再回収できるが、graphがある不完全な組は保持する。
            if "metadata.rmeta" not in names or ("dep-graph.bin" in indexes and indexes != INCREMENTAL_INDEX_NAMES):
                return False
        elif indexes != INCREMENTAL_INDEX_NAMES:
            return False
    else:
        if names != [_WORKING_SESSION_FILE]:
            return False
    for name in names:
        known = (name in INCREMENTAL_INDEX_NAMES or Path(name).suffix == ".o"
                 or cache_leaves_only and name == "metadata.rmeta") if finalized else name == _WORKING_SESSION_FILE
        if not known:
            return False
        try:
            value = os.stat(name, dir_fd=session_fd, follow_symlinks=False)
        except OSError:
            return False
        if not stat.S_ISREG(value.st_mode):
            return False
        if cache_leaves_only and name in INCREMENTAL_INDEX_NAMES and (
            value.st_nlink != 1 or getattr(value, "st_flags", 0) != 0
        ):
            return False
    return True


def _incremental_session(path: Path, profile: Path) -> Path | None:
    relative = path.relative_to(profile).parts
    if (len(relative) != 4 or relative[0] != "incremental"
            or re.fullmatch(r"s-[a-z0-9]+-[a-z0-9]+-[a-z0-9]+", relative[2]) is None
            or relative[2].endswith("-working")):
        return None
    return path.parent


def _incremental_index_closure(
    rows: Sequence[_Candidate], aliases: dict[_Inode, list[_Candidate]], sessions: dict[Path, Path],
    indexes: dict[Path, dict[str, _Candidate]], members: dict[Path, list[_Candidate]],
    protected: set[_Inode], cache_leaf_sessions: set[Path] | None = None,
    owned_members: dict[Path, list[_Candidate]] | None = None,
) -> dict[_Inode, list[_Candidate]] | None:
    pending = list(rows)
    closure: dict[_Inode, list[_Candidate]] = {}
    expanded_sessions: set[Path] = set()
    while pending:
        row = pending.pop()
        key = row[2].st_dev, row[2].st_ino
        if key in closure:
            continue
        linked = aliases[key]
        if key in protected or len(linked) != linked[0][2].st_nlink:
            return None
        closure[key] = linked
        for _, path, _ in linked:
            if owned_members is not None and path in owned_members:
                pending.extend(owned_members[path])
            session = sessions.get(path)
            if session is not None:
                index_rows = indexes[session]
                if (cache_leaf_sessions is None or session not in cache_leaf_sessions) and set(index_rows) != INCREMENTAL_INDEX_NAMES:
                    return None
                if session not in expanded_sessions:
                    expanded_sessions.add(session)
                    # WHY: indexだけを先に消すと、未選択の兄弟objectが次回unknownとして取り残される。
                    pending.extend(members[session])
    return closure


def evict_artifacts(
    target: Path, threshold: int, current_bytes: int, dry_run: bool,
    protected_refs: Sequence[_PathValue] = (), epoch: ProfileLocks | None = None,
    incremental_prefixes: Sequence[str] = (), inactive_prefixes: Sequence[str] = (),
    retained_profiles: Sequence[_PathValue] = (),
    auxiliary_only_profiles: Sequence[_PathValue] = (),
    unselected_incremental_prefixes: Sequence[str] = (),
    selected_package_profiles: Sequence[_PathValue] = (),
    build_script_ownership: BuildScriptOwnership | None = None,
    selected_cache_ownership: SelectedCacheOwnership | None = None,
    library_alias_ownership: LibraryAliasOwnership | None = None,
) -> tuple[int, EvictionReport]:
    """Invalidate fingerprints before one finite eviction of managed cache files."""
    report: EvictionReport = {"candidate_paths": [], "staged_paths": [], "invalidated_profiles": [],
              "retained_unknown_alias_inodes": 0, "retained_protected_inodes": 0,
              "estimated_candidate_bytes": 0, "deleted_files": 0, "retained_unknown_entries": 0,
              "retained_unknown_alias_diagnostics": []}
    held: list[_Held] = []
    root_checks: list[_Held] = []
    descriptors: dict[Path, int] = {}
    identities: dict[Path, _Identity] = {}
    retained_session_locks: dict[Path, tuple[Path, int, _Identity]] = {}
    retained_session_dirs: dict[Path, _Identity] = {}
    retained_session_names: dict[Path, list[str]] = {}
    retained_session_files: set[Path] = set()
    retained_working_sessions: set[Path] = set()
    retained_session_for_file: dict[Path, Path] = {}
    retained_cache_files: dict[Path, list[_File]] = {}
    removed_cache_leaves: set[Path] = set()
    removed_session_locks: set[Path] = set()
    owned_files: set[Path] = set()
    library_alias_files: set[Path] = set()
    owned_members: dict[Path, list[_Candidate]] = {}
    owned_records: tuple[OrphanBuildScript, ...] = ()
    removed_owned_files: set[Path] = set()
    selected_cache_sessions: set[Path] = set()
    selected_cache_latest_sessions: set[Path] = set()
    selected_cache_old_rows: list[_Candidate] = []
    selected_cache_latest_rows: list[_Candidate] = []
    selected_cache_collect: Callable[[], None] | None = None
    protected_directory_snapshots: list[tuple[Path, list[_File], list[_Directory]]] = []
    def verify_session_lock(session: Path) -> None:
        lock_path, lock_fd, lock_identity = retained_session_locks[session]
        parent_fd = descriptors[lock_path.parent]
        lock_value = os.stat(lock_path.name, dir_fd=parent_fd, follow_symlinks=False)
        if (_identity(os.fstat(lock_fd)) != lock_identity
                or _identity(lock_value) != lock_identity or lock_value.st_nlink != 1):
            raise RuntimeError("incremental session lock changed")
        if session in retained_cache_files and getattr(lock_value, "st_flags", 0) != 0:
            raise RuntimeError("incremental cache lock flags changed")
    def verify_session_locks() -> None:
        for session in retained_session_locks:
            if session not in removed_session_locks:
                verify_session_lock(session)
    def verify_roots(parents: Iterable[Path] = (), full: bool = False) -> None:
        if epoch is not None:
            if (library_alias_ownership is not None
                    and epoch.input_check is library_alias_ownership.verify):
                # alias証跡のfull input/tool snapshotは下段で実行するため、変異検出用のidentity順序を維持する。
                epoch._verify_identities()
            elif full:
                epoch.verify()
            else:
                epoch._verify_identities()
        checks = list(held if full else root_checks)
        for parent in parents:
            chain = []
            while parent in descriptors:
                chain.append((parent, descriptors[parent], identities[parent]))
                parent = parent.parent
            checks.extend(reversed(chain))
        seen = set()
        for path, descriptor, identity in checks:
            if descriptor in seen:
                continue
            seen.add(descriptor)
            parent_fd = descriptors.get(path.parent)
            named = (os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
                     if parent_fd is not None else path.lstat())
            if (_identity(os.fstat(descriptor)) != identity
                    or _identity(named) != identity):
                raise RuntimeError("cache root identity changed")
        verify_session_locks()
        if library_alias_ownership is not None:
            library_alias_ownership.verify()
            if epoch is not None and epoch.input_check is library_alias_ownership.verify:
                epoch._verify_identities()
        for path, files, directories in protected_directory_snapshots:
            current_files, current_directories = _fingerprint_files(path, device)
            if ([(p, _stamp(v)) for p, v in files] != [(p, _stamp(v)) for p, v in current_files]
                    or directories != current_directories):
                raise RuntimeError("protected directory closure changed")
        if build_script_ownership is not None:
            verify_orphan_build_scripts(build_script_ownership, descriptors[build_script_ownership.profile],
                                        descriptors, owned_records, removed_owned_files)
        for session, files in retained_cache_files.items():
            expected_names = sorted(name for name in retained_session_names[session]
                                    if session / name not in removed_cache_leaves)
            if sorted(os.listdir(descriptors[session])) != expected_names:
                raise RuntimeError("incremental cache namespace changed")
            if getattr(os.fstat(descriptors[session]), "st_flags", 0) != 0:
                raise RuntimeError("incremental cache directory flags changed")
            for path, value in files:
                actual = _metadata_at(descriptors[session], path.name, device)
                if (_stamp(actual) != _stamp(value)
                        or getattr(actual, "st_flags", 0) != getattr(value, "st_flags", 0)):
                    raise RuntimeError("retained incremental output changed")
    try:
        device = target.lstat().st_dev
        retained_profiles = retained_profile_paths(target, retained_profiles)
        auxiliary_only_profiles = retained_profile_paths(target, auxiliary_only_profiles)
        selected_package_profiles = retained_profile_paths(target, selected_package_profiles)
        if "build_script_build-" in unselected_incremental_prefixes:
            raise RuntimeError("custom-build incremental namespace must remain retained")
        if selected_package_profiles and (
            not (unselected_incremental_prefixes or build_script_ownership is not None
                 or library_alias_ownership is not None)
            or not set(selected_package_profiles).issubset(retained_profiles)
            or epoch is None or epoch.input_check is None
        ):
            raise RuntimeError("selected package cleanup requires bound metadata inputs and retained profiles")
        if unselected_incremental_prefixes and not selected_package_profiles:
            raise RuntimeError("unselected namespaces require verified selected package profiles")
        if build_script_ownership is not None and (
            build_script_ownership.target != target
            or build_script_ownership.profile not in selected_package_profiles
            or epoch is None or epoch.input_check is not build_script_ownership.verify
            or not protected_refs
        ):
            raise RuntimeError("orphan build-script cleanup requires bound selected metadata and protected consumers")
        if selected_cache_ownership is not None and (
            selected_cache_ownership.target != target
            or selected_cache_ownership.profile not in selected_package_profiles
            or epoch is None or epoch.input_check is not selected_cache_ownership.verify
            or library_alias_ownership is None and (build_script_ownership is None
                or selected_cache_ownership.verify is not build_script_ownership.verify)
            or library_alias_ownership is not None and selected_cache_ownership is not library_alias_ownership.cache
            or not protected_refs
        ):
            raise RuntimeError("selected cache cleanup requires a bound selected host and protected consumers")
        if library_alias_ownership is not None and (
            library_alias_ownership.target != target
            or library_alias_ownership.target / "debug" not in selected_package_profiles
            or epoch is None or epoch.input_check is not library_alias_ownership.verify
            or not protected_refs or build_script_ownership is not None
            or selected_cache_ownership is not library_alias_ownership.cache
        ):
            raise RuntimeError("library alias cleanup requires bound classifiers and protected consumers")
        if auxiliary_only_profiles:
            if protected_refs:
                raise RuntimeError("auxiliary-only cleanup requires an empty protected reference set")
            if not set(auxiliary_only_profiles).issubset(retained_profiles):
                raise RuntimeError("auxiliary-only profiles must be validated retained build profiles")
            # 手動指定では許可せず、入口で検証したビルド情報だけを補助キャッシュ整理の根拠にする。
            retained_profiles = auxiliary_only_profiles
        auxiliary_profiles = selected_package_profiles or auxiliary_only_profiles
        if retained_profiles:
            report["retained_profiles"] = [str(path.relative_to(target)) for path in retained_profiles]
        if not protected_refs and not auxiliary_profiles:
            raise RuntimeError("exact protected consumer references are required; artifacts retained")
        profile_paths = list(auxiliary_profiles) if auxiliary_profiles else _profiles(target)
        protected: set[_Inode] = set()
        protected_snapshots: list[_File] = []
        for reference in protected_refs:
            reference_path = Path(reference)
            if library_alias_ownership is not None and reference_path.is_dir():
                if (reference_path.resolve() != reference_path or reference_path.is_symlink()
                        or not reference_path.is_relative_to(target)):
                    raise RuntimeError("library alias protected directory is not canonical")
                ref_files, ref_dirs = _fingerprint_files(reference_path, device)
                protected_directory_snapshots.append((reference_path, ref_files, ref_dirs))
                protected.update((v.st_dev, v.st_ino) for _, v in ref_files)
                protected_snapshots.extend(ref_files)
                continue
            value = _reference_metadata(Path(reference), device)
            protected.add((value.st_dev, value.st_ino))
            protected_snapshots.append((Path(reference), value))
        candidates: list[_Candidate] = []
        fingerprints: list[_Fingerprint] = []
        namespaces: list[tuple[Path, list[str]]] = []
        incremental_descendants: set[Path] = set()
        incremental_sessions: dict[Path, Path] = {}
        incremental_indexes: defaultdict[Path, dict[str, _Candidate]] = defaultdict(dict)
        incremental_members: defaultdict[Path, list[_Candidate]] = defaultdict(list)
        if any(not isinstance(prefix, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*-", prefix)
               for prefix in (*incremental_prefixes, *inactive_prefixes, *unselected_incremental_prefixes)):
            raise RuntimeError("invalid incremental crate prefix")
        for profile in profile_paths:
            descriptor = os.open(profile, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            held.append((profile, descriptor, _identity(os.fstat(descriptor))))
            if os.fstat(descriptor).st_dev != device:
                raise RuntimeError("profile device boundary")
            fp = profile / ".fingerprint"
            recovery = sorted(profile.glob(".capacity-invalidated-*"))
            if recovery or not fp.exists():
                report["recovery_required_paths"] = [str(path.relative_to(target)) for path in recovery]
                report["recovery_required_profile"] = str(profile.relative_to(target))
                raise RuntimeError("interrupted fingerprint invalidation requires bounded recovery; artifacts retained")
            fp_files, fp_dirs = _fingerprint_files(fp, device)
            fingerprints.append((profile, fp, fp_files, fp_dirs))
            if not auxiliary_profiles:
                for name in ("deps", "examples"):
                    directory = profile / name
                    if not directory.exists():
                        continue
                    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                    held.append((directory, descriptor, _identity(os.fstat(descriptor))))
                    if os.fstat(descriptor).st_dev != device:
                        raise RuntimeError("artifact device boundary")
                    names = sorted(os.listdir(descriptor))
                    namespaces.append((directory, names))
                    for filename in names:
                        # Cargoの管理namespaceにある既知形式だけを扱い、名前からfingerprint hashを結合しない。
                        if not re.search(r"-[0-9a-f]{16}(?:\.|$)", filename):
                            report["retained_unknown_entries"] += 1
                            continue
                        suffix = Path(filename).suffix
                        if suffix not in {"", ".rlib", ".rmeta", ".so", ".dylib", ".dll", ".lib", ".a",
                                          ".exe", ".d", ".o", ".obj", ".dwo", ".dwp", ".pdb"}:
                            report["retained_unknown_entries"] += 1
                            continue
                        path = directory / filename
                        if not stat.S_ISREG(os.stat(filename, dir_fd=descriptor, follow_symlinks=False).st_mode):
                            report["retained_unknown_entries"] += 1
                            continue
                        candidates.append((profile, path, _metadata_at(descriptor, filename, device)))
            profile_prefixes = tuple(incremental_prefixes)
            if retained_profiles and profile not in retained_profiles:
                profile_prefixes += tuple(inactive_prefixes)
            if profile in selected_package_profiles:
                profile_prefixes += tuple(unselected_incremental_prefixes)
            if library_alias_ownership is not None:
                profile_prefixes = ()
            if profile_prefixes or selected_cache_ownership is not None and profile == selected_cache_ownership.profile:
                incremental = profile / "incremental"
                if incremental.exists():
                    root_fd = os.open(incremental, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                    root_value = os.fstat(root_fd)
                    held.append((incremental, root_fd, _identity(root_value)))
                    if root_value.st_dev != device:
                        raise RuntimeError("incremental device boundary")
                    root_names = sorted(os.listdir(root_fd))
                    namespaces.append((incremental, root_names))
                    def visit_incremental(
                        path: Path, parent_fd: int, owner: Path = profile,
                        incremental_root: Path = incremental,
                        selected_cache_visit: bool = False,
                    ) -> None:
                        if (library_alias_ownership is not None and selected_cache_visit
                                and path in retained_session_locks and path not in selected_cache_sessions):
                            return
                        descriptor = os.open(path.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                             dir_fd=parent_fd)
                        value = os.fstat(descriptor)
                        held.append((path, descriptor, _identity(value)))
                        incremental_descendants.add(path)
                        named = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
                        if value.st_dev != device or _identity(value) != _identity(named):
                            raise RuntimeError("incremental directory identity changed")
                        names = sorted(os.listdir(descriptor))
                        namespaces.append((path, names))
                        if retained_profiles and owner in retained_profiles and path.parent == incremental_root:
                            unselected_owner = (
                                owner in selected_package_profiles
                                and any(re.fullmatch(re.escape(prefix) + r"[0-9a-z]{13}", path.name)
                                        for prefix in unselected_incremental_prefixes)
                            )
                            cache_owner = next((item for item in selected_cache_ownership.owners
                                                if re.fullmatch(re.escape(item.prefix) + r"[0-9a-z]{13}", path.name)), None) if (
                                selected_cache_visit and selected_cache_ownership is not None
                            ) else None
                            lock_prefix_counts = Counter(
                                lock_name for name in names
                                if (lock_name := _rust_session_lock_name(name)) is not None
                            )
                            parsed_sessions: list[tuple[str, int, str]] = []
                            for child_name in names:
                                parsed = _rust_session_parts(child_name)
                                if parsed is None:
                                    continue
                                try:
                                    child_value = os.stat(child_name, dir_fd=descriptor, follow_symlinks=False)
                                except OSError:
                                    continue
                                if stat.S_ISDIR(child_value.st_mode) and child_value.st_dev == device:
                                    parsed_sessions.append((child_name, parsed[0], parsed[1]))
                            latest_finalized = max(
                                (timestamp for _, timestamp, suffix in parsed_sessions if suffix != "working"),
                                default=-1,
                            )
                            now_micros = time.time_ns() // 1000
                            for child_name, timestamp, suffix in parsed_sessions:
                                session_path = path / child_name
                                finalized = suffix != "working"
                                lock_name = _rust_session_lock_name(child_name)
                                if lock_name is None or lock_prefix_counts[lock_name] != 1:
                                    continue
                                eligible = (timestamp < latest_finalized if finalized
                                            else now_micros - timestamp > 10_000_000)
                                if unselected_owner:
                                    eligible = finalized
                                if cache_owner is not None:
                                    eligible = finalized
                                if selected_cache_ownership is not None and owner == selected_cache_ownership.profile and not finalized:
                                    eligible = False
                                if not eligible:
                                    continue
                                if (library_alias_ownership is not None and selected_cache_visit
                                        and session_path in retained_session_locks
                                        and session_path not in selected_cache_sessions):
                                    continue
                                lock = _acquire_incremental_session_lock(
                                    path, descriptor, child_name, device, protected,
                                )
                                if lock is None:
                                    continue
                                lock_fd, lock_path, lock_identity = lock
                                session_fd: int | None = None
                                try:
                                    session_fd = os.open(child_name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                                         dir_fd=descriptor)
                                    session_value = os.fstat(session_fd)
                                    named_value = os.stat(child_name, dir_fd=descriptor, follow_symlinks=False)
                                    session_names = sorted(os.listdir(session_fd))
                                    cache_leaves_only = unselected_owner and finalized and "metadata.rmeta" in session_names
                                    selected_cache = cache_owner is not None and finalized
                                    # 同じCargo lib targetのtest-mode cacheにはrmetaがなく、既存binary形でindicesだけを再生成できる。
                                    selected_schema = (
                                        _known_session_members(session_fd, session_names, True, True)
                                        if cache_owner is not None and cache_owner.kind == "library"
                                        and "metadata.rmeta" in session_names
                                        else binary_cache_members(session_fd, session_names)
                                    ) if selected_cache else False
                                    if selected_cache:
                                        cache_leaves_only = selected_schema
                                    strict_optional_cache = (
                                        selected_cache_ownership is not None and owner == selected_cache_ownership.profile
                                    )
                                    if (_identity(session_value) != _identity(named_value)
                                            or session_value.st_dev != device
                                            or strict_optional_cache and not cache_leaves_only
                                            or (not selected_schema if selected_cache else
                                                not _known_session_members(session_fd, session_names, finalized, cache_leaves_only))
                                            or cache_leaves_only and (
                                                getattr(session_value, "st_flags", 0) != 0
                                                or getattr(os.fstat(lock_fd), "st_flags", 0) != 0
                                                or not os.access(session_path, os.W_OK | os.X_OK, effective_ids=True)
                                            )):
                                        os.close(session_fd)
                                        session_fd = None
                                        os.close(lock_fd)
                                        continue
                                    candidate_names = (
                                        sorted({"dep-graph.bin", "query-cache.bin"}.intersection(session_names))
                                        if selected_cache or strict_optional_cache else sorted(INCREMENTAL_INDEX_NAMES.intersection(session_names))
                                        if cache_leaves_only else session_names
                                    )
                                    preserved_cache_files = [
                                        (session_path / name, _metadata_at(session_fd, name, device))
                                        for name in session_names if cache_leaves_only and name not in candidate_names
                                    ]
                                    retained_session_locks[session_path] = (lock_path, lock_fd, lock_identity)
                                    retained_session_dirs[session_path] = _identity(session_value)
                                    retained_session_names[session_path] = session_names
                                    if cache_leaves_only:
                                        retained_cache_files[session_path] = preserved_cache_files
                                    if selected_cache:
                                        selected_cache_sessions.add(session_path)
                                        if timestamp == latest_finalized:
                                            selected_cache_latest_sessions.add(session_path)
                                    retained_session_files.update(session_path / filename for filename in candidate_names)
                                    if not finalized:
                                        retained_working_sessions.add(session_path)
                                    retained_session_for_file.update(
                                        (session_path / filename, session_path) for filename in candidate_names
                                    )
                                    os.close(session_fd)
                                    session_fd = None
                                except (OSError, RuntimeError):
                                    if session_fd is not None:
                                        os.close(session_fd)
                                    os.close(lock_fd)
                        for filename in names:
                            child = path / filename
                            item = os.stat(filename, dir_fd=descriptor, follow_symlinks=False)
                            if item.st_dev != device:
                                raise RuntimeError("incremental device boundary")
                            if stat.S_ISDIR(item.st_mode):
                                visit_incremental(child, descriptor, owner, incremental_root, selected_cache_visit)
                            elif stat.S_ISREG(item.st_mode):
                                session = _incremental_session(child, owner)
                                working_session = (child.parent if child.parent in retained_working_sessions
                                                   and filename == _WORKING_SESSION_FILE else None)
                                retained_profile = owner in retained_profiles
                                retained_eligible = (child in retained_session_files if retained_profile else True)
                                if (session is not None and retained_eligible
                                        and (filename in INCREMENTAL_INDEX_NAMES or child.suffix == ".o")):
                                    row = (owner, child, _metadata_at(descriptor, filename, device))
                                    if session in selected_cache_sessions:
                                        destination = (selected_cache_latest_rows if session in selected_cache_latest_sessions
                                                       else selected_cache_old_rows)
                                        destination.append(row)
                                    else:
                                        candidates.append(row)
                                    incremental_sessions[child] = session
                                    incremental_members[session].append(row)
                                    if filename in INCREMENTAL_INDEX_NAMES:
                                        incremental_indexes[session][filename] = row
                                elif working_session is not None:
                                    candidates.append((owner, child, _metadata_at(descriptor, filename, device)))
                                elif session in retained_cache_files:
                                    pass
                                elif retained_profile and session is not None:
                                    report["retained_unknown_entries"] += 1
                                else:
                                    report["retained_unknown_entries"] += 1
                            else:
                                raise RuntimeError("unknown incremental kind; artifacts retained")
                        if (_stamp(value) != _stamp(os.fstat(descriptor))
                                or names != sorted(os.listdir(descriptor))):
                            raise RuntimeError("incremental changed during selection")
                    for filename in root_names:
                        known_unselected = any(
                            re.fullmatch(re.escape(prefix) + r"[0-9a-z]{13}", filename)
                            for prefix in unselected_incremental_prefixes
                        ) if profile in selected_package_profiles else False
                        existing_prefixes = tuple(incremental_prefixes) + (
                            tuple(inactive_prefixes) if retained_profiles and profile not in retained_profiles else ()
                        )
                        known_existing = any(
                            re.fullmatch(re.escape(prefix) + r"[0-9a-z]{13}", filename)
                            if profile in selected_package_profiles else filename.startswith(prefix)
                            for prefix in existing_prefixes
                        )
                        if known_unselected or known_existing:
                            visit_incremental(incremental / filename, root_fd)
                        else:
                            report["retained_unknown_entries"] += 1
                    if selected_cache_ownership is not None and profile == selected_cache_ownership.profile:
                        def collect_selected_cache(
                            cache_names: list[str] = root_names, cache_path: Path = incremental,
                            cache_fd: int = root_fd, cache_profile: Path = profile,
                            visit: Callable[[Path, int, Path, Path, bool], None] = visit_incremental,
                            cache_proof: SelectedCacheOwnership = selected_cache_ownership,
                        ) -> None:
                            cache_proof.verify()
                            for filename in cache_names:
                                if any(re.fullmatch(re.escape(item.prefix) + r"[0-9a-z]{13}", filename)
                                       for item in cache_proof.owners):
                                    visit(cache_path / filename, cache_fd, cache_profile, cache_path, True)
                            if cache_names != sorted(os.listdir(cache_fd)):
                                raise RuntimeError("selected cache namespace changed during admission")

                        selected_cache_collect = collect_selected_cache
                    if root_names != sorted(os.listdir(root_fd)):
                        raise RuntimeError("incremental namespace changed during selection")
            if build_script_ownership is not None and profile == build_script_ownership.profile:
                build = profile / "build"
                profile_fd = next(fd for path, fd, _ in held if path == profile)
                build_fd = os.open("build", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=profile_fd)
                held.append((build, build_fd, _identity(os.fstat(build_fd))))
                namespaces.append((build, sorted(os.listdir(build_fd))))
                owned_records = orphan_build_scripts(build_script_ownership, profile_fd, build_fd, device, protected)
                for record in owned_records:
                    owner_fd = os.open(record.directory.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                       dir_fd=build_fd)
                    if _identity(os.fstat(owner_fd)) != record.identity:
                        os.close(owner_fd)
                        raise RuntimeError("compiled build-script owner changed after selection")
                    held.append((record.directory, owner_fd, record.identity))
                    rows = [(profile, path, value) for path, value in record.leaves]
                    candidates.extend(rows)
                    for _, path, _ in rows:
                        owned_files.add(path)
                        owned_members[path] = rows
        if library_alias_ownership is not None:
            references = [Path(value) for value in protected_refs]
            census, _ = _fingerprint_files(target, device)
            # クォータ目標の達成可否は結果判定の責務にし、所有証明を抑制しない。
            groups = library_alias_groups(library_alias_ownership, census, protected, references)

            def hold_alias_parent(path: Path) -> int:
                existing = next((fd for item, fd, _ in held if item == path), None)
                if existing is not None:
                    return existing
                parent = hold_alias_parent(path.parent)
                descriptor = os.open(path.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
                value = os.fstat(descriptor)
                held.append((path, descriptor, _identity(value)))
                if (value.st_dev != device or getattr(value, "st_flags", 0) != 0
                        or _identity(value) != _identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False))):
                    raise RuntimeError("library alias parent changed")
                namespaces.append((path, sorted(os.listdir(descriptor))))
                return descriptor

            for group in groups:
                indexes_for_group: list[_Candidate] = []
                eligible = True
                for session in group.sessions:
                    owner_fd = hold_alias_parent(session.parent)
                    session_fd = hold_alias_parent(session)
                    names = sorted(os.listdir(session_fd))
                    if (not _known_session_members(session_fd, names, True, True)
                            or any((_metadata_at(session_fd, name, device).st_dev,
                                    _metadata_at(session_fd, name, device).st_ino) in protected
                                   for name in names)
                            or any(ref.is_relative_to(session) or session.is_relative_to(ref)
                                   for ref in references)):
                        eligible = False
                        break
                    if session not in retained_session_locks:
                        acquired = _acquire_incremental_session_lock(
                            session.parent, owner_fd, session.name, device, protected,
                        )
                        if acquired is None:
                            eligible = False
                            break
                        lock_fd, lock_path, lock_identity = acquired
                        retained_session_locks[session] = (lock_path, lock_fd, lock_identity)
                        retained_session_dirs[session] = _identity(os.fstat(session_fd))
                        retained_session_names[session] = names
                        retained_cache_files[session] = [
                            (session / name, _metadata_at(session_fd, name, device)) for name in names
                        ]
                    for name in ("dep-graph.bin", "query-cache.bin"):
                        if name in names:
                            indexes_for_group.append((target / "debug", session / name,
                                                      _metadata_at(session_fd, name, device)))
                if not eligible:
                    continue
                rows = [(target / "debug", path, value) for path, value in group.files]
                bound = list({row[1]: row for row in rows + indexes_for_group}.values())
                for row in bound:
                    profile, path, value = row
                    parent_fd = hold_alias_parent(path.parent)
                    if _stamp(_metadata_at(parent_fd, path.name, device)) != _stamp(value):
                        raise RuntimeError("library alias changed during admission")
                    owned_files.add(path)
                    library_alias_files.add(path)
                    owned_members[path] = bound
                    if path.parent in group.sessions:
                        session = path.parent
                        retained_session_files.add(path)
                        retained_session_for_file[path] = session
                        incremental_sessions[path] = session
                        if not any(item[1] == path for item in incremental_members[session]):
                            incremental_members[session].append(row)
                        if path.name in INCREMENTAL_INDEX_NAMES:
                            incremental_indexes[session][path.name] = row
                        retained_cache_files[session] = [
                            item for item in retained_cache_files[session] if item[0] != path
                        ]
                    if not any(item[1] == path for item in candidates):
                        candidates.append(row)
            library_alias_ownership.verify()
            debug_objects = library_alias_ownership.debug_objects
            if debug_objects is not None and debug_objects.snapshot is not None:
                # consumer証明付きの選択は、そのsnapshotが検証したobject/index集合に閉じる。
                admitted_paths = {path for path, _ in (*debug_objects.snapshot.objects,
                                                        *debug_objects.snapshot.indexes,
                                                        *debug_objects.snapshot.archives)}
                candidates[:] = [row for row in candidates if row[1] in admitted_paths]
                selected_cache_old_rows[:] = [row for row in selected_cache_old_rows
                                              if row[1] in admitted_paths]
                selected_cache_latest_rows[:] = [row for row in selected_cache_latest_rows
                                                 if row[1] in admitted_paths]
                selected_cache_collect = None
            if library_alias_ownership.archives_only:
                # ARCHIVES-onlyはLLVMに束縛したarchive群だけを選び、一般cacheを混在させない。
                candidates[:] = [row for row in candidates if row[1] in library_alias_files]
                selected_cache_old_rows.clear()
                selected_cache_latest_rows.clear()
                selected_cache_collect = None
        aliases: defaultdict[_Inode, list[_Candidate]] = defaultdict(list)
        selected: list[_Candidate] = []
        selected_keys: set[_Inode] = set()
        needed = max(0, current_bytes - threshold)

        def extend_selection(rows_to_add: Sequence[_Candidate]) -> None:
            added_keys: dict[_Inode, None] = {}
            for row in rows_to_add:
                value = row[2]
                key = value.st_dev, value.st_ino
                aliases[key].append(row)
                added_keys[key] = None
            for key, rows in sorted(((key, aliases[key]) for key in added_keys),
                                    key=lambda item: max(row[2].st_mtime_ns for row in item[1])):
                if key in protected:
                    report["retained_protected_inodes"] += 1
                elif len(rows) != rows[0][2].st_nlink:
                    report["retained_unknown_alias_inodes"] += 1
                    diagnostics = report["retained_unknown_alias_diagnostics"]
                    if len(diagnostics) < 5:
                        # 集計値だけでは外部hardlinkと同一候補pathの重複登録を区別できないため、拒否を維持したまま先頭例を記録する。
                        diagnostics.append({
                            "device": key[0],
                            "inode": key[1],
                            "st_nlink": rows[0][2].st_nlink,
                            "candidate_row_count": len(rows),
                            "unique_path_count": len({row[1] for row in rows}),
                            "paths": [str(row[1].relative_to(target)) for row in rows[:5]],
                        })
                elif report["estimated_candidate_bytes"] < needed:
                    closure = _incremental_index_closure(
                        rows, aliases, incremental_sessions, incremental_indexes, incremental_members, protected,
                        set(retained_cache_files),
                        owned_members,
                    )
                    if closure is None:
                        report["retained_unknown_entries"] += len(rows)
                        continue
                    touches_retained_profile = any(
                        row[0] in retained_profiles for closed_rows in closure.values() for row in closed_rows
                    )
                    if (touches_retained_profile
                            and any(row[1] not in retained_session_files | owned_files
                                    for closed_rows in closure.values() for row in closed_rows)):
                        report["retained_active_closures"] = report.get("retained_active_closures", 0) + 1
                        continue
                    for closed_key, closed_rows in closure.items():
                        if closed_key not in selected_keys:
                            selected_keys.add(closed_key)
                            selected.extend(closed_rows)
                            report["estimated_candidate_bytes"] += closed_rows[0][2].st_blocks * 512

        extend_selection(candidates)
        if report["estimated_candidate_bytes"] < needed and selected_cache_collect is not None:
            selected_cache_collect()
            extend_selection(selected_cache_old_rows)
            if report["estimated_candidate_bytes"] < needed:
                # 最新cacheは要求cleanup goalへ届かない時だけ候補にし、成果物とwork-productsは保持する。
                extend_selection(selected_cache_latest_rows)
        # 欠落objectを指す旧indexを先に外し、中断時も残indexからの再利用を防ぐ。
        selected.sort(key=lambda row: 0 if row[1] in incremental_sessions and row[1].name == "dep-graph.bin"
                      else 1 if row[1] in incremental_sessions and row[1].name in INCREMENTAL_INDEX_NAMES else 2)
        report["candidate_paths"] = [str(row[1].relative_to(target)) for row in selected]
        if auxiliary_profiles and any(
            row[0] not in auxiliary_profiles or row[1] not in retained_session_files | owned_files
            for row in selected
        ):
            raise RuntimeError("auxiliary-only selection escaped verified closed incremental sessions")
        if retained_profiles and report["estimated_candidate_bytes"] < needed:
            partial_alias_units = (
                library_alias_ownership is not None
                and any(path in library_alias_files for _, path, _ in selected)
            )
            if not partial_alias_units:
                raise RuntimeError("insufficient closed inactive artifact capacity; active profiles retained")
        selected_incremental_files = {row[1] for row in selected} & retained_session_files
        selected_sessions = {retained_session_for_file[path] for path in selected_incremental_files}
        affected = {row[0] for row in selected if row[1] not in retained_session_files | owned_files}
        if auxiliary_profiles and affected:
            raise RuntimeError("auxiliary-only selection cannot invalidate profile fingerprints")
        fp_aliases = Counter((value.st_dev, value.st_ino)
                             for profile, _, files, _ in fingerprints if profile in affected
                             for _, value in files)
        for profile, fp, files, directories in fingerprints:
            if profile not in affected:
                continue
            for _path, value in files:
                key = value.st_dev, value.st_ino
                if key in protected or fp_aliases[key] != value.st_nlink:
                    raise RuntimeError("fingerprint protected or unknown alias; artifacts retained")
            current_files, current_directories = _fingerprint_files(fp, device)
            if ([(path, _stamp(value)) for path, value in files]
                    != [(path, _stamp(value)) for path, value in current_files]
                    or directories != current_directories):
                raise RuntimeError("fingerprint closure changed")
        descriptors.update((path, descriptor) for path, descriptor, _ in held)
        identities.update((path, identity) for path, _, identity in held)
        root_checks.extend(row for row in held if row[0] not in incremental_descendants)
        standard_selected = [row for row in selected if row[1] not in retained_session_files | owned_files]
        if library_alias_ownership is not None:
            for _, path, value in selected:
                if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != _stamp(value):
                    raise RuntimeError("library alias selected physical input changed")
        for _, path, value in standard_selected:
            if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != _stamp(value):
                raise RuntimeError("artifact changed before staging")
        for path, names in namespaces:
            if sorted(os.listdir(descriptors[path])) != names:
                raise RuntimeError("artifact namespace changed before staging")
        # 最初のrename/unlink前にfull input/tool検証を行い、admission境界を閉じる。
        verify_roots(full=True)
        if library_alias_ownership is not None and library_alias_ownership.debug_objects is not None:
            library_alias_ownership.debug_objects.validate_selection(frozenset(path for _, path, _ in selected))
        if dry_run or not selected:
            return 0, report
        for reference, value in protected_snapshots:
            if _stamp(_reference_metadata(reference, device)) != _stamp(value):
                raise RuntimeError("protected reference identity changed")
        for _, path, value in selected:
            if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != _stamp(value):
                raise RuntimeError("selected artifact changed before staging")
        verify_roots()
        def hold_directory(path: Path, parent_fd: int, name: str, identity: _Identity, root: bool = False) -> int:
            descriptor = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
            held.append((path, descriptor, identity))
            if _identity(os.fstat(descriptor)) != identity:
                raise RuntimeError("stage directory identity changed")
            descriptors[path] = descriptor
            identities[path] = identity
            if root:
                root_checks.append((path, descriptor, identity))
            return descriptor
        def destination_absent(descriptor: int, name: str) -> None:
            try:
                os.stat(name, dir_fd=descriptor, follow_symlinks=False)
            except FileNotFoundError:
                return
            raise RuntimeError("unknown stage destination exists")
        # 全元fingerprint pathを先に外す。途中失敗しても旧hashを戻さず、次buildに再生成を要求する。
        stages: dict[Path, Path] = {}
        for profile, fp, _files, directories in fingerprints:
            if profile not in affected:
                continue
            verify_roots()
            stage = profile / (".capacity-invalidated-" + uuid.uuid4().hex)
            destination_absent(descriptors[profile], stage.name)
            if _identity(os.stat(fp.name, dir_fd=descriptors[profile], follow_symlinks=False)) != directories[0][1][0]:
                raise RuntimeError("fingerprint root changed before invalidation")
            os.rename(fp.name, stage.name, src_dir_fd=descriptors[profile], dst_dir_fd=descriptors[profile])
            stages[profile] = stage
            report["invalidated_profiles"].append(str(profile.relative_to(target)))
            if fp.exists():
                raise RuntimeError("fingerprint invalidation incomplete")
            for path, stamp, _ in directories:
                staged_directory = stage / path.relative_to(fp)
                parent = profile if path == fp else staged_directory.parent
                hold_directory(staged_directory, descriptors[parent], staged_directory.name, stamp[0], root=path == fp)
        verify_roots(full=True)
        staged_artifacts: list[_File] = []
        expected = {(value.st_dev, value.st_ino): _stamp(value) for _, _, value in standard_selected}
        for profile, path, value in standard_selected:
            verify_roots((path.parent,))
            key = value.st_dev, value.st_ino
            if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != expected[key]:
                raise RuntimeError("artifact changed during staging")
            staged = stages[profile] / (".capacity-artifact-" + uuid.uuid4().hex)
            destination_absent(descriptors[staged.parent], staged.name)
            os.rename(path.name, staged.name, src_dir_fd=descriptors[path.parent], dst_dir_fd=descriptors[staged.parent])
            report["staged_paths"].append(str(path.relative_to(target)))
            staged_value = _metadata_at(descriptors[staged.parent], staged.name, device)
            stamp = _stamp(staged_value)
            if stamp[:4] + stamp[5:] != expected[key][:4] + expected[key][5:]:
                raise RuntimeError("staged artifact identity changed")
            expected[key] = stamp
            staged_artifacts.append((staged, value))
        def purge(rows: Sequence[_File], stamps: dict[_Inode, _Stamp], artifacts: bool) -> None:
            aliases: defaultdict[_Inode, list[Path]] = defaultdict(list)
            for path, value in rows:
                aliases[(value.st_dev, value.st_ino)].append(path)
            for key, paths in aliases.items():
                if stamps[key][1] != len(paths):
                    raise RuntimeError("staged alias closure changed")
                for index, path in enumerate(paths):
                    verify_roots((path.parent,))
                    if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != stamps[key]:
                        raise RuntimeError("staged leaf changed before purge")
                    session = retained_session_for_file.get(path)
                    if session is not None:
                        verify_session_lock(session)
                    os.unlink(path.name, dir_fd=descriptors[path.parent])
                    if path in owned_files:
                        removed_owned_files.add(path)
                    if session in retained_cache_files:
                        removed_cache_leaves.add(path)
                    if artifacts:
                        report["deleted_files"] += 1
                    transition = None
                    if index + 1 < len(paths):
                        next_path = paths[index + 1]
                        next_value = _metadata_at(descriptors[next_path.parent], next_path.name, device)
                        actual = _stamp(next_value)
                        previous = stamps[key]
                        # 自分のunlinkによるnlink減少とctime更新だけを次aliasへ引き継ぐ。
                        if (actual[0] != previous[0] or actual[1] != previous[1] - 1
                                or actual[2:4] != previous[2:4] or actual[5] != previous[5]):
                            raise RuntimeError("alias changed after owned unlink")
                        stamps[key] = actual
                        transition = file_stamp(next_value)
                    if (library_alias_ownership is not None and library_alias_ownership.debug_objects is not None
                            and path in owned_files):
                        library_alias_ownership.debug_objects.record_owned_unlink(path, transition)
        # 全staged artifactのrename完了後、purge開始前にもsource変化を拒否する。
        verify_roots(full=True)
        purge(staged_artifacts, expected, True)
        verify_roots(full=True)
        fp_rows = [(stages[profile] / path.relative_to(profile / ".fingerprint"), value)
                   for profile, _, files, _ in fingerprints if profile in affected
                   for path, value in files]
        purge(fp_rows, {(value.st_dev, value.st_ino): _stamp(value) for _, value in fp_rows}, False)
        for profile, _, _files, directories in fingerprints:
            if profile not in affected:
                continue
            stage = stages[profile]
            for path, _, _ in reversed(directories):
                staged = stage / path.relative_to(profile / ".fingerprint")
                verify_roots((staged,))
                os.rmdir(staged.name, dir_fd=descriptors[staged.parent])
                descriptor = descriptors.pop(staged)
                identities.pop(staged)
                held[:] = [row for row in held if row[1] != descriptor]
                root_checks[:] = [row for row in root_checks if row[1] != descriptor]
                os.close(descriptor)
            os.mkdir(".fingerprint", dir_fd=descriptors[profile])
        retained_rows = [(path, value) for _, path, value in selected if path in retained_session_files | owned_files]
        if retained_rows:
            retained_stamps = {(value.st_dev, value.st_ino): _stamp(value) for _, value in retained_rows}
            purge(retained_rows, retained_stamps, True)
            rows_by_path = {path: value for path, value in retained_rows}
            for session in sorted(selected_sessions):
                session_fd = descriptors[session]
                owner_fd = descriptors[session.parent]
                session_identity = retained_session_dirs[session]
                named_session = os.stat(session.name, dir_fd=owner_fd, follow_symlinks=False)
                if session in retained_cache_files:
                    expected_remaining = sorted(name for name in retained_session_names[session]
                                                if session / name not in rows_by_path)
                    if sorted(os.listdir(session_fd)) != expected_remaining:
                        raise RuntimeError("incremental cache namespace changed after trimming")
                    verify_roots((session,))
                    report["trimmed_incremental_sessions"] = report.get("trimmed_incremental_sessions", 0) + 1
                    continue
                if (_identity(os.fstat(session_fd)) != session_identity
                        or _identity(named_session) != session_identity
                        or os.listdir(session_fd)):
                    raise RuntimeError("incremental session changed before removal")
                verify_session_lock(session)
                expected_names = retained_session_names[session]
                if any(session / name not in rows_by_path for name in expected_names):
                    raise RuntimeError("incremental session closure incomplete")
                os.rmdir(session.name, dir_fd=owner_fd)
                descriptors.pop(session, None)
                identities.pop(session, None)
                held[:] = [row for row in held if row[1] != session_fd]
                root_checks[:] = [row for row in root_checks if row[1] != session_fd]
                os.close(session_fd)
                lock_path, _lock_fd, lock_identity = retained_session_locks[session]
                if _identity(os.fstat(_lock_fd)) != lock_identity:
                    raise RuntimeError("incremental session lock descriptor changed")
                lock_value = os.stat(lock_path.name, dir_fd=owner_fd, follow_symlinks=False)
                if _identity(lock_value) != lock_identity or lock_value.st_nlink != 1:
                    raise RuntimeError("incremental session lock changed before removal")
                os.unlink(lock_path.name, dir_fd=owner_fd)
                removed_session_locks.add(session)
                report["removed_incremental_sessions"] = report.get("removed_incremental_sessions", 0) + 1
        verify_roots(full=True)
        return 0, report
    except (OSError, RuntimeError) as error:
        report["error"] = str(error)
        if report["invalidated_profiles"]:
            report["recovery_required_paths"] = [str(path.relative_to(target)) for path in stages.values()]
        return 1, report
    finally:
        for _session, (_lock_path, descriptor, _identity_value) in retained_session_locks.items():
            try:
                if os.name == "nt" or fcntl is None:
                    pass
                elif sys.platform.startswith("linux"):
                    fcntl.flock(descriptor, fcntl.LOCK_UN)
                else:
                    fcntl.lockf(descriptor, fcntl.LOCK_UN)
            except OSError:
                pass
            os.close(descriptor)
        for _path, descriptor, _held_identity in reversed(held):
            os.close(descriptor)


def is_successful_build(command: list[str]) -> bool:
    """Accept cargo/rtk proxy cargo build, including +toolchain syntax only."""
    if not command:
        return False
    cargo_index = next((index for index, part in enumerate(command[:4])
                        if Path(part).name in {"cargo", "rtk"}), None)
    if cargo_index is None:
        return False
    if Path(command[cargo_index]).name == "rtk":
        cargo_index += 1
        if cargo_index < len(command) and command[cargo_index] == "proxy":
            cargo_index += 1
        if cargo_index >= len(command) or Path(command[cargo_index]).name != "cargo":
            return False
    subcommands = command[cargo_index + 1:]
    if subcommands and subcommands[0].startswith("+"):
        subcommands = subcommands[1:]
    return bool(subcommands and subcommands[0] == "build" and "--help" not in subcommands)
