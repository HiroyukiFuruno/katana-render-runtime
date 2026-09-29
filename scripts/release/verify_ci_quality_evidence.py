#!/usr/bin/env python3
"""Verify reusable Ubuntu CI quality evidence for a release pull request."""

from __future__ import annotations

import argparse
import errno
import http.client
import io
import json
import os
import re
import socket
import subprocess
import sys
import zipfile
from collections.abc import Callable
from typing import Any
from urllib import error, parse, request


class EvidenceError(RuntimeError):
    """Raised when CI evidence is absent, stale, ambiguous, or malformed."""


class EvidencePendingError(EvidenceError):
    """Raised when CI evidence is valid but the workflow has not completed."""


JsonValue = Any
Fetcher = Callable[[str], JsonValue]
ArtifactFetcher = Callable[[str], bytes]
SnapshotValidator = Callable[[JsonValue], None]
PENDING_EVIDENCE_EXIT = 75
RETRYABLE_HTTP_STATUSES = frozenset({408, 429, 500, 502, 503, 504})
RETRYABLE_SOCKET_ERRNOS = frozenset(
    {
        errno.ECONNABORTED,
        errno.ECONNREFUSED,
        errno.ECONNRESET,
        errno.EHOSTUNREACH,
        errno.ENETDOWN,
        errno.ENETUNREACH,
        errno.ETIMEDOUT,
    }
)

REPOSITORY_RE = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z")
SHA_RE = re.compile(r"[0-9a-fA-F]{40}\Z")
RELEASE_HEAD_REF_RE = re.compile(r"release/v\d+\.\d+\.\d+\Z")
WORKFLOW_PATH = ".github/workflows/test-and-build.yml"
IMMUTABLE_BASE_EVIDENCE_FILE = "ci-quality-evidence.json"
MAX_IMMUTABLE_BASE_EVIDENCE_ARCHIVE_BYTES = 64 * 1024
MAX_IMMUTABLE_BASE_EVIDENCE_PAYLOAD_BYTES = 1024
JOB_NAME = "Test and Build (ubuntu-latest, linux64)"
REQUIRED_STEPS = (
    "Run runtime asset script tests",
    "Run automation contract tests",
    "Run runtime package asset check",
    "Run coverage",
)


class _ArtifactRedirectHandler(request.HTTPRedirectHandler):
    """Prevent the GitHub API bearer token from crossing to artifact storage."""

    def redirect_request(
        self, request_value: request.Request, file_pointer: Any, status_code: int, reason: str, headers: Any, url: str
    ) -> request.Request | None:
        redirected = super().redirect_request(
            request_value, file_pointer, status_code, reason, headers, url
        )
        if redirected is not None and parse.urlsplit(request_value.full_url).netloc != parse.urlsplit(url).netloc:
            redirected.remove_header("Authorization")
        return redirected


def _canonical(value: JsonValue) -> str:
    try:
        return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    except (TypeError, ValueError) as exc:
        raise EvidenceError("GitHub API response is not canonical JSON") from exc


def _validate_inputs(
    repository: str, pull_request: int, base_sha: str, head_sha: str, head_ref: str
) -> None:
    if REPOSITORY_RE.fullmatch(repository) is None:
        raise EvidenceError(f"invalid repository: {repository!r}")
    if type(pull_request) is not int or pull_request < 1:
        raise EvidenceError("pull request number must be positive")
    if SHA_RE.fullmatch(base_sha) is None or SHA_RE.fullmatch(head_sha) is None:
        raise EvidenceError("base/head must be 40-character hexadecimal SHAs")
    if RELEASE_HEAD_REF_RE.fullmatch(head_ref) is None:
        raise EvidenceError("head ref must be a stable release branch")


def _read_pull_snapshot_twice(
    fetch: Fetcher,
    path: str,
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    head_ref: str,
) -> dict[str, JsonValue]:
    """Return a stable snapshot bound to the expected release branch and commit."""

    first = _require_dict(fetch(path), "pull request")
    second = _require_dict(fetch(path), "pull request")
    _verify_pull_request(first, repository, pull_request, base_sha, head_sha, head_ref)
    _verify_pull_request(second, repository, pull_request, base_sha, head_sha, head_ref)
    if _canonical(first) != _canonical(second):
        raise EvidencePendingError(f"pull request snapshot changed during verification: {path}")
    return first


