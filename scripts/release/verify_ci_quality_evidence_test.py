from __future__ import annotations

import argparse
import copy
import http.client
import importlib.util
import io
import json
from functools import partial
import unittest
import zipfile
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
ARTIFACTS_PATH = "repos/owner/repository/actions/runs/123/artifacts?per_page=100&page=1"
ARTIFACT_URL = "https://api.github.com/artifacts/789"


def object_mapping(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise AssertionError("fixture value must be a string-keyed object")
    return value


def object_sequence(value: object) -> list[object]:
    if not isinstance(value, list):
        raise AssertionError("fixture value must be an array")
    return value


def mapping_field(value: object, key: str) -> dict[str, object]:
    return object_mapping(object_mapping(value)[key])


def sequence_field(value: object, key: str) -> list[object]:
    return object_sequence(object_mapping(value)[key])


def mapping_item(value: object, index: int) -> dict[str, object]:
    return object_mapping(object_sequence(value)[index])


def integer_field(value: object, key: str) -> int:
    item = object_mapping(value)[key]
    if not isinstance(item, int) or isinstance(item, bool):
        raise AssertionError(f"fixture field {key} must be an integer")
    return item


def string_mapping(value: object) -> dict[str, str]:
    mapping = object_mapping(value)
    result: dict[str, str] = {}
    for key, item in mapping.items():
        if not isinstance(item, str):
            raise AssertionError("fixture result must contain only string values")
        result[key] = item
    return result


def fixture_download(responses: dict[str, object], url: str) -> bytes:
    body = responses[url]
    if not isinstance(body, bytes):
        raise AssertionError(f"fixture download is not bytes: {url}")
    return body


def run_imported_main() -> int:
    result: object = MODULE.main()
    if not isinstance(result, int) or isinstance(result, bool):
        raise AssertionError("fixture main result must be an integer exit status")
    return result


def historical_run_pages() -> tuple[dict[str, object], dict[str, object]]:
    return (
        {
            "total_count": 101,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100)],
        },
        {
            "total_count": 102,
            "workflow_runs": [{"id": index, "head_sha": "c" * 40} for index in range(100, 102)],
        },
    )


def immutable_base_evidence_archive(
    *, base_sha: str = BASE, head_sha: str = HEAD, run_id: int = RUN_ID, run_attempt: int = 1
) -> bytes:
    payload = json.dumps(
        {"schema": 2, "run_id": run_id, "run_attempt": run_attempt, "base_sha": base_sha, "head_sha": head_sha,
         "html_fidelity": MODULE.HTML_POLICY.select_html_policy("0.4.22", HEAD_REF)},
        sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as bundle:
        bundle.writestr(MODULE.IMMUTABLE_BASE_EVIDENCE_FILE, payload)
    return output.getvalue()

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
        "run_started_at": "2026-10-01T12:00:00Z",
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
        ARTIFACTS_PATH: {
            "total_count": 1,
            "artifacts": [{
                "id": 789,
                "name": "ci-quality-evidence-123-1",
                "expired": False,
                "size_in_bytes": len(immutable_base_evidence_archive()),
                "archive_download_url": ARTIFACT_URL,
            }],
        },
        ARTIFACT_URL: immutable_base_evidence_archive(),
        "repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1": {
            "total_count": 1,
            "jobs": [job],
        },
    }


