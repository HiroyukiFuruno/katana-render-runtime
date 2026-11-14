"""Run one fixed Bun runtime-script producer and commit its receipt."""

from __future__ import annotations

import json
import os
import secrets
import stat
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO

import native_coverage_local_anchor as anchor
import runtime_script_interrupt as interrupt
import runtime_script_junit as junit
import runtime_script_junit_outputs as junit_outputs
import runtime_script_junit_publish as junit_publish
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result


@dataclass(frozen=True)
class RuntimeScriptRun:
    producer: str
    identity_sha256: str
    epoch: str
    process_exit: int
    raw_log_directory: str
    receipt_path: str
    reports: tuple[tuple[str, bytes], ...]


class RuntimeScriptProducerError(RuntimeError):
    """Raised when a fixed runtime-script producer cannot be trusted."""


def _ensure_directory(path: Path) -> None:
    missing: list[Path] = []
    current = path
    while not current.exists():
        missing.append(current)
        current = current.parent
    for component in reversed(missing):
        component.mkdir(mode=0o700)
    current = path
    while True:
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeScriptProducerError("runtime-script directory is not private")
        if current == current.parent:
            break
        current = current.parent
    if stat.S_IMODE(path.lstat().st_mode) != 0o700:
        raise RuntimeScriptProducerError("runtime-script directory is not private")


def _new_directory(parent: Path) -> Path:
    _ensure_directory(parent)
    for _ in range(16):
        candidate = parent / f"run-{secrets.token_hex(12)}"
        try:
            candidate.mkdir(mode=0o700)
        except FileExistsError:
            continue
        return candidate
    raise RuntimeScriptProducerError("runtime-script raw log directory unavailable")


def _lease(repo: Path, producer: str) -> tuple[Path, int, int]:
    directory = repo / "tmp" / "quality" / "runtime-script-results"
    _ensure_directory(directory)
    path = directory / f".{producer}.lease"
    try:
        descriptor = os.open(
            path,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
    except FileExistsError as error:
        raise RuntimeScriptProducerError("same runtime-script profile is already running") from error
    except OSError as error:
        raise RuntimeScriptProducerError("runtime-script lease unavailable") from error
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(f"producer={producer}\n".encode())
        stream.flush()
        os.fsync(stream.fileno())
    metadata = path.stat()
    return path, metadata.st_dev, metadata.st_ino


def _release_lease(path: Path, device: int, inode: int) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode) or metadata.st_dev != device or metadata.st_ino != inode:
        raise RuntimeScriptProducerError("runtime-script lease changed before release")
    path.unlink()