def _read_ci_snapshot_twice(fetch: Fetcher, path: str, validate: SnapshotValidator) -> JsonValue:
    """Read a mutable CI endpoint twice without accepting an integrity change."""
    first = fetch(path)
    second = fetch(path)
    if _canonical(first) == _canonical(second):
        return first
    validate(first)
    validate(second)
    raise EvidencePendingError(f"CI evidence changed during verification: {path}")


def _require_dict(value: JsonValue, label: str) -> dict[str, JsonValue]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} is not an object")
    return value


def _require_sha(value: JsonValue, label: str) -> str:
    if not isinstance(value, str) or SHA_RE.fullmatch(value) is None:
        raise EvidenceError(f"{label} is not a valid SHA")
    return value


def _require_completed_success(value: dict[str, JsonValue], label: str) -> None:
    if value.get("status") != "completed":
        raise EvidencePendingError(f"{label} is still in progress")
    if value.get("conclusion") != "success":
        raise EvidenceError(f"{label} completed unsuccessfully")


def _verify_pull_request(
    pull: dict[str, JsonValue],
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    head_ref: str,
) -> None:
    if pull.get("number") != pull_request or pull.get("state") != "open":
        raise EvidenceError("release pull request identity/state changed")
    base = _require_dict(pull.get("base"), "pull request base")
    head = _require_dict(pull.get("head"), "pull request head")
    base_repo = _require_dict(base.get("repo"), "pull request base repository")
    head_repo = _require_dict(head.get("repo"), "pull request head repository")
    if (
        base.get("sha") != base_sha
        or base_repo.get("full_name") != repository
        or base.get("ref") != "master"
    ):
        raise EvidenceError("release pull request base identity does not match")
    if head.get("sha") != head_sha:
        raise EvidenceError("release pull request head SHA does not match")
    if head.get("ref") != head_ref:
        raise EvidenceError("release pull request head ref does not match")
    if head_repo.get("full_name") != repository:
        raise EvidenceError("release pull request head repository does not match")


def _pull_request_identity(pull: dict[str, JsonValue]) -> tuple[JsonValue, ...]:
    """Return the immutable release-candidate identity from a validated PR snapshot."""
    base = _require_dict(pull.get("base"), "pull request base")
    head = _require_dict(pull.get("head"), "pull request head")
    base_repo = _require_dict(base.get("repo"), "pull request base repository")
    head_repo = _require_dict(head.get("repo"), "pull request head repository")
    return (
        pull.get("number"),
        base.get("sha"),
        base.get("ref"),
        base_repo.get("full_name"),
        head.get("sha"),
        head.get("ref"),
        head_repo.get("full_name"),
    )


def _workflow_runs_path(repository: str, workflow: str, head_sha: str, page: int) -> str:
    workflow_query = parse.quote(workflow.rsplit("/", 1)[-1], safe="")
    head_query = parse.quote(head_sha, safe="")
    return (
        f"repos/{repository}/actions/workflows/{workflow_query}/runs"
        f"?event=pull_request&head_sha={head_query}&per_page=100&page={page}"
    )


def _validate_workflow_run_snapshot(
    payload: JsonValue,
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    workflow: str,
    page: int,
) -> None:
    snapshot = _require_dict(payload, "workflow runs")
    total = snapshot.get("total_count")
    runs = snapshot.get("workflow_runs")
    expected_count = min(100, max(total - (page - 1) * 100, 0)) if type(total) is int else -1
    if (
        type(total) is not int
        or total < 0
        or not isinstance(runs, list)
        or len(runs) != expected_count
    ):
        raise EvidenceError("workflow run snapshot is incomplete or paginated")
    if any(not isinstance(run, dict) or run.get("head_sha") != head_sha for run in runs):
        raise EvidenceError("workflow head filter returned a non-current-head run")
    run_ids = [run.get("id") for run in runs if isinstance(run, dict)]
    if len(set(run_ids)) != len(run_ids):
        raise EvidenceError("current-head CI runs have duplicated run ids")