class HtmlPhaseArtifactTests(unittest.TestCase):
    def test_accepts_current_generation_authorized_interim_phase(self) -> None:
        responses = payloads()
        ref = "release/v0.4.24"
        mapping_field(mapping_field(responses, "repos/owner/repository/pulls/90"), "head")["ref"] = ref
        with zipfile.ZipFile(io.BytesIO(fixture_download(responses, ARTIFACT_URL))) as bundle:
            evidence = json.loads(bundle.read(MODULE.IMMUTABLE_BASE_EVIDENCE_FILE))
        evidence["html_fidelity"] = MODULE.HTML_POLICY.select_html_policy("0.4.24", ref)
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as bundle:
            bundle.writestr(MODULE.IMMUTABLE_BASE_EVIDENCE_FILE, json.dumps(evidence))
        responses[ARTIFACT_URL] = output.getvalue()
        mapping_item(sequence_field(responses[ARTIFACTS_PATH], "artifacts"), 0)["size_in_bytes"] = len(output.getvalue())
        result = MODULE.verify_quality_evidence(SequenceFetcher(responses), partial(fixture_download, responses), repository=REPOSITORY,
            pull_request=90, base_sha=BASE, head_sha=HEAD, head_ref=ref)
        self.assertEqual(result["head_sha"], HEAD)

    def test_current_generation_rejects_wrong_phase_version_threshold_and_source(self) -> None:
        for key, value in (("phase", "interim"), ("version", "0.4.24"), ("threshold", 90), ("source_sha256", "0" * 64), ("policy_sha256", "0" * 64)):
            responses = payloads()
            with zipfile.ZipFile(io.BytesIO(fixture_download(responses, ARTIFACT_URL))) as bundle:
                evidence = json.loads(bundle.read(MODULE.IMMUTABLE_BASE_EVIDENCE_FILE))
            evidence["html_fidelity"][key] = value
            output = io.BytesIO()
            with zipfile.ZipFile(output, "w") as bundle:
                bundle.writestr(MODULE.IMMUTABLE_BASE_EVIDENCE_FILE, json.dumps(evidence))
            responses[ARTIFACT_URL] = output.getvalue()
            mapping_item(sequence_field(responses[ARTIFACTS_PATH], "artifacts"), 0)["size_in_bytes"] = len(output.getvalue())
            with self.assertRaisesRegex(MODULE.EvidenceError, "immutable base evidence"):
                MODULE.verify_quality_evidence(SequenceFetcher(responses), partial(fixture_download, responses), repository=REPOSITORY,
                    pull_request=90, base_sha=BASE, head_sha=HEAD, head_ref=HEAD_REF)


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


def run_main_with_fixture_responses(
    responses: dict[str, object], args: argparse.Namespace, *, stub_html_source: bool = False
) -> int:
    with (
        mock.patch.object(MODULE, "_api_fetch", SequenceFetcher(responses)),
        mock.patch.object(MODULE, "_api_download", SequenceFetcher(responses)),
        mock.patch.object(MODULE, "parse_args", return_value=args),
    ):
        if stub_html_source:
            with (
                mock.patch.object(MODULE.HTML_POLICY, "assert_head_source_binding"),
                mock.patch.object(MODULE.HTML_POLICY, "workspace_version", return_value="0.4.22"),
            ):
                return run_imported_main()
        return run_imported_main()


