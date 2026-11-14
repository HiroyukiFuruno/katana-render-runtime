"""Contract tests for the fixed runtime-script result receipt."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import native_coverage_local_anchor as local_anchor
import runtime_script_junit as junit
import runtime_script_junit_outputs as junit_outputs
import runtime_script_profiles as profiles
import runtime_script_result as result
import runtime_script_result_verify as result_verify

JUNIT = b"""<?xml version='1.0'?>
<testsuites><testsuite name='fixed-leaf' tests='2'>
<testcase classname='a' name='one'/><testcase classname='a' name='two'/>
</testsuite></testsuites>"""
IDENTITY_PARTS = (
    "runtime-unused-script-regression",
    "a" * 64,
)
EPOCH = "e" * 64


def junit_path(producer: str, epoch: str, group: int) -> str:
    return (
        "tmp/quality/runtime-script-results/"
        f"{producer}-{epoch}-group-{group:03}.xml"
    )


def current_junit_path(producer: str, group: int) -> str:
    return (
        "tmp/quality/runtime-script-results/"
        f"{producer}-current-group-{group:03}.xml"
    )


class RuntimeScriptResultTests(unittest.TestCase):
    def test_all_fixed_profile_producers_bind_identity(self) -> None:
        for producer in profiles.RUNTIME_SCRIPT_PROFILES:
            identity = result.build_identity(producer, "a" * 64)
            self.assertEqual(len(identity), 64)

    def test_identity_is_bound_to_fixed_leaf_and_all_live_inputs(self) -> None:
        identity = result.build_identity(*IDENTITY_PARTS)
        self.assertEqual(len(identity), 64)
        self.assertNotEqual(
            identity,
            result.build_identity(IDENTITY_PARTS[0], "d" * 64),
        )
        with self.assertRaises(result.RuntimeScriptResultError):
            result.build_identity("other", *IDENTITY_PARTS[1:])

    def test_junit_counts_and_failures_are_read_from_cases(self) -> None:
        self.assertEqual(
            junit.parse_junit(JUNIT),
            {"tests": 2, "failures": 0, "errors": 0, "skipped": 0},
        )
        grouped = (
            b"<testsuites tests='2'><testsuite file='./scripts/runtime-bundles/"
            b"runtime-fragment-loader_test.ts' tests='2'><testcase name='a'/>"
            b"<testcase name='b'/></testsuite></testsuites>"
        )
        self.assertEqual(
            junit.parse_junit(
                grouped,
                ("./scripts/runtime-bundles/runtime-fragment-loader_test.ts",),
            )["tests"],
            2,
        )
        with self.assertRaisesRegex(junit.RuntimeScriptJUnitError, "declared test files"):
            junit.parse_junit(
                grouped,
                ("./scripts/runtime-bundles/runtime-bundle-definitions_test.ts",),
            )
        quality_path = profiles.QUALITY_SCRIPT_PROFILES[
            "runtime-quality-owned-source-inventory"
        ]
        quality_grouped = (
            b"<testsuites tests='2'><testsuite file='"
            + quality_path.encode()
            + b"' tests='2'><testcase name='a'/><testcase name='b'/></testsuite></testsuites>"
        )
        self.assertEqual(
            junit.parse_junit(quality_grouped, (quality_path,))["tests"], 2
        )
        with self.assertRaisesRegex(junit.RuntimeScriptJUnitError, "declared test files"):
            junit.parse_junit(
                quality_grouped,
                (profiles.QUALITY_SCRIPT_PROFILES[
                    "runtime-quality-owned-validation-contract"
                ],),
            )
        unsupported_quality_path = (
            b"<testsuite file='./scripts/quality/unlisted_test.ts'><testcase name='a'/></testsuite>"
        )
        with self.assertRaisesRegex(junit.RuntimeScriptJUnitError, "file path is unsupported"):
            junit.parse_junit(unsupported_quality_path)
        second = JUNIT.replace(b"name='one'", b"name='three'").replace(
            b"name='two'", b"name='four'"
        )
        receipt = result.make_receipt(
            IDENTITY_PARTS[0],
            "d" * 64,
            EPOCH,
            [
                (
                    current_junit_path(IDENTITY_PARTS[0], 1),
                    JUNIT,
                ),
                (
                    current_junit_path(IDENTITY_PARTS[0], 2),
                    second,
                ),
            ],
            0,
        )
        self.assertEqual(len(json.loads(receipt)["junitReports"]), 2)
        current_records = junit.build_current_records(
            [
                (
                    (
                        "tmp/quality/runtime-script-results/"
                        "runtime-unused-script-regression-current-group-001.xml"
                    ),
                    JUNIT,
                )
            ],
            IDENTITY_PARTS[0],
        )
        self.assertEqual(current_records[0]["tests"], 2)
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            junit.build_current_records(
                [(junit_path(IDENTITY_PARTS[0], EPOCH, 1), JUNIT)],
                IDENTITY_PARTS[0],
            )
        failed = JUNIT.replace(b"name='two'/", b"name='two'><failure/></testcase>")
        with self.assertRaises(result.RuntimeScriptResultError):
            result.make_receipt(
                "runtime-unused-script-regression",
                "d" * 64,
                EPOCH,
                [(current_junit_path(IDENTITY_PARTS[0], 1), failed)],
                0,
            )
        with self.assertRaises(result.RuntimeScriptResultError):
            result.make_receipt(
                "runtime-unused-script-regression",
                "d" * 64,
                EPOCH,
                [(current_junit_path(IDENTITY_PARTS[0], 1), JUNIT)],
                1,
            )
        mismatched_count = JUNIT.replace(b"tests='2'", b"tests='3'")
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            junit.parse_junit(mismatched_count)
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            junit.parse_junit(b"<!DOCTYPE x [<!ENTITY e 'x'>]><testsuite/>")
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            junit.parse_junit(
                b"<testsuites tests='1'><testcase name='outside'/></testsuites>"
            )
        nested = (
            b"<testsuites tests='6'><testsuite tests='6'>"
            b"<testsuite file='./scripts/runtime-bundles/inner_a_test.ts' tests='2'>"
            b"<testcase name='a1'/><testcase name='a2'/></testsuite>"
            b"<testsuite file='./scripts/runtime-bundles/inner_b_test.ts' tests='4'>"
            b"<testcase name='b1'/><testcase name='b2'/><testcase name='b3'/><testcase name='b4'/>"
            b"</testsuite></testsuite></testsuites>"
        )
        self.assertEqual(
            junit.parse_junit(
                nested,
                (
                    "./scripts/runtime-bundles/inner_a_test.ts",
                    "./scripts/runtime-bundles/inner_b_test.ts",
                ),
            )["tests"],
            6,
        )
        with self.assertRaisesRegex(junit.RuntimeScriptJUnitError, "declared count"):
            junit.parse_junit(nested.replace(b"<testsuite tests='6'>", b"<testsuite tests='5'>"))
        all_skipped = JUNIT.replace(
            b"<testcase classname='a' name='one'/>",
            b"<testcase classname='a' name='one'><skipped/></testcase>",
        ).replace(
            b"<testcase classname='a' name='two'/>",
            b"<testcase classname='a' name='two'><skipped/></testcase>",
        ).replace(b"tests='2'", b"tests='2' skipped='2'")
        with self.assertRaises(result.RuntimeScriptResultError):
            result.make_receipt(
                IDENTITY_PARTS[0],
                "d" * 64,
                EPOCH,
                [(current_junit_path(IDENTITY_PARTS[0], 1), all_skipped)],
                0,
            )
        with self.assertRaises(junit.RuntimeScriptJUnitError):
            junit.require_output_path(
                junit_path("runtime-asset-script-other", EPOCH, 1),
                ".xml",
                IDENTITY_PARTS[0],
            )

    def test_receipt_anchor_reuse_and_output_tampering_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            subprocess.run(["git", "init", "--quiet"], cwd=repo, check=True)
            identity = result.build_identity(*IDENTITY_PARTS)
            producer = IDENTITY_PARTS[0]
            epoch = local_anchor.prepare(repo, producer, identity)
            self.assertIsNotNone(epoch)
            selected_epoch = epoch or ""
            report_relative = current_junit_path(producer, 1)
            report_path = repo / report_relative
            report_path.parent.mkdir(parents=True)
            report_path.write_bytes(JUNIT)
            receipt_path = repo / "tmp/quality/runtime-script-results" / f"{producer}.json"
            receipt = result.make_receipt(
                producer,
                identity,
                selected_epoch,
                [(report_relative, JUNIT)],
                0,
            )
            result.commit_receipt(repo, producer, identity, selected_epoch, receipt_path, receipt)
            self.assertEqual(
                result_verify.verify_receipt(repo, producer, identity, selected_epoch, receipt_path)["producer"],
                producer,
            )
            report_path.write_bytes(JUNIT + b" ")
            with self.assertRaises(result.RuntimeScriptResultError):
                result_verify.verify_receipt(repo, producer, identity, selected_epoch, receipt_path)
            report_path.write_bytes(JUNIT)
            receipt_path.write_bytes(receipt + b" ")
            with self.assertRaises(result.RuntimeScriptResultError):
                result_verify.verify_receipt(repo, producer, identity, epoch or "", receipt_path)

    def test_fresh_run_invalidates_prior_success_epoch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            subprocess.run(["git", "init", "--quiet"], cwd=repo, check=True)
            producer = IDENTITY_PARTS[0]
            identity = result.build_identity(*IDENTITY_PARTS)
            first_epoch = result.begin_fresh_run(repo, producer, identity)
            self.assertIsNotNone(first_epoch)
            selected_first = first_epoch or ""
            first_relative = current_junit_path(producer, 1)
            first_report_path = repo / first_relative
            first_report_path.parent.mkdir(parents=True)
            first_report_path.write_bytes(JUNIT)
            receipt_path = repo / "tmp/quality/runtime-script-results" / f"{producer}.json"
            receipt = result.make_receipt(
                producer,
                identity,
                selected_first,
                [(first_relative, JUNIT)],
                0,
            )
            result.commit_receipt(
                repo, producer, identity, selected_first, receipt_path, receipt
            )
            self.assertIsNotNone(result_verify.verify_receipt(
                repo, producer, identity, selected_first, receipt_path
            ))
            second_epoch = result.begin_fresh_run(repo, producer, identity)
            self.assertIsNotNone(second_epoch)
            self.assertNotEqual(first_epoch, second_epoch)
            with self.assertRaises(result.RuntimeScriptResultError):
                result_verify.verify_receipt(repo, producer, identity, selected_first, receipt_path)

    def test_report_plan_rejects_stale_missing_and_failed_rerun_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            subprocess.run(["git", "init", "--quiet"], cwd=repo, check=True)
            producer = IDENTITY_PARTS[0]
            first = junit_outputs.plan_reports(repo, producer, EPOCH, 1)
            output = repo / first.paths[0]
            output.write_bytes(JUNIT)
            self.assertEqual(junit_outputs.collect_reports(repo, first, 0), [(first.paths[0], JUNIT)])

            with self.assertRaises(junit.RuntimeScriptJUnitError):
                junit_outputs.plan_reports(repo, producer, EPOCH, 1)
            with self.assertRaises(junit.RuntimeScriptJUnitError):
                junit_outputs.collect_reports(repo, first, 1)

            next_epoch = "f" * 64
            second = junit_outputs.plan_reports(repo, producer, next_epoch, 1)
            with self.assertRaises(junit.RuntimeScriptJUnitError):
                junit_outputs.collect_reports(repo, second, 0)


if __name__ == "__main__":
    unittest.main()
