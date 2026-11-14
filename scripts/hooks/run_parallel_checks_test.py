from __future__ import annotations

from contextlib import ExitStack, redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

import run_parallel_checks as bridge
from local_review_lock import review_lock
from local_review_state import ReviewError, strict_json
from run_parallel_checks import main, run_checks


ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_DIR = ROOT / "tmp/quality/moon-terminal-runner-implementation-r1"
HEAD = "a" * 40
EXPECTED_AUTHORITY_PATHS = frozenset(
    {
        ".moon/workspace.yml",
        "moon.yml",
        "Justfile",
        "package.json",
        "bun.lock",
        "scripts/html/harness-contract-cli.ts",
        "scripts/html/harness-contract-graph.ts",
        "scripts/html/native-coverage-cache-contract.ts",
        "scripts/html/moon-quality-inputs.ts",
        "scripts/html/harness-contract-policy.json",
        "scripts/hooks/run_parallel_checks.py",
        "scripts/release/native_coverage_local_aggregate.py",
        "scripts/release/native_coverage_local_anchor.py",
        "scripts/release/native_coverage_local_context.py",
        "scripts/release/native_coverage_local_result.py",
        "scripts/release/native_coverage_linux_context.py",
        "scripts/release/native_coverage_result.py",
        "scripts/release/native_coverage_result_cli.py",
        "scripts/mermaid/native-source-manifest.ts",
        "scripts/mermaid/mermaid-pair-manifest.ts",
        "scripts/mermaid/native-build-receipt.ts",
        "scripts/mermaid/native-cli-managed-build.ts",
        "scripts/mermaid/native-cli-managed-build-schema.ts",
        "scripts/mermaid/native-cli-managed-json.ts",
        "scripts/html/moon-authority-inputs.json",
    }
)

EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)


def success_proof(request: dict[str, object]) -> str:
    proof = {
        "schema": 1,
        "status": "verified",
        "reportSha256": request["reportSha256"],
        "inputSha256": request["inputSha256Before"],
        "head": request["headBefore"],
        "startedNs": request["startedNs"],
        "finishedNs": request["finishedNs"],
        "reportMtimeNs": request["reportMtimeNs"],
    }
    return json.dumps(proof)


