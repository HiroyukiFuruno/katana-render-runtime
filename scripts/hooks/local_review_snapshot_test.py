from __future__ import annotations

import json
from pathlib import Path
import sys
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
                self.assertEqual(api_calls, [["gh", "api", f"repos/{CANONICAL_REPOSITORY}/issues/105"]])

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
        self.assertEqual(api_calls, [["gh", "api", f"repos/{CANONICAL_REPOSITORY}/issues/105"]])


if __name__ == "__main__":
    unittest.main()
