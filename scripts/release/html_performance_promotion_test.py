"""HTML性能昇格のpaired方向判定。"""
from __future__ import annotations

import copy
import contextlib
import io
import json
import os
import subprocess
import sys
import unittest
from functools import cache
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import IO
from unittest.mock import Mock, patch

import html_performance_paired_stats as paired_stats
import html_performance_promotion as promotion
import html_performance_python_identity as python_identity
import html_performance_source_reuse_cli as reuse
import run_html_performance as runner
import verify_html_performance_evidence as gate
import verify_html_performance_evidence_test as gate_fixture


class PromotionComparisonTest(unittest.TestCase):
    def test_equal_and_mixed_noise_are_not_improvements(self) -> None:
        baseline = [100.0] * 25
        noise = [99.0, 101.0] * 12 + [100.0]
        self.assertFalse(promotion._compare(baseline, baseline))
        self.assertFalse(promotion._compare(baseline, noise))

    def test_directional_improvement_and_regression_are_distinct(self) -> None:
        baseline = [100.0] * 25
        improved = [90.0] * 25
        self.assertTrue(promotion._compare(baseline, improved))
        with self.assertRaisesRegex(ValueError, "regression"):
            promotion._compare(improved, baseline)

    def test_v2_peak_and_time_use_their_respective_comparison_rules(self) -> None:
        original = paired_stats.evaluate_paired_regression

        @cache
        def paired(baseline: tuple[float, ...], candidate: tuple[float, ...]) -> object:
            return original(baseline, candidate)

        def sample(time: float, rss: float) -> gate.JsonObject:
            phase: gate.JsonObject = {"elapsed_ms": time, "peak_rss_bytes": rss}
            return {"phases": {name: ({"measurement": dict(phase)}
                                      if name in ("post_operation", "same_session_resize")
                                      else dict(phase)) for name in gate.PHASES}}

        bundle = Mock(spec=gate.Bundle)
        bundle.manifest = {"attempts": [{"expected": {}} for _ in range(84)]}
        bundle.json.return_value = {"reportPins": []}
        result: gate.JsonObject = {"candidateConditions": 84, "pairedConditions": 84,
                                   "nonComparableConditions": 0, "performanceNonregression": True,
                                   "memoryPolicyVersion": "html-memory-v2"}
        cases = [(18.0, 102.0, True), (20.0, 90.0, True), (20.0, 102.0, False)]
        for elapsed, rss, improved in cases:
            grouped = {arm: {pair: {"samples": [sample(time, peak)] * 5} for pair in range(5)}
                       for arm, time, peak in (("baseline", 20.0, 100.0),
                                               ("candidate", elapsed, rss))}
            with self.subTest(elapsed=elapsed, rss=rss), \
                 patch.object(gate, "_group_reports", return_value=grouped), \
                 patch.object(paired_stats, "evaluate_paired_regression",
                              side_effect=lambda base, cand: paired(tuple(base), tuple(cand))):
                actual, _ = promotion._measure(bundle, result)
                self.assertEqual(actual, improved)
                if rss > 100:
                    with self.assertRaisesRegex(ValueError, "regression"):
                        promotion._measure(bundle, {key: value for key, value in result.items()
                                                    if key != "memoryPolicyVersion"})
        grouped = {arm: {pair: {"samples": [sample(time, 100.0)] * 5} for pair in range(5)}
                   for arm, time in (("baseline", 20.0), ("candidate", 21.0))}
        with patch.object(gate, "_group_reports", return_value=grouped), \
             patch.object(paired_stats, "evaluate_paired_regression",
                          side_effect=lambda base, cand: paired(tuple(base), tuple(cand))):
            with self.assertRaisesRegex(ValueError, "regression"):
                promotion._measure(bundle, result)


