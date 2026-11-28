from __future__ import annotations

from contextlib import redirect_stderr
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import automation_release_test_cases as subject


class PassingCase(unittest.TestCase):
    def case_ok(self) -> None:
        self.assertTrue(True)


class SkippedMethodCase(unittest.TestCase):
    @unittest.skip("intentional unit skip")
    def case_skip(self) -> None:
        self.fail("unreachable")


@unittest.skip("class-level skip is not cacheable")
class SkippedClassCase(unittest.TestCase):
    def case_skip(self) -> None:
        self.fail("unreachable")


class FailingCase(unittest.TestCase):
    def case_failure(self) -> None:
        self.fail("expected failure")


class LaterCase(unittest.TestCase):
    ran = False

    def case_later(self) -> None:
        type(self).ran = True


class SuccessfulSubtestsCase(unittest.TestCase):
    def case_subtests(self) -> None:
        for index in range(2):
            with self.subTest(index=index):
                self.assertGreaterEqual(index, 0)


class SkippedSubtestCase(unittest.TestCase):
    def case_subtest_skip(self) -> None:
        with self.subTest(label="skip"):
            self.skipTest("subtest skip is not attributable to the parent roster")


class ReleaseTestCaseReportTests(unittest.TestCase):
    def suite_for(self, case: type[unittest.TestCase], method: str) -> unittest.TestSuite:
        return unittest.TestSuite([case(method)])

    def test_records_exact_pass_and_skip_roster(self) -> None:
        suite = unittest.TestSuite(
            [self.suite_for(PassingCase, "case_ok"), self.suite_for(SkippedMethodCase, "case_skip")]
        )
        status, report = subject.run_suite(suite)
        self.assertEqual(status, 0)
        self.assertEqual(
            report,
            {
                "schema": subject.SCHEMA,
                "cases": [
                    {"id": f"{PassingCase.__module__}.{PassingCase.__qualname__}.case_ok", "status": "passed"},
                    {"id": f"{SkippedMethodCase.__module__}.{SkippedMethodCase.__qualname__}.case_skip", "status": "skipped"},
                ],
            },
        )

    def test_class_level_skip_runs_without_cacheable_report(self) -> None:
        status, report = subject.run_suite(self.suite_for(SkippedClassCase, "case_skip"))
        self.assertEqual(status, 0)
        self.assertIsNone(report)

    def test_failure_has_nonzero_exit_and_no_report(self) -> None:
        diagnostics = io.StringIO()
        with redirect_stderr(diagnostics):
            status, report = subject.run_suite(self.suite_for(FailingCase, "case_failure"))
        self.assertEqual(status, 1)
        self.assertIsNone(report)
        self.assertIn("FailingCase.case_failure", diagnostics.getvalue())
        self.assertIn("AssertionError: expected failure", diagnostics.getvalue())

    def test_failure_stops_before_later_case(self) -> None:
        LaterCase.ran = False
        suite = unittest.TestSuite(
            [self.suite_for(FailingCase, "case_failure"), self.suite_for(LaterCase, "case_later")]
        )
        with redirect_stderr(io.StringIO()):
            status, report = subject.run_suite(suite)
        self.assertEqual(status, 1)
        self.assertIsNone(report)
        self.assertFalse(LaterCase.ran)

    def test_duplicate_discovery_roster_runs_uncached(self) -> None:
        suite = unittest.TestSuite(
            [self.suite_for(PassingCase, "case_ok"), self.suite_for(PassingCase, "case_ok")]
        )
        status, report = subject.run_suite(suite)
        self.assertEqual(status, 0)
        self.assertIsNone(report)

    def test_empty_discovery_returns_unittest_no_tests_exit(self) -> None:
        self.assertEqual(subject.run_suite(unittest.TestSuite()), (5, None))

    def test_successful_subtests_keep_the_parent_case_cacheable(self) -> None:
        status, report = subject.run_suite(self.suite_for(SuccessfulSubtestsCase, "case_subtests"))
        self.assertEqual(status, 0)
        self.assertEqual(
            report,
            {
                "schema": subject.SCHEMA,
                "cases": [
                    {
                        "id": f"{SuccessfulSubtestsCase.__module__}.{SuccessfulSubtestsCase.__qualname__}.case_subtests",
                        "status": "passed",
                    }
                ],
            },
        )

    def test_skipped_subtest_runs_uncached(self) -> None:
        status, report = subject.run_suite(self.suite_for(SkippedSubtestCase, "case_subtest_skip"))
        self.assertEqual(status, 0)
        self.assertIsNone(report)

    def test_report_is_exclusive_structured_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "result.json"
            payload = {"schema": subject.SCHEMA, "cases": [{"id": "case", "status": "passed"}]}
            subject.write_report(path, payload)
            self.assertEqual(json.loads(path.read_text()), payload)
            with self.assertRaises(FileExistsError):
                subject.write_report(path, payload)

    def test_discovery_matches_canonical_cli_scope(self) -> None:
        suite = unittest.TestSuite()
        with patch.object(unittest.defaultTestLoader, "discover", return_value=suite) as discover:
            self.assertIs(subject.discover_suite(), suite)
        discover.assert_called_once_with("scripts/release", pattern="*_test.py")


if __name__ == "__main__":
    unittest.main()
