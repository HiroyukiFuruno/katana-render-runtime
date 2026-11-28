"""Reuse a trusted pass for KRR's fixed release Python unit-test lane."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from pathlib import Path

import native_coverage_local_anchor as anchor

PRODUCER = "automation-performance-bootstrap-test"
REPORT_RELATIVE = "tmp/quality/automation-release-test-cache/results.json"
LOCK_RELATIVE = ".moon/cache/automation-release-test-cache.lock"
REPORT_SCHEMA = "krr-release-unittest-results-v1"


class AutomationReleaseTestCacheError(RuntimeError):
    """Raised when a fixed release-unit result cannot be safely reused."""


@contextmanager
def _locks(repo: Path) -> Iterator[None]:
    hooks = str(Path(__file__).resolve().parents[1] / "hooks")
    sys.path.insert(0, hooks)
    try:
        from local_review_lock import review_lock
        from local_review_state import ReviewError
    finally:
        sys.path.remove(hooks)
    try:
        path = repo / LOCK_RELATIVE
        path.parent.mkdir(parents=True, exist_ok=True)
        with review_lock(path):
            yield
    except ReviewError as error:
        raise AutomationReleaseTestCacheError(str(error)) from error


def _read_report(repo: Path, path: Path | None = None) -> bytes | None:
    path = repo / REPORT_RELATIVE if path is None else path
    try:
        if path.parent.resolve(strict=True) != path.parent:
            return None
        metadata = path.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            return None
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        with os.fdopen(descriptor, "rb") as source:
            opened = os.fstat(source.fileno())
            if opened.st_dev != metadata.st_dev or opened.st_ino != metadata.st_ino:
                return None
            value = source.read()
    except OSError:
        return None
    try:
        report = json.loads(value)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    cases = report.get("cases") if isinstance(report, dict) else None
    if (
        not isinstance(report, dict)
        or set(report) != {"schema", "cases"}
        or report.get("schema") != REPORT_SCHEMA
        or not isinstance(cases, list)
        or not cases
    ):
        return None
    identifiers: set[str] = set()
    for case in cases:
        if (
            not isinstance(case, dict)
            or set(case) != {"id", "status"}
            or not isinstance(case.get("id"), str)
            or not case["id"]
            or case["id"] in identifiers
            or case.get("status") not in ("passed", "skipped")
        ):
            return None
        identifiers.add(case["id"])
    return value


def _run_tests(repo: Path, environment: dict[str, str]) -> int:
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts/release",
            "-p",
            "*_test.py",
            "--failfast",
        ],
        cwd=repo,
        env=environment,
        check=False,
    )
    return completed.returncode


def _write_report(
    repo: Path, interpreter: str, environment: dict[str, str], report_path: Path
) -> int:
    completed = subprocess.run(
        [interpreter, "scripts/release/automation_release_test_cases.py", str(report_path)],
        cwd=repo,
        env=environment,
        check=False,
    )
    return completed.returncode


def _private_result_base(repo: Path) -> Path:
    base = repo / "tmp/quality/automation-release-test-cache"
    base.mkdir(parents=True, exist_ok=True, mode=0o700)
    metadata = base.lstat()
    if base.resolve(strict=True) != base or not stat.S_ISDIR(metadata.st_mode):
        raise AutomationReleaseTestCacheError("release unit-test result base is unsafe")
    return base


def _replace_report(repo: Path, temporary: Path, report: bytes) -> None:
    destination = repo / REPORT_RELATIVE
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if _read_report(repo, temporary) != report:
        raise AutomationReleaseTestCacheError("release unit-test temporary report changed")
    os.replace(temporary, destination)
    if _read_report(repo) != report:
        raise AutomationReleaseTestCacheError("release unit-test report replacement failed")


def ensure(
    repo: Path,
    interpreter: str,
    environment: dict[str, str],
    identity: Callable[[], str | None],
) -> int:
    """Return the fixed lane's exit code; reuse only same-input anchored results."""
    root = repo.resolve(strict=True)
    with _locks(root):
        before = identity()
        if before is None:
            return _run_tests(root, environment)
        epoch = anchor.prepare(root, PRODUCER, before)
        if epoch is None:
            return _run_tests(root, environment)
        report = _read_report(root)
        if report is not None and anchor.verify(root, PRODUCER, before, epoch, report):
            if identity() == before:
                print("Release Python tests: reused verified successful result", file=sys.stderr)
                return 0
            anchor.invalidate(root, PRODUCER, before, epoch)
            return 1
        if not anchor.begin_runtime_script(root, PRODUCER, before, epoch):
            raise AutomationReleaseTestCacheError("release unit-test anchor conflict")
        base = _private_result_base(root)
        with tempfile.TemporaryDirectory(prefix=".run-", dir=base) as directory:
            temporary = Path(directory) / "results.json"
            status = _write_report(root, interpreter, environment, temporary)
            if status != 0:
                anchor.invalidate(root, PRODUCER, before, epoch)
                return status
            report = _read_report(root, temporary)
            if identity() != before:
                anchor.invalidate(root, PRODUCER, before, epoch)
                return 1
            if report is None:
                anchor.invalidate(root, PRODUCER, before, epoch)
                return 0
            _replace_report(root, temporary, report)
            if not anchor.commit(root, PRODUCER, before, epoch, report):
                raise AutomationReleaseTestCacheError("release unit-test anchor commit failed")
            if not anchor.verify(root, PRODUCER, before, epoch, report) or identity() != before:
                anchor.invalidate(root, PRODUCER, before, epoch)
                return 1
            return 0
