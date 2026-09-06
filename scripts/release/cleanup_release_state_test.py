from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import cleanup_release_state as subject


class CleanupReleaseStateTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.remote = self.root / "remote.git"
        self.repository = self.root / "repository"
        self.git("init", "--bare", "--initial-branch=master", str(self.remote), cwd=self.root)
        self.git("init", "--initial-branch=master", str(self.repository), cwd=self.root)
        self.git("config", "user.name", "Cleanup Test", cwd=self.repository)
        self.git("config", "user.email", "cleanup@example.com", cwd=self.repository)
        (self.repository / "README.md").write_text("base\n", encoding="utf-8")
        self.git("add", "README.md", cwd=self.repository)
        self.git("commit", "-m", "initial", cwd=self.repository)
        self.git("remote", "add", "origin", str(self.remote), cwd=self.repository)
        self.git("push", "-u", "origin", "master", cwd=self.repository)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def git(self, *arguments: str, cwd: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            check=True,
            capture_output=True,
            text=True,
        )

    def create_release_branch(self, *, merge: bool) -> None:
        self.git("switch", "-c", "release/v9.9.9", cwd=self.repository)
        (self.repository / "release.txt").write_text("release\n", encoding="utf-8")
        self.git("add", "release.txt", cwd=self.repository)
        self.git("commit", "-m", "release", cwd=self.repository)
        self.git("push", "-u", "origin", "release/v9.9.9", cwd=self.repository)
        self.git("switch", "master", cwd=self.repository)
        if merge:
            self.git(
                "merge",
                "--no-ff",
                "release/v9.9.9",
                "-m",
                "merge release",
                cwd=self.repository,
            )
            self.git("push", "origin", "master", cwd=self.repository)

    def cleanup(self, *, published: bool = True) -> list[str]:
        return subject.cleanup_release_state(
            repository=self.repository,
            version="v9.9.9",
            release_branch="release/v9.9.9",
            remote="origin",
            default_branch="master",
            release_checker=lambda _version: published,
        )

    def remote_branch_exists(self, branch: str) -> bool:
        result = subprocess.run(
            ["git", "ls-remote", "--exit-code", "--heads", "origin", branch],
            cwd=self.repository,
            capture_output=True,
            text=True,
            check=False,
        )
        return result.returncode == 0

    def test_refuses_cleanup_before_public_release(self) -> None:
        self.create_release_branch(merge=True)
        with self.assertRaisesRegex(subject.CleanupError, "公開"):
            self.cleanup(published=False)
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_refuses_mismatched_version_and_release_branch_before_mutation(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)

        with self.assertRaisesRegex(subject.CleanupError, "version.*一致しません"):
            subject.cleanup_release_state(
                repository=self.repository,
                version="v9.9.9-other",
                release_branch="release/v9.9.9",
                remote="origin",
                default_branch="master",
                release_checker=lambda _version: True,
            )

        self.assertTrue(worktree.exists())
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertNotEqual(local.strip(), "")

    def test_switches_to_default_and_deletes_merged_local_and_remote_branch(self) -> None:
        self.create_release_branch(merge=True)
        self.git("switch", "release/v9.9.9", cwd=self.repository)
        actions = self.cleanup()
        current = self.git("branch", "--show-current", cwd=self.repository).stdout.strip()
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertEqual(current, "master")
        self.assertEqual(local.strip(), "")
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))
        self.assertIn("pulled origin/master with --ff-only", actions)
        self.assertIn("remote branch release/v9.9.9 deleted", actions)

    def test_retains_unmerged_branch(self) -> None:
        self.create_release_branch(merge=False)
        with self.assertRaisesRegex(subject.CleanupError, "未統合"):
            self.cleanup()
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertNotEqual(local.strip(), "")

    def test_refuses_cleanup_when_local_release_tip_is_unpushed(self) -> None:
        self.create_release_branch(merge=True)
        self.git("switch", "release/v9.9.9", cwd=self.repository)
        (self.repository / "unpushed.txt").write_text("unpushed\n", encoding="utf-8")
        self.git("add", "unpushed.txt", cwd=self.repository)
        self.git("commit", "-m", "unpushed release change", cwd=self.repository)
        self.git("switch", "master", cwd=self.repository)

        with self.assertRaisesRegex(subject.CleanupError, "未統合"):
            self.cleanup()

        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertNotEqual(local.strip(), "")
        preserved = self.git(
            "show",
            "release/v9.9.9:unpushed.txt",
            cwd=self.repository,
        ).stdout
        self.assertEqual(preserved, "unpushed\n")

    def test_retains_dirty_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        (worktree / "dirty.txt").write_text("dirty\n", encoding="utf-8")
        with self.assertRaisesRegex(subject.CleanupError, "dirty"):
            self.cleanup()
        self.assertTrue(worktree.exists())
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_removes_clean_merged_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        actions = self.cleanup()
        self.assertFalse(worktree.exists())
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))
        self.assertTrue(any(action.startswith("worktree ") for action in actions))

    def test_retains_locked_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        self.git("worktree", "lock", str(worktree), cwd=self.repository)
        with self.assertRaisesRegex(subject.CleanupError, "locked"):
            self.cleanup()
        self.assertTrue(worktree.exists())
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_fetches_release_branch_when_remote_refspec_is_narrow(self) -> None:
        self.create_release_branch(merge=True)
        self.git("config", "--unset-all", "remote.origin.fetch", cwd=self.repository)
        self.git(
            "config",
            "--add",
            "remote.origin.fetch",
            "+refs/heads/master:refs/remotes/origin/master",
            cwd=self.repository,
        )
        self.git(
            "update-ref",
            "-d",
            "refs/remotes/origin/release/v9.9.9",
            cwd=self.repository,
        )
        self.cleanup()
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))

    def test_refuses_remote_delete_when_branch_advances_after_audit(self) -> None:
        self.create_release_branch(merge=True)
        racing_repository = self.root / "racing-repository"
        self.git("clone", str(self.remote), str(racing_repository), cwd=self.root)
        self.git("config", "user.name", "Racing Test", cwd=racing_repository)
        self.git("config", "user.email", "racing@example.com", cwd=racing_repository)
        self.git("switch", "release/v9.9.9", cwd=racing_repository)
        (racing_repository / "raced.txt").write_text("raced\n", encoding="utf-8")
        self.git("add", "raced.txt", cwd=racing_repository)
        self.git("commit", "-m", "advance release branch", cwd=racing_repository)

        original_run_git = subject._run_git

        def advance_before_delete(repository: Path, *arguments: str, **kwargs: object):
            if arguments[:1] == ("push",) and any(
                argument.startswith("--force-with-lease=") for argument in arguments
            ):
                self.git("push", "origin", "release/v9.9.9", cwd=racing_repository)
            return original_run_git(repository, *arguments, **kwargs)

        with mock.patch.object(subject, "_run_git", side_effect=advance_before_delete):
            with self.assertRaisesRegex(subject.CleanupError, "push .* failed"):
                self.cleanup()
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_refuses_cleanup_when_push_url_targets_different_repository_at_same_sha(self) -> None:
        self.create_release_branch(merge=True)
        push_remote = self.root / "push-remote.git"
        self.git("init", "--bare", "--initial-branch=master", str(push_remote), cwd=self.root)
        self.git("remote", "add", "push-target", str(push_remote), cwd=self.repository)
        self.git("push", "push-target", "master", cwd=self.repository)
        self.git("push", "push-target", "release/v9.9.9", cwd=self.repository)
        self.git(
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            str(push_remote),
            cwd=self.repository,
        )

        with self.assertRaisesRegex(subject.CleanupError, "push URL.*異なるrepository"):
            self.cleanup()

        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertNotEqual(local.strip(), "")
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_allows_equivalent_fetch_and_push_urls_for_same_repository(self) -> None:
        self.create_release_branch(merge=True)
        self.git(
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            self.remote.resolve().as_uri(),
            cwd=self.repository,
        )

        actions = self.cleanup()

        self.assertIn("remote branch release/v9.9.9 deleted", actions)
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))

    def test_deletes_once_when_push_url_is_repeated(self) -> None:
        self.create_release_branch(merge=True)
        for _ in range(2):
            self.git(
                "remote",
                "set-url",
                "--add",
                "--push",
                "origin",
                str(self.remote),
                cwd=self.repository,
            )

        push_urls = self.git(
            "remote",
            "get-url",
            "--push",
            "--all",
            "origin",
            cwd=self.repository,
        ).stdout.splitlines()
        self.assertEqual(push_urls, [str(self.remote), str(self.remote)])

        with mock.patch.object(subject, "_run_git", wraps=subject._run_git) as run_git:
            self.cleanup()

        delete_pushes = [
            call
            for call in run_git.call_args_list
            if call.args[1:2] == ("push",)
            and any(
                str(argument).startswith("--force-with-lease=") for argument in call.args[2:]
            )
        ]
        self.assertEqual(len(delete_pushes), 1)
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))

    def test_uses_audited_push_url_when_remote_config_changes(self) -> None:
        self.create_release_branch(merge=True)
        changed_push_remote = self.root / "changed-push-remote.git"
        self.git(
            "init",
            "--bare",
            "--initial-branch=master",
            str(changed_push_remote),
            cwd=self.root,
        )
        original_run_git = subject._run_git
        push_arguments: list[str] = []

        def change_config_before_delete(repository: Path, *arguments: str, **kwargs: object):
            if arguments[:1] == ("push",) and any(
                argument.startswith("--force-with-lease=") for argument in arguments
            ):
                push_arguments.extend(arguments)
                self.git(
                    "remote",
                    "set-url",
                    "--push",
                    "origin",
                    str(changed_push_remote),
                    cwd=self.repository,
                )
            return original_run_git(repository, *arguments, **kwargs)

        with mock.patch.object(subject, "_run_git", side_effect=change_config_before_delete):
            self.cleanup()

        self.assertEqual(push_arguments[1], str(self.remote))
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))

    def test_canonicalizes_github_fetch_and_push_url_variants(self) -> None:
        ssh = subject._remote_repository_identity(
            self.repository,
            "git@github.com:HiroyukiFuruno/katana-render-runtime.git",
        )
        https = subject._remote_repository_identity(
            self.repository,
            "https://github.com/hiroyukifuruno/katana-render-runtime/",
        )

        self.assertEqual(ssh, https)

    def test_rejects_default_branch_as_cleanup_target(self) -> None:
        with self.assertRaisesRegex(subject.CleanupError, "default branch"):
            subject.cleanup_release_state(
                repository=self.repository,
                version="v9.9.9",
                release_branch="master",
                remote="origin",
                default_branch="master",
                release_checker=lambda _version: True,
            )


if __name__ == "__main__":
    unittest.main()
