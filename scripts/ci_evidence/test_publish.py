from __future__ import annotations

import base64
import copy
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))

from publish import ACTIONS_APP_ID, PublishError, complete_check, publish


SHA_A = "a" * 40
SHA_B = "b" * 40


def content(value: bytes) -> dict[str, str]:
    return {"encoding": "base64", "content": base64.b64encode(value).decode("ascii")}


def event() -> dict[str, object]:
    return {
        "repository": {"default_branch": "master"},
        "workflow_run": {
            "id": 42,
            "status": "completed",
            "conclusion": "success",
            "event": "pull_request",
            "path": ".github/workflows/test-and-build.yml",
            "run_attempt": 1,
            "head_sha": SHA_A,
            "head_repository": {"full_name": "acme/krr"},
            "pull_requests": [
                {
                    "number": 9,
                    "head": {"sha": SHA_A, "repo": {"full_name": "acme/krr"}},
                    "base": {"ref": "master", "sha": SHA_B, "repo": {"full_name": "acme/krr"}},
                }
            ],
        },
    }


class Adapter:
    def __init__(self) -> None:
        workflow = b"name: CI\\n"
        self.responses: dict[tuple[str, str], object] = {
            ("GET", "/repos/acme/krr/check-runs/7"): {
                "id": 7,
                "app": {"id": ACTIONS_APP_ID, "slug": "github-actions"},
                "output": {
                    "title": "Trusted Linux release-quality evidence published",
                    "summary": "manifest-sha256: " + "c" * 64,
                    "text": "source-run: 42\nartifact: ci-evidence-linux-release-quality-42",
                },
            },
            ("GET", "/repos/acme/krr/git/ref/heads/master"): {"object": {"sha": SHA_B}},
            ("GET", f"/repos/acme/krr/contents/.github/workflows/test-and-build.yml?ref={SHA_B}"): content(workflow),
            ("GET", f"/repos/acme/krr/contents/.github/workflows/test-and-build.yml?ref={SHA_A}"): content(workflow),
            ("GET", "/repos/acme/krr/actions/runs/42/jobs?per_page=100"): {
                "total_count": 1,
                "jobs": [
                    {
                        "name": "Test and Build (ubuntu-latest, linux64)",
                        "status": "completed",
                        "conclusion": "success",
                        "steps": [
                            {
                                "name": "Run shared release quality gate",
                                "status": "completed",
                                "conclusion": "success",
                            }
                        ],
                    }
                ]
            },
            ("GET", f"/repos/acme/krr/git/trees/{SHA_A}?recursive=1"): {
                "truncated": False,
                "tree": [{"path": "src/lib.rs", "mode": "100644", "type": "blob", "sha": SHA_A}],
            },
        }
        self.writes: list[tuple[str, str, object | None]] = []

    def request(self, method: str, path: str, body: object | None = None) -> object:
        if method in ("POST", "PATCH"):
            self.writes.append((method, path, body))
            return {"id": 7, "app": {"id": ACTIONS_APP_ID, "slug": "github-actions"}}
        return self.responses[(method, path)]


class PublishTests(unittest.TestCase):
    def test_publishes_canonical_input_profile_and_actions_check(self) -> None:
        adapter = Adapter()
        manifest, check_id = publish(event(), "acme/krr", 50, adapter)
        self.assertEqual(check_id, 7)
        self.assertEqual(manifest["check_id"], "linux-release-quality")
        self.assertEqual(len(manifest["input"]["digest"]), 64)
        self.assertEqual(manifest["input"]["input_tree"]["entries"][0]["path"], "src/lib.rs")
        self.assertEqual(len(manifest["profile"]["digest"]), 64)
        self.assertEqual(manifest["profile"]["profile"]["workflow_path"], ".github/workflows/test-and-build.yml")
        self.assertEqual(manifest["source_run"]["base_sha"], SHA_B)
        self.assertEqual(len(adapter.writes), 1)
        _, path, body = adapter.writes[0]
        self.assertEqual(path, "/repos/acme/krr/check-runs")
        self.assertEqual(body["head_sha"], SHA_A)
        self.assertEqual(body["details_url"], "https://github.com/acme/krr/actions/runs/50")
        self.assertEqual(body["status"], "in_progress")
        self.assertIn("ci-evidence-linux-release-quality-42", body["output"]["text"])
        self.assertEqual(body["output"]["title"], "Trusted Linux release-quality evidence published")
        complete_check("acme/krr", check_id, adapter)
        self.assertEqual(adapter.writes[1][0:2], ("PATCH", "/repos/acme/krr/check-runs/7"))
        self.assertEqual(adapter.writes[1][2]["conclusion"], "success")
        self.assertEqual(
            adapter.writes[1][2]["output"]["title"],
            "Trusted Linux release-quality evidence published",
        )
        self.assertEqual(
            adapter.writes[1][2]["output"]["summary"],
            "manifest-sha256: " + "c" * 64,
        )
        self.assertEqual(
            adapter.writes[1][2]["output"]["text"],
            "source-run: 42\nartifact: ci-evidence-linux-release-quality-42",
        )

    def test_rejects_retry_and_stale_default_branch(self) -> None:
        retried = event()
        retried["workflow_run"]["run_attempt"] = 2
        with self.assertRaisesRegex(PublishError, "first attempt"):
            publish(retried, "acme/krr", 50, Adapter())

        adapter = Adapter()
        adapter.responses[("GET", "/repos/acme/krr/git/ref/heads/master")] = {"object": {"sha": SHA_A}}
        with self.assertRaisesRegex(PublishError, "stale"):
            publish(event(), "acme/krr", 50, adapter)

    def test_rejects_forked_or_noncanonical_workflow_run(self) -> None:
        fork = event()
        fork["workflow_run"]["head_repository"]["full_name"] = "fork/krr"
        with self.assertRaisesRegex(PublishError, "local pull request"):
            publish(fork, "acme/krr", 50, Adapter())

        wrong_workflow = event()
        wrong_workflow["workflow_run"]["path"] = ".github/workflows/other.yml"
        with self.assertRaisesRegex(PublishError, "canonical CI workflow"):
            publish(wrong_workflow, "acme/krr", 50, Adapter())

    def test_rejects_workflow_byte_mismatch_or_noncanonical_job(self) -> None:
        adapter = Adapter()
        adapter.responses[
            ("GET", f"/repos/acme/krr/contents/.github/workflows/test-and-build.yml?ref={SHA_A}")
        ] = content(b"changed")
        with self.assertRaisesRegex(PublishError, "workflow bytes differ"):
            publish(event(), "acme/krr", 50, adapter)

        adapter = Adapter()
        jobs = copy.deepcopy(adapter.responses[("GET", "/repos/acme/krr/actions/runs/42/jobs?per_page=100")])
        jobs["jobs"][0]["steps"][0]["conclusion"] = "failure"
        adapter.responses[("GET", "/repos/acme/krr/actions/runs/42/jobs?per_page=100")] = jobs
        with self.assertRaisesRegex(PublishError, "did not succeed"):
            publish(event(), "acme/krr", 50, adapter)

    def test_rejects_non_actions_check_identity_before_accepting_evidence(self) -> None:
        class WrongIdentity(Adapter):
            def request(self, method: str, path: str, body: object | None = None) -> object:
                if method == "POST":
                    return {"id": 7, "app": {"id": 1, "slug": "octocat"}}
                return super().request(method, path, body)

        with self.assertRaisesRegex(PublishError, "GitHub Actions App"):
            publish(event(), "acme/krr", 50, WrongIdentity())


if __name__ == "__main__":
    unittest.main()
