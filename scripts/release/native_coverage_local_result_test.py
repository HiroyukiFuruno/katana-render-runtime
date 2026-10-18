from __future__ import annotations

import hashlib
import importlib
import json
import os
import subprocess
import tempfile
import types
import unittest
from pathlib import Path
from typing import cast
from unittest.mock import patch

import native_coverage_local_result as local_result
import native_coverage_result as coverage_result
import native_coverage_result_cli as result_cli
import native_coverage_result_test as fixture_helpers


class NativeCoverageLocalResultTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        fixture_helpers.initialize_fixture_repository(self.root)
        self.source_digest = "a" * 64
        self.context = self.make_context(True)

    @staticmethod
    def make_context(cacheable: bool) -> dict[str, object]:
        context: dict[str, object] = {
            "schema": local_result.LOCAL_CONTEXT_SCHEMA,
            "cacheable": cacheable,
            "missReasons": [] if cacheable else ["font_inventory_incomplete"],
            "host": {
                "os": "macos",
                "productVersion": "15",
                "buildVersion": "24",
                "architecture": "arm64",
            },
            "tools": {key: f"{key}-1" for key in local_result.LOCAL_TOOL_KEYS},
            "environmentSha256": "b" * 64,
            "resourcesSha256": "c" * 64,
        }
        context["hostContextSha256"] = local_result._digest(context)
        return context

    def prepare_reports(self) -> None:
        for domain in ("root", "baseline", "candidate"):
            fixture_helpers.write_fixture_report(self.root, domain)

    def test_source_manifest_failure_preserves_exit_and_stderr(self) -> None:
        failed = subprocess.CompletedProcess(
            args=["bun", "run", "native-source-manifest.ts"],
            returncode=17,
            stdout="",
            stderr="manifest verification detail",
        )
        with patch.object(subprocess, "run", return_value=failed):
            with self.assertRaisesRegex(
                local_result.LocalCoverageError,
                r"exit 17: manifest verification detail",
            ):
                local_result._source_digest(self.root)

    def begin_producer(self, producer: str) -> None:
        local_result.local_key(self.root, producer)
        local_result.local_begin(self.root, producer)

    def test_root_receipt_binds_one_domain_and_strict_report_bytes(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", side_effect=lambda _: self.source_digest
            ),
            patch.object(
                local_result, "_local_context", side_effect=lambda _: self.context
            ),
        ):
            key = local_result.local_key(self.root, "root")
            self.assertTrue(key["cacheable"])
            self.assertEqual(
                local_result.local_begin(self.root, "root")["started"], True
            )
            local_result.local_finish(self.root, "root", 0)
            self.assertTrue(local_result.local_validate(self.root, "root")["valid"])
            receipt = json.loads(
                (
                    self.root
                    / coverage_result.REPORT_DIRECTORY
                    / "root-local-result.json"
                ).read_bytes()
            )
        self.assertEqual(
            [
                row["domain"]
                for row in cast(list[dict[str, object]], receipt["domains"])
            ],
            ["root"],
        )
        self.assertEqual(receipt["authenticity"], local_result.AUTHENTICITY)

    def test_performance_receipt_binds_baseline_and_candidate_only(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("performance")
            local_result.local_finish(self.root, "performance", 0)
            local_result.local_validate(self.root, "performance")
        receipt = json.loads(
            (
                self.root
                / coverage_result.REPORT_DIRECTORY
                / "performance-local-result.json"
            ).read_bytes()
        )
        domains = cast(list[dict[str, object]], receipt["domains"])
        self.assertEqual(
            [row["domain"] for row in domains],
            ["performance-baseline", "performance-candidate"],
        )

    def test_changed_source_context_or_report_bytes_reject(self) -> None:
        self.prepare_reports()
        current_source = [self.source_digest]
        current_context = [self.context]
        with (
            patch.object(
                local_result, "_source_digest", side_effect=lambda _: current_source[0]
            ),
            patch.object(
                local_result, "_local_context", side_effect=lambda _: current_context[0]
            ),
            patch.dict(os.environ, {"MOON_TASK_ID": "krr:unit-test"}, clear=True),
        ):
            local_result.local_key(self.root, "root")
            current_source[0] = "d" * 64
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_begin(self.root, "root")
            current_source[0] = self.source_digest
            self.begin_producer("root")
            current_source[0] = "d" * 64
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_finish(self.root, "root", 0)
            current_source[0] = self.source_digest
            self.begin_producer("root")
            current_context[0] = self.make_context(False)
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_finish(self.root, "root", 0)
            current_context[0] = self.context
            self.begin_producer("root")
            local_result.local_finish(self.root, "root", 0)
            report = self.root / coverage_result.REPORT_DIRECTORY / "root.json"
            fixture_helpers.write_fixture_report(
                self.root, "root", count=13, covered=13
            )
            receipt_path = (
                self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
            )
            receipt = cast(dict[str, object], json.loads(receipt_path.read_bytes()))
            domains = cast(list[dict[str, object]], receipt["domains"])
            self.assertNotEqual(
                hashlib.sha256(report.read_bytes()).hexdigest(), domains[0]["sha256"]
            )
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_validate(self.root, "root")

    def test_standalone_just_begin_can_start_without_a_moon_key_probe(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
            patch.dict(os.environ, {}, clear=True),
        ):
            self.assertTrue(local_result.local_begin(self.root, "root")["started"])
            local_result.local_finish(self.root, "root", 0)
            self.assertTrue(local_result.local_validate(self.root, "root")["valid"])

    def test_moon_managed_begin_requires_a_matching_key_probe(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
            patch.dict(os.environ, {"MOON_TASK_ID": "krr:unit-test"}, clear=True),
        ):
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_begin(self.root, "root")
            local_result.local_key(self.root, "root")
            self.assertTrue(local_result.local_begin(self.root, "root")["started"])

    def test_nonzero_exit_and_invalid_coverage_never_create_receipt(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_finish(self.root, "root", 1)
            fixture_helpers.write_fixture_report(
                self.root, "root", count=10, covered=9, percent=90.0
            )
            with self.assertRaises(coverage_result.CoverageResultError):
                local_result.local_finish(self.root, "root", 0)
        self.assertFalse(
            (
                self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
            ).exists()
        )

    def test_missing_performance_report_never_creates_receipt(self) -> None:
        self.prepare_reports()
        candidate = self.root / coverage_result.REPORT_DIRECTORY / "candidate.json"
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("performance")
            candidate.unlink()
            with self.assertRaises(coverage_result.CoverageResultError):
                local_result.local_finish(self.root, "performance", 0)
        self.assertFalse(
            (
                self.root
                / coverage_result.REPORT_DIRECTORY
                / "performance-local-result.json"
            ).exists()
        )

    def test_tool_and_thread_profile_changes_reject_finish(self) -> None:
        self.prepare_reports()
        current_context = [self.context]
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(
                local_result, "_local_context", side_effect=lambda _: current_context[0]
            ),
        ):
            self.begin_producer("root")
            changed = dict(self.context)
            changed["tools"] = {
                **cast(dict[str, str], self.context["tools"]),
                "rustc": "rustc-drift",
            }
            changed.pop("hostContextSha256")
            changed["hostContextSha256"] = local_result._digest(changed)
            current_context[0] = changed
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_finish(self.root, "root", 0)
            current_context[0] = self.context
            self.begin_producer("root")
            with patch.dict(os.environ, {"TEST_THREADS": "2"}):
                with self.assertRaises(local_result.LocalCoverageError):
                    local_result.local_finish(self.root, "root", 0)
            self.begin_producer("root")
            with patch.dict(os.environ, {"CARGO": "cargo-local-override"}):
                with self.assertRaises(local_result.LocalCoverageError):
                    local_result.local_finish(self.root, "root", 0)

    def test_malformed_report_and_tampered_receipt_reject_validation(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            local_result.local_finish(self.root, "root", 0)
            report = self.root / coverage_result.REPORT_DIRECTORY / "root.json"
            report.write_text("{malformed\n")
            with self.assertRaises(coverage_result.CoverageResultError):
                local_result.local_validate(self.root, "root")
            fixture_helpers.write_fixture_report(self.root, "root")
            self.begin_producer("root")
            local_result.local_finish(self.root, "root", 0)
            receipt_path = (
                self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
            )
            receipt = cast(dict[str, object], json.loads(receipt_path.read_bytes()))
            receipt["producerExit"] = 1
            receipt_path.write_text(json.dumps(receipt) + "\n")
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_validate(self.root, "root")

    def test_uncacheable_context_requires_current_per_run_token(self) -> None:
        self.prepare_reports()
        self.context = self.make_context(False)
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            first = local_result.local_key(self.root, "root")
            local_result.local_begin(self.root, "root")
            local_result.local_finish(self.root, "root", 0)
            self.assertTrue(local_result.local_validate(self.root, "root")["valid"])
            second = local_result.local_key(self.root, "root")
            self.assertNotEqual(first["fingerprint"], second["fingerprint"])
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_validate(self.root, "root")

    def test_unknown_producer_and_malformed_context_reject(self) -> None:
        with self.assertRaises(local_result.LocalCoverageError):
            local_result.local_key(self.root, "all")
        bad_context = dict(self.context)
        bad_context["unexpected"] = "value"
        fake_module = types.SimpleNamespace(collect_local_context=lambda _: bad_context)
        with (
            patch.object(
                local_result, "_source_digest", return_value=self.source_digest
            ),
            patch.object(importlib, "import_module", return_value=fake_module),
        ):
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_key(self.root, "root")

    def test_cli_requires_explicit_fingerprint_mode_and_routes_local_producer(
        self,
    ) -> None:
        parser = result_cli._parser()
        with self.assertRaises(SystemExit):
            parser.parse_args(
                ["--repo", str(self.root), "local-key", "--producer", "root"]
            )
        args = parser.parse_args(
            [
                "--repo",
                str(self.root),
                "local-key",
                "--producer",
                "root",
                "--moon-fingerprint",
            ]
        )
        expected = {
            "schema": local_result.KEY_SCHEMA,
            "producer": "root",
            "cacheable": True,
            "fingerprint": "e" * 64,
        }
        with patch.object(local_result, "local_key", return_value=expected):
            self.assertEqual(result_cli._run(args), expected)


if __name__ == "__main__":
    unittest.main()
