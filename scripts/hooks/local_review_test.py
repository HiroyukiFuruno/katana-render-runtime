from __future__ import annotations

import copy
from contextlib import redirect_stdout
import errno
import io
import json
import os
from pathlib import Path
import signal
import shutil
import shlex
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parent))

import local_review
from local_review_contract import CHECKS, receipt_payload, validate_receipt
from local_review_lock import review_lock
from local_review_state import (
    ReviewError,
    command,
    digest,
    file_record,
    issue_context,
    source_snapshot,
    strict_json,
    working_entry,
)


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

    def test_print_input_escapes_surrogate_git_paths(self) -> None:
        value = {"source": {"files": {"bad\udcff": {"kind": "deleted"}}}}
        args = SimpleNamespace(issue=[89], base="HEAD", requirements=None,
                               receipt="tmp/receipt.json", print_input=True, check_receipt=False)
        output = io.StringIO()
        root = Path.cwd()
        with patch.dict(os.environ, {}, clear=True), redirect_stdout(output), \
                patch.object(local_review, "repository_root", return_value=root), \
                patch.object(local_review, "cache_path", return_value=root / "tmp/receipt.json"), \
                patch.object(local_review, "issue_numbers", return_value=[89]), \
                patch.object(local_review, "requirements_path", return_value=None), \
                patch.object(local_review, "build_inputs", return_value=value):
            self.assertEqual(local_review.run(args), 0)
        encoded = output.getvalue().encode("ascii")
        self.assertIn(b"bad\\udcff", encoded)


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

    @unittest.skipUnless(os.name == "posix", "POSIX file permission bits are required")
    def test_real_git_reset_one_staged_path_then_commit_another_invalidates_receipt(self) -> None:
        for kind in ("modified", "deleted", "executable"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "config", "core.filemode", "true"], root)
                first = root / "first.py"
                second = root / "second.py"
                first.write_text("first baseline\n")
                second.write_text("second baseline\n")
                command(["git", "add", "-A"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "baseline"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()

                if kind == "modified":
                    first.write_text("first working change\n")
                elif kind == "deleted":
                    first.unlink()
                else:
                    first.chmod(0o755)
                second.write_text("second working change\n")
                command(["git", "add", "-A"], root)
                before = source_snapshot(root, base)
                value = {**inputs(), "source": before}
                receipt = receipt_payload(value, review(value))

                command(["git", "reset", "--quiet", "--", "first.py"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "commit second", "--",
                         "second.py"], root)
                after = source_snapshot(root, base)

                self.assertEqual(before["files"], after["files"])
                self.assertEqual(before["working_blobs"], after["working_blobs"])
                self.assertNotEqual(before, after)
                self.assertEqual(after["omitted_working_paths"], ["first.py"])
                with self.assertRaisesRegex(ReviewError, "another input|stale"):
                    validate_receipt(receipt, {**value, "source": after})

    def test_real_git_stage_and_commit_all_tracked_changes_reuses_receipt(self) -> None:
        first = self.root / "first.py"
        second = self.root / "second.py"
        first.write_text("first working change\n")
        second.write_text("second working change\n")
        command(["git", "add", "-A"], self.root)
        staged = source_snapshot(self.root, self.base)
        value = {**inputs(), "source": staged}
        receipt = receipt_payload(value, review(value))

        self.commit("commit all tracked changes")
        committed = source_snapshot(self.root, self.base)

        self.assertEqual(staged, committed)
        self.assertEqual(validate_receipt(receipt, {**value, "source": committed})["verdict"], "PASS")

    def test_hidden_git_index_flags_reject_review_candidates(self) -> None:
        flag_cases = (
            ("assume-unchanged", ("--assume-unchanged",), ("--no-assume-unchanged",), "h", "H"),
            ("skip-worktree", ("--skip-worktree",), ("--no-skip-worktree",), "S", "S"),
            ("combined", ("--assume-unchanged", "--skip-worktree"),
             ("--no-assume-unchanged", "--no-skip-worktree"), "s", "S"),
        )
        for label, set_flags, clear_flags, expected_v, expected_f in flag_cases:
            with self.subTest(flag=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                path = root / "tracked.py"
                path.write_text("base content\n")
                command(["git", "add", "tracked.py"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "base"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()
                original = source_snapshot(root, base)
                original_value = {**inputs(), "source": original}
                original_receipt = receipt_payload(original_value, review(original_value))

                for option in set_flags:
                    command(["git", "update-index", option, "--", "tracked.py"], root)
                marker_v = command(["git", "ls-files", "-v", "-z"], root).split("\0")[0]
                marker_f = command(["git", "ls-files", "-f", "-z"], root).split("\0")[0]
                self.assertEqual(marker_v[0], expected_v)
                self.assertEqual(marker_f[0], expected_f)

                path.write_text("hidden changed content\n")
                compact = local_review.compact_input(root, {
                    **original_value,
                    "requirements": None,
                    "gate_configuration": {},
                })
                self.assertEqual(compact["changes"]["working"], [])
                self.assertEqual(compact["changes"]["head"], [])
                self.assertEqual(command(["git", "diff", "--name-only", "--", "tracked.py"], root), "")
                with self.assertRaisesRegex(ReviewError, "unsupported .* marker"):
                    source_snapshot(root, base)

                for option in clear_flags:
                    command(["git", "update-index", option, "--", "tracked.py"], root)
                command(["git", "add", "tracked.py"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "commit after clearing flags"], root)
                committed = source_snapshot(root, base)
                with self.assertRaisesRegex(ReviewError, "another input|stale"):
                    validate_receipt(original_receipt, {**original_value, "source": committed})

    def test_fsmonitor_valid_index_flag_is_rejected_when_git_exposes_it(self) -> None:
        command(["git", "update-index", "--fsmonitor-valid", "--", "example.py"], self.root)
        marker = command(["git", "ls-files", "-f", "-z", "--", "example.py"], self.root)
        if not marker.startswith("h "):
            self.skipTest("local Git did not expose an fsmonitor-valid marker; update-index left flags unset")
        with self.assertRaisesRegex(ReviewError, "unsupported .* marker"):
            source_snapshot(self.root, self.base)

    @unittest.skipUnless(os.name == "posix", "POSIX file permission bits are required")
    def test_owner_group_other_execute_bits_match_git_index_mode(self) -> None:
        command(["git", "config", "core.filemode", "true"], self.root)
        path = self.root / "example.py"
        expected_modes = {
            0o644: "100644", 0o654: "100644", 0o645: "100644", 0o655: "100644",
            0o744: "100755", 0o754: "100755", 0o745: "100755", 0o755: "100755",
        }
        baseline = source_snapshot(self.root, self.base)
        for mode, expected_index_mode in expected_modes.items():
            with self.subTest(mode=oct(mode)):
                path.chmod(mode)
                command(["git", "add", "example.py"], self.root)
                index_record = command(["git", "ls-files", "--stage", "-z", "--", "example.py"], self.root)
                actual_index_mode = index_record.split(" ", 1)[0]
                self.assertEqual(actual_index_mode, expected_index_mode)
                record = file_record(self.root, "example.py")
                entry = working_entry(self.root, "example.py")
                owner_executable = bool(mode & stat.S_IXUSR)
                self.assertEqual(record["executable"], owner_executable)
                self.assertTrue(entry is not None)
                self.assertEqual(entry.split(" ", 1)[0], expected_index_mode)

                staged = source_snapshot(self.root, self.base)
                if owner_executable:
                    self.assertNotEqual(staged, baseline)
                else:
                    self.assertEqual(staged, baseline)

    @unittest.skipUnless(os.name == "posix", "POSIX file permission bits are required")
    def test_owner_execute_change_invalidates_same_bytes_receipt(self) -> None:
        command(["git", "config", "core.filemode", "true"], self.root)
        path = self.root / "example.py"
        path.chmod(0o654)
        initial = source_snapshot(self.root, self.base)
        value = {**inputs(), "source": initial}
        receipt = receipt_payload(value, review(value))

        path.chmod(0o744)
        command(["git", "add", "example.py"], self.root)
        updated = source_snapshot(self.root, self.base)
        self.assertNotEqual(initial, updated)
        self.assertEqual(file_record(self.root, "example.py")["sha256"], initial["files"]["example.py"]["sha256"])
        self.assertEqual(working_entry(self.root, "example.py").split(" ", 1)[0], "100755")
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, {**value, "source": updated})

    @unittest.skipUnless(os.name == "posix", "POSIX permits CR/LF filenames")
    def test_crlf_and_lf_paths_remain_distinct_in_snapshot_and_invalidate_receipt(self) -> None:
        (self.root / "example.py").unlink()
        lf_name = "same\nname.py"
        crlf_name = "same\r\nname.py"
        (self.root / lf_name).write_text("LF path original\n")
        (self.root / crlf_name).write_text("CRLF path original\n")
        self.commit("add colliding line-ending paths")
        base = command(["git", "rev-parse", "HEAD"], self.root).strip()
        before = source_snapshot(self.root, base)
        self.assertEqual(set(before["files"]).intersection((lf_name, crlf_name)), {lf_name, crlf_name})
        value = {**inputs(), "source": before}
        receipt = receipt_payload(value, review(value))

        (self.root / crlf_name).write_text("CRLF path changed\n")
        changed = source_snapshot(self.root, base)
        self.assertEqual(set(changed["files"]).intersection((lf_name, crlf_name)), {lf_name, crlf_name})
        self.assertEqual(changed["files"][lf_name], before["files"][lf_name])
        self.assertNotEqual(changed["files"][crlf_name], before["files"][crlf_name])
        self.assertEqual(changed["index_overrides"], [])
        compact = local_review.compact_input(self.root, {
            **value,
            "requirements": None,
            "gate_configuration": {},
        })
        self.assertEqual(compact["changes"]["working"], [crlf_name])
        self.assertEqual(compact["changes"]["head"], [])
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, {**value, "source": changed})

        command(["git", "add", "--", crlf_name], self.root)
        staged = source_snapshot(self.root, base)
        compact = local_review.compact_input(self.root, {
            **value,
            "requirements": None,
            "gate_configuration": {},
        })
        self.assertEqual(compact["changes"]["staged"], [crlf_name])
        self.assertEqual(staged["new_tracked_paths"], [])

    @unittest.skipUnless(os.name == "posix", "non-UTF-8 Git paths require POSIX")
    def test_non_utf8_git_paths_remain_distinct_and_digestable(self) -> None:
        def raw(arguments: list[str], input_bytes: bytes = b"") -> bytes:
            return command(arguments, self.root, input_bytes=input_bytes).encode()

        def direct_git(arguments: list[str]) -> None:
            subprocess.run(arguments, cwd=self.root, capture_output=True, check=True)

        bad_ff = b"bad\xff"
        bad_fe = b"bad\xfe"
        blob_ff = raw(["git", "hash-object", "-w", "--stdin"], b"ff\n").strip()
        blob_fe = raw(["git", "hash-object", "-w", "--stdin"], b"fe\n").strip()
        tree_input = (b"100644 blob " + blob_ff + b"\t" + bad_ff + b"\0" +
                      b"100644 blob " + blob_fe + b"\t" + bad_fe + b"\0")
        tree = raw(["git", "mktree", "-z"], tree_input).strip()
        commit = command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                          "-c", "commit.gpgsign=false", "commit-tree", tree], self.root,
                         input_bytes=b"non-UTF-8 paths\n").strip()
        command(["git", "update-ref", "refs/heads/non-utf8", commit], self.root)

        output = command(["git", "ls-tree", "-r", "--name-only", "-z", commit], self.root)
        self.assertEqual({os.fsencode(name) for name in output.split("\0") if name}, {bad_ff, bad_fe})
        snapshot = source_snapshot(self.root, commit)
        snapshot_paths = {os.fsencode(name) for name in snapshot["files"]}
        self.assertTrue({bad_ff, bad_fe}.issubset(snapshot_paths))
        self.assertEqual(len({name for name in snapshot["files"] if name.startswith("bad")}), 2)
        value = {**inputs(), "source": snapshot}
        self.assertEqual(digest(value), digest(json.loads(json.dumps(value))))
        validate_receipt(receipt_payload(value, review(value)), value)

        # macOS のファイルシステムでは作れない名前でも、Git index は保持できるため、
        # stage から commit まで同じ raw bytes として扱うことを確認する。
        for name, blob in ((bad_ff, blob_ff), (bad_fe, blob_fe)):
            direct_git(["git", "update-index", "--add", "--cacheinfo",
                        f"100644,{blob.decode()},{os.fsdecode(name)}"])
        staged = source_snapshot(self.root, self.base)
        index_tree = command(["git", "write-tree"], self.root).strip()
        index_commit = command(["git", "-c", "user.name=Review fixture", "-c",
                                "user.email=review@example.invalid", "-c", "commit.gpgsign=false",
                                "commit-tree", index_tree, "-p", self.base], self.root,
                               input_bytes=b"stage raw paths\n").strip()
        staged_index_paths = {os.fsencode(entry.rsplit("\t", 1)[-1])
                              for entry in staged["index_overrides"]}
        self.assertEqual(staged_index_paths, {bad_ff, bad_fe})
        command(["git", "update-ref", "HEAD", index_commit], self.root)
        self.assertEqual(command(["git", "rev-parse", "HEAD"], self.root).strip(), index_commit)
        committed = source_snapshot(self.root, self.base)
        committed_head_paths = {os.fsencode(entry.rsplit("\t", 1)[-1])
                                for entry in committed["head_overrides"]}
        committed_index_paths = {os.fsencode(entry.rsplit("\t", 1)[-1])
                                 for entry in committed["index_overrides"]}
        self.assertEqual(staged["files"], committed["files"])
        self.assertEqual(staged["working_blobs"], committed["working_blobs"])
        self.assertEqual(committed_index_paths, {bad_ff, bad_fe})
        self.assertEqual(committed_head_paths, {bad_ff, bad_fe})
        committed_value = {**inputs(), "source": committed}
        validate_receipt(receipt_payload(committed_value, review(committed_value)), committed_value)

        changed_blob = raw(["git", "hash-object", "-w", "--stdin"], b"ff changed\n").strip()
        direct_git(["git", "update-index", "--add", "--cacheinfo",
                    f"100644,{changed_blob.decode()},{os.fsdecode(bad_ff)}"])
        changed = source_snapshot(self.root, self.base)
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt_payload(committed_value, review(committed_value)),
                             {**committed_value, "source": changed})

    @unittest.skipUnless(os.name == "posix", "non-UTF-8 worktree names require POSIX")
    def test_non_utf8_worktree_paths_survive_stage_commit_and_receipt_reuse(self) -> None:
        if os.name == "nt":
            self.skipTest("Windows does not support this filename contract")
        root_bytes = os.fsencode(str(self.root))
        names = (b"bad\xff", b"bad\xfe")

        def write_bytes(name: bytes, content: bytes) -> None:
            try:
                fd = os.open(root_bytes + b"/" + name, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
            except OSError as error:
                if error.errno == errno.EILSEQ:
                    self.skipTest("filesystem rejects non-UTF-8 worktree names")
                raise
            try:
                os.write(fd, content)
            finally:
                os.close(fd)

        def direct_git(arguments: list[str]) -> None:
            subprocess.run(arguments, cwd=self.root, capture_output=True, check=True)

        write_bytes(names[0], b"ff original\n")
        write_bytes(names[1], b"fe original\n")
        untracked = source_snapshot(self.root, self.base)
        untracked_paths = {os.fsencode(name) for name in untracked["files"]}
        self.assertTrue(set(names).issubset(untracked_paths))
        direct_git(["git", "add", "--", *(os.fsdecode(name) for name in names)])
        staged = source_snapshot(self.root, self.base)
        self.assertEqual(staged["files"], untracked["files"])
        staged_value = {**inputs(), "source": staged}
        staged_receipt = receipt_payload(staged_value, review(staged_value))
        self.commit("commit non-UTF-8 worktree paths")
        committed = source_snapshot(self.root, self.base)
        self.assertEqual(committed, staged)
        validate_receipt(staged_receipt, {**staged_value, "source": committed})

        write_bytes(names[0], b"ff changed\n")
        changed = source_snapshot(self.root, self.base)
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(staged_receipt, {**staged_value, "source": changed})

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

    def test_worktree_clean_filter_blob_change_invalidates_receipt_but_same_filter_reuses(self) -> None:
        for change_clean_filter in (False, True):
            label = "changed-clean-filter" if change_clean_filter else "unchanged-clean-filter"
            with self.subTest(filter=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "config", "filter.review.clean", "cat"], root)
                command(["git", "config", "filter.review.smudge", "cat"], root)
                command(["git", "config", "filter.review.required", "true"], root)
                (root / ".gitattributes").write_text("*.txt filter=review\n")
                source = root / "example.txt"
                source.write_text("baseline\n")
                command(["git", "add", ".gitattributes", "example.txt"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "filter baseline"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()

                source.write_text("reviewed\n")
                reviewed_bytes = source.read_bytes()
                before = source_snapshot(root, base)
                self.assertEqual(before["index_overrides"], [])
                self.assertEqual(before["head_overrides"], [])
                value = {**inputs(), "source": before, "requirements": None,
                         "gate_configuration": {}}
                receipt = receipt_payload(value, review(value))

                if change_clean_filter:
                    command(["git", "config", "filter.review.clean", "sed 's/reviewed/pushed/g'"], root)
                command(["git", "add", "example.txt"], root)
                clean_blob = command(["git", "cat-file", "blob", ":example.txt"], root)
                expected_blob = "pushed\n" if change_clean_filter else "reviewed\n"
                self.assertEqual(clean_blob, expected_blob)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", label], root)

                self.assertEqual(source.read_bytes(), reviewed_bytes)
                after = source_snapshot(root, base)
                if change_clean_filter:
                    self.assertNotEqual(before, after)
                    with self.assertRaisesRegex(ReviewError, "another input|stale"):
                        validate_receipt(receipt, {**value, "source": after})
                else:
                    self.assertEqual(before, after)
                    self.assertEqual(validate_receipt(receipt, {**value, "source": after})["verdict"], "PASS")

    def test_tracked_leaf_replaced_by_directory_snapshots_and_reuses_after_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            command(["git", "init", "--quiet"], root)
            (root / "pkg").write_text("old leaf\n")
            command(["git", "add", "pkg"], root)
            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "tracked package leaf"], root)
            base = command(["git", "rev-parse", "HEAD"], root).strip()

            (root / "pkg").unlink()
            (root / "pkg").mkdir()
            (root / "pkg" / "mod.py").write_text("new module\n")
            worktree = source_snapshot(root, base)
            self.assertEqual(worktree["files"]["pkg"], {"kind": "deleted"})
            self.assertEqual(worktree["files"]["pkg/mod.py"]["kind"], "file")

            command(["git", "add", "-A"], root)
            staged = source_snapshot(root, base)
            self.assertEqual(staged["files"], worktree["files"])
            self.assertEqual(staged["new_tracked_paths"], ["pkg/mod.py"])
            value = {**inputs(), "source": staged, "requirements": None,
                     "gate_configuration": {}}
            receipt = receipt_payload(value, review(value))

            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "commit package directory"], root)
            committed = source_snapshot(root, base)
            self.assertEqual(staged, committed)
            self.assertEqual(validate_receipt(receipt, {**value, "source": committed})["verdict"], "PASS")

    def test_tracked_directory_replaced_by_leaf_snapshots_and_reuses_after_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            command(["git", "init", "--quiet"], root)
            (root / "pkg").mkdir()
            (root / "pkg" / "mod.py").write_text("old module\n")
            command(["git", "add", "pkg/mod.py"], root)
            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "tracked package module"], root)
            base = command(["git", "rev-parse", "HEAD"], root).strip()

            (root / "pkg" / "mod.py").unlink()
            (root / "pkg").rmdir()
            (root / "pkg").write_text("new package leaf\n")
            worktree = source_snapshot(root, base)
            self.assertEqual(worktree["files"]["pkg/mod.py"], {"kind": "deleted"})
            self.assertEqual(worktree["files"]["pkg"]["kind"], "file")

            command(["git", "add", "-A"], root)
            staged = source_snapshot(root, base)
            self.assertEqual(staged["files"], worktree["files"])
            self.assertEqual(staged["new_tracked_paths"], ["pkg"])
            value = {**inputs(), "source": staged, "requirements": None,
                     "gate_configuration": {}}
            receipt = receipt_payload(value, review(value))

            command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "commit package leaf"], root)
            committed = source_snapshot(root, base)
            self.assertEqual(staged, committed)
            self.assertEqual(validate_receipt(receipt, {**value, "source": committed})["verdict"], "PASS")

    def test_gitlink_entries_are_rejected_in_index_and_head(self) -> None:
        for committed in (False, True):
            with self.subTest(committed=committed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                (root / "tracked.py").write_text("base\n")
                command(["git", "add", "tracked.py"], root)
                command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                         "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "base"], root)
                base = command(["git", "rev-parse", "HEAD"], root).strip()
                command(["git", "update-index", "--add", "--cacheinfo", "160000",
                         "1" * 40, "vendor"], root)
                if committed:
                    command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                             "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "gitlink"], root)
                with self.assertRaisesRegex(ReviewError, "submodule entries cannot be reviewed: vendor"):
                    source_snapshot(root, base)

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
    def test_actual_just_gate_tracks_cargo_environment_presence_and_receipt_identity(self) -> None:
        from local_review_state import gate_configuration

        root = Path(__file__).resolve().parents[2]
        cargo_names = (
            "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET_DIR",
            "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "RUSTC",
        )
        clean_environment = {"PATH": os.environ["PATH"], "CI": "true"}

        with patch.dict(os.environ, clean_environment, clear=True):
            absent = gate_configuration(root)
        empty_environment = {
            **clean_environment,
            **{name: "" for name in cargo_names if name != "CARGO_BUILD_TARGET_DIR"},
        }
        with patch.dict(os.environ, empty_environment, clear=True):
            present_empty = gate_configuration(root)
        nonempty_environment = {
            **clean_environment,
            **{name: f"review-value-{index}" for index, name in enumerate(cargo_names)},
        }
        nonempty_environment["CARGO_BUILD_TARGET_DIR"] = "tmp/review-value-target"
        nonempty_environment["GITHUB_TOKEN"] = "secret-must-not-enter-the-input"
        with patch.dict(os.environ, nonempty_environment, clear=True):
            nonempty = gate_configuration(root)

        for name in cargo_names:
            with self.subTest(variable=name):
                self.assertIn(name, absent)
                self.assertIn(f"{name}_PRESENT", absent)
                self.assertEqual(absent[name], "rustc" if name == "RUSTC" else "")
                self.assertEqual(absent[f"{name}_PRESENT"], "false")
                self.assertEqual(present_empty[name], "")
                expected_empty_presence = name != "CARGO_BUILD_TARGET_DIR"
                self.assertEqual(present_empty[f"{name}_PRESENT"], str(expected_empty_presence).lower())
                expected_value = (
                    str((root / nonempty_environment[name]).resolve())
                    if name == "CARGO_BUILD_TARGET_DIR"
                    else f"review-value-{cargo_names.index(name)}"
                )
                self.assertEqual(nonempty[name], expected_value)
                self.assertEqual(nonempty[f"{name}_PRESENT"], "true")

        baseline = {**inputs(), "source": source_snapshot(root, "HEAD"),
                    "requirements": None, "gate_configuration": absent}
        receipt = receipt_payload(baseline, review(baseline))
        empty_inputs = {**baseline, "gate_configuration": present_empty}
        nonempty_inputs = {**baseline, "gate_configuration": nonempty}
        self.assertNotEqual(digest(baseline), digest(empty_inputs))
        self.assertNotEqual(digest(empty_inputs), digest(nonempty_inputs))
        for changed in (empty_inputs, nonempty_inputs):
            with self.assertRaisesRegex(ReviewError, "another input|stale"):
                validate_receipt(receipt, changed)
        self.assertNotIn("secret-must-not-enter-the-input", json.dumps(nonempty))
        self.assertNotIn("GITHUB_TOKEN", nonempty)

    def test_empty_cargo_target_directories_are_rejected(self) -> None:
        from local_review_state import gate_configuration

        root = Path(__file__).resolve().parents[2]
        clean_environment = {"PATH": os.environ["PATH"], "CI": "true"}
        for name in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"):
            with self.subTest(variable=name), patch.dict(
                os.environ, {**clean_environment, name: ""}, clear=True,
            ), self.assertRaisesRegex(ReviewError, f"{name} must not be empty"):
                gate_configuration(root)

    def test_cargo_target_directory_direct_alias_and_default_precedence(self) -> None:
        from local_review_state import gate_configuration

        root = Path(__file__).resolve().parents[2]
        clean_environment = {"PATH": os.environ["PATH"], "CI": "true"}
        with patch.dict(os.environ, clean_environment, clear=True):
            default = gate_configuration(root)
        with patch.dict(os.environ, {**clean_environment, "CARGO_BUILD_TARGET_DIR": "tmp/alias-target"}, clear=True):
            alias = gate_configuration(root)
        with patch.dict(os.environ, {
            **clean_environment,
            "CARGO_BUILD_TARGET_DIR": "tmp/alias-target",
            "CARGO_TARGET_DIR": "tmp/direct-target",
        }, clear=True):
            direct = gate_configuration(root)
        with patch.dict(os.environ, {
            **clean_environment,
            "CARGO_BUILD_TARGET_DIR": str(root / "tmp/alias-target"),
        }, clear=True):
            absolute_alias = gate_configuration(root)
        with patch.dict(os.environ, {
            **clean_environment,
            "CARGO_BUILD_TARGET_DIR": "tmp/alias-target",
            "CARGO_TARGET_DIR": str(root / "tmp/direct-target"),
        }, clear=True):
            absolute_direct = gate_configuration(root)

        self.assertEqual(default["CARGO_TARGET_DIR"], str(root / "target"))
        self.assertEqual(alias["CARGO_TARGET_DIR"], str(root / "tmp/alias-target"))
        self.assertEqual(alias["CARGO_BUILD_BUILD_DIR"], str(root / "tmp/alias-target"))
        self.assertEqual(direct["CARGO_TARGET_DIR"], str(root / "tmp/direct-target"))
        self.assertEqual(absolute_alias["CARGO_TARGET_DIR"], alias["CARGO_TARGET_DIR"])
        self.assertEqual(absolute_alias, alias)
        self.assertEqual(absolute_direct, direct)

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
                self.assertTrue((receipt.parent / "review.lock").is_file())
                with review_lock(receipt.parent / "review.lock"):
                    pass

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
            self.assertTrue((root / "review.lock").is_file())
            with review_lock(root / "review.lock"):
                pass
            self.assertEqual(json.loads((root / "last-input.json").read_text()), value)
            self.assertEqual((root / "last-codex-stderr.log").read_text(), "partial review; not a PASS")

    def test_review_lock_reuses_existing_empty_persistent_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "review.lock"
            lock.write_bytes(b"")
            original_stat = lock.stat()
            with review_lock(lock):
                self.assertTrue(lock.is_file())
            self.assertTrue(lock.is_file())
            self.assertEqual(lock.stat().st_size, 0)
            if os.name == "posix":
                self.assertEqual(lock.stat().st_ino, original_stat.st_ino)
            with review_lock(lock):
                self.assertTrue(lock.is_file())

    def test_review_lock_excludes_live_child_and_recovers_after_process_death(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            lock = root / "review.lock"
            lock.write_bytes(b"")
            original_inode = lock.stat().st_ino
            child_environment = os.environ.copy()
            child_environment["PYTHONPATH"] = str(Path(__file__).parent)
            child_code = (
                "import sys, time\n"
                "from pathlib import Path\n"
                "from local_review_lock import review_lock\n"
                "with review_lock(Path(sys.argv[1])):\n"
                "    Path(sys.argv[2]).write_text('ready')\n"
                "    time.sleep(3600)\n"
            )
            terminations = ((signal.SIGTERM, "SIGTERM"), (signal.SIGKILL, "SIGKILL")) \
                if os.name == "posix" else ((None, "terminate"),)
            for child_signal, label in terminations:
                with self.subTest(termination=label):
                    ready = root / f"{label}.ready"
                    child = subprocess.Popen(
                        [sys.executable, "-c", child_code, str(lock), str(ready)],
                        cwd=Path(__file__).parent,
                        env=child_environment,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                    )
                    try:
                        deadline = time.monotonic() + 10
                        while not ready.exists():
                            if child.poll() is not None:
                                _stdout, stderr = child.communicate()
                                self.fail(f"lock child exited before acquiring lock: {stderr}")
                            if time.monotonic() >= deadline:
                                self.fail("lock child did not signal acquisition before deadline")
                            time.sleep(0.01)
                        with self.assertRaisesRegex(ReviewError, "another local review is active"):
                            with review_lock(lock):
                                pass
                        if child_signal is None:
                            child.terminate()
                        else:
                            os.kill(child.pid, child_signal)
                        child.wait(timeout=10)
                        if child_signal is not None:
                            self.assertEqual(child.returncode, -child_signal)
                        self.assertTrue(lock.is_file())
                        if os.name == "posix":
                            self.assertEqual(lock.stat().st_ino, original_inode)
                        with review_lock(lock):
                            self.assertTrue(lock.is_file())
                    finally:
                        if child.poll() is None:
                            child.kill()
                            child.wait(timeout=10)
                        if child.stdout is not None:
                            child.stdout.close()
                        if child.stderr is not None:
                            child.stderr.close()

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
            "https://github.com:443/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/%48iroyukiFuruno/katana-render-runtime.git",
            "https://%67ithub.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://%47itHub.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/%6batana-render-runtime.git",
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git",
            "git@GitHub.com:HiroyukiFuruno/katana-render-runtime.git",
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

    def test_invalid_scp_origins_are_rejected_before_github_api_call(self) -> None:
        invalid_origins = (
            "Git@github.com:HiroyukiFuruno/katana-render-runtime.git",
            "git@github.com:2222/HiroyukiFuruno/katana-render-runtime.git",
            "git@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "git@github.com:HiroyukiFuruno/nested/katana-render-runtime.git",
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git?query=1",
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git#fragment",
            "git%40github.com:HiroyukiFuruno/katana-render-runtime.git",
            "git@github%2ecom:HiroyukiFuruno/katana-render-runtime.git",
            "git@github.com%3aHiroyukiFuruno/katana-render-runtime.git",
            "git@github.com:OtherOwner/katana-render-runtime.git@extra",
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git%0a",
        )
        for origin in invalid_origins:
            with self.subTest(origin=origin), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "config", "remote.origin.url", origin], root)
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

    def test_invalid_https_origins_are_rejected_before_github_api_call(self) -> None:
        invalid_origins = (
            "https://github.com:444/HiroyukiFuruno/katana-render-runtime.git",
            "https://user@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://git:secret@github.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git?query=1",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git?",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git#fragment",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git#",
            "https://github.com:/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git%G1",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git%FF",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git?%FF",
            "https://user%40github.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git%3Fquery=1",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git%23fragment",
            "https://github.com/HiroyukiFuruno%2fnested/katana-render-runtime.git",
            "https://github.com:444/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com:%34%34%34/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com%2fHiroyukiFuruno/katana-render-runtime.git",
            "https://github.com%3a443/HiroyukiFuruno/katana-render-runtime.git",
            "https://git%40github.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://%2567ithub.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/katana-render-%2572untime.git",
            "https://github.com/HiroyukiFuruno/katana-render-runtime.git\r",
            "https://github.com/HiroyukiFuruno/\r\nkatana-render-runtime.git",
        )
        for origin in invalid_origins:
            with self.subTest(origin=repr(origin)), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                command(["git", "init", "--quiet"], root)
                command(["git", "config", "remote.origin.url", origin], root)
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

    def test_invalid_surrogate_https_origin_is_rejected_before_github_api_call(self) -> None:
        gh_calls = []
        origin = "https://github.com/HiroyukiFuruno/katana-render-runtime\udcff.git"

        def mocked_command(arguments, _root, input_bytes=None):
            if arguments[:3] == ["git", "remote", "get-url"]:
                return origin
            if arguments[0] == "gh":
                gh_calls.append(arguments)
                return "{}"
            self.fail(f"unexpected command: {arguments}")

        with patch("local_review_state.command", side_effect=mocked_command):
            with self.assertRaisesRegex(ReviewError, "GitHub repository"):
                issue_context(Path.cwd(), [89])
        self.assertFalse(gh_calls)

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
            "ssh://git%40github.com/HiroyukiFuruno/katana-render-runtime.git",
            "ssh://git@github.com%2fHiroyukiFuruno/katana-render-runtime.git",
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
