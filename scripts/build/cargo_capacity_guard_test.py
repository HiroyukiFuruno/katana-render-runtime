#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import contextlib
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from pathlib import Path
from collections.abc import Callable, Iterator, Sequence
from types import SimpleNamespace
from typing import IO, Literal, TextIO

from cargo_capacity_guard import inspect as inspect_capacity
from cargo_capacity_guard import (
    record_cleanup_result,
    current_build_profiles,
    load_state,
    resolve_target,
    runtime_incremental_prefixes,
    workspace_incremental_prefixes,
    selected_package_incremental_prefixes,
    selected_package_cleanup,
    _selected_input_check,
    _object,
    _array,
    run as run_guard,
    size_bytes,
)
from cargo_capacity_guard_lock import (
    EvictionReport,
    GuardLock,
    ProfileLocks,
    _acquire_incremental_session_lock,
    _metadata_at,
    busy_processes,
    evict_artifacts,
    is_successful_build,
)


@contextlib.contextmanager
def windows_lock_fixture(
    events: list[tuple[int, int, int]], failure: BaseException | None = None,
) -> Iterator[tuple[SimpleNamespace, list[TextIO], list[int]]]:
    handles: list[TextIO] = []
    descriptors: list[int] = []
    native_open = Path.open
    native_fdopen = os.fdopen
    native_path = type(Path.cwd())

    def remember(handle: TextIO) -> TextIO:
        handles.append(handle)
        descriptors.append(handle.fileno())
        return handle

    def open_guard(path: Path, mode: str) -> TextIO:
        if mode != "a+":
            raise AssertionError("guard lock must preserve a+ mode")
        handle = native_open(path, "a+")
        return remember(handle)

    def open_profile(descriptor: int, mode: str) -> TextIO:
        if mode != "a+":
            raise AssertionError("profile lock must preserve a+ mode")
        return remember(native_fdopen(descriptor, "a+"))

    def locking(descriptor: int, operation: int, count: int) -> None:
        events.append((descriptor, operation, count))
        if failure is not None:
            raise failure

    module = SimpleNamespace(LK_LOCK=1, LK_NBLCK=2, LK_UNLCK=3, locking=locking)
    # Windows APIだけを差し替え、FDの実検証には実OSのPathを維持する。
    with patch.dict(sys.modules, {"msvcrt": module}), patch("cargo_capacity_guard_lock.os.name", "nt"), \
            patch("cargo_capacity_guard_lock.Path", native_path), \
            patch.object(Path, "open", open_guard), patch.object(os, "fdopen", open_profile):
        yield module, handles, descriptors