def _read_all_workflow_runs(
    fetch: Fetcher,
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    workflow: str,
) -> list[JsonValue]:
    first_path = _workflow_runs_path(repository, workflow, head_sha, 1)
    first_payload = _require_dict(
        _read_ci_snapshot_twice(
            fetch,
            first_path,
            lambda payload: _validate_workflow_run_snapshot(
                payload, repository, pull_request, base_sha, head_sha, workflow, 1
            ),
        ),
        "workflow runs",
    )
    _validate_workflow_run_snapshot(
        first_payload, repository, pull_request, base_sha, head_sha, workflow, 1
    )
    total = first_payload.get("total_count")
    first_runs = first_payload.get("workflow_runs")
    if (
        type(total) is not int
        or total < 0
        or not isinstance(first_runs, list)
        or len(first_runs) != min(total, 100)
    ):
        raise EvidenceError("workflow run page is incomplete or paginated")

    pages = (total + 99) // 100
    all_runs = list(first_runs)
    for page in range(2, pages + 1):
        path = _workflow_runs_path(repository, workflow, head_sha, page)
        payload = _require_dict(
            _read_ci_snapshot_twice(
                fetch,
                path,
                lambda snapshot: _validate_workflow_run_snapshot(
                    snapshot, repository, pull_request, base_sha, head_sha, workflow, page
                ),
            ),
            "workflow runs",
        )
        _validate_workflow_run_snapshot(
            payload, repository, pull_request, base_sha, head_sha, workflow, page
        )
        page_total = payload.get("total_count")
        page_runs = payload.get("workflow_runs")
        expected_count = min(100, total - (page - 1) * 100)
        if type(page_total) is not int or not isinstance(page_runs, list):
            raise EvidenceError("workflow run page is incomplete or paginated")
        if page_total != total:
            # workflow runs はページを跨ぐ間にも追加・削除される。各ページの
            # 個別スナップショットを検証済みなら、全体の世代差は再試行対象である。
            raise EvidencePendingError("workflow run snapshot changed during pagination")
        if len(page_runs) != expected_count:
            raise EvidenceError("workflow run page is incomplete or paginated")
        all_runs.extend(page_runs)

    if len(all_runs) != total:
        raise EvidenceError("workflow run page is incomplete or paginated")
    return all_runs


def _artifacts_path(repository: str, run_id: int) -> str:
    return f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100&page=1"


def _immutable_base_evidence_name(run_id: int, run_attempt: int) -> str:
    return f"ci-quality-evidence-{run_id}-{run_attempt}"


def _verify_immutable_base_evidence(
    fetch: Fetcher,
    artifact_fetch: ArtifactFetcher,
    repository: str,
    run: dict[str, JsonValue],
    base_sha: str,
    head_sha: str,
) -> None:
    """Require CI-produced, run-bound base evidence instead of mutable PR relations."""
    run_id = run.get("id")
    run_attempt = run.get("run_attempt")
    if type(run_id) is not int or run_id < 1 or type(run_attempt) is not int or run_attempt < 1:
        raise EvidenceError("CI workflow run generation is invalid")
    expected_name = _immutable_base_evidence_name(run_id, run_attempt)
    path = _artifacts_path(repository, run_id)

    def validate(payload: JsonValue) -> None:
        value = _require_dict(payload, "CI immutable base evidence")
        total = value.get("total_count")
        artifacts = value.get("artifacts")
        if type(total) is not int or total < 0 or not isinstance(artifacts, list) or total != len(artifacts):
            raise EvidenceError("CI immutable base evidence page is incomplete or paginated")
        matches = [artifact for artifact in artifacts if isinstance(artifact, dict) and artifact.get("name") == expected_name]
        if len(matches) != 1:
            raise EvidencePendingError("CI immutable base evidence is absent or ambiguous")
        artifact = matches[0]
        if (
            type(artifact.get("id")) is not int
            or artifact["id"] < 1
            or artifact.get("expired") is not False
            or type(artifact.get("size_in_bytes")) is not int
            or artifact["size_in_bytes"] < 1
            or artifact["size_in_bytes"] > MAX_IMMUTABLE_BASE_EVIDENCE_ARCHIVE_BYTES
            or not isinstance(artifact.get("archive_download_url"), str)
            or not artifact["archive_download_url"]
        ):
            raise EvidenceError("CI immutable base evidence metadata is invalid")

    payload = _require_dict(_read_ci_snapshot_twice(fetch, path, validate), "CI immutable base evidence")
    artifacts = payload.get("artifacts")
    assert isinstance(artifacts, list)
    artifact = next(item for item in artifacts if isinstance(item, dict) and item.get("name") == expected_name)
    archive_url = artifact.get("archive_download_url")
    assert isinstance(archive_url, str)
    archive = artifact_fetch(archive_url)
    if not isinstance(archive, bytes) or not (0 < len(archive) <= MAX_IMMUTABLE_BASE_EVIDENCE_ARCHIVE_BYTES):
        raise EvidenceError("CI immutable base evidence archive is invalid")
    try:
        with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
            members = bundle.infolist()
            if len(members) != 1:
                raise EvidenceError("CI immutable base evidence archive members are invalid")
            member = members[0]
            if (
                member.filename != IMMUTABLE_BASE_EVIDENCE_FILE
                or member.is_dir()
                or member.file_size < 1
                or member.file_size > MAX_IMMUTABLE_BASE_EVIDENCE_PAYLOAD_BYTES
            ):
                raise EvidenceError("CI immutable base evidence archive members are invalid")
            raw = bundle.read(member)
            if len(raw) != member.file_size or len(raw) > MAX_IMMUTABLE_BASE_EVIDENCE_PAYLOAD_BYTES:
                raise EvidenceError("CI immutable base evidence payload is invalid")
    except (OSError, zipfile.BadZipFile, zipfile.LargeZipFile) as exc:
        raise EvidenceError("CI immutable base evidence archive is malformed") from exc
    try:
        evidence = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise EvidenceError("CI immutable base evidence payload is malformed") from exc
    if (
        not isinstance(evidence, dict)
        or set(evidence) != {"base_sha", "head_sha", "run_attempt", "run_id", "schema"}
        or evidence.get("schema") != 1
        or evidence.get("run_id") != run_id
        or evidence.get("run_attempt") != run_attempt
        or evidence.get("base_sha") != base_sha
        or evidence.get("head_sha") != head_sha
    ):
        raise EvidenceError("CI immutable base evidence does not match the current generation")


