"""Publish validated runtime-script JUnit groups at stable Moon output paths."""

from __future__ import annotations

import os
import stat
import tempfile
from pathlib import Path

import runtime_script_junit as junit
import runtime_script_junit_outputs as outputs


def publish_current_reports(
    repo: Path, plan: outputs.ReportPlan, actual_exit: int
) -> list[tuple[str, bytes]]:
    """Validate an epoch's reports, then atomically replace the fixed output set."""
    staged = outputs.collect_reports(repo, plan, actual_exit)
    current = _current_reports(plan, staged)
    directory = _current_directory(repo)
    _reject_unexpected_outputs(directory, plan.producer, current)
    temporary = _stage_current(directory, current)
    try:
        for relative, staged_path in zip(current, temporary, strict=True):
            target = directory / Path(relative).name
            os.replace(staged_path, target)
        _sync_directory(directory)
    except OSError as error:
        raise junit.RuntimeScriptJUnitError("current JUnit publish failed") from error
    finally:
        _remove_temporary(temporary)
    return [(path, data) for path, data in current.items()]


def validate_current_records(repo: Path, records: object, producer: str) -> None:
    """Re-read and strictly compare one receipt's fixed current JUnit set."""
    if not isinstance(records, list) or not records or len(records) > 32:
        raise junit.RuntimeScriptJUnitError(
            "current JUnit receipt has no supported report set"
        )
    paths: list[str] = []
    for index, record in enumerate(records, start=1):
        if not isinstance(record, dict) or set(record) != {
            "path",
            "sha256",
            "tests",
            "failures",
            "errors",
            "skipped",
        }:
            raise junit.RuntimeScriptJUnitError(
                "current JUnit receipt has an unsupported shape"
            )
        if any(type(record.get(key)) is not int for key in junit.COUNT_KEYS):
            raise junit.RuntimeScriptJUnitError(
                "current JUnit receipt counts are invalid"
            )
        path = record.get("path")
        if not isinstance(path, str):
            raise junit.RuntimeScriptJUnitError("current JUnit receipt path is invalid")
        expected = _current_relative_path(producer, index)
        if (
            junit.require_current_output_path(path, ".xml", producer).as_posix()
            != expected
        ):
            raise junit.RuntimeScriptJUnitError(
                "current JUnit receipt set is not contiguous"
            )
        paths.append(path)
    directory = _current_directory(repo)
    allowed = {path: b"" for path in paths}
    _reject_unexpected_outputs(directory, producer, allowed)
    previous = _read_previous(directory, allowed)
    if any(data is None for data in previous.values()):
        raise junit.RuntimeScriptJUnitError("current JUnit receipt output is missing")
    report_bytes: list[tuple[str, bytes]] = []
    for path in paths:
        data = previous[Path(path).name]
        if data is None:
            raise junit.RuntimeScriptJUnitError("current JUnit report is missing")
        report_bytes.append((path, data))
    actual = junit.build_current_records(report_bytes, producer)
    if actual != records:
        raise junit.RuntimeScriptJUnitError("current JUnit bytes differ from receipt")


def _current_reports(
    plan: outputs.ReportPlan, staged: list[tuple[str, bytes]]
) -> dict[str, bytes]:
    if len(plan.paths) != len(staged) or not staged:
        raise junit.RuntimeScriptJUnitError("staged JUnit set is incomplete")
    current: dict[str, bytes] = {}
    if len(staged) > 32:
        raise junit.RuntimeScriptJUnitError("staged JUnit group count is unsupported")
    for index, (relative, data) in enumerate(staged, start=1):
        expected_stage = (
            "tmp/quality/runtime-script-results/"
            f"{plan.producer}-{plan.epoch}-group-{index:03}.xml"
        )
        if relative != expected_stage:
            raise junit.RuntimeScriptJUnitError("staged JUnit groups are not canonical")
        target = _current_relative_path(plan.producer, index)
        canonical = junit.require_current_output_path(target, ".xml", plan.producer)
        current[canonical.as_posix()] = data
    return current


def _current_directory(repo: Path) -> Path:
    root = repo.resolve(strict=True)
    directory = root
    for component in ("tmp", "quality", "runtime-script-results"):
        directory /= component
        metadata = directory.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise junit.RuntimeScriptJUnitError(
                "current JUnit parent is not a real directory"
            )
    return directory


def _reject_unexpected_outputs(
    directory: Path, producer: str, current: dict[str, bytes]
) -> None:
    prefix = f"{producer}-current-group-"
    allowed = {Path(path).name for path in current}
    with os.scandir(directory) as entries:
        for entry in entries:
            if not entry.name.startswith(prefix):
                continue
            if entry.name not in allowed:
                raise junit.RuntimeScriptJUnitError(
                    "unexpected current JUnit output exists"
                )
            metadata = entry.stat(follow_symlinks=False)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise junit.RuntimeScriptJUnitError(
                    "current JUnit output is not a private file"
                )


def _read_previous(
    directory: Path, current: dict[str, bytes]
) -> dict[str, bytes | None]:
    previous: dict[str, bytes | None] = {}
    for relative in current:
        name = Path(relative).name
        path = directory / name
        try:
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        except FileNotFoundError:
            previous[name] = None
            continue
        with os.fdopen(descriptor, "rb") as source:
            metadata = os.fstat(source.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise junit.RuntimeScriptJUnitError(
                    "current JUnit output is not a private file"
                )
            previous[name] = source.read()
    return previous


def _stage_current(directory: Path, current: dict[str, bytes]) -> list[Path]:
    staged: list[Path] = []
    try:
        for relative, data in current.items():
            name = Path(relative).name
            descriptor, raw_path = tempfile.mkstemp(prefix=f".{name}.", dir=directory)
            path = Path(raw_path)
            staged.append(path)
            with os.fdopen(descriptor, "wb") as target:
                os.fchmod(target.fileno(), 0o600)
                target.write(data)
                target.flush()
                os.fsync(target.fileno())
        return staged
    except OSError:
        _remove_temporary(staged)
        raise


def _remove_temporary(paths: list[Path]) -> None:
    for path in paths:
        path.unlink(missing_ok=True)


def _sync_directory(directory: Path) -> None:
    descriptor = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _current_relative_path(producer: str, index: int) -> str:
    return f"tmp/quality/runtime-script-results/{producer}-current-group-{index:03}.xml"