class PromotionMemoryPolicyTest(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = gate_fixture.PerformanceEvidenceTest("runTest")
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.setUp()
        self.policy: gate.JsonObject = {"version": "html-memory-v2", "policyPass": True}
        self.retention = self.fixture.add_file("/captured/memory-retention", "memory/retention.json",
                                               {"retained": "authenticated elsewhere"})
        result = self.fixture.add_file("/captured/memory-result", "memory/result.json",
                                       {"memoryPolicy": self.policy})
        expected = self.fixture.add_file("/captured/memory-expected", "memory/expected.json",
                                         {"memoryRetentionRef": self.retention})
        self.fixture.save_transport()
        self.bundle = gate.Bundle(self.fixture.bundle)
        self.bundle.manifest["attempts"] = [{"result": result, "expected": expected} for _ in range(84)]
        self.summary: gate.JsonObject = {"memoryPolicyVersion": "html-memory-v2",
                                        "retainedMemoryConditions": 84}

    def test_v1_has_no_new_fields_and_v2_binds_policy_and_retention(self) -> None:
        self.assertEqual(promotion._memory_policy_record(self.bundle, {}), {})
        record = promotion._memory_policy_record(self.bundle, self.summary)
        self.assertEqual(record, {"memoryPolicy": {
            "version": "html-memory-v2", "policySha256": gate._v3_json_digest([self.policy] * 84),
            "retentionEvidenceSha256": gate._v3_json_digest([self.retention] * 84),
            "candidateConditions": 84, "acceptedConditions": 84, "warmupCycles": 5,
            "measuredCycles": 25,
        }})
        self.bundle.file(self.retention).write_text("tampered")
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            promotion._memory_policy_record(self.bundle, self.summary)

    def test_v2_rejects_unknown_version_and_incomplete_conditions(self) -> None:
        for value in ({**self.summary, "memoryPolicyVersion": "unknown"},
                      {**self.summary, "retainedMemoryConditions": 83}):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "all retained-memory"):
                promotion._memory_policy_record(self.bundle, value)
        self.bundle.manifest["attempts"] = gate._array(self.bundle.manifest["attempts"], "attempts")[:-1]
        with self.assertRaisesRegex(ValueError, "attempt count"):
            promotion._memory_policy_record(self.bundle, self.summary)

    def test_record_adds_only_v2_memory_metadata_to_existing_fields(self) -> None:
        result: gate.JsonObject = {
            "head": self.bundle.manifest["head"], "checks": 1680,
            "htmlStage90": {"acceptance": True, "fontIdentityEquivalence": "established",
                            "minimumScore": 95, "threshold": 90},
        }
        with patch.object(promotion, "_candidate_pins", return_value={"pin": "fixture"}):
            v1 = promotion._record(self.bundle, result, {}, "run", "a" * 64, "b" * 64,
                                   "c" * 64, "2026-01-01T00:00:00Z")
            v2 = promotion._record(self.bundle, {**result, **self.summary}, {}, "run",
                                   "a" * 64, "b" * 64, "c" * 64, "2026-01-01T00:00:00Z")
        self.assertNotIn("memoryPolicy", v1)
        self.assertEqual({key: value for key, value in v2.items() if key != "memoryPolicy"}, v1)
        self.assertEqual(v2["memoryPolicy"], promotion._memory_policy_record(self.bundle, self.summary)["memoryPolicy"])


