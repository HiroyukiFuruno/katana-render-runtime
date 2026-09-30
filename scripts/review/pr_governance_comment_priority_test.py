from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "owner/repository"
NUMBER = 72
PR_URL = f"https://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}"


class GovernanceCommentPriorityTest(unittest.TestCase):
    """Execute the resolver against a bounded fake gh API boundary."""

    @staticmethod
    def pull(*, base_ref: str = "master", head_repository: str = REPOSITORY) -> dict[str, object]:
        return {
            "number": NUMBER,
            "state": "open",
            "body": "",
            "base": {"ref": base_ref, "repo": {"full_name": REPOSITORY}},
            "head": {"sha": "a" * 40, "repo": {"full_name": head_repository}},
        }

    def resolve(
        self,
        *,
        url: str = PR_URL,
        pulls: list[dict[str, object]] | None = None,
        issue_number: str = str(NUMBER),
    ) -> dict[str, object]:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "output"
            gh = directory / "gh"
            gh.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "if any('pulls?state=open&per_page=100&page=1' in arg for arg in sys.argv):\n"
                " print(os.environ['PULLS'])\n"
                "else:\n"
                " sys.exit(91)\n",
                encoding="utf-8",
            )
            gh.chmod(0o755)
            environment = os.environ | {
                "GITHUB_REPOSITORY": REPOSITORY,
                "EVENT_NAME": "issue_comment",
                "DEFAULT_BRANCH": "master",
                "ISSUE_NUMBER": issue_number,
                "ISSUE_PULL_REQUEST_URL": url,
                "GITHUB_OUTPUT": str(output),
                "PULLS": json.dumps(pulls if pulls is not None else [self.pull()]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, str(ROOT / "scripts/review/pr_governance_resolve_event.py")],
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode != 0:
                return {"error": result.stderr}
            return {
                key: json.loads(value) if key.endswith("targets") else value
                for key, value in (line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            }

    def test_current_local_default_pr_comment_is_priority(self) -> None:
        result = self.resolve()
        self.assertEqual(result["event_targets"], [NUMBER])
        self.assertEqual(result["priority_targets"], [NUMBER])

    def test_ordinary_issue_comment_is_not_priority(self) -> None:
        result = self.resolve(url="")
        self.assertEqual(result["priority_targets"], [])

    def test_unrelated_issue_pr_url_is_not_priority(self) -> None:
        result = self.resolve(url=f"https://api.github.com/repos/{REPOSITORY}/pulls/73")
        self.assertEqual(result["event_targets"], [])
        self.assertEqual(result["priority_targets"], [])

    def test_malformed_issue_number_fails_closed(self) -> None:
        result = self.resolve(issue_number="072")
        self.assertIn("Issue event number is invalid", result["error"])

    def test_malformed_or_non_governed_pr_url_is_not_priority(self) -> None:
        for url, pull in (
            (PR_URL + "/", self.pull()),
            (PR_URL + "?page=1", self.pull()),
            (PR_URL + "#fragment", self.pull()),
            ("https://api.github.com/repos/other/repository/pulls/72", self.pull()),
            (PR_URL, self.pull(base_ref="release/v1")),
            (PR_URL, self.pull(head_repository="fork/repository")),
        ):
            with self.subTest(url=url, pull=pull):
                result = self.resolve(url=url, pulls=[pull])
                self.assertEqual(result["priority_targets"], [])


if __name__ == "__main__":
    unittest.main()
