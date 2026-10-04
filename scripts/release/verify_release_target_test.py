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
REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_SPEC = util.spec_from_file_location("verify_release_target", SCRIPT)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
VERIFY_RELEASE_TARGET = util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = VERIFY_RELEASE_TARGET
MODULE_SPEC.loader.exec_module(VERIFY_RELEASE_TARGET)
REQUIRED_RELEASE_COMMITS = VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_COMMITS
REQUIRED_CANDIDATE_ANCESTORS = (
    VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_CANDIDATE_ANCESTORS
)
CURRENT_RELEASE_VALIDATION_SNAPSHOT = "fe7260ef1a79f9f9cbe5dd9c5cb39e3c3bc73317"
PUBLISHED_V0422_SNAPSHOT = "185d056de67282a4056729296e57c164a6a343d6"
PUBLISHED_V0422_RELEASE_BASE = "7f984d16400fe3e097a3380dd7641d78c5852352"
PUBLISHED_V0422_RELEASE_MANIFEST = "1ddeddc37c0735a7278755f26f8985aee9176fc978b437766d57be7d0e342d9e"
PUBLISHED_V0422_REQUIRED_PINS = (
    "1bc497bdfd3b8c4318e9ee1609d147b6925b43e5",
    "d0ab9c408e9b876f130dbd5a8f33051cc44b5242",
    "78ed1b87cc94bcdd158b42e7c00e69e882bbb094",
    "a8481e9c13ceb43d9e08388958c32f507f9ddb85",
    "f5ff74ecf7287375bc0e835e8db374766145a55b",
    "4d1ec0a03c6fd95a8ca9f4c1ad8a30d4ad84f76e",
)


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
            cwd=REPOSITORY_ROOT,
            check=True,
            capture_output=True,
            text=True,
            env=isolated_git_environment(),
        ).stdout.strip()

    def historical_v0422_checker(self) -> str:
        return self.source_git(
            "show",
            f"{PUBLISHED_V0422_SNAPSHOT}:scripts/release/verify-release-target.py",
        )

    def run_historical_check(
        self,
        source: str,
        target: str,
        latest: str,
        head_ref: str = PUBLISHED_V0422_SNAPSHOT,
        cwd: Path = REPOSITORY_ROOT,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-c",
                source,
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

    def test_published_v0422_checker_fixture_preserves_historical_contract(self) -> None:
        source = self.historical_v0422_checker()
        self.assertIn(f'REQUIRED_TARGET_RELEASE = "v0.4.22"', source)
        self.assertIn(f'REQUIRED_RELEASE_BASE = "{PUBLISHED_V0422_RELEASE_BASE}"', source)
        self.assertIn(
            f'REQUIRED_RELEASE_MANIFEST_SHA256 = "{PUBLISHED_V0422_RELEASE_MANIFEST}"',
            source,
        )
        for commit in PUBLISHED_V0422_REQUIRED_PINS:
            self.assertIn(commit, source)

        accepted = self.run_historical_check(source, "v0.4.22", "v0.4.21")
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        retry = self.run_historical_check(source, "v0.4.22", "v0.4.22")
        self.assertEqual(retry.returncode, 0, retry.stderr)
        for target, latest in (
            ("v0.4.23", "v0.4.22"),
            ("v0.4.22", "v0.4.20"),
            ("v0.5.0", "v0.4.21"),
        ):
            rejected = self.run_historical_check(source, target, latest)
            self.assertNotEqual(rejected.returncode, 0, (target, latest))

        missing_pin_parent = self.source_git(
            "rev-parse", f"{PUBLISHED_V0422_REQUIRED_PINS[0]}^"
        )
        missing_pin = self.run_historical_check(
            source, "v0.4.22", "v0.4.21", missing_pin_parent
        )
        self.assertNotEqual(missing_pin.returncode, 0)

    def test_published_v0422_checker_fixture_prints_its_historical_manifest(self) -> None:
        source = self.historical_v0422_checker()
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                source,
                "--target-version",
                "v0.4.22",
                "--head-ref",
                PUBLISHED_V0422_SNAPSHOT,
                "--print-release-manifest",
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=REPOSITORY_ROOT,
            env=isolated_git_environment(),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), PUBLISHED_V0422_RELEASE_MANIFEST)

    def test_accepts_v0423_after_published_v0422(self) -> None:
        result = self.run_check(
            "v0.4.23", "v0.4.22", CURRENT_RELEASE_VALIDATION_SNAPSHOT
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_allows_idempotent_retry_after_v0423(self) -> None:
        result = self.run_check(
            "v0.4.23", "v0.4.23", CURRENT_RELEASE_VALIDATION_SNAPSHOT
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_v0423_before_v0422_is_published(self) -> None:
        result = self.run_check("v0.4.23", "v0.4.21")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_skipped_target(self) -> None:
        result = self.run_check("v0.4.24", "v0.4.22")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_old_target(self) -> None:
        result = self.run_check("v0.4.22", "v0.4.22")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_published_v0422_as_a_fake_v0423_candidate(self) -> None:
        result = self.run_check(
            "v0.4.23", "v0.4.22", PUBLISHED_V0422_SNAPSHOT
        )
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_the_v0423_release_validation_snapshot(self) -> None:
        result = self.run_check(
            "v0.4.23", "v0.4.22", CURRENT_RELEASE_VALIDATION_SNAPSHOT
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_a_v0423_candidate_ancestor(self) -> None:
        for commit in REQUIRED_CANDIDATE_ANCESTORS:
            parent = self.source_git("rev-parse", f"{commit}^")
            result = self.run_check("v0.4.23", "v0.4.22", parent)
            self.assertNotEqual(result.returncode, 0, commit)

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
                f"{self.source_git('rev-parse', 'HEAD')}:refs/heads/release",
            )
            release_head = git("rev-parse", "refs/heads/release")
            git(
                "read-tree",
                f"{release_head}^{{tree}}",
            )
            altered_blob = subprocess.run(
                ["git", "hash-object", "-w", "--stdin"],
                cwd=repository,
                check=True,
                capture_output=True,
                input=b"intentionally altered release payload\n",
                env=isolated_git_environment(),
            ).stdout.decode("ascii").strip()
            git("update-index", "--cacheinfo", f"100644,{altered_blob},docs/release.md")
            altered_tree = git("write-tree")
            manifest_mismatch = git(
                "-c",
                "user.name=release-target-test",
                "-c",
                "user.email=release-target-test@example.invalid",
                "commit-tree",
                altered_tree,
                "-p",
                release_head,
                "-m",
                "release descendant with a mismatched manifest",
            )
            git("update-ref", "refs/heads/manifest-mismatch", manifest_mismatch)
            for required_commit in REQUIRED_CANDIDATE_ANCESTORS:
                self.assertEqual(
                    subprocess.run(
                        [
                            "git",
                            "merge-base",
                            "--is-ancestor",
                            required_commit,
                            manifest_mismatch,
                        ],
                        cwd=repository,
                        check=False,
                        capture_output=True,
                        text=True,
                        env=isolated_git_environment(),
                    ).returncode,
                    0,
                    required_commit,
                )

            result = self.run_check("v0.4.23", "v0.4.22", "manifest-mismatch", repository)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required release content manifest", result.stderr)

    def test_printed_manifest_matches_the_checked_in_release_head(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                "v0.4.23",
                "--head-ref",
                "HEAD",
                "--print-release-manifest",
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=REPOSITORY_ROOT,
            env=isolated_git_environment(),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.strip(), VERIFY_RELEASE_TARGET.release_manifest_sha256("HEAD")
        )

    def test_manifest_rewriter_replaces_only_a_valid_single_declaration(self) -> None:
        source = 'before\nREQUIRED_RELEASE_MANIFEST_SHA256 = "' + ("0" * 64) + '"\nafter\n'
        updated = VERIFY_RELEASE_TARGET.rewrite_required_release_manifest(source, "f" * 64)
        self.assertIn('REQUIRED_RELEASE_MANIFEST_SHA256 = "' + ("f" * 64) + '"', updated)
        with self.assertRaises(ValueError):
            VERIFY_RELEASE_TARGET.rewrite_required_release_manifest("before\n", "f" * 64)
        with self.assertRaises(ValueError):
            VERIFY_RELEASE_TARGET.rewrite_required_release_manifest(source, "F" * 64)

    def test_manifest_update_rejects_a_head_without_the_required_source_commit(self) -> None:
        parent = self.source_git("rev-parse", f"{REQUIRED_RELEASE_COMMITS[0]}^")
        self.assertEqual(
            self.source_git("merge-base", "--is-ancestor", REQUIRED_CANDIDATE_ANCESTORS[0], parent),
            "",
        )
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                "v0.4.23",
                "--head-ref",
                parent,
                "--update-release-manifest",
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=REPOSITORY_ROOT,
            env=isolated_git_environment(),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("manifest update failed", result.stderr)

    def test_rejects_release_candidate_before_the_pr_default_base(self) -> None:
        parent = self.source_git("rev-parse", f"{REQUIRED_CANDIDATE_ANCESTORS[0]}^")
        result = self.run_check("v0.4.23", "v0.4.22", parent)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(REQUIRED_CANDIDATE_ANCESTORS[0], result.stderr)

    def test_rejects_head_missing_current_release_candidate_ancestor(self) -> None:
        required_commit = REQUIRED_RELEASE_COMMITS[0]
        parent = self.source_git("rev-parse", f"{required_commit}^")
        result = self.run_check("v0.4.23", "v0.4.22", parent)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(required_commit, result.stderr)

    def test_rejects_equivalent_squash_candidate_missing_required_release_commits(
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
                git(
                    "fetch",
                    "-q",
                    str(source_repository),
                    f"{source_head_before}:refs/heads/release",
                )
                squash = git(
                    "commit-tree",
                    # Model GitHub's squash merge: the candidate retains the
                    # reviewed default base but not the PR source commit.
                    "release^{tree}",
                    "-p",
                    REQUIRED_CANDIDATE_ANCESTORS[0],
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
                self.assertNotEqual(
                    fresh_git(
                        "cat-file", "-e", f"{REQUIRED_RELEASE_COMMITS[0]}^{{commit}}"
                    ).returncode,
                    0,
                )

                rejected_squash = self.run_check(
                    "v0.4.23", "v0.4.22", "HEAD", fresh_repository
                )
                self.assertNotEqual(rejected_squash.returncode, 0)
                self.assertIn(REQUIRED_RELEASE_COMMITS[0], rejected_squash.stderr)

                invalid_squash = git(
                    "commit-tree",
                    "release^{tree}",
                    "-p",
                    f"{VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE}^",
                    "-m",
                    "release tree without the reviewed default base",
                )
                git("branch", "-f", "invalid", invalid_squash)
                rejected = self.run_check("v0.4.23", "v0.4.22", "invalid", repository)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn(REQUIRED_CANDIDATE_ANCESTORS[0], rejected.stderr)

                arbitrary_tree_with_required_base = git(
                    "commit-tree",
                    f"{VERIFY_RELEASE_TARGET.REQUIRED_RELEASE_BASE}^{{tree}}",
                    "-p",
                    "release",
                    "-m",
                    "arbitrary release tree with all required commits",
                )
                git("branch", "-f", "arbitrary-tree", arbitrary_tree_with_required_base)
                for required_commit in REQUIRED_CANDIDATE_ANCESTORS:
                    self.assertEqual(
                        subprocess.run(
                            [
                                "git",
                                "merge-base",
                                "--is-ancestor",
                                required_commit,
                                "arbitrary-tree",
                            ],
                            cwd=repository,
                            check=False,
                            capture_output=True,
                            text=True,
                            env=isolated_git_environment(),
                        ).returncode,
                        0,
                        required_commit,
                    )
                rejected_manifest = self.run_check(
                    "v0.4.23", "v0.4.22", "arbitrary-tree", repository
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
