from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]
REPOSITORY = "owner/repository"
REPOSITORY_ID = 101
NUMBER = 999
BASE = "b" * 40
HEAD = "a" * 40
PR_URL = f"https://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}"
PR_ENDPOINT = f"repos/{REPOSITORY}/pulls/{NUMBER}"


class GovernancePreflightIdentityTest(unittest.TestCase):
    """Execute the dispatcher inline preflight at its gh-api boundary."""

    def setUp(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?"
            r"python3 - <<'PY'\n(.*?)\n          PY",
            workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        self.scope = textwrap.dedent(match.group(1))

    @staticmethod
    def repository(default_branch: object = "master", identifier: object = REPOSITORY_ID) -> dict[str, object]:
        return {"id": identifier, "full_name": REPOSITORY, "default_branch": default_branch}

    @staticmethod
    def pull(
        *,
        number: object = NUMBER,
        state: object = "open",
        base_ref: object = "master",
        base_sha: object = BASE,
        head_sha: object = HEAD,
        head_repo: object = None,
        base_repo: object | None = None,
    ) -> dict[str, object]:
        if head_repo is None:
            head_repo = {"id": REPOSITORY_ID, "full_name": REPOSITORY}
        return {
            "number": number,
            "state": state,
            "base": {"ref": base_ref, "sha": base_sha, "repo": base_repo or {"id": REPOSITORY_ID, "full_name": REPOSITORY}},
            "head": {"sha": head_sha, "repo": head_repo},
        }

    def execute(self, *, url: object = PR_URL, responses: dict[str, list[object]] | None = None) -> dict[str, str]:
        """Return scope outputs while each endpoint may provide a read sequence.

        ``False`` models a non-zero gh API response; the other values are JSON
        payloads.  This intentionally tests the production inline program, not
        a copy of its identity predicates.
        """
        supplied = responses or {}
        reads: dict[str, int] = {}

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = arguments[-1]
            sequence = supplied.get(endpoint)
            index = reads.get(endpoint, 0)
            reads[endpoint] = index + 1
            value = sequence[min(index, len(sequence) - 1)] if sequence else self.repository() if endpoint == f"repos/{REPOSITORY}" else self.pull()
            if value is False:
                return subprocess.CompletedProcess(arguments, 1, "", "unavailable")
            return subprocess.CompletedProcess(arguments, 0, json.dumps(value), "")

        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "scope-output"
            environment = {
                "GITHUB_REPOSITORY": REPOSITORY,
                "GITHUB_OUTPUT": str(output),
                "EVENT_NAME": "issue_comment",
                "EVENT_ACTION": "deleted",
                "ISSUE_NUMBER": str(NUMBER),
                "ISSUE_PULL_REQUEST_URL": url,
            }
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                exec(self.scope, {"__name__": "__main__"})
            return dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())

    def assert_fail_closed(self, result: dict[str, str]) -> None:
        self.assertEqual(result["valid"], "false")
        self.assertEqual(result["reconcile"], "true")
        self.assertEqual(result["issue_event_noop"], "false")

    def test_deleted_fork_comment_is_a_verified_noop_after_two_reads(self) -> None:
        deleted_fork = self.pull()
        deleted_fork["head"] = {"sha": HEAD, "repo": None}
        result = self.execute(responses={PR_ENDPOINT: [deleted_fork, deleted_fork]})
        self.assertEqual(result["valid"], "true")
        self.assertEqual(result["reconcile"], "false")
        self.assertEqual(result["issue_event_noop"], "true")

    def test_issue_comment_url_identity_rejects_noncanonical_paths(self) -> None:
        for url in (
            f"http://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}",
            f"https://api.github.com/repos/other/repository/pulls/{NUMBER}",
            f"https://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}/",
            f"https://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}?page=1",
            f"https://api.github.com/repos/{REPOSITORY}/pulls/{NUMBER}#fragment",
        ):
            with self.subTest(url=url):
                self.assert_fail_closed(self.execute(url=url))

    def test_repository_and_pull_api_failures_cannot_be_quiet_noops(self) -> None:
        repository_endpoint = f"repos/{REPOSITORY}"
        for endpoint, sequence in (
            (repository_endpoint, [False]),
            (PR_ENDPOINT, [False]),
            (PR_ENDPOINT, [self.pull(), False]),
            (repository_endpoint, [self.repository(), False]),
        ):
            with self.subTest(endpoint=endpoint, sequence=sequence):
                self.assert_fail_closed(self.execute(responses={endpoint: sequence}))

    def test_repository_identity_and_default_branch_races_fail_closed(self) -> None:
        endpoint = f"repos/{REPOSITORY}"
        for final in (
            self.repository(identifier=202),
            {"id": REPOSITORY_ID, "full_name": "other/repository", "default_branch": "master"},
            self.repository(default_branch="trunk"),
        ):
            with self.subTest(final=final):
                self.assert_fail_closed(self.execute(responses={endpoint: [self.repository(), final]}))

    def test_pr_races_stay_in_the_resolver_lane(self) -> None:
        transitions = (
            (self.pull(state="closed"), self.pull(state="open")),
            (self.pull(base_ref="release/v1"), self.pull(base_ref="master")),
            (self.pull(base_sha="c" * 40), self.pull(base_sha=BASE)),
            (self.pull(head_sha="d" * 40), self.pull(head_sha=HEAD)),
        )
        for initial, final in transitions:
            with self.subTest(initial=initial, final=final):
                result = self.execute(responses={PR_ENDPOINT: [initial, final]})
                self.assertEqual(result["reconcile"], "true")
                self.assertEqual(result["issue_event_noop"], "false")

    def test_malformed_pr_fields_cannot_be_mistaken_for_a_noop(self) -> None:
        malformed = (
            self.pull(number=True),
            self.pull(number=0),
            self.pull(state=True),
            self.pull(state=[]),
            self.pull(state={"open": True}),
            self.pull(state="merged"),
            {"state": "open", "base": {}, "head": {}},
            {"number": NUMBER, "state": "open", "base": {}, "head": {"sha": HEAD}},
            self.pull(head_repo={"id": REPOSITORY_ID}),
            self.pull(head_repo={"id": True, "full_name": REPOSITORY}),
            self.pull(head_repo=[]),
            self.pull(base_repo={"id": True, "full_name": REPOSITORY}),
            self.pull(base_ref=True),
        )
        for source in malformed:
            with self.subTest(source=source):
                self.assert_fail_closed(self.execute(responses={PR_ENDPOINT: [source, source]}))


if __name__ == "__main__":
    unittest.main()