def _verify_runs(
    runs: list[JsonValue],
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    workflow: str,
) -> dict[str, JsonValue]:
    matching = [run for run in runs if isinstance(run, dict) and run.get("head_sha") == head_sha]
    if not matching:
        # workflow_run から起動した preflight より CI run の登録が遅れることがある。
        raise EvidencePendingError("expected at least one current-head CI run")

    verified = []
    for run in matching:
        pull_requests = run.get("pull_requests")
        if isinstance(pull_requests, list) and len(pull_requests) == 1 and isinstance(pull_requests[0], dict):
            run_base = pull_requests[0].get("base")
            if isinstance(run_base, dict) and isinstance(run_base.get("sha"), str) and run_base["sha"] != base_sha:
                # 同一headでbaseだけが古い履歴runは、現在の候補証跡に使わない。
                continue
        verified.append(_verify_run_binding(run, repository, pull_request, base_sha, head_sha, workflow))
    if not verified:
        raise EvidencePendingError("expected at least one current-base CI run")

    # run id と attempt は不変の workflow 世代を表す。GitHub は PR base 更新時に
    # 古い run の埋め込み pull_requests 関係を書き換えることがあるため、古い成功
    # 世代が新しい pending / failed 世代を隠せないよう、全候補の不変な結合を検証後に
    # 最新世代だけを評価する。
    generations = [(run["id"], run["run_attempt"]) for run in verified]
    if len(set(generations)) != len(generations):
        raise EvidenceError("current-head CI runs have duplicated run generations")
    latest = max(verified, key=lambda run: (run["id"], run["run_attempt"]))
    if latest.get("status") != "completed":
        raise EvidencePendingError("all current-head CI runs are still in progress or unsuccessful")
    if latest.get("conclusion") != "success":
        raise EvidenceError("all current-head CI runs completed unsuccessfully")
    return latest


def _verify_run_binding(
    run: dict[str, JsonValue],
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    workflow: str,
) -> dict[str, JsonValue]:
    run_repository = _require_dict(run.get("repository"), "CI workflow repository")
    if (
        run.get("event") != "pull_request"
        or run.get("path") != workflow
        or run_repository.get("full_name") != repository
    ):
        raise EvidenceError("CI workflow run identity does not match")
    _require_sha(run.get("head_sha"), "CI workflow head")
    if type(run.get("id")) is not int or run["id"] < 1:
        raise EvidenceError("CI workflow run id is invalid")
    if type(run.get("run_attempt")) is not int or run["run_attempt"] < 1:
        raise EvidenceError("CI workflow run attempt is invalid")
    pull_requests = run.get("pull_requests")
    if not isinstance(pull_requests, list) or len(pull_requests) != 1:
        raise EvidenceError("CI workflow run PR binding is missing or duplicated")
    run_pull = _require_dict(pull_requests[0], "CI workflow run PR binding")
    if run_pull.get("number") != pull_request:
        raise EvidenceError("CI workflow run PR number does not match")
    run_base = _require_dict(run_pull.get("base"), "CI workflow run base")
    run_head = _require_dict(run_pull.get("head"), "CI workflow run head")
    if run_base.get("sha") != base_sha or run_head.get("sha") != head_sha:
        raise EvidenceError("CI workflow run base/head does not match")
    return run