class PromotionAtomicityTest(unittest.TestCase):
    def reuse_fixture(self) -> tuple[Path, Path, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        bundle = root / "bundle"
        bundle.mkdir()
        (bundle / "bundle.json").write_bytes(b"original")
        request = root / "request.json"
        request.write_bytes(b"request")
        return root, bundle, request

    def test_improvement_uses_separately_bound_reuse_request(self) -> None:
        root, bundle, request = self.reuse_fixture()
        measured: gate.JsonObject = {"candidateConditions": 84}
        result: gate.JsonObject = {"reuseEligibility": True, "measuredResult": measured}
        with (patch.object(promotion, "_runtime_identity", return_value="runtime"),
              patch.object(reuse, "verify_reuse_request", return_value=result) as verify,
              patch.object(gate, "verify", side_effect=AssertionError("unbound verification")),
              patch.object(gate, "Bundle", return_value=object()),
              patch.object(promotion, "_measure", return_value=(True, {})) as measure):
            self.assertTrue(promotion.improvement_established(bundle, root, reuse_request=request))
        self.assertGreaterEqual(verify.call_count, 2)
        self.assertEqual(verify.call_args.kwargs["expected_measured_dir"], bundle)
        self.assertEqual(verify.call_args.kwargs["expected_measured_manifest_sha256"], gate.sha256(bundle / "bundle.json"))
        self.assertEqual(verify.call_args.kwargs["expected_request_sha256"], gate.sha256(request))
        measure.assert_called_once_with(unittest.mock.ANY, measured)

    def test_improvement_rejects_mismatched_or_changed_request(self) -> None:
        root, bundle, request = self.reuse_fixture()
        for reason in ("request measured bundle differs", "source changed", "request file changed"):
            with self.subTest(reason=reason), patch.object(promotion, "_runtime_identity", return_value="runtime"), \
                 patch.object(reuse, "verify_reuse_request", side_effect=ValueError(reason)):
                with self.assertRaisesRegex(ValueError, reason):
                    promotion.improvement_established(bundle, root, reuse_request=request)

    def test_cli_forwards_separate_reuse_request(self) -> None:
        with (patch.object(sys, "argv", ["promotion", "--repo", "/repo", "--evidence-dir", "/bundle",
                                        "--promotion-evidence-dir", "/proof", "--run-id", "run",
                                        "--reuse-request", "/request"]),
              patch.object(promotion, "promote", return_value="record") as promote,
              contextlib.redirect_stdout(io.StringIO())):
            self.assertEqual(promotion.main(), 0)
        self.assertEqual(promote.call_args.kwargs["reuse_request"], Path("/request"))

    def test_changed_bundle_is_rejected_after_formal_verification(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "bundle.json"
            manifest.write_bytes(b"original")

            def altered(_directory: Path, _root: Path) -> gate.JsonObject:
                manifest.write_bytes(b"modified")
                return {}

            with (patch.object(gate, "verify", side_effect=altered),
                  patch.object(promotion, "_runtime_identity", return_value="runtime")):
                with self.assertRaisesRegex(ValueError, "bundle changed"):
                    promotion.improvement_established(root, root)

    def test_promotion_appends_only_after_validation_and_keeps_old_record(self) -> None:
        repository = Path(__file__).resolve().parents[2]
        original = (repository / "docs/performance/html-performance-history.json").read_bytes()
        history = gate._object(json.loads(original), "test history")
        first = gate._objects(history["records"], "test records")[0]
        candidate = copy.deepcopy(first)
        candidate["id"] = "new-record"
        head = "a" * 40
        for mode in ("accepted", "unchanged", "runtime-changed", "environment-changed", "child-failed",
                     "environment-comparison", "environment-final-reuse", "accepted-reuse", "unchanged-reuse",
                     "v1", "short", "unknown", "v1-reuse"):
            with self.subTest(mode=mode), TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target = root / "docs/performance/html-performance-history.json"
                target.parent.mkdir(parents=True)
                target.write_bytes(original)
                bundle_path = root / "bundle"
                bundle_path.mkdir()
                (bundle_path / "bundle.json").write_text("{}", encoding="utf8")
                (root / "tmp").mkdir()
                request = root / "tmp/request.json"
                request.write_bytes(b"request")
                measured: gate.JsonObject = {"memoryPolicyVersion": "html-memory-v2",
                                             "retainedMemoryConditions": 84}
                if mode.startswith("v1"):
                    measured = {}
                elif mode == "short":
                    measured["retainedMemoryConditions"] = 83
                elif mode == "unknown":
                    measured["memoryPolicyVersion"] = "unknown"
                reused = {"reuseEligibility": True, "measuredResult": measured}
                bundle = gate.Bundle.__new__(gate.Bundle)
                bundle.manifest = {"head": head}

                def fake_git(_root: Path, *args: str) -> str:
                    return head + "\n" if args[0] == "rev-parse" else original.decode()

                def fake_run(argv: list[str], *, stdout: IO[bytes], stderr: IO[bytes],
                             selected_mode: str = mode, selected_request: Path = request,
                             selected_result: dict[str, object] = reused,
                             selected_measured: gate.JsonObject = measured,
                             **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
                    self.assertTrue(_kwargs.get("env") == dict(os.environ))
                    self.assertIn("--require-memory-policy-v2", argv)
                    if selected_mode.endswith("-reuse"):
                        self.assertEqual(argv[-2:], ["--reuse-request", str(selected_request)])
                    stdout.write(json.dumps(selected_result if selected_mode.endswith("-reuse") else selected_measured).encode() + b"\n")
                    if selected_mode == "environment-changed":
                        os.environ["KRR_PROMOTION_TEST_MUTATION"] = "changed"
                    if selected_mode == "child-failed":
                        stderr.write(b"dummy-private-verifier-marker\n")
                    return subprocess.CompletedProcess([], 7 if selected_mode == "child-failed" else 0)

                def fake_measure(*_args: object, selected_mode: str = mode) -> tuple[bool, gate.JsonObject]:
                    if selected_mode == "environment-comparison":
                        os.environ["KRR_PROMOTION_TEST_MUTATION"] = "changed"
                    return not selected_mode.startswith("unchanged"), {}

                reuse_calls = [0]
                def fake_reuse(*_args: object, selected_mode: str = mode,
                               calls: list[int] = reuse_calls,
                               result: dict[str, object] = reused, **_kwargs: object) -> dict[str, object]:
                    calls[0] += 1
                    if selected_mode == "environment-final-reuse" and calls[0] == 2:
                        os.environ["KRR_PROMOTION_TEST_MUTATION"] = "changed"
                    return result

                with (patch.dict(os.environ, {}, clear=False),
                      patch.object(gate, "git", side_effect=fake_git),
                      patch.object(promotion, "_runtime_identity",
                                   side_effect=["runtime", "changed" if mode == "runtime-changed" else "runtime",
                                                "runtime"]),
                      patch.object(promotion, "_schema_validate"),
                      patch.object(reuse, "verify_reuse_request", side_effect=fake_reuse) as verify_reuse,
                      patch.object(subprocess, "run", side_effect=fake_run),
                      patch.object(gate, "Bundle", return_value=bundle),
                      patch.object(promotion, "_measure", side_effect=fake_measure) as measure,
                      patch.object(promotion, "_record", return_value=candidate) as record):
                    request_arg = request if mode.endswith("-reuse") else None
                    if mode.startswith("accepted"):
                        self.assertEqual(promotion.promote(bundle_path, root, root / "tmp/proof", "run", reuse_request=request_arg), "new-record")
                        updated = gate.read_json(target)
                        self.assertEqual(updated["activeReferenceRecordId"], "new-record")
                        self.assertEqual(gate._objects(updated["records"], "updated")[0], first)
                        self.assertEqual((root / "tmp/proof").stat().st_mode & 0o777, 0o700)
                        if request_arg is not None:
                            self.assertEqual(verify_reuse.call_count, 2)
                            self.assertEqual(verify_reuse.call_args.kwargs["expected_measured_dir"], bundle_path)
                    else:
                        reason = ("requires v2 retained memory" if mode in ("v1", "short", "unknown", "v1-reuse") else
                                  "runtime identity changed" if mode == "runtime-changed" else
                                  "environment changed" if mode.startswith("environment-") else
                                  "formal performance verification failed" if mode == "child-failed" else "improvement")
                        with self.assertRaisesRegex(ValueError, reason) as caught:
                            promotion.promote(bundle_path, root, root / "tmp/proof", "run", reuse_request=request_arg)
                        self.assertEqual(target.read_bytes(), original)
                        if mode in ("v1", "short", "unknown", "v1-reuse"):
                            measure.assert_not_called()
                            record.assert_not_called()
                        if mode == "child-failed":
                            proof = root / "tmp/proof"
                            self.assertNotIn("dummy-private-verifier-marker", str(caught.exception))
                            self.assertEqual((proof / "verifier.stderr").read_bytes(), b"dummy-private-verifier-marker\n")
                            self.assertEqual(gate.read_json(proof / "exit.json")["exitCode"], 7)
                            self.assertEqual(proof.stat().st_mode & 0o777, 0o700)

    def test_atomic_update_preserves_history_on_conflict_and_releases_lock(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            history = root / "history.json"
            history.write_bytes(b"changed")
            with patch.object(gate, "git", return_value="head\n"):
                with self.assertRaisesRegex(ValueError, "changed"):
                    promotion._atomic_history_update(root, history, b"original", b"new", "head")
            self.assertEqual(history.read_bytes(), b"changed")
            self.assertFalse(history.with_name("history.json.promotion.lock").exists())

    def test_atomic_update_rejects_concurrent_owner_and_preserves_original(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            history = root / "history.json"
            history.write_bytes(b"original")
            lock = history.with_name("history.json.promotion.lock")
            lock.write_bytes(b"other owner")
            with self.assertRaises(FileExistsError):
                promotion._atomic_history_update(root, history, b"original", b"new", "head")
            self.assertEqual(history.read_bytes(), b"original")
            self.assertEqual(lock.read_bytes(), b"other owner")

    def test_atomic_update_replaces_complete_bytes_and_keeps_mode(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            history = root / "history.json"
            history.write_bytes(b"original")
            history.chmod(0o640)
            with patch.object(gate, "git", return_value="head\n"):
                promotion._atomic_history_update(root, history, b"original", b"new", "head")
            self.assertEqual(history.read_bytes(), b"new")
            self.assertEqual(history.stat().st_mode & 0o777, 0o640)
            self.assertFalse(history.with_name("history.json.promotion.lock").exists())

    def test_record_rejects_unaccepted_html(self) -> None:
        bundle = gate.Bundle.__new__(gate.Bundle)
        bundle.manifest = {"head": "a" * 40}
        result: gate.JsonObject = {"head": "a" * 40, "htmlStage90": {"acceptance": False}}
        with self.assertRaisesRegex(ValueError, "HTML acceptance"):
            promotion._record(bundle, result, {}, "run", "a" * 64, "b" * 64, "c" * 64,
                              "2026-01-01T00:00:00Z")

    def test_history_schema_rejects_malformed_input(self) -> None:
        root = Path(__file__).resolve().parents[2]
        with self.assertRaises(subprocess.CalledProcessError):
            promotion._schema_validate(root, b'{"schema":"invalid"}')

    def test_direct_call_without_prepared_environment_is_rejected(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "bootstrap root"):
                promotion.promote(root, root, root / "proof", "run")

    def test_scientific_runtime_identity_must_match_bootstrap(self) -> None:
        root = runner.ROOT
        with (patch.object(runner, "_require_private_venv_path"),
              patch.object(runner, "_verify_prepared_python"),
              patch.object(runner, "_venv_python", return_value=Path(sys.executable)),
              patch.object(runner, "VENV", Path(sys.prefix)),
              patch.object(runner, "_file_sha256", return_value="uv-hash"),
              patch.object(python_identity, "installed_identity", return_value="different"),
              patch.dict(os.environ, {runner.UV_PATH_ENV: "/fixture/uv", runner.UV_ENV: "uv-hash",
                                      runner.IDENTITY_ENV: "expected"})):
            with self.assertRaisesRegex(ValueError, "scientific runtime"):
                promotion._runtime_identity(root)

    def test_cli_does_not_echo_untrusted_arguments(self) -> None:
        stream = io.StringIO()
        with (patch.object(sys, "argv", ["promotion", "--unexpected", "private-value"]),
              contextlib.redirect_stderr(stream)):
            self.assertEqual(promotion.main(), 1)
        self.assertNotIn("private-value", stream.getvalue())


if __name__ == "__main__":
    unittest.main()