class MoonBridgeTest(unittest.TestCase):
    def run_isolated(
        self,
        *,
        report_text: str | None = '{"status":"completed"}',
        prior_report: str | None = None,
        preflight_return: int = 0,
        preflight_text: str | None = None,
        moon_return: int = 0,
        verifier_return: int = 0,
        verifier_stdout: str | None = None,
        authority_hashes: list[dict[str, str]] | None = None,
        head_values: list[str] | None = None,
        duplicate_preflight: bool = False,
        lock_contention: bool = False,
    ) -> tuple[
        int,
        list[tuple[list[str], dict[str, object]]],
        dict[str, object] | None,
        list[tuple[list[str], dict[str, object]]],
    ]:
        with tempfile.TemporaryDirectory(dir=EVIDENCE_DIR) as temporary:
            temp_root = Path(temporary)
            report_path = temp_root / "runReport.json"
            lock_path = temp_root / "krr-terminal-report.lock"
            report_relative = report_path.relative_to(ROOT)
            lock_relative = lock_path.relative_to(ROOT)
            if prior_report is not None:
                report_path.write_text(prior_report, encoding="utf-8")
                old_ns = time.time_ns() - 1_000_000_000
                os.utime(report_path, ns=(old_ns, old_ns))

            actual_hashes = bridge._authority_sha256(ROOT)
            if preflight_text is None:
                preflight_text = json.dumps(
                    {
                        "schema": 1,
                        "staticContractOnly": True,
                        "errors": [],
                        "moonRunContract": {"inputSha256": actual_hashes},
                    }
                )
            if duplicate_preflight:
                preflight_text = preflight_text.replace(
                    '"schema": 1,', '"schema": 1, "schema": 1,', 1
                )
            calls: list[tuple[list[str], dict[str, object]]] = []
            received_request: dict[str, object] | None = None

            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                nonlocal received_request
                calls.append((arguments, kwargs))
                if arguments == ["bun", "scripts/html/harness-contract-cli.ts"]:
                    return subprocess.CompletedProcess(
                        arguments,
                        preflight_return,
                        stdout=preflight_text,
                        stderr="",
                    )
                if arguments[:3] == ["bun", "run", "moon"]:
                    if report_text is not None and moon_return == 0:
                        report_path.write_text(report_text, encoding="utf-8")
                        now_ns = time.time_ns()
                        os.utime(report_path, ns=(now_ns, now_ns))
                    return subprocess.CompletedProcess(arguments, moon_return, stdout="", stderr="")
                if arguments == bridge.VERIFY_COMMAND:
                    raw_request = kwargs.get("input")
                    if not isinstance(raw_request, str):
                        self.fail("verification request was not text")
                    parsed = json.loads(raw_request)
                    if not isinstance(parsed, dict):
                        self.fail("verification request was not an object")
                    received_request = parsed
                    proof_text = verifier_stdout if verifier_stdout is not None else success_proof(parsed)
                    return subprocess.CompletedProcess(
                        arguments,
                        verifier_return,
                        stdout=proof_text,
                        stderr="",
                    )
                self.fail(f"unexpected subprocess command: {arguments!r}")

            with ExitStack() as stack:
                stack.enter_context(mock.patch.object(bridge, "REPORT_RELATIVE_PATH", report_relative))
                stack.enter_context(mock.patch.object(bridge, "LOCK_RELATIVE_PATH", lock_relative))
                if authority_hashes is not None:
                    stack.enter_context(
                        mock.patch.object(bridge, "_authority_sha256", side_effect=authority_hashes)
                    )
                stack.enter_context(
                    mock.patch.object(
                        bridge,
                        "_head",
                        side_effect=head_values if head_values is not None else [HEAD] * 4,
                    )
                )
                if lock_contention:
                    stack.enter_context(
                        mock.patch(
                            "run_parallel_checks.review_lock",
                            side_effect=ReviewError(
                                "another local review is active; do not start recursive reviews"
                            ),
                        )
                    )
                stack.enter_context(mock.patch("subprocess.run", side_effect=fake_run))
                with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    result = run_checks(4)
            return result, calls, received_request, calls

    def test_runs_fixed_moon_command_and_consumes_bound_verification_proof(self) -> None:
        result, calls, request, actual_calls = self.run_isolated()
        self.assertEqual(result, 0)
        self.assertEqual(len(calls), 3)
        self.assertIsNotNone(request)
        assert request is not None
        self.assertEqual(
            set(request),
            {
                "schema",
                "preflightText",
                "reportText",
                "reportSha256",
                "inputSha256Before",
                "inputSha256After",
                "headBefore",
                "headAfter",
                "startedNs",
                "finishedNs",
                "reportMtimeNs",
            },
        )
        self.assertEqual(request["schema"], 1)
        self.assertEqual(request["headBefore"], HEAD)
        self.assertEqual(request["headAfter"], HEAD)
        self.assertEqual(request["inputSha256Before"], bridge._authority_sha256(ROOT))
        self.assertEqual(request["inputSha256After"], request["inputSha256Before"])
        for key in ("startedNs", "finishedNs", "reportMtimeNs"):
            self.assertIsInstance(request[key], str)
            self.assertTrue(str(request[key]).isdecimal())
        self.assertEqual(actual_calls[0][0], ["bun", "scripts/html/harness-contract-cli.ts"])
        self.assertEqual(
            actual_calls[1][0],
            [
                "bun",
                "run",
                "moon",
                "--concurrency",
                "4",
                "exec",
                "krr:coverage",
                "--on-failure",
                "bail",
                "--no-actions",
            ],
        )
        self.assertEqual(actual_calls[2][0], bridge.VERIFY_COMMAND)
        self.assertFalse(actual_calls[0][1]["shell"])
        self.assertFalse(actual_calls[1][1]["shell"])
        self.assertFalse(actual_calls[2][1]["shell"])

    def test_preflight_nonzero_code_stops_before_moon(self) -> None:
        result, calls, request, actual_calls = self.run_isolated(preflight_return=37)
        self.assertEqual(result, 37)
        self.assertEqual(len(calls), 1)
        self.assertIsNone(request)
        self.assertEqual(actual_calls[0][0], ["bun", "scripts/html/harness-contract-cli.ts"])

    def test_preflight_duplicate_json_is_rejected_before_moon(self) -> None:
        result, calls, request, _ = self.run_isolated(duplicate_preflight=True)
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 1)
        self.assertIsNone(request)

    def test_runner_requires_repository_root_working_directory(self) -> None:
        with (
            mock.patch.object(Path, "cwd", return_value=ROOT.parent),
            mock.patch("subprocess.run") as run,
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(run_checks(1), 1)
        run.assert_not_called()

    def test_preflight_wrong_authority_and_invalid_envelope_are_rejected(self) -> None:
        wrong = dict(bridge._authority_sha256(ROOT))
        wrong[next(iter(wrong))] = "0" * 64
        result, calls, _request, _ = self.run_isolated(authority_hashes=[wrong])
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 1)

    def test_current_typescript_preflight_authority_registry_is_consumed_exactly(self) -> None:
        producer_expression = (
            'import { moonRunContract } from "./scripts/html/harness-contract-cli.ts"; '
            'console.log(JSON.stringify({schema: 1, staticContractOnly: true, '
            'errors: [], moonRunContract: moonRunContract(process.cwd())}));'
        )
        preflight = subprocess.run(
            ["bun", "-e", producer_expression],
            check=False,
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(preflight.returncode, 0, preflight.stderr)
        envelope = strict_json(preflight.stdout)
        self.assertEqual(
            set(envelope["moonRunContract"]["inputSha256"]), EXPECTED_AUTHORITY_PATHS
        )
        result, calls, request, _ = self.run_isolated(preflight_text=preflight.stdout)
        self.assertEqual(result, 0)
        self.assertEqual(len(calls), 3)
        self.assertIsNotNone(request)

    def test_authority_registry_rejects_duplicate_empty_and_unsafe_inputs(self) -> None:
        invalid_registries = [
            '{"schema":1,"inputs":{"scripts/html/moon-authority-inputs.json":true,"scripts/html/moon-authority-inputs.json":true}}',
            '{"schema":1,"inputs":{}}',
            '{"schema":1,"inputs":{"../outside":true}}',
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            registry = root / bridge.AUTHORITY_REGISTRY_PATH
            registry.parent.mkdir(parents=True)
            for raw in invalid_registries:
                registry.write_text(raw, encoding="utf-8")
                with self.subTest(raw=raw), self.assertRaises(ReviewError):
                    bridge._authority_paths(root)

    def test_moon_nonzero_exit_is_forwarded_without_verification(self) -> None:
        result, calls, request, actual_calls = self.run_isolated(moon_return=23)
        self.assertEqual(result, 23)
        self.assertEqual(len(calls), 2)
        self.assertIsNone(request)
        self.assertEqual(actual_calls[1][0][-1], "--no-actions")

    def test_missing_report_is_rejected_after_zero_exit(self) -> None:
        result, calls, request, _ = self.run_isolated(report_text=None)
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 2)
        self.assertIsNone(request)

    def test_malformed_and_duplicate_report_json_are_rejected(self) -> None:
        for report_text in ("{", '{"status":"completed","status":"interrupted"}'):
            with self.subTest(report_text=report_text):
                result, calls, request, _ = self.run_isolated(report_text=report_text)
                self.assertEqual(result, 1)
                self.assertEqual(len(calls), 2)
                self.assertIsNone(request)

    def test_unchanged_old_success_report_is_rejected(self) -> None:
        old_report = '{"status":"completed"}'
        result, calls, request, _ = self.run_isolated(
            report_text=old_report,
            prior_report=old_report,
        )
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 2)
        self.assertIsNone(request)

    def test_verifier_nonzero_and_mismatched_proof_are_not_success(self) -> None:
        failed, calls, _request, _ = self.run_isolated(verifier_return=31)
        self.assertEqual(failed, 31)
        self.assertEqual(len(calls), 3)

        wrong_proof = json.dumps(
            {
                "schema": 1,
                "status": "verified",
                "reportSha256": "0" * 64,
                "inputSha256": bridge._authority_sha256(ROOT),
                "head": HEAD,
                "startedNs": "1",
                "finishedNs": "2",
                "reportMtimeNs": "2",
            }
        )
        result, calls, _request, _ = self.run_isolated(verifier_stdout=wrong_proof)
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 3)

        input_sha256 = bridge._authority_sha256(ROOT)
        request: dict[str, object] = {
            "reportSha256": "1" * 64,
            "inputSha256Before": input_sha256,
            "headBefore": HEAD,
            "startedNs": "10",
            "finishedNs": "20",
            "reportMtimeNs": "15",
        }
        boolean_schema_proof = json.loads(success_proof(request))
        boolean_schema_proof["schema"] = True
        with self.assertRaises(ReviewError):
            bridge._validate_success_proof(
                json.dumps(boolean_schema_proof),
                str(request["reportSha256"]),
                input_sha256,
                str(request["headBefore"]),
                10,
                20,
                15,
            )

    def test_authority_or_head_change_during_moon_or_verification_is_rejected(self) -> None:
        current = bridge._authority_sha256(ROOT)
        changed = dict(current)
        changed[next(iter(current))] = "0" * 64
        result, calls, request, _ = self.run_isolated(authority_hashes=[current, changed])
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 2)
        self.assertIsNone(request)

        result, calls, request, _ = self.run_isolated(
            authority_hashes=[current, current, current, changed]
        )
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 3)
        self.assertIsNotNone(request)

        result, calls, request, _ = self.run_isolated(head_values=[HEAD, "b" * 40])
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 2)
        self.assertIsNone(request)

    def test_lock_contention_prevents_moon_start(self) -> None:
        result, calls, request, _ = self.run_isolated(lock_contention=True)
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 1)
        self.assertIsNone(request)

    def test_r22_interrupted_report_excerpt_requires_verifier_rejection(self) -> None:
        r22_report = json.dumps(
            {
                "status": "interrupted",
                "context": {
                    "affected": None,
                    "initialTargets": ["krr:fmt-check", "krr:source-static"],
                    "primaryTargets": ["krr:source-static", "krr:fmt-check"],
                    "passthroughArgs": [],
                    "targetStates": {"krr:duplication-check": {"state": "skipped"}},
                },
                "actions": [
                    {"node": {"action": "run-task", "params": {"target": target}}, "status": "skipped"}
                    for target in (
                        "krr:duplication-check",
                        "krr:html-harness-contract-check",
                        "krr:owned-validation-contract-check",
                        "krr:runtime-asset-check",
                        "krr:runtime-bundle-check",
                        "krr:biome",
                        "krr:python-lint",
                        "krr:python-typecheck",
                    )
                ],
            }
        )
        result, calls, request, _ = self.run_isolated(
            report_text=r22_report,
            verifier_return=1,
        )
        self.assertEqual(result, 1)
        self.assertEqual(len(calls), 3)
        self.assertIsNotNone(request)
        assert request is not None
        self.assertIn('"status": "interrupted"', str(request["reportText"]))

    def test_moon_interruption_propagates_and_lock_releases(self) -> None:
        with tempfile.TemporaryDirectory(dir=EVIDENCE_DIR) as temporary:
            temp_root = Path(temporary)
            lock_path = temp_root / "krr-terminal-report.lock"
            lock_relative = lock_path.relative_to(ROOT)
            report_relative = (temp_root / "runReport.json").relative_to(ROOT)
            preflight = subprocess.CompletedProcess(
                args=[], returncode=0, stdout=json.dumps({
                    "schema": 1,
                    "staticContractOnly": True,
                    "errors": [],
                    "moonRunContract": {"inputSha256": bridge._authority_sha256(ROOT)},
                }), stderr=""
            )
            with (
                mock.patch.object(bridge, "REPORT_RELATIVE_PATH", report_relative),
                mock.patch.object(bridge, "LOCK_RELATIVE_PATH", lock_relative),
                mock.patch.object(bridge, "_head", return_value=HEAD),
                mock.patch(
                    "subprocess.run",
                    side_effect=[preflight, KeyboardInterrupt],
                ) as run,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    with self.assertRaises(KeyboardInterrupt):
                        run_checks(1)
            with review_lock(lock_path):
                pass
            self.assertEqual(run.call_count, 2)

    def test_rejects_nonpositive_job_counts_before_starting_bridge(self) -> None:
        with mock.patch("subprocess.run") as run:
            for jobs in (0, -1):
                with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                    run_checks(jobs)
            run.assert_not_called()

    def test_cli_passes_jobs_to_the_bridge(self) -> None:
        with (
            mock.patch.object(sys, "argv", ["run_parallel_checks.py", "--jobs", "5"]),
            mock.patch("run_parallel_checks.run_checks", return_value=19) as check,
        ):
            self.assertEqual(main(), 19)
        check.assert_called_once_with(5)

    def test_cli_rejects_nonpositive_jobs_before_starting_the_bridge(self) -> None:
        with (
            mock.patch.object(sys, "argv", ["run_parallel_checks.py", "--jobs", "0"]),
            mock.patch("run_parallel_checks.run_checks") as check,
        ):
            with self.assertRaises(SystemExit) as raised:
                main()
        self.assertEqual(raised.exception.code, 2)
        check.assert_not_called()


if __name__ == "__main__":
    unittest.main()
