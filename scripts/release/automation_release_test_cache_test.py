from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import automation_release_test_cache as cache
import native_coverage_local_anchor as anchor
from native_coverage_result_test import initialize_fixture_repository


class AutomationReleaseTestCacheTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.report = self.root / cache.REPORT_RELATIVE
        self.report.parent.mkdir(parents=True)
        self.environment = {"PATH": os.environ.get("PATH", "")}
        self.identity = "a" * 64

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _report(self) -> bytes:
        return json.dumps({"schema": cache.REPORT_SCHEMA,
                           "cases": [{"id": "release.case", "status": "passed"}]},
                          separators=(",", ":")).encode()

    def _write_report(self) -> bytes:
        value = self._report()
        self.report.write_bytes(value)
        os.chmod(self.report, 0o600)
        return value

    def test_valid_hit_skips_test_execution(self) -> None:
        report = self._write_report()
        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=True), \
             patch.object(cache, "_run_tests") as run:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: self.identity), 0)
        run.assert_not_called()
        self.assertEqual(report, self._report())

    def test_missing_report_runs_and_commits(self) -> None:
        def write_report(*args: object) -> int:
            path = Path(str(args[-1]))
            path.write_bytes(self._report())
            os.chmod(path, 0o600)
            return 0
        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=True), \
             patch.object(anchor, "begin_runtime_script", return_value=True), \
             patch.object(anchor, "commit", return_value=True) as commit, \
             patch.object(cache, "_write_report", side_effect=write_report) as run:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: self.identity), 0)
        run.assert_called_once()
        commit.assert_called_once()

    def test_private_anchor_reuses_success_and_replaces_tampered_result(self) -> None:
        initialize_fixture_repository(self.root)
        scripts = self.root / "scripts/release"
        scripts.mkdir(parents=True, exist_ok=True)
        helper = "automation_release_test_cases.py"
        shutil.copyfile(Path(cache.__file__).with_name(helper), scripts / helper)
        (scripts / "fixture_test.py").write_text(
            "from unittest import TestCase\n\n"
            "class Example(TestCase):\n"
            "    def test_example(self):\n"
            "        self.assertEqual(1 + 1, 2)\n",
            encoding="utf-8",
        )
        with patch.object(cache, "_write_report", wraps=cache._write_report) as run:
            for _ in range(2):
                self.assertEqual(cache.ensure(self.root, sys.executable, self.environment, lambda: self.identity), 0)
            run.assert_called_once()
            original = self.report.read_bytes()
            self.report.write_bytes(original.replace(b'"passed"', b"{}"))
            self.assertEqual(cache.ensure(self.root, sys.executable, self.environment, lambda: self.identity), 0)
            self.assertEqual(run.call_count, 2)
        self.assertEqual(self.report.read_bytes(), original)

    def test_unavailable_identity_uses_canonical_fallback_once(self) -> None:
        with patch.object(cache, "_locks"), \
             patch.object(cache, "_run_tests", return_value=19) as run, \
             patch.object(cache, "_write_report") as helper:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: None), 19)
        run.assert_called_once()
        helper.assert_not_called()

    def test_failed_tests_never_commit(self) -> None:
        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=False), \
             patch.object(anchor, "begin_runtime_script", return_value=True), \
             patch.object(anchor, "commit") as commit, \
             patch.object(cache, "_write_report", return_value=7), \
             patch.object(anchor, "invalidate") as invalidate:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: self.identity), 7)
        commit.assert_not_called()
        self.assertGreaterEqual(invalidate.call_count, 1)

    def test_tampered_report_is_not_a_hit(self) -> None:
        self.report.write_bytes(b"tampered")
        os.chmod(self.report, 0o600)
        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=True), \
             patch.object(anchor, "begin_runtime_script", return_value=True), \
             patch.object(cache, "_write_report", return_value=3) as run:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: self.identity), 3)
        run.assert_called_once()

    def test_unsupported_successful_report_is_uncached(self) -> None:
        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=False), \
             patch.object(anchor, "begin_runtime_script", return_value=True), \
             patch.object(cache, "_write_report", return_value=0), \
             patch.object(anchor, "commit") as commit:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: self.identity), 0)
        commit.assert_not_called()

    def test_identity_change_rejects_result_after_execution(self) -> None:
        identities = iter([self.identity, "c" * 64])

        def produce(_repo: Path, _python: str, _environment: dict[str, str], path: Path) -> int:
            path.write_bytes(self._report())
            path.chmod(0o600)
            return 0

        with patch.object(cache, "_locks"), \
             patch.object(anchor, "prepare", return_value="b" * 64), \
             patch.object(anchor, "verify", return_value=False), \
             patch.object(anchor, "begin_runtime_script", return_value=True), \
             patch.object(cache, "_write_report", side_effect=produce), \
             patch.object(anchor, "invalidate") as invalidate:
            self.assertEqual(cache.ensure(self.root, "python", self.environment,
                                          lambda: next(identities)), 1)
        self.assertGreaterEqual(invalidate.call_count, 1)


if __name__ == "__main__":
    unittest.main()
