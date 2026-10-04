from __future__ import annotations

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
