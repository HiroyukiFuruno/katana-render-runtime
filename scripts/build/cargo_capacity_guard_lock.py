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
from collections.abc import Iterable, Iterator, Sequence
from contextlib import ExitStack, contextmanager
from typing import TYPE_CHECKING, Protocol, TextIO, TypedDict, runtime_checkable

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


class _EvictionReportFields(TypedDict):
    candidate_paths: list[str]
    staged_paths: list[str]
    invalidated_profiles: list[str]
    retained_unknown_alias_inodes: int
    retained_protected_inodes: int
    estimated_candidate_bytes: int
    deleted_files: int
    retained_unknown_entries: int


class EvictionReport(_EvictionReportFields, total=False):
    retained_profiles: list[str]
    recovery_required_paths: list[str]
    recovery_required_profile: str
    retained_active_closures: int
    removed_incremental_sessions: int
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
    def __init__(self, target: Path) -> None:
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

    def __enter__(self) -> ProfileLocks:
        try:
            cursor = Path(os.path.abspath(self.target))
            for path in [*reversed(cursor.parents), cursor]:
                descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                self.ancestors.append((path, descriptor, _identity(os.fstat(descriptor))))
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
            for suffix in ("debug", "release", "build/debug", "build/release"):
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
        if (not parts or parts[-1] not in {"debug", "release"}
                or len(parts) > 3 or ".." in parts
                or (len(parts) == 3 and parts[1] != "build")):
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
    session_fd: int, names: list[str], finalized: bool,
) -> bool:
    if finalized:
        if not INCREMENTAL_INDEX_NAMES.issubset(names):
            return False
    else:
        if names != [_WORKING_SESSION_FILE]:
            return False
    for name in names:
        known = (name in INCREMENTAL_INDEX_NAMES or Path(name).suffix == ".o") if finalized else name == _WORKING_SESSION_FILE
        if not known:
            return False
        try:
            value = os.stat(name, dir_fd=session_fd, follow_symlinks=False)
        except OSError:
            return False
        if not stat.S_ISREG(value.st_mode):
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
    protected: set[_Inode],
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
            session = sessions.get(path)
            if session is not None:
                index_rows = indexes[session]
                if set(index_rows) != INCREMENTAL_INDEX_NAMES:
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
) -> tuple[int, EvictionReport]:
    """Invalidate fingerprints before one finite eviction of managed cache files."""
    report: EvictionReport = {"candidate_paths": [], "staged_paths": [], "invalidated_profiles": [],
              "retained_unknown_alias_inodes": 0, "retained_protected_inodes": 0,
              "estimated_candidate_bytes": 0, "deleted_files": 0, "retained_unknown_entries": 0}
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
    removed_session_locks: set[Path] = set()
    def verify_session_lock(session: Path) -> None:
        lock_path, lock_fd, lock_identity = retained_session_locks[session]
        parent_fd = descriptors[lock_path.parent]
        lock_value = os.stat(lock_path.name, dir_fd=parent_fd, follow_symlinks=False)
        if (_identity(os.fstat(lock_fd)) != lock_identity
                or _identity(lock_value) != lock_identity or lock_value.st_nlink != 1):
            raise RuntimeError("incremental session lock changed")
    def verify_session_locks() -> None:
        for session in retained_session_locks:
            if session not in removed_session_locks:
                verify_session_lock(session)
    def verify_roots(parents: Iterable[Path] = (), full: bool = False) -> None:
        if epoch is not None:
            epoch.verify()
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
    try:
        device = target.lstat().st_dev
        retained_profiles = retained_profile_paths(target, retained_profiles)
        auxiliary_only_profiles = retained_profile_paths(target, auxiliary_only_profiles)
        if auxiliary_only_profiles:
            if protected_refs:
                raise RuntimeError("auxiliary-only cleanup requires an empty protected reference set")
            if not set(auxiliary_only_profiles).issubset(retained_profiles):
                raise RuntimeError("auxiliary-only profiles must be validated retained build profiles")
            # 手動指定では許可せず、入口で検証したビルド情報だけを補助キャッシュ整理の根拠にする。
            retained_profiles = auxiliary_only_profiles
        if retained_profiles:
            report["retained_profiles"] = [str(path.relative_to(target)) for path in retained_profiles]
        if not protected_refs and not auxiliary_only_profiles:
            raise RuntimeError("exact protected consumer references are required; artifacts retained")
        profile_paths = list(auxiliary_only_profiles) if auxiliary_only_profiles else _profiles(target)
        protected: set[_Inode] = set()
        protected_snapshots: list[_File] = []
        for reference in protected_refs:
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
               for prefix in (*incremental_prefixes, *inactive_prefixes)):
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
            if not auxiliary_only_profiles:
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
            if profile_prefixes:
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
                    ) -> None:
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
                                if not eligible:
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
                                    if (_identity(session_value) != _identity(named_value)
                                            or session_value.st_dev != device
                                            or not _known_session_members(session_fd, session_names, finalized)):
                                        os.close(session_fd)
                                        session_fd = None
                                        os.close(lock_fd)
                                        continue
                                    retained_session_locks[session_path] = (lock_path, lock_fd, lock_identity)
                                    retained_session_dirs[session_path] = _identity(session_value)
                                    retained_session_names[session_path] = session_names
                                    retained_session_files.update(session_path / filename for filename in session_names)
                                    if not finalized:
                                        retained_working_sessions.add(session_path)
                                    retained_session_for_file.update(
                                        (session_path / filename, session_path) for filename in session_names
                                    )
                                    os.close(session_fd)
                                    session_fd = None
                                except OSError:
                                    if session_fd is not None:
                                        os.close(session_fd)
                                    os.close(lock_fd)
                        for filename in names:
                            child = path / filename
                            item = os.stat(filename, dir_fd=descriptor, follow_symlinks=False)
                            if item.st_dev != device:
                                raise RuntimeError("incremental device boundary")
                            if stat.S_ISDIR(item.st_mode):
                                visit_incremental(child, descriptor, owner)
                            elif stat.S_ISREG(item.st_mode):
                                session = _incremental_session(child, owner)
                                working_session = (child.parent if child.parent in retained_working_sessions
                                                   and filename == _WORKING_SESSION_FILE else None)
                                retained_profile = owner in retained_profiles
                                retained_eligible = (child in retained_session_files if retained_profile else True)
                                if (session is not None and retained_eligible
                                        and (filename in INCREMENTAL_INDEX_NAMES or child.suffix == ".o")):
                                    row = (owner, child, _metadata_at(descriptor, filename, device))
                                    candidates.append(row)
                                    incremental_sessions[child] = session
                                    incremental_members[session].append(row)
                                    if filename in INCREMENTAL_INDEX_NAMES:
                                        incremental_indexes[session][filename] = row
                                elif working_session is not None:
                                    candidates.append((owner, child, _metadata_at(descriptor, filename, device)))
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
                        if any(filename.startswith(prefix) for prefix in profile_prefixes):
                            visit_incremental(incremental / filename, root_fd)
                        else:
                            report["retained_unknown_entries"] += 1
                    if root_names != sorted(os.listdir(root_fd)):
                        raise RuntimeError("incremental namespace changed during selection")
        aliases: defaultdict[_Inode, list[_Candidate]] = defaultdict(list)
        for row in candidates:
            value = row[2]
            aliases[(value.st_dev, value.st_ino)].append(row)
        selected: list[_Candidate] = []
        selected_keys: set[_Inode] = set()
        needed = max(0, current_bytes - threshold)
        for key, rows in sorted(aliases.items(), key=lambda item: max(row[2].st_mtime_ns for row in item[1])):
            if key in protected:
                report["retained_protected_inodes"] += 1
            elif len(rows) != rows[0][2].st_nlink:
                report["retained_unknown_alias_inodes"] += 1
            elif report["estimated_candidate_bytes"] < needed:
                closure = _incremental_index_closure(
                    rows, aliases, incremental_sessions, incremental_indexes, incremental_members, protected,
                )
                if closure is None:
                    report["retained_unknown_entries"] += len(rows)
                    continue
                touches_retained_profile = any(
                    row[0] in retained_profiles for closed_rows in closure.values() for row in closed_rows
                )
                if (touches_retained_profile
                        and any(row[1] not in retained_session_files
                                for closed_rows in closure.values() for row in closed_rows)):
                    report["retained_active_closures"] = report.get("retained_active_closures", 0) + 1
                    continue
                for closed_key, closed_rows in closure.items():
                    if closed_key not in selected_keys:
                        selected_keys.add(closed_key)
                        selected.extend(closed_rows)
                        report["estimated_candidate_bytes"] += closed_rows[0][2].st_blocks * 512
        # 欠落objectを指す旧indexを先に外し、中断時も残indexからの再利用を防ぐ。
        selected.sort(key=lambda row: 0 if row[1] in incremental_sessions
                      and row[1].name in INCREMENTAL_INDEX_NAMES else 1)
        report["candidate_paths"] = [str(row[1].relative_to(target)) for row in selected]
        if auxiliary_only_profiles and any(
            row[0] not in auxiliary_only_profiles or row[1] not in retained_session_files
            for row in selected
        ):
            raise RuntimeError("auxiliary-only selection escaped verified closed incremental sessions")
        if retained_profiles and report["estimated_candidate_bytes"] < needed:
            raise RuntimeError("insufficient closed inactive artifact capacity; active profiles retained")
        selected_incremental_files = {row[1] for row in selected} & retained_session_files
        selected_sessions = {retained_session_for_file[path] for path in selected_incremental_files}
        affected = {row[0] for row in selected if row[1] not in retained_session_files}
        if auxiliary_only_profiles and affected:
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
        standard_selected = [row for row in selected if row[1] not in retained_session_files]
        for _, path, value in standard_selected:
            if _stamp(_metadata_at(descriptors[path.parent], path.name, device)) != _stamp(value):
                raise RuntimeError("artifact changed before staging")
        for path, names in namespaces:
            if sorted(os.listdir(descriptors[path])) != names:
                raise RuntimeError("artifact namespace changed before staging")
        verify_roots()
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
                    if artifacts:
                        report["deleted_files"] += 1
                    if index + 1 < len(paths):
                        next_path = paths[index + 1]
                        actual = _stamp(_metadata_at(descriptors[next_path.parent], next_path.name, device))
                        previous = stamps[key]
                        # 自分のunlinkによるnlink減少とctime更新だけを次aliasへ引き継ぐ。
                        if (actual[0] != previous[0] or actual[1] != previous[1] - 1
                                or actual[2:4] != previous[2:4] or actual[5] != previous[5]):
                            raise RuntimeError("alias changed after owned unlink")
                        stamps[key] = actual
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
        retained_rows = [(path, value) for _, path, value in selected if path in retained_session_files]
        if retained_rows:
            retained_stamps = {(value.st_dev, value.st_ino): _stamp(value) for _, value in retained_rows}
            purge(retained_rows, retained_stamps, True)
            rows_by_path = {path: value for path, value in retained_rows}
            for session in sorted(selected_sessions):
                session_fd = descriptors[session]
                owner_fd = descriptors[session.parent]
                session_identity = retained_session_dirs[session]
                named_session = os.stat(session.name, dir_fd=owner_fd, follow_symlinks=False)
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
