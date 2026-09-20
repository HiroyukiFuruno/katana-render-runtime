from __future__ import annotations

import base64
import copy
import io
import json
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile
from pathlib import Path
from urllib.parse import parse_qs, urlparse

sys.path.insert(0, str(Path(__file__).parent))

from consume import APP_ID, CHECK, CHECK_TITLE, EvidenceError, PROFILE, ReusePending, ReuseUnavailable, canonical_json_bytes, expected_profile, main, verify_reusable_evidence
from tree_digest import digest_input_tree


SHA = "a" * 40
BASE_SHA = "c" * 40
TREE = "b" * 40
REPO = "acme/krr"
PR = 9
REF = "release/v1.2.3"
RUN = 42
WORKFLOW = b"name: CI\n"


def tree() -> dict[str, object]:
    return {"sha": TREE, "truncated": False, "tree": [{"path": "src/lib.rs", "mode": "100644", "type": "blob", "sha": "c" * 40}]}


def zip_manifest(value: object) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", zipfile.ZIP_STORED) as archive:
        archive.writestr("manifest.json", canonical_json_bytes(value) + b"\n")
    return output.getvalue()


class Api:
    def __init__(self) -> None:
        input_tree = digest_input_tree(tree())
        manifest = {"schema": "krr-ci-evidence-publisher-v1", "check_id": "linux-release-quality", "input": input_tree, "profile": expected_profile(WORKFLOW), "source_run": {"id": RUN, "head_sha": SHA, "base_sha": BASE_SHA, "pull_request": PR, "run_attempt": 1, "workflow_path": ".github/workflows/test-and-build.yml"}}
        run = {"id": RUN, "event": "pull_request", "head_sha": SHA, "head_branch": REF, "path": ".github/workflows/test-and-build.yml", "run_attempt": 1, "status": "completed", "conclusion": "success", "head_repository": {"full_name": REPO}, "pull_requests": [{"number": PR}]}
        self.values: dict[str, object] = {
            f"/repos/{REPO}/pulls/{PR}": {"number": PR, "state": "open", "head": {"sha": SHA, "ref": REF}, "base": {"ref": "master", "sha": BASE_SHA}},
            f"/repos/{REPO}/git/trees/{SHA}": tree(),
            f"/repos/{REPO}/contents/.github/workflows/test-and-build.yml": {"encoding": "base64", "content": base64.b64encode(WORKFLOW).decode()},
            f"/repos/{REPO}/actions/workflows/test-and-build.yml/runs": {"workflow_runs": [run]},
            f"/repos/{REPO}/actions/runs/{RUN}/jobs": {"jobs": [{"name": "Test and Build (ubuntu-latest, linux64)", "status": "completed", "conclusion": "success", "steps": [{"name": "Run shared release quality gate", "status": "completed", "conclusion": "success"}]}]},
            f"/repos/{REPO}/actions/artifacts": {"artifacts": [{"id": 5, "name": "ci-evidence-linux-release-quality-42", "expired": False, "archive_download_url": "https://artifact.test/5", "workflow_run": {"id": 50}}]},
            f"/repos/{REPO}/actions/runs/50": {"id": 50, "event": "workflow_run", "path": ".github/workflows/ci-evidence-publisher.yml", "status": "completed", "conclusion": "success"},
            f"/repos/{REPO}/commits/{SHA}/check-runs": {"check_runs": [{"id": 6, "name": CHECK, "head_sha": SHA, "status": "completed", "conclusion": "success", "app": {"id": APP_ID, "slug": "github-actions"}, "output": {"title": CHECK_TITLE, "text": f"source-run: {RUN}\nartifact: ci-evidence-linux-release-quality-{RUN}"}, "details_url": f"https://github.com/{REPO}/actions/runs/50"}]},
        }
        self.artifact = zip_manifest(manifest)

    def get(self, path: str) -> object:
        parsed = urlparse(path)
        base, query = parsed.path, parse_qs(parsed.query)
        if base.endswith("/runs"):
            self.assert_page(query, runs=True)
        elif base.endswith("/artifacts"):
            self.assert_equal(query, {"name": ["ci-evidence-linux-release-quality-42"], "per_page": ["100"], "page": ["1"]})
        elif base.endswith("/check-runs"):
            self.assert_equal(query, {"check_name": [CHECK], "per_page": ["100"], "page": ["1"]})
        elif base.endswith("/jobs"):
            self.assert_page(query)
        elif base.endswith("/trees/" + SHA):
            self.assert_equal(query, {"recursive": ["1"]})
        elif base.endswith("test-and-build.yml"):
            self.assert_equal(query, {"ref": [SHA]})
        return copy.deepcopy(self.values[base])

    @staticmethod
    def assert_equal(value: object, expected: object) -> None:
        if value != expected:
            raise AssertionError(f"unexpected query {value!r}")

    def assert_page(self, query: dict[str, list[str]], runs: bool = False) -> None:
        expected = {"per_page": ["100"], "page": ["1"]}
        if runs:
            expected = {"event": ["pull_request"], "head_sha": [SHA], **expected}
        self.assert_equal(query, expected)

    def download(self, url: str) -> bytes:
        if url != "https://artifact.test/5":
            raise AssertionError(url)
        return self.artifact


