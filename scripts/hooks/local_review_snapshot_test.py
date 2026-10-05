from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import local_review_state
from local_review_state import ReviewError, issue_context


CANONICAL_REPOSITORY = "HiroyukiFuruno/katana-render-runtime"


class RepositoryOriginIdentityTest(unittest.TestCase):
    def issue_payload(self, repository: str = CANONICAL_REPOSITORY) -> dict[str, object]:
        return {
            "number": 105,
            "title": "Fixture issue",
            "body": "Expected behavior",
            "state": "open",
            "html_url": f"https://github.com/{repository}/issues/105",
        }

    def command_for_origin(self, origin: str, api_calls: list[list[str]]):
        def fake_command(arguments: list[str], root: Path, input_bytes: bytes | None = None) -> str:
            if arguments == ["git", "remote", "get-url", "origin"]:
                return origin
            api_calls.append(arguments)
            return json.dumps(self.issue_payload())

        return fake_command

    def test_case_variants_use_canonical_api_and_html_url_identity(self) -> None:
        origins = (
            "https://github.com/hiroyukifuruno/KATANA-RENDER-RUNTIME.git",
            "git@github.com:HiroyukiFuruno/Katana-Render-Runtime.git",
        )
        for origin in origins:
            with self.subTest(origin=origin):
                api_calls: list[list[str]] = []
                with patch.object(local_review_state, "command", self.command_for_origin(origin, api_calls)):
                    result = issue_context(Path("."), [105])

                self.assertEqual(result[0]["html_url"],
                                 f"https://github.com/{CANONICAL_REPOSITORY}/issues/105")
                self.assertEqual(api_calls, [["gh", "api", "--hostname", "github.com",
                                              f"repos/{CANONICAL_REPOSITORY}/issues/105"]])

    def test_issue_api_pins_github_host_when_gh_host_environment_targets_enterprise(self) -> None:
        invocations: list[tuple[list[str], dict[str, object]]] = []

        def fake_run(arguments: list[str], **options: object) -> SimpleNamespace:
            invocations.append((arguments, options))
            if arguments[2] == "git":
                return SimpleNamespace(stdout="https://github.com/HiroyukiFuruno/katana-render-runtime.git\n")
            if arguments[2] == "gh":
                return SimpleNamespace(stdout=json.dumps(self.issue_payload()))
            raise AssertionError(f"unexpected executable: {arguments}")

        with patch.dict("os.environ", {"GH_HOST": "enterprise.example"}), \
                patch.object(local_review_state.shutil, "which", return_value="/test-bin/rtk"), \
                patch.object(local_review_state.subprocess, "run", side_effect=fake_run):
            result = issue_context(Path("."), [105])

        api_arguments, api_options = invocations[-1]
        self.assertEqual(
            api_arguments,
            ["/test-bin/rtk", "proxy", "gh", "api", "--hostname", "github.com",
             f"repos/{CANONICAL_REPOSITORY}/issues/105"],
        )
        self.assertEqual(api_options["env"]["GH_HOST"], "enterprise.example")
        self.assertEqual(result[0]["html_url"], f"https://github.com/{CANONICAL_REPOSITORY}/issues/105")

    def test_non_github_and_different_repository_origins_are_rejected(self) -> None:
        origins = (
            "https://example.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/katana-render-runtime-extra.git",
        )
        for origin in origins:
            with self.subTest(origin=origin):
                api_calls: list[list[str]] = []
                with patch.object(local_review_state, "command", self.command_for_origin(origin, api_calls)):
                    with self.assertRaises(ReviewError):
                        issue_context(Path("."), [105])
                self.assertEqual(api_calls, [])

    def test_noncanonical_api_html_url_is_rejected_after_canonical_request(self) -> None:
        api_calls: list[list[str]] = []

        def fake_command(arguments: list[str], root: Path, input_bytes: bytes | None = None) -> str:
            if arguments == ["git", "remote", "get-url", "origin"]:
                return "https://github.com/hiroyukifuruno/katana-render-runtime.git"
            api_calls.append(arguments)
            return json.dumps(self.issue_payload("hiroyukifuruno/katana-render-runtime"))

        with patch.object(local_review_state, "command", fake_command):
            with self.assertRaisesRegex(ReviewError, "URL does not match"):
                issue_context(Path("."), [105])
        self.assertEqual(api_calls, [["gh", "api", "--hostname", "github.com",
                                      f"repos/{CANONICAL_REPOSITORY}/issues/105"]])


class SelectedRemoteIdentityTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def add_remote(self, name: str, url: str) -> None:
        subprocess.run(["git", "remote", "add", name, url], cwd=self.root, check=True)

    def test_case_variant_github_host_matches_configured_remote(self) -> None:
        configured = "git@github.com:HiroyukiFuruno/katana-render-runtime.git"
        selected = "git@GitHub.com:HiroyukiFuruno/katana-render-runtime.git"
        self.add_remote("origin", configured)
        actual_command = local_review_state.command
        api_calls: list[list[str]] = []

        def track_api(arguments: list[str], root: Path, input_bytes: bytes | None = None) -> str:
            if arguments[0] == "gh":
                api_calls.append(arguments)
                return json.dumps(RepositoryOriginIdentityTest().issue_payload())
            return actual_command(arguments, root, input_bytes)

        with patch.object(local_review_state, "command", track_api):
            result = issue_context(self.root, [105], selected)

        self.assertEqual(result[0]["html_url"],
                         f"https://github.com/{CANONICAL_REPOSITORY}/issues/105")
        self.assertEqual(api_calls, [["gh", "api", "--hostname", "github.com",
                                      f"repos/{CANONICAL_REPOSITORY}/issues/105"]])

    def test_case_variant_matching_multiple_remotes_remains_ambiguous(self) -> None:
        configured = "git@github.com:HiroyukiFuruno/katana-render-runtime.git"
        self.add_remote("origin", configured)
        self.add_remote("upstream", "https://github.com/hiroyukifuruno/KATANA-RENDER-RUNTIME")
        selected = "git@GitHub.com:HiroyukiFuruno/katana-render-runtime.git"
        with self.assertRaisesRegex(ReviewError, "does not identify one configured remote"):
            local_review_state.resolve_remote_name(self.root, selected)

    def test_malformed_non_github_and_other_repository_selections_fail_closed(self) -> None:
        self.add_remote("origin", "git@github.com:HiroyukiFuruno/katana-render-runtime.git")
        selected_urls = (
            "git@github.com:HiroyukiFuruno/katana-render-runtime%ZZ.git",
            "https://example.com/HiroyukiFuruno/katana-render-runtime.git",
            "https://github.com/HiroyukiFuruno/other-repository.git",
        )
        actual_command = local_review_state.command
        api_calls: list[list[str]] = []

        def track_api(arguments: list[str], root: Path, input_bytes: bytes | None = None) -> str:
            if arguments[0] == "gh":
                api_calls.append(arguments)
            return actual_command(arguments, root, input_bytes)

        for selected in selected_urls:
            with self.subTest(selected=selected), \
                    patch.object(local_review_state, "command", track_api):
                with self.assertRaises(ReviewError):
                    issue_context(self.root, [105], selected)
        self.assertEqual(api_calls, [])


class ReplaceRefSnapshotTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.name", "Review Test"], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.email", "review@example.invalid"], check=True)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def git(self, *arguments: str) -> str:
        result = subprocess.run(["git", *arguments], cwd=self.root, check=True,
                                capture_output=True, text=True)
        return result.stdout.strip()

    def test_replace_ref_cannot_hide_review_delta_or_reuse_empty_snapshot(self) -> None:
        (self.root / "review.txt").write_text("base\n")
        self.git("add", "review.txt")
        self.git("commit", "-m", "base")
        base = self.git("rev-parse", "HEAD")
        (self.root / "review.txt").write_text("head change\n")
        self.git("commit", "-am", "review change")
        head = self.git("rev-parse", "HEAD")
        self.git("replace", base, head)

        self.assertEqual(local_review_state.command(
            ["git", "diff", "--no-renames", "--name-only", "-z", base, head, "--"], self.root
        ), "review.txt\0")
        snapshot = local_review_state.source_snapshot(self.root, base)

        self.assertEqual(snapshot["base_sha"], base)
        self.assertNotEqual(snapshot["base_sha"], head)
        self.assertIn("review.txt", snapshot["working_blobs"])


if __name__ == "__main__":
    unittest.main()