@contextlib.contextmanager
def incremental_session_lock_holder(
    lock_path: Path, mode: Literal["shared", "exclusive"],
) -> Iterator[IO[str]]:
    operation = "LOCK_SH" if mode == "shared" else "LOCK_EX"
    if sys.platform.startswith("linux"):
        lock_call = f"fcntl.flock(h.fileno(),fcntl.{operation})"
    else:
        lock_call = f"fcntl.lockf(h.fileno(),fcntl.{operation})"
    child_code = (
        "import fcntl,sys; h=open(sys.argv[1],'r+'); "
        + lock_call
        + "; print('ready',flush=True); sys.stdin.read()"
    )
    child = subprocess.Popen(
        [sys.executable, "-c", child_code, str(lock_path)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        child_stdout = child.stdout
        if child_stdout is None:
            raise AssertionError("session lock holder stdout pipe was not created")
        yield child_stdout
    finally:
        if child.stdin is not None:
            try:
                child.stdin.close()
            except BrokenPipeError:
                pass
        try:
            child.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            child.terminate()
            try:
                child.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()


class GuardTests(unittest.TestCase):
    def test_windows_guard_lock_preserves_native_modes_and_fd_lifecycle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = GuardLock(Path(directory) / "guard.lock")
            events: list[tuple[int, int, int]] = []
            with windows_lock_fixture(events) as (_, handles, descriptors):
                with lock as entered:
                    self.assertIs(entered, lock)
                    self.assertFalse(handles[0].closed)
                    os.fstat(descriptors[0])
                self.assertEqual(events, [(descriptors[0], 1, 1), (descriptors[0], 3, 1)])
                self.assertTrue(handles[0].closed)
                self.assertIsNone(lock.handle)
                with self.assertRaises(OSError):
                    os.fstat(descriptors[0])

    def test_windows_guard_lock_rejects_invalid_api_and_closes_fd(self) -> None:
        for field, value in (("LK_LOCK", True), ("LK_NBLCK", "2"), ("LK_UNLCK", None), ("locking", object())):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                lock = GuardLock(Path(directory) / "guard.lock")
                events: list[tuple[int, int, int]] = []
                with windows_lock_fixture(events) as (module, handles, descriptors), patch.object(module, field, value):
                    with self.assertRaisesRegex(RuntimeError, "file-lock API is unavailable"):
                        with lock:
                            self.fail("invalid Windows API admitted guard work")
                    self.assertFalse(events)
                    self.assertTrue(handles[0].closed)
                    self.assertIsNone(lock.handle)
                    with self.assertRaises(OSError):
                        os.fstat(descriptors[0])

    def test_windows_guard_lock_closes_fd_when_native_acquire_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = GuardLock(Path(directory) / "guard.lock")
            events: list[tuple[int, int, int]] = []
            failure = OSError("native acquire failed")
            with windows_lock_fixture(events, failure) as (_, handles, descriptors):
                with self.assertRaises(OSError) as rejected:
                    with lock:
                        self.fail("failed native acquisition admitted guard work")
                self.assertIs(rejected.exception, failure)
                self.assertEqual(events, [(descriptors[0], 1, 1)])
                self.assertTrue(handles[0].closed)
                self.assertIsNone(lock.handle)
                with self.assertRaises(OSError):
                    os.fstat(descriptors[0])

    def test_profile_locks_acquire_failure_closes_unregistered_handle(self) -> None:
        for failure in (None, RuntimeError("native runtime failure"), KeyboardInterrupt(), OSError("native denied")):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                target = Path(directory).resolve()
                epoch = ProfileLocks(target)
                events: list[tuple[int, int, int]] = []
                with windows_lock_fixture(events, failure) as (module, handles, descriptors), \
                        patch.object(module, "LK_NBLCK", "invalid" if failure is None else 2):
                    expected = (RuntimeError if failure is None else
                                BlockingIOError if isinstance(failure, OSError) else type(failure))
                    with self.assertRaises(expected) as rejected:
                        with epoch:
                            self.fail("failed profile acquisition admitted guard work")
                    if isinstance(failure, OSError):
                        self.assertIs(rejected.exception.__cause__, failure)
                    elif failure is not None:
                        self.assertIs(rejected.exception, failure)
                    self.assertEqual(len(handles), 1)
                    self.assertTrue(handles[0].closed)
                    self.assertEqual(epoch.handles, [])
                    self.assertEqual(epoch.identities, [])
                    self.assertEqual(epoch.ancestors, [])
                    with self.assertRaises(OSError):
                        os.fstat(descriptors[0])

    def test_profile_locks_release_failure_closes_all_fds_and_state(self) -> None:
        def release_case(invalid_api: bool) -> None:
            with tempfile.TemporaryDirectory() as directory:
                target = Path(directory).resolve()
                for profile in ("debug", "release"):
                    (target / profile / "deps").mkdir(parents=True)
                epoch = ProfileLocks(target)
                events: list[tuple[int, int, int]] = []
                with windows_lock_fixture(events) as (module, handles, descriptors):
                    epoch.__enter__()
                    ancestor_fds = [descriptor for _, descriptor, _ in epoch.ancestors]
                    native_close = os.close
                    closed_ancestors: list[int] = []
                    released_handles: list[int] = []

                    def unlock(descriptor: int, operation: int, count: int) -> None:
                        released_handles.append(descriptor)
                        self.assertEqual((operation, count), (3, 1))
                        if len(released_handles) == 1:
                            raise RuntimeError("first unlock failed")

                    def close(descriptor: int) -> None:
                        closed_ancestors.append(descriptor)
                        native_close(descriptor)

                    message = "file-lock API is unavailable" if invalid_api else "first unlock failed"
                    with patch.object(module, "LK_UNLCK", "invalid" if invalid_api else 3), \
                            patch.object(module, "locking", unlock), patch.object(os, "close", close), \
                            self.assertRaisesRegex(RuntimeError, message) as rejected:
                        epoch.__exit__(None, None, None)
                    self.assertEqual(released_handles, [] if invalid_api else list(reversed(descriptors)))
                    self.assertEqual(closed_ancestors, list(reversed(ancestor_fds)))
                    self.assertTrue(all(handle.closed for handle in handles))
                    self.assertEqual(epoch.handles, [])
                    self.assertEqual(epoch.identities, [])
                    self.assertEqual(epoch.ancestors, [])
                    for descriptor in [*descriptors, *ancestor_fds]:
                        with self.assertRaises(OSError):
                            os.fstat(descriptor)
                    if invalid_api:
                        error: BaseException | None = rejected.exception
                        chain = []
                        while error is not None:
                            chain.append(error)
                            error = error.__context__
                        self.assertEqual(len(chain), len(handles))

        for invalid_api in (False, True):
            with self.subTest(invalid_api=invalid_api):
                release_case(invalid_api)

    def artifact_fixture(self, directory: str) -> tuple[Path, Path, Path, Path]:
        target = Path(directory).resolve() / "target"
        profile = target / "debug"
        (profile / "deps").mkdir(parents=True)
        fp = profile / ".fingerprint/fixture-1111111111111111"
        fp.mkdir(parents=True)
        (fp / "lib-fixture").write_bytes(b"old-hash")
        artifact = profile / "deps/libfixture-2222222222222222.rlib"
        artifact.write_bytes(b"x" * 8192)
        (profile / "app").write_bytes(b"current-app")
        return target, profile, fp, artifact

    def evict_fixture(
        self, target: Path, protected: tuple[Path, ...] | list[Path] = (), dry_run: bool = False,
    ) -> tuple[int, EvictionReport]:
        with ProfileLocks(target) as epoch:
            current_bytes = self.measured_size(target)
            return evict_artifacts(target, 1, current_bytes, dry_run,
                                   [target / "debug/app", *protected], epoch)

    def measured_size(self, target: Path) -> int:
        value = size_bytes(target)
        if value is None:
            raise AssertionError("fixture target must have an available allocated-byte measurement")
        return value

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_missing_protection_input_blocks_eviction(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact = self.artifact_fixture(directory)
            with ProfileLocks(target) as epoch:
                code, report = evict_artifacts(target, 1, self.measured_size(target), False, [], epoch)
            self.assertEqual(code, 1)
            self.assertIn("protected consumer references", report["error"])
            self.assertTrue(fp.exists())
            self.assertTrue(artifact.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_eviction_invalidates_fingerprint_without_hash_pairing_and_keeps_uplift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact = self.artifact_fixture(directory)
            code, report = self.evict_fixture(target)
            self.assertEqual(code, 0, report)
            self.assertFalse(fp.exists())
            self.assertFalse(artifact.exists())
            self.assertEqual((profile / "app").read_bytes(), b"current-app")
            self.assertEqual(report["deleted_files"], 1)
            self.assertEqual(report["candidate_paths"], report["staged_paths"])

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_unknown_alias_and_explicit_protected_inode_are_retained(self) -> None:
        for explicit in (False, True):
            with self.subTest(explicit=explicit), tempfile.TemporaryDirectory() as directory:
                target, _, fp, artifact = self.artifact_fixture(directory)
                outside = Path(directory) / "consumer"
                os.link(artifact, outside)
                inode = artifact.stat().st_ino
                code, report = self.evict_fixture(target, [outside] if explicit else [])
                self.assertEqual(code, 0, report)
                self.assertEqual(artifact.stat().st_ino, inode)
                self.assertEqual(outside.read_bytes(), b"x" * 8192)
                self.assertTrue(fp.exists())
                reason: Literal["retained_protected_inodes", "retained_unknown_alias_inodes"] = (
                    "retained_protected_inodes" if explicit else "retained_unknown_alias_inodes"
                )
                self.assertEqual(report[reason], 1)

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_closed_hardlink_set_is_evicted_once_in_allocated_metric(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, _, artifact = self.artifact_fixture(directory)
            alias = artifact.with_name("libalias-3333333333333333.rlib")
            os.link(artifact, alias)
            allocated = artifact.stat().st_blocks * 512
            code, report = self.evict_fixture(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["estimated_candidate_bytes"], allocated)
            self.assertEqual(report["deleted_files"], 2)
            self.assertFalse(alias.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_fingerprint_protected_or_unknown_alias_blocks_all_artifact_operations(self) -> None:
        for explicit in (False, True):
            with self.subTest(explicit=explicit), tempfile.TemporaryDirectory() as directory:
                target, _, fp, artifact = self.artifact_fixture(directory)
                hash_file = fp / "lib-fixture"
                if explicit:
                    protected = [hash_file]
                else:
                    os.link(hash_file, Path(directory) / "hash-alias")
                    protected = []
                code, report = self.evict_fixture(target, protected)
                self.assertEqual(code, 1)
                self.assertTrue(fp.exists())
                self.assertEqual(artifact.read_bytes(), b"x" * 8192)
                self.assertEqual(report["staged_paths"], [])

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_dry_run_and_unknown_artifact_names_leave_originals(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact = self.artifact_fixture(directory)
            unknown = profile / "deps/unknown.bin"
            unknown.write_bytes(b"retain")
            code, report = self.evict_fixture(target, dry_run=True)
            self.assertEqual(code, 0)
            self.assertTrue(artifact.exists())
            self.assertTrue(fp.exists())
            self.assertEqual(report["staged_paths"], [])
            code, report = self.evict_fixture(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(unknown.read_bytes(), b"retain")

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_auxiliary_eviction_also_invalidates_all_fingerprints(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact = self.artifact_fixture(directory)
            auxiliary = artifact.with_suffix(".d")
            artifact.rename(auxiliary)
            code, report = self.evict_fixture(target)
            self.assertEqual(code, 0, report)
            self.assertFalse(auxiliary.exists())
            self.assertFalse(fp.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_dry_run_uses_metadata_without_opening_regular_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact = self.artifact_fixture(directory)
            with patch("cargo_capacity_guard_lock._regular", side_effect=AssertionError("regular FD opened")):
                code, report = self.evict_fixture(target, dry_run=True)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["candidate_paths"], [str(artifact.relative_to(target))])
            self.assertTrue(fp.exists())
            self.assertTrue(artifact.exists())
            self.assertEqual(report["staged_paths"], [])

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_actual_metadata_revalidates_selected_without_rechecking_unselected(self) -> None:
        from cargo_capacity_guard_lock import _metadata_at
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _, artifact = self.artifact_fixture(directory)
            unselected = artifact.with_name("libnewer-3333333333333333.rlib")
            unselected.write_bytes(b"newer" * 2048)
            os.utime(artifact, ns=(1, 1))
            opened = []
            def observe(descriptor: int, name: str, device: int) -> os.stat_result:
                opened.append(name)
                return _metadata_at(descriptor, name, device)
            current = self.measured_size(target)
            needed = artifact.stat().st_blocks * 512
            with ProfileLocks(target) as epoch, patch("cargo_capacity_guard_lock._metadata_at", side_effect=observe), patch("cargo_capacity_guard_lock._regular", side_effect=AssertionError("payload FD opened")):
                code, report = evict_artifacts(target, current - needed, current, False,
                                               [profile / "app"], epoch)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["candidate_paths"], [str(artifact.relative_to(target))])
            self.assertIn(artifact.name, opened)
            self.assertEqual(opened.count(unselected.name), 1)
            self.assertTrue(unselected.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_selected_symlink_mutation_is_rejected_before_fingerprint_invalidation(self) -> None:
        from cargo_capacity_guard_lock import _metadata_at
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact = self.artifact_fixture(directory)
            referent = Path(directory).resolve() / "referent"
            referent.write_bytes(b"retain-referent")
            backup = Path(directory).resolve() / "saved-artifact"
            calls = 0
            def mutate(descriptor: int, name: str, device: int) -> os.stat_result:
                nonlocal calls
                if name == artifact.name:
                    calls += 1
                if name == artifact.name and calls == 3 and not backup.exists():
                    artifact.rename(backup)
                    artifact.symlink_to(referent)
                return _metadata_at(descriptor, name, device)
            with patch("cargo_capacity_guard_lock._metadata_at", side_effect=mutate):
                code, report = self.evict_fixture(target)
            self.assertEqual(code, 1, report)
            self.assertTrue(artifact.is_symlink())
            self.assertTrue(fp.exists())
            self.assertEqual(referent.read_bytes(), b"retain-referent")
            self.assertEqual(report["staged_paths"], [])
            self.assertEqual(report["deleted_files"], 0)

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_interruption_before_invalidation_preserves_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact = self.artifact_fixture(directory)
            with patch("cargo_capacity_guard_lock.os.rename", side_effect=OSError("owned interruption")):
                code, report = self.evict_fixture(target)
            self.assertEqual(code, 1)
            self.assertTrue(fp.exists())
            self.assertTrue(artifact.exists())
            self.assertEqual(report["deleted_files"], 0)

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_source_leaf_exchange_at_rename_retains_replacement_and_referent(self) -> None:
        for symlink in (False, True):
            with self.subTest(symlink=symlink), tempfile.TemporaryDirectory() as directory:
                target, _, fp, artifact = self.artifact_fixture(directory)
                referent = Path(directory).resolve() / "referent"
                referent.write_bytes(b"retain-referent")
                backup = Path(directory).resolve() / "original-artifact"
                replacement = Path(directory).resolve() / "replacement"
                replacement.write_bytes(b"replacement")
                original = os.rename
                def exchange(
                    source: str,
                    destination: str,
                    *,
                    src_dir_fd: int | None = None,
                    dst_dir_fd: int | None = None,
                    artifact_name: str = artifact.name,
                    rename: Callable[..., None] = original,
                    backup_path: Path = backup,
                    is_symlink: bool = symlink,
                    referent_path: Path = referent,
                    replacement_path: Path = replacement,
                ) -> None:
                    if source == artifact_name:
                        rename(source, str(backup_path), src_dir_fd=src_dir_fd)
                        if is_symlink:
                            os.symlink(referent_path, source, dir_fd=src_dir_fd)
                        else:
                            rename(str(replacement_path), source, dst_dir_fd=src_dir_fd)
                    rename(source, destination, src_dir_fd=src_dir_fd, dst_dir_fd=dst_dir_fd)
                with patch("cargo_capacity_guard_lock.os.rename", side_effect=exchange):
                    code, report = self.evict_fixture(target)
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertTrue(report["recovery_required_paths"])
                self.assertFalse(fp.exists())
                self.assertEqual(backup.read_bytes(), b"x" * 8192)
                self.assertEqual(referent.read_bytes(), b"retain-referent")

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_stage_leaf_or_alias_change_refuses_unlink_and_preserves_dirty_state(self) -> None:
        from cargo_capacity_guard_lock import _metadata_at
        for alias in (False, True):
            with self.subTest(alias=alias), tempfile.TemporaryDirectory() as directory:
                target, _, fp, _ = self.artifact_fixture(directory)
                outside = Path(directory).resolve() / "outside"
                replacement = Path(directory).resolve() / "replacement"
                replacement.write_bytes(b"replacement")
                calls = 0
                def change(
                    descriptor: int,
                    name: str,
                    device: int,
                    *,
                    is_alias: bool = alias,
                    outside_path: Path = outside,
                    replacement_path: Path = replacement,
                ) -> os.stat_result:
                    nonlocal calls
                    if name.startswith(".capacity-artifact-"):
                        calls += 1
                        if calls == 2:
                            if is_alias:
                                os.link(name, outside_path, src_dir_fd=descriptor)
                            else:
                                os.rename(name, str(outside_path), src_dir_fd=descriptor)
                                os.rename(str(replacement_path), name, dst_dir_fd=descriptor)
                    return _metadata_at(descriptor, name, device)
                with patch("cargo_capacity_guard_lock._metadata_at", side_effect=change):
                    code, report = self.evict_fixture(target)
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertTrue(report["recovery_required_paths"])
                self.assertFalse(fp.exists())
                self.assertTrue(outside.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_leaf_metadata_checks_do_not_scale_with_other_fingerprint_directories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _, artifact = self.artifact_fixture(directory)
            for index in range(100):
                fp = profile / ".fingerprint" / f"extra-{index}"
                fp.mkdir()
                (fp / "hash").write_bytes(b"old")
            for index in range(3):
                artifact.with_name(f"libother-{index:016x}.rlib").write_bytes(b"x" * 8192)
            original_stat, original_fstat = os.stat, os.fstat
            original_rename, original_unlink = os.rename, os.unlink
            calls = 0
            events: dict[str, list[int]] = {"move": [], "purge": []}
            def count_stat(
                path: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *, dir_fd: int | None = None, follow_symlinks: bool = True,
            ) -> os.stat_result:
                nonlocal calls
                calls += 1
                return original_stat(path, dir_fd=dir_fd, follow_symlinks=follow_symlinks)
            def count_fstat(descriptor: int) -> os.stat_result:
                nonlocal calls
                calls += 1
                return original_fstat(descriptor)
            def move(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                destination: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *, src_dir_fd: int | None = None, dst_dir_fd: int | None = None,
            ) -> None:
                if str(destination).startswith(".capacity-artifact-"):
                    events["move"].append(calls)
                original_rename(source, destination, src_dir_fd=src_dir_fd, dst_dir_fd=dst_dir_fd)
            def purge(name: str, *, dir_fd: int | None = None) -> None:
                if str(name).startswith(".capacity-artifact-"):
                    events["purge"].append(calls)
                original_unlink(name, dir_fd=dir_fd)
            with patch("cargo_capacity_guard_lock.os.stat", side_effect=count_stat), \
                    patch("cargo_capacity_guard_lock.os.fstat", side_effect=count_fstat), \
                    patch("cargo_capacity_guard_lock.os.rename", side_effect=move), \
                    patch("cargo_capacity_guard_lock.os.unlink", side_effect=purge):
                code, report = self.evict_fixture(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["deleted_files"], 4)
            for phase, observations in events.items():
                self.assertEqual(len(observations), 4, phase)
                self.assertLess(max(observations[index + 1] - left for index, left in enumerate(observations[:-1])), 100, phase)

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_interruption_after_invalidation_does_not_restore_old_hash(self) -> None:
        for after_purge in (False, True):
            with self.subTest(after_purge=after_purge), tempfile.TemporaryDirectory() as directory:
                target, profile, fp, artifact = self.artifact_fixture(directory)
                other = artifact.with_name("libother-4444444444444444.rlib")
                other.write_bytes(b"y" * 8192)
                original = os.unlink if after_purge else os.rename
                calls = 0
                def interrupted(
                    path: str,
                    *args: str,
                    rename_operation: Callable[..., None] = original,
                    **kwargs: int | None,
                ) -> None:
                    nonlocal calls
                    calls += 1
                    if calls == 2:
                        raise OSError("owned interruption")
                    rename_operation(path, *args, **kwargs)
                with patch("cargo_capacity_guard_lock.os.unlink" if after_purge else "cargo_capacity_guard_lock.os.rename", interrupted):
                    code, report = self.evict_fixture(target)
                self.assertEqual(code, 1)
                self.assertFalse(fp.exists())
                self.assertTrue(list(profile.glob(".capacity-invalidated-*")))
                self.assertEqual(report["deleted_files"], 1 if after_purge else 0)
                before = self.measured_size(target)
                code, recovery = self.evict_fixture(target)
                self.assertEqual(code, 1)
                self.assertTrue(recovery["recovery_required_paths"])
                self.assertEqual(recovery["deleted_files"], 0)
                self.assertEqual(self.measured_size(target), before)
                self.assertFalse(fp.exists())

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_interrupted_profile_is_visible_with_explicit_recovery_information(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact = self.artifact_fixture(directory)
            original = os.rename
            def interrupted(
                path: str,
                destination: str,
                *,
                rename_operation: Callable[..., None] = original,
                src_dir_fd: int | None = None,
                dst_dir_fd: int | None = None,
            ) -> None:
                if path == artifact.name:
                    raise OSError("owned interruption after invalidation")
                rename_operation(path, destination, src_dir_fd=src_dir_fd, dst_dir_fd=dst_dir_fd)
            with patch("cargo_capacity_guard_lock.os.rename", interrupted):
                code, report = self.evict_fixture(target)
            self.assertEqual(code, 1)
            self.assertFalse(fp.parent.exists())
            before = artifact.stat().st_ino
            code, recovery = self.evict_fixture(target)
            self.assertEqual(code, 1)
            self.assertEqual(recovery["recovery_required_profile"], "debug")
            self.assertTrue(recovery["recovery_required_paths"])
            self.assertEqual(artifact.stat().st_ino, before)
            self.assertEqual(recovery["staged_paths"], [])

    @unittest.skipIf(os.name == "nt", "POSIX native no-follow eviction fixture")
    def test_post_capacity_failure_blocks_command_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _, _ = self.artifact_fixture(directory)
            root = Path(directory)
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes",
                                        "metric_version": 1, "target": str(target)}))
            marker = root / "command-ran"
            args = argparse.Namespace(target=str(target), target_dir=None, state=str(state), factor=2.0,
                                      dry_run=False, print_json=True, protected_ref=[profile / "app"],
                                      command=[sys.executable, "-c", f"open({str(marker)!r},'w').close()"])
            output = io.StringIO()
            with patch("cargo_capacity_guard._root", return_value=root), \
                 patch("cargo_capacity_guard.busy_processes", return_value=[]), \
                 contextlib.redirect_stdout(output):
                result = run_guard(args)
            report = json.loads(output.getvalue())
            self.assertEqual(result, 1)
            self.assertEqual(report["cleanup_status"], "incomplete")
            self.assertEqual(report["threshold_bytes"], 2)
            self.assertEqual(len(report["cleanup_attempts"]), 1)
            self.assertFalse(marker.exists())
            self.assertTrue((profile / "app").exists())

    def test_windows_inspection_unavailable_skips_native_cleanup_and_command(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            target.mkdir()
            (target / "artifact").write_bytes(b"x" * 8192)
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes",
                                        "metric_version": 1, "target": str(target)}))
            with patch("cargo_capacity_guard_lock.os.name", "nt"):
                busy = busy_processes(target, root)
            self.assertEqual(busy, ["process inspection unavailable on Windows"])
            args = argparse.Namespace(target=str(target), target_dir=None, state=str(state), factor=2.0,
                                      dry_run=False, print_json=True, command=["never-run"])
            output = io.StringIO()
            with patch("cargo_capacity_guard._root", return_value=root), \
                 patch("cargo_capacity_guard.busy_processes", return_value=busy), \
                 patch("cargo_capacity_guard.size_bytes", return_value=8192), \
                 patch("cargo_capacity_guard.ProfileLocks") as locks, \
                 patch("cargo_capacity_guard.subprocess.run") as command, \
                 contextlib.redirect_stdout(output):
                result = run_guard(args)
            self.assertEqual(result, 1)
            self.assertEqual(json.loads(output.getvalue())["clean_skipped"], "busy")
            locks.assert_not_called()
            command.assert_not_called()

    def test_missing_baseline_reports_uncalibrated_without_cleaning(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            artifact = target / "existing"
            artifact.write_bytes(b"existing build")
            result = subprocess.run(
                [sys.executable, str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"),
                 "--root", str(root), "--target", str(target), "--print-json"],
                cwd=root, capture_output=True, text=True, check=True,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report["cleanup_status"], "uncalibrated")
            self.assertIsNone(report["baseline_bytes"])
            self.assertEqual(artifact.read_bytes(), b"existing build")

    @unittest.skipIf(os.name == "nt", "allocated block metric unavailable on Windows")
    def test_size_counts_unique_inode_allocated_blocks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            target.mkdir()
            original = target / "original"
            original.write_bytes(b"x" * 4096)
            duplicate = target / "duplicate"
            os.link(original, duplicate)
            sparse = target / "sparse"
            with sparse.open("wb") as handle:
                handle.truncate(1024 * 1024)
            expected = sum(
                stat.st_blocks * 512
                for stat in (original.stat(), sparse.stat())
            )
            self.assertEqual(self.measured_size(target), expected)
            self.assertLess(self.measured_size(target), original.stat().st_size + duplicate.stat().st_size + sparse.stat().st_size)

    def test_legacy_logical_size_state_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "target": str(target)}))
            result = subprocess.run(
                [sys.executable, str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"),
                 "--root", str(root), "--target", str(target), "--state", str(state)],
                cwd=root, capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("metric is missing or obsolete", result.stderr)

    def test_unavailable_metric_skips_capacity_and_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            with patch("cargo_capacity_guard.size_bytes", return_value=None):
                report = inspect_capacity(root, target, state, 2.0)
            self.assertFalse(report["metric_available"])
            self.assertIsNone(report["current_bytes"])
            self.assertFalse(report["over_capacity"])
            self.assertIsNone(report["baseline_bytes"])

    def test_metadata_uses_project_configuration_from_another_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "Cargo.toml").write_text("[package]\nname='configured-fixture'\nversion='0.1.0'\nedition='2021'\n")
            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}\n")
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text('[build]\ntarget-dir="build-output"\n')
            self.assertEqual(resolve_target(root), root / "build-output")

    def test_rejects_escape_and_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outside = Path(directory).parent / f"{root.name}-outside"
            outside.mkdir()
            self.assertRaises(RuntimeError, resolve_target, root, str(outside))
            target = root / "target"
            target.symlink_to(outside)
            self.assertRaises(RuntimeError, resolve_target, root, "target")

    def test_size_and_dry_run_do_not_clean(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target/debug/incremental"
            target.mkdir(parents=True)
            (target / "old").write_bytes(b"x" * 20)
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes", "metric_version": 1, "target": str(root / "target")}))
            command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(root / "target"), "--state", str(state), "--dry-run", "--print-json", "--protected-ref", str(target / "old")]
            result = subprocess.run(command, cwd=Path(__file__).parent, text=True, capture_output=True, check=True)
            self.assertTrue((target / "old").exists())
            report = json.loads(result.stdout)
            self.assertTrue(report["over_capacity"])
            self.assertIn(report["cleanup_status"], {"dry-run", "failed"})
            self.assertEqual(report["post_clean_bytes"], report["current_bytes"])
            self.assertEqual(report["actual_removed_bytes"], 0)
            self.assertTrue(report["remaining_over_capacity"])

    def test_success_sets_baseline_only_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            (root / "Cargo.toml").write_text("[package]\nname='guard-fixture'\nversion='0.1.0'\nedition='2021'\n")
            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}\n")
            state = root / "state.json"
            command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--state", str(state), "--", "cargo", "build"]
            subprocess.run(command, cwd=root, check=True)
            baseline = json.loads(state.read_text())["baseline_bytes"]
            subprocess.run(command, cwd=root, check=True)
            self.assertEqual(json.loads(state.read_text())["baseline_bytes"], baseline)

    def test_successful_build_on_dirty_target_does_not_record_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            (target / "preexisting-artifact").write_bytes(b"old")
            (root / "Cargo.toml").write_text("[package]\nname='dirty-guard-fixture'\nversion='0.1.0'\nedition='2021'\n")
            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}\n")
            state = root / "state.json"
            command = [sys.executable, str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--state", str(state), "--print-json", "--", "cargo", "build"]
            result = subprocess.run(command, cwd=root, check=True, capture_output=True, text=True)
            report = json.loads(result.stdout)
            self.assertFalse(report["baseline_eligible"])
            self.assertFalse(state.exists())

    def test_failed_build_does_not_create_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--state", str(state), "--", "python3", "-c", "raise SystemExit(7)"]
            result = subprocess.run(command, cwd=root, check=False)
            self.assertEqual(result.returncode, 7)
            self.assertFalse(state.exists())

    def test_normal_wrapper_report_is_stderr_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--", "python3", "-c", "pass"]
            result = subprocess.run(command, cwd=root, check=True, capture_output=True, text=True)
            self.assertEqual(result.stdout, "")
            report = json.loads(result.stderr)
            self.assertIn("over_capacity", report)

    def test_non_cargo_command_named_build_does_not_create_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--state", str(state), "--", "python3", "-c", "open('target/output','wb').write(b'abc')", "build"]
            subprocess.run(command, cwd=root, check=True)
            self.assertFalse(state.exists())

    def test_build_command_shape_is_strict(self) -> None:
        self.assertTrue(is_successful_build(["cargo", "+stable", "build"]))
        self.assertTrue(is_successful_build(["rtk", "proxy", "cargo", "build"]))
        self.assertTrue(is_successful_build(["rtk", "cargo", "+stable", "build"]))
        self.assertFalse(is_successful_build(["python3", "build"]))
        self.assertFalse(is_successful_build(["cargo", "check"]))
        self.assertFalse(is_successful_build(["cargo", "build", "--help"]))

    @unittest.skipIf(os.name == "nt", "fcntl lock fixture is POSIX-only")
    def test_held_real_profile_lock_skips_cleanup(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            lock_path = target / "debug/.cargo-lock"
            lock_path.parent.mkdir(parents=True)
            (target / "debug/artifact").write_bytes(b"x" * 4)
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes", "metric_version": 1, "target": str(target)}))
            holder_code = "import fcntl,os,time; h=open(os.environ['GUARD_LOCK_PATH'],'a+'); fcntl.flock(h.fileno(), fcntl.LOCK_EX); print('ready',flush=True); time.sleep(10)"
            holder_env = os.environ.copy()
            holder_env["GUARD_LOCK_PATH"] = str(lock_path)
            holder = subprocess.Popen([sys.executable, "-c", holder_code], env=holder_env, stdout=subprocess.PIPE, text=True)
            try:
                holder_stdout = holder.stdout
                if holder_stdout is None:
                    raise AssertionError("lock holder stdout pipe was not created")
                self.assertEqual(holder_stdout.readline().strip(), "ready")
                command = ["python3", str(Path(__file__).resolve().parent / "cargo_capacity_guard.py"), "--root", str(root), "--target", str(target), "--state", str(state), "--dry-run", "--print-json"]
                result = subprocess.run(command, cwd=root, check=True, capture_output=True, text=True)
                report = json.loads(result.stdout)
                self.assertIn(report["clean_skipped"], {"busy", "cargo-lock-busy"})
                if report["clean_skipped"] == "cargo-lock-busy":
                    self.assertIn(".cargo-lock", report["busy_lock"])
            finally:
                if holder_stdout is not None:
                    holder_stdout.close()
                holder.terminate()
                holder.wait(timeout=5)

    def test_invalid_factor_and_state_target_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            script = str(Path(__file__).resolve().parent / "cargo_capacity_guard.py")
            for factor in ("nan", "inf", "0", "-2"):
                result = subprocess.run(["python3", script, "--root", str(root), "--target", str(target), "--factor", factor], cwd=root, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, factor)
            state = root / "state.json"
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes", "metric_version": 1, "target": str(root / "other-target")}))
            result = subprocess.run(["python3", script, "--root", str(root), "--target", str(target), "--state", str(state)], cwd=root, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            state.write_text(json.dumps({"baseline_bytes": 1, "metric": "unique-inode-allocated-bytes", "metric_version": 1}))
            result = subprocess.run(["python3", script, "--root", str(root), "--target", str(target), "--state", str(state)], cwd=root, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("requires its recorded target", result.stderr)

    def test_target_dir_conflict_and_child_symlink_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            (target / "linked").symlink_to(root)
            with self.assertRaises(RuntimeError):
                resolve_target(root, str(target))
            script = str(Path(__file__).resolve().parent / "cargo_capacity_guard.py")
            result = subprocess.run(["python3", script, "--root", str(root), "--target", str(target), "--target-dir", str(target)], cwd=root, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)

    def test_root_and_metadata_mismatch_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text("[package]\nname='metadata-fixture'\nversion='0.1.0'\nedition='2021'\n")
            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}\n")
            self.assertRaises(RuntimeError, resolve_target, root, str(root))
            self.assertRaises(RuntimeError, resolve_target, root, str(root / "other-target"))

    def test_process_inspection_failure_is_busy(self) -> None:
        class Result:
            returncode = 1
            stdout = ""

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch("cargo_capacity_guard_lock.subprocess.run", return_value=Result()):
                busy = busy_processes(root / "target", root)
            self.assertTrue(busy)

    def test_lsof_failure_ignores_only_process_confirmed_gone(self) -> None:
        class LsofResult:
            returncode = 1
            stdout = ""

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
            child.terminate()
            child.wait(timeout=5)
            ps_result = SimpleNamespace(
                returncode=0,
                stdout=f"{child.pid} 1 cargo build --workspace\n",
            )
            with patch("cargo_capacity_guard_lock.os.getpid", return_value=999999), \
                    patch("cargo_capacity_guard_lock.subprocess.run", side_effect=[ps_result, LsofResult()]):
                busy = busy_processes(root / "target", root)
            self.assertEqual(busy, [])

    def test_lsof_failure_stays_busy_for_alive_or_unverifiable_process(self) -> None:
        class LsofResult:
            returncode = 1
            stdout = ""

        for failure in (PermissionError("permission denied"), OSError("unknown process state")):
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                ps_result = SimpleNamespace(returncode=0, stdout="424242 1 cargo build --workspace\n")
                with patch("cargo_capacity_guard_lock.os.getpid", return_value=999999), \
                        patch("cargo_capacity_guard_lock.subprocess.run", side_effect=[ps_result, LsofResult()]), \
                        patch("cargo_capacity_guard_lock.os.kill", side_effect=failure):
                    busy = busy_processes(root / "target", root)
                self.assertEqual(busy, ["pid 424242: lsof failed"])

    def test_target_binary_is_busy_even_without_cargo_name(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            class Result:
                returncode = 0
                stdout = f"123 1 /tmp/katana_render_runtime_test {target}/debug/app --token guard-test-secret\n"
            with patch("cargo_capacity_guard_lock.os.getpid", return_value=999), patch(
                "cargo_capacity_guard_lock.subprocess.run", return_value=Result()
            ):
                busy = busy_processes(target, root)
            self.assertEqual(len(busy), 1)
            self.assertIn("123", busy[0])
            self.assertNotIn("guard-test-secret", busy[0])
            self.assertNotIn("--token", busy[0])

    def test_invalid_baseline_values_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            script = str(Path(__file__).resolve().parent / "cargo_capacity_guard.py")
            for value in (True, -1, 0, "bad", float("nan")):
                state = root / f"state-{len(list(root.iterdir()))}.json"
                state.write_text(json.dumps({"baseline_bytes": value, "target": str(target), "allow_nan": True}))
                result = subprocess.run(["python3", script, "--root", str(root), "--target", str(target), "--state", str(state)], cwd=root, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2, value)


    def test_incremental_prefixes_bind_saved_actual_runtime_targets_only(self) -> None:
        root = Path('/fixture-workspace')
        targets = [
            {'name': 'katana_render_runtime', 'kind': ['lib']},
            {'name': 'html_browser_preview', 'kind': ['example']},
            {'name': 'html_css_probe', 'kind': ['example']},
            {'name': 'html_fidelity_probe', 'kind': ['example']},
            {'name': 'html_initial_frame_probe', 'kind': ['example']},
            {'name': 'drawio_katana_transfer', 'kind': ['test']},
            {'name': 'drawio_official_transfer', 'kind': ['test']},
            {'name': 'html_dom_coverage', 'kind': ['test']},
            {'name': 'html_public_layout', 'kind': ['test']},
            {'name': 'html_table_vertical_alignment', 'kind': ['test']},
            {'name': 'mathjax_renderer', 'kind': ['test']},
            {'name': 'mermaid_katana_transfer', 'kind': ['test']},
            {'name': 'plantuml_renderer', 'kind': ['test']},
            {'name': 'release_check', 'kind': ['test']},
            {'name': 'renderer_transfer', 'kind': ['test']},
            {'name': 'build-script-build', 'kind': ['custom-build']},
        ]
        metadata = {'packages': [
            {'manifest_path': str(root / 'crates/katana-render-runtime/Cargo.toml'),
             'targets': targets},
            {'manifest_path': str(root / 'crates/katana-render-runtime-cli/Cargo.toml'),
             'targets': [{'name': 'krr', 'kind': ['bin']}]},
        ]}
        prefixes = runtime_incremental_prefixes(root, metadata)
        self.assertEqual(len(prefixes), 15)
        self.assertIn('katana_render_runtime-', prefixes)
        self.assertNotIn('build_script_build-', prefixes)
        self.assertNotIn('krr-', prefixes)
        self.assertEqual(runtime_incremental_prefixes(root / 'unknown', metadata), [])

    def incremental_fixture(
        self, directory: str,
    ) -> tuple[Path, Path, Path, Path, Path, Path]:
        target, profile, fp, artifact = self.artifact_fixture(directory)
        session = profile / 'incremental/fixture-abc/s-owned-0-cache'
        session.mkdir(parents=True)
        cache = session / 'dep-graph.bin'
        cache.write_bytes(b'cache' * 2048)
        for name in ['query-cache.bin', 'work-products.bin']:
            (session / name).write_bytes(b'index' * 2048)
        obj = session / 'product.o'
        obj.write_bytes(b'object' * 2048)
        return target, profile, fp, artifact, cache, obj

    def incremental_evict(
        self, target: Path, protected: tuple[Path, ...] | list[Path] = (), dry_run: bool = False,
    ) -> tuple[int, EvictionReport]:
        with ProfileLocks(target) as epoch:
            return evict_artifacts(target, 1, self.measured_size(target), dry_run,
                                   [target / 'debug/app', *protected], epoch,
                                   incremental_prefixes=['fixture-'])

    @staticmethod
    def rust_base36(value: int, width: int) -> str:
        digits = "0123456789abcdefghijklmnopqrstuvwxyz"
        result = ""
        while value:
            value, remainder = divmod(value, 36)
            result = digits[remainder] + result
        return result.rjust(width, "0")

    def retained_incremental_fixture(
        self, directory: str,
    ) -> tuple[Path, Path, Path, Path, Path, Path, Path]:
        target, profile, fp, artifact = self.artifact_fixture(directory)
        owner = profile / "incremental/fixture-owner-123456"
        owner.mkdir(parents=True)
        now_micros = time.time_ns() // 1000
        old_stamp = self.rust_base36(now_micros - 30_000_000, 10)
        latest_stamp = self.rust_base36(now_micros - 1_000_000, 10)
        working_stamp = self.rust_base36(now_micros - 20_000_000, 10)
        old_random = "0000001"
        latest_random = "0000002"
        working_random = "0000003"
        svh = "a" * 25
        old = owner / f"s-{old_stamp}-{old_random}-{svh}"
        latest = owner / f"s-{latest_stamp}-{latest_random}-{svh}"
        working = owner / f"s-{working_stamp}-{working_random}-working"
        for session, names in (
            (old, ("dep-graph.bin", "query-cache.bin", "work-products.bin", "product.o")),
            (latest, ("dep-graph.bin", "query-cache.bin", "work-products.bin")),
            (working, ("dep-graph.part.bin",)),
        ):
            session.mkdir()
            for name in names:
                (session / name).write_bytes((name.encode() + b"-") * 4096)
            (owner / f"s-{session.name.split('-')[1]}-{session.name.split('-')[2]}.lock").write_bytes(b"")
        build = profile / "build/fixture/out/product.o"
        build.parent.mkdir(parents=True)
        build.write_bytes(b"preserve build output")
        example = profile / "examples/fixture-example-1234567890abcdef"
        example.parent.mkdir(parents=True)
        example.write_bytes(b"preserve example output")
        return target, profile, fp, artifact, old, latest, working

    @staticmethod
    def file_tree_bytes(root: Path) -> dict[str, bytes]:
        return {path.relative_to(root).as_posix(): path.read_bytes()
                for path in sorted(root.rglob("*")) if path.is_file()}

    def retained_incremental_evict(
        self, target: Path, current_bytes: int, threshold: int, dry_run: bool = False,
        protected_refs: list[Path] | None = None,
    ) -> tuple[int, EvictionReport]:
        profile = target / "debug"
        with ProfileLocks(target) as epoch:
            return evict_artifacts(
                target, threshold, current_bytes, dry_run,
                ([target / "debug/app"] if protected_refs is None else protected_refs), epoch,
                incremental_prefixes=["fixture-"], retained_profiles=[profile],
            )

    def auxiliary_incremental_evict(
        self, target: Path, current_bytes: int, threshold: int, dry_run: bool = False,
        profiles: list[Path] | None = None, auxiliary_profiles: list[Path] | None = None,
    ) -> tuple[int, EvictionReport]:
        verified = [target / "debug"] if auxiliary_profiles is None else auxiliary_profiles
        retained = verified if profiles is None else profiles
        with ProfileLocks(target) as epoch:
            return evict_artifacts(
                target, threshold, current_bytes, dry_run, [], epoch,
                incremental_prefixes=["fixture-"], inactive_prefixes=["other-"],
                retained_profiles=retained, auxiliary_only_profiles=verified,
            )

    @staticmethod
    def file_tree_identity_bytes(root: Path) -> dict[str, tuple[int, int, int, bytes]]:
        return {
            path.relative_to(root).as_posix(): (
                path.stat().st_dev, path.stat().st_ino, path.stat().st_size, path.read_bytes(),
            )
            for path in sorted(root.rglob("*")) if path.is_file() and path.name != ".cargo-lock"
        }

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_verified_auxiliary_mode_dry_run_and_cleanup_preserve_build_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact, old, latest, working = self.retained_incremental_fixture(directory)
            build = profile / "build/fixture/out/product.o"
            example = profile / "examples/fixture-example-1234567890abcdef"
            release = target / "release"
            (release / "deps").mkdir(parents=True)
            (release / ".fingerprint/release-fixture-1111111111111111").mkdir(parents=True)
            release_artifact = release / "deps/librelease-fixture-1111111111111111.rlib"
            release_artifact.write_bytes(b"preserve other build profile")
            inactive = profile / "incremental/other-package-123456/s-unselected-entry"
            inactive.mkdir(parents=True)
            (inactive / "unknown.bin").write_bytes(b"preserve outside runtime prefix")
            available_files = [*old.iterdir(), working / "dep-graph.part.bin"]
            available = sum(path.stat().st_blocks * 512 for path in available_files)
            before = self.file_tree_identity_bytes(target)
            fingerprint_before = self.file_tree_bytes(fp)
            removed_paths = set(available_files)
            removed_paths.update(
                session.parent / ("-".join(session.name.split("-")[:3]) + ".lock")
                for session in (old, working)
            )
            removed_names = {path.relative_to(target).as_posix() for path in removed_paths}
            preserved = {name: value for name, value in before.items() if name not in removed_names}

            code, preview = self.auxiliary_incremental_evict(
                target, available + 1, 1, dry_run=True, profiles=[profile, release],
            )
            expected = {str(path.relative_to(target)) for path in available_files}
            self.assertEqual(code, 0, preview)
            self.assertEqual(set(preview["candidate_paths"]), expected)
            self.assertEqual(preview["invalidated_profiles"], [])
            self.assertTrue(all("/deps/" not in path and "/examples/" not in path
                                and "/.fingerprint/" not in path and not path.startswith("release/")
                                for path in preview["candidate_paths"]))
            self.assertEqual(self.file_tree_identity_bytes(target), before)

            code, report = self.auxiliary_incremental_evict(
                target, available + 1, 1, profiles=[profile, release],
            )
            self.assertEqual(code, 0, report)
            self.assertEqual(report["deleted_files"], len(available_files))
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertEqual(report.get("removed_incremental_sessions"), 2)
            self.assertFalse(old.exists())
            self.assertFalse(working.exists())
            self.assertTrue(latest.exists())
            self.assertTrue((latest.parent / ("-".join(latest.name.split("-")[:3]) + ".lock")).exists())
            self.assertFalse((old.parent / ("-".join(old.name.split("-")[:3]) + ".lock")).exists())
            self.assertFalse((working.parent / ("-".join(working.name.split("-")[:3]) + ".lock")).exists())
            self.assertTrue(release_artifact.exists())
            after = self.file_tree_identity_bytes(target)
            self.assertEqual({name: after[name] for name in preserved}, preserved)
            self.assertEqual(self.file_tree_bytes(fp), fingerprint_before)
            self.assertEqual(artifact.read_bytes(), before["debug/deps/libfixture-2222222222222222.rlib"][3])
            self.assertEqual(build.read_bytes(), before["debug/build/fixture/out/product.o"][3])
            self.assertEqual(example.read_bytes(), before["debug/examples/fixture-example-1234567890abcdef"][3])

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_verified_auxiliary_mode_shortfall_deletes_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _fp, _artifact, old, _latest, working = self.retained_incremental_fixture(directory)
            available_files = [*old.iterdir(), working / "dep-graph.part.bin"]
            available = sum(path.stat().st_blocks * 512 for path in available_files)
            before = self.file_tree_identity_bytes(target)
            code, report = self.auxiliary_incremental_evict(
                target, available + 2, 1, profiles=[profile],
            )
            self.assertEqual(code, 1, report)
            self.assertIn("insufficient", report["error"])
            self.assertEqual(report["deleted_files"], 0)
            self.assertEqual(report["staged_paths"], [])
            self.assertEqual(self.file_tree_identity_bytes(target), before)

    def test_unverified_profile_or_non_build_cannot_authorize_auxiliary_cleanup(self) -> None:
        for label, manual_profile, command, build_profile in (
            ("manual-retention", True, [sys.executable, "-c", "raise SystemExit(0)"], None),
            ("cargo-command-without-verified-retention", False, ["cargo", "build"], None),
        ):
            with self.subTest(case=label), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target, profile, _fp, _artifact = self.artifact_fixture(directory)
                state = root / "state.json"
                state.write_text(json.dumps({
                    "baseline_bytes": 1, "metric": "unique-inode-allocated-bytes",
                    "metric_version": 1, "target": str(target),
                }))
                marker = root / "command-ran"
                child = [*command]
                if child[0] == sys.executable:
                    child[-1] = f"open({str(marker)!r}, 'w').close()"
                args = argparse.Namespace(
                    target=str(target), target_dir=None, state=str(state), factor=2.0,
                    dry_run=False, print_json=True, protected_ref=[],
                    retain_profile=[str(profile)] if manual_profile else [],
                    retain_build_profile=build_profile, coverage_domain=None,
                    expected_state_sha256=None, command=child,
                )
                output = io.StringIO()
                environment = os.environ.copy()
                environment.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
                environment.pop("CARGO_CAPACITY_FACTOR", None)
                with patch("cargo_capacity_guard._root", return_value=root), \
                        patch("cargo_capacity_guard.busy_processes", return_value=[]), \
                        patch.dict(os.environ, environment, clear=True), \
                        contextlib.redirect_stdout(output):
                    result = run_guard(args)
                report = json.loads(output.getvalue())
                self.assertEqual(result, 1)
                self.assertIn("exact protected consumer references", report["finite_cleanup"]["error"])
                self.assertFalse(marker.exists())

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, _profile, _fp, _artifact = self.artifact_fixture(directory)
            args = argparse.Namespace(
                target=str(target), target_dir=None, state=None, factor=2.0,
                dry_run=False, print_json=True, protected_ref=[], retain_profile=[],
                retain_build_profile="debug", coverage_domain=None,
                expected_state_sha256=None, command=[sys.executable, "-c", "pass"],
            )
            with patch("cargo_capacity_guard._root", return_value=root):
                with self.assertRaisesRegex(RuntimeError, "direct Cargo build"):
                    run_guard(args)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_verified_build_metadata_alone_authorizes_scoped_dry_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, profile, _fp, _artifact, old, _latest, working = self.retained_incremental_fixture(directory)
            release = target / "release"
            (release / "deps").mkdir(parents=True)
            (release / ".fingerprint/release-fixture-1111111111111111").mkdir(parents=True)
            (release / "deps/librelease-fixture-1111111111111111.rlib").write_bytes(b"other profile")
            available_files = [*old.iterdir(), working / "dep-graph.part.bin"]
            available = sum(path.stat().st_blocks * 512 for path in available_files)
            current = self.measured_size(target)
            needed = max(512, available // 2)
            baseline = (current - needed) / 2
            state = root / "state.json"
            state.write_text(json.dumps({
                "baseline_bytes": baseline, "factor": 2.0,
                "metric": "unique-inode-allocated-bytes", "metric_version": 1,
                "target": str(target),
            }))
            state_before = state.read_bytes()
            args = argparse.Namespace(
                target=str(target), target_dir=None, state=str(state), factor=2.0,
                dry_run=True, print_json=True, protected_ref=[],
                retain_profile=[str(release)], retain_build_profile="debug",
                coverage_domain=None, expected_state_sha256=None, command=["cargo", "build"],
            )

            def resolve_with_metadata(
                selected_root: Path, value: str | None = None,
                incremental_prefixes: list[str] | None = None,
                inactive_prefixes: list[str] | None = None,
                build_metadata: dict[str, object] | None = None,
            ) -> Path:
                if build_metadata is not None:
                    build_metadata.update({"target_directory": str(target), "build_directory": str(target)})
                if incremental_prefixes is not None:
                    incremental_prefixes.append("fixture-")
                return target

            authorized: list[tuple[Path, ...]] = []
            real_evict = evict_artifacts

            def record_authorization(
                selected_target: Path, threshold: int, current_bytes: int, dry_run: bool,
                protected_refs: Sequence[str | Path] = (), epoch: ProfileLocks | None = None,
                *, incremental_prefixes: Sequence[str] = (),
                inactive_prefixes: Sequence[str] = (),
                retained_profiles: Sequence[str | Path] = (),
                auxiliary_only_profiles: Sequence[str | Path] = (),
                unselected_incremental_prefixes: Sequence[str] = (),
                selected_package_profiles: Sequence[str | Path] = (),
            ) -> tuple[int, EvictionReport]:
                authorized.append(tuple(Path(path) for path in auxiliary_only_profiles))
                return real_evict(
                    selected_target, threshold, current_bytes, dry_run, protected_refs, epoch,
                    incremental_prefixes=incremental_prefixes, inactive_prefixes=inactive_prefixes,
                    retained_profiles=retained_profiles, auxiliary_only_profiles=auxiliary_only_profiles,
                    unselected_incremental_prefixes=unselected_incremental_prefixes,
                    selected_package_profiles=selected_package_profiles,
                )

            output = io.StringIO()
            environment = os.environ.copy()
            environment.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
            environment.pop("CARGO_CAPACITY_FACTOR", None)
            with patch("cargo_capacity_guard._root", return_value=root), \
                    patch("cargo_capacity_guard.resolve_target", side_effect=resolve_with_metadata), \
                    patch("cargo_capacity_guard.busy_processes", return_value=[]), \
                    patch("cargo_capacity_guard.evict_artifacts", side_effect=record_authorization), \
                    patch.dict(os.environ, environment, clear=True), \
                    contextlib.redirect_stdout(output):
                result = run_guard(args)
            report = json.loads(output.getvalue())
            self.assertEqual(result, 0, report)
            self.assertEqual(authorized, [(profile,)])
            self.assertEqual(state.read_bytes(), state_before)
            self.assertEqual(report["metric"], "unique-inode-allocated-bytes")
            self.assertEqual(report["metric_version"], 1)
            self.assertEqual(report["factor"], 2.0)
            self.assertEqual(report["threshold_bytes"], baseline * 2)
            self.assertEqual(report["finite_cleanup"]["invalidated_profiles"], [])
            self.assertTrue(report["finite_cleanup"]["candidate_paths"])
            self.assertTrue(all(path.startswith("debug/incremental/fixture-owner-123456/")
                                for path in report["finite_cleanup"]["candidate_paths"]))
            self.assertTrue((release / "deps/librelease-fixture-1111111111111111.rlib").exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_removes_only_closed_old_and_stale_working_sessions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact, old, latest, working = self.retained_incremental_fixture(directory)
            build = profile / "build/fixture/out/product.o"
            example = profile / "examples/fixture-example-1234567890abcdef"
            old_files = sorted(path for path in old.iterdir())
            working_file = working / "dep-graph.part.bin"
            candidate_files = [*old_files, working_file]
            available = sum(path.stat().st_blocks * 512 for path in candidate_files)
            fingerprint_before = self.file_tree_bytes(profile / ".fingerprint")
            preserved_before = {path: path.read_bytes() for path in (artifact, build, example)}
            current_bytes = available + 1
            code, preview = self.retained_incremental_evict(target, current_bytes, 1, dry_run=True)
            expected = {str(path.relative_to(target)) for path in candidate_files}
            self.assertEqual(code, 0, preview)
            self.assertEqual(set(preview["candidate_paths"]), expected)
            self.assertEqual(preview["invalidated_profiles"], [])
            self.assertTrue(all(path.exists() for path in [*candidate_files, artifact, build]))
            self.assertEqual(self.file_tree_bytes(profile / ".fingerprint"), fingerprint_before)

            code, report = self.retained_incremental_evict(target, current_bytes, 1)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["deleted_files"], len(candidate_files))
            self.assertEqual(report.get("removed_incremental_sessions"), 2)
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertFalse(old.exists())
            self.assertFalse(working.exists())
            self.assertTrue(latest.exists())
            self.assertTrue((latest.parent / f"s-{latest.name.split('-')[1]}-{latest.name.split('-')[2]}.lock").exists())
            self.assertTrue(all(path.read_bytes() == value for path, value in preserved_before.items()))
            self.assertEqual(self.file_tree_bytes(profile / ".fingerprint"), fingerprint_before)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_thresholds_and_unknown_members_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, working = self.retained_incremental_fixture(directory)
            eligible = [*old.iterdir(), working / "dep-graph.part.bin"]
            capacity = sum(path.stat().st_blocks * 512 for path in eligible)
            code, report = self.retained_incremental_evict(target, capacity + 1, capacity + 1, dry_run=True)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["candidate_paths"], [])
            code, report = self.retained_incremental_evict(target, capacity + 2, 1, dry_run=True)
            self.assertEqual(code, 1, report)
            self.assertIn("insufficient", report["error"])
            self.assertTrue(old.exists())
            self.assertTrue(working.exists())

        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, working = self.retained_incremental_fixture(directory)
            unknown = old / "unknown.bin"
            unknown.write_bytes(b"unknown")
            available = sum(path.stat().st_blocks * 512 for path in [*old.iterdir(), working / "dep-graph.part.bin"])
            code, report = self.retained_incremental_evict(target, available + 1, 1, dry_run=True)
            self.assertEqual(code, 1, report)
            self.assertTrue(old.exists())
            self.assertTrue(unknown.exists())
            self.assertTrue(working.exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_protected_alias_blocks_whole_session(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _fp, _artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            index = old / "dep-graph.bin"
            alias = profile / "deps/libfixture-alias-4444444444444444.rlib"
            os.link(index, alias)
            available = sum(path.stat().st_blocks * 512 for path in [*old.iterdir(), alias])
            code, report = self.retained_incremental_evict(target, available + 1, 1, dry_run=True)
            self.assertEqual(code, 1, report)
            self.assertFalse(any(str(old.relative_to(target)) in path for path in report["candidate_paths"]))
            self.assertTrue(index.exists())
            self.assertTrue(alias.exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_protected_session_lock_retains_whole_session(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            lock_path = old.parent / ("-".join(old.name.split("-")[:3]) + ".lock")
            old_files = sorted(old.iterdir())
            old_bytes = sum(path.stat().st_blocks * 512 for path in old_files)
            code, report = self.retained_incremental_evict(
                target, old_bytes + 1, 1, protected_refs=[artifact, lock_path],
            )
            self.assertEqual(code, 1, report)
            self.assertIn("insufficient", report["error"])
            self.assertFalse(any(str(old.relative_to(target)) in path for path in report["candidate_paths"]))
            self.assertTrue(all(path.exists() for path in old_files))
            self.assertTrue(lock_path.exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_ambiguous_lock_prefix_waits_for_real_shared_reader(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            duplicate = old.with_name(old.name[:-25] + "b" * 25)
            duplicate.mkdir()
            (duplicate / "unknown.bin").write_bytes(b"preserve unknown session")
            lock_path = old.parent / ("-".join(old.name.split("-")[:3]) + ".lock")
            old_files = sorted(old.iterdir())
            old_bytes = sum(path.stat().st_blocks * 512 for path in old_files)
            real_acquire = _acquire_incremental_session_lock
            real_rmdir = os.rmdir
            attempted_sessions: list[str] = []
            children: list[subprocess.Popen[str]] = []
            reader_state: list[str] = []

            def acquire(
                owner: Path, owner_fd: int, session_name: str, device: int,
                protected: set[tuple[int, int]],
            ) -> tuple[int, Path, tuple[int, int, int]] | None:
                attempted_sessions.append(session_name)
                result = real_acquire(owner, owner_fd, session_name, device, protected)
                if result is not None and not children:
                    code = (
                        "import fcntl,sys; h=open(sys.argv[1],'r+'); print('waiting',flush=True); "
                        "fcntl.lockf(h.fileno(),fcntl.LOCK_SH); print('acquired',flush=True); sys.stdin.read()"
                    )
                    child = subprocess.Popen(
                        [sys.executable, "-c", code, str(lock_path)], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                    )
                    children.append(child)
                    if child.stdout is None or child.stdout.readline().strip() != "waiting":
                        raise AssertionError("shared reader did not start")
                return result

            def rmdir(name: str, *, dir_fd: int | None = None) -> None:
                if name == old.name and children:
                    child_stdout = children[0].stdout
                    if child_stdout is None:
                        raise AssertionError("shared reader stdout missing")
                    reader_state.append(child_stdout.readline().strip())
                    if reader_state[-1] != "acquired":
                        raise AssertionError("shared reader did not acquire before old session removal")
                real_rmdir(name, dir_fd=dir_fd)

            try:
                with patch("cargo_capacity_guard_lock._acquire_incremental_session_lock", side_effect=acquire), \
                        patch("cargo_capacity_guard_lock.os.rmdir", side_effect=rmdir):
                    code, report = self.retained_incremental_evict(target, old_bytes + 1, 1)
                self.assertEqual(code, 1, report)
                self.assertIn("insufficient", report["error"])
                self.assertNotIn(old.name, attempted_sessions)
                self.assertFalse(any(str(old.relative_to(target)) in path for path in report["candidate_paths"]))
                self.assertTrue(all(path.exists() for path in old_files))
                self.assertTrue((duplicate / "unknown.bin").exists())
                if children and children[0].stdout is not None:
                    reader_state.append(children[0].stdout.readline().strip())
                self.assertIn("acquired", reader_state)
            finally:
                for child in children:
                    if child.stdin is not None:
                        child.stdin.close()
                    child.communicate(timeout=5)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_malformed_sibling_entry_blocks_shared_lock_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            lock_prefix = "-".join(old.name.split("-")[:3])
            malformed = old.parent / f"{lock_prefix}-unknown"
            malformed.write_bytes(b"unknown owner entry sharing lock prefix")
            lock_attempts: list[str] = []
            real_acquire = _acquire_incremental_session_lock

            def acquire(
                owner: Path, owner_fd: int, session_name: str, device: int,
                protected: set[tuple[int, int]],
            ) -> tuple[int, Path, tuple[int, int, int]] | None:
                lock_attempts.append(session_name)
                return real_acquire(owner, owner_fd, session_name, device, protected)

            old_bytes = sum(path.stat().st_blocks * 512 for path in old.iterdir())
            with patch("cargo_capacity_guard_lock._acquire_incremental_session_lock", side_effect=acquire):
                code, report = self.retained_incremental_evict(target, old_bytes + 1, 1, dry_run=True)
            self.assertEqual(code, 1, report)
            self.assertIn("insufficient", report["error"])
            self.assertNotIn(old.name, lock_attempts)
            self.assertFalse(any(str(old.relative_to(target)) in path for path in report["candidate_paths"]))
            self.assertTrue(old.exists())
            self.assertTrue(malformed.exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_lock_replacement_before_purge_fails_without_deletion(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            lock_path = old.parent / ("-".join(old.name.split("-")[:3]) + ".lock")
            detached_lock = lock_path.with_suffix(".detached")
            real_metadata = _metadata_at
            product_calls = 0
            children: list[subprocess.Popen[str]] = []
            reader_state: list[str] = []

            def metadata(descriptor: int, name: str, device: int) -> os.stat_result:
                nonlocal product_calls
                if name == "product.o":
                    product_calls += 1
                if not children and name == "product.o" and product_calls == 2:
                    lock_path.rename(detached_lock)
                    lock_path.write_bytes(b"replacement lock inode")
                    code = (
                        "import fcntl,sys; h=open(sys.argv[1],'r+'); "
                        "fcntl.lockf(h.fileno(),fcntl.LOCK_SH|fcntl.LOCK_NB); "
                        "print('acquired',flush=True); sys.stdin.read()"
                    )
                    child = subprocess.Popen(
                        [sys.executable, "-c", code, str(lock_path)], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                    )
                    children.append(child)
                    if child.stdout is None:
                        raise AssertionError("shared reader stdout missing")
                    reader_state.append(child.stdout.readline().strip())
                return real_metadata(descriptor, name, device)

            try:
                with patch("cargo_capacity_guard_lock._metadata_at", side_effect=metadata):
                    old_bytes = sum(path.stat().st_blocks * 512 for path in old.iterdir())
                    code, report = self.retained_incremental_evict(target, old_bytes + 1, 1)
                self.assertEqual(code, 1, report)
                self.assertIn("incremental session lock changed", report["error"])
                self.assertTrue(old.exists())
                self.assertTrue(all(path.is_file() for path in old.iterdir()))
                self.assertTrue(lock_path.exists())
                self.assertTrue(detached_lock.exists())
                self.assertEqual(reader_state, ["acquired"])
            finally:
                for child in children:
                    if child.stdin is not None:
                        child.stdin.close()
                    child.communicate(timeout=5)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_shared_reader_lock_blocks_finalized_session(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, old, _latest, _working = self.retained_incremental_fixture(directory)
            match = old.name.split("-")
            lock_path = old.parent / f"s-{match[1]}-{match[2]}.lock"
            with incremental_session_lock_holder(lock_path, "shared") as child_stdout:
                self.assertEqual(child_stdout.readline().strip(), "ready")
                old_bytes = sum(path.stat().st_blocks * 512 for path in old.iterdir())
                code, report = self.retained_incremental_evict(target, old_bytes + 1, 1, dry_run=True)
                self.assertEqual(code, 1, report)
                self.assertFalse(any(str(old.relative_to(target)) in path for path in report["candidate_paths"]))
                self.assertTrue(old.exists())

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_retained_profile_active_working_lock_blocks_removal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _profile, _fp, _artifact, _old, _latest, working = self.retained_incremental_fixture(directory)
            match = working.name.split("-")
            lock_path = working.parent / f"s-{match[1]}-{match[2]}.lock"
            with incremental_session_lock_holder(lock_path, "exclusive") as child_stdout:
                self.assertEqual(child_stdout.readline().strip(), "ready")
                working_file = working / "dep-graph.part.bin"
                old_capacity = sum(path.stat().st_blocks * 512 for path in _old.iterdir())
                working_capacity = working_file.stat().st_blocks * 512
                code, report = self.retained_incremental_evict(
                    target, old_capacity + working_capacity + 1, 1, dry_run=True,
                )
                self.assertEqual(code, 1, report)
                self.assertNotIn(str(working_file.relative_to(target)), report["candidate_paths"])
                self.assertTrue(working.exists())

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_finite_suffix_scope_and_profile_invalidation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, profile, fp, artifact, cache, obj = self.incremental_fixture(directory)
            retained = [cache.with_suffix('.txt'), cache.parent / 'session.lock',
                        profile / 'build/owned/out/product.o',
                        profile / 'gn_out/product.o',
                        profile / 'incremental/other-abc/session/product.o']
            for path in retained:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'retain')
            code, preview = self.incremental_evict(target, dry_run=True)
            self.assertEqual(code, 0, preview)
            self.assertEqual(set(preview['candidate_paths']),
                             {str(p.relative_to(target)) for p in [artifact, obj, *cache.parent.glob('*.bin')]})
            self.assertTrue(fp.exists())
            code, report = self.incremental_evict(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(report['deleted_files'], 5)
            self.assertFalse(fp.exists())
            self.assertTrue(all(not p.exists() for p in [artifact, cache, obj]))
            self.assertTrue(all(p.read_bytes() == b'retain' for p in retained))

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_protected_and_outside_alias_cache_are_retained(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, _, _, cache, obj = self.incremental_fixture(directory)
            outside = Path(directory).resolve() / 'outside-cache-alias'
            os.link(obj, outside)
            code, report = self.incremental_evict(target, [cache])
            self.assertEqual(code, 0, report)
            self.assertTrue(cache.exists())
            self.assertTrue(obj.exists())
            self.assertEqual(obj.read_bytes(), outside.read_bytes())
            self.assertEqual(report['retained_protected_inodes'], 1)
            self.assertEqual(report['retained_unknown_alias_inodes'], 1)

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_and_deps_aliases_are_counted_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, _, artifact, cache, obj = self.incremental_fixture(directory)
            obj.unlink()
            os.link(artifact, obj)
            expected = artifact.stat().st_blocks * 512 + sum(p.stat().st_blocks * 512 for p in cache.parent.glob('*.bin'))
            code, preview = self.incremental_evict(target, dry_run=True)
            self.assertEqual(code, 0, preview)
            self.assertEqual(preview['estimated_candidate_bytes'], expected)
            code, report = self.incremental_evict(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(report['deleted_files'], 5)
            self.assertFalse(artifact.exists())
            self.assertFalse(obj.exists())

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_symlink_mutation_refuses_operations(self) -> None:
        from cargo_capacity_guard_lock import _metadata_at
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, _, cache, _ = self.incremental_fixture(directory)
            referent = Path(directory).resolve() / 'referent'
            referent.write_bytes(b'retain-referent')
            backup = Path(directory).resolve() / 'saved-cache'
            calls = 0
            def mutate(descriptor: int, name: str, device: int) -> os.stat_result:
                nonlocal calls
                if name == cache.name:
                    calls += 1
                    if calls == 2:
                        cache.rename(backup)
                        cache.symlink_to(referent)
                return _metadata_at(descriptor, name, device)
            with patch('cargo_capacity_guard_lock._metadata_at', side_effect=mutate):
                code, report = self.incremental_evict(target)
            self.assertEqual(code, 1, report)
            self.assertTrue(fp.exists())
            self.assertEqual(report['staged_paths'], [])
            self.assertEqual(report['deleted_files'], 0)
            self.assertEqual(referent.read_bytes(), b'retain-referent')

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_post_threshold_shortfall_remains_incomplete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, _, _, cache, obj = self.incremental_fixture(directory)
            before = self.measured_size(target)
            code, report = self.incremental_evict(target, [cache, obj])
            status = {'current_bytes': before, 'threshold_bytes': 1}
            record_cleanup_result(status, target, code, 'finite incremental eviction', False)
            self.assertTrue(status['remaining_over_capacity'])
            self.assertEqual(status['cleanup_status'], 'incomplete')


    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_object_low_budget_includes_three_indexes_and_stages_them_first(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact, cache, obj = self.incremental_fixture(directory)
            os.utime(obj, ns=(1, 1))
            indexes = sorted(cache.parent.glob('*.bin'))
            expected = sum(p.stat().st_blocks * 512 for p in [obj, *indexes])
            with ProfileLocks(target) as epoch:
                current = self.measured_size(target)
                code, preview = evict_artifacts(target, current - 1,
                    current, True, [target / 'debug/app'], epoch,
                    incremental_prefixes=['fixture-'])
            self.assertEqual(code, 0, preview)
            self.assertEqual(set(preview['candidate_paths']),
                             {str(p.relative_to(target)) for p in [obj, *indexes]})
            self.assertEqual(preview['estimated_candidate_bytes'], expected)
            with ProfileLocks(target) as epoch:
                current = self.measured_size(target)
                code, actual = evict_artifacts(target, current - 1,
                    current, False, [target / 'debug/app'], epoch,
                    incremental_prefixes=['fixture-'])
            self.assertEqual(code, 0, actual)
            self.assertEqual(set(actual['staged_paths'][:3]),
                             {str(p.relative_to(target)) for p in indexes})
            self.assertEqual(actual['staged_paths'][3], str(obj.relative_to(target)))
            self.assertTrue(artifact.exists())
            self.assertFalse(fp.exists())

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_protected_or_outside_alias_index_keeps_session_objects(self) -> None:
        for protected_index in [True, False]:
            with self.subTest(protected_index=protected_index), tempfile.TemporaryDirectory() as directory:
                target, _, _, _, cache, obj = self.incremental_fixture(directory)
                index = cache.parent / 'work-products.bin'
                protected = [index] if protected_index else []
                if not protected_index:
                    os.link(index, Path(directory).resolve() / 'outside-index')
                code, report = self.incremental_evict(target, protected)
                self.assertEqual(code, 0, report)
                self.assertTrue(obj.exists())
                self.assertTrue(all(p.exists() for p in cache.parent.glob('*.bin')))
                self.assertFalse(any('/incremental/' in p for p in report['candidate_paths']))

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_missing_index_or_unknown_session_is_retained(self) -> None:
        for unknown in [False, True]:
            with self.subTest(unknown=unknown), tempfile.TemporaryDirectory() as directory:
                target, _, _, _, cache, obj = self.incremental_fixture(directory)
                if unknown:
                    renamed = cache.parent.with_name('unknown-session')
                    cache.parent.rename(renamed)
                    obj = renamed / obj.name
                else:
                    (cache.parent / 'work-products.bin').unlink()
                code, report = self.incremental_evict(target)
                self.assertEqual(code, 0, report)
                self.assertTrue(obj.exists())
                self.assertFalse(any('/incremental/' in p for p in report['candidate_paths']))

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_index_stage_interruption_moves_no_objects_and_keeps_recovery(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, fp, artifact, cache, obj = self.incremental_fixture(directory)
            rename = os.rename
            def interrupt(
                source: str,
                destination: str,
                *args: str,
                rename_operation: Callable[..., None] = rename,
                **kwargs: int | None,
            ) -> None:
                if source == 'work-products.bin':
                    raise OSError('owned index stage interruption')
                rename_operation(source, destination, *args, **kwargs)
            with patch('cargo_capacity_guard_lock.os.rename', side_effect=interrupt):
                code, report = self.incremental_evict(target)
            self.assertEqual(code, 1, report)
            self.assertFalse(fp.exists())
            self.assertTrue(obj.exists())
            self.assertTrue(artifact.exists())
            self.assertEqual(report['deleted_files'], 0)
            self.assertTrue(report['recovery_required_paths'])
            self.assertTrue(all(p.endswith('.bin') for p in report['staged_paths']))

    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_incremental_three_indexes_closed_hardlinks_count_unique_allocation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, _, _, artifact, cache, obj = self.incremental_fixture(directory)
            other = cache.parent.parent / 's-owned-1-cache'
            other.mkdir()
            for index in cache.parent.glob('*.bin'):
                os.link(index, other / index.name)
            expected = sum(p.stat().st_blocks * 512 for p in [artifact, obj, *cache.parent.glob('*.bin')])
            code, report = self.incremental_evict(target, dry_run=True)
            self.assertEqual(code, 0, report)
            self.assertEqual(report['estimated_candidate_bytes'], expected)
            self.assertEqual(len(report['candidate_paths']), 8)
            code, report = self.incremental_evict(target)
            self.assertEqual(code, 0, report)
            self.assertEqual(report['deleted_files'], 8)
            self.assertEqual(list(other.iterdir()), [])


    @unittest.skipIf(os.name == 'nt', 'POSIX native no-follow eviction fixture')
    def test_cli_and_environment_protected_refs_form_union(self) -> None:
        script = str(Path(__file__).resolve().parent / "cargo_capacity_guard.py")
        cases: tuple[tuple[str, list[int], list[int] | None, list[int]], ...] = (
            ("cli-only", [0], None, [0]),
            ("environment-only", [], [0], [0]),
            ("both-equal", [0], [0], [0]),
            ("both-different", [1], [0], [0, 1]),
        )
        for label, cli_indexes, env_indexes, expected_indexes in cases:
            with self.subTest(case=label), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target, profile, _, first = self.artifact_fixture(directory)
                second = profile / "deps/libsecond-3333333333333333.rlib"
                bulk = profile / "deps/libbulk-4444444444444444.rlib"
                second.write_bytes(b"y" * 8192)
                bulk.write_bytes(b"z" * 262144)
                references = [first, second]
                for index, reference in enumerate((first, second, bulk)):
                    timestamp = 1_700_000_000_000_000_000 + index * 1_000_000_000
                    os.utime(reference, ns=(timestamp, timestamp))
                state = root / "state.json"
                state.write_text(json.dumps({
                    "baseline_bytes": 32768,
                    "metric": "unique-inode-allocated-bytes",
                    "metric_version": 1,
                    "target": str(target),
                }))
                marker = root / "command-ran"
                command = [sys.executable, script, "--root", str(root),
                           "--target", str(target), "--state", str(state), "--print-json"]
                for index in cli_indexes:
                    command.extend(["--protected-ref", str(references[index])])
                command.extend(["--", sys.executable, "-c",
                                f"open({str(marker)!r}, 'w').close()"])
                environment = os.environ.copy()
                environment["CARGO_CAPACITY_FACTOR"] = "2"
                if env_indexes is None:
                    environment.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
                else:
                    environment["CARGO_CAPACITY_PROTECTED_REFS"] = json.dumps(
                        [str(references[index]) for index in env_indexes]
                    )
                result = subprocess.run(
                    command, cwd=root, env=environment, capture_output=True, text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                report = json.loads(result.stdout)
                self.assertEqual(report["cleanup_status"], "completed", report)
                self.assertTrue(marker.exists())
                for index in expected_indexes:
                    self.assertTrue(references[index].exists(), label)

    def test_invalid_environment_protected_refs_with_cli_fail_before_cleanup_or_child(self) -> None:
        script = str(Path(__file__).resolve().parent / "cargo_capacity_guard.py")
        invalid_values = ("{", "[]", json.dumps(["relative/path"]), json.dumps([17]))
        for invalid_value in invalid_values:
            with self.subTest(environment_value=invalid_value), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target, profile, fingerprint, artifact = self.artifact_fixture(directory)
                bulk = profile / "deps/libbulk-5555555555555555.rlib"
                bulk.write_bytes(b"z" * 262144)
                state = root / "state.json"
                state_contents = json.dumps({
                    "baseline_bytes": 32768,
                    "metric": "unique-inode-allocated-bytes",
                    "metric_version": 1,
                    "target": str(target),
                })
                state.write_text(state_contents)
                before_bytes = self.measured_size(target)
                marker = root / "command-ran"
                command = [sys.executable, script, "--root", str(root),
                           "--target", str(target), "--state", str(state),
                           "--protected-ref", str(artifact), "--", sys.executable,
                           "-c", f"open({str(marker)!r}, 'w').close()"]
                environment = os.environ.copy()
                environment["CARGO_CAPACITY_FACTOR"] = "2"
                environment["CARGO_CAPACITY_PROTECTED_REFS"] = invalid_value
                result = subprocess.run(
                    command, cwd=root, env=environment, capture_output=True, text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("CARGO_CAPACITY_PROTECTED_REFS", result.stderr)
                self.assertFalse(marker.exists())
                self.assertEqual(self.measured_size(target), before_bytes)
                self.assertTrue(fingerprint.exists())
                self.assertTrue(artifact.exists())
                self.assertTrue(bulk.exists())
                self.assertEqual(state.read_text(), state_contents)



    @staticmethod
    def selected_metadata(root: Path) -> dict[str, object]:
        packages = []
        for identity, name, target in (("selected", "kdr-linter", "mathjax_consumer_facts"),
                                       ("unselected", "runtime", "unselected"),
                                       ("external", "external", "external")):
            manifest = root / ("crates/kdr-linter/Cargo.toml" if identity == "selected"
                               else f"crates/{name}/Cargo.toml")
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text('[package]\nname = "' + name + '"\n')
            packages.append({"id": identity, "name": name, "manifest_path": str(manifest),
                             "targets": [{"name": target, "kind": ["bin"]}]})
        (root / "Cargo.toml").write_text('[workspace]\n')
        (root / "Cargo.lock").write_text('# locked\n')
        return {"packages": packages, "workspace_members": ["selected", "unselected"],
                "target_directory": str(root / "target"), "build_directory": str(root / "target"),
                "resolve": {"nodes": [{"id": "selected", "deps": [{"pkg": "external"}]},
                                      {"id": "unselected", "deps": []},
                                      {"id": "external", "deps": []}]}}

    def test_selected_graph_baseline_and_transitive_collision_invalid_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            baseline = self.selected_metadata(root)
            self.assertEqual(selected_package_incremental_prefixes(root, baseline), ["unselected-"])
            for mutation in ("transitive", "collision", "numeric-external", "custom-build", "missing",
                             "duplicate", "dangling", "foreign", "missing-bin", "duplicate-bin"):
                with self.subTest(mutation=mutation):
                    value = json.loads(json.dumps(baseline))
                    self.assertEqual(selected_package_incremental_prefixes(root, value), ["unselected-"])
                    if mutation == "transitive":
                        value["resolve"]["nodes"][2]["deps"] = [{"pkg": "unselected"}]
                        self.assertEqual(selected_package_incremental_prefixes(root, value), [])
                    elif mutation == "collision":
                        value["packages"][2]["targets"][0]["name"] = "unselected"
                        self.assertEqual(selected_package_incremental_prefixes(root, value), [])
                    elif mutation == "numeric-external":
                        value["packages"][2]["targets"][0]["name"] = "01_quick"
                        self.assertEqual(selected_package_incremental_prefixes(root, value), ["unselected-"])
                    elif mutation == "custom-build":
                        value["packages"][1]["targets"][0]["kind"] = ["custom-build"]
                        self.assertEqual(selected_package_incremental_prefixes(root, value), [])
                    else:
                        if mutation == "missing":
                            value["resolve"]["nodes"].pop()
                        elif mutation == "duplicate":
                            value["resolve"]["nodes"].append(value["resolve"]["nodes"][0])
                        elif mutation == "dangling":
                            value["resolve"]["nodes"][0]["deps"] = [{"pkg": "unknown"}]
                        elif mutation == "missing-bin":
                            value["packages"][0]["targets"][0]["kind"] = ["lib"]
                        elif mutation == "duplicate-bin":
                            value["packages"][0]["targets"].append(value["packages"][0]["targets"][0])
                        else:
                            value["packages"][1]["manifest_path"] = str(root.parent / "Cargo.toml")
                        with self.assertRaises(RuntimeError):
                            selected_package_incremental_prefixes(root, value)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_custom_build_namespace_collision_retains_whole_selected_owner(self) -> None:
        for custom_name, ordinary_name in (("build-script-build", "unselected"),
                                           ("build-script-build", "build-script-build"),
                                           ("build_script_build", "build_script_build"),
                                           ("build-script-build", "build-script_build"),
                                           ("host-custom", "host-custom"),
                                           (None, "build-script-build")):
            with self.subTest(custom_name=custom_name, ordinary_name=ordinary_name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target, profile, _fp, artifact, old, latest, working = self.retained_incremental_fixture(directory)
                unselected = old.parent.with_name("unselected-0000000000001")
                old.parent.rename(unselected)
                old, latest, working = (unselected / path.name for path in (old, latest, working))
                custom = unselected.with_name("build_script_build-0000000000002")
                custom.mkdir()
                for path in unselected.iterdir():
                    destination = custom / path.name
                    if path.is_dir():
                        destination.mkdir()
                        for member in path.iterdir():
                            (destination / member.name).write_bytes(member.read_bytes())
                    else:
                        destination.write_bytes(path.read_bytes())
                metadata = self.selected_metadata(root)
                packages = _array(metadata["packages"], "fixture packages")
                if custom_name is not None:
                    selected = _object(packages[0], "fixture selected package")
                    _array(selected["targets"], "fixture selected targets").append({"name": custom_name, "kind": ["custom-build"]})
                protected = [target / "debug/app", artifact, profile / "examples/fixture-example-1234567890abcdef"]
                for index in range(34):
                    reference = root / "frozen" / str(index)
                    reference.parent.mkdir(exist_ok=True)
                    reference.write_bytes(f"custom-build protected {index}".encode())
                    protected.append(reference)
                identities = [(path, path.stat().st_ino, path.read_bytes()) for path in protected]
                custom_before = self.file_tree_bytes(custom)
                verify = _selected_input_check(root, metadata)
                verify()
                baseline = selected_package_incremental_prefixes(root, metadata)
                self.assertEqual(baseline, ["unselected-"])
                needed = sum(path.stat().st_blocks * 512 for session in (old, latest) for path in session.iterdir())
                current = self.measured_size(target)
                with ProfileLocks(target, verify) as epoch:
                    baseline_code, preview = evict_artifacts(
                        target, current - needed, current, True, protected, epoch,
                        retained_profiles=[profile], unselected_incremental_prefixes=baseline,
                        selected_package_profiles=[profile],
                    )
                self.assertEqual(baseline_code, 0, preview)
                self.assertEqual(len(preview["candidate_paths"]), 7)
                self.assertTrue(all(path.startswith("debug/incremental/unselected-") for path in preview["candidate_paths"]))
                unselected_package = _object(packages[1], "fixture unselected package")
                ordinary = _object(_array(unselected_package["targets"], "fixture unselected targets")[0], "fixture ordinary target")
                ordinary["name"] = ordinary_name
                prefixes = selected_package_incremental_prefixes(root, metadata)
                self.assertEqual(prefixes, baseline if ordinary_name == "unselected" else [])
                with ProfileLocks(target, verify) as epoch:
                    code, report = evict_artifacts(
                        target, current - needed, current, False, protected, epoch,
                        retained_profiles=[profile], unselected_incremental_prefixes=prefixes,
                        selected_package_profiles=[profile],
                    )
                self.assertEqual(self.file_tree_bytes(custom), custom_before)
                self.assertEqual(report["invalidated_profiles"], [])
                self.assertTrue(working.exists())
                for path, inode, data in identities:
                    self.assertEqual(path.stat().st_ino, inode)
                    self.assertEqual(path.read_bytes(), data)
                if ordinary_name == "unselected":
                    self.assertEqual(code, 0, report)
                    self.assertEqual(report["deleted_files"], 7)
                    self.assertFalse(latest.exists())
                else:
                    self.assertEqual(code, 1, report)
                    self.assertEqual(report["deleted_files"], 0)
                    self.assertTrue(latest.exists())
                with ProfileLocks(target, verify) as epoch:
                    code, report = evict_artifacts(
                        target, current - needed, current, False, protected, epoch,
                        retained_profiles=[profile], unselected_incremental_prefixes=["build_script_build-"],
                        selected_package_profiles=[profile],
                    )
                self.assertEqual(code, 1, report)
                self.assertIn("custom-build incremental namespace", report["error"])
                self.assertEqual(report["deleted_files"], 0)
                self.assertEqual(self.file_tree_bytes(custom), custom_before)

    def test_custom_build_incomplete_metadata_fails_closed_from_valid_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            baseline = self.selected_metadata(root)
            selected = _object(_array(baseline["packages"], "fixture packages")[0], "fixture selected package")
            _array(selected["targets"], "fixture selected targets").append({"name": "build-script-build", "kind": ["custom-build"]})
            same_package = _object(json.loads(json.dumps(baseline)), "fixture metadata")
            self.assertEqual(selected_package_incremental_prefixes(root, same_package), ["unselected-"])
            unselected = _object(_array(same_package["packages"], "fixture packages")[1], "fixture unselected package")
            _array(unselected["targets"], "fixture unselected targets").append({"name": "unselected", "kind": ["custom-build"]})
            self.assertEqual(selected_package_incremental_prefixes(root, same_package), [])
            for mutation in ("missing-targets", "empty-targets", "missing-kind", "empty-kind", "unknown-kind", "missing-name"):
                with self.subTest(mutation=mutation):
                    value = _object(json.loads(json.dumps(baseline)), "fixture metadata")
                    self.assertEqual(selected_package_incremental_prefixes(root, value), ["unselected-"])
                    package = _object(_array(value["packages"], "fixture packages")[0], "fixture selected package")
                    custom_target = _object(_array(package["targets"], "fixture selected targets")[1], "fixture custom target")
                    if mutation == "missing-targets":
                        del package["targets"]
                    elif mutation == "empty-targets":
                        package["targets"] = []
                    elif mutation == "missing-kind":
                        del custom_target["kind"]
                    elif mutation == "empty-kind":
                        custom_target["kind"] = []
                    elif mutation == "unknown-kind":
                        custom_target["kind"] = ["unknown-builder"]
                    else:
                        del custom_target["name"]
                    with self.assertRaises(RuntimeError):
                        selected_package_incremental_prefixes(root, value)

    def test_selected_input_baseline_then_manifest_lock_config_environment_drift(self) -> None:
        for mutation in ("manifest", "lock", "config", "environment", "workspace-add"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                value = self.selected_metadata(root)
                verify = _selected_input_check(root, value)
                verify()
                with patch.dict(os.environ, os.environ.copy(), clear=True):
                    if mutation == "manifest":
                        (root / "crates/runtime/Cargo.toml").write_text('[dependencies]\nkdr = "1"\n')
                    elif mutation == "lock":
                        (root / "Cargo.lock").write_text('# changed\n')
                    elif mutation == "config":
                        (root / ".cargo").mkdir()
                        (root / ".cargo/config.toml").write_text('[build]\n')
                    elif mutation == "workspace-add":
                        (root / "crates/new-package").mkdir()
                    else:
                        os.environ["RUSTFLAGS"] = os.environ.get("RUSTFLAGS", "") + " --cfg input_mutation"
                    with self.assertRaisesRegex(RuntimeError, "selected metadata inputs changed"):
                        verify()

    def test_canonical_command_only_and_same_input_metadata_epoch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            full = self.selected_metadata(root)
            initial = json.loads(json.dumps(full))
            initial["packages"] = initial["packages"][:2]
            command = ["cargo", "build", "--offline", "--locked", "-p", "kdr-linter", "--bin", "mathjax_consumer_facts"]
            result = subprocess.CompletedProcess(command, 0, json.dumps(full), "")
            with patch("cargo_capacity_guard.subprocess.run", return_value=result) as query:
                prefixes, verify = selected_package_cleanup(root, root / "target", command, initial)
                self.assertEqual(prefixes, ["unselected-"])
                self.assertIsNotNone(verify)
                if verify is not None:
                    verify()
                for tail in (["--workspace"], ["-p", "runtime"], ["--features", "anything"], ["--release"]):
                    self.assertEqual(selected_package_cleanup(root, root / "target", command + tail, initial), ([], None))
                for key, value in (("RUSTC_WRAPPER", "/unknown/wrapper"), ("RUSTFLAGS", "--crate-name unselected"),
                                   ("CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS", "--crate-name unselected")):
                    with patch.dict(os.environ, {key: value}):
                        self.assertEqual(selected_package_cleanup(root, root / "target", command, initial), ([], None))
                self.assertEqual(query.call_count, 1)
            def drift(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
                (root / "Cargo.lock").write_text('# changed during metadata\n')
                return result
            with patch("cargo_capacity_guard.subprocess.run", side_effect=drift):
                with self.assertRaisesRegex(RuntimeError, "selected metadata inputs changed"):
                    selected_package_cleanup(root, root / "target", command, initial)

    @unittest.skipIf(os.name == "nt", "POSIX rustc session lock fixture")
    def test_selected_package_latest_whole_session_and_retained_outputs(self) -> None:
        for mutation in ("positive", "dry-run", "metadata", "outside-alias", "protected", "protected-lock",
                         "shared-lock", "duplicate-prefix", "unknown-owner", "input-drift", "input-drift-late"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                target, profile, _fp, _artifact, old, latest, working = self.retained_incremental_fixture(directory)
                owner = old.parent
                renamed = owner.with_name("unselected-0000000000001")
                owner.rename(renamed)
                old, latest, working = (renamed / path.name for path in (old, latest, working))
                root = Path(directory).resolve()
                metadata = self.selected_metadata(root)
                selected_owner = renamed.with_name("mathjax_consumer_facts-0000000000002")
                selected_owner.mkdir()
                for session in (old, latest, working):
                    selected_session = selected_owner / session.name
                    selected_session.mkdir()
                    for path in session.iterdir():
                        (selected_session / path.name).write_bytes(path.read_bytes())
                protected = [target / "debug/app", _artifact, profile / "examples/fixture-example-1234567890abcdef"]
                for index in range(34):
                    reference = root / "frozen" / str(index)
                    reference.parent.mkdir(exist_ok=True)
                    reference.write_bytes(f"immutable consumer {index}".encode())
                    protected.append(reference)
                self.assertEqual(len(protected), 37)
                protected_before = [(path, path.stat().st_ino, path.read_bytes()) for path in protected]
                before = self.file_tree_bytes(target)
                prefixes = selected_package_incremental_prefixes(root, metadata)
                self.assertEqual(prefixes, ["unselected-"])
                verify = _selected_input_check(root, metadata)
                verify()
                needed = sum(path.stat().st_blocks * 512 for path in latest.iterdir())
                lock_path = latest.parent / ("-".join(latest.name.split("-")[:3]) + ".lock")
                if mutation == "metadata":
                    (latest / "metadata.rmeta").write_bytes(b"retain whole session")
                elif mutation == "outside-alias":
                    os.link(latest / "dep-graph.bin", profile / "deps/alias-1111111111111111.rmeta")
                elif mutation == "protected":
                    protected.append(latest / "query-cache.bin")
                elif mutation == "protected-lock":
                    protected.append(lock_path)
                elif mutation == "duplicate-prefix":
                    (latest.parent / ("-".join(latest.name.split("-")[:3]) + "-unknown")).write_bytes(b"ambiguous")
                elif mutation == "unknown-owner":
                    unknown = renamed.with_name("unselected-malformed")
                    renamed.rename(unknown)
                    old, latest, working = (unknown / path.name for path in (old, latest, working))
                elif mutation == "input-drift":
                    (root / "crates/runtime/Cargo.toml").write_text('[dependencies]\nnew = "1"\n')
                # 最新 session だけで到達できる境界を使い、部分削除を成功扱いしない。
                old_bytes = sum(path.stat().st_blocks * 512 for path in old.iterdir())
                current = self.measured_size(target)
                verification_count = 0
                def verify_during_cleanup(
                    selected_mutation: str = mutation, selected_root: Path = root,
                    selected_verify: Callable[[], None] = verify,
                ) -> None:
                    nonlocal verification_count
                    verification_count += 1
                    if selected_mutation == "input-drift-late" and verification_count == 2:
                        (selected_root / "Cargo.lock").write_text('# changed after selection\n')
                    selected_verify()
                with contextlib.ExitStack() as stack:
                    if mutation == "shared-lock":
                        child_stdout = stack.enter_context(incremental_session_lock_holder(lock_path, "shared"))
                        self.assertEqual(child_stdout.readline().strip(), "ready")
                    epoch = stack.enter_context(ProfileLocks(target, verify_during_cleanup))
                    code, report = evict_artifacts(
                        target, current - old_bytes - needed, current, mutation == "dry-run", protected, epoch,
                        retained_profiles=[profile], unselected_incremental_prefixes=prefixes,
                        selected_package_profiles=[profile],
                    )
                self.assertEqual(report["invalidated_profiles"], [])
                for path, inode, data in protected_before:
                    self.assertEqual(path.stat().st_ino, inode)
                    self.assertEqual(path.read_bytes(), data)
                if mutation == "dry-run":
                    self.assertEqual(code, 0, report)
                    self.assertEqual(report["deleted_files"], 0)
                    self.assertEqual({name: data for name, data in self.file_tree_bytes(target).items()
                                      if not name.endswith(".cargo-lock")}, before)
                elif mutation == "positive":
                    self.assertEqual(code, 0, report)
                    self.assertFalse(latest.exists())
                    self.assertFalse(old.exists())
                    self.assertTrue(working.exists())
                    self.assertEqual(self.file_tree_bytes(selected_owner), {
                        name.removeprefix(str(selected_owner.relative_to(target)) + "/"): data
                        for name, data in before.items()
                        if name.startswith(str(selected_owner.relative_to(target)) + "/")
                    })
                    after = self.file_tree_bytes(target)
                    for name, data in before.items():
                        if not name.startswith("debug/incremental/") and not name.endswith(".cargo-lock"):
                            self.assertEqual(after[name], data)
                elif mutation == "metadata":
                    self.assertEqual(code, 0, report)
                    self.assertTrue(latest.is_dir())
                    self.assertEqual((latest / "metadata.rmeta").read_bytes(), b"retain whole session")
                    self.assertTrue(lock_path.exists())
                    self.assertFalse(any((latest / name).exists() for name in (
                        "dep-graph.bin", "query-cache.bin", "work-products.bin",
                    )))
                else:
                    self.assertEqual(code, 1, report)
                    self.assertEqual(report["deleted_files"], 0)
                    self.assertTrue(latest.exists())
                    self.assertTrue(old.exists())


    def metadata_leaf_fixture(self, directory: str) -> tuple[Path, Path, list[Path], list[Path]]:
        target, profile, _fp, artifact, old, latest, _working = self.retained_incremental_fixture(directory)
        owner = old.parent.with_name("unselected-0000000000001")
        old.parent.rename(owner)
        sessions = [owner / old.name, owner / latest.name]
        for index, session in enumerate(sessions):
            metadata = session / "metadata.rmeta"
            metadata.write_bytes(b"published metadata" * 1024)
            os.link(metadata, profile / "deps" / f"libunselected-{index:016x}.rmeta")
        root = Path(directory).resolve()
        selected = owner.with_name("mathjax_consumer_facts-0000000000002")
        selected.mkdir()
        for session in sessions:
            copy = selected / session.name
            copy.mkdir()
            for path in session.iterdir():
                (copy / path.name).write_bytes(path.read_bytes())
        protected = [target / "debug/app", artifact, profile / "examples/fixture-example-1234567890abcdef"]
        for index in range(34):
            copy = root / "frozen" / str(index)
            copy.parent.mkdir(exist_ok=True)
            copy.write_bytes(f"protected copy {index}".encode())
            protected.append(copy)
        return root, target, sessions, protected

    def metadata_leaf_evict(
        self, root: Path, target: Path, protected: list[Path], needed: int, dry_run: bool = False,
    ) -> tuple[int, EvictionReport]:
        metadata = self.selected_metadata(root)
        with ProfileLocks(target, _selected_input_check(root, metadata)) as epoch:
            current = self.measured_size(target)
            return evict_artifacts(
                target, current - needed, current, dry_run, protected, epoch,
                retained_profiles=[target / "debug"], selected_package_profiles=[target / "debug"],
                unselected_incremental_prefixes=selected_package_incremental_prefixes(root, metadata),
            )

    @unittest.skipIf(os.name == "nt", "POSIX metadata cache leaf fixture")
    def test_nonselected_metadata_cache_leaves_preserve_all_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, target, sessions, protected = self.metadata_leaf_fixture(directory)
            leaves = [session / name for session in sessions
                      for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin")]
            available = sum(path.stat().st_blocks * 512 for path in leaves)
            before = self.file_tree_identity_bytes(root)
            code, preview = self.metadata_leaf_evict(root, target, protected, available, True)
            self.assertEqual(code, 0, preview)
            self.assertEqual(set(preview["candidate_paths"]), {str(path.relative_to(target)) for path in leaves})
            self.assertEqual(self.file_tree_identity_bytes(target), {
                name.removeprefix("target/"): value for name, value in before.items() if name.startswith("target/")
            })
            code, report = self.metadata_leaf_evict(root, target, protected, available)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertEqual(report.get("trimmed_incremental_sessions"), 2)
            self.assertTrue(all(session.is_dir() for session in sessions))
            removed = {str(path.relative_to(root)) for path in leaves}
            after = self.file_tree_identity_bytes(root)
            for name, value in before.items():
                if name not in removed:
                    self.assertEqual(after[name], value, name)
            self.assertFalse(any(path.exists() for path in leaves))

    @unittest.skipIf(os.name == "nt", "POSIX metadata cache leaf fixture")
    def test_metadata_cache_leaf_unsafe_sessions_are_retained(self) -> None:
        for mutation in ("selected", "unknown-owner", "unknown-session", "unknown-member", "outside-alias",
                         "published-alias", "protected-leaf", "protected-lock", "missing-lock", "ambiguous-lock",
                         "metadata-symlink", "missing-index", "shared-lock", "insufficient"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root, target, sessions, protected = self.metadata_leaf_fixture(directory)
                latest = sessions[-1]
                graph = latest / "dep-graph.bin"
                needed = sum((session / name).stat().st_blocks * 512 for session in sessions
                             for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"))
                lock = latest.parent / ("-".join(latest.name.split("-")[:3]) + ".lock")
                if mutation in ("selected", "unknown-owner"):
                    latest.parent.rename(latest.parent.with_name(
                        "mathjax_consumer_facts-0000000000003" if mutation == "selected" else "unselected-malformed",
                    ))
                elif mutation == "unknown-session":
                    latest.rename(latest.with_name("unknown-session"))
                elif mutation == "unknown-member":
                    (latest / "unknown.bin").write_bytes(b"keep")
                elif mutation in ("outside-alias", "published-alias"):
                    alias = (root / "unmanaged-cache-link" if mutation == "outside-alias"
                             else target / "debug/deps/libpublished-4444444444444444.rmeta")
                    os.link(graph, alias)
                elif mutation == "protected-leaf":
                    protected.append(graph)
                elif mutation == "protected-lock":
                    protected.append(lock)
                elif mutation == "missing-lock":
                    lock.unlink()
                elif mutation == "ambiguous-lock":
                    (latest.parent / (lock.stem + "-unknown")).write_bytes(b"keep")
                elif mutation == "metadata-symlink":
                    metadata = latest / "metadata.rmeta"
                    metadata.unlink()
                    metadata.symlink_to(target / "debug/deps/libunselected-0000000000000001.rmeta")
                elif mutation == "missing-index":
                    (latest / "query-cache.bin").unlink()
                elif mutation == "insufficient":
                    needed += 1
                before = self.file_tree_identity_bytes(target)
                with contextlib.ExitStack() as stack:
                    if mutation == "shared-lock":
                        stdout = stack.enter_context(incremental_session_lock_holder(lock, "shared"))
                        self.assertEqual(stdout.readline().strip(), "ready")
                    code, report = self.metadata_leaf_evict(root, target, protected, needed)
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertEqual(self.file_tree_identity_bytes(target), before)

    @unittest.skipUnless(hasattr(os, "chflags"), "native BSD immutable flag fixture")
    def test_metadata_cache_leaf_immutable_files_are_retained(self) -> None:
        for kind in ("leaf", "directory", "lock", "metadata", "protected"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root, target, sessions, protected = self.metadata_leaf_fixture(directory)
                session = sessions[-1]
                flagged = (session if kind == "directory" else protected[-1] if kind == "protected"
                           else session.parent / ("-".join(session.name.split("-")[:3]) + ".lock") if kind == "lock"
                           else session / ("metadata.rmeta" if kind == "metadata" else "query-cache.bin"))
                needed = sum((item / name).stat().st_blocks * 512 for item in sessions
                             for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"))
                os.chflags(flagged, stat.UF_IMMUTABLE)
                try:
                    before = self.file_tree_identity_bytes(target)
                    code, report = self.metadata_leaf_evict(root, target, protected, needed)
                    self.assertEqual(code, 0 if kind in ("metadata", "protected") else 1, report)
                    self.assertEqual(flagged.stat().st_flags, stat.UF_IMMUTABLE)
                    if kind not in ("metadata", "protected"):
                        self.assertEqual(report["deleted_files"], 0)
                        self.assertEqual(self.file_tree_identity_bytes(target), before)
                finally:
                    os.chflags(flagged, 0)

    @unittest.skipIf(os.name == "nt", "POSIX metadata cache leaf fixture")
    def test_metadata_cache_leaf_interruption_is_graph_first_and_retryable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, target, sessions, protected = self.metadata_leaf_fixture(directory)
            leaves = [session / name for session in sessions
                      for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin")]
            needed = sum(path.stat().st_blocks * 512 for path in leaves)
            before = self.file_tree_identity_bytes(target)
            original_unlink = os.unlink
            attempts: list[str] = []
            def interrupt(name: str, *, dir_fd: int | None = None) -> None:
                attempts.append(name)
                if len(attempts) == 2:
                    raise OSError("interrupted after first graph")
                original_unlink(name, dir_fd=dir_fd)
            with patch("cargo_capacity_guard_lock.os.unlink", side_effect=interrupt):
                code, report = self.metadata_leaf_evict(root, target, protected, needed)
            self.assertEqual(code, 1, report)
            self.assertEqual(attempts, ["dep-graph.bin", "dep-graph.bin"])
            self.assertEqual(report["deleted_files"], 1)
            removed = {str(path.relative_to(target)) for path in leaves if not path.exists()}
            self.assertEqual(len(removed), 1)
            after = self.file_tree_identity_bytes(target)
            self.assertEqual(after, {name: value for name, value in before.items() if name not in removed})
            remaining = [path for path in leaves if path.exists()]
            needed = sum(path.stat().st_blocks * 512 for path in remaining)
            code, report = self.metadata_leaf_evict(root, target, protected, needed)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["deleted_files"], 5)
            self.assertEqual(report.get("trimmed_incremental_sessions"), 2)
            self.assertTrue(all(session.is_dir() for session in sessions))

    @unittest.skipIf(os.name == "nt", "POSIX metadata cache leaf fixture")
    def test_metadata_cache_leaf_preserved_output_changes_block_unlink(self) -> None:
        for mutation in ("metadata", "input", "namespace", "lock", "metadata-alias"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root, target, sessions, protected = self.metadata_leaf_fixture(directory)
                metadata = self.selected_metadata(root)
                verify = _selected_input_check(root, metadata)
                needed = sum((session / name).stat().st_blocks * 512 for session in sessions
                             for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"))
                count = 0
                def change_metadata(
                    selected: str = mutation, selected_root: Path = root, selected_session: Path = sessions[-1],
                    selected_verify: Callable[[], None] = verify,
                ) -> None:
                    nonlocal count
                    count += 1
                    if count == 2:
                        if selected == "input":
                            (selected_root / "Cargo.lock").write_text("changed\n")
                        elif selected == "namespace":
                            (selected_session / "unknown.bin").write_bytes(b"keep")
                        elif selected == "lock":
                            lock = selected_session.parent / ("-".join(selected_session.name.split("-")[:3]) + ".lock")
                            lock.unlink()
                            lock.write_bytes(b"replacement")
                        elif selected == "metadata-alias":
                            os.link(selected_session / "metadata.rmeta", selected_root / "new-metadata-alias")
                        else:
                            (selected_session / "metadata.rmeta").write_bytes(b"changed by another writer")
                    selected_verify()
                with ProfileLocks(target, change_metadata) as epoch:
                    current = self.measured_size(target)
                    code, report = evict_artifacts(
                        target, current - needed, current, False, protected, epoch,
                        retained_profiles=[target / "debug"], selected_package_profiles=[target / "debug"],
                        unselected_incremental_prefixes=selected_package_incremental_prefixes(root, metadata),
                    )
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertTrue(report.get("error"), report)

    @unittest.skipIf(os.name == "nt", "POSIX actual rustc incremental fixture")
    def test_actual_rustc_rebuilds_after_metadata_cache_leaf_trim(self) -> None:
        rustc = shutil.which("rustc")
        if rustc is None:
            self.skipTest("rustc unavailable")
        with tempfile.TemporaryDirectory() as directory:
            target, profile, _fp, artifact = self.artifact_fixture(directory)
            root = Path(directory).resolve()
            source = root / "fixture.rs"
            source.write_text("pub fn value() -> u32 { 7 }\n")
            output = profile / "deps/libunselected-5555555555555555.rmeta"
            command = [rustc, "--crate-name", "unselected", "--crate-type", "lib", "--emit=metadata",
                       "-C", f"incremental={profile / 'incremental'}", "-o", str(output), str(source)]
            subprocess.run(command, check=True, capture_output=True, text=True)
            caches = list((profile / "incremental").glob("unselected-*/*/metadata.rmeta"))
            self.assertEqual(len(caches), 1)
            metadata = caches[0]
            self.assertEqual(metadata.stat().st_ino, output.stat().st_ino)
            leaves = [metadata.parent / name for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin")]
            needed = sum(path.stat().st_blocks * 512 for path in leaves)
            before = self.file_tree_identity_bytes(target)
            code, report = self.metadata_leaf_evict(root, target, [artifact, output], needed)
            self.assertEqual(code, 0, report)
            removed = {str(path.relative_to(target)) for path in leaves}
            self.assertEqual(self.file_tree_identity_bytes(target), {
                name: value for name, value in before.items() if name not in removed
            })
            subprocess.run(command, check=True, capture_output=True, text=True)
            consumer = root / "consumer.rs"
            consumer.write_text("pub fn check() -> u32 { unselected::value() }\n")
            subprocess.run([rustc, "--crate-type", "lib", "--emit=metadata", "--extern", f"unselected={output}",
                            "-o", str(root / "consumer.rmeta"), str(consumer)],
                           check=True, capture_output=True, text=True)


class GuardJsonBoundaryUnitTests(unittest.TestCase):
    def test_metadata_shapes_fail_closed_without_path_or_kind_coercion(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            manifest = str(root / "crates/katana-render-runtime/Cargo.toml")
            invalid_metadata: list[object] = [None, [], {"packages": None}, {"packages": [False]}]
            target_shapes: list[tuple[str, object]] = [("targets", {}), ("targets", [None])]
            for field, value in target_shapes:
                invalid_metadata.append({"packages": [{"manifest_path": manifest, field: value}]})
            target_fields: list[tuple[str, object]] = [("kind", "lib"), ("kind", [True]), ("name", 17), ("name", True)]
            for field, value in target_fields:
                invalid_metadata.append({"packages": [{"manifest_path": manifest,
                                                       "targets": [{"kind": ["lib"], "name": "runtime", field: value}]}]})
            for metadata in invalid_metadata:
                with self.subTest(metadata=metadata), self.assertRaises(RuntimeError):
                    runtime_incremental_prefixes(root, metadata)
            for metadata in (None, [], {"workspace_members": "package"}, {"workspace_members": [True]},
                             {"workspace_members": ["package"], "packages": [{"id": []}]}):
                with self.subTest(workspace=metadata), self.assertRaises(RuntimeError):
                    workspace_incremental_prefixes(root, metadata)
            for value in (True, 17, [str(target)], {"path": str(target)}):
                with self.subTest(directory=value), self.assertRaises(RuntimeError):
                    current_build_profiles(target, {"target_directory": value}, "debug")

    def test_state_shapes_and_baseline_scalars_fail_closed_without_rewriting_state(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            invalid_values: list[object] = [None, [], True, "state"]
            invalid_baselines: list[object] = [True, False, "4096", [], {}, float("nan"), float("inf")]
            for baseline in invalid_baselines:
                invalid_values.append({"baseline_bytes": baseline, "target": str(target),
                                       "metric": "unique-inode-allocated-bytes", "metric_version": 1})
            for value in invalid_values:
                contents = json.dumps(value)
                state.write_text(contents)
                with self.subTest(state=value), self.assertRaises(RuntimeError):
                    inspect_capacity(root, target, state, 2.0)
                self.assertEqual(state.read_text(), contents)
            for text in ("not-json", "{"):
                state.write_text(text)
                self.assertEqual(load_state(state), {})

    def test_cleanup_report_scalars_and_attempt_shapes_fail_closed_before_mutation(self) -> None:
        invalid_fields: list[tuple[str, object]] = [
            ("current_bytes", True), ("current_bytes", "4096"), ("current_bytes", 4096.5),
            ("post_clean_bytes", False), ("post_clean_bytes", "0"),
            ("threshold_bytes", True), ("threshold_bytes", "2048"),
            ("active_sweep_maxsize_bytes", False), ("active_sweep_maxsize_bytes", "2048"),
            ("cleanup_attempts", {}), ("cleanup_attempts", [False]),
        ]
        attempt: dict[str, object] = {
            "attempt": 1, "exit_code": 0, "actual_removed_bytes": 0, "pre_clean_bytes": 4096,
            "post_clean_bytes": 4096, "maxsize_bytes": 2048, "output": "measured", "dry_run": False,
        }
        invalid_attempt_fields: list[tuple[str, object]] = [
            ("attempt", True), ("exit_code", "0"), ("actual_removed_bytes", False),
            ("pre_clean_bytes", "4096"), ("post_clean_bytes", True),
            ("maxsize_bytes", "2048"), ("output", 17), ("dry_run", 0),
        ]
        for field, value in invalid_attempt_fields:
            invalid_fields.append(("cleanup_attempts", [{**attempt, field: value}]))
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            artifact = target / "retained"
            artifact.write_bytes(b"retained")
            for field, value in invalid_fields:
                report: dict[str, object] = {"current_bytes": 4096, "threshold_bytes": 2048, field: value}
                before = json.dumps(report, sort_keys=True)
                with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                    record_cleanup_result(report, target, 0, "measured", False)
                self.assertEqual(json.dumps(report, sort_keys=True), before)
                self.assertEqual(artifact.read_bytes(), b"retained")


class ExpectedStateBindingUnitTests(unittest.TestCase):
    def test_verified_state_parses_the_same_single_read_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state.json"
            raw = b'{"baseline_bytes": 17}'
            state.write_bytes(raw)
            checksum = hashlib.sha256(raw).hexdigest()
            def replace_after_read(path: Path) -> bytes:
                path.write_bytes(b'{"baseline_bytes": 29}')
                return raw
            with patch.object(Path, "read_bytes", autospec=True, side_effect=replace_after_read) as read, \
                    patch.object(Path, "read_text", side_effect=AssertionError("second state read")):
                self.assertEqual(load_state(state, checksum), {"baseline_bytes": 17})
                read.assert_called_once_with(state)

    def test_expected_state_missing_digest_or_json_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state.json"
            with self.assertRaisesRegex(RuntimeError, "unavailable"):
                load_state(state, "0" * 64)
            for raw, checksum in ((b"{}", "F" * 64), (b"{}", "0" * 64),
                                  (b"{", hashlib.sha256(b"{").hexdigest()),
                                  (b"[]", hashlib.sha256(b"[]").hexdigest())):
                state.write_bytes(raw)
                with self.subTest(raw=raw, checksum=checksum), self.assertRaises(RuntimeError):
                    load_state(state, checksum)
            saved = state.with_name("saved.json")
            state.rename(saved)
            state.symlink_to(saved)
            with self.assertRaisesRegex(RuntimeError, "symlink"):
                load_state(state, hashlib.sha256(saved.read_bytes()).hexdigest())

    def test_state_mutations_reject_cleanup_build_and_post_build_acceptance(self) -> None:
        for stage in ("locked-read", "cleanup", "build", "post-build"):
            with self.subTest(stage=stage):
                self.mutation_case(stage)

    def mutation_case(self, stage: str) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            raw = json.dumps({"baseline_bytes": 1 if stage == "cleanup" else 8192,
                              "target": str(target), "metric": "unique-inode-allocated-bytes", "metric_version": 1}).encode()
            state.write_bytes(raw)
            checksum = hashlib.sha256(raw).hexdigest()
            args = argparse.Namespace(target=str(target), target_dir=None, state=str(state), factor=2.0,
                                      dry_run=False, print_json=True, command=["never-run"],
                                      expected_state_sha256=checksum)
            locked = False
            @contextlib.contextmanager
            def held_lock(path: Path) -> Iterator[None]:
                nonlocal locked
                locked = True
                if stage == "locked-read":
                    state.write_bytes(b"{}")
                yield
                locked = False
            native_read = Path.read_bytes
            def read_locked(path: Path) -> bytes:
                self.assertTrue(locked)
                return native_read(path)
            busy_calls = 0
            def busy(path: Path, repo: Path) -> list[str]:
                nonlocal busy_calls
                busy_calls += 1
                if busy_calls == 2:
                    state.write_bytes(b"{}")
                return []
            def report(*values: object, **options: object) -> None:
                if stage == "build":
                    state.write_bytes(b"{}")
            def child(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
                state.write_bytes(b"{}")
                return subprocess.CompletedProcess(command, 0)
            with patch("cargo_capacity_guard._root", return_value=root), \
                    patch("cargo_capacity_guard.GuardLock", side_effect=held_lock), \
                    patch.object(Path, "read_bytes", autospec=True, side_effect=read_locked), \
                    patch("cargo_capacity_guard.size_bytes", return_value=8192), \
                    patch("cargo_capacity_guard.busy_processes", side_effect=busy), \
                    patch("cargo_capacity_guard.evict_artifacts") as eviction, \
                    patch("cargo_capacity_guard.subprocess.run", side_effect=child) as build, \
                    patch("cargo_capacity_guard.save_state") as save, patch("builtins.print", side_effect=report):
                with self.assertRaisesRegex(RuntimeError, "SHA256 differs"):
                    run_guard(args)
                eviction.assert_not_called()
                self.assertEqual(build.call_count, int(stage == "post-build"))
                save.assert_not_called()

    def test_expected_state_never_records_a_new_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            target.mkdir()
            state = root / "state.json"
            state.write_bytes(b"{}")
            args = argparse.Namespace(target=str(target), target_dir=None, state=str(state), factor=2.0,
                                      dry_run=False, print_json=True, command=["cargo", "build"],
                                      expected_state_sha256=hashlib.sha256(b"{}").hexdigest())
            with patch("cargo_capacity_guard._root", return_value=root), \
                    patch("cargo_capacity_guard.size_bytes", side_effect=[0, 8192]), \
                    patch("cargo_capacity_guard.subprocess.run", return_value=subprocess.CompletedProcess(args.command, 0)), \
                    patch("cargo_capacity_guard.save_state") as save, contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(run_guard(args), 0)
                save.assert_not_called()
            self.assertEqual(state.read_bytes(), b"{}")


class CoverageDomainUnitTests(unittest.TestCase):
    @contextlib.contextmanager
    def fixture(self, domain: str = "performance-baseline", action: str = "clean") -> Iterator[tuple[Path, argparse.Namespace, dict[str, str]]]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            target.mkdir()
            (root / "Cargo.toml").write_text('[workspace]\n')
            manifest = root / "scripts/html/performance-harness/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text('[workspace]\n')
            state = root / "tmp/.cargo-capacity-guard.json"
            state.parent.mkdir()
            state.write_text(json.dumps({"baseline_bytes": 16777216, "factor": 2.0,
                                         "target": str(target), "metric": "unique-inode-allocated-bytes",
                                         "metric_version": 1}))
            reference = root / "consumer"
            reference.write_bytes(b"admitted-consumer")
            command = ["cargo", "llvm-cov"]
            if action == "clean":
                command.append("clean")
            if domain != "root":
                command += ["--manifest-path", "scripts/html/performance-harness/Cargo.toml", "--locked"]
            command.append("--workspace")
            if action == "test":
                command += ["--all-targets", "--summary-only", "--fail-under-lines", "100", "--fail-uncovered-lines", "0"]
                command += (["--all-features", "--locked"] if domain == "root" else
                            ["--no-default-features", "--features", domain.removeprefix("performance-")])
            args = argparse.Namespace(target=str(target), target_dir=None, state=None, factor=2.0,
                                      dry_run=False, print_json=True, command=command, coverage_domain=domain,
                                      expected_state_sha256=None, protected_ref=[str(reference)])
            environment = {"CARGO_TARGET_DIR": str(target), "CARGO_LLVM_COV_TARGET_DIR": str(target / (
                "llvm-cov-target" if domain == "root" else f"llvm-cov-{domain}"))}
            self.calls: list[list[str]] = []
            self.output = io.StringIO()
            self.errors = io.StringIO()
            yield root, args, environment

    def execute(self, root: Path, args: argparse.Namespace, environment: dict[str, str],
                version: bytes = b"cargo-llvm-cov 0.9.1\n", code: int = 0,
                configuration: Path | None = None, worker: Callable[[], None] | None = None) -> int:
        def child(command: list[str], **options: object) -> subprocess.CompletedProcess[bytes]:
            self.calls.append(command)
            if command[-1] == "--version":
                self.assertEqual(options.get("timeout"), 30)
                return subprocess.CompletedProcess(command, 0, stdout=version, stderr=b"tool-raw\n")
            if worker is not None:
                worker()
            return subprocess.CompletedProcess(command, code)
        with patch("cargo_capacity_guard._root", return_value=root), \
                patch("cargo_capacity_guard.resolve_target", return_value=root / "target"), \
                patch("cargo_capacity_guard.cargo_configuration_paths", return_value=[] if configuration is None else [configuration]), \
                patch("cargo_capacity_guard.busy_processes", return_value=[]), \
                patch("cargo_capacity_guard.subprocess.run", side_effect=child), \
                patch.dict(os.environ, environment, clear=True), \
                contextlib.redirect_stdout(self.output), contextlib.redirect_stderr(self.errors):
            return run_guard(args)

    def assert_no_worker(self) -> None:
        self.assertFalse(any(command[-1] != "--version" for command in self.calls))

    def test_coverage_domains_clean_and_default_test_preserve_exit_and_state(self) -> None:
        for domain in ("root", "performance-baseline", "performance-candidate"):
            for action in ("clean", "test"):
                with self.subTest(domain=domain, action=action), self.fixture(domain, action) as (root, args, environment):
                    state = root / "tmp/.cargo-capacity-guard.json"
                    before = state.read_bytes()
                    self.assertEqual(self.execute(root, args, environment, code=7), 7)
                    self.assertEqual(self.calls[-1], args.command)
                    self.assertEqual(state.read_bytes(), before)
                    evidence = json.loads(self.errors.getvalue())['coverage_tool_query']
                    self.assertEqual(bytes.fromhex(evidence['stdout_hex']), b"cargo-llvm-cov 0.9.1\n")
                    self.assertEqual(bytes.fromhex(evidence['stderr_hex']), b"tool-raw\n")

    def test_coverage_fresh_ci_empty_consumer_closure_is_recorded_exactly(self) -> None:
        for declared in (False, True):
            with self.subTest(declared=declared), self.fixture() as (root, args, environment):
                args.protected_ref = []
                if declared:
                    environment['CARGO_CAPACITY_PROTECTED_REFS'] = '[]'
                self.assertEqual(self.execute(root, args, environment), 0)
                self.assertEqual(json.loads(self.output.getvalue())['coverage_inputs']['protected_references'], [])

    def test_coverage_wrappers_jobs_threads_and_fixed_json_report(self) -> None:
        for prefix in (["rtk", "cargo"], ["rtk", "proxy", "cargo"], ["/usr/local/bin/cargo"]):
            with self.subTest(prefix=prefix), self.fixture(action="test") as (root, args, environment):
                args.command = [*prefix, *args.command[1:], "--json", "--output-path",
                                str(root / "tmp/quality/standalone-performance-coverage/baseline.json"),
                                "-j", "2", "--", "--test-threads=1"]
                self.assertEqual(self.execute(root, args, environment), 0)

    def test_coverage_unknown_dispatch_and_options_never_execute_worker(self) -> None:
        extra = [["--no-clean"], ["--no-run"], ["--no-report"], ["--profraw-only"],
                 ["--target-dir", "elsewhere"], ["--config", "foo"], ["--profile", "other"],
                 ["--all-features"], ["--workspace"], ["--jobs", "0"], ["--", "--test-threads=0"]]
        for words in extra:
            with self.subTest(words=words), self.fixture(action="test") as (root, args, environment):
                args.command += words
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment)
                self.assertEqual(self.calls, [])
        with self.fixture() as (root, args, environment):
            args.command = ["cargo", "+nightly", *args.command[1:]]
            with self.assertRaises(RuntimeError):
                self.execute(root, args, environment)
            self.assertEqual(self.calls, [])

    def test_coverage_wrong_manifest_features_threshold_report_or_missing_arm_rejects(self) -> None:
        replacements = [("--manifest-path", "Cargo.toml"), ("--features", "candidate"),
                        ("--fail-under-lines", "99"), ("--fail-uncovered-lines", "1")]
        for option, value in replacements:
            with self.subTest(option=option), self.fixture(action="test") as (root, args, environment):
                args.command[args.command.index(option) + 1] = value
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment)
                self.assertEqual(self.calls, [])
        for suffix in (["--json", "--output-path", "/tmp/unknown.json"], []):
            with self.subTest(suffix=suffix), self.fixture(action="test") as (root, args, environment):
                args.command += suffix
                if not suffix:
                    args.command.remove("--no-default-features")
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment)
                self.assertEqual(self.calls, [])

    def test_coverage_target_build_and_profile_environment_overrides_reject(self) -> None:
        overrides = {"CARGO_TARGET_DIR": "/tmp/other", "CARGO_LLVM_COV_TARGET_DIR": "/tmp/other",
                     "CARGO_LLVM_COV_BUILD_DIR": "/tmp/other", "CARGO_BUILD_BUILD_DIR": "/tmp/other",
                     "LLVM_PROFILE_FILE_NAME": "other.profraw", "CARGO_CAPACITY_FACTOR": "3"}
        for name, value in overrides.items():
            with self.subTest(name=name), self.fixture() as (root, args, environment):
                environment[name] = value
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment)
                self.assert_no_worker()

    def test_coverage_unknown_build_configuration_rejects_before_worker(self) -> None:
        for contents in ('[build]\nbuild-dir="elsewhere"\n', '[env]\nCARGO_LLVM_COV_TARGET_DIR="elsewhere"\n',
                         '[build]\ntarget="other"\n'):
            with self.subTest(contents=contents), self.fixture() as (root, args, environment):
                config = root / "config.toml"
                config.write_text(contents)
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment, configuration=config)
                self.assertEqual(self.calls, [])

    def test_coverage_manifest_and_configuration_symlinks_refuse_before_queries(self) -> None:
        for kind in ("manifest", "configuration"):
            with self.subTest(kind=kind), self.fixture() as (root, args, environment):
                path = (root / "scripts/html/performance-harness/Cargo.toml" if kind == "manifest"
                        else root / "config.toml")
                if not path.exists():
                    path.write_text('[build]\n')
                saved = path.with_suffix('.saved')
                path.rename(saved)
                path.symlink_to(saved)
                with self.assertRaisesRegex(RuntimeError, "symlink"):
                    self.execute(root, args, environment, configuration=path if kind == "configuration" else None)
                self.assertEqual(self.calls, [])

    def test_coverage_invalid_or_missing_baseline_and_noncanonical_state_reject(self) -> None:
        for kind in ("missing", "empty", "other-state", "metric"):
            with self.subTest(kind=kind), self.fixture() as (root, args, environment):
                state = root / "tmp/.cargo-capacity-guard.json"
                if kind == "missing":
                    state.unlink()
                elif kind == "empty":
                    state.write_text('{}')
                elif kind == "other-state":
                    args.state = str(root / "arm-state.json")
                else:
                    state.write_text(json.dumps({"baseline_bytes": 1, "target": str(root / "target")}))
                with self.assertRaises(RuntimeError):
                    self.execute(root, args, environment)
                self.assert_no_worker()

    def test_coverage_scalar_expected_state_environment_binds_and_rejects_conflict(self) -> None:
        for kind in ("environment", "same-cli", "invalid", "conflict", "mismatch"):
            with self.subTest(kind=kind), self.fixture() as (root, args, environment):
                state = root / "tmp/.cargo-capacity-guard.json"
                checksum = hashlib.sha256(state.read_bytes()).hexdigest()
                environment["CARGO_CAPACITY_EXPECTED_STATE_SHA256"] = checksum
                if kind == "same-cli":
                    args.expected_state_sha256 = checksum
                elif kind == "invalid":
                    environment["CARGO_CAPACITY_EXPECTED_STATE_SHA256"] = "invalid"
                elif kind == "conflict":
                    args.expected_state_sha256 = "0" * 64
                elif kind == "mismatch":
                    state.write_text('{}')
                if kind in {"environment", "same-cli"}:
                    self.assertEqual(self.execute(root, args, environment), 0)
                else:
                    with self.assertRaises(RuntimeError):
                        self.execute(root, args, environment)
                    self.assert_no_worker()

    def test_coverage_unknown_tool_version_refuses_and_keeps_raw_query(self) -> None:
        with self.fixture() as (root, args, environment):
            with self.assertRaisesRegex(RuntimeError, "0.9.1"):
                self.execute(root, args, environment, version=b"cargo-llvm-cov 0.9.2\n")
            self.assert_no_worker()
            evidence = json.loads(self.errors.getvalue())['coverage_tool_query']
            self.assertEqual(bytes.fromhex(evidence['stdout_hex']), b"cargo-llvm-cov 0.9.2\n")

    def test_coverage_tool_query_timeout_failure_keeps_partial_raw(self) -> None:
        from cargo_capacity_guard import coverage_tool_version
        command = ["cargo", "llvm-cov", "clean", "--workspace"]
        error = subprocess.TimeoutExpired(command, 30, output=b"partial-out", stderr=b"partial-err")
        with patch("cargo_capacity_guard.subprocess.run", side_effect=error), \
                contextlib.redirect_stderr(io.StringIO()) as output:
            with self.assertRaisesRegex(RuntimeError, "timed out"):
                coverage_tool_version(Path.cwd(), command)
        evidence = json.loads(output.getvalue())['coverage_tool_query']
        self.assertEqual(evidence['failure'], "timeout")
        self.assertEqual(bytes.fromhex(evidence['stdout_hex']), b"partial-out")
        self.assertEqual(bytes.fromhex(evidence['stderr_hex']), b"partial-err")

    def test_coverage_tool_query_nonzero_or_unavailable_keeps_failure_evidence(self) -> None:
        from cargo_capacity_guard import coverage_tool_version
        command = ["cargo", "llvm-cov", "clean", "--workspace"]
        failures: list[object] = [OSError('query missing'), subprocess.CompletedProcess(
            command, 9, stdout=b'failed-out', stderr=b'failed-err')]
        for failure in failures:
            with self.subTest(failure=failure), patch('cargo_capacity_guard.subprocess.run') as query, \
                    contextlib.redirect_stderr(io.StringIO()) as output:
                if isinstance(failure, OSError):
                    query.side_effect = failure
                else:
                    query.return_value = failure
                with self.assertRaises(RuntimeError):
                    coverage_tool_version(Path.cwd(), command)
            evidence = json.loads(output.getvalue())['coverage_tool_query']
            if isinstance(failure, OSError):
                self.assertEqual(evidence['failure'], 'query missing')
            else:
                self.assertEqual(evidence['exit'], 9)
                self.assertEqual(bytes.fromhex(evidence['stderr_hex']), b'failed-err')

    def test_coverage_official_shared_footprints_protect_hardlink_consumers_on_both_actions(self) -> None:
        for action in ("clean", "test"):
            for suffix in ("llvm-cov-performance-baseline/debug", "llvm-cov/html", "llvm-cov/text",
                           "ui", "tests/trybuild", "tests/target"):
                with self.subTest(action=action, suffix=suffix), self.fixture(action=action) as (root, args, environment):
                    directory = root / "target" / suffix
                    directory.mkdir(parents=True)
                    alias = directory / "alias"
                    os.link(args.protected_ref[0], alias)
                    with self.assertRaisesRegex(RuntimeError, "inode"):
                        self.execute(root, args, environment)
                    self.assert_no_worker()
                    self.assertEqual(alias.read_bytes(), b"admitted-consumer")

    def test_coverage_missing_symlink_and_unknown_protected_entries_refuse(self) -> None:
        for kind in ("missing", "symlink", "directory", "protected-path", "footprint-symlink"):
            with self.subTest(kind=kind), self.fixture() as (root, args, environment):
                reference = Path(args.protected_ref[0])
                if kind == "missing":
                    reference.unlink()
                elif kind == "directory":
                    args.protected_ref = [str(root)]
                elif kind == "symlink":
                    alias = root / "alias"
                    alias.symlink_to(reference)
                    args.protected_ref = [str(alias)]
                elif kind == "protected-path":
                    directory = Path(environment['CARGO_LLVM_COV_TARGET_DIR'])
                    directory.mkdir()
                    destination = directory / 'protected'
                    reference.rename(destination)
                    args.protected_ref = [str(destination)]
                else:
                    Path(environment['CARGO_LLVM_COV_TARGET_DIR']).symlink_to(root, target_is_directory=True)
                with self.assertRaises((RuntimeError, OSError)):
                    self.execute(root, args, environment)
                self.assert_no_worker()

    def test_coverage_busy_and_lock_failure_do_not_run_child(self) -> None:
        from cargo_capacity_guard import coverage_cleanup_preflight
        for kind in ("busy", "lock", "unavailable"):
            with self.subTest(kind=kind), self.fixture() as (root, args, environment):
                busy = ["inspection unavailable"] if kind == "unavailable" else ["consumer busy"] if kind == "busy" else []
                with patch("cargo_capacity_guard.busy_processes", return_value=busy), \
                        patch("cargo_capacity_guard.ProfileLocks", side_effect=BlockingIOError("held")):
                    with self.assertRaises(RuntimeError):
                        coverage_cleanup_preflight(root, root / "target", Path(environment['CARGO_LLVM_COV_TARGET_DIR']), args.protected_ref)

    def test_coverage_profile_probe_releases_before_child_while_guard_lock_is_held(self) -> None:
        events: list[str] = []
        @contextlib.contextmanager
        def guard(path: Path) -> Iterator[None]:
            events.append("guard-enter")
            yield
            events.append("guard-exit")
        class Probe:
            def __init__(self, target: Path) -> None:
                pass
            def __enter__(self) -> Probe:
                events.append("probe-enter")
                return self
            def verify(self) -> None:
                events.append("probe-verify")
            def __exit__(self, *args: object) -> None:
                events.append("probe-exit")
        def worker() -> None:
            self.assertEqual(events[-1], "probe-exit")
            events.append("child")
        with self.fixture() as (root, args, environment), \
                patch("cargo_capacity_guard.GuardLock", side_effect=guard), \
                patch("cargo_capacity_guard.ProfileLocks", Probe):
            self.assertEqual(self.execute(root, args, environment, worker=worker), 0)
        self.assertEqual(events, ["guard-enter", "probe-enter", "probe-verify", "probe-exit", "child", "guard-exit"])

    def test_coverage_reference_mutation_is_rejected_before_worker(self) -> None:
        from cargo_capacity_guard import _coverage_stamp
        with self.fixture() as (root, args, environment):
            reference = Path(args.protected_ref[0])
            observed = 0
            def mutate(path: Path, regular: bool = False) -> tuple[int, int, int, int, int, int]:
                nonlocal observed
                if path == reference:
                    observed += 1
                    if observed == 2:
                        reference.write_bytes(b"changed-consumer")
                return _coverage_stamp(path, regular)
            with patch("cargo_capacity_guard._coverage_stamp", side_effect=mutate):
                with self.assertRaisesRegex(RuntimeError, "changed"):
                    self.execute(root, args, environment)
            self.assert_no_worker()

    def test_general_command_does_not_read_coverage_scalar_environment(self) -> None:
        with self.fixture() as (root, args, environment):
            args.coverage_domain = None
            args.command = ["owned-child"]
            environment["CARGO_CAPACITY_EXPECTED_STATE_SHA256"] = "invalid"
            self.assertEqual(self.execute(root, args, environment), 0)
            self.assertEqual(self.calls, [["owned-child"]])


if __name__ == "__main__":
    unittest.main()