class ConsumeTests(unittest.TestCase):
    def verify(self, api: Api | None = None) -> dict[str, object]:
        api = Api() if api is None else api
        return verify_reusable_evidence(api.get, api.download, repository=REPO, pr_number=PR, head_sha=SHA, head_ref=REF, profile=PROFILE)

    def test_accepts_unique_current_pr_head_tree_profile_and_actions_evidence(self) -> None:
        result = self.verify()
        self.assertEqual(result["decision"], "reuse")
        self.assertEqual(result["source_run_id"], RUN)
        self.assertEqual(result["artifact_id"], 5)
        self.assertEqual(result["check_id"], 6)

    def test_unavailable_evidence_falls_back_without_accepting_it(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
        with self.assertRaises(ReuseUnavailable):
            self.verify(api)
        api = Api()
        api.values[f"/repos/{REPO}/commits/{SHA}/check-runs"]["check_runs"][0]["app"]["id"] = 1
        with self.assertRaises(ReuseUnavailable):
            self.verify(api)

    def test_base_change_invalidates_source_evidence(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/pulls/{PR}"]["base"]["sha"] = "d" * 40
        with self.assertRaises(ReuseUnavailable):
            self.verify(api)

    def test_in_progress_publisher_run_is_retried(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/actions/runs/50"]["conclusion"] = None
        with self.assertRaises(ReusePending):
            self.verify(api)

    def test_pending_source_run_statuses_are_retried(self) -> None:
        for status in ("in_progress", "queued", "requested", "waiting", "pending"):
            with self.subTest(status=status):
                api = Api()
                source = api.values[f"/repos/{REPO}/actions/workflows/test-and-build.yml/runs"]["workflow_runs"][0]
                source["status"] = status
                source["conclusion"] = None
                with self.assertRaises(ReusePending):
                    self.verify(api)

    def test_unpublished_artifact_waits_only_for_its_running_publisher(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
        publisher = api.values[f"/repos/{REPO}/actions/runs/50"]
        publisher["status"] = "in_progress"
        publisher["conclusion"] = None
        with self.assertRaises(ReusePending):
            self.verify(api)

    def test_unpublished_artifact_waits_before_publisher_creates_a_check(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
        api.values[f"/repos/{REPO}/commits/{SHA}/check-runs"]["check_runs"] = []
        with self.assertRaises(ReusePending):
            self.verify(api)

    def test_unpublished_artifact_falls_back_when_publisher_failed_or_check_is_invalid(self) -> None:
        for state in ("failed", "duplicate", "invalid"):
            with self.subTest(state=state):
                api = Api()
                api.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
                if state == "failed":
                    publisher = api.values[f"/repos/{REPO}/actions/runs/50"]
                    publisher["status"] = "completed"
                    publisher["conclusion"] = "failure"
                elif state == "duplicate":
                    checks = api.values[f"/repos/{REPO}/commits/{SHA}/check-runs"]["check_runs"]
                    checks.append(copy.deepcopy(checks[0]))
                else:
                    check = api.values[f"/repos/{REPO}/commits/{SHA}/check-runs"]["check_runs"][0]
                    check["app"]["id"] = 1
                with self.assertRaises(ReuseUnavailable):
                    self.verify(api)

    def test_pending_and_permanent_mismatch_have_distinct_cli_outcomes(self) -> None:
        pending_source = Api()
        source = pending_source.values[f"/repos/{REPO}/actions/workflows/test-and-build.yml/runs"]["workflow_runs"][0]
        source["status"] = "in_progress"
        source["conclusion"] = None
        pending_artifact = Api()
        pending_artifact.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
        pending_artifact.values[f"/repos/{REPO}/actions/runs/50"]["status"] = "queued"
        pending_artifact.values[f"/repos/{REPO}/actions/runs/50"]["conclusion"] = None
        pending_publisher = Api()
        pending_publisher.values[f"/repos/{REPO}/actions/runs/50"]["conclusion"] = None
        pending_startup = Api()
        pending_startup.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"] = []
        pending_startup.values[f"/repos/{REPO}/commits/{SHA}/check-runs"]["check_runs"] = []
        unavailable = Api()
        unavailable.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"].append(copy.deepcopy(unavailable.values[f"/repos/{REPO}/actions/artifacts"]["artifacts"][0]))

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "evidence.json"
            arguments = ["--repository", REPO, "--pr-number", str(PR), "--head-sha", SHA, "--head-ref", REF, "--profile", PROFILE, "--output", str(output)]
            for pending in (pending_source, pending_artifact, pending_publisher, pending_startup):
                with self.subTest(pending=pending):
                    with patch("consume.clients", return_value=(pending.get, pending.download)):
                        self.assertEqual(main(arguments), 11)
                    self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["decision"], "pending")
            with patch("consume.clients", return_value=(unavailable.get, unavailable.download)):
                self.assertEqual(main(arguments), 10)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["decision"], "rerun")

    def test_malformed_tree_or_manifest_is_a_hard_failure(self) -> None:
        api = Api()
        api.values[f"/repos/{REPO}/git/trees/{SHA}"]["truncated"] = True
        with self.assertRaises(EvidenceError):
            self.verify(api)
        api = Api()
        api.artifact = b"not a zip"
        with self.assertRaises(EvidenceError):
            self.verify(api)


if __name__ == "__main__":
    unittest.main()