def _write_exclusive(path: Path, data: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def _open_raw(path: Path) -> BinaryIO:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    return os.fdopen(descriptor, "wb")


def _expected_files(argv: tuple[str, ...], producer: str) -> tuple[str, ...]:
    if argv[: len(recipe_reader.TEST_PREFIX)] != recipe_reader.TEST_PREFIX:
        raise RuntimeScriptProducerError("unsupported Bun producer command")
    files = argv[len(recipe_reader.TEST_PREFIX) :]
    if not files or any(
        not profiles.is_supported_test_path(path, producer) for path in files
    ):
        raise RuntimeScriptProducerError("Bun producer test inventory is unsupported")
    expected = profiles.fixed_profile_test_path(producer)
    if expected is not None and tuple(files) != (expected,):
        raise RuntimeScriptProducerError("Bun producer test inventory does not match profile")
    return tuple(path.removeprefix("./") for path in files)


def run_runtime_script(
    repo: Path,
    producer: str,
    input_sha256: str,
    recipe: recipe_reader.RuntimeScriptRecipe,
) -> RuntimeScriptRun:
    """Execute the recipe's literal Bun argv groups and commit a trusted receipt."""
    if producer not in profiles.accepted_producers():
        raise RuntimeScriptProducerError("unsupported runtime-script producer")
    if not repo.is_absolute() or repo.resolve(strict=True) != repo:
        raise RuntimeScriptProducerError("repository root is not canonical")
    if not recipe.commands:
        raise RuntimeScriptProducerError("runtime-script recipe has no commands")
    if producer != profiles.DEFAULT_PRODUCER and len(recipe.commands) != 1:
        raise RuntimeScriptProducerError("fixed runtime-script profile requires one command")
    try:
        identity_sha256 = result.build_identity(producer, input_sha256)
    except result.RuntimeScriptResultError as error:
        raise RuntimeScriptProducerError(str(error)) from error
    interrupt.raise_if_interrupted()
    lease_path, lease_device, lease_inode = _lease(repo, producer)
    try:
        epoch = anchor.prepare(repo, producer, identity_sha256)
        if epoch is None or not anchor.begin_runtime_script(repo, producer, identity_sha256, epoch):
            raise RuntimeScriptProducerError("runtime-script anchor could not be made pending")
        plan = junit_outputs.plan_reports(
            repo, producer, secrets.token_hex(32), len(recipe.commands)
        )
    except BaseException:
        try:
            _release_lease(lease_path, lease_device, lease_inode)
        except OSError:
            pass
        raise
    try:
        raw_directory = _new_directory(
            repo / "tmp" / "quality" / "runtime-script-results" / "raw"
        )
    except BaseException:
        _release_lease(lease_path, lease_device, lease_inode)
        raise
    process_exit = 0
    reaped = True
    try:
        for index, argv in enumerate(recipe.commands, start=1):
            interrupt.raise_if_interrupted()
            expected = _expected_files(argv, producer)
            actual_argv = (*argv, "--reporter=junit", "--reporter-outfile", plan.paths[index - 1])
            _write_exclusive(
                raw_directory / f"group-{index:03}-command.json",
                (json.dumps({"argv": actual_argv}, separators=(",", ":")) + "\n").encode(),
            )
            stdout = _open_raw(raw_directory / f"group-{index:03}-stdout.log")
            try:
                stderr = _open_raw(raw_directory / f"group-{index:03}-stderr.log")
            except BaseException:
                stdout.close()
                raise
            child: subprocess.Popen[bytes] | None = None
            try:
                child = subprocess.Popen(
                    list(actual_argv), cwd=repo, stdout=stdout, stderr=stderr
                )
            except OSError as error:
                stdout.close()
                stderr.close()
                raise RuntimeScriptProducerError("Bun producer could not start") from error
            try:
                reaped = False
                actual_exit = interrupt.wait_for_child(child)
                reaped = True
            finally:
                stdout.close()
                stderr.close()
            _write_exclusive(
                raw_directory / f"group-{index:03}-exit.txt",
                f"{actual_exit}\n".encode(),
            )
            interrupt.raise_if_interrupted()
            if actual_exit is None:
                raise RuntimeScriptProducerError("Bun producer exit status is unavailable")
            process_exit = actual_exit
            if actual_exit != 0:
                break
            if index <= len(plan.paths):
                staged = junit_outputs.read_output(
                    repo, plan.paths[index - 1], producer, ".xml", plan.epoch
                )
                try:
                    junit.parse_junit(staged, expected_files=expected)
                except junit.RuntimeScriptJUnitError as error:
                    raise RuntimeScriptProducerError(str(error)) from error
        if process_exit != 0:
            raise RuntimeScriptProducerError(f"Bun producer failed with exit {process_exit}")
        interrupt.raise_if_interrupted()
        reports = tuple(junit_publish.publish_current_reports(repo, plan, process_exit))
        receipt = result.make_receipt(producer, identity_sha256, epoch, list(reports), process_exit)
        receipt_path = repo / result.RECEIPT_DIRECTORY / f"{producer}.json"
        interrupt.raise_if_interrupted()
        result.commit_receipt(repo, producer, identity_sha256, epoch, receipt_path, receipt)
        return RuntimeScriptRun(
            producer,
            identity_sha256,
            epoch,
            process_exit,
            raw_directory.relative_to(repo).as_posix(),
            receipt_path.relative_to(repo).as_posix(),
            reports,
        )
    finally:
        if reaped:
            _release_lease(lease_path, lease_device, lease_inode)
