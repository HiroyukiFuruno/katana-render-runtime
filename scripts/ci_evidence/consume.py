#!/usr/bin/env python3
"""Fail closed consumer for protected Linux CI evidence.

The release workflow fetches this verifier from protected master.  Missing or
nonmatching evidence returns 10 so the caller runs the complete quality gate;
malformed API data and loader/verifier failures return a hard error.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from io import BytesIO
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

try:
    from .schema import TreeSchemaError
    from .tree_digest import canonical_json_bytes, digest_input_tree
except ImportError:
    from schema import TreeSchemaError
    from tree_digest import canonical_json_bytes, digest_input_tree

APP_ID = 15368
PROFILE = "linux-release-quality-v1"
SOURCE_WORKFLOW = ".github/workflows/test-and-build.yml"
PUBLISHER_WORKFLOW = ".github/workflows/ci-evidence-publisher.yml"
JOB = "Test and Build (ubuntu-latest, linux64)"
STEP = "Run shared release quality gate"
CHECK = "KRR / CI evidence (linux release quality)"
CHECK_TITLE = "Trusted Linux release-quality evidence published"
ARTIFACT_PREFIX = "ci-evidence-linux-release-quality-"
PENDING_WORKFLOW_STATUSES = frozenset(("in_progress", "queued", "requested", "waiting", "pending"))
SHA = re.compile(r"^[0-9a-f]{40}$")
REPOSITORY = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
MAX_BYTES = 65536


class EvidenceError(RuntimeError):
    pass


class ReuseUnavailable(RuntimeError):
    pass


class ReusePending(ReuseUnavailable):
    """Trusted evidence exists but its publisher has not completed yet."""


Getter = Callable[[str], object]
Downloader = Callable[[str], bytes]


def obj(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise EvidenceError(f"{label} must be an object")
    return value


def array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise EvidenceError(f"{label} must be an array")
    return value


def text(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{label} must be a string")
    try:
        value.encode("utf-8", "strict")
    except UnicodeEncodeError as exc:
        raise EvidenceError(f"{label} is not UTF-8") from exc
    return value


def number(value: object, label: str) -> int:
    if type(value) is not int:
        raise EvidenceError(f"{label} must be an integer")
    return value


def sha(value: object, label: str) -> str:
    value = text(value, label)
    if SHA.fullmatch(value) is None:
        raise EvidenceError(f"{label} must be a lowercase 40-character SHA")
    return value


def matches(value: object, expected: object, label: str) -> None:
    if value != expected:
        raise ReuseUnavailable(f"{label} did not match")


def clients(api_url: str, repository: str, token: str) -> tuple[Getter, Downloader]:
    base = api_url.rstrip("/")
    if not base.startswith("https://") or REPOSITORY.fullmatch(repository) is None or not token:
        raise EvidenceError("invalid GitHub API configuration")

    def request(path: str, accept: str, limit: int | None = None) -> bytes:
        url = path if path.startswith("https://") else base + path
        request_value = urllib.request.Request(url, headers={"Accept": accept, "Authorization": f"Bearer {token}", "X-GitHub-Api-Version": "2022-11-28"})
        try:
            with urllib.request.urlopen(request_value, timeout=30) as response:
                if response.status != 200:
                    raise EvidenceError(f"GitHub API returned HTTP {response.status}")
                return response.read() if limit is None else response.read(limit + 1)
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as exc:
            raise EvidenceError("GitHub API request failed") from exc

    def get(path: str) -> object:
        try:
            return json.loads(request(path, "application/vnd.github+json").decode("utf-8", "strict"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise EvidenceError("GitHub API returned invalid JSON") from exc

    def download(url: str) -> bytes:
        data = request(url, "application/vnd.github+json", MAX_BYTES)
        if len(data) > MAX_BYTES:
            raise EvidenceError("evidence artifact is too large")
        return data

    return get, download


def pages(get: Getter, path: str, field: str) -> list[object]:
    result: list[object] = []
    for page in range(1, 101):
        delimiter = "&" if "?" in path else "?"
        page_data = obj(get(f"{path}{delimiter}per_page=100&page={page}"), f"{field} page")
        entries = array(page_data.get(field), field)
        result.extend(entries)
        if len(entries) < 100:
            return result
    raise EvidenceError(f"too many {field} pages")


def content(value: object) -> bytes:
    value = obj(value, "workflow content")
    if text(value.get("encoding"), "workflow content.encoding") != "base64":
        raise EvidenceError("workflow content is not base64")
    encoded = text(value.get("content"), "workflow content.content").replace("\r", "").replace("\n", "")
    if any(character.isspace() for character in encoded):
        raise EvidenceError("workflow content contains invalid whitespace")
    try:
        return base64.b64decode(encoded.encode("ascii", "strict"), validate=True)
    except (UnicodeEncodeError, binascii.Error) as exc:
        raise EvidenceError("workflow content has invalid base64") from exc


def expected_profile(workflow: bytes) -> dict[str, object]:
    profile = {"check_id": "linux-release-quality", "job_name": JOB, "runner": "ubuntu-latest/linux64", "schema": "krr-ci-evidence-profile-v1", "step_name": STEP, "workflow_path": SOURCE_WORKFLOW, "workflow_sha256": hashlib.sha256(workflow).hexdigest()}
    return {"digest": hashlib.sha256(canonical_json_bytes(profile)).hexdigest(), "profile": profile}


def source_run(value: object, repository: str, pr: int, head: str, ref: str) -> Mapping[str, Any] | None:
    value = obj(value, "workflow run")
    if value.get("event") != "pull_request" or value.get("head_sha") != head or value.get("head_branch") != ref:
        return None
    matches(text(value.get("path"), "workflow run.path"), SOURCE_WORKFLOW, "source workflow path")
    matches(number(value.get("run_attempt"), "workflow run.run_attempt"), 1, "source workflow attempt")
    number(value.get("id"), "workflow run.id")
    matches(text(obj(value.get("head_repository"), "workflow run.head_repository").get("full_name"), "workflow run repository"), repository, "source workflow repository")
    requests = array(value.get("pull_requests"), "workflow run.pull_requests")
    if len(requests) != 1:
        raise ReuseUnavailable("source workflow pull request is not unique")
    matches(number(obj(requests[0], "workflow pull request").get("number"), "workflow pull request number"), pr, "source workflow pull request")
    status = text(value.get("status"), "workflow run.status")
    if status in PENDING_WORKFLOW_STATUSES:
        raise ReusePending(f"source workflow run is still {status}")
    matches(status, "completed", "workflow run status")
    matches(text(value.get("conclusion"), "workflow run.conclusion"), "success", "workflow run conclusion")
    return value


def verify_job(get: Getter, repository: str, run_id: int) -> None:
    jobs = [obj(item, "workflow job") for item in pages(get, f"/repos/{repository}/actions/runs/{run_id}/jobs", "jobs")]
    candidates = [job for job in jobs if job.get("name") == JOB]
    if len(candidates) != 1:
        raise ReuseUnavailable("source workflow Linux job is not unique")
    job = candidates[0]
    matches(text(job.get("status"), "job.status"), "completed", "job status")
    matches(text(job.get("conclusion"), "job.conclusion"), "success", "job conclusion")
    steps = [obj(item, "job step") for item in array(job.get("steps"), "job.steps")]
    candidates = [step for step in steps if step.get("name") == STEP]
    if len(candidates) != 1:
        raise ReuseUnavailable("source workflow quality step is not unique")
    matches(text(candidates[0].get("status"), "step.status"), "completed", "quality step status")
    matches(text(candidates[0].get("conclusion"), "step.conclusion"), "success", "quality step conclusion")


def publisher_run_id(check: Mapping[str, Any], repository: str, source_run_id: int) -> int:
    """Bind an in-progress publisher check to the CI run that triggered it."""

    matches(text(check.get("name"), "publisher check.name"), CHECK, "publisher check name")
    app = obj(check.get("app"), "publisher check app")
    matches(number(app.get("id"), "publisher check app.id"), APP_ID, "publisher check app")
    matches(text(app.get("slug"), "publisher check app.slug"), "github-actions", "publisher check app slug")
    matches(text(obj(check.get("output"), "publisher check.output").get("title"), "publisher check title"), CHECK_TITLE, "publisher check title")
    lines = text(obj(check.get("output"), "publisher check.output").get("text"), "publisher check.output.text").splitlines()
    if not lines or lines[0] != f"source-run: {source_run_id}":
        raise ReuseUnavailable("publisher check does not identify the source CI run")
    details = urllib.parse.urlparse(text(check.get("details_url"), "publisher check.details_url"))
    if details.scheme != "https" or details.netloc != "github.com" or details.query or details.fragment:
        raise ReuseUnavailable("publisher check details URL is invalid")
    parts = details.path.strip("/").split("/")
    expected = repository.split("/") + ["actions", "runs"]
    if parts[:4] != expected or len(parts) < 5 or not parts[4].isdigit() or int(parts[4]) <= 0:
        raise ReuseUnavailable("publisher check details URL does not identify an Actions run")
    return int(parts[4])


def pending_publisher(get: Getter, repository: str, head_sha: str, source_run_id: int) -> None:
    """Wait only for the Actions publisher run proven to belong to this source run."""

    checks = [obj(item, "check") for item in pages(get, f"/repos/{repository}/commits/{head_sha}/check-runs?check_name={urllib.parse.quote(CHECK, safe='')}", "check_runs")]
    checks = [item for item in checks if item.get("name") == CHECK]
    # The publisher creates its check from its first step.  A successful source
    # run can therefore be visible before the workflow_run delivery has started
    # the publisher.  That absence is transient and must use the caller's
    # bounded retry budget; a present but ambiguous check is never trusted.
    if not checks:
        raise ReusePending("publisher workflow has not created its check yet")
    if len(checks) != 1:
        raise ReuseUnavailable("publisher workflow check is not unique")
    publisher_id = publisher_run_id(checks[0], repository, source_run_id)
    publisher = obj(get(f"/repos/{repository}/actions/runs/{publisher_id}"), "publisher run")
    matches(number(publisher.get("id"), "publisher run.id"), publisher_id, "publisher run ID")
    matches(text(publisher.get("event"), "publisher run.event"), "workflow_run", "publisher run event")
    matches(text(publisher.get("path"), "publisher run.path"), PUBLISHER_WORKFLOW, "publisher workflow")
    status = text(publisher.get("status"), "publisher run.status")
    if status in PENDING_WORKFLOW_STATUSES:
        raise ReusePending(f"publisher workflow run is still {status}")
    matches(status, "completed", "publisher run status")
    matches(text(publisher.get("conclusion"), "publisher run.conclusion"), "success", "publisher conclusion")
    raise ReuseUnavailable("publisher completed without publishing its evidence artifact")


def manifest(download: Downloader, artifact: Mapping[str, Any]) -> object:
    data = download(text(artifact.get("archive_download_url"), "artifact archive URL"))
    try:
        with zipfile.ZipFile(BytesIO(data)) as archive:
            entries = archive.infolist()
            if len(entries) != 1 or entries[0].filename != "manifest.json" or entries[0].is_dir() or entries[0].file_size > MAX_BYTES or (entries[0].external_attr >> 16) & 0o170000 == 0o120000:
                raise EvidenceError("artifact must contain one bounded regular manifest.json")
            data = archive.read(entries[0])
    except (OSError, zipfile.BadZipFile) as exc:
        raise EvidenceError("artifact is not a valid zip") from exc
    try:
        value = json.loads(data.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise EvidenceError("manifest is not UTF-8 JSON") from exc
    if canonical_json_bytes(value) + b"\n" != data:
        raise EvidenceError("manifest is not canonical JSON")
    return value


def verify_reusable_evidence(get: Getter, download: Downloader, *, repository: str, pr_number: int, head_sha: str, head_ref: str, profile: str) -> dict[str, object]:
    if profile != PROFILE or SHA.fullmatch(head_sha) is None or not head_ref or "\x00" in head_ref:
        raise EvidenceError("invalid evidence arguments")
    pr = obj(get(f"/repos/{repository}/pulls/{pr_number}"), "pull request")
    matches(number(pr.get("number"), "pull request.number"), pr_number, "pull request number")
    matches(text(pr.get("state"), "pull request.state"), "open", "pull request state")
    head, base = obj(pr.get("head"), "pull request.head"), obj(pr.get("base"), "pull request.base")
    matches(sha(head.get("sha"), "pull request.head.sha"), head_sha, "pull request SHA")
    matches(text(head.get("ref"), "pull request.head.ref"), head_ref, "pull request ref")
    matches(text(base.get("ref"), "pull request.base.ref"), "master", "pull request base")
    base_sha = sha(base.get("sha"), "pull request.base.sha")
    raw_tree = get(f"/repos/{repository}/git/trees/{head_sha}?recursive=1")
    try:
        tree = digest_input_tree(raw_tree)
    except TreeSchemaError as exc:
        raise EvidenceError("source tree cannot be digested") from exc
    profile_data = expected_profile(content(get(f"/repos/{repository}/contents/{SOURCE_WORKFLOW}?ref={head_sha}")))
    runs = pages(get, f"/repos/{repository}/actions/workflows/test-and-build.yml/runs?event=pull_request&head_sha={head_sha}", "workflow_runs")
    sources = [candidate for value in runs if (candidate := source_run(value, repository, pr_number, head_sha, head_ref)) is not None]
    if len(sources) != 1:
        raise ReuseUnavailable("matching successful source CI run is not unique")
    source = sources[0]
    run_id = number(source.get("id"), "source run.id")
    verify_job(get, repository, run_id)
    artifact_name = ARTIFACT_PREFIX + str(run_id)
    artifacts = [obj(item, "artifact") for item in pages(get, f"/repos/{repository}/actions/artifacts?name={urllib.parse.quote(artifact_name, safe='')}", "artifacts")]
    artifacts = [item for item in artifacts if item.get("name") == artifact_name]
    if not artifacts:
        pending_publisher(get, repository, head_sha, run_id)
    if len(artifacts) != 1:
        raise ReuseUnavailable("matching evidence artifact is not unique")
    artifact = artifacts[0]
    matches(artifact.get("expired"), False, "artifact expiry")
    artifact_id = number(artifact.get("id"), "artifact.id")
    publisher_id = number(obj(artifact.get("workflow_run"), "artifact.workflow_run").get("id"), "publisher run.id")
    publisher = obj(get(f"/repos/{repository}/actions/runs/{publisher_id}"), "publisher run")
    matches(number(publisher.get("id"), "publisher run.id"), publisher_id, "publisher run ID")
    matches(text(publisher.get("event"), "publisher run.event"), "workflow_run", "publisher run event")
    matches(text(publisher.get("path"), "publisher run.path"), PUBLISHER_WORKFLOW, "publisher workflow")
    if publisher.get("conclusion") is None:
        raise ReusePending("publisher workflow run is still in progress")
    matches(text(publisher.get("conclusion"), "publisher run.conclusion"), "success", "publisher conclusion")
    data = obj(manifest(download, artifact), "manifest")
    matches(data.get("schema"), "krr-ci-evidence-publisher-v1", "manifest schema")
    matches(data.get("check_id"), "linux-release-quality", "manifest check")
    matches(data.get("input"), tree, "manifest input tree")
    matches(data.get("profile"), profile_data, "manifest profile")
    source_data = obj(data.get("source_run"), "manifest source")
    matches(source_data.get("id"), run_id, "manifest source run")
    matches(source_data.get("head_sha"), head_sha, "manifest source SHA")
    matches(source_data.get("base_sha"), base_sha, "manifest source base SHA")
    matches(source_data.get("pull_request"), pr_number, "manifest source pull request")
    matches(source_data.get("run_attempt"), 1, "manifest source attempt")
    matches(source_data.get("workflow_path"), SOURCE_WORKFLOW, "manifest source workflow")
    checks = [obj(item, "check") for item in pages(get, f"/repos/{repository}/commits/{head_sha}/check-runs?check_name={urllib.parse.quote(CHECK, safe='')}", "check_runs")]
    checks = [item for item in checks if item.get("name") == CHECK]
    if len(checks) != 1:
        raise ReuseUnavailable("evidence check is not unique")
    check = checks[0]
    app = obj(check.get("app"), "check app")
    matches(number(app.get("id"), "check app.id"), APP_ID, "check app")
    matches(text(app.get("slug"), "check app.slug"), "github-actions", "check app slug")
    matches(text(check.get("status"), "check.status"), "completed", "check status")
    matches(text(check.get("conclusion"), "check.conclusion"), "success", "check conclusion")
    matches(sha(check.get("head_sha"), "check.head_sha"), head_sha, "check head SHA")
    matches(text(obj(check.get("output"), "check.output").get("title"), "check title"), CHECK_TITLE, "check title")
    return {"artifact_id": artifact_id, "check_id": number(check.get("id"), "check.id"), "decision": "reuse", "profile": profile_data, "schema": "krr-ci-evidence-reuse-v1", "source_run_id": run_id}


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api-url", default="https://api.github.com")
    parser.add_argument("--repository", required=True)
    parser.add_argument("--pr-number", required=True, type=int)
    parser.add_argument("--head-sha", required=True)
    parser.add_argument("--head-ref", required=True)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    if args.pr_number <= 0:
        parser.error("--pr-number must be positive")
    try:
        get, download = clients(args.api_url, args.repository, os.environ.get("GH_TOKEN", ""))
        result = verify_reusable_evidence(get, download, repository=args.repository, pr_number=args.pr_number, head_sha=args.head_sha, head_ref=args.head_ref, profile=args.profile)
        args.output.write_bytes(canonical_json_bytes(result) + b"\n")
    except ReusePending as exc:
        args.output.write_bytes(canonical_json_bytes({"decision": "pending", "reason": str(exc)}) + b"\n")
        return 11
    except ReuseUnavailable as exc:
        args.output.write_bytes(canonical_json_bytes({"decision": "rerun", "reason": str(exc)}) + b"\n")
        return 10
    except (EvidenceError, OSError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
