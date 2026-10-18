from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import cast

import native_coverage_result as coverage_result
import native_coverage_result_test as fixture_helpers
import native_coverage_reuse as coverage_reuse


SCRIPT = Path(__file__).with_name("native_coverage_reuse.py").resolve()
WORKFLOW = Path(__file__).parents[2] / ".github/workflows/test-and-build.yml"


class NativeCoverageReuseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.tested_sha, self.profile, self.tools = fixture_helpers.initialize_fixture_repository(self.root)
        self.context = {
            "schema": coverage_result.COVERAGE_CONTEXT_SCHEMA,
            "testedSha": self.tested_sha,
            "workflowSha": "b" * 40,
            "profile": dict(self.profile),
            "tools": dict(self.tools),
        }
        self.output = self.root / "tmp/quality/native-coverage-reuse"
        self.output.mkdir(parents=True)
        self.context_path = self.output / "context.json"
        self.manifest_path = self.output / "manifest.json"
        self.context_path.write_text(json.dumps(self.context), encoding="utf-8")
        self._create_manifest()

    def _create_manifest(self) -> None:
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("native_coverage_result_cli.py")),
             "--repo", str(self.root), "create", "--context", str(self.context_path),
             "--output", str(self.manifest_path.relative_to(self.root))],
            cwd=self.root, text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def _invoke(self, command: str, *arguments: str, extra_env: dict[str, str] | None = None
                ) -> subprocess.CompletedProcess[str]:
        environment = dict(os.environ)
        if extra_env is not None:
            environment.update(extra_env)
        return subprocess.run(
            [sys.executable, str(SCRIPT), command, *arguments], cwd=self.root,
            text=True, capture_output=True, check=False, env=environment,
        )

    def _invoke_cache_verification(self, source_ref: str) -> subprocess.CompletedProcess[str]:
        return self._invoke(
            "verify-cache", "--repo", str(self.root), "--manifest", str(self.manifest_path),
            "--context", str(self.context_path), "--repository", "owner/repository",
            "--tested-sha", self.tested_sha,
            "--workflow-sha", cast(str, self.context["workflowSha"]), "--source-ref", source_ref,
        )

    def _write_seed_status(self, fresh: bool) -> Path:
        status_path = self.output / "status.json"
        status_path.write_text(json.dumps({
            "schema": "krr-native-test-coverage-seed-v1", "fresh": fresh,
            "runId": "123", "runAttempt": "2",
        }), encoding="utf-8")
        return status_path

    def _write_current_cache_key(self) -> Path:
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("native_coverage_result_cli.py")),
             "--repo", str(self.root), "cache-key", "--context", str(self.context_path)],
            cwd=self.root, text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        key_path = self.output / "cache-key.txt"
        key_path.write_text(cast(str, json.loads(result.stdout)["key"]), encoding="ascii")
        return key_path

    def _invoke_seed_verification(self, key_path: Path, status_path: Path
                                  ) -> subprocess.CompletedProcess[str]:
        return self._invoke(
            "verify-seed", "--repo", str(self.root), "--manifest", str(self.manifest_path),
            "--context", str(self.context_path), "--cache-key", str(key_path),
            "--status", str(status_path), "--tested-sha", self.tested_sha,
            "--workflow-sha", cast(str, self.context["workflowSha"]), "--run-id", "123",
            "--run-attempt", "2",
        )

    def test_attestation_command_binds_protected_source_and_immutable_subject(self) -> None:
        immutable = self.output / "immutable-manifest.json"
        immutable.write_bytes(self.manifest_path.read_bytes())
        immutable.chmod(0o400)
        command = coverage_reuse._attestation_command(
            immutable, "owner/repository", self.tested_sha,
            cast(str, self.context["workflowSha"]), "refs/heads/master")
        self.assertEqual(command[:4], ["gh", "attestation", "verify", str(immutable)])
        self.assertIn("owner/repository/.github/workflows/test-and-build.yml", command)
        self.assertIn(cast(str, self.context["workflowSha"]), command)
        self.assertIn(self.tested_sha, command)
        self.assertIn("refs/heads/master", command)
        self.assertIn("--deny-self-hosted-runners", command)
        self.assertEqual(immutable.stat().st_mode & 0o777, 0o400)

    def test_attestation_output_requires_documented_verification_result_shape(self) -> None:
        statement: dict[str, object] = {
            "subject": [{"name": "manifest.json", "digest": {"sha256": "a" * 64}}],
            "predicateType": "https://slsa.dev/provenance/v1",
            "predicate": {},
        }
        def row(attestation: object = {"bundle": {}}, signature: object = {"certificate": {}},
                timestamps: object = (), statement_value: object = statement) -> dict[str, object]:
            return {"attestation": attestation, "verificationResult": {
                "signature": signature, "verifiedTimestamps": timestamps, "statement": statement_value}}

        verified_row = row()
        encoded_row = json.loads(json.dumps([verified_row]))
        self.assertEqual(coverage_reuse._parse_attestation_output(json.dumps([verified_row])), encoded_row)
        malformed_rows: tuple[object, ...] = (
            {"verified": True},
            row(attestation={}),
            row(signature={}),
            row(timestamps="unverified"),
            row(timestamps=[None]),
            row(statement_value={"subject": [], "predicateType": "x", "predicate": {}}),
            row(statement_value={"subject": [None], "predicateType": "x", "predicate": {}}),
            row(statement_value={"subject": statement["subject"], "predicateType": "", "predicate": {}}),
        )
        for malformed in ("", "{}", "[]", "null", json.dumps([None]),
                          *(json.dumps([row]) for row in malformed_rows)):
            with self.subTest(malformed=malformed), self.assertRaises(
                    coverage_reuse.CoverageReuseError):
                coverage_reuse._parse_attestation_output(malformed)

    def test_cache_verification_fails_closed_on_ref_or_context_drift(self) -> None:
        rejected_ref = self._invoke_cache_verification("refs/heads/feature")
        self.assertEqual(rejected_ref.returncode, 1)
        self.context["testedSha"] = "c" * 40
        self.context_path.write_text(json.dumps(self.context), encoding="utf-8")
        rejected_context = self._invoke_cache_verification("refs/heads/master")
        self.assertEqual(rejected_context.returncode, 1)

    def test_cache_verification_uses_immutable_snapshot_and_rejects_snapshot_change(self) -> None:
        original = coverage_reuse._snapshot_manifest(self.manifest_path)
        with tempfile.TemporaryDirectory(dir=self.output) as temporary:
            copy = coverage_reuse._immutable_copy(Path(temporary), original)
            self.assertEqual(copy.read_bytes(), original)
            self.assertEqual(copy.stat().st_mode & 0o777, 0o400)
            self.manifest_path.write_bytes(b"changed")
            self.assertEqual(copy.read_bytes(), original)
            copy.chmod(0o600)
            with self.assertRaises(coverage_reuse.CoverageReuseError):
                coverage_reuse._assert_copy_unchanged(copy, original)
            copy.write_bytes(original + b" ")
            copy.chmod(0o400)
            with self.assertRaises(coverage_reuse.CoverageReuseError):
                coverage_reuse._assert_copy_unchanged(copy, original)

    def test_fresh_seed_binds_run_context_manifest_and_exact_cache_key(self) -> None:
        key_path = self._write_current_cache_key()
        status_path = self._write_seed_status(True)
        result = self._invoke_seed_verification(key_path, status_path)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)["eligible"])
        key_path.write_text("0" * 64, encoding="ascii")
        mismatch = self._invoke_seed_verification(key_path, status_path)
        self.assertEqual(mismatch.returncode, 1)

    def test_fresh_seed_installs_the_validated_bytes_as_a_new_read_only_cache_file(self) -> None:
        key_path = self._write_current_cache_key()
        status_path = self._write_seed_status(True)
        cache_output = Path("tmp/quality/native-coverage-cache/manifest.json")
        result = self._invoke("verify-seed", "--repo", str(self.root), "--manifest",
                              str(self.manifest_path), "--context", str(self.context_path),
                              "--cache-key", str(key_path), "--status", str(status_path),
                              "--tested-sha", self.tested_sha, "--workflow-sha",
                              cast(str, self.context["workflowSha"]), "--run-id", "123",
                              "--run-attempt", "2", "--cache-output", str(cache_output))
        self.assertEqual(result.returncode, 0, result.stderr)
        installed = self.root / cache_output
        self.assertEqual(installed.read_bytes(), self.manifest_path.read_bytes())
        self.assertEqual(installed.stat().st_mode & 0o777, 0o400)
        collision = self._invoke("verify-seed", "--repo", str(self.root), "--manifest",
                                 str(self.manifest_path), "--context", str(self.context_path),
                                 "--cache-key", str(key_path), "--status", str(status_path),
                                 "--tested-sha", self.tested_sha, "--workflow-sha",
                                 cast(str, self.context["workflowSha"]), "--run-id", "123",
                                 "--run-attempt", "2", "--cache-output", str(cache_output))
        self.assertEqual(collision.returncode, 1)

    def test_cache_hit_is_never_eligible_for_attestation_seed(self) -> None:
        key_path = self.output / "cache-key.txt"
        key_path.write_text("0" * 64, encoding="ascii")
        status_path = self._write_seed_status(False)
        result = self._invoke_seed_verification(key_path, status_path)
        self.assertEqual(result.returncode, 1)

    def test_workflow_scopes_restore_sign_and_save_and_keeps_pr_fresh(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        test_job_start = workflow.index("\n  test:")
        signer_start = workflow.index("\n  sign-native-coverage-cache:")
        paired_start = workflow.index("\n  paired-performance:")
        test_job = workflow[test_job_start:signer_start]
        signer_job = workflow[signer_start:paired_start]
        self.assertIn("attestations: read", test_job)
        self.assertNotIn("attestations: write", test_job)
        self.assertNotIn("id-token: write", test_job)
        self.assertIn("actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9", workflow)
        self.assertIn("actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9", workflow)
        self.assertIn("actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6", workflow)
        self.assertIn("actions/download-artifact@37930b1c2abaa49bbe596cd826c3c89aef350131", workflow)
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/master'", test_job)
        self.assertIn("github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master'", test_job)
        self.assertIn("if: matrix.os == 'ubuntu-latest'", test_job)
        coverage_start = test_job.index("- name: Run coverage")
        coverage_end = test_job.index("- name: Prepare fresh-only coverage attestation seed")
        coverage_step = test_job[coverage_start:coverage_end]
        self.assertIn("id: coverage", coverage_step)
        self.assertIn("just coverage", coverage_step)
        self.assertIn("cache_verify.outputs.verified", coverage_step)
        cache_restore = test_job[test_job.index("- name: Restore exact trusted Linux coverage result"):
                                 test_job.index("- name: Verify trusted Linux coverage result")]
        cache_save = signer_job[signer_job.index("- name: Save immutable exact-key coverage result"):]
        self.assertIn("path: tmp/quality/native-coverage-cache/manifest.json", cache_restore)
        self.assertIn("path: tmp/quality/native-coverage-cache/manifest.json", cache_save)
        self.assertIn("${{ steps.coverage.outcome }}", test_job)
        self.assertIn("!= 'success'", test_job)
        seed_step = test_job[test_job.index("- name: Prepare fresh-only coverage attestation seed"):
                             test_job.index("- name: Upload sanitized coverage seed for master signing")]
        self.assertIn("collect-context", seed_step)
        self.assertIn("cmp -s", seed_step)
        self.assertIn("--context \"$post_context\"", seed_step)
        self.assertIn("continue-on-error: true\n        uses: actions/upload-artifact@", test_job)
        restore_start = test_job.index("- name: Restore exact trusted Linux coverage result")
        restore_end = test_job.index("- name: Verify trusted Linux coverage result")
        self.assertNotIn("restore-keys:", test_job[restore_start:restore_end])
        self.assertIn("actions: write", signer_job)
        self.assertIn("id-token: write", signer_job)
        self.assertIn("attestations: write", signer_job)
        self.assertIn("artifact-metadata: write", signer_job)
        self.assertIn("needs: [test, paired-performance]", signer_job)
        self.assertIn("steps.native_coverage_seed_verify.outputs.eligible == 'true'", signer_job)
        self.assertIn("id: native_coverage_attest", signer_job)
        self.assertIn("steps.native_coverage_attest.outcome == 'success'", signer_job)
        self.assertIn("continue-on-error: true\n        uses: actions/attest@", signer_job)
        self.assertIn("continue-on-error: true\n        uses: actions/cache/save@", signer_job)


if __name__ == "__main__":
    unittest.main()
