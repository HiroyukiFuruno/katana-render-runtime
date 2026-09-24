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
FINAL_RELEASE_TREE = "aa76ab0538beb70c5960077e30296340368869d9"


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

    def test_accepts_v0421_after_published_v0420(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.20")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_allows_idempotent_retry_after_v0421(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.21")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_v0421_before_v0420_is_published(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.19")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_skipped_target(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.20")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_old_target(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.20")
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_head_containing_every_v0421_issue_commit(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.20")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_a_v0421_issue_commit(self) -> None:
        for commit in REQUIRED_COMMITS[1:]:
            parent = self.source_git("rev-parse", f"{commit}^")
            result = self.run_check("v0.4.21", "v0.4.20", parent)
            self.assertNotEqual(result.returncode, 0, commit)

    def test_accepts_the_complete_intended_release_head(self) -> None:
        result = self.run_check(
            "v0.4.21", "v0.4.20", "459eef383cf953f4c97ae5b063f4b72931e4a5ee"
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_current_release_terminal(self) -> None:
        parent = self.source_git(
            "rev-parse", "459eef383cf953f4c97ae5b063f4b72931e4a5ee^"
        )
        result = self.run_check("v0.4.21", "v0.4.20", parent)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("459eef383cf953f4c97ae5b063f4b72931e4a5ee", result.stderr)

    def test_accepts_actual_head_equivalent_squash_but_rejects_changed_required_path(
        self,
    ) -> None:
        self.assertEqual(VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_TERMINAL, FINAL_RELEASE_TREE)
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = Path(temporary_directory)
            repository = temporary_root / "candidate"
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
                git("branch", "-f", "final-release", FINAL_RELEASE_TREE)
                git("branch", "-f", "base", VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE)
                squash = git(
                    "commit-tree",
                    # Model the documented squash exception with the actual
                    # final non-gate v0.4.21 release tree, rather than the
                    # terminal constant under test.
                    "final-release^{tree}",
                    "-p",
                    "base",
                    "-m",
                    "actual release tree as a squash",
                )

                accepted = self.run_check("v0.4.21", "v0.4.20", squash, repository)
                self.assertEqual(accepted.returncode, 0, accepted.stderr)

                git("switch", "-q", "-c", "changed-required-path", squash)
                changed_path = "crates/katana-render-runtime/src/markdown/svg_rasterize_text_fallback.rs"
                self.assertIn(
                    changed_path,
                    subprocess.run(
                        [
                            "git",
                            "diff",
                            "--name-only",
                            VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE,
                            VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_TERMINAL,
                        ],
                        cwd=repository,
                        check=True,
                        capture_output=True,
                        text=True,
                        env=isolated_git_environment(),
                    ).stdout.splitlines(),
                )
                with (repository / changed_path).open("a", encoding="utf-8") as manifest:
                    manifest.write("\n# 必須release pathの変更\n")
                git("add", changed_path)
                git("commit", "-q", "-m", "change required release path")
                rejected = self.run_check("v0.4.21", "v0.4.20", "HEAD", repository)
                self.assertNotEqual(rejected.returncode, 0)

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

    def test_preflight_runs_the_release_target_gate_once_through_release_specific(self) -> None:
        workflow = (SCRIPT.parents[2] / ".github/workflows/release-preflight.yml").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("- name: Release target check", workflow)
        start = workflow.index("- name: Release-specific verification")
        end = workflow.find("\n      - name:", start + 1)
        body = workflow[start:] if end == -1 else workflow[start:end]
        self.assertIn("GH_TOKEN: ${{ github.token }}", body)


if __name__ == "__main__":
    unittest.main()
