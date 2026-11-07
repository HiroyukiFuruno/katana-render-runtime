from __future__ import annotations

import hashlib
import json
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

import html_performance_source_reuse_cli as cli
import verify_html_performance_evidence as gate


class ReuseRequestTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.tmp = self.root / "tmp"
        self.tmp.mkdir()
        self.measured = self.tmp / "measured"
        self.current = self.tmp / "current"
        self.measured.mkdir()
        self.current.mkdir()
        self.output = self.tmp / "request.json"
        self.request: cli.ReuseRequest = {
            "schema": cli.SCHEMA,
            "measuredDir": str(self.measured),
            "measuredManifestSha256": "a" * 64,
            "currentBuildDir": str(self.current),
            "currentBuildManifestSha256": "b" * 64,
            "currentBuildArtifactPath": "build/proof.json",
            "currentBuildArtifactSha256": "c" * 64,
        }

    def test_create_and_verify_bind_all_request_fields(self) -> None:
        result = {"measuredHead": "old", "eligibleHead": "new"}
        with patch.object(gate, "verify_source_reuse", return_value=result) as verify:
            self.assertEqual(cli.create_request(self.request, self.output, self.root), result)
            self.assertEqual(json.loads(self.output.read_text()), self.request)
            self.assertEqual(cli.verify_reuse_request(self.output, self.root), result)
            self.assertEqual(verify.call_count, 2)
            verify.assert_called_with(self.measured, self.root, "a" * 64, self.current,
                                      "b" * 64, "build/proof.json", "c" * 64)
            verify.reset_mock()
            with self.assertRaises(FileExistsError):
                cli.create_request(self.request, self.output, self.root)
            verify.assert_not_called()
            self.assertEqual(json.loads(self.output.read_text()), self.request)

    def test_existing_symlink_rejected_before_validation(self) -> None:
        self.output.symlink_to(self.tmp / "missing")
        with patch.object(gate, "verify_source_reuse") as verify:
            with self.assertRaises(FileExistsError):
                cli.create_request(self.request, self.output, self.root)
            verify.assert_not_called()

    def test_rejects_request_rewrite_or_replacement_during_validation(self) -> None:
        for operation in ("rewrite", "replace"):
            with self.subTest(operation=operation):
                self.output.write_text(json.dumps(self.request))
                def mutate(*_args: object, current_operation: str = operation) -> dict[str, object]:
                    if current_operation == "rewrite":
                        self.output.write_text(json.dumps(self.request) + " ")
                    else:
                        replacement = self.tmp / "replacement.json"
                        replacement.write_text(json.dumps(self.request))
                        replacement.replace(self.output)
                    return {"eligibleHead": "new"}
                with patch.object(gate, "verify_source_reuse", side_effect=mutate):
                    with self.assertRaisesRegex(ValueError, "request file changed during validation"):
                        cli.verify_reuse_request(self.output, self.root)

    def test_failure_never_creates_request(self) -> None:
        with patch.object(gate, "verify_source_reuse", side_effect=ValueError("pin mismatch")):
            with self.assertRaisesRegex(ValueError, "pin mismatch"):
                cli.create_request(self.request, self.output, self.root)
        self.assertFalse(self.output.exists())

    def test_cli_forwards_explicit_pins(self) -> None:
        argv = ["reuse", "--repo", str(self.root), "--measured-dir", str(self.measured),
                "--measured-manifest-sha256", "a" * 64, "--current-build-dir", str(self.current),
                "--current-build-manifest-sha256", "b" * 64,
                "--current-build-artifact-path", "build/proof.json",
                "--current-build-artifact-sha256", "c" * 64, "--request-output", str(self.output)]
        with (patch.object(sys, "argv", argv),
              patch.object(gate, "verify_source_reuse", return_value={"eligibleHead": "new"}) as verify,
              redirect_stdout(io.StringIO())):
            self.assertEqual(cli.main(), 0)
        verify.assert_called_once_with(self.measured, self.root, "a" * 64, self.current,
                                       "b" * 64, "build/proof.json", "c" * 64)

    def test_rejects_malformed_and_noncanonical_requests(self) -> None:
        for value in (
            {**self.request, "extra": True},
            {**self.request, "measuredManifestSha256": "invalid"},
            {**self.request, "currentBuildArtifactPath": "../proof"},
            {**self.request, "measuredDir": "relative"},
        ):
            self.output.write_text(json.dumps(value))
            with self.assertRaises(ValueError):
                cli.verify_reuse_request(self.output, self.root)
        self.output.write_text('{"schema":"first","schema":"second"}')
        with self.assertRaises(ValueError):
            cli.verify_reuse_request(self.output, self.root)
        with self.assertRaisesRegex(ValueError, "inside repository tmp"):
            cli.verify_reuse_request(self.root / "outside.json", self.root)

    def test_expected_bundle_and_request_digest_bind_original_bytes(self) -> None:
        self.output.write_text(json.dumps(self.request))
        digest = hashlib.sha256(self.output.read_bytes()).hexdigest()
        with patch.object(gate, "verify_source_reuse", return_value={"eligibleHead": "new"}) as verify:
            cli.verify_reuse_request(self.output, self.root, expected_measured_dir=self.measured,
                                     expected_measured_manifest_sha256="a" * 64,
                                     expected_request_sha256=digest)
            verify.assert_called_once()
            for kwargs in ({"expected_measured_dir": self.current},
                           {"expected_measured_manifest_sha256": "f" * 64},
                           {"expected_request_sha256": "f" * 64}):
                with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                    cli.verify_reuse_request(self.output, self.root, **kwargs)

    def test_artifact_eligibility_is_immutable_and_revalidated(self) -> None:
        result = {"measuredHead": "old", "eligibleHead": "new",
                  "reuseKind": "test-source-artifact-equivalence-v1", "binarySha256": "a" * 64}
        with patch.object(gate, "verify_source_reuse", return_value=result):
            cli.create_request(self.request, self.output, self.root)
            stored = json.loads(self.output.read_text())
            self.assertEqual(stored["schema"], cli.ELIGIBILITY_SCHEMA)
            self.assertEqual(stored["result"], result)
            self.assertEqual(cli.verify_reuse_request(self.output, self.root), result)
            stored["result"]["binarySha256"] = "b" * 64
            self.output.write_text(json.dumps(stored))
            with self.assertRaisesRegex(ValueError, "stored eligibility differs"):
                cli.verify_reuse_request(self.output, self.root)
            stored["result"] = None
            self.output.write_text(json.dumps(stored))
            with self.assertRaisesRegex(ValueError, "stored eligibility differs"):
                cli.verify_reuse_request(self.output, self.root)

    def test_program_artifact_eligibility_requires_immutable_revalidation(self) -> None:
        result = {"measuredHead": "old", "eligibleHead": "new",
                  "reuseKind": "program-source-artifact-equivalence-v1", "binarySha256": "a" * 64}
        with patch.object(gate, "verify_source_reuse", return_value=result):
            self.output.write_text(json.dumps(self.request))
            with self.assertRaisesRegex(ValueError, "immutable eligibility is required"):
                cli.verify_reuse_request(self.output, self.root)
        output = self.output.with_name("program-eligibility.json")
        with patch.object(gate, "verify_source_reuse", return_value=result) as verify:
            cli.create_request(self.request, output, self.root)
            self.assertEqual(cli.verify_reuse_request(output, self.root), result)
            verify.return_value = {**result, "binarySha256": "b" * 64}
            with self.assertRaisesRegex(ValueError, "stored eligibility differs"):
                cli.verify_reuse_request(output, self.root)

    def test_v3_reassessment_keeps_original_eligibility_unchanged(self) -> None:
        original = {"measuredHead": "old", "eligibleHead": "new",
                    "reuseKind": "test-source-artifact-equivalence-v1"}
        reassessed = {**original, "measuredResult": {"memoryPolicyVersion": "html-memory-v3"}}
        with patch.object(gate, "verify_source_reuse", return_value=original):
            cli.create_request(self.request, self.output, self.root)
        before = self.output.read_bytes()
        with patch.object(gate, "verify_source_reuse", side_effect=[original, reassessed]) as verify:
            self.assertEqual(cli.verify_reuse_request(
                self.output, self.root, memory_policy_version="html-memory-v3"), reassessed)
        self.assertEqual(self.output.read_bytes(), before)
        self.assertNotIn("memory_policy_version", verify.call_args_list[0].kwargs)
        self.assertEqual(verify.call_args_list[1].kwargs["memory_policy_version"], "html-memory-v3")

    def test_v3_eligibility_is_reauthenticated_using_its_own_version(self) -> None:
        result = {"measuredHead": "old", "eligibleHead": "new",
                  "reuseKind": "test-source-artifact-equivalence-v1",
                  "measuredResult": {"memoryPolicyVersion": "html-memory-v3"}}
        with patch.object(gate, "verify_source_reuse", return_value=result):
            cli.create_request(self.request, self.output, self.root, memory_policy_version="html-memory-v3")
        with patch.object(gate, "verify_source_reuse", return_value=result) as verify:
            self.assertEqual(cli.verify_reuse_request(self.output, self.root), result)
        self.assertEqual(verify.call_args.kwargs["memory_policy_version"], "html-memory-v3")


if __name__ == "__main__":
    unittest.main()