class VerifyCiQualityEvidenceTest(unittest.TestCase):
    def verify(self, responses: dict[str, object] | None = None) -> dict[str, str]:
        data = responses or payloads()
        fetcher = SequenceFetcher(data)
        result = MODULE.verify_quality_evidence(
            fetcher,
            SequenceFetcher(data),
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        self.assertEqual(fetcher.calls.count("repos/owner/repository/pulls/90"), 4)
        return string_mapping(result)

    def test_queries_only_current_head_runs_without_historical_pages(self) -> None:
        responses = payloads()
        fetcher = SequenceFetcher(responses)
        result = MODULE.verify_quality_evidence(
            fetcher,
            SequenceFetcher(responses),
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        self.assertEqual(result["run_id"], str(RUN_ID))
        self.assertEqual(fetcher.calls.count(WORKFLOW_RUNS_PATH), 4)
        self.assertFalse(any("actions/workflows" in path and "page=2" in path for path in fetcher.calls))

    def test_accepts_exact_same_head_quality_evidence(self) -> None:
        self.assertEqual(self.verify(), {"run_id": "123", "run_attempt": "1", "head_sha": HEAD})

    def test_rejects_stable_duplicate_named_artifact(self) -> None:
        responses = payloads()
        metadata = object_mapping(responses[ARTIFACTS_PATH])
        artifacts = sequence_field(metadata, "artifacts")
        duplicate = copy.deepcopy(mapping_item(artifacts, 0))
        duplicate["id"] = integer_field(duplicate, "id") + 1
        artifacts.append(duplicate)
        metadata["total_count"] = 2
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "absent or ambiguous"):
            self.verify(responses)

    def test_rejects_stable_incomplete_artifact_page(self) -> None:
        responses = payloads()
        object_mapping(responses[ARTIFACTS_PATH])["total_count"] = 2
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated"):
            self.verify(responses)

    def test_rejects_stable_expired_artifact(self) -> None:
        responses = payloads()
        mapping_item(sequence_field(responses[ARTIFACTS_PATH], "artifacts"), 0)["expired"] = True
        with self.assertRaisesRegex(MODULE.EvidenceError, "metadata is invalid"):
            self.verify(responses)

    def test_rejects_stable_noninteger_artifact_id(self) -> None:
        responses = payloads()
        mapping_item(sequence_field(responses[ARTIFACTS_PATH], "artifacts"), 0)["id"] = True
        with self.assertRaisesRegex(MODULE.EvidenceError, "metadata is invalid"):
            self.verify(responses)

    def test_rereads_pull_identity_after_ci_job_verification(self) -> None:
        responses = payloads()
        fetcher = SequenceFetcher(responses)
        MODULE.verify_quality_evidence(
            fetcher,
            SequenceFetcher(responses),
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
        )
        pull_path = "repos/owner/repository/pulls/90"
        self.assertEqual(fetcher.calls[-4:-2], [pull_path, pull_path])
        self.assertLess(
            fetcher.calls.index("repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"),
            len(fetcher.calls) - 2,
        )

    def test_final_fence_rejects_new_generation_after_job_verification(self) -> None:
        for rerun in (False, True):
            for status, conclusion, error_type, message in (
                ("in_progress", None, MODULE.EvidencePendingError, "still in progress"),
                ("completed", "failure", MODULE.EvidenceError, "completed unsuccessfully"),
                ("completed", "success", MODULE.EvidencePendingError, "generation changed"),
            ):
                with self.subTest(rerun=rerun, status=status, conclusion=conclusion):
                    responses = payloads()
                    initial = object_mapping(responses[WORKFLOW_RUNS_PATH])
                    final = copy.deepcopy(initial)
                    workflow_runs = sequence_field(final, "workflow_runs")
                    newer = copy.deepcopy(mapping_item(workflow_runs, 0))
                    newer.update(status=status, conclusion=conclusion)
                    newer["run_started_at"] = "2026-10-01T12:01:00Z"
                    if rerun:
                        newer["run_attempt"] = integer_field(newer, "run_attempt") + 1
                        final["workflow_runs"] = [newer]
                    else:
                        newer["id"] = integer_field(newer, "id") + 1
                        workflow_runs.append(newer)
                        final["total_count"] = integer_field(final, "total_count") + 1
                    responses[WORKFLOW_RUNS_PATH] = [initial, initial, final, final]
                    with self.assertRaisesRegex(error_type, message):
                        self.verify(responses)

    def test_final_fence_reads_generation_after_artifact_and_jobs(self) -> None:
        responses = payloads()
        fetcher = SequenceFetcher(responses)
        MODULE.verify_quality_evidence(
            fetcher, fetcher, repository=REPOSITORY, pull_request=90,
            base_sha=BASE, head_sha=HEAD, head_ref=HEAD_REF,
        )
        self.assertEqual(fetcher.calls[-2:], [WORKFLOW_RUNS_PATH, WORKFLOW_RUNS_PATH])
        self.assertLess(fetcher.calls.index(ARTIFACT_URL), len(fetcher.calls) - 4)
        self.assertLess(
            max(index for index, path in enumerate(fetcher.calls) if "/jobs?" in path),
            len(fetcher.calls) - 4,
        )

    def test_rejects_changed_second_read(self) -> None:
        responses = payloads()
        original = object_mapping(responses["repos/owner/repository/pulls/90"])
        changed = copy.deepcopy(original)
        mapping_field(changed, "head")["sha"] = "c" * 40
        responses["repos/owner/repository/pulls/90"] = [original, changed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head SHA does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_with_same_head_sha(self) -> None:
        responses = payloads()
        pull = object_mapping(responses["repos/owner/repository/pulls/90"])
        renamed = copy.deepcopy(pull)
        mapping_field(renamed, "head")["ref"] = "release/v0.4.23"
        responses["repos/owner/repository/pulls/90"] = [renamed, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_between_pull_snapshot_reads(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        first = object_mapping(responses[path])
        renamed = copy.deepcopy(first)
        mapping_field(renamed, "head")["ref"] = "release/v0.4.23"
        responses[path] = [first, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_release_branch_rename_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = object_mapping(responses[path])
        renamed = copy.deepcopy(initial)
        mapping_field(renamed, "head")["ref"] = "release/v0.4.23"
        responses[path] = [initial, initial, renamed, renamed]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head ref does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository(self) -> None:
        responses = payloads()
        pull = object_mapping(responses["repos/owner/repository/pulls/90"])
        mapping_field(mapping_field(pull, "head"), "repo")["full_name"] = "fork/repository"
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository_between_pull_snapshot_reads(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = object_mapping(responses[path])
        fork = copy.deepcopy(initial)
        mapping_field(mapping_field(fork, "head"), "repo")["full_name"] = "fork/repository"
        responses[path] = [initial, fork]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_fork_head_repository_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = object_mapping(responses[path])
        fork = copy.deepcopy(initial)
        mapping_field(mapping_field(fork, "head"), "repo")["full_name"] = "fork/repository"
        responses[path] = [initial, initial, fork, fork]
        with self.assertRaisesRegex(MODULE.EvidenceError, "head repository does not match"):
            self.verify(responses)

    def test_rejects_replaced_pull_request_after_ci_job_verification(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        initial = object_mapping(responses[path])
        replaced = copy.deepcopy(initial)
        replaced["number"] = 91
        responses[path] = [initial, initial, replaced, replaced]
        with self.assertRaisesRegex(MODULE.EvidenceError, "identity/state changed"):
            self.verify(responses)

    def test_marks_benign_pull_snapshot_decoration_change_as_retryable(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/pulls/90"
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        second["mergeable_state"] = "unknown"
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "pull request snapshot changed"):
            self.verify(responses)

    def test_marks_ci_run_status_change_during_second_read_as_retryable(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        run = mapping_item(sequence_field(second, "workflow_runs"), 0)
        run["status"] = "in_progress"
        run["conclusion"] = None
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_marks_new_current_head_run_during_second_read_as_retryable(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        runs = sequence_field(second, "workflow_runs")
        new_run = copy.deepcopy(mapping_item(runs, 0))
        new_run["id"] = RUN_ID + 1
        new_run["status"] = "in_progress"
        new_run["conclusion"] = None
        second["total_count"] = 2
        runs.append(new_run)
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_rejects_changed_ci_run_binding_during_second_read(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        run = mapping_item(sequence_field(second, "workflow_runs"), 0)
        pull = mapping_item(sequence_field(run, "pull_requests"), 0)
        mapping_field(pull, "base")["sha"] = "c" * 40
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed") as error:
            self.verify(responses)
        self.assertIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_incomplete_ci_run_snapshot_during_second_read(self) -> None:
        responses = payloads()
        path = WORKFLOW_RUNS_PATH
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        second["total_count"] = 2
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_duplicate_current_head_run_ids(self) -> None:
        responses = payloads()
        runs = object_mapping(responses[WORKFLOW_RUNS_PATH])
        workflow_runs = sequence_field(runs, "workflow_runs")
        runs["total_count"] = 2
        workflow_runs.append(copy.deepcopy(mapping_item(workflow_runs, 0)))
        with self.assertRaisesRegex(MODULE.EvidenceError, "duplicated run ids"):
            self.verify(responses)

    def test_accepts_latest_successful_current_head_run_after_reopen(self) -> None:
        responses = payloads()
        runs = object_mapping(responses[WORKFLOW_RUNS_PATH])
        workflow_runs = sequence_field(runs, "workflow_runs")
        reopened = copy.deepcopy(mapping_item(workflow_runs, 0))
        reopened["id"] = 124
        reopened["run_attempt"] = 1
        reopened["run_started_at"] = "2026-10-01T12:01:00Z"
        runs["total_count"] = 2
        workflow_runs.append(reopened)
        job = copy.deepcopy(object_mapping(responses["repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"]))
        mapping_item(sequence_field(job, "jobs"), 0)["run_id"] = 124
        responses["repos/owner/repository/actions/runs/124/jobs?per_page=100&page=1"] = job
        artifact_path = "repos/owner/repository/actions/runs/124/artifacts?per_page=100&page=1"
        artifact_url = "https://api.github.com/artifacts/790"
        archive = immutable_base_evidence_archive(run_id=124)
        responses[artifact_path] = {
            "total_count": 1,
            "artifacts": [{
                "id": 790, "name": "ci-quality-evidence-124-1", "expired": False,
                "size_in_bytes": len(archive), "archive_download_url": artifact_url,
            }],
        }
        responses[artifact_url] = archive
        self.assertEqual(self.verify(responses), {"run_id": "124", "run_attempt": "1", "head_sha": HEAD})

    def test_newest_run_generation_cannot_be_masked_by_older_success(self) -> None:
        responses = payloads()
        runs = object_mapping(responses[WORKFLOW_RUNS_PATH])
        workflow_runs = sequence_field(runs, "workflow_runs")
        pending = copy.deepcopy(mapping_item(workflow_runs, 0))
        pending["id"] = RUN_ID + 1
        pending["run_started_at"] = "2026-10-01T12:01:00Z"
        pending["status"] = "in_progress"
        pending["conclusion"] = None
        runs["total_count"] = 2
        workflow_runs.append(pending)
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "still in progress"):
            self.verify(responses)

    def test_older_run_rerun_supersedes_newer_run_success(self) -> None:
        for status, conclusion, error_type in (
            ("in_progress", None, MODULE.EvidencePendingError),
            ("completed", "failure", MODULE.EvidenceError),
            ("completed", "success", None),
        ):
            with self.subTest(status=status, conclusion=conclusion):
                successful = mapping_item(sequence_field(payloads()[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
                rerun = copy.deepcopy(successful)
                rerun.update(
                    id=RUN_ID - 1, run_attempt=2,
                    run_started_at="2026-10-01T12:01:00Z",
                    status=status, conclusion=conclusion,
                )
                args = ([rerun, successful], REPOSITORY, 90, BASE, HEAD, MODULE.WORKFLOW_PATH)
                if error_type is None:
                    self.assertEqual(MODULE._verify_runs(*args), rerun)
                else:
                    with self.assertRaises(error_type):
                        MODULE._verify_runs(*args)

    def test_run_chronology_rejects_invalid_or_missing_start_time(self) -> None:
        for started_at in (None, 1, "", "2026-02-30T12:00:00Z", "2026-10-01T12:00:00+00:00"):
            with self.subTest(started_at=started_at):
                run = mapping_item(sequence_field(payloads()[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
                run["run_started_at"] = started_at
                with self.assertRaisesRegex(MODULE.EvidenceError, "start time is invalid"):
                    MODULE._verify_runs([run], REPOSITORY, 90, BASE, HEAD, MODULE.WORKFLOW_PATH)

    def test_same_second_generations_do_not_select_a_successful_run(self) -> None:
        successful = mapping_item(sequence_field(payloads()[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
        rerun = copy.deepcopy(successful)
        rerun.update(id=RUN_ID - 1, run_attempt=2, status="in_progress", conclusion=None)
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "start times are ambiguous"):
            MODULE._verify_runs([successful, rerun], REPOSITORY, 90, BASE, HEAD, MODULE.WORKFLOW_PATH)

    def test_rejects_malformed_binding_among_current_head_runs(self) -> None:
        responses = payloads()
        runs = object_mapping(responses[WORKFLOW_RUNS_PATH])
        workflow_runs = sequence_field(runs, "workflow_runs")
        malformed = copy.deepcopy(mapping_item(workflow_runs, 0))
        malformed["id"] = 124
        malformed["pull_requests"] = []
        runs["total_count"] = 2
        workflow_runs.append(malformed)
        with self.assertRaisesRegex(MODULE.EvidenceError, "PR binding is missing or duplicated"):
            self.verify(responses)

    def test_marks_pending_current_head_run_as_retryable(self) -> None:
        responses = payloads()
        run = mapping_item(sequence_field(responses[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
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
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        job = mapping_item(sequence_field(second, "jobs"), 0)
        job["status"] = "in_progress"
        job["conclusion"] = None
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "CI evidence changed"):
            self.verify(responses)

    def test_rejects_changed_ci_job_binding_during_second_read(self) -> None:
        responses = payloads()
        path = "repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"
        first = object_mapping(responses[path])
        second = copy.deepcopy(first)
        mapping_item(sequence_field(second, "jobs"), 0)["run_id"] = RUN_ID + 1
        responses[path] = [first, second]
        with self.assertRaisesRegex(MODULE.EvidenceError, "bound to another workflow run") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_rejects_completed_failed_current_head_run_without_retry(self) -> None:
        responses = payloads()
        run = mapping_item(sequence_field(responses[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
        run["conclusion"] = "failure"
        with self.assertRaisesRegex(MODULE.EvidenceError, "completed unsuccessfully") as error:
            self.verify(responses)
        self.assertNotIsInstance(error.exception, MODULE.EvidencePendingError)

    def test_main_returns_retryable_exit_only_for_pending_evidence(self) -> None:
        responses = payloads()
        run = mapping_item(sequence_field(responses[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
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
        self.assertEqual(
            run_main_with_fixture_responses(responses, args, stub_html_source=True),
            MODULE.PENDING_EVIDENCE_EXIT,
        )

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
        job = mapping_item(
            sequence_field(responses["repos/owner/repository/actions/runs/123/jobs?per_page=100&page=1"], "jobs"),
            0,
        )
        steps = sequence_field(job, "steps")
        steps.append(copy.deepcopy(steps[0]))
        with self.assertRaisesRegex(MODULE.EvidenceError, "missing or duplicated"):
            self.verify(responses)

    def test_rejects_base_head_mismatch(self) -> None:
        responses = payloads()
        run = mapping_item(sequence_field(responses[WORKFLOW_RUNS_PATH], "workflow_runs"), 0)
        pull = mapping_item(sequence_field(run, "pull_requests"), 0)
        mapping_field(pull, "base")["sha"] = "d" * 40
        with self.assertRaisesRegex(MODULE.EvidencePendingError, "current-base"):
            self.verify(responses)

    def test_rejects_mutated_run_relation_without_matching_immutable_base_evidence(self) -> None:
        responses = payloads()
        artifact = mapping_item(sequence_field(responses[ARTIFACTS_PATH], "artifacts"), 0)
        responses[ARTIFACT_URL] = immutable_base_evidence_archive(base_sha="c" * 40)
        artifact["size_in_bytes"] = len(fixture_download(responses, ARTIFACT_URL))
        # GitHub may rewrite this relation after master advances.  The artifact
        # was emitted by the CI generation and remains the binding authority.
        with self.assertRaisesRegex(MODULE.EvidenceError, "immutable base evidence does not match"):
            self.verify(responses)

    def test_rejects_paginated_or_incomplete_response(self) -> None:
        responses = payloads()
        object_mapping(responses[WORKFLOW_RUNS_PATH])["total_count"] = 2
        with self.assertRaisesRegex(MODULE.EvidenceError, "incomplete or paginated"):
            self.verify(responses)

    def test_rejects_short_page_even_when_later_page_compensates(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        run = mapping_item(sequence_field(responses[first_path], "workflow_runs"), 0)
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
        responses[first_path], responses[f"{first_path[:-1]}2"] = historical_run_pages()
        with self.assertRaisesRegex(MODULE.EvidenceError, "head filter returned"):
            self.verify(responses)

    def test_main_rejects_historical_runs_returned_by_head_filter(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        responses[first_path], responses[f"{first_path[:-1]}2"] = historical_run_pages()
        args = argparse.Namespace(
            repository=REPOSITORY,
            pull_request=90,
            base_sha=BASE,
            head_sha=HEAD,
            head_ref=HEAD_REF,
            workflow=MODULE.WORKFLOW_PATH,
        )
        self.assertEqual(run_main_with_fixture_responses(responses, args), 1)

    def test_rejects_historical_run_before_later_malformed_run(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        malformed = copy.deepcopy(mapping_item(sequence_field(payloads()[first_path], "workflow_runs"), 0))
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
            "workflow_runs": [mapping_item(sequence_field(payloads()[first_path], "workflow_runs"), 0)],
        }
        with self.assertRaisesRegex(MODULE.EvidenceError, "head filter returned"):
            self.verify(responses)

    def test_rejects_duplicate_current_head_runs_across_pages(self) -> None:
        responses = payloads()
        first_path = WORKFLOW_RUNS_PATH
        run = mapping_item(sequence_field(responses[first_path], "workflow_runs"), 0)
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

    @staticmethod
    def workflow_step(workflow: str, name: str) -> str:
        start = workflow.index(f"      - name: {name}\n")
        end = workflow.find("\n      - ", start + 1)
        return workflow[start:] if end == -1 else workflow[start:end]

    def assert_v8_rebuild_precedes_link_steps(
        self,
        workflow: str,
        setup_name: str,
        platform_condition: str,
        smoke_name: str,
        *,
        windows: bool = False,
    ) -> None:
        setup_start = workflow.index(f"- name: {setup_name}")
        setup = self.workflow_step(workflow, setup_name)
        self.assertIn(f"if: {platform_condition}", setup)
        if windows:
            self.assertIn("shell: pwsh", setup)
        self.assertIn("cargo clean -p v8", setup)
        self.assertIn("cargo build -p katana-render-runtime-cli --locked", setup)

        type_check = workflow.index("- name: Check Rust types")
        run_tests = workflow.index("- name: Run tests")
        smoke_check = workflow.index(f"- name: {smoke_name}")
        self.assertLess(type_check, setup_start)
        self.assertLess(setup_start, run_tests)
        self.assertLess(setup_start, smoke_check)

    def test_html_performance_requires_fresh_all84_capture_manifest(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        justfile = (self.ROOT / "Justfile").read_text(encoding="utf-8")
        capture_step = self.workflow_step(workflow, "Capture all 84 HTML catalog conditions")
        measure_step = self.workflow_step(workflow, "Measure all 84 catalog conditions against baseline")

        self.assertIn(
            'output_root="tmp/html-performance-stage90/${{ github.run_id }}-${{ github.run_attempt }}"',
            capture_step,
        )
        self.assertIn(
            'manifest="$output_root/captures/catalog-stage90-captures.json"', capture_step
        )
        self.assertIn("run_id", capture_step)
        self.assertIn("run_attempt", capture_step)
        self.assertIn("tmp/html-performance-stage90/", capture_step)
        self.assertIn('just html-performance-catalog-captures "$output_root"', capture_step)
        self.assertGreater(
            capture_step.index("just html-performance-catalog-captures"),
            capture_step.index("output_root="),
        )
        self.assertGreater(
            capture_step.index('echo "manifest=$manifest"'),
            capture_step.index("just html-performance-catalog-captures"),
        )
        self.assertIn('just html-performance-evidence-produce "${{ steps.html84.outputs.manifest }}"', measure_step)
        self.assertLess(
            workflow.index("Initialize fixed capacity reference before interaction Cargo"),
            workflow.index("Verify real interaction state for measurement fixture"),
        )
        self.assertLess(
            workflow.index("Verify real interaction state for measurement fixture"),
            workflow.index("Capture all 84 HTML catalog conditions"),
        )
        self.assertLess(
            workflow.index("Capture all 84 HTML catalog conditions"),
            workflow.index("Measure all 84 catalog conditions against baseline"),
        )
        self.assertIn("html-performance-catalog-captures output_root:", justfile)
        self.assertIn("catalog-stage90-plan", justfile)
        self.assertIn("--require-capacity --print-json", justfile)
        self.assertIn(
            'HTML84_CARGO := if RTK == "" { "cargo" } else { RTK + " proxy cargo" }',
            justfile,
        )
        self.assertIn("python3 scripts/build/cargo_capacity_guard.py --root", justfile)
        guarded_build = (
            '--state {{quote(REPO_ROOT + "/tmp/.cargo-capacity-guard.json")}} -- '
            '{{HTML84_CARGO}} test -p katana-render-runtime --lib --no-run'
        )
        self.assertIn(guarded_build, justfile)
        self.assertNotIn(
            "{{CARGO}} test -p katana-render-runtime --lib --no-run", justfile
        )
        self.assertIn("--no-run --locked --message-format=json", justfile)
        self.assertIn("catalog-stage90-capture", justfile)
        self.assertIn("refusing symlinked HTML stage90 output parent", justfile)
        self.assertIn("refusing existing HTML stage90 output root", justfile)
        self.assertIn("html-performance-evidence-produce catalog_manifest:", justfile)
        self.assertIn('--catalog-plan "$manifest"', justfile)
        self.assertNotIn("html-performance-evidence-produce:\n", justfile)

    def test_ubuntu_ci_owns_the_quality_steps_and_bounded_coverage(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        for step in MODULE.REQUIRED_STEPS:
            self.assertIn(f"- name: {step}", workflow)
        coverage = workflow[workflow.index("- name: Run coverage") :]
        self.assertIn("timeout-minutes: 45", coverage)
        self.assertIn("- name: Diagnose coverage failure", coverage)
        self.assertIn("- name: Record immutable PR base evidence", workflow)
        self.assertIn("- name: Upload immutable PR base evidence", workflow)
        self.assertIn("ci-quality-evidence-${{ github.run_id }}-${{ github.run_attempt }}", workflow)
        self.assertIn("github.event.pull_request.base.sha", workflow)
        expected_artifacts = {
            "Upload independent HTML comparison evidence": (
                "name: html-chrome-fidelity",
                "path: tmp/html-chrome-fidelity-suite",
                "if-no-files-found: error",
            ),
            "Upload interim HTML comparison evidence": (
                "name: html-chrome-fidelity-interim-v0.4.24",
                "tmp/html-fidelity/interim",
                "tmp/html-fidelity/dynamic-interim",
                "tmp/html-fidelity/input-state-interim",
                "if-no-files-found: error",
            ),
            "Upload HTML interaction evidence": (
                "name: html-chrome-interactions",
                "path: tmp/html-interaction-suite",
                "if-no-files-found: error",
            ),
            "Upload immutable PR base evidence": (
                "name: ci-quality-evidence-${{ github.run_id }}-${{ github.run_attempt }}",
                "path: ${{ runner.temp }}/ci-quality-evidence.json",
                "retention-days: 7",
                "if-no-files-found: error",
            ),
            "Upload immutable source and run bound paired evidence": (
                "name: html-performance-paired-${{ steps.source.outputs.head }}-${{ github.run_id }}-${{ github.run_attempt }}",
                "path: tmp/html-performance-producer/bundle",
                "retention-days: 14",
                "if-no-files-found: error",
            ),
            "Upload failed measurement diagnostics": (
                "name: html-performance-diagnostics-${{ steps.source.outputs.head }}-${{ github.run_id }}-${{ github.run_attempt }}",
                "tmp/html-performance-producer/*/reports/*.json",
                "if: failure()",
                "if-no-files-found: warn",
            ),
        }
        upload_pin = "uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
        for name, required_attributes in expected_artifacts.items():
            block = self.workflow_step(workflow, name)
            with self.subTest(artifact=name):
                self.assertIn(upload_pin, block)
                for attribute in required_attributes:
                    self.assertIn(attribute, block)

    def test_macos_arm64_rebuilds_v8_before_linking(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        self.assert_v8_rebuild_precedes_link_steps(
            workflow,
            "Rebuild V8 static archive on macOS arm64",
            "startsWith(matrix.platform, 'mac-')",
            "Run PlantUML smoke check",
        )

    def test_windows_rebuilds_v8_before_linking(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        self.assert_v8_rebuild_precedes_link_steps(
            workflow,
            "Rebuild V8 static archive on Windows",
            "matrix.platform == 'win64'",
            "Run PlantUML smoke check on Windows",
            windows=True,
        )

    def test_v8_rebuild_order_contract_rejects_linking_before_archive_repair(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        setup_name = "Rebuild V8 static archive on macOS arm64"
        setup_start = workflow.index(f"      - name: {setup_name}\n")
        setup_end = workflow.index("\n      - name:", setup_start + 1)
        setup = workflow[setup_start:setup_end + 1]
        mutated = workflow[:setup_start] + workflow[setup_end + 1:]
        run_tests_end = mutated.index("\n      - name:", mutated.index("      - name: Run tests\n"))
        mutated = mutated[:run_tests_end + 1] + setup + mutated[run_tests_end + 1:]
        with self.assertRaises(AssertionError):
            self.assert_v8_rebuild_precedes_link_steps(
                mutated,
                setup_name,
                "startsWith(matrix.platform, 'mac-')",
                "Run PlantUML smoke check",
            )

    def test_release_preflight_uses_evidence_then_release_only_target(self) -> None:
        ci_workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        ci_preflight = ci_workflow[ci_workflow.index("  preflight:\n") :]
        self.assertIn("needs: [test, paired-performance]", ci_preflight)
        self.assertIn("needs.test.result == 'success'", ci_preflight)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", ci_preflight)
        self.assertIn("startsWith(github.head_ref, 'release/v')", ci_preflight)
        self.assertIn("verify_html_performance_evidence.py --acquire-current-producer", ci_preflight)
        self.assertIn("release-preflight-check", ci_preflight)

        workflow = (self.ROOT / ".github/workflows/release-preflight.yml").read_text(encoding="utf-8")
        self.assertIn("  workflow_dispatch:", workflow)
        self.assertNotIn("  pull_request:", workflow)
        self.assertIn("if: github.event_name == 'workflow_dispatch'", workflow)
        self.assertIn("just html-performance-evidence-acquire", workflow)
        self.assertIn("release-check", workflow)
        self.assertNotIn("release-preflight-check", workflow)

    def test_ci_cancels_older_runs_for_the_same_pull_request(self) -> None:
        workflow = (self.ROOT / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        self.assertIn("concurrency:\n", workflow)
        concurrency = workflow[workflow.index("concurrency:\n") : workflow.index("\njobs:")]
        group_expression = "${{ github.workflow }}-${{ github.ref }}"

        self.assertLess(workflow.index("concurrency:\n"), workflow.index("jobs:\n"))
        self.assertIn(f"group: {group_expression}", concurrency)
        self.assertIn("cancel-in-progress: true", concurrency)
        pull_request_trigger = workflow[workflow.index("  pull_request:\n") : workflow.index("  workflow_dispatch:")]
        for event_type in ("synchronize", "ready_for_review", "converted_to_draft"):
            self.assertIn(event_type, pull_request_trigger)

    def test_local_release_check_keeps_full_quality_gate(self) -> None:
        justfile = (self.ROOT / "Justfile").read_text(encoding="utf-8")
        self.assertIn("release-check: release-quality release-specific", justfile)
        self.assertIn("release-preflight-check: release-openspec-archive release-verify", justfile)


if __name__ == "__main__":
    unittest.main()
