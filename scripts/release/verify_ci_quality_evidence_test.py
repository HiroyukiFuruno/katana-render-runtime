from __future__ import annotations

import argparse
import copy
import http.client
import importlib.util
import io
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("verify_ci_quality_evidence.py")
SPEC = importlib.util.spec_from_file_location("verify_ci_quality_evidence", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


REPOSITORY = "owner/repository"
BASE = "b" * 40
HEAD = "a" * 40
HEAD_REF = "release/v0.4.22"
RUN_ID = 123


WORKFLOW_RUNS_PATH = (
    "repos/owner/repository/actions/workflows/test-and-build.yml/"
    f"runs?event=pull_request&head_sha={HEAD}&per_page=100&page=1"
)

def payloads() -> dict[str, object]:
    pull = {
        "number": 90,
        "state": "open",
        "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
        "head": {"sha": HEAD, "ref": HEAD_REF, "repo": {"full_name": REPOSITORY}},
    }
    steps = [
        {"name": name, "status": "completed", "conclusion": "success"}
        for name in MODULE.REQUIRED_STEPS
    ]
    run = {
        "id": RUN_ID,
        "run_attempt": 1,
        "event": "pull_request",
        "path": MODULE.WORKFLOW_PATH,
        "head_sha": HEAD,
        "status": "completed",
        "conclusion": "success",
        "repository": {"full_name": REPOSITORY},
        "pull_requests": [{"number": 90, "base": {"sha": BASE}, "head": {"sha": HEAD}}],
    }
    job = {
        "id": 456,
        "run_id": RUN_ID,
        "name": MODULE.JOB_NAME,
        "status": "completed",
        "conclusion": "success",
        "steps": steps,
    }
    return {
        "repos/owner/repository/pulls/90": pull,
        WORKFLOW_RUNS_PATH: {
            "total_count": 1,
            "workflow_runs": [run],
        },
        "repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1": {
            "total_count": 1,
            "jobs": [job],
        },
    }


class SequenceFetcher:
    def __init__(self, responses: dict[str, object]) -> None:
        self.responses = responses
        self.calls: list[str] = []

    def __call__(self, path: str) -> object:
        self.calls.append(path)
        value = self.responses[path]
        if isinstance(value, list):
            if not value:
                raise AssertionError(f"no response left for {path}")
            return copy.deepcopy(value.pop(0))
        return copy.deepcopy(value)


class VerifyCiQualityEvidenceTest(unittest.TestCase):
    def verify(self, responses: dict[str, object] | None = None) -> dict[str, str]:
        fetcher = SequenceFetcher(responses or payloads())
        result = MODULE.verify_quality_evidence(
            fetcher,
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        self.assertEqual(fetcher.calls.count("repos/owner/repository/pulls/90"), 4)
        return result

    def test_queries_only_current_head_runs_without_historical_pages(self) -> None:
        responses = payloads()
        fetcher = SequenceFetcher(responses)
        result = MODULE.verify_quality_evidence(
            fetcher,
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        self.assertEqual(result["run_id"], str(RUN_ID))
        self.assertEqual(fetcher.calls.count(WORKFLOW_RUNS_PATH), 2)
        self.assertFalse(any("actions/workflows" in path and "page=2" in path for path in fetcher.calls))

    def test_accepts_exact_same_head_quality_evidence(self) -> None:
        self.assertEqual(self.verify(), {"run_id": "123", "run_attempt": "1", "head_sha": HEAD})

    def test_rereads_pull_identity_after_ci_job_verification(self) -> None:
        fetcher = SequenceFetcher(payloads())
        MODULE.verify_quality_evidence(
            fetcher,
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        pull_path = "repos/owner/repository/pulls/90"
        self.assertEqual(fetcher.calls[-2:], [pull_path, pull_path])
        self.assertLess(
            fetcher.calls.index("repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"),
            len(fetcher.calls) - 2,
        )

    def test_rejects_changed_second_read(self) -> None:
        responses = payloads()
        original = responses["repos/owner/repository/pulls/90"]
        changed = copy.deepcopy(original)
        changed["head"]["sha"] = "c" * 40  # type: ignore[index]
        responses["repos/owner/repository/pulls/90"] = [original, changed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head SHA does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_with_same_head_sha(self) -> None:
        responses = payloads()
        pull = responses["repos/owner/repository/pulls/90"]
        renamed = copy.deepcopy(pull)
        renamed["head"]["ref"] = "release/v0.4.23"  # type: ignore[index]
        responses["repos/owner/repository/pulls/90"] = [renamed, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_between_pull_snapshot_reads(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        first = responses[path]
        renamed = copy.deepcopy(first)
        renamed["head"]["ref"] = "release/v0.4.23"  # type: ignore[index]
        responses[path] = [first, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = responses[path]
        renamed = copy.deepcopy(initial)
        renamed["head"]["ref"] = "release/v0.4.23"  # type: ignore[index]
        responses[path] = [initial, initial, renamed, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository(self) -> None:
        responses = payloads()
        pull = responses["repos/owner/repository/pulls/90"]
        pull["head"]["repo"]["full_name"] = "fork/repository"  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository_between_pull_snapshot_reads(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = responses[path]
        fork = copy.deepcopy(initial)
        fork["head"]["repo"]["full_name"] = "fork/repository"  # type: ignore[index]
        responses[path] = [initial, fork]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = responses[path]
        fork = copy.deepcopy(initial)
        fork["head"]["repo"]["full_name"] = "fork/repository"  # type: ignore[index]
        responses[path] = [initial, initial, fork, fork]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_replaced_pull_request_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = responses[path]
        replaced = copy.deepcopy(initial)
        replaced["number"] = 91  # type: ignore[index]
        responses[path] = [initial, initial, replaced, replaced]
        with self.assertRaisesRegex(MODULE.EvidenceError, "identity/state changed"):
            self.verify(responses)

    def test_marks_benign_pull_snapshot_decoration_change_as_retryable(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        first = responses[path]
        second = copy.deepcopy(first)
        second["mergeable_state"] = "unknown"  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "pull request snapshot changed"):
            self.verify(responses)

    def test_marks_ci_run_status_change_during_second_read_as_retryable(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = responses[path]
        second = copy.deepcopy(first)
        second["workflow_runs"][0]["status"] = "in_progress"  # type: ignore[index]
        second["workflow_runs"][0]["conclusion"] = None  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_marks_new_current_head_run_during_second_read_as_retryable(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = responses[path]
        second = copy.deepcopy(first)
        new_run = copy.deepcopy(second["workflow_runs"][0])  # type: ignore[index]
        new_run["id"] = RUN_ID + 1
        new_run["status"] = "in_progress"
        new_run["conclusion"] = None
        second["total_count"] = 2  # type: ignore[index]
        second["workflow_runs"].append(new_run)  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_rejects_changed_ci_run_binding_during_second_read(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = responses[path]
        second = copy.deepcopy(first)
        second["workflow_runs"][0]["pull_requests"][0]["base"]["sha"] = "c" * 40  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed") as error:
            self.verify(responses)
        self.assertIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_incomplete_ci_run_snapshot_during_second_read(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = responses[path]
        second = copy.deepcopy(first)
        second["total_count"] = 2  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_duplicate_current_head_run_ids(self) -> None:
        responses = payloads()
        runs = responses[
            WORKFLOW_RUNS_PATH
        ]
        runs["total_count"] = 2  # type: ignore[index]
        runs["workflow_runs"].append(copy.deepcopy(runs["workflow_runs"][0]))  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidenceError, "duplicated run ids"):
            self.verify(responses)

    def test_accepts_latest_successful_current_head_run_after_reopen(self) -> None:
        responses = payloads()
        runs = responses[
            WORKFLOW_RUNS_PATH
        ]
        reopened = copy.deepcopy(runs["workflow_runs"][0])  # type: ignore[index]
        reopened["id"] = 124
        reopened["run_attempt"] = 1
        runs["total_count"] = 2  # type: ignore[index]
        runs["workflow_runs"].append(reopened)  # type: ignore[index]
        job = copy.deepcopy(responses["repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"])
        job["jobs"][0]["run_id"] = 124  # type: ignore[index]
        responses["repos/owner/repository/actions/runs/124/jobs?per_page=100&page=1"] = job
        self.assertEqual(self.verify(responses), {"run_id": "124", "run_attempt": "1", "head_sha": HEAD})

    def test_newest_run_generation_cannot_be_masked_by_older_success(self) -> None:
        responses = payloads()
        runs = responses[WORKFLOW_RUNS_PATH]
        pending = copy.deepcopy(runs["workflow_runs"][0])  # type: ignore[index]
        pending["id"] = RUN_ID + 1
        pending["status"] = "in_progress"
        pending["conclusion"] = None
        runs["total_count"] = 2  # type: ignore[index]
        runs["workflow_runs"].append(pending)  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "still in progress"):
            self.verify(responses)

    def test_rejects_malformed_binding_among_current_head_runs(self) -> None:
        responses = payloads()
        runs = responses[
            WORKFLOW_RUNS_PATH
        ]
        malformed = copy.deepcopy(runs["workflow_runs"][0])  # type: ignore[index]
        malformed["id"] = 124
        malformed["pull_requests"] = []
        runs["total_count"] = 2  # type: ignore[index]
        runs["workflow_runs"].append(malformed)  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidenceError, "PR binding is missing or duplicated"):
            self.verify(responses)

    def test_marks_pending_current_head_run_as_retryable(self) -> None:
        responses = payloads()
        run = responses[
            WORKFLOW_RUNS_PATH
        ]["workflow_runs"][0]  # type: ignore[index]
        run["status"] = "in_progress"
        run["conclusion"] = None
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "still in progress"):
            self.verify(responses)

    def test_marks_absent_current_head_run_as_retryable(self) -> None:
        responses = payloads()
        responses[WORKFLOW_RUNS_PATH] = {"total_count": 0, "workflow_runs": []}
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "expected at least one current-head CI run"):
            self.verify(responses)

    def test_marks_ci_job_status_change_during_second_read_as_retryable(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"
        first = responses[path]
        second = copy.deepcopy(first)
        second["jobs"][0]["status"] = "in_progress"  # type: ignore[index]
        second["jobs"][0]["conclusion"] = None  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_rejects_changed_ci_job_binding_during_second_read(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"
        first = responses[path]
        second = copy.deepcopy(first)
        second["jobs"][0]["run_id"] = RUN_ID + 1  # type: ignore[index]
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidenceError, "bound to another workflow run") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_completed_failed_current_head_run_without_retry(self) -> None:
        responses = payloads()
        run = responses[
            WORKFLOW_RUNS_PATH
        ]["workflow_runs"][0]  # type: ignore[index]
        run["conclusion"] = "failure"
        with self.assertRaisesRegex(MODULE.EvidenceError, "completed unsuccessfully") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_main_returns_retryable_exit_only_for_pending_evidence(self) -> None:
        responses = payloads()
        run = responses[
            WORKFLOW_RUNS_PATH
        ]["workflow_runs"][0]  # type: ignore[index]
        run["status"] = "in_progress"
        run["conclusion"] = None
        args = argparse.Namespace(
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
            workflow=MODULE.WORKFLOW_PATH,
        )
        with (
            mock.patch.object(MODULE, "_api_fetch", SequenceFetcher(responses)),
            mock.patch.object(MODULE, "parse_args", return_value=args),
        ):
            self.assertEqual(MODULE.main(), MODULE.PENDING_EVIDENCE_EXIT)

    def test_marks_interrupted_api_read_as_retryable(self) -> None:
        interrupted = http.client.IncompleteRead(b'{"partial":', 20)
        with mock.patch.object(MODULE.request, "urlopen", side_effect=interrupted):
            with self.assertRaisesRegex(MODULE.EvidencePendingError, "transport was interrupted"):
                MODULE._api_fetch("repos/owner/repository/pulls/90")

    def test_marks_retryable_url_transport_error_as_retryable(self) -> None:
        interrupted = MODULE.error.URLError(ConnectionResetError(104, "connection reset"))
        with mock.patch.object(MODULE.request, "urlopen", side_effect=interrupted):
            with self.assertRaisesRegex(MODULE.EvidencePendingError, "temporarily unavailable"):
                MODULE._api_fetch("repos/owner/repository/pulls/90")

    def test_rejects_non_retryable_api_response_without_retry(self) -> None:
        rejected = MODULE.error.HTTPError(
            "https://api.github.com/repos/owner/repository/pulls/90",
            401,
            "Unauthorized",
            {},
            io.BytesIO(),
        )
        try:
            with mock.patch.object(MODULE.request, "urlopen", side_effect=rejected):
                with self.assertRaisesRegex(MODULE.EvidenceError, "HTTP status 401") as error:
                    MODULE._api_fetch("repos/owner/repository/pulls/90")
        finally:
            rejected.close()
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_marks_retryable_api_response_as_retryable(self) -> None:
        unavailable = MODULE.error.HTTPError(
            "https://api.github.com/repos/owner/repository/pulls/90",
            503,
            "Unavailable",
            {},
            io.BytesIO(),
        )
        try:
            with mock.patch.object(MODULE.request, "urlopen", side_effect=unavailable):
                with self.assertRaisesRegex(MODULE.EvidencePendingError, "temporarily unavailable"):
                    MODULE._api_fetch("repos/owner/repository/pulls/90")
        finally:
            unavailable.close()

    def test_rejects_malformed_api_json_without_retry(self) -> None:
        response = mock.MagicMock()
        response.read.return_value = b"{"
        response.__enter__.return_value = response
        with mock.patch.object(MODULE.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(MODULE.EvidenceError, "malformed JSON") as error:
                MODULE._api_fetch("repos/owner/repository/pulls/90")
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_duplicate_required_step(self) -> None:
        responses = payloads()
        job = responses["repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"]["jobs"][0]  # type: ignore[index]
        job["steps"].append(copy.deepcopy(job["steps"][0]))  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidenceError, "missing or duplicated"):
            self.verify(responses)

    def test_rejects_base_head_mismatch(self) -> None:
        responses = payloads()
        run = responses[
            WORKFLOW_RUNS_PATH
        ]["workflow_runs"][0]  # type: ignore[index]
        run["pull_requests"][0]["base"]["sha"] = "d" * 40  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "current-base"):
            self.verify(responses)

    def test_rejects_paginated_or_incomplete_response(self) -> None:
        responses = payloads()
        responses[
            WORKFLOW_RUNS_PATH
        ]["total_count"] = 2  # type: ignore[index]
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated"):
            self.verify(responses)

    def test_rejects_short_page_even_when_later_page_compensates(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        run = responses[first_path]["workflow_runs"][0]  # type: ignore[index]
        responses[first_path] = {"total_count": 101, "workflow_runs": [copy.deepcopy(run)]}
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 101,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100)],
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated"):
            self.verify(responses)

    def test_rejects_historical_runs_returned_by_head_filter(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        responses[first_path] = {
            "total_count": 101,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100)],
        }
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 102,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100, 102)],
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "head filter returned"):
            self.verify(responses)

    def test_main_rejects_historical_runs_returned_by_head_filter(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        responses[first_path] = {
            "total_count": 101,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100)],
        }
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 102,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100, 102)],
        }
        args = argparse.Namespace(
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
            workflow=MODULE.WORKFLOW_PATH,
        )
        with (
            mock.patch.object(MODULE, "_api_fetch", SequenceFetcher(responses)),
            mock.patch.object(MODULE, "parse_args", return_value=args),
        ):
            self.assertEqual(MODULE.main(), 1)

    def test_rejects_historical_run_before_later_malformed_run(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        malformed = copy.deepcopy(payloads()[first_path]["workflow_runs"][0])  # type: ignore[index]
        malformed["pull_requests"] = []
        responses[first_path] = {
            "total_count": 101,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100)],
        }
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 102,
            "workflow_runs": [malformed, {"id": 1000, "head_sha": "c" * 40}],
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "head filter returned") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_unfiltered_historical_page_before_current_head_run(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        historical = {"id": 999, "head_sha": "c" * 40}
        responses[first_path] = {
            "total_count": 101,
            "workflow_runs": [copy.deepcopy(historical) for _ in range(100)],
        }
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 101,
            "workflow_runs": [payloads()[first_path]["workflow_runs"][0]],  # type: ignore[index]
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "head filter returned"):
            self.verify(responses)

    def test_rejects_duplicate_current_head_runs_across_pages(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        run = responses[first_path]["workflow_runs"][0]  # type: ignore[index]
        responses[first_path] = {
            "total_count": 101,
            "workflow_runs": [copy.deepcopy(run) for _ in range(100)],
        }
        responses[f"{first_path[:-1]}2"] = {
            "total_count": 101,
            "workflow_runs": [copy.deepcopy(run)],
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "duplicated run ids"):
            self.verify(responses)


class WorkflowAndJustfileContractTest(unittest.TestCase):
    ROOT = MODULE_PATH.parents[2]

    def test_ubuntu_ci_owns_the_quality_steps_and_bounded_coverage(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        for step in MODULE.REQUIRED_STEPS:
            self.assertIn(f"- name: {step}", workflow)
        coverage = workflow[workflow.index("- name: Run coverage") :]
        self.assertIn("timeout-minutes: 45", coverage)
        self.assertIn("- name: Diagnose coverage failure", coverage)

    def test_macos_arm64_rebuilds_v8_before_linking(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        start = workflow.index("- name: Rebuild V8 static archive on macOS arm64")
        end = workflow.index("\n      - name:", start + 1)
        v8_setup = workflow[start:end]
        self.assertIn("if: startsWith(matrix.platform, 'mac-')", v8_setup)
        self.assertIn("cargo clean -p v8", v8_setup)
        self.assertIn("cargo build -p katana-render-runtime-cli --locked", v8_setup)
        self.assertLess(start, workflow.index("- name: Check Rust types"))

    def test_windows_rebuilds_v8_before_linking(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        start = workflow.index("- name: Rebuild V8 static archive on Windows")
        end = workflow.index("\n      - name:", start + 1)
        v8_setup = workflow[start:end]
        self.assertIn("if: matrix.platform == 'win64'", v8_setup)
        self.assertIn("shell: pwsh", v8_setup)
        self.assertIn("cargo clean -p v8", v8_setup)
        self.assertIn("cargo build -p katana-render-runtime-cli --locked", v8_setup)
        self.assertLess(start, workflow.index("- name: Check Rust types"))

    def test_release_preflight_uses_evidence_then_release_only_target(self) -> None:
        workflow = (self.ROOT / ".github/workflows/release-preflight.yml").read_text(encoding="utf-8")
        self.assertIn("verify_ci_quality_evidence.py", workflow)
        evidence_job = workflow[workflow.index("  evidence:\n") : workflow.index("  preflight:\n")]
        preflight_job = workflow[workflow.index("  preflight:\n") :]
        self.assertIn("timeout-minutes: 360", evidence_job)
        self.assertIn("for attempt in {1..990}", evidence_job)
        self.assertIn("else\n              ci_evidence_status=$?", workflow)
        self.assertIn('if [ "$ci_evidence_status" -ne 75 ]; then', workflow)
        self.assertIn('if [ "$attempt" -eq 990 ]; then', workflow)
        self.assertIn("sleep 20", workflow)
        self.assertIn("after 330 minutes", evidence_job)
        self.assertIn("needs: evidence", preflight_job)
        self.assertIn("timeout-minutes: 360", preflight_job)
        self.assertIn("release-preflight-check", preflight_job)
        self.assertIn('github.event_name == \'workflow_dispatch\'', preflight_job)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", evidence_job)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", preflight_job)
        self.assertIn('startsWith(github.head_ref, \'release/v\')', evidence_job)

    def test_local_release_check_keeps_full_quality_gate(self) -> None:
        justfile = (self.ROOT / "Justfile").read_text(encoding="utf-8")
        self.assertIn("release-check: release-quality release-specific", justfile)
        self.assertIn("release-preflight-check: release-openspec-archive release-verify", justfile)


if __name__ == "__main__":
    unittest.main()