def _verify_jobs(
    jobs_payload: dict[str, JsonValue], run_id: int, required_steps: tuple[str, ...]
) -> None:
    total = jobs_payload.get("total_count")
    jobs = jobs_payload.get("jobs")
    if type(total) is not int or total < 0 or not isinstance(jobs, list) or total != len(jobs):
        raise EvidenceError("CI job page is incomplete or paginated")
    candidates = [job for job in jobs if isinstance(job, dict) and job.get("name") == JOB_NAME]
    if len(candidates) != 1:
        raise EvidenceError("expected exactly one Ubuntu CI job")
    job = _require_dict(candidates[0], "Ubuntu CI job")
    if job.get("run_id") != run_id:
        raise EvidenceError("Ubuntu CI job is bound to another workflow run")
    _require_completed_success(job, "Ubuntu CI job")
    steps = job.get("steps")
    if not isinstance(steps, list):
        raise EvidenceError("Ubuntu CI job steps are missing")
    by_name: dict[str, list[dict[str, JsonValue]]] = {}
    for raw_step in steps:
        step = _require_dict(raw_step, "Ubuntu CI step")
        name = step.get("name")
        if isinstance(name, str):
            by_name.setdefault(name, []).append(step)
    for name in required_steps:
        matches = by_name.get(name, [])
        if len(matches) != 1:
            raise EvidenceError(f"required CI step is missing or duplicated: {name}")
        _require_completed_success(matches[0], f"CI step {name}")


def _validate_jobs_snapshot(payload: JsonValue, run_id: int, required_steps: tuple[str, ...]) -> None:
    """Validate immutable job bindings while permitting CI state to advance."""
    jobs_payload = _require_dict(payload, "workflow jobs")
    total = jobs_payload.get("total_count")
    jobs = jobs_payload.get("jobs")
    if type(total) is not int or total < 0 or not isinstance(jobs, list) or total != len(jobs):
        raise EvidenceError("CI job page is incomplete or paginated")
    candidates = [job for job in jobs if isinstance(job, dict) and job.get("name") == JOB_NAME]
    if len(candidates) != 1:
        raise EvidenceError("expected exactly one Ubuntu CI job")
    job = _require_dict(candidates[0], "Ubuntu CI job")
    if job.get("run_id") != run_id:
        raise EvidenceError("Ubuntu CI job is bound to another workflow run")
    steps = job.get("steps")
    if not isinstance(steps, list):
        raise EvidenceError("Ubuntu CI job steps are missing")
    names = [step.get("name") for step in steps if isinstance(step, dict)]
    for name in required_steps:
        if names.count(name) != 1:
            raise EvidenceError(f"required CI step is missing or duplicated: {name}")


def verify_quality_evidence(
    fetch: Fetcher,
    artifact_fetch: ArtifactFetcher,
    *,
    repository: str,
    pull_request: int,
    base_sha: str,
    head_sha: str,
    head_ref: str,
    workflow: str = WORKFLOW_PATH,
) -> dict[str, str]:
    _validate_inputs(repository, pull_request, base_sha, head_sha, head_ref)
    pull_path = f"repos/{repository}/pulls/{pull_request}"
    pull = _read_pull_snapshot_twice(
        fetch, pull_path, repository, pull_request, base_sha, head_sha, head_ref
    )
    runs = _verify_runs(
        _read_all_workflow_runs(fetch, repository, pull_request, base_sha, head_sha, workflow),
        repository,
        pull_request,
        base_sha,
        head_sha,
        workflow,
    )
    _verify_immutable_base_evidence(
        fetch, artifact_fetch, repository, runs, base_sha, head_sha
    )
    run_id = runs["id"]
    jobs_path = f"repos/{repository}/actions/runs/{run_id}/jobs?per_page=100&page=1"
    jobs = _read_ci_snapshot_twice(
        fetch,
        jobs_path,
        lambda payload: _validate_jobs_snapshot(payload, run_id, REQUIRED_STEPS),
    )
    _verify_jobs(_require_dict(jobs, "workflow jobs"), run_id, REQUIRED_STEPS)
    final_pull = _read_pull_snapshot_twice(
        fetch, pull_path, repository, pull_request, base_sha, head_sha, head_ref
    )
    if _pull_request_identity(pull) != _pull_request_identity(final_pull):
        raise EvidenceError("release pull request identity changed during CI verification")
    return {"run_id": str(run_id), "run_attempt": str(runs["run_attempt"]), "head_sha": head_sha}


