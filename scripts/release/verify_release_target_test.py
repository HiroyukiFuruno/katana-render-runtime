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
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("verify-release-target.py")
MODULE_SPEC = util.spec_from_file_location("verify_release_target", SCRIPT)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
VERIFY_RELEASE_TARGET = util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = VERIFY_RELEASE_TARGET
MODULE_SPEC.loader.exec_module(VERIFY_RELEASE_TARGET)
REQUIRED_COMMITS = VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_COMMITS


def isolated_git_environment() -> dict[str, str]:
    """Keep test repositories independent from a caller's Git worktree."""
    return {
        name: value
        for name, value in os.environ.items()
        if not name.startswith("GIT_")
    }


class VerifyReleaseTargetTests(unittest.TestCase):
    def run_check(
        self,
        target: str,
        latest: str,
        head_ref: str = "HEAD",
        cwd: Path | None = None,
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
            cwd=cwd,
            env=isolated_git_environment(),
        )

    def source_git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args],
            cwd=SCRIPT.parents[2],
            check=True,
            capture_output=True,
            text=True,
            env=isolated_git_environment(),
        ).stdout.strip()

    def test_accepts_v0422_after_published_v0421(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.21")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_allows_idempotent_retry_after_v0422(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.22")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_v0422_before_v0421_is_published(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.20")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_skipped_target(self) -> None:
        result = self.run_check("v0.4.23", "v0.4.21")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_old_target(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.21")
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_head_containing_the_v0422_paired_updater_candidate(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.21")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_the_v0422_paired_updater_candidate(self) -> None:
        for commit in REQUIRED_COMMITS:
            parent = self.source_git("rev-parse", f"{commit}^")
            result = self.run_check("v0.4.22", "v0.4.21", parent)
            self.assertNotEqual(result.returncode, 0, commit)

    def test_accepts_the_complete_intended_release_head(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.21")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_reviewed_release_head_missing_the_required_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)
            subprocess.run(
                ["git", "init", "-q"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
                env=isolated_git_environment(),
            )

            def git(*args: str) -> str:
                return subprocess.run(
                    ["git", *args],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                ).stdout.strip()

            source_repository = self.source_git("rev-parse", "--show-toplevel")
            git(
                "fetch",
                "-q",
                source_repository,
                REQUIRED_COMMITS[0],
                VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE,
            )
            manifest_mismatch = git(
                "-c",
                "user.name=release-target-test",
                "-c",
                "user.email=release-target-test@example.invalid",
                "commit-tree",
                f"{VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE}^{{tree}}",
                "-p",
                REQUIRED_COMMITS[0],
                "-m",
                "release descendant with a mismatched manifest",
            )
            git("update-ref", "refs/heads/manifest-mismatch", manifest_mismatch)
            self.assertEqual(
                subprocess.run(
                    [
                        "git",
                        "merge-base",
                        "--is-ancestor",
                        REQUIRED_COMMITS[0],
                        manifest_mismatch,
                    ],
                    cwd=repository,
                    check=False,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                ).returncode,
                0,
            )

            result = self.run_check(
                "v0.4.22", "v0.4.21", "manifest-mismatch", repository
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required release content manifest", result.stderr)

    def test_printed_manifest_matches_the_checked_in_release_head(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                "v0.4.22",
                "--print-release-manifest",
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=SCRIPT.parents[2],
            env=isolated_git_environment(),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.strip(), VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_MANIFEST_SHA256
        )

    def test_manifest_rewriter_replaces_only_a_valid_single_declaration(self) -> None:
        source = 'before\nREQUIRED_RELEASE_MANIFEST_SHA256 = "' + ("0" * 64) + '"\nafter\n'
        updated = VERIFY_RELEASE_TARGET.rewrite_required_release_manifest(source, "f" * 64)
        self.assertIn('REQUIRED_RELEASE_MANIFEST_SHA256 = "' + ("f" * 64) + '"', updated)
        with self.assertRaises(ValueError):
            VERIFY_RELEASE_TARGET.rewrite_required_release_manifest("before\n", "f" * 64)
        with self.assertRaises(ValueError):
            VERIFY_RELEASE_TARGET.rewrite_required_release_manifest(source, "F" * 64)

    def test_manifest_update_rejects_a_head_without_the_required_base_commit(self) -> None:
        parent = self.source_git("rev-parse", f"{REQUIRED_COMMITS[0]}^")
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                "v0.4.22",
                "--head-ref",
                parent,
                "--update-release-manifest",
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=SCRIPT.parents[2],
            env=isolated_git_environment(),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("manifest update failed", result.stderr)

    def test_rejects_release_candidate_before_the_pr_default_base(self) -> None:
        parent = self.source_git("rev-parse", f"{REQUIRED_COMMITS[0]}^")
        result = self.run_check("v0.4.22", "v0.4.21", parent)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(REQUIRED_COMMITS[0], result.stderr)

    def test_rejects_head_missing_current_release_paired_updater_candidate(self) -> None:
        required_commit = REQUIRED_COMMITS[0]
        parent = self.source_git("rev-parse", f"{required_commit}^")
        result = self.run_check("v0.4.22", "v0.4.21", parent)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(required_commit, result.stderr)

    def test_accepts_actual_head_equivalent_squash_with_required_base_ancestry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = Path(temporary_directory)
            repository = temporary_root / "candidate"
            fresh_origin = temporary_root / "fresh-origin.git"
            fresh_repository = temporary_root / "fresh-candidate"
            isolation_parent = temporary_root / "pre-push-parent.git"
            inherited_work_tree = temporary_root / "inherited-work-tree"

            repository.mkdir()
            subprocess.run(
                ["git", "init", "--bare", "-q", str(isolation_parent)],
                check=True,
                capture_output=True,
                text=True,
                env=isolated_git_environment(),
            )
            source_repository = Path(self.source_git("rev-parse", "--show-toplevel"))
            source_config_before = self.source_git("config", "--local", "--null", "--list")
            source_head_before = self.source_git("rev-parse", "HEAD")
            source_refs_before = self.source_git(
                "for-each-ref", "--format=%(refname) %(objectname)", "refs/heads"
            )
            parent_config_before = (isolation_parent / "config").read_bytes()
            parent_refs_before = subprocess.run(
                [
                    "git",
                    f"--git-dir={isolation_parent}",
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/heads",
                ],
                check=True,
                capture_output=True,
                text=True,
                env=isolated_git_environment(),
            ).stdout

            def git(*args: str) -> str:
                return subprocess.run(
                    ["git", *args],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                ).stdout.strip()

            with patch.dict(
                os.environ,
                {
                    "GIT_DIR": str(isolation_parent),
                    "GIT_WORK_TREE": str(inherited_work_tree),
                    "GIT_COMMON_DIR": str(isolation_parent),
                    "GIT_INDEX_FILE": str(isolation_parent / "index"),
                    "GIT_OBJECT_DIRECTORY": str(isolation_parent / "objects"),
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES": str(isolation_parent / "objects"),
                    "GIT_CONFIG_GLOBAL": str(isolation_parent / "config"),
                },
            ):
                git("init", "-q")
                git("config", "user.email", "release-test@example.invalid")
                git("config", "user.name", "Release Target Test")
                git("fetch", "-q", str(source_repository), "HEAD:refs/heads/release")
                git("branch", "-f", "base", REQUIRED_COMMITS[0])
                squash = git(
                    "commit-tree",
                    # Model the documented squash exception from the reviewed
                    # release head. The release comparison itself must not
                    # retain that source commit after the squash.
                    "release^{tree}",
                    "-p",
                    "base",
                    "-m", "actual release tree as a squash",
                )
                git("branch", "-f", "candidate", squash)
                source_head = self.source_git("rev-parse", "HEAD")
                subprocess.run(
                    ["git", "init", "--bare", "-q", str(fresh_origin)],
                    check=True,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                )
                git("push", "-q", str(fresh_origin), "candidate:refs/heads/candidate")
                subprocess.run(
                    [
                        "git",
                        "clone",
                        "-q",
                        "--no-local",
                        "--branch",
                        "candidate",
                        str(fresh_origin),
                        str(fresh_repository),
                    ],
                    check=True,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                )

                def fresh_git(*args: str) -> subprocess.CompletedProcess[str]:
                    return subprocess.run(
                        ["git", *args],
                        cwd=fresh_repository,
                        check=False,
                        capture_output=True,
                        text=True,
                        env=isolated_git_environment(),
                    )

                self.assertNotEqual(
                    fresh_git("cat-file", "-e", f"{source_head}^{{commit}}").returncode,
                    0,
                )

                accepted = self.run_check("v0.4.22", "v0.4.21", "HEAD", fresh_repository)
                self.assertEqual(accepted.returncode, 0, accepted.stderr)

                invalid_squash = git(
                    "commit-tree",
                    "release^{tree}",
                    "-p",
                    self.source_git("rev-parse", f"{REQUIRED_COMMITS[0]}^"),
                    "-m",
                    "release tree without the required default base",
                )
                git("branch", "-f", "invalid", invalid_squash)
                rejected = self.run_check("v0.4.22", "v0.4.21", "invalid", repository)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn(REQUIRED_COMMITS[0], rejected.stderr)

                arbitrary_tree_with_required_base = git(
                    "commit-tree",
                    f"{VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE}^{{tree}}",
                    "-p",
                    "base",
                    "-m",
                    "arbitrary tree with the required default base",
                )
                git("branch", "-f", "arbitrary-tree", arbitrary_tree_with_required_base)
                rejected_manifest = self.run_check(
                    "v0.4.22", "v0.4.21", "arbitrary-tree", repository
                )
                self.assertNotEqual(rejected_manifest.returncode, 0)
                self.assertIn("required release content manifest", rejected_manifest.stderr)

            self.assertEqual(
                self.source_git("config", "--local", "--null", "--list"),
                source_config_before,
            )
            self.assertEqual(self.source_git("rev-parse", "HEAD"), source_head_before)
            self.assertEqual(
                self.source_git(
                    "for-each-ref", "--format=%(refname) %(objectname)", "refs/heads"
                ),
                source_refs_before,
            )
            self.assertEqual((isolation_parent / "config").read_bytes(), parent_config_before)
            self.assertEqual(
                subprocess.run(
                    [
                        "git",
                        f"--git-dir={isolation_parent}",
                        "for-each-ref",
                        "--format=%(refname) %(objectname)",
                        "refs/heads",
                    ],
                    check=True,
                    capture_output=True,
                    text=True,
                    env=isolated_git_environment(),
                ).stdout,
                parent_refs_before,
            )

    def test_preflight_runs_the_release_target_gate_once_through_release_check(self) -> None:
        workflow = (SCRIPT.parents[2] / ".github/workflows/release-preflight.yml").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("- name: Release target check", workflow)
        start = workflow.index("- name: Release check")
        end = workflow.find("\n      - name:", start + 1)
        body = workflow[start:] if end == -1 else workflow[start:end]
        self.assertIn("if: github.event_name == 'workflow_dispatch'", body)
        self.assertIn("timeout-minutes: 100", body)
        self.assertIn("env:\n          GH_TOKEN: ${{ github.token }}", body)
        self.assertIn('run: just VERSION="${{ steps.version.outputs.version }}" release-check', body)


if __name__ == "__main__":
    unittest.main()
