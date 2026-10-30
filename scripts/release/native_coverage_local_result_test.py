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

import native_coverage_local_anchor as local_anchor
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
        self.coverage_input_digest = "f" * 64
        self.source_state = {
            "schema": local_result.SOURCE_INPUTS_SCHEMA,
            "sourceDigest": self.source_digest,
            "coverageInputDigest": self.coverage_input_digest,
        }
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

    def test_local_context_accepts_nextest_tool_identity(self) -> None:
        self.assertIn("cargoNextest", local_result.LOCAL_TOOL_KEYS)
        self.assertIn("cargoNextest", cast(dict[str, str], self.context["tools"]))

    def prepare_reports(self) -> None:
        for domain in ("root", "baseline", "candidate"):
            fixture_helpers.write_fixture_report(self.root, domain)

    def test_source_inputs_failure_preserves_exit_and_stderr(self) -> None:
        failed = subprocess.CompletedProcess(
            args=["bun", "run", "native-source-manifest.ts", "--coverage-inputs"],
            returncode=17,
            stdout="",
            stderr="manifest verification detail",
        )
        with (
            patch.object(subprocess, "run", return_value=failed),
            self.assertRaisesRegex(
                local_result.LocalCoverageError,
                r"exit 17: manifest verification detail",
            ),
        ):
            local_result._source_state(self.root)

    def test_source_inputs_require_exact_schema_and_sha256_fields(self) -> None:
        malformed = subprocess.CompletedProcess(
            args=["bun", "run", "native-source-manifest.ts", "--coverage-inputs"],
            returncode=0,
            stdout=json.dumps({
                "schema": local_result.SOURCE_INPUTS_SCHEMA,
                "sourceDigest": self.source_digest,
                "coverageInputDigest": "short",
            }),
            stderr="",
        )
        with (
            patch.object(subprocess, "run", return_value=malformed) as run,
            self.assertRaisesRegex(local_result.LocalCoverageError, "invalid shape"),
        ):
            local_result._source_state(self.root)
        self.assertEqual(run.call_args.args[0][-1], "--coverage-inputs")

    def test_full_source_digest_uses_default_manifest_mode(self) -> None:
        self.assertFalse(
            (self.root / "docs/quality-gate-improvement-tasks.md").exists()
        )
        manifest = {
            "schema": "krr-native-build-source-manifest-v1",
            "digest": self.source_digest,
            "files": [{"path": "src/lib.rs", "sha256": "b" * 64}],
        }
        completed = subprocess.CompletedProcess(
            args=["bun", "run", "native-source-manifest.ts", "--root", str(self.root)],
            returncode=0,
            stdout=json.dumps(manifest),
            stderr="",
        )
        with patch.object(subprocess, "run", return_value=completed) as run:
            self.assertEqual(local_result._source_digest(self.root), self.source_digest)
        self.assertNotIn("--coverage-inputs", run.call_args.args[0])

    def test_full_source_digest_rejects_malformed_manifest(self) -> None:
        valid = {
            "schema": "krr-native-build-source-manifest-v1",
            "digest": self.source_digest,
            "files": [{"path": "src/lib.rs", "sha256": "b" * 64}],
        }
        malformed_manifests = (
            {**valid, "schema": "wrong-schema"},
            {**valid, "digest": "short"},
            {**valid, "files": []},
            {**valid, "unexpected": True},
        )
        for manifest in malformed_manifests:
            with self.subTest(manifest=manifest):
                completed = subprocess.CompletedProcess(
                    args=[], returncode=0, stdout=json.dumps(manifest), stderr=""
                )
                with patch.object(subprocess, "run", return_value=completed):
                    with self.assertRaisesRegex(
                        local_result.LocalCoverageError, "invalid shape"
                    ):
                        local_result._source_digest(self.root)

    def test_identity_uses_coverage_inputs_but_not_full_source_digest(self) -> None:
        current_source = [self.source_state]
        with (
            patch.object(
                local_result, "_source_state", side_effect=lambda _: current_source[0]
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            initial = local_result.local_key(self.root, "root")
            current_source[0] = {**self.source_state, "sourceDigest": "d" * 64}
            ledger_only = local_result.local_key(self.root, "root")
            self.assertEqual(initial["fingerprint"], ledger_only["fingerprint"])
            self.assertEqual(ledger_only["coverageInputDigest"], self.coverage_input_digest)
            self.assertNotIn("sourceDigest", ledger_only)

            current_source[0] = {
                **current_source[0],
                "sourceDigest": "e" * 64,
                "coverageInputDigest": "9" * 64,
            }
            changed_source = local_result.local_key(self.root, "root")
            self.assertNotEqual(initial["fingerprint"], changed_source["fingerprint"])

    def test_ledger_change_during_producer_rejects_finish(self) -> None:
        self.prepare_reports()
        current_source = [self.source_state]
        with (
            patch.object(
                local_result, "_source_state", side_effect=lambda _: current_source[0]
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
            patch.dict(os.environ, {"MOON_TASK_ID": "krr:unit-test"}, clear=True),
        ):
            self.begin_producer("root")
            current_source[0] = {**self.source_state, "sourceDigest": "d" * 64}
            with self.assertRaisesRegex(
                local_result.LocalCoverageError, "inputs changed between begin and finish"
            ):
                local_result.local_finish(self.root, "root", 0)

    def test_cached_receipt_survives_later_ledger_only_change(self) -> None:
        self.prepare_reports()
        current_source = [self.source_state]
        with (
            patch.object(
                local_result, "_source_state", side_effect=lambda _: current_source[0]
            ),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            local_result.local_finish(self.root, "root", 0)
            receipt_path = (
                self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
            )
            receipt_bytes = receipt_path.read_bytes()
            current_source[0] = {**self.source_state, "sourceDigest": "d" * 64}
            self.assertTrue(local_result.local_validate(self.root, "root")["valid"])
            self.assertTrue(
                local_result.local_collection_validate(self.root, "root")["valid"]
            )
            self.assertEqual(receipt_path.read_bytes(), receipt_bytes)

    def begin_producer(self, producer: str) -> None:
        local_result.local_key(self.root, producer)
        local_result.local_begin(self.root, producer)

    def test_root_receipt_binds_one_domain_and_strict_report_bytes(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_state", side_effect=lambda _: self.source_state
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
                local_result, "_source_state", return_value=self.source_state
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

    def test_performance_begin_revokes_old_receipt_until_new_collection_succeeds(self) -> None:
        self.prepare_reports()
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            initial_key = local_result.local_key(self.root, "performance")
            local_result.local_begin(self.root, "performance")
            local_result.local_finish(self.root, "performance", 0)
            self.assertTrue(
                local_result.local_collection_validate(self.root, "performance")["valid"]
            )
            self.assertEqual(
                local_result.local_key(self.root, "performance")["fingerprint"],
                initial_key["fingerprint"],
            )

            receipt_path = (
                self.root / coverage_result.REPORT_DIRECTORY / "performance-local-result.json"
            )
            old_receipt = receipt_path.read_bytes()
            anchor_directory = local_anchor._anchor_directory(self.root, create=False)
            self.assertIsNotNone(anchor_directory)
            previous_anchor = local_anchor._read(cast(Path, anchor_directory), "performance")
            if previous_anchor is None:
                self.fail("performance anchor is missing")

            local_result.local_begin(self.root, "performance")
            pending_anchor = local_anchor._read(cast(Path, anchor_directory), "performance")
            if pending_anchor is None:
                self.fail("performance anchor disappeared after begin")
            self.assertEqual(pending_anchor["state"], "pending")
            self.assertEqual(pending_anchor["epoch"], previous_anchor["epoch"])
            self.assertEqual(
                local_result.local_key(self.root, "performance")["fingerprint"],
                initial_key["fingerprint"],
            )
            with self.assertRaisesRegex(local_result.LocalCoverageError, "successfully"):
                local_result.local_finish(self.root, "performance", 1)
            with self.assertRaisesRegex(local_result.LocalCoverageError, "anchor"):
                local_result.local_collection_validate(self.root, "performance")
            self.assertEqual(receipt_path.read_bytes(), old_receipt)
            refreshed_key = local_result.local_key(self.root, "performance")
            self.assertNotEqual(refreshed_key["fingerprint"], initial_key["fingerprint"])

            local_result.local_begin(self.root, "performance")
            local_result.local_finish(self.root, "performance", 0)
            self.assertTrue(
                local_result.local_collection_validate(self.root, "performance")["valid"]
            )
            self.assertEqual(
                local_result.local_key(self.root, "performance")["fingerprint"],
                refreshed_key["fingerprint"],
            )

    def test_performance_begin_anchor_write_failure_stops_before_run_start(self) -> None:
        self.prepare_reports()
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            local_result.local_key(self.root, "performance")
            local_result.local_begin(self.root, "performance")
            local_result.local_finish(self.root, "performance", 0)
            run_path = self.root / coverage_result.REPORT_DIRECTORY / "performance-local-run.json"
            previous_run = run_path.read_bytes()

            with patch.object(local_anchor, "_write", return_value=False):
                with self.assertRaisesRegex(local_result.LocalCoverageError, "marked pending"):
                    local_result.local_begin(self.root, "performance")
            self.assertEqual(run_path.read_bytes(), previous_run)

    def test_changed_source_context_or_report_bytes_reject(self) -> None:
        self.prepare_reports()
        current_source = [self.source_state]
        current_context = [self.context]
        with (
            patch.object(
                local_result, "_source_state", side_effect=lambda _: current_source[0]
            ),
            patch.object(
                local_result, "_local_context", side_effect=lambda _: current_context[0]
            ),
            patch.dict(os.environ, {"MOON_TASK_ID": "krr:unit-test"}, clear=True),
        ):
            local_result.local_key(self.root, "root")
            current_source[0] = {
                **self.source_state,
                "sourceDigest": "d" * 64,
                "coverageInputDigest": "e" * 64,
            }
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_begin(self.root, "root")
            current_source[0] = self.source_state
            self.begin_producer("root")
            current_source[0] = {
                **self.source_state,
                "sourceDigest": "d" * 64,
                "coverageInputDigest": "e" * 64,
            }
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_finish(self.root, "root", 0)
            current_source[0] = self.source_state
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
                local_result.local_collection_validate(self.root, "root")

    def test_standalone_just_begin_can_start_without_a_moon_key_probe(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_state", return_value=self.source_state
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
                local_result, "_source_state", return_value=self.source_state
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
                local_result, "_source_state", return_value=self.source_state
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
        receipt_path = self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
        self.assertTrue(receipt_path.exists())
        self.assertEqual(json.loads(receipt_path.read_bytes())["role"], "collection")

    def test_collection_receipt_accepts_partial_and_strict_wrapper_rejects(self) -> None:
        fixture_helpers.write_fixture_report(self.root, "root", count=10, covered=9, percent=90.0)
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            collection = local_result.local_collect_finish(self.root, "root", 0)
            self.assertTrue(collection["created"])
            self.assertTrue(local_result.local_collection_validate(self.root, "root")["valid"])
            with self.assertRaisesRegex(coverage_result.CoverageResultError, "100/0"):
                local_result.local_validate(self.root, "root")
            anchor_directory = local_anchor._anchor_directory(self.root, create=False)
            self.assertIsNotNone(anchor_directory)
            anchor = local_anchor._read(cast(Path, anchor_directory), "root")
            if anchor is None:
                self.fail("collection anchor is missing")
            self.assertEqual(anchor["state"], "valid")
        receipt = json.loads(
            (self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json").read_bytes()
        )
        self.assertEqual(receipt["role"], "collection")

    def test_collection_validation_rejects_malformed_report(self) -> None:
        self.prepare_reports()
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            local_result.local_collect_finish(self.root, "root", 0)
            (self.root / coverage_result.REPORT_DIRECTORY / "root.json").write_text("{bad\n")
            with self.assertRaises(coverage_result.CoverageResultError):
                local_result.local_collection_validate(self.root, "root")
            anchor_directory = local_anchor._anchor_directory(self.root, create=False)
            self.assertIsNotNone(anchor_directory)
            anchor = local_anchor._read(cast(Path, anchor_directory), "root")
            if anchor is None:
                self.fail("collection anchor is missing")
            self.assertEqual(anchor["state"], "pending")

    def test_strict_policy_uses_rows_authenticated_before_report_mutation(self) -> None:
        fixture_helpers.write_fixture_report(self.root, "root", count=10, covered=9, percent=90.0)
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context) as context_mock,
        ):
            self.begin_producer("root")
            local_result.local_collect_finish(self.root, "root", 0)
            context_before_validate = context_mock.call_count
            original = local_result._collection_report_records

            def rows_then_mutate(repo: Path, producer: str) -> list[dict[str, object]]:
                rows = original(repo, producer)
                fixture_helpers.write_fixture_report(repo, "root", count=10, covered=10)
                return rows

            with (
                patch.object(local_result, "_collection_report_records", side_effect=rows_then_mutate) as rows_mock,
                patch.object(local_result, "_bound_identity", wraps=local_result._bound_identity) as identity_mock,
            ):
                with self.assertRaises(coverage_result.CoveragePolicyError):
                    local_result.local_validate(self.root, "root")
            self.assertEqual(identity_mock.call_count, 1)
            self.assertEqual(rows_mock.call_count, 1)
            self.assertEqual(context_mock.call_count, context_before_validate + 1)

    def test_collection_receipt_symlink_is_rejected(self) -> None:
        self.prepare_reports()
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            local_result.local_collect_finish(self.root, "root", 0)
            receipt = self.root / coverage_result.REPORT_DIRECTORY / "root-local-result.json"
            target = self.root / "receipt-target.json"
            target.write_text(receipt.read_text(encoding="utf-8"), encoding="utf-8")
            receipt.unlink()
            receipt.symlink_to(target)
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_validate(self.root, "root")

    def test_missing_performance_report_never_creates_receipt(self) -> None:
        self.prepare_reports()
        candidate = self.root / coverage_result.REPORT_DIRECTORY / "candidate.json"
        with (
            patch.object(
                local_result, "_source_state", return_value=self.source_state
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
                local_result, "_source_state", return_value=self.source_state
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
            recorded_threads = local_result._threads()
            changed_threads = "1" if recorded_threads != "1" else "2"
            with patch.dict(os.environ, {"TEST_THREADS": changed_threads}):
                self.assertNotEqual(local_result._threads(), recorded_threads)
                with self.assertRaises(local_result.LocalCoverageError):
                    local_result.local_finish(self.root, "root", 0)
            self.begin_producer("root")
            with patch.dict(os.environ, {"CARGO": "cargo-local-override"}):
                with self.assertRaises(local_result.LocalCoverageError):
                    local_result.local_finish(self.root, "root", 0)

    def test_build_jobs_environment_change_rejects_collection_finish(self) -> None:
        self.prepare_reports()
        with (
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            self.begin_producer("root")
            recorded_jobs = os.environ.get("CARGO_BUILD_JOBS", "")
            changed_jobs = "1" if recorded_jobs != "1" else "2"
            with patch.dict(os.environ, {"CARGO_BUILD_JOBS": changed_jobs}):
                with self.assertRaises(local_result.LocalCoverageError):
                    local_result.local_finish(self.root, "root", 0)

    def test_malformed_report_and_tampered_receipt_reject_validation(self) -> None:
        self.prepare_reports()
        with (
            patch.object(
                local_result, "_source_state", return_value=self.source_state
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
            patch.object(local_result, "_source_state", return_value=self.source_state),
            patch.object(local_result, "_local_context", return_value=self.context),
        ):
            for producer in ("root", "performance"):
                with self.subTest(producer=producer):
                    first = local_result.local_key(self.root, producer)
                    local_result.local_begin(self.root, producer)
                    local_result.local_finish(self.root, producer, 0)
                    self.assertTrue(local_result.local_validate(self.root, producer)["valid"])
                    second = local_result.local_key(self.root, producer)
                    self.assertNotEqual(first["fingerprint"], second["fingerprint"])
                    with self.assertRaises(local_result.LocalCoverageError):
                        local_result.local_validate(self.root, producer)

    def test_unknown_producer_and_malformed_context_reject(self) -> None:
        with self.assertRaises(local_result.LocalCoverageError):
            local_result.local_key(self.root, "all")
        bad_context = dict(self.context)
        bad_context["unexpected"] = "value"
        fake_module = types.SimpleNamespace(collect_local_context=lambda _: bad_context)
        with (
            patch.object(
                local_result, "_source_state", return_value=self.source_state
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
