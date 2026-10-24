from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

import native_cli_observation_view as view
from produce_html_performance_evidence import NativeCliCommandEvidence


class NativeCliObservationViewTests(unittest.TestCase):
    def test_summary_accepts_native_namedtuple_wrapper(self) -> None:
        secret = "SYNTHETIC_WRAPPER_SECRET_SENTINEL"
        payload = valid_observation()
        payload["environment"] = {"AUTH_KEY": secret}
        evidence = NativeCliCommandEvidence(payload, {"environment": secret})

        summary = json.dumps(view.safe_summary(evidence))

        self.assertNotIn(secret, summary)
        self.assertEqual(json.loads(summary)["actualExit"], 0)

    def test_summary_omits_private_command_and_environment_values(self) -> None:
        secret = "SYNTHETIC_SECRET_SENTINEL_DO_NOT_PRINT"
        payload = valid_observation()
        payload["argv"] = ["tool", "--token", secret]
        payload["environment"] = {"PLATFORM_AUTH_KEY": secret}
        payload["cwd"] = secret
        payload["label"] = secret
        lifecycle = {"environment": {"nested": secret}, "unexpected": secret}
        inputs = ({"observation": payload, "lifecycle": lifecycle}, [payload, lifecycle])
        for evidence in inputs:
            self.assert_safe_cli_summary(evidence, secret)

    def assert_safe_cli_summary(self, evidence: object, secret: str) -> None:
        result, stdout, stderr = run_view_text(json.dumps(evidence))
        self.assertEqual((result, stderr), (0, ""))
        self.assertNotIn(secret, stdout)
        summary = json.loads(stdout)
        self.assertEqual(set(summary), {
            "actualExit", "pid", "startedNs", "endedNs", "stdout", "stderr",
        })
        self.assertEqual((summary["actualExit"], summary["pid"]), (0, 42))
        self.assertEqual((summary["startedNs"], summary["endedNs"]), ("100", "200"))

    def test_invalid_nested_fields_fail_without_echoing_secret(self) -> None:
        secret = "SYNTHETIC_NESTED_SECRET_SENTINEL"
        payload = valid_observation()
        payload["stdout"] = {
            "path": "/tmp/native-cli/stdout.log",
            "sha256": "a" * 64,
            "size": 12,
            "unexpected": {"secret": secret},
        }
        result, stdout, stderr = run_view_text(json.dumps(payload))

        self.assertEqual(result, 2)
        self.assertNotIn(secret, stdout + stderr)
        self.assertEqual(stdout, "")
        self.assertEqual(stderr, "invalid native CLI observation\n")

    def test_duplicate_json_keys_fail_without_echoing_input(self) -> None:
        payload = json.dumps(valid_observation()).replace('"pid": 42', '"pid": 42, "pid": 43')
        result, stdout, stderr = run_view_text(payload)

        self.assertEqual(result, 2)
        self.assertEqual(stdout, "")
        self.assertEqual(stderr, "invalid native CLI observation\n")

    def test_malformed_json_is_reported_without_echoing_input(self) -> None:
        secret = "SYNTHETIC_MALFORMED_SECRET"
        result, stdout, stderr = run_view_text("{" + secret)

        self.assertEqual(result, 2)
        self.assertEqual(stdout, "")
        self.assertNotIn(secret, stderr)
        self.assertEqual(stderr, "invalid native CLI observation\n")

    def test_symlink_input_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result, stdout, stderr = run_view_symlink(Path(temporary))

        self.assertEqual((result, stdout), (2, ""))
        self.assertEqual(stderr, "invalid native CLI observation\n")

    def test_argument_count_failure_is_generic(self) -> None:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = view.main(["private-argument", "secret"])

        self.assertEqual(result, 2)
        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(stderr.getvalue(), "invalid native CLI observation\n")


def valid_observation() -> dict[str, object]:
    artifact: dict[str, object] = {
        "path": "/tmp/native-cli/stdout.log", "sha256": "a" * 64, "size": 12,
    }
    return {
        "argv": ["tool", "build"], "stdout": artifact, "stderr": dict(artifact),
        "label": "synthetic", "cwd": "/repo", "attemptToken": "b" * 64,
        "environment": {"PATH": "/bin"}, "environmentSha256": "c" * 64,
        "pid": 42, "actualExit": 0, "startedNs": "100", "endedNs": "200",
        "reaped": True, "errors": [],
    }


def run_view_text(payload: str) -> tuple[int, str, str]:
    stdout = io.StringIO()
    stderr = io.StringIO()
    with tempfile.TemporaryDirectory() as temporary:
        source = Path(temporary) / "observation.json"
        source.write_text(payload, encoding="utf-8")
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = view.main([str(source)])
    return result, stdout.getvalue(), stderr.getvalue()


def run_view_symlink(directory: Path) -> tuple[int, str, str]:
    source = directory / "observation.json"
    target = directory / "target.json"
    target.write_text(json.dumps(valid_observation()), encoding="utf-8")
    source.symlink_to(target)
    stdout = io.StringIO()
    stderr = io.StringIO()
    with redirect_stdout(stdout), redirect_stderr(stderr):
        result = view.main([str(source)])
    return result, stdout.getvalue(), stderr.getvalue()


if __name__ == "__main__":
    unittest.main()
