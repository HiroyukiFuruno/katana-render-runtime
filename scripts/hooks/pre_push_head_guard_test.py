from __future__ import annotations

import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import pre_push_head_guard as subject
from verify_push_issue import ContractViolation


class PrePushHeadGuardTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.repository = Path(self.temporary_directory.name) / "repository"
        self.repository.mkdir()
        self.git("init", "--initial-branch=master")
        (self.repository / "tracked.txt").write_text("first\n", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "first",
        )
        self.head = self.git("rev-parse", "HEAD")

    def git(self, *arguments: str) -> str:
        result = subprocess.run(
            ["git", *arguments],
            cwd=self.repository,
            check=True,
            capture_output=True,
            text=True,
        )
        return result.stdout.strip()

    def update(
        self,
        *,
        local_ref: str = "HEAD",
        remote_ref: str = "refs/heads/topic",
        local_sha: str | None = None,
    ) -> str:
        return f"{local_ref} {local_sha or self.head} {remote_ref} {'0' * 40}\n"

    def test_accepts_branch_update_at_reviewed_checkout_head(self) -> None:
        subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_unstaged_worktree_bytes_that_differ_from_head(self) -> None:
        (self.repository / "tracked.txt").write_text("good\n", encoding="utf-8")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_staged_index_that_differs_from_head(self) -> None:
        (self.repository / "tracked.txt").write_text("good\n", encoding="utf-8")
        self.git("add", "tracked.txt")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    @unittest.skipUnless(os.name == "posix", "POSIX file modes are required")
    def test_rejects_worktree_mode_that_differs_from_head(self) -> None:
        self.git("config", "core.fileMode", "true")
        tracked = self.repository / "tracked.txt"
        tracked.chmod(tracked.stat().st_mode | stat.S_IXUSR)

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    @unittest.skipUnless(os.name == "posix", "POSIX file modes are required")
    def test_accepts_checkout_mode_difference_when_core_filemode_is_false(self) -> None:
        executable = self.repository / "tool.sh"
        executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
        self.git("add", "tool.sh")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "executable",
        )
        current_head = self.git("rev-parse", "HEAD")
        self.git("config", "core.fileMode", "false")
        executable.chmod(executable.stat().st_mode & ~0o111)

        subject.validate_push_head(
            self.update(local_sha=current_head), current_head, self.repository
        )

    def test_rejects_index_tree_mode_difference_when_core_filemode_is_false(self) -> None:
        self.git("config", "core.fileMode", "false")
        self.git("update-index", "--chmod=+x", "tracked.txt")

        with self.assertRaisesRegex(ContractViolation, "indexがHEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_skip_worktree_index_flag(self) -> None:
        self.git("update-index", "--skip-worktree", "tracked.txt")

        with self.assertRaisesRegex(ContractViolation, "index flag"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_assume_unchanged_index_flag(self) -> None:
        self.git("update-index", "--assume-unchanged", "tracked.txt")

        with self.assertRaisesRegex(ContractViolation, "index flag"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    @unittest.skipUnless(hasattr(Path, "symlink_to"), "symlinks are required")
    def test_rejects_symlink_target_that_differs_from_head(self) -> None:
        tracked = self.repository / "link.txt"
        tracked.symlink_to("first-target")
        self.git("add", "link.txt")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "symlink",
        )
        current_head = self.git("rev-parse", "HEAD")
        tracked.unlink()
        tracked.symlink_to("second-target")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(
                self.update(local_sha=current_head), current_head, self.repository
            )

    def test_rejects_nonignored_untracked_quality_input(self) -> None:
        (self.repository / "untracked.py").write_text("good\n", encoding="utf-8")

        with self.assertRaisesRegex(ContractViolation, "未追跡ファイル"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_allows_ignored_quality_outputs(self) -> None:
        exclude = self.repository / ".git" / "info" / "exclude"
        exclude.write_text("target/\n", encoding="utf-8")
        generated = self.repository / "target" / "generated.txt"
        generated.parent.mkdir()
        generated.write_text("output\n", encoding="utf-8")

        subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rechecks_worktree_after_quality_gate(self) -> None:
        subject.validate_push_head(self.update(), self.head, self.repository)
        (self.repository / "tracked.txt").write_text("changed during check\n", encoding="utf-8")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_smudged_filter_bytes_that_differ_from_head_blob(self) -> None:
        self.git("config", "filter.replace.clean", "sed s/good/bad/")
        self.git("config", "filter.replace.smudge", "sed s/bad/good/")
        (self.repository / ".gitattributes").write_text(
            "tracked.txt filter=replace\n", encoding="utf-8"
        )
        self.git("add", ".gitattributes")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "filter",
        )
        (self.repository / "tracked.txt").write_text("bad\n", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "filtered blob",
        )
        current_head = self.git("rev-parse", "HEAD")
        (self.repository / "tracked.txt").unlink()
        self.git("checkout", "--force", "HEAD", "--", "tracked.txt")
        self.assertEqual((self.repository / "tracked.txt").read_text(), "good\n")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(
                self.update(local_sha=current_head), current_head, self.repository
            )

    def test_rejects_worktree_matching_a_replaced_head_blob(self) -> None:
        blob = self.git("rev-parse", "HEAD:tracked.txt")
        replacement_result = subprocess.run(
            ["git", "hash-object", "-w", "--stdin"],
            cwd=self.repository,
            input=b"good\n",
            check=True,
            capture_output=True,
            text=False,
        )
        replacement = replacement_result.stdout.decode("ascii").strip()
        self.git("replace", blob, replacement)
        (self.repository / "tracked.txt").write_text("good\n", encoding="utf-8")
        self.assertEqual(self.git("cat-file", "blob", blob), "good")

        with self.assertRaisesRegex(ContractViolation, "HEADと一致しません"):
            subject.validate_push_head(self.update(), self.head, self.repository)

    def test_rejects_explicit_other_commit_before_quality_check(self) -> None:
        (self.repository / "tracked.txt").write_text("second\n", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "second",
        )
        other_head = self.git("rev-parse", "HEAD")

        with self.assertRaisesRegex(ContractViolation, "review対象HEADと異なります"):
            subject.validate_push_head(
                self.update(local_ref=other_head, local_sha=other_head),
                self.head,
                self.repository,
            )

    def test_rejects_checkout_move_during_quality_check(self) -> None:
        (self.repository / "tracked.txt").write_text("second\n", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.git(
            "-c",
            "user.name=Hook test",
            "-c",
            "user.email=hook@example.test",
            "commit",
            "-m",
            "second",
        )

        with self.assertRaisesRegex(ContractViolation, "checkout HEADが変わりました"):
            subject.validate_push_head(
                self.update(local_sha=self.head), self.head, self.repository
            )

    def test_deletion_and_tag_updates_keep_existing_semantics(self) -> None:
        (self.repository / "tracked.txt").write_text("dirty\n", encoding="utf-8")
        deletion = f"(delete) {'0' * 40} refs/heads/topic {self.head}\n"
        tag = self.update(remote_ref="refs/tags/v0.0.1", local_ref=self.head)
        subject.validate_push_head(deletion + tag, self.head, self.repository)

    def test_malformed_updates_fail_closed(self) -> None:
        with self.assertRaises(ContractViolation):
            subject.validate_push_head(
                "refs/heads/topic malformed\n", self.head, self.repository
            )


if __name__ == "__main__":
    unittest.main()