def _is_retryable_url_error(exc: error.URLError) -> bool:
    reason = exc.reason
    if isinstance(reason, (socket.timeout, TimeoutError, ConnectionError)):
        return True
    return isinstance(reason, OSError) and reason.errno in RETRYABLE_SOCKET_ERRNOS


def _api_headers() -> dict[str, str]:
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "katana-render-runtime-ci-quality-evidence",
    }
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def _api_fetch(path: str) -> JsonValue:
    api_request = request.Request(f"https://api.github.com/{path}", headers=_api_headers())
    try:
        with request.urlopen(api_request, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
    except error.HTTPError as exc:
        if exc.code in RETRYABLE_HTTP_STATUSES:
            raise EvidencePendingError("GitHub API is temporarily unavailable") from exc
        raise EvidenceError(f"GitHub API request failed with HTTP status {exc.code}") from exc
    except error.URLError as exc:
        if _is_retryable_url_error(exc):
            raise EvidencePendingError("GitHub API transport is temporarily unavailable") from exc
        raise EvidenceError("GitHub API transport request failed") from exc
    except (
        http.client.IncompleteRead,
        http.client.RemoteDisconnected,
        ConnectionError,
        socket.timeout,
        TimeoutError,
    ) as exc:
        raise EvidencePendingError("GitHub API transport was interrupted") from exc
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise EvidenceError("GitHub API returned malformed JSON") from exc


def _api_download(url: str) -> bytes:
    if not isinstance(url, str) or not url.startswith("https://api.github.com/"):
        raise EvidenceError("CI immutable base evidence download URL is invalid")
    try:
        opener = request.build_opener(_ArtifactRedirectHandler())
        with opener.open(request.Request(url, headers=_api_headers()), timeout=20) as response:
            data = response.read(MAX_IMMUTABLE_BASE_EVIDENCE_ARCHIVE_BYTES + 1)
    except error.HTTPError as exc:
        if exc.code in RETRYABLE_HTTP_STATUSES:
            raise EvidencePendingError("CI immutable base evidence is temporarily unavailable") from exc
        raise EvidenceError(f"CI immutable base evidence download failed with HTTP status {exc.code}") from exc
    except error.URLError as exc:
        if _is_retryable_url_error(exc):
            raise EvidencePendingError("CI immutable base evidence transport is temporarily unavailable") from exc
        raise EvidenceError("CI immutable base evidence transport failed") from exc
    except (http.client.IncompleteRead, http.client.RemoteDisconnected, ConnectionError, socket.timeout, TimeoutError) as exc:
        raise EvidencePendingError("CI immutable base evidence transport was interrupted") from exc
    if len(data) > MAX_IMMUTABLE_BASE_EVIDENCE_ARCHIVE_BYTES:
        raise EvidenceError("CI immutable base evidence archive is too large")
    return data


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--pull-request", required=True, type=int)
    parser.add_argument("--base-sha", required=True)
    parser.add_argument("--head-sha", required=True)
    parser.add_argument(
        "--head-ref",
        default=os.environ.get("GITHUB_HEAD_REF"),
        help="Expected release branch; defaults to GitHub Actions GITHUB_HEAD_REF",
    )
    parser.add_argument("--workflow", default=WORKFLOW_PATH)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.head_ref is None:
            raise EvidenceError("expected release head ref is required")
        evidence = verify_quality_evidence(
            _api_fetch,
            _api_download,
            repository=args.repository,
            pull_request=args.pull_request,
            base_sha=args.base_sha,
            head_sha=args.head_sha,
            head_ref=args.head_ref,
            workflow=args.workflow,
        )
    except EvidencePendingError as exc:
        print(f"CI quality evidence verification is pending: {exc}", file=sys.stderr)
        return PENDING_EVIDENCE_EXIT
    except EvidenceError as exc:
        print(f"CI quality evidence verification failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({"status": "verified", **evidence}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
