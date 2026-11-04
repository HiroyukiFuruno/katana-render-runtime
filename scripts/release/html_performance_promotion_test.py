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
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import IO
from unittest.mock import patch

import html_performance_promotion as promotion
import html_performance_python_identity as python_identity
import html_performance_source_reuse_cli as reuse
import run_html_performance as runner
import verify_html_performance_evidence as gate


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
        for mode in ("accepted", "unchanged", "runtime-changed", "accepted-reuse", "unchanged-reuse"):
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
                reused = {"reuseEligibility": True, "measuredResult": {}}
                bundle = gate.Bundle.__new__(gate.Bundle)
                bundle.manifest = {"head": head}

                def fake_git(_root: Path, *args: str) -> str:
                    return head + "\n" if args[0] == "rev-parse" else original.decode()

                def fake_run(argv: list[str], *, stdout: IO[bytes],
                             selected_mode: str = mode, selected_request: Path = request,
                             selected_result: dict[str, object] = reused,
                             **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
                    if selected_mode.endswith("-reuse"):
                        self.assertEqual(argv[-2:], ["--reuse-request", str(selected_request)])
                    stdout.write(json.dumps(selected_result if selected_mode.endswith("-reuse") else {}).encode() + b"\n")
                    return subprocess.CompletedProcess([], 0)

                with (patch.object(gate, "git", side_effect=fake_git),
                      patch.object(promotion, "_runtime_identity",
                                   side_effect=["runtime", "changed" if mode == "runtime-changed" else "runtime",
                                                "runtime"]),
                      patch.object(promotion, "_schema_validate"),
                      patch.object(reuse, "verify_reuse_request", return_value=reused) as verify_reuse,
                      patch.object(subprocess, "run", side_effect=fake_run),
                      patch.object(gate, "Bundle", return_value=bundle),
                      patch.object(promotion, "_measure", return_value=(not mode.startswith("unchanged"), {})),
                      patch.object(promotion, "_record", return_value=candidate)):
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
                        reason = "runtime identity changed" if mode == "runtime-changed" else "improvement"
                        with self.assertRaisesRegex(ValueError, reason):
                            promotion.promote(bundle_path, root, root / "tmp/proof", "run", reuse_request=request_arg)
                        self.assertEqual(target.read_bytes(), original)

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
