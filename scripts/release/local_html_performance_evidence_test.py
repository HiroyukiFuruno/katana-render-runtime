from __future__ import annotations

import copy
import datetime
import importlib.metadata
import json
import platform
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from html_performance_memory_policy import CURRENT_ACCEPTANCE_POLICY_VERSION
import local_html_performance_evidence as proof
import html_performance_source_reuse_cli as reuse_cli
import local_macos_evidence as mac
import verify_html_performance_evidence as gate


class LocalHtmlPerformanceEvidenceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.now = 1800000000
        timestamp = datetime.datetime.fromtimestamp(self.now, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        self.workflow = "a" * 64
        self.payload = {"schema": proof.SCHEMA, "repository": "owner/repo", "pull_request": 3,
            "base_sha": "b" * 40, "head_sha": "c" * 40, "workflow_sha256": self.workflow,
            "scope_sha256": proof.scope(self.workflow), "completed_at": timestamp, "command": proof.COMMAND,
            "bundle_manifest_sha256": "d" * 64, "result_sha256": "e" * 64,
            "conditions": 84, "candidate_conditions": 84, "checks": 1680,
            "formal_pass": True, "platform": "macos-arm64", "os_version": "27.2",
            "memory_policy_version": CURRENT_ACCEPTANCE_POLICY_VERSION,
            "tools": {"python": "3.13.0", "scipy": "1.18.1", "runtime_sha256": "f" * 64}}
        self.comment: dict[str, object] = {"body": proof.MARKER + "\n" + json.dumps(self.payload),
                        "user": {"login": "owner"}, "author_association": "OWNER",
                        "created_at": timestamp, "updated_at": timestamp}

    def accepted(self, comment: dict[str, object]) -> bool:
        return proof.valid(comment, "owner/repo", 3, "b" * 40, "c" * 40,
                           self.workflow, "owner", self.now)

    def test_requires_exact_identity_and_formal_scope(self) -> None:
        self.assertTrue(self.accepted(self.comment))
        for key, bad in (("head_sha", "f" * 40), ("base_sha", "f" * 40),
                         ("conditions", 83), ("candidate_conditions", 83),
                         ("checks", 1), ("formal_pass", False), ("scope_sha256", "0" * 64), ("os_version", "invalid"),
                         ("memory_policy_version", "html-memory-v6"),
                         ("tools", {"python": "3", "scipy": "wrong", "runtime_sha256": "f" * 64})):
            payload = {**self.payload, key: bad}
            comment = {**self.comment, "body": proof.MARKER + "\n" + json.dumps(payload)}
            self.assertFalse(self.accepted(comment), key)
        changed = copy.deepcopy(self.comment)
        changed["updated_at"] = "2020-01-01T00:00:00Z"
        self.assertFalse(self.accepted(changed))
        changed = copy.deepcopy(self.comment)
        changed["user"] = {"login": "other"}
        self.assertFalse(self.accepted(changed))
        self.now += mac.FRESHNESS_SECONDS + 1
        self.assertFalse(self.accepted(self.comment))

    def test_os_version_and_measurement_host(self) -> None:
        from pathlib import Path
        self.assertTrue(proof.valid_macos_version("27.2"))
        self.assertTrue(proof.valid_macos_version("11.0"))
        self.assertTrue(proof.valid_macos_version("10.15"))
        self.assertFalse(proof.valid_macos_version("27.2-extra"))
        bundle = unittest.mock.Mock()
        bundle.manifest = {"attempts": [{"expected": "expected", "pairManifest": "manifest"}] * 84}
        bundle.json.side_effect = lambda pin: ({"endpointRefs": {"before": "before"}} if pin == "expected" else
                                               {"toolContext": {"platform": "host"}} if pin == "before" else
                                               {"platform": "host"})
        with patch.object(gate, "Bundle", return_value=bundle):
            proof.require_measurement_host(Path("/unused"), "host")
            with self.assertRaisesRegex(ValueError, "platform differs"):
                proof.require_measurement_host(Path("/unused"), "other")

    def test_ci_accepts_local_version_at_most_one_major_ahead(self) -> None:
        for host, accepted in (("26.4", True), ("27.0.1", True), ("25.5", False),
                               ("28.0", False), ("unknown", False)):
            with self.subTest(host=host):
                self.assertEqual(proof.valid(self.comment, "owner/repo", 3, "b" * 40,
                                            "c" * 40, self.workflow, "owner", self.now,
                                            hosted_macos=host), accepted)

    def test_visual_proof_is_v024_owner_exact_and_27_to_26_only(self) -> None:
        visual = {"schema": proof.SCHEMA, "repository": "owner/repo", "pull_request": 3,
                  "base_sha": "b" * 40, "head_sha": "c" * 40, "workflow_sha256": self.workflow,
                  "scope_sha256": proof.visual_scope(self.workflow), "completed_at": self.comment["created_at"],
                  "command": proof.VISUAL_COMMAND, "verification_kind": proof.VISUAL_KIND,
                  "platform": "macos-arm64", "os_version": "27.2.1",
                  "capture_manifest_sha256": "d" * 64, "plan_sha256": "e" * 64,
                  "source_digest": "f" * 64, "result_sha256": "1" * 64,
                  "accepted_conditions": 84, "threshold": 90, "font_identity_equivalence": "established"}
        comment = {**self.comment, "body": proof.MARKER + "\n" + json.dumps(visual)}
        self.assertTrue(proof.valid(comment, "owner/repo", 3, "b" * 40, "c" * 40,
                                    self.workflow, "owner", self.now, hosted_macos="26.4"))
        self.assertTrue(proof.valid(comment, "owner/repo", 3, "b" * 40, "c" * 40,
                                    self.workflow, "owner", self.now))
        for host in ("27.1", "25.9", "26.invalid"):
            self.assertFalse(proof.valid(comment, "owner/repo", 3, "b" * 40, "c" * 40,
                                         self.workflow, "owner", self.now, hosted_macos=host))
        visual["verification_kind"] = "other"
        self.assertFalse(proof.valid({**comment, "body": proof.MARKER + "\n" + json.dumps(visual)},
                                     "owner/repo", 3, "b" * 40, "c" * 40, self.workflow, "owner", self.now,
                                     hosted_macos="26.4"))

    def test_visual_collector_requires_validated_source_and_fixed_observer(self) -> None:
        source = "a" * 64
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, plan, receipt = (root / name for name in ("captures.json", "plan.json", "receipt.json"))
            plan.write_text("{}", encoding="utf8")
            context = {"sourceManifest": {"digest": source},
                       "os": {"release": platform.release(), "version": platform.version()}}
            receipt.write_text(json.dumps({"browserExecutableSha256": proof.VISUAL_OBSERVER_SHA256}),
                               encoding="utf8")
            condition = {"chromeReceiptPath": str(receipt), "chromeReceiptSha256": gate.sha256(receipt)}
            manifest.write_text(json.dumps({"producerContext": context, "producerContextAfter": context,
                                           "conditions": [condition] * 84}),
                                encoding="utf8")
            validated = {"schema": "krr-html84-independent-stage90-validation-v1",
                         "acceptance": True, "threshold": 90, "fontIdentityEquivalence": "established",
                         "conditions": [{}] * 84, "manifestSha256": gate.sha256(manifest),
                         "planSha256": gate.sha256(plan)}
            completed = subprocess.CompletedProcess(["bun"], 0, json.dumps(validated))
            with (patch.object(proof, "_current_source_digest", return_value=source) as current_source,
                  patch.object(subprocess, "run", return_value=completed) as check):
                self.assertEqual(proof.validate_visual(manifest, plan)["accepted_conditions"], 84)
                self.assertIn(proof.VISUAL_COMMAND, check.call_args.args[0])
                plan.write_text('{"changed":true}', encoding="utf8")
                with self.assertRaisesRegex(mac.EvidenceError, "did not establish"):
                    proof.validate_visual(manifest, plan)
                receipt.write_text("{}", encoding="utf8")
                with self.assertRaisesRegex(mac.EvidenceError, "receipt SHA differs"):
                    proof.validate_visual(manifest, plan)
                condition["chromeReceiptSha256"] = gate.sha256(receipt)
                manifest.write_text(json.dumps({"producerContext": context, "producerContextAfter": context,
                                                "conditions": [condition] * 84}),
                                    encoding="utf8")
                with self.assertRaisesRegex(mac.EvidenceError, "observer differs"):
                    proof.validate_visual(manifest, plan)
                current_source.return_value = "b" * 64
                with self.assertRaisesRegex(mac.EvidenceError, "source differs"):
                    proof.validate_visual(manifest, plan)

    def test_owner_api_double_read_rejects_duplicates_and_changes(self) -> None:
        owner = "owner"
        with patch.object(mac, "workflow_scope_supported", return_value=True), \
             patch.object(mac, "gh_token", return_value="token"), \
             patch.object(mac, "api_request", return_value={}), \
             patch.object(mac, "canonical_owner", return_value=owner), \
             patch.object(mac, "validate_pr", return_value=("b" * 40, "c" * 40)), \
             patch.object(mac, "base_is_merged", return_value=True), \
             patch.object(mac, "workflow_digest", return_value=self.workflow), \
             patch.object(time, "time", return_value=self.now), \
             patch.object(mac, "read_comments_api") as read:
            read.side_effect = [[self.comment], [self.comment]]
            self.assertTrue(proof.verify_from_api("owner/repo", 3, "b" * 40, "c" * 40))
            read.side_effect = [[self.comment, self.comment], [self.comment, self.comment]]
            self.assertFalse(proof.verify_from_api("owner/repo", 3, "b" * 40, "c" * 40))
            read.side_effect = [[self.comment], []]
            self.assertFalse(proof.verify_from_api("owner/repo", 3, "b" * 40, "c" * 40))

    def test_expired_local_proof_requires_new_ci_measurement(self) -> None:
        step = {"name": "Report local HTML performance evidence", "conclusion": "success"}
        job: dict[str, object] = {"name": "Native paired HTML time and RSS nonregression", "steps": [step]}
        with self.assertRaisesRegex(ValueError, "rerun the required CI job"):
            proof.require_fresh_measurement_if_local_expired([job])
        step["conclusion"] = "skipped"
        proof.require_fresh_measurement_if_local_expired([job])

    def test_paired_workflow_falls_back_when_local_evidence_is_missing(self) -> None:
        from local_macos_workflow_contract_test import workflow_job, workflow_steps
        workflow = mac.WORKFLOW.read_text(encoding="utf8")
        paired = workflow_job(workflow, "paired-performance")
        steps = {step.get("name"): step for step in workflow_steps(paired)}
        self.assertEqual(steps["Verify optional local HTML performance evidence"].get("continue-on-error"), "true")
        self.assertEqual(steps["Verify optional local HTML performance evidence"].get("if"),
                         "github.event_name == 'pull_request' || github.ref == 'refs/heads/master'")
        self.assertIn('--verify-merged-head "$GITHUB_SHA"', paired)
        expected = "steps.html_local_proof.outcome != 'success' || steps.html_local_proof.outputs.reuse != 'true'"
        for name in ("Install Rust toolchain", "Capture all 84 HTML catalog conditions"):
            self.assertEqual(steps[name].get("if"), expected)
        for name in ("Measure all 84 catalog conditions against baseline",
                     "Upload immutable source and run bound paired evidence",
                     "Attest immutable paired performance artifact"):
            self.assertEqual(steps[name].get("if"), "steps.source.outputs.version != '0.4.24' && (" + expected + ")")
        self.assertEqual(steps["Verify all 84 HTML images and font identities for v0.4.24"].get("if"),
                         "steps.source.outputs.version == '0.4.24' && (" + expected + ")")
        self.assertIn("needs: [test, paired-performance]", workflow_job(workflow, "preflight"))

    def test_merged_proof_reuses_the_exact_binding_and_host_policy(self) -> None:
        base, head, merge = "b" * 40, "c" * 40, "d" * 40
        binding = 3, base, head
        with (patch.object(mac, "workflow_scope_supported", return_value=True),
              patch.object(mac, "gh_token", return_value="token"),
              patch.object(proof, "verified_merged_pr_binding", return_value=binding) as resolved,
              patch.object(proof, "verify_from_api", return_value=True) as verify):
            self.assertTrue(proof.verify_merged_from_api("owner/repo", merge, hosted_macos="26.7.2"))
            verify.assert_called_once_with("owner/repo", 3, base, head, hosted_macos="26.7.2", merge_head=merge,
                                           visual=False)
            self.assertEqual(resolved.call_count, 2)
            resolved.side_effect = [binding, (4, base, head)]
            self.assertFalse(proof.verify_merged_from_api("owner/repo", merge))
            resolved.side_effect = ValueError("different tree")
            verify.reset_mock()
            self.assertFalse(proof.verify_merged_from_api("owner/repo", merge))
            verify.assert_not_called()

    def test_merged_proof_rejects_a_closed_unmerged_or_different_pull(self) -> None:
        base, head, merge = "b" * 40, "c" * 40, "d" * 40
        repository = {"full_name": "owner/repo"}
        pull = {"number": 3, "state": "closed", "merged": True, "draft": False,
                "merge_commit_sha": merge,
                "base": {"ref": "master", "sha": base, "repo": repository},
                "head": {"ref": "release/v0.4.24", "sha": head, "repo": repository}}
        self.assertEqual(mac.validate_pr(pull, "owner/repo", 3, merge_head=merge), (base, head))
        with self.assertRaises(mac.EvidenceError):
            mac.validate_pr(pull, "owner/repo", 3)
        for change in ({"merged": False}, {"draft": True}, {"merge_commit_sha": head}, {"state": "open"}):
            with self.assertRaises(mac.EvidenceError):
                mac.validate_pr({**pull, **change}, "owner/repo", 3, merge_head=merge)

    def test_collector_never_accepts_failed_formal_verification(self) -> None:
        from contextlib import ExitStack
        from pathlib import Path
        with ExitStack() as stack:
            for platform_target, platform_value in (("system", "Darwin"), ("machine", "arm64")):
                stack.enter_context(patch.object(platform, platform_target, return_value=platform_value))
            for mac_target, mac_value in (("macos_product_version", "15.0"),
                                  ("require_standard_index_flags", None), ("git_status", ""),
                                  ("workflow_scope_supported", True), ("head_worktree_bytes_digest", "source"),
                                  ("workflow_digest", self.workflow), ("gh_token", "token"),
                                  ("canonical_owner", "owner"), ("validate_pr", ("b" * 40, "c" * 40))):
                stack.enter_context(patch.object(mac, mac_target, return_value=mac_value))
            stack.enter_context(patch.object(mac, "run_git", side_effect=lambda *args:
                                            "b" * 40 if args[0] == "merge-base" else "c" * 40))
            api = stack.enter_context(patch.object(mac, "api_request", return_value={"login": "owner"}))
            stack.enter_context(patch.object(gate, "sha256", return_value="d" * 64))
            stack.enter_context(patch.object(proof, "installed_identity", return_value="f" * 64))
            verifier = stack.enter_context(patch.object(gate, "verify", side_effect=ValueError("failed")))
            with self.assertRaisesRegex(ValueError, "failed"):
                proof.collect("owner/repo", 3, Path("/unused"))
            verifier.assert_called_once_with(Path("/unused"), mac.ROOT,
                                              memory_policy_version=CURRENT_ACCEPTANCE_POLICY_VERSION)
            self.assertTrue(all(call.args[1] != "POST" for call in api.call_args_list))

    def test_collector_requires_current_memory_policy_for_fresh_and_rejects_legacy_results(self) -> None:
        from contextlib import ExitStack
        from pathlib import Path
        formal: dict[str, object] = {
            "head": "c" * 40, "conditions": 84, "candidateConditions": 84,
            "checks": 1680, "memoryPolicyVersion": CURRENT_ACCEPTANCE_POLICY_VERSION,
            "retainedMemoryConditions": 84, "memoryPolicyAssessments": [{}] * 84,
        }
        with ExitStack() as stack:
            for name, value in (("system", "Darwin"), ("machine", "arm64"), ("platform", "host")):
                stack.enter_context(patch.object(platform, name, return_value=value))
            mac_patches: tuple[tuple[str, object], ...] = (
                ("macos_product_version", "27.2"),
                ("require_standard_index_flags", None), ("git_status", ""),
                ("workflow_scope_supported", True),
                ("head_worktree_bytes_digest", "source"),
                ("workflow_digest", self.workflow), ("gh_token", "token"),
                ("canonical_owner", "owner"), ("validate_pr", ("b" * 40, "c" * 40)),
                ("base_is_merged", True), ("utc_now", "2027-01-01T00:00:00Z"),
            )
            for patch_name, patch_return_value in mac_patches:
                stack.enter_context(patch.object(mac, patch_name, return_value=patch_return_value))
            stack.enter_context(patch.object(mac, "run_git", side_effect=lambda *args:
                                            "b" * 40 if args[0] == "merge-base" else "c" * 40))
            stack.enter_context(patch.object(mac, "api_request", return_value={"login": "owner"}))
            stack.enter_context(patch.object(gate, "sha256", return_value="d" * 64))
            stack.enter_context(patch.object(proof, "installed_identity", return_value="f" * 64))
            stack.enter_context(patch.object(proof, "require_measurement_host"))
            stack.enter_context(patch.object(importlib.metadata, "version", return_value="1.18.1"))
            verify = stack.enter_context(patch.object(gate, "verify", return_value=formal))
            require_current = stack.enter_context(patch.object(
                gate, "_require_cli_current_memory", wraps=gate._require_cli_current_memory))
            proof.collect("owner/repo", 3, Path("/unused"))
            verify.assert_called_once_with(Path("/unused"), mac.ROOT,
                                           memory_policy_version=CURRENT_ACCEPTANCE_POLICY_VERSION)
            require_current.assert_called_once_with(formal, reused=False)

        with self.assertRaisesRegex(ValueError, "retained memory"):
            gate._require_cli_current_memory({}, reused=False)
        legacy: gate.JsonObject = {
            "memoryPolicyVersion": "html-memory-v6", "retainedMemoryConditions": 84,
            "memoryPolicyAssessments": [{}] * 84,
        }
        with self.assertRaisesRegex(ValueError, "retained memory"):
            gate._require_cli_current_memory(legacy, reused=False)

    def test_reuse_projection_binds_measured_result_and_eligibility(self) -> None:
        from contextlib import ExitStack
        from pathlib import Path
        measured, request = Path("/bundle"), Path("/request")
        old, head = "a" * 40, "c" * 40
        formal: dict[str, object] = {"head": old, "conditions": 84,
                                      "candidateConditions": 84, "checks": 1680,
                                      "memoryPolicyVersion": CURRENT_ACCEPTANCE_POLICY_VERSION,
                                      "retainedMemoryConditions": 84,
                                      "memoryPolicyAssessments": [{}] * 84}
        reused: dict[str, object] = {"measuredHead": old, "eligibleHead": head,
                                      "reuseEligibility": True, "measuredResult": formal,
                                      "reuseKind": "test-source-artifact-equivalence-v1"}
        with ExitStack() as stack:
            for platform_name, platform_value in (("system", "Darwin"), ("machine", "arm64"), ("platform", "host")):
                stack.enter_context(patch.object(platform, platform_name, return_value=platform_value))
            mac_values: list[tuple[str, object]] = [("macos_product_version", "27.2"),
                ("require_standard_index_flags", None), ("git_status", ""),
                ("workflow_scope_supported", True), ("head_worktree_bytes_digest", "source"),
                ("workflow_digest", self.workflow), ("gh_token", "token"), ("canonical_owner", "owner"),
                ("validate_pr", ("b" * 40, head)), ("base_is_merged", True), ("utc_now", "2027-01-01T00:00:00Z")]
            for mac_name, mac_value in mac_values:
                stack.enter_context(patch.object(mac, mac_name, return_value=mac_value))
            stack.enter_context(patch.object(mac, "run_git", side_effect=lambda *args:
                                            "b" * 40 if args[0] == "merge-base" else head))
            stack.enter_context(patch.object(mac, "api_request", return_value={"login": "owner"}))
            digest = stack.enter_context(patch.object(gate, "sha256", return_value="f" * 64))
            stack.enter_context(patch.object(gate, "Bundle", return_value=unittest.mock.Mock(manifest={"head": old})))
            stack.enter_context(patch.object(gate, "_require_formal_release_result"))
            require_current = stack.enter_context(patch.object(
                gate, "_require_cli_current_memory", wraps=gate._require_cli_current_memory))
            stack.enter_context(patch.object(proof, "require_measurement_host"))
            stack.enter_context(patch.object(proof, "installed_identity", return_value="d" * 64))
            stack.enter_context(patch.object(importlib.metadata, "version", return_value="1.18.1"))
            verify = stack.enter_context(patch.object(reuse_cli, "verify_reuse_request", return_value=reused))
            result = proof.collect("owner/repo", 3, measured, request)
            self.assertEqual(result["measured_head"], old)
            self.assertEqual(result["eligibility_sha256"], "f" * 64)
            self.assertEqual(result["verification_kind"], "test-source-artifact-equivalence-v1")
            verify.assert_called_with(request, mac.ROOT, expected_measured_dir=measured,
                                      expected_measured_manifest_sha256="f" * 64,
                                      expected_request_sha256="f" * 64,
                                      memory_policy_version=CURRENT_ACCEPTANCE_POLICY_VERSION)
            require_current.assert_called_with(formal, reused=False)
            reused["reuseKind"] = "program-source-artifact-equivalence-v1"
            self.assertEqual(proof.collect("owner/repo", 3, measured, request)["verification_kind"],
                             "program-source-artifact-equivalence-v1")
            reused["reuseKind"] = "binary-bound-product-source-equivalence-v1"
            self.assertEqual(proof.collect("owner/repo", 3, measured, request)["verification_kind"],
                             "binary-bound-product-source-equivalence-v1")
            reused["reuseKind"] = "unknown-artifact-equivalence-v1"
            with self.assertRaisesRegex(ValueError, "unsupported reuse kind"):
                proof.collect("owner/repo", 3, measured, request)
            reused.pop("reuseKind")
            self.assertEqual(proof.collect("owner/repo", 3, measured, request)["verification_kind"],
                             "documentation-source-reuse-v1")
            reused["eligibleHead"] = "e" * 40
            with self.assertRaisesRegex(ValueError, "PR head"):
                proof.collect("owner/repo", 3, measured, request)
            reused["eligibleHead"] = head
            formal["conditions"] = 83
            with self.assertRaisesRegex(mac.EvidenceError, "formal HTML84"):
                proof.collect("owner/repo", 3, measured, request)
            formal["conditions"] = 84
            digest.side_effect = ["f" * 64, "f" * 64, "0" * 64]
            with self.assertRaisesRegex(ValueError, "request changed"):
                proof.collect("owner/repo", 3, measured, request)

    def test_reuse_owner_projection_requires_kind_head_and_digest(self) -> None:
        payload = {**self.payload, "command": proof.REUSE_COMMAND,
                   "verification_kind": "test-source-artifact-equivalence-v1",
                   "measured_head": "d" * 40, "eligibility_sha256": "e" * 64}
        comment = {**self.comment, "body": proof.MARKER + "\n" + json.dumps(payload)}
        self.assertTrue(self.accepted(comment))
        program_payload = {**payload, "verification_kind": "program-source-artifact-equivalence-v1"}
        program_comment = {**self.comment, "body": proof.MARKER + "\n" + json.dumps(program_payload)}
        self.assertTrue(self.accepted(program_comment))
        program_payload["verification_kind"] = "binary-bound-product-source-equivalence-v1"
        program_comment["body"] = proof.MARKER + "\n" + json.dumps(program_payload)
        self.assertTrue(self.accepted(program_comment))
        program_payload["verification_kind"] = "unknown-artifact-equivalence-v1"
        program_comment["body"] = proof.MARKER + "\n" + json.dumps(program_payload)
        self.assertFalse(self.accepted(program_comment))
        for key in ("verification_kind", "measured_head", "eligibility_sha256"):
            altered = dict(payload)
            altered.pop(key)
            comment = {**self.comment, "body": proof.MARKER + "\n" + json.dumps(altered)}
            self.assertFalse(self.accepted(comment))

    def test_api_failure_falls_back(self) -> None:
        with patch.object(mac, "workflow_scope_supported", return_value=True), \
             patch.object(mac, "gh_token", side_effect=OSError("unavailable")):
            self.assertFalse(proof.verify_from_api("owner/repo", 3, "b" * 40, "c" * 40))


if __name__ == "__main__":
    unittest.main()
