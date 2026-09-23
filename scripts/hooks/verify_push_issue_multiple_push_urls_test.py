from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import verify_push_issue as subject


class MultiplePushUrlsTest(unittest.TestCase):
    repository = Path("/tmp/repository")
    first_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
    second_url = "git@github.com:HiroyukiFuruno/katana-render-runtime.git"

    def run_git(self, _repository: Path, *arguments: str) -> str:
        if arguments == ("remote",):
            return "origin\n"
        if arguments == ("remote", "get-url", "--push", "--all", "origin"):
            return f"{self.first_url}\n{self.second_url}\n"
        raise AssertionError(f"unexpected git invocation: {arguments}")

    def test_second_configured_push_url_is_allowed(self) -> None:
        with patch.object(subject, "_run_git", side_effect=self.run_git):
            self.assertEqual(
                subject._remote_for_push(
                    self.repository,
                    remote_name="origin",
                    remote_url=self.second_url,
                    fallback_branch=None,
                ),
                ("origin", self.second_url),
            )

    def test_push_url_whitespace_is_stripped(self) -> None:
        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return f"  {self.first_url}  \n\n"
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            self.assertEqual(
                subject._effective_remote_urls(self.repository, "origin"),
                (self.first_url,),
            )

    def test_unconfigured_push_url_is_rejected(self) -> None:
        with patch.object(subject, "_run_git", side_effect=self.run_git):
            with self.assertRaises(subject.ContractViolation):
                subject._remote_for_push(
                    self.repository,
                    remote_name="origin",
                    remote_url="https://github.com/HiroyukiFuruno/other.git",
                    fallback_branch=None,
                )

    def test_push_urls_for_different_repositories_are_rejected(self) -> None:
        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return f"{self.first_url}\nhttps://github.com/example/other.git\n"
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            with self.assertRaises(subject.ContractViolation):
                subject._remote_for_push(
                    self.repository,
                    remote_name="origin",
                    remote_url=self.first_url,
                    fallback_branch=None,
                )


if __name__ == "__main__":
    unittest.main()
