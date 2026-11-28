"""Run the fixed release unittest suite and publish an exact successful roster."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import sys
from types import TracebackType
import unittest
from collections.abc import Callable, Mapping, Sequence
from typing import cast


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "krr-release-unittest-results-v1"
_ExceptionInfo = (
    tuple[type[BaseException], BaseException, TracebackType] | tuple[None, None, None]
)


class _RecordedResult(unittest.TextTestResult):
    executed: list[str]
    statuses: dict[str, str]
    unsupported: bool

    def ensure_state(self) -> None:
        if not hasattr(self, "executed"):
            self.executed = []
            self.statuses = {}
            self.unsupported = False

    def startTest(self, test: unittest.case.TestCase) -> None:
        self.ensure_state()
        super().startTest(test)
        identifier = _test_id(test)
        if identifier is None or identifier in self.executed:
            self.unsupported = True
            return
        self.executed.append(identifier)

    def addSuccess(self, test: unittest.case.TestCase) -> None:
        super().addSuccess(test)
        self._record(test, "passed")

    def addSkip(self, test: unittest.case.TestCase, reason: str) -> None:
        super().addSkip(test, reason)
        self._record(test, "skipped")

    def addExpectedFailure(
        self,
        test: unittest.case.TestCase,
        err: _ExceptionInfo,
    ) -> None:
        self.ensure_state()
        super().addExpectedFailure(test, err)
        self.unsupported = True

    def addSubTest(
        self,
        test: unittest.case.TestCase,
        subtest: unittest.case.TestCase,
        err: _ExceptionInfo | None,
    ) -> None:
        self.ensure_state()
        super().addSubTest(test, subtest, err)
        if err is not None:
            self.unsupported = True

    def _record(self, test: unittest.case.TestCase, status: str) -> None:
        self.ensure_state()
        identifier = _test_id(test)
        if identifier is None or identifier in self.statuses:
            self.unsupported = True
            return
        self.statuses[identifier] = status


def _test_id(test: unittest.case.TestCase) -> str | None:
    try:
        identifier = test.id()
    except Exception:
        return None
    if not isinstance(identifier, str) or not identifier or not identifier.isprintable():
        return None
    return identifier


def _roster(suite: unittest.TestSuite) -> tuple[str, ...] | None:
    cases: list[str] = []

    def visit(item: object) -> bool:
        if isinstance(item, unittest.TestSuite):
            return all(visit(child) for child in item)
        if not isinstance(item, unittest.case.TestCase):
            return False
        if bool(getattr(type(item), "__unittest_skip__", False)):
            return False
        identifier = _test_id(item)
        if identifier is None or identifier in cases:
            return False
        cases.append(identifier)
        return True

    return tuple(cases) if visit(suite) and cases else None


def run_suite(suite: unittest.TestSuite) -> tuple[int, dict[str, object] | None]:
    roster = _roster(suite)
    raw_result = unittest.TextTestRunner(
        stream=sys.stderr,
        verbosity=1,
        failfast=True,
        warnings=None if sys.warnoptions else "default",
        resultclass=cast(Callable[[object, bool, int], unittest.TextTestResult], _RecordedResult),
    ).run(suite)
    if not isinstance(raw_result, _RecordedResult):
        return 1, None
    result = raw_result
    result.ensure_state()
    if not result.wasSuccessful():
        return 1, None
    if result.testsRun == 0 and not result.skipped:
        return 5, None
    if (
        roster is None
        or result.unsupported
        or result.expectedFailures
        or result.unexpectedSuccesses
        or tuple(result.executed) != roster
        or set(result.statuses) != set(roster)
        or result.testsRun != len(roster)
    ):
        return 0, None
    return 0, {
        "schema": SCHEMA,
        "cases": [{"id": identifier, "status": result.statuses[identifier]} for identifier in roster],
    }


def discover_suite() -> unittest.TestSuite:
    return unittest.defaultTestLoader.discover("scripts/release", pattern="*_test.py")


def _report_path(value: str) -> Path:
    candidate = Path(value)
    path = candidate if candidate.is_absolute() else ROOT / candidate
    if not path.is_relative_to(ROOT) or path.name in {"", ".", ".."}:
        raise ValueError("report path is outside the repository")
    current = ROOT
    for part in path.relative_to(ROOT).parts[:-1]:
        current = current / part
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ValueError("report parent is not a real directory")
    return path


def write_report(path: Path, payload: Mapping[str, object]) -> None:
    if not _valid_payload(payload):
        raise ValueError("release unittest report has an invalid schema")
    encoded = (json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(encoded)
        output.flush()
        os.fsync(output.fileno())


def _valid_payload(payload: object) -> bool:
    if not isinstance(payload, Mapping) or set(payload) != {"schema", "cases"}:
        return False
    cases = payload.get("cases")
    if payload.get("schema") != SCHEMA or not isinstance(cases, list) or not cases:
        return False
    identifiers: set[str] = set()
    for case in cases:
        if (
            not isinstance(case, Mapping)
            or set(case) != {"id", "status"}
            or not isinstance(case.get("id"), str)
            or not case["id"]
            or not case["id"].isprintable()
            or case["id"] in identifiers
            or case.get("status") not in {"passed", "skipped"}
        ):
            return False
        identifiers.add(case["id"])
    return True


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report_path")
    args = parser.parse_args(argv)
    if Path.cwd().resolve() != ROOT:
        print("run from the repository root", file=sys.stderr)
        return 2
    try:
        report_path = _report_path(args.report_path)
        suite = discover_suite()
        status, payload = run_suite(suite)
        if status != 0:
            print("release unittest suite failed", file=sys.stderr)
            return status
        if payload is not None:
            write_report(report_path, payload)
    except (OSError, ValueError) as error:
        print(f"release unittest report unavailable: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
