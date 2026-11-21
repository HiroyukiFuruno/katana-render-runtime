"""Filesystem tests for fixed runtime-script JUnit publication."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_junit as junit
import runtime_script_junit_outputs as outputs
import runtime_script_junit_publish as publish

PRODUCER = "runtime-unused-script-regression"
EPOCH = "a" * 64
REPORT = b"<testsuite tests='1'><testcase name='pass'/></testsuite>"
OLD_REPORT = b"<testsuite tests='1'><testcase name='old'/></testsuite>"


class RuntimeScriptJunitPublishTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name)
        self.plan = outputs.plan_reports(self.repo, PRODUCER, EPOCH, 2)
        for relative in self.plan.paths:
            path = self.repo / relative
            path.write_bytes(REPORT)
            os.utime(path, ns=(self.plan.started_ns + 1, self.plan.started_ns + 1))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_publishes_only_validated_bytes_at_fixed_current_paths(self) -> None:
        reports = publish.publish_current_reports(self.repo, self.plan, 0)
        self.assertEqual(
            [path for path, _ in reports],
            [
                (
                    "tmp/quality/runtime-script-results/"
                    f"{PRODUCER}-current-group-001.xml"
                ),
                (
                    "tmp/quality/runtime-script-results/"
                    f"{PRODUCER}-current-group-002.xml"
                ),
            ],
        )
        self.assertEqual([data for _, data in reports], [REPORT, REPORT])
        records = junit.build_current_records(reports, PRODUCER)
        self.assertEqual([record["tests"] for record in records], [1, 1])
        publish.validate_current_records(self.repo, records, PRODUCER)
        self.assertEqual(
            [self.repo.joinpath(path).read_bytes() for path, _ in reports],
            [REPORT, REPORT],
        )
        self.assertTrue(all((self.repo / path).exists() for path in self.plan.paths))

    def test_current_validation_rejects_changed_bytes_and_noninteger_counts(
        self,
    ) -> None:
        reports = publish.publish_current_reports(self.repo, self.plan, 0)
        records = junit.build_current_records(reports, PRODUCER)
        target = self.repo / reports[0][0]
        target.write_bytes(REPORT.replace(b"pass", b"diff"))
        with self.assertRaisesRegex(
            junit.RuntimeScriptJUnitError, "differ from receipt"
        ):
            publish.validate_current_records(self.repo, records, PRODUCER)
        target.write_bytes(REPORT)
        malformed = [dict(record) for record in records]
        malformed[0]["tests"] = True
        with self.assertRaisesRegex(
            junit.RuntimeScriptJUnitError, "counts are invalid"
        ):
            publish.validate_current_records(self.repo, malformed, PRODUCER)

    def test_nonzero_process_exit_does_not_publish_or_remove_staging(self) -> None:
        directory = self.repo / "tmp/quality/runtime-script-results"
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            publish.publish_current_reports(self.repo, self.plan, 1)
        self.assertFalse(any(directory.glob(f"{PRODUCER}-current-group-*.xml")))
        self.assertTrue(
            all((self.repo / path).read_bytes() == REPORT for path in self.plan.paths)
        )

    def test_unexpected_current_group_rejects_before_publication(self) -> None:
        directory = self.repo / "tmp/quality/runtime-script-results"
        (directory / f"{PRODUCER}-current-group-999.xml").write_bytes(REPORT)
        with self.assertRaisesRegex(
            junit.RuntimeScriptJUnitError, "unexpected current"
        ):
            publish.publish_current_reports(self.repo, self.plan, 0)
        self.assertFalse(any(directory.glob(f"{PRODUCER}-current-group-00*.xml")))

    @unittest.skipUnless(os.name == "posix", "symlink rejection requires POSIX")
    def test_symlink_target_rejects_before_publication(self) -> None:
        directory = self.repo / "tmp/quality/runtime-script-results"
        target = directory / f"{PRODUCER}-current-group-001.xml"
        target.symlink_to(directory / "missing-target.xml")
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            publish.publish_current_reports(self.repo, self.plan, 0)
        self.assertTrue(target.is_symlink())
        self.assertFalse((directory / f"{PRODUCER}-current-group-002.xml").exists())

    def test_partial_replace_failure_cannot_validate_previous_receipt(self) -> None:
        directory = self.repo / "tmp/quality/runtime-script-results"
        existing = [
            directory / f"{PRODUCER}-current-group-{index:03}.xml" for index in (1, 2)
        ]
        for path in existing:
            path.write_bytes(OLD_REPORT)
        old_records = junit.build_current_records(
            [
                (
                    (
                        "tmp/quality/runtime-script-results/"
                        f"{PRODUCER}-current-group-{index:03}.xml"
                    ),
                    OLD_REPORT,
                )
                for index in (1, 2)
            ],
            PRODUCER,
        )
        original_replace = os.replace
        failed = False

        def fail_second_once(
            source: str | bytes | os.PathLike[str],
            destination: str | bytes | os.PathLike[str],
        ) -> None:
            nonlocal failed
            destination_name = os.path.basename(os.fsdecode(destination))
            if destination_name.endswith("group-002.xml") and not failed:
                failed = True
                raise OSError("injected second-file replacement failure")
            original_replace(source, destination)

        with (
            patch(
                "runtime_script_junit_publish.os.replace", side_effect=fail_second_once
            ),
            self.assertRaisesRegex(junit.RuntimeScriptJUnitError, "publish failed"),
        ):
            publish.publish_current_reports(self.repo, self.plan, 0)
        self.assertEqual([path.read_bytes() for path in existing], [REPORT, OLD_REPORT])
        with self.assertRaisesRegex(
            junit.RuntimeScriptJUnitError, "differ from receipt"
        ):
            publish.validate_current_records(self.repo, old_records, PRODUCER)
        self.assertFalse(any(directory.glob(f".{PRODUCER}-current-group-*.xml.*")))
        self.assertTrue(
            all((self.repo / path).read_bytes() == REPORT for path in self.plan.paths)
        )


if __name__ == "__main__":
    unittest.main()
