#!/usr/bin/env python3
"""Regression tests for the consecutive KRR release target guard."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from importlib import util
from pathlib import Path


SCRIPT = Path(__file__).with_name("verify-release-target.py")
REQUIRED_COMMITS = (
    "02a73d293c04f9635fd3a822ac865bf81d4c8745",
    "8552c63457480379922c7076bcff604b5401ae20",
    "694ac82a85d555485e46eb46cf882c8db11b2fe5",
    "007ab829df39ed40bbfcfc19205ad21f3da32fe8",
    "91f07699b26567658f76021050ca7ec7b5c10df1",
)

MODULE_SPEC = util.spec_from_file_location("verify_release_target", SCRIPT)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
VERIFY_RELEASE_TARGET = util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = VERIFY_RELEASE_TARGET
MODULE_SPEC.loader.exec_module(VERIFY_RELEASE_TARGET)


class VerifyReleaseTargetTests(unittest.TestCase):
    def run_check(
        self, target: str, latest: str, head_ref: str = "HEAD"
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                target,
                "--latest-version",
                latest,
                "--head-ref",
                head_ref,
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_v0420_after_published_v0419(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.19")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_allows_idempotent_retry_after_v0420(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.20")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_v0420_before_v0419_is_published(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.18")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_skipped_target(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.19")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_old_target(self) -> None:
        result = self.run_check("v0.4.19", "v0.4.19")
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_head_containing_every_v0420_issue_commit(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.19")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_a_v0420_issue_commit(self) -> None:
        for commit in REQUIRED_COMMITS[1:]:
            parent = subprocess.run(
                ["git", "rev-parse", f"{commit}^"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            result = self.run_check("v0.4.20", "v0.4.19", parent)
            self.assertNotEqual(result.returncode, 0, commit)

    def test_accepts_squash_tree_but_rejects_an_incomplete_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)

            def git(*args: str) -> str:
                return subprocess.run(
                    ["git", *args],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()

            def commit(name: str, content: str, message: str) -> str:
                (repository / name).write_text(content, encoding="utf-8")
                git("add", name)
                git("commit", "-q", "-m", message)
                return git("rev-parse", "HEAD")

            git("init", "-q")
            git("config", "user.email", "release-test@example.invalid")
            git("config", "user.name", "Release Target Test")
            base = commit("runtime.txt", "baseline\n", "base")
            required_first = commit("runtime.txt", "first required change\n", "required first")
            required_tree = commit("coverage.txt", "required regression\n", "required final")

            git("switch", "-q", "-c", "squash", base)
            (repository / "runtime.txt").write_text("first required change\n", encoding="utf-8")
            (repository / "coverage.txt").write_text("required regression\n", encoding="utf-8")
            git("add", "runtime.txt", "coverage.txt")
            git("commit", "-q", "-m", "squash required changes")
            squash = git("rev-parse", "HEAD")

            git("switch", "-q", "-c", "incomplete", base)
            incomplete = commit("runtime.txt", "first required change\n", "incomplete squash")

            required_commits = (required_first, required_tree)
            original_directory = Path.cwd()
            try:
                os.chdir(repository)
                self.assertEqual(
                    VERIFY_RELEASE_TARGET.missing_required_commits(squash, required_commits),
                    list(required_commits),
                )
                self.assertTrue(
                    VERIFY_RELEASE_TARGET.release_tree_matches(squash, base, required_tree)
                )
                self.assertFalse(
                    VERIFY_RELEASE_TARGET.release_tree_matches(incomplete, base, required_tree)
                )
                self.assertEqual(
                    VERIFY_RELEASE_TARGET.missing_required_commits(required_tree, required_commits),
                    [],
                )
            finally:
                os.chdir(original_directory)

    def test_preflight_passes_github_token_to_every_release_target_invocation(self) -> None:
        workflow = (SCRIPT.parents[2] / ".github/workflows/release-preflight.yml").read_text(
            encoding="utf-8"
        )
        for step in ("Release target check", "Release check"):
            with self.subTest(step=step):
                start = workflow.index(f"- name: {step}")
                end = workflow.find("\n      - name:", start + 1)
                body = workflow[start:] if end == -1 else workflow[start:end]
                self.assertIn("GH_TOKEN: ${{ github.token }}", body)


if __name__ == "__main__":
    unittest.main()
