from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parent))

import local_review
from local_review_contract import CHECKS, receipt_payload, validate_receipt
from local_review_state import ReviewError, command, digest, issue_context, source_snapshot, strict_json


def inputs() -> dict:
    return {"source": {"base_sha": "a" * 40, "files": {"example.py": {"sha256": "b" * 64}}},
            "issues": [{"number": 89, "body_sha256": "c" * 64}],
            "model": "gpt-6.1-sol", "reasoning": "high", "schema_sha256": "d" * 64}


def review(value: dict) -> dict:
    evidence = [{"source": "example.py:1", "quote": "assert observed == expected",
                 "observation": "the assertion agrees with the requirement"}]
    return {"input_sha256": digest(value), "verdict": "PASS", "summary": "reviewed",
            "full_quality_gate": "not_run", "pending": ["full_quality_gate", "native_ci", "protected_merge"],
            "checks": {key: {"result": "verified", "reason": "verified against source",
                              "evidence": copy.deepcopy(evidence)} for key in CHECKS},
            "issues": [{"number": 89, "body_sha256": "c" * 64, "result": "verified",
                        "evidence": copy.deepcopy(evidence)}], "findings": []}


class ReceiptContractTest(unittest.TestCase):
    def test_changed_source_invalidates_old_receipt(self) -> None:
        before = inputs()
        receipt = receipt_payload(before, review(before))
        after = copy.deepcopy(before)
        after["source"]["files"]["example.py"]["sha256"] = "e" * 64
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, after)

    def test_receipt_integrity_rejects_modified_verdict(self) -> None:
        value = inputs()
        receipt = receipt_payload(value, review(value))
        receipt["review"]["summary"] = "changed after review"
        with self.assertRaisesRegex(ReviewError, "integrity"):
            validate_receipt(receipt, value)

    def test_changed_base_issue_model_schema_or_requirements_invalidates_receipt(self) -> None:
        value = inputs()
        receipt = receipt_payload(value, review(value))
        for key, replacement in [("model", "other"), ("reasoning", "low"),
                                 ("schema_sha256", "f" * 64), ("requirements", "changed")]:
            with self.subTest(key=key):
                changed = {**value, key: replacement}
                with self.assertRaisesRegex(ReviewError, "another input"):
                    validate_receipt(receipt, changed)
        for key in ("source", "issues"):
            changed = copy.deepcopy(value)
            if key == "source":
                changed[key]["base_sha"] = "f" * 40
            else:
                changed[key][0]["body_sha256"] = "f" * 64
            with self.assertRaisesRegex(ReviewError, "another input"):
                validate_receipt(receipt, changed)

    def test_unverified_or_missing_assertion_oracle_rejects_pass(self) -> None:
        value = inputs()
        for missing in (True, False):
            result = review(value)
            if missing:
                del result["checks"]["specification_and_assertion_oracle"]
            else:
                result["checks"]["specification_and_assertion_oracle"]["result"] = "blocked"
            with self.assertRaises(ReviewError):
                validate_receipt(receipt_payload(value, result), value)

    def test_priority_triage_blocks_relevant_findings_and_requires_acceptance(self) -> None:
        value = inputs()
        for priority in ("P0", "P1", "P2", "P3"):
            result = review(value)
            result["findings"] = [{"priority": priority, "blocking": True, "status": "open",
                                  "title": "wrong assertion", "reason": "violates requirement",
                                  "evidence": result["checks"][CHECKS[0]]["evidence"]}]
            with self.subTest(priority=priority), self.assertRaisesRegex(ReviewError, "blocking"):
                validate_receipt(receipt_payload(value, result), value)
        result["findings"][0].update(priority="P2", blocking=False, status="accepted",
                                       reason="unrelated to the requirement and compatibility")
        validate_receipt(receipt_payload(value, result), value)
        result["findings"][0]["reason"] = ""
        with self.assertRaises(ReviewError):
            validate_receipt(receipt_payload(value, result), value)

    def test_pending_later_gates_are_allowed_but_not_a_fake_quality_pass(self) -> None:
        value = inputs()
        validate_receipt(receipt_payload(value, review(value)), value)
        result = review(value)
        result["full_quality_gate"] = "passed"
        with self.assertRaises(ReviewError):
            validate_receipt(receipt_payload(value, result), value)

    def test_json_duplicate_fields_are_rejected(self) -> None:
        with self.assertRaisesRegex(ReviewError, "duplicate"):
            strict_json('{"verdict":"FAIL","verdict":"PASS"}')


class GitSnapshotTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        command(["git", "init", "--quiet"], self.root)
        (self.root / ".gitignore").write_text("tmp/\n")
        (self.root / "example.py").write_text("original\n")
        self.commit("initial")
        self.base = command(["git", "rev-parse", "HEAD"], self.root).strip()

    def commit(self, message: str) -> None:
        command(["git", "add", "-A"], self.root)
        command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", message], self.root)

    def test_staging_and_committing_same_bytes_preserves_source_snapshot(self) -> None:
        (self.root / "example.py").write_text("changed\n")
        (self.root / "new.py").write_text("new\n")
        untracked = source_snapshot(self.root, self.base)
        command(["git", "add", "-A"], self.root)
        staged = source_snapshot(self.root, self.base)
        self.assertNotEqual(untracked, staged)
        self.commit("changes")
        self.assertEqual(staged, source_snapshot(self.root, self.base))

    def test_resetting_staged_new_file_while_committing_tracked_source_invalidates_receipt(self) -> None:
        tracked = self.root / "example.py"
        tracked.write_text("reviewed tracked change\n")
        (self.root / "new.py").write_text("reviewed new source\n")
        command(["git", "add", "-A"], self.root)
        reviewed = source_snapshot(self.root, self.base)
        value = {**inputs(), "source": reviewed}
        receipt = receipt_payload(value, review(value))

        command(["git", "reset", "--quiet", "HEAD", "--", "new.py"], self.root)
        self.commit_index("commit tracked source only")

        current = source_snapshot(self.root, self.base)
        self.assertEqual(reviewed["files"], current["files"])
        self.assertNotEqual(reviewed, current)
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, {**value, "source": current})

    def test_intent_to_add_is_distinct_from_fully_staged_new_file(self) -> None:
        for contents in ("", "new source\n"):
            with self.subTest(contents=contents):
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    command(["git", "init", "--quiet"], root)
                    (root / "example.py").write_text("original\n")
                    command(["git", "add", "example.py"], root)
                    command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                             "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "initial"], root)
                    base = command(["git", "rev-parse", "HEAD"], root).strip()
                    path = root / "candidate.py"
                    path.write_text(contents)
                    untracked = source_snapshot(root, base)
                    command(["git", "add", "-N", "candidate.py"], root)
                    intent = source_snapshot(root, base)
                    self.assertEqual(untracked["files"], intent["files"])
                    self.assertEqual(untracked, intent)

                    command(["git", "add", "candidate.py"], root)
                    staged = source_snapshot(root, base)
                    self.assertEqual(intent["files"], staged["files"])
                    self.assertNotEqual(untracked, staged)
                    command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                             "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "commit candidate"], root)
                    self.assertEqual(staged, source_snapshot(root, base))

    def test_real_git_clean_filters_preserve_same_working_bytes_across_commit(self) -> None:
        for filter_kind in ("autocrlf", "custom"):
            with self.subTest(filter=filter_kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                if filter_kind == "autocrlf":
                    command(["git", "config", "core.autocrlf", "true"], root)
                    (root / ".gitattributes").write_text("*.txt text eol=lf\n")
                else:
                    (root / ".gitattributes").write_text("*.txt filter=uppercase\n")
                    clean = (f"{shlex.quote(sys.executable)} -c "
                             '"import sys; sys.stdout.write(sys.stdin.read().upper())"')
                    command(["git", "config", "filter.uppercase.clean", clean], root)
                source = root / "example.txt"
                source.write_text("initial tracked content\n")
                command(["git", "add", ".gitattributes", "example.txt"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "filter setup"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()
                source.write_bytes(b"same working bytes\r\n" if filter_kind == "autocrlf" else b"same working bytes\n")
                before = source_snapshot(root, base)
                command(["git", "add", "example.txt"], root)
                staged = source_snapshot(root, base)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "filtered source"], root)
                committed = source_snapshot(root, base)
                self.assertEqual(before, staged)
                self.assertEqual(staged, committed)
                source.write_bytes(b"different working bytes\r\n" if filter_kind == "autocrlf"
                                   else b"SAME WORKING BYTES\n")
                if filter_kind == "custom":
                    head_blob = command(["git", "rev-parse", "HEAD:example.txt"], root).strip()
                    filtered_blob = command(["git", "hash-object", "--path=example.txt", "--", "example.txt"],
                                            root).strip()
                    self.assertEqual(head_blob, filtered_blob)
                self.assertNotEqual(committed, source_snapshot(root, base))

    def test_sha256_repository_preserves_regular_and_symlink_blob_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            command(["git", "init", "--quiet", "--object-format=sha256"], root)
            try:
                (root / ".gitattributes").write_text("*.txt filter=uppercase\n")
                clean = (f"{shlex.quote(sys.executable)} -c "
                         '"import sys; sys.stdout.write(sys.stdin.read().upper())"')
                command(["git", "config", "filter.uppercase.clean", clean], root)
                (root / "example.txt").write_text("initial content\n")
                (root / "target-a").write_text("target a\n")
                (root / "target-b").write_text("target b\n")
                (root / "link.txt").symlink_to("target-a")
            except OSError as error:
                self.skipTest(f"filesystem cannot create symbolic links: {error}")
            command(["git", "add", "-A"], root)
            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "initial SHA-256 tree"], root)
            base = command(["git", "rev-parse", "HEAD"], root).strip()

            (root / "example.txt").write_bytes(b"same working bytes\n")
            (root / "link.txt").unlink()
            (root / "link.txt").symlink_to("target-b")
            before = source_snapshot(root, base)
            command(["git", "add", "example.txt", "link.txt"], root)
            staged = source_snapshot(root, base)
            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "SHA-256 changes"], root)
            committed = source_snapshot(root, base)
            self.assertEqual(before, staged)
            self.assertEqual(staged, committed)
            self.assertEqual(command(["git", "show", "HEAD:link.txt"], root).strip(), "target-b")

            (root / "example.txt").write_bytes(b"different working bytes\n")
            self.assertNotEqual(committed, source_snapshot(root, base))
            (root / "example.txt").write_bytes(b"same working bytes\n")
            (root / "link.txt").unlink()
            (root / "link.txt").write_text("target-b")
            changed_mode = source_snapshot(root, base)
            self.assertNotEqual(committed, changed_mode)
            self.assertEqual(changed_mode["files"]["link.txt"]["kind"], "file")
            (root / "link.txt").unlink()
            deleted = source_snapshot(root, base)
            self.assertNotEqual(changed_mode, deleted)
            self.assertEqual(deleted["files"]["link.txt"], {"kind": "deleted"})

    def commit_index(self, message: str) -> None:
        command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", message], self.root)

    def test_rename_detection_cannot_hide_index_or_head_deletion(self) -> None:
        command(["git", "config", "diff.renames", "true"], self.root)
        (self.root / "new.py").write_bytes((self.root / "example.py").read_bytes())
        before = source_snapshot(self.root, self.base)
        command(["git", "rm", "--cached", "example.py"], self.root)
        command(["git", "add", "new.py"], self.root)
        staged = source_snapshot(self.root, self.base)
        self.commit_index("same content rename with restored working files")
        after = source_snapshot(self.root, self.base)
        value = {**inputs(), "source": after, "requirements": None, "gate_configuration": {}}
        compact = local_review.compact_input(self.root, value)
        observed = {"staged_deleted": "deleted\texample.py" in staged["index_overrides"],
                    "index_deleted": "deleted\texample.py" in after["index_overrides"],
                    "head_deleted": "deleted\texample.py" in after["head_overrides"],
                    "compact_head_deleted": "example.py" in compact["changes"]["head"]}
        self.assertEqual(observed, {key: True for key in observed})
        self.assertEqual(before["files"], after["files"])
        self.assertNotEqual(before, after)
        prior = {**value, "source": before}
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt_payload(prior, review(prior)), value)

    def test_compact_input_exposes_head_only_change_after_restoring_working_and_index(self) -> None:
        (self.root / "example.py").write_text("unreviewed HEAD C\n")
        command(["git", "add", "example.py"], self.root)
        self.commit_index("HEAD only change")
        command(["git", "restore", "--source", self.base, "--staged", "--worktree", "--", "example.py"], self.root)
        value = {**inputs(), "source": source_snapshot(self.root, self.base),
                 "requirements": None, "gate_configuration": {}}
        compact = local_review.compact_input(self.root, value)
        self.assertEqual(compact["changes"]["head"], ["example.py"])
        self.assertEqual(compact["changes"]["working"], [])
        self.assertEqual(compact["changes"]["staged"], [])
        self.assertTrue(value["source"]["head_overrides"])
        self.assertIn("git diff base..HEAD", local_review.review_prompt())
        self.assertIn("実際のHEAD tree", local_review.review_prompt())

    def test_unreviewed_head_with_index_restored_to_base_invalidates_receipt(self) -> None:
        path = self.root / "example.py"
        path.write_text("working B\n")
        before = source_snapshot(self.root, self.base)
        value = inputs()
        value["source"] = before
        receipt = receipt_payload(value, review(value))
        path.write_text("unreviewed C\n")
        command(["git", "add", "example.py"], self.root)
        self.commit_index("unreviewed HEAD")
        path.write_text("working B\n")
        command(["git", "restore", "--source", self.base, "--staged", "--", "example.py"], self.root)
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["files"], after["files"])
        self.assertTrue(after["index_overrides"])
        self.assertNotEqual(before, after)
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, {**value, "source": after})

    def test_head_mode_is_bound_after_index_mode_is_restored_to_base(self) -> None:
        before = source_snapshot(self.root, self.base)
        command(["git", "update-index", "--chmod=+x", "example.py"], self.root)
        self.commit_index("HEAD executable mode")
        command(["git", "restore", "--source", self.base, "--staged", "--", "example.py"], self.root)
        after = source_snapshot(self.root, self.base)
        self.assertNotEqual(before, after)
        self.assertTrue(after["head_overrides"])
        self.assertEqual(before["index_overrides"], after["index_overrides"])

    def test_head_deletion_is_bound_after_index_file_is_restored_to_base(self) -> None:
        before = source_snapshot(self.root, self.base)
        command(["git", "rm", "--cached", "example.py"], self.root)
        self.commit_index("HEAD deletion")
        command(["git", "restore", "--source", self.base, "--staged", "--", "example.py"], self.root)
        after = source_snapshot(self.root, self.base)
        self.assertNotEqual(before, after)
        self.assertIn("deleted\texample.py", after["head_overrides"])
        self.assertEqual(before["files"], after["files"])

    def test_head_new_file_is_bound_when_index_is_restored_to_base(self) -> None:
        path = self.root / "new.py"
        path.write_text("working B\n")
        before = source_snapshot(self.root, self.base)
        path.write_text("unreviewed C\n")
        command(["git", "add", "new.py"], self.root)
        self.commit_index("HEAD new file")
        path.write_text("working B\n")
        command(["git", "restore", "--source", self.base, "--staged", "--", "new.py"], self.root)
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["files"], after["files"])
        self.assertNotEqual(before, after)
        self.assertTrue(any(entry.endswith("\tnew.py") for entry in after["head_overrides"]))

    def test_exact_head_worktree_bytes_still_bind_after_index_restore_for_all_entry_kinds(self) -> None:
        for kind in ("content", "mode", "deletion", "newfile"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                (root / "example.py").write_text("original\n")
                command(["git", "add", "-A"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "initial"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()
                if kind == "content":
                    (root / "example.py").write_text("changed C\n")
                    path = "example.py"
                elif kind == "mode":
                    (root / "example.py").chmod(0o755)
                    path = "example.py"
                elif kind == "deletion":
                    command(["git", "rm", "example.py"], root)
                    path = "example.py"
                else:
                    (root / "new.py").write_text("new C\n")
                    path = "new.py"
                command(["git", "add", "-A"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", f"HEAD {kind}"], root)
                before = source_snapshot(root, base)
                command(["git", "restore", "--source", base, "--staged", "--", path], root)
                after = source_snapshot(root, base)
                self.assertEqual(before["files"], after["files"])
                self.assertEqual(before["head_overrides"], after["head_overrides"])
                self.assertFalse(before["index_overrides"])
                self.assertTrue(after["index_overrides"])
                value = {**inputs(), "source": before, "requirements": None, "gate_configuration": {}}
                with self.assertRaisesRegex(ReviewError, "another input|stale"):
                    validate_receipt(receipt_payload(value, review(value)), {**value, "source": after})

    def test_head_tracked_ignored_file_remains_in_snapshot_without_index_entry(self) -> None:
        (self.root / ".gitignore").write_text("tmp/\nignored.py\n")
        path = self.root / "ignored.py"
        path.write_text("committed content\n")
        before = source_snapshot(self.root, self.base)
        self.assertNotIn("ignored.py", before["files"])
        command(["git", "add", "--force", "ignored.py"], self.root)
        self.commit_index("HEAD tracked ignored file")
        command(["git", "rm", "--cached", "ignored.py"], self.root)
        after = source_snapshot(self.root, self.base)
        self.assertIn("ignored.py", after["files"])
        self.assertNotEqual(before, after)

    def test_committed_index_content_remains_bound_with_different_working_bytes(self) -> None:
        path = self.root / "example.py"
        path.write_text("working B\n")
        before = source_snapshot(self.root, self.base)
        value = inputs()
        value["source"] = before
        receipt = receipt_payload(value, review(value))
        path.write_text("commit C\n")
        command(["git", "add", "example.py"], self.root)
        path.write_text("working B\n")
        staged = source_snapshot(self.root, self.base)
        self.commit_index("different index bytes")
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["files"], after["files"])
        self.assertEqual(staged["index_overrides"], after["index_overrides"])
        self.assertTrue(after["head_overrides"])
        self.assertNotEqual(before, after)
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, {**value, "source": after})

    def test_committed_index_mode_remains_bound_with_same_working_mode(self) -> None:
        command(["git", "update-index", "--chmod=+x", "example.py"], self.root)
        before = source_snapshot(self.root, self.base)
        self.commit_index("index executable bit")
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["index_overrides"], after["index_overrides"])
        self.assertTrue(after["head_overrides"])
        self.assertTrue(before["index_overrides"])

    def test_committed_deletion_with_restored_file_keeps_index_tombstone(self) -> None:
        command(["git", "rm", "example.py"], self.root)
        (self.root / "example.py").write_text("restored\n")
        before = source_snapshot(self.root, self.base)
        self.commit_index("delete indexed file")
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["index_overrides"], after["index_overrides"])
        self.assertIn("deleted\texample.py", after["head_overrides"])
        self.assertIn("deleted\texample.py", before["index_overrides"])

    def test_new_index_file_keeps_distinct_working_content_after_commit(self) -> None:
        path = self.root / "new.py"
        path.write_text("index C\n")
        command(["git", "add", "new.py"], self.root)
        path.write_text("working B\n")
        before = source_snapshot(self.root, self.base)
        self.commit_index("new index file")
        after = source_snapshot(self.root, self.base)
        self.assertEqual(before["index_overrides"], after["index_overrides"])
        self.assertTrue(any(entry.endswith("\tnew.py") for entry in after["head_overrides"]))
        self.assertTrue(any(entry.endswith("\tnew.py") for entry in before["index_overrides"]))

    def test_untracked_deleted_mode_and_hidden_staged_changes_are_bound(self) -> None:
        initial = source_snapshot(self.root, self.base)
        path = self.root / "example.py"
        path.chmod(0o755)
        self.assertNotEqual(initial, source_snapshot(self.root, self.base))
        path.write_text("staged\n")
        command(["git", "add", "example.py"], self.root)
        path.write_text("unstaged\n")
        before = source_snapshot(self.root, self.base)
        self.assertTrue(before["index_overrides"])
        path.write_text("different staged\n")
        command(["git", "add", "example.py"], self.root)
        path.write_text("unstaged\n")
        self.assertNotEqual(before, source_snapshot(self.root, self.base))
        path.unlink()
        self.assertEqual(source_snapshot(self.root, self.base)["files"]["example.py"], {"kind": "deleted"})

    def test_committed_deletion_retains_baseline_tombstone(self) -> None:
        (self.root / "example.py").unlink()
        before = source_snapshot(self.root, self.base)
        self.commit("delete")
        self.assertEqual(before, source_snapshot(self.root, self.base))

    def test_staged_deletion_with_restored_untracked_file_is_not_lost(self) -> None:
        command(["git", "rm", "example.py"], self.root)
        (self.root / "example.py").write_text("restored\n")
        state = source_snapshot(self.root, self.base)
        self.assertIn("deleted\texample.py", state["index_overrides"])

    def test_matching_multi_issue_receipt_is_reused_for_run_and_check(self) -> None:
        (self.root / "example.py").write_text("first\n")
        self.commit("first change Refs #89")
        (self.root / "example.py").write_text("second\n")
        self.commit("second change Refs #95")
        value = inputs()
        value["issues"].append({"number": 95, "body_sha256": "e" * 64})
        result = review(value)
        result["issues"].append({"number": 95, "body_sha256": "e" * 64, "result": "verified",
                                 "evidence": copy.deepcopy(result["issues"][0]["evidence"])})
        payload = receipt_payload(value, result)
        validate_receipt(payload, value)
        receipt = self.root / "tmp" / "receipt.json"
        receipt.parent.mkdir()
        receipt.write_text(json.dumps(payload))
        receipt.with_suffix(".review.json").write_text(json.dumps(result))
        args = SimpleNamespace(issue=[], base=self.base, requirements=None, receipt="tmp/receipt.json",
                               check_receipt=False, print_input=False)

        def git_command(arguments: list[str], root: Path) -> str:
            return subprocess.run(arguments, cwd=root, check=True, capture_output=True, text=True).stdout

        for check_receipt in (False, True):
            with self.subTest(check_receipt=check_receipt), \
                    patch.dict(os.environ, {}, clear=True), \
                    patch.object(local_review, "command", side_effect=git_command), \
                    patch.object(local_review, "repository_root", return_value=self.root), \
                    patch.object(local_review, "cache_path", return_value=receipt), \
                    patch.object(local_review, "build_inputs", return_value=value), \
                    patch.object(local_review, "invoke_review") as invoke:
                args.check_receipt = check_receipt
                self.assertEqual(local_review.run(args), 0)
                invoke.assert_not_called()

    def test_multi_issue_receipt_mismatch_or_invalid_receipt_requires_explicit_issue(self) -> None:
        (self.root / "example.py").write_text("first\n")
        self.commit("first change Refs #89")
        (self.root / "example.py").write_text("second\n")
        self.commit("second change Refs #95")
        args = SimpleNamespace(issue=[], base=self.base)

        def git_command(arguments: list[str], root: Path) -> str:
            return subprocess.run(arguments, cwd=root, check=True, capture_output=True, text=True).stdout

        receipt = self.root / "tmp" / "receipt.json"
        value = inputs()
        value["issues"].append({"number": 95, "body_sha256": "e" * 64})
        result = review(value)
        result["issues"].append({"number": 95, "body_sha256": "e" * 64, "result": "verified",
                                 "evidence": copy.deepcopy(result["issues"][0]["evidence"])})
        valid_payload = receipt_payload(value, result)
        receipt.parent.mkdir(exist_ok=True)
        receipt.write_text(json.dumps(valid_payload))
        sidecar = receipt.with_suffix(".review.json")
        for label, original in (("missing sidecar", None), ("corrupt sidecar", "{"),
                                ("mismatched sidecar", json.dumps({**result, "summary": "changed"}))):
            with self.subTest(sidecar=label), \
                    patch.dict(os.environ, {}, clear=True), \
                    patch.object(local_review, "command", side_effect=git_command), \
                    patch.object(local_review, "repository_root", return_value=self.root), \
                    patch.object(local_review, "cache_path", return_value=receipt), \
                    patch.object(local_review, "invoke_review") as invoke:
                if original is None:
                    sidecar.unlink(missing_ok=True)
                else:
                    sidecar.write_text(original)
                with self.assertRaisesRegex(ReviewError, "multiple branch Issues"):
                    local_review.issue_numbers(self.root, args, receipt)
                run_args = SimpleNamespace(issue=[], base=self.base, requirements=None, receipt="tmp/receipt.json",
                                           check_receipt=False, print_input=False)
                with self.assertRaisesRegex(ReviewError, "multiple branch Issues"):
                    local_review.run(run_args)
                invoke.assert_not_called()

        receipt.write_text(json.dumps(receipt_payload(inputs(), review(inputs()))))
        sidecar.write_text(json.dumps(review(inputs())))
        for contents in (json.dumps(receipt_payload(inputs(), review(inputs()))), "{", "{}"):
            with self.subTest(receipt=contents), \
                    patch.dict(os.environ, {}, clear=True), \
                    patch.object(local_review, "command", side_effect=git_command):
                receipt.write_text(contents)
                with self.assertRaisesRegex(ReviewError, "multiple branch Issues"):
                    local_review.issue_numbers(self.root, args, receipt)
        receipt.unlink()
        with patch.dict(os.environ, {}, clear=True), \
                patch.object(local_review, "command", side_effect=git_command):
            with self.assertRaisesRegex(ReviewError, "multiple branch Issues"):
                local_review.issue_numbers(self.root, args, receipt)

    def test_receipt_only_issue_and_requirements_recovery_validates_original_review(self) -> None:
        requirement = self.root / "requirements.md"
        requirement.write_text("receipt-only requirements\n")
        self.commit("add retained requirements fixture")
        root = self.root.resolve()
        value = inputs()
        value["requirements"] = local_review.requirements_context(root, "requirements.md")
        result = review(value)
        receipt = self.root / "tmp" / "receipt.json"
        receipt.parent.mkdir(exist_ok=True)
        receipt.write_text(json.dumps(receipt_payload(value, result)))
        sidecar = receipt.with_suffix(".review.json")
        sidecar.write_text(json.dumps(result))
        run_args = SimpleNamespace(issue=[], base=self.base, requirements=None, receipt="tmp/receipt.json",
                                   check_receipt=False, print_input=False)
        requirements_args = SimpleNamespace(requirements=None)

        def git_command(arguments: list[str], root: Path) -> str:
            return subprocess.run(arguments, cwd=root, check=True, capture_output=True, text=True).stdout

        with patch.dict(os.environ, {}, clear=True), \
                patch.object(local_review, "command", side_effect=git_command), \
                patch.object(local_review, "repository_root", return_value=root), \
                patch.object(local_review, "cache_path", return_value=receipt), \
                patch.object(local_review, "build_inputs", return_value=value), \
                patch.object(local_review, "invoke_review") as invoke:
            self.assertEqual(local_review.issue_numbers(self.root, run_args, receipt), [89])
            self.assertEqual(local_review.requirements_path(root, requirements_args, receipt, [89]),
                             "requirements.md")
            self.assertEqual(local_review.run(run_args), 0)
            invoke.assert_not_called()

        for label, original in (("missing", None), ("corrupt", "{"),
                                ("mismatch", json.dumps({**result, "summary": "changed"}))):
            with self.subTest(original=label), \
                    patch.dict(os.environ, {}, clear=True), \
                    patch.object(local_review, "command", side_effect=git_command), \
                    patch.object(local_review, "repository_root", return_value=root), \
                    patch.object(local_review, "cache_path", return_value=receipt), \
                    patch.object(local_review, "build_inputs", return_value=value), \
                    patch.object(local_review, "invoke_review") as invoke:
                if original is None:
                    sidecar.unlink(missing_ok=True)
                else:
                    sidecar.write_text(original)
                with self.assertRaises((OSError, ReviewError)):
                    local_review.issue_numbers(self.root, run_args, receipt)
                with self.assertRaises((OSError, ReviewError)):
                    local_review.requirements_path(root, requirements_args, receipt, [89])
                with self.assertRaises((OSError, ReviewError)):
                    local_review.run(run_args)
                invoke.assert_not_called()


class DriverContractTest(unittest.TestCase):
    def test_actual_just_overrides_reach_review_and_quality_runner(self) -> None:
        repository = Path(__file__).resolve().parents[2]
        names = ("COVERAGE_MIN_LINES", "COVERAGE_MAX_UNCOVERED_LINES", "TEST_THREADS",
                 "RUSTFLAGS", "CARGO", "JOBS", "CHECK_JOBS")
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {"PATH": os.environ["PATH"], "CI": "true"}, clear=True):
            root = Path(temporary).resolve()
            (root / "Justfile").write_text((repository / "Justfile").read_text())
            (root / "Cargo.toml").write_text('[package]\nversion = "0.4.22"\n')
            hooks = root / "scripts" / "hooks"
            hooks.mkdir(parents=True)
            cache_script = root / "scripts" / "plantuml" / "cache-dir.sh"
            cache_script.parent.mkdir()
            cache_script.write_text((repository / "scripts/plantuml/cache-dir.sh").read_text())
            common = f"import json,sys\nfrom pathlib import Path\nsys.path.insert(0, {str(repository / 'scripts/hooks')!r})\nfrom local_review_state import gate_configuration\n"
            (hooks / "local_review.py").write_text(common + "Path('review.json').write_text(json.dumps(gate_configuration(Path.cwd())))\n")
            (hooks / "run_parallel_checks.py").write_text(common + "Path('runner.json').write_text(json.dumps({'config':gate_configuration(Path.cwd()),'jobs':sys.argv[-1]}))\n")
            def run(overrides):
                command(["just", "--justfile", str(root / "Justfile"), *overrides, "check"], root)
                reviewed = json.loads((root / "review.json").read_text())
                executed = json.loads((root / "runner.json").read_text())
                self.assertEqual(reviewed, executed["config"])
                self.assertEqual(reviewed["CHECK_JOBS"], executed["jobs"])
                return reviewed
            baseline = run([])
            self.assertEqual(baseline, local_review.gate_configuration(root))
            explicit = [argument for name in names if name != "RUSTFLAGS" for argument in ("--set", name, baseline[name])]
            explicit.append(f"RUSTFLAGS={baseline['RUSTFLAGS']}")
            self.assertEqual(baseline, run(explicit))
            for name, changed in [("CHECK_JOBS", "1"), ("TEST_THREADS", "2"), ("CARGO", "cargo --offline"),
                                  ("JOBS", "4"), ("RUSTFLAGS", "-D warnings -C opt-level=1"),
                                  ("COVERAGE_MIN_LINES", "100.0"), ("COVERAGE_MAX_UNCOVERED_LINES", "00")]:
                with self.subTest(name=name):
                    override = [f"RUSTFLAGS={changed}"] if name == "RUSTFLAGS" else ["--set", name, changed]
                    value = run(override)
                    self.assertEqual(value[name], changed)
                    self.assertNotEqual(digest(baseline), digest(value))

    def test_coverage_target_paths_are_effective_and_normalized(self) -> None:
        from local_review_state import gate_configuration
        root = Path(__file__).resolve().parents[2]
        with patch("local_review_state.command", return_value="value\n"), patch.dict(os.environ, {}, clear=True):
            before = gate_configuration(root)
            self.assertEqual(before["CARGO_LLVM_COV_TARGET_DIR"], str(root / "target/llvm-cov-target"))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_BUILD_DIR": "tmp/unused-without-target"}):
                self.assertEqual(before, gate_configuration(root))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": "target/llvm-cov-target"}):
                self.assertEqual(before, gate_configuration(root))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": "tmp/coverage-target"}):
                changed = gate_configuration(root)
                self.assertNotEqual(digest(before), digest(changed))
                with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": str(root / "tmp/coverage-target")}):
                    self.assertEqual(changed, gate_configuration(root))
                with patch.dict(os.environ, {"CARGO_LLVM_COV_BUILD_DIR": "tmp/coverage-build"}):
                    self.assertNotEqual(changed, gate_configuration(root))
                    self.assertEqual(gate_configuration(root)["CARGO_LLVM_COV_BUILD_DIR"], str(root / "tmp/coverage-build"))

    def test_effective_gate_settings_invalidate_receipt_and_defaults_reuse(self) -> None:
        root = Path(__file__).resolve().parents[2]
        args = SimpleNamespace(base="origin/master", requirements=None)
        with patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]):
            with patch.dict(os.environ, {"PATH": os.environ["PATH"], "CI": "true"}, clear=True):
                baseline = local_review.build_inputs(root, args, [89])
            for name, changed in [("COVERAGE_MIN_LINES", "95"), ("COVERAGE_MAX_UNCOVERED_LINES", "2"),
                                  ("TEST_THREADS", "8"), ("RUSTFLAGS", ""), ("CARGO", "cargo --offline"),
                                  ("CARGO_BUILD_TARGET", "aarch64-apple-darwin")]:
                with self.subTest(name=name), patch.dict(os.environ, {"PATH": os.environ["PATH"], "CI": "true", name: changed}, clear=True):
                    updated = local_review.build_inputs(root, args, [89])
                    self.assertNotEqual(baseline, updated)
                    with self.assertRaisesRegex(ReviewError, "another input"):
                        validate_receipt(receipt_payload(baseline, review(baseline)), updated)
            defaults = {"PATH": os.environ["PATH"], "CI": "true", "COVERAGE_MIN_LINES": "100",
                        "COVERAGE_MAX_UNCOVERED_LINES": "0", "TEST_THREADS": "1", "RUSTFLAGS": "-D warnings",
                        "GITHUB_TOKEN": "secret-not-an-input"}
            with patch.dict(os.environ, defaults, clear=True):
                self.assertEqual(baseline, local_review.build_inputs(root, args, [89]))
            self.assertNotIn("GITHUB_TOKEN", json.dumps(baseline))

    def test_requirements_are_retained_for_same_task_and_rehashed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("native behavior\n")
            value = inputs()
            value["requirements"] = local_review.requirements_context(root, "requirements.md")
            receipt = root / "receipt.json"
            result = review(value)
            receipt.write_text(json.dumps(receipt_payload(value, result)))
            receipt.with_suffix(".review.json").write_text(json.dumps(result))
            args = SimpleNamespace(requirements=None)
            selected = local_review.requirements_path(root, args, receipt, [89])
            self.assertEqual(selected, "requirements.md")
            self.assertEqual(value["requirements"], local_review.requirements_context(root, selected))
            requirement.write_text("changed native requirement\n")
            self.assertNotEqual(value["requirements"], local_review.requirements_context(root, selected))
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))
            with patch.dict(os.environ, {"REVIEW_REQUIREMENTS": "environment.md"}):
                self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "environment.md")
                args.requirements = "explicit.md"
                self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "explicit.md")
            args.requirements = None
            requirement.unlink()
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])
            value["requirements"] = {"content": "lost path", "sha256": "a" * 64}
            result = review(value)
            receipt.write_text(json.dumps(receipt_payload(value, result)))
            receipt.with_suffix(".review.json").write_text(json.dumps(result))
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])

    def test_standard_check_reuses_required_input_and_rejects_changed_requirement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("native requirement\n")
            path = root / "tmp" / "receipt.json"
            path.parent.mkdir()
            args = SimpleNamespace(issue=[89], base="origin/master", requirements="requirements.md",
                                   receipt="tmp/receipt.json", print_input=False, check_receipt=True)
            with patch.object(local_review, "repository_root", return_value=root), patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]), patch.object(local_review, "gate_configuration", return_value={"TEST_THREADS": "1"}), patch.object(local_review, "invoke_review") as invoke:
                value = local_review.build_inputs(root, args, [89])
                result = review(value)
                path.write_text(json.dumps(receipt_payload(value, result)))
                path.with_suffix(".review.json").write_text(json.dumps(result))
                args.requirements = None
                self.assertEqual(local_review.run(args), 0)
                self.assertEqual(args.requirements, "requirements.md")
                invoke.assert_not_called()
                requirement.write_text("different requirement\n")
                args.requirements = None
                with self.assertRaisesRegex(ReviewError, "receipt rejected"):
                    local_review.run(args)
                invoke.assert_not_called()

    def test_ci_and_recursion_do_not_start_codex(self) -> None:
        args = SimpleNamespace()
        for flag in ("CI", "GITHUB_ACTIONS"):
            with patch.dict(os.environ, {flag: "true"}, clear=True), patch.object(local_review, "invoke_review") as run:
                self.assertEqual(local_review.run(args), 0)
                run.assert_not_called()
        with patch.dict(os.environ, {"KRR_LOCAL_REVIEW_ACTIVE": "1"}, clear=True):
            with self.assertRaisesRegex(ReviewError, "nested"):
                local_review.run(args)

    def test_command_uses_rtk_locally_and_raw_tool_in_ci_when_rtk_is_absent(self) -> None:
        completed = subprocess.CompletedProcess(["git"], 0, stdout="ok\n", stderr="")
        with patch("local_review_state.shutil.which", return_value="/bin/rtk"), patch("local_review_state.subprocess.run", return_value=completed) as run:
            with patch.dict(os.environ, {}, clear=True):
                self.assertEqual(command(["git", "status"], Path.cwd()), "ok\n")
            self.assertEqual(run.call_args.args[0], ["/bin/rtk", "proxy", "git", "status"])
        with patch("local_review_state.shutil.which", return_value=None), patch("local_review_state.subprocess.run", return_value=completed) as run:
            with patch.dict(os.environ, {"CI": "true"}, clear=True):
                self.assertEqual(command(["git", "status"], Path.cwd()), "ok\n")
            self.assertEqual(run.call_args.args[0], ["git", "status"])
        with patch("local_review_state.shutil.which", return_value=None), patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ReviewError, "rtk is required"):
                command(["git", "status"], Path.cwd())
        for flag in ("CI", "GITHUB_ACTIONS"):
            with patch("local_review_state.shutil.which", return_value=None), patch.dict(os.environ, {flag: "false"}, clear=True):
                with self.assertRaisesRegex(ReviewError, "rtk is required"):
                    command(["git", "status"], Path.cwd())

    def test_failed_initial_review_preserves_requirements_and_issue_without_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("required native semantics\n")
            value = inputs()
            value["requirements"] = local_review.requirements_context(root, "requirements.md")
            result = review(value)
            result["verdict"] = "FAIL"
            child = root / "codex-test"
            child.mkdir()
            def returned(arguments, *_options):
                Path(arguments[arguments.index("--output-last-message") + 1]).write_text(json.dumps(result))
                return 0
            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", side_effect=returned):
                with self.assertRaisesRegex(ReviewError, "findings retained"):
                    local_review.invoke_review(root, child, value)
            receipt = root / "receipt.json"
            self.assertFalse(receipt.exists())
            args = SimpleNamespace(issue=[], base="origin/master", requirements=None)
            self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "requirements.md")
            with patch.object(local_review, "command", return_value=""):
                self.assertEqual(local_review.issue_numbers(root, args, receipt), [89])
            with patch.object(local_review, "command", return_value="Refs #120"):
                self.assertEqual(local_review.issue_numbers(root, args, receipt), [120])
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))
            requirement.unlink()
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])
            value["requirements"]["path"] = "different.md"
            (root / "last-input.json").write_text(json.dumps(value))
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [89]))
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))

    def test_first_cli_failure_allows_ordinary_retry_with_explicit_context(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("required native semantics\n")
            receipt = root / "tmp" / "receipt.json"
            args = SimpleNamespace(issue=[89], base="origin/master", requirements=None,
                                   receipt="tmp/receipt.json", print_input=False, check_receipt=False)
            with patch.object(local_review, "repository_root", return_value=root), \
                    patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), \
                    patch.object(local_review, "issue_context", return_value=inputs()["issues"]), \
                    patch.object(local_review, "gate_configuration", return_value={}):
                value = local_review.build_inputs(root, args, [89])
                child = root / "codex-test"
                child.mkdir(parents=True)
                with patch.object(local_review, "compact_input", return_value={}), \
                        patch.object(local_review, "run_review_process", return_value=2):
                    with self.assertRaisesRegex(ReviewError, "exit 2"):
                        local_review.obtain_receipt(root, args, receipt, value, [89])
                self.assertTrue((receipt.parent / "last-input.json").is_file())
                self.assertFalse((receipt.parent / "last-review.json").exists())
                self.assertIsNone(local_review.retained_inputs(receipt))

                result = review(value)
                def successful_review(arguments, *_options):
                    result_path = Path(arguments[arguments.index("--output-last-message") + 1])
                    result_path.write_text(json.dumps(result))
                    return 0

                with patch.object(local_review, "compact_input", return_value={}), \
                        patch.object(local_review, "run_review_process", side_effect=successful_review):
                    self.assertEqual(local_review.run(args), 0)
                self.assertTrue(receipt.is_file())
                self.assertEqual(json.loads(receipt.read_text())["review"], result)

    def test_stale_retained_review_is_not_reused_for_new_review(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("required native semantics\n")
            receipt = root / "tmp" / "receipt.json"
            receipt.parent.mkdir(parents=True)
            args = SimpleNamespace(issue=[89], base="origin/master", requirements=None,
                                   receipt="tmp/receipt.json", print_input=False, check_receipt=False)
            with patch.object(local_review, "repository_root", return_value=root), \
                    patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), \
                    patch.object(local_review, "issue_context", return_value=inputs()["issues"]), \
                    patch.object(local_review, "gate_configuration", return_value={}):
                current = local_review.build_inputs(root, args, [89])
                stale = copy.deepcopy(current)
                stale["source"] = {**stale["source"], "files": {"old.py": {"sha256": "e" * 64}}}
                (receipt.parent / "last-input.json").write_text(json.dumps(stale))
                (receipt.parent / "last-review.json").write_text(json.dumps(review(current)))
                self.assertIsNone(local_review.retained_inputs(receipt, [89]))
                fresh = review(current)
                with patch.object(local_review, "invoke_review", return_value=fresh) as invoke:
                    self.assertEqual(local_review.run(args), 0)
                invoke.assert_called_once()
                self.assertEqual(json.loads(receipt.read_text())["review"], fresh)

    def test_malformed_or_nonobject_retained_review_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt = root / "receipt.json"
            (root / "last-input.json").write_text(json.dumps(inputs()))
            for raw in ("{bad", "[]"):
                with self.subTest(raw=raw):
                    (root / "last-review.json").write_text(raw)
                    with self.assertRaisesRegex(ReviewError, "invalid review JSON|structured object|does not match"):
                        local_review.retained_inputs(receipt)

    def test_failed_findings_survive_temporary_review_directory(self) -> None:
        value = inputs()
        result = review(value)
        result["verdict"] = "FAIL"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            child = root / "codex-test"
            child.mkdir()

            def returned(arguments, *_options):
                Path(arguments[arguments.index("--output-last-message") + 1]).write_text(json.dumps(result))
                return 0

            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", side_effect=returned):
                with self.assertRaisesRegex(ReviewError, "findings retained"):
                    local_review.invoke_review(root, child, value)
            self.assertEqual(json.loads((root / "last-review.json").read_text()), result)
            self.assertEqual(json.loads((root / "last-input.json").read_text()), value)

    def test_cli_error_does_not_create_a_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            child = root / "codex-test"
            child.mkdir()
            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", return_value=2):
                with self.assertRaisesRegex(ReviewError, "exit 2"):
                    local_review.invoke_review(root, child, inputs())
            self.assertFalse((root / "receipt.json").exists())


    def test_prompt_scope_and_finite_review_budget_are_input_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = SimpleNamespace(base="origin/master", requirements=None)
            with patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]), patch.object(local_review, "gate_configuration", return_value={}):
                before = local_review.build_inputs(root, args, [89])
                with patch.object(local_review, "REVIEW_TIMEOUT_SECONDS", 1801):
                    after = local_review.build_inputs(root, args, [89])
                self.assertNotEqual(before["prompt_sha256"], after["prompt_sha256"])
                with self.assertRaisesRegex(ReviewError, "another input|stale"):
                    validate_receipt(receipt_payload(before, review(before)), after)
            prompt = local_review.review_prompt()
            self.assertLess(local_review.REVIEW_ANALYSIS_SECONDS, local_review.REVIEW_TIMEOUT_SECONDS)
            self.assertIn("blocked/FAIL", prompt)
            self.assertIn("runtime 60秒/close 5秒", prompt)
            self.assertTrue(all(check in prompt for check in CHECKS))

    def test_review_request_carries_deadline_and_timeout_rejects_receipt(self) -> None:
        value = inputs()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt = root / "receipt.json"
            args = SimpleNamespace()
            def timed_out(arguments, _root, _environment, prompt, diagnostic):
                payload = json.loads(prompt.split("固定入力:\n", 1)[1])
                budget = payload["review_budget"]
                self.assertEqual(budget["process_timeout_seconds"], 1800)
                self.assertEqual(budget["analysis_seconds"], 1500)
                self.assertIn("+00:00", budget["analysis_deadline_utc"])
                diagnostic.write_text("partial review; not a PASS")
                raise subprocess.TimeoutExpired(arguments, 1800)
            with patch.object(local_review, "compact_input", return_value={"input_sha256": digest(value)}), patch.object(local_review, "run_review_process", side_effect=timed_out):
                with self.assertRaisesRegex(ReviewError, "exceeded 1800s; no receipt accepted; inspect"):
                    local_review.obtain_receipt(root, args, receipt, value, [89])
            self.assertFalse(receipt.exists())
            self.assertFalse(receipt.with_suffix(".review.json").exists())
            self.assertFalse((root / "review.lock").exists())
            self.assertEqual(json.loads((root / "last-input.json").read_text()), value)
            self.assertEqual((root / "last-codex-stderr.log").read_text(), "partial review; not a PASS")

    @unittest.skipUnless(os.name == "posix", "POSIX process groups")
    def test_timeout_kills_review_process_group_and_reaps_wrapper(self) -> None:
        process = unittest.mock.MagicMock()
        process.pid = 12345
        process.communicate.side_effect = [subprocess.TimeoutExpired("codex", 1800), (None, None)]
        process.__enter__.return_value = process
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(subprocess, "Popen", return_value=process) as launch, patch.object(os, "killpg") as kill:
                with self.assertRaises(subprocess.TimeoutExpired):
                    local_review.run_review_process(["codex"], root, {}, "review", root / "stderr.log")
            self.assertTrue(launch.call_args.kwargs["start_new_session"])
            kill.assert_called_once_with(12345, local_review.signal.SIGKILL)
            self.assertEqual(process.communicate.call_count, 2)

    def test_real_review_process_timeout_retains_stderr(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            diagnostic = root / "stderr.log"
            script = "import sys,time; print('partial review', file=sys.stderr, flush=True); time.sleep(10)"
            with patch.object(local_review, "REVIEW_TIMEOUT_SECONDS", 0.2):
                with self.assertRaises(subprocess.TimeoutExpired):
                    local_review.run_review_process([sys.executable, "-c", script], root, dict(os.environ), "review", diagnostic)
            self.assertIn("partial review", diagnostic.read_text())


    def test_same_issue_body_does_not_expire_on_comment_timestamp(self) -> None:
        payload = {"number": 89, "title": "quality", "body": "requirements", "state": "open",
                   "html_url": "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/89",
                   "updated_at": "before"}
        responses = ["git@github.com:HiroyukiFuruno/katana-render-runtime.git", json.dumps(payload)]
        with patch("local_review_state.command", side_effect=responses):
            before = issue_context(Path.cwd(), [89])
        payload["updated_at"] = "after comment"
        responses[-1] = json.dumps(payload)
        with patch("local_review_state.command", side_effect=responses):
            self.assertEqual(before, issue_context(Path.cwd(), [89]))

    def test_github_origin_url_forms_share_the_same_issue_context(self) -> None:
        payload = {"number": 89, "title": "quality", "body": "requirements", "state": "open",
                   "html_url": "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/89"}
        origins = (
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com:22/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://g%69t@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github%2ecom/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com:%32%32/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno%2fkatana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime%2egit",
        )
        observed = []
        for origin in origins:
            with self.subTest(origin=origin), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "remote", "add", "origin", origin], root)
                gh_calls = []

                def real_git_mock_gh(arguments, cwd, input_bytes=None):
                    if arguments[0] == "gh":
                        gh_calls.append(arguments)
                        return json.dumps(payload)
                    return command(arguments, cwd, input_bytes)

                with patch("local_review_state.command", side_effect=real_git_mock_gh):
                    observed.append(issue_context(root, [89]))
                self.assertEqual(len(gh_calls), 1)
                self.assertEqual(gh_calls[0], ["gh", "api", "repos/HiroyukiFuruno/katana-render-runtime/issues/89"])
        self.assertTrue(all(value == observed[0] for value in observed[1:]))

    def test_invalid_ssh_origins_are_rejected_before_github_api_call(self) -> None:
        invalid_origins = (
            "ssh://git@not-github.example/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/OtherOwner/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/nested/katana-render-runtime.git",
            "ssh://other@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git:secret@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com:2222/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git?query=1",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git#fragment",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git?",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git#",
            "ssh://git@github.com:/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%0a",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%00",
            "ssh://git@not-github%2eexample/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://g%69t@github.com/Other%4fwner/katana-render-runtime.git",
            "ssh://other%40git@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git%3apassword@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com:%32%32%32%32/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno%2fnested%2fkatana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render%252druntime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%3f",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%23",
            "ssh://git@github.com%3a/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://g%2569t@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%FF",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git%G1",
        )
        for origin in invalid_origins:
            with self.subTest(origin=origin), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "remote", "add", "origin", origin], root)
                gh_calls = []

                def real_git_record_gh(arguments, cwd, input_bytes=None):
                    if arguments[0] == "gh":
                        gh_calls.append(arguments)
                        return "{}"
                    return command(arguments, cwd, input_bytes)

                with patch("local_review_state.command", side_effect=real_git_record_gh):
                    with self.assertRaisesRegex(ReviewError, "GitHub repository|scoped"):
                        issue_context(root, [89])
                self.assertFalse(gh_calls)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            gh_calls = []

            def malformed_remote(arguments, cwd, input_bytes=None):
                if arguments == ["git", "remote", "get-url", "origin"]:
                    return "ssh://git@[github.com/HiroyukiFuruno/katana-render-runtime.git\n"
                if arguments[0] == "gh":
                    gh_calls.append(arguments)
                    return "{}"
                return command(arguments, cwd, input_bytes)

            with patch("local_review_state.command", side_effect=malformed_remote):
                with self.assertRaisesRegex(ReviewError, "GitHub repository"):
                    issue_context(root, [89])
            self.assertFalse(gh_calls)

        invalid_remote_outputs = (
            "ssh://git@github\t.com/HiroyukiFuruno/katana-render-runtime.git\n",
            "ssh://git@github.com/HiroyukiFuruno/\r\nkatana-render-runtime.git\n",
            "\tssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\n",
            "\rssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\n",
            " ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\n",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\t\n",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\r\n",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git \n",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\x00\n",
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\x7f\n",
        )
        for remote_output in invalid_remote_outputs:
            with self.subTest(remote_output=repr(remote_output)), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                gh_calls = []

                def injected_git_output(arguments, cwd, input_bytes=None):
                    if arguments == ["git", "remote", "get-url", "origin"]:
                        return remote_output
                    if arguments[0] == "gh":
                        gh_calls.append(arguments)
                        return "{}"
                    return command(arguments, cwd, input_bytes)

                # NUL は Git 設定へ保存できないため、command 出力の境界で検証する。
                with patch("local_review_state.command", side_effect=injected_git_output):
                    with self.assertRaisesRegex(ReviewError, "GitHub repository|scoped"):
                        issue_context(root, [89])
                self.assertFalse(gh_calls)

    def test_real_git_control_char_origins_are_rejected_before_github_api_call(self) -> None:
        invalid_origins = (
            "ssh://git@github.com/HiroyukiFuruno/katana-render-runtime.git\r",
            "ssh://git@github.com/HiroyukiFuruno/\r\nkatana-render-runtime.git",
        )
        for origin in invalid_origins:
            with self.subTest(origin=repr(origin)), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "config", "remote.origin.url", origin], root)
                git_arguments = ["git", "remote", "get-url", "origin"]
                rtk = shutil.which("rtk")
                if rtk is not None:
                    git_arguments = [rtk, "proxy", *git_arguments]
                raw_stdout = subprocess.run(git_arguments, cwd=root, capture_output=True,
                                            check=True, text=False).stdout
                normalized_stdout = subprocess.run(git_arguments, cwd=root, capture_output=True,
                                                   check=True, text=True).stdout
                if origin.endswith("\r"):
                    self.assertTrue(raw_stdout.endswith(b".git\r\n"))
                    self.assertTrue(normalized_stdout.endswith(".git\n"))
                    self.assertFalse(normalized_stdout.endswith(".git\r\n"))
                else:
                    self.assertIn(b"/\r\nkatana-render-runtime.git\n", raw_stdout)
                    self.assertIn("/\nkatana-render-runtime.git\n", normalized_stdout)
                    self.assertNotIn("/\r\nkatana-render-runtime.git", normalized_stdout)
                gh_calls = []

                def real_git_record_gh(arguments, cwd, input_bytes=None):
                    if arguments[0] == "gh":
                        gh_calls.append(arguments)
                        return "{}"
                    return command(arguments, cwd, input_bytes)

                with patch("local_review_state.command", side_effect=real_git_record_gh):
                    with self.assertRaisesRegex(ReviewError, "GitHub repository|scoped"):
                        issue_context(root, [89])
                self.assertFalse(gh_calls)

    def test_codex_is_read_only_high_and_precedes_quality_lanes(self) -> None:
        arguments = local_review.review_command(Path.cwd(), Path("result.json"))
        self.assertIn("read-only", arguments)
        self.assertIn("gpt-6.1-sol", arguments)
        self.assertIn('model_reasoning_effort="high"', arguments)
        self.assertNotIn("--worktree", arguments)
        root = Path(__file__).resolve().parents[2]
        justfile = (root / "Justfile").read_text()
        section = justfile.split("\ncheck:\n", 1)[1].split("\n\n", 1)[0]
        self.assertLess(section.index("scripts/hooks/local_review.py"), section.index("run_parallel_checks.py"))

    def test_new_branch_issue_precedes_old_receipt_and_ambiguous_refs_reject(self) -> None:
        args = SimpleNamespace(issue=[], base="origin/master")
        with tempfile.TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "receipt.json"
            receipt.write_text(json.dumps(receipt_payload(inputs(), review(inputs()))))
            with patch.dict(os.environ, {}, clear=True), patch.object(local_review, "command", return_value="fix\nRefs #120"):
                self.assertEqual(local_review.issue_numbers(Path.cwd(), args, receipt), [120])
            with patch.dict(os.environ, {}, clear=True), patch.object(local_review, "command", return_value="Refs #120\nRefs #121"):
                with self.assertRaisesRegex(ReviewError, "multiple"):
                    local_review.issue_numbers(Path.cwd(), args, receipt)

    def test_explicit_issue_and_environment_precede_inference(self) -> None:
        args = SimpleNamespace(issue=[123], base="origin/master")
        with patch.dict(os.environ, {"REVIEW_ISSUE": "124"}, clear=True):
            self.assertEqual(local_review.issue_numbers(Path.cwd(), args, Path("absent")), [123])
            args.issue = []
            self.assertEqual(local_review.issue_numbers(Path.cwd(), args, Path("absent")), [124])

    def test_original_structured_result_tamper_is_rejected(self) -> None:
        value = inputs()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            path.write_text(json.dumps(receipt_payload(value, review(value))))
            original = review(value)
            original["summary"] = "modified original"
            path.with_suffix(".review.json").write_text(json.dumps(original))
            with self.assertRaisesRegex(ReviewError, "original structured"):
                local_review.read_receipt(path, value)


if __name__ == "__main__":
    unittest.main()
