#!/usr/bin/env python3
"""Publish fail-closed evidence for a successful PR Linux quality run."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Mapping, Protocol, Sequence

try:
    from .schema import TreeSchemaError
    from .tree_digest import canonical_json_bytes, digest_input_tree
except ImportError:  # Direct workflow execution has no package context.
    from schema import TreeSchemaError
    from tree_digest import canonical_json_bytes, digest_input_tree


SCHEMA = "krr-ci-evidence-publisher-v1"
PROFILE_SCHEMA = "krr-ci-evidence-profile-v1"
CHECK_ID = "linux-release-quality"
WORKFLOW_PATH = ".github/workflows/test-and-build.yml"
CHECK_TITLE = "Trusted Linux release-quality evidence published"
CANONICAL_JOB_NAME = "Test and Build (ubuntu-latest, linux64)"
CANONICAL_STEP_NAME = "Run shared release quality gate"
ACTIONS_APP_ID = 15368
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
REPOSITORY_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


class PublishError(ValueError):
    """Raised when a trusted CI input cannot be proven."""


class RestAdapter(Protocol):
    """The only network boundary used by publisher logic."""

    def request(self, method: str, path: str, body: object | None = None) -> object:
        ...


def _mapping(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise PublishError(f"{label} must be an object")
    return value


def _string(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise PublishError(f"{label} must be a string")
    try:
        value.encode("utf-8", "strict")
    except UnicodeEncodeError as exc:
        raise PublishError(f"{label} must be UTF-8") from exc
    return value


def _sha(value: object, label: str) -> str:
    value = _string(value, label)
    if SHA_RE.fullmatch(value) is None:
        raise PublishError(f"{label} must be a lowercase 40-character SHA")
    return value


def _integer(value: object, label: str) -> int:
    if type(value) is not int:
        raise PublishError(f"{label} must be an integer")
    return value


def _content_bytes(document: object, label: str) -> bytes:
    content = _mapping(document, label)
    if content.get("encoding") != "base64":
        raise PublishError(f"{label}.encoding must be base64")
    encoded = _string(content.get("content"), f"{label}.content")
    compact = encoded.replace("\r", "").replace("\n", "")
    if any(character.isspace() for character in compact):
        raise PublishError(f"{label}.content may only contain CR/LF whitespace")
    try:
        return base64.b64decode(compact.encode("ascii", "strict"), validate=True)
    except (UnicodeEncodeError, binascii.Error) as exc:
        raise PublishError(f"{label}.content is not strict base64") from exc


def _workflow_run(event: object) -> Mapping[str, Any]:
    root = _mapping(event, "event")
    run = _mapping(root.get("workflow_run"), "event.workflow_run")
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise PublishError("workflow run must be completed successfully")
    if run.get("event") != "pull_request":
        raise PublishError("workflow run must originate from pull_request")
    if run.get("path") != WORKFLOW_PATH:
        raise PublishError("workflow run did not use the canonical CI workflow path")
    if _integer(run.get("run_attempt"), "workflow_run.run_attempt") != 1:
        raise PublishError("workflow run must be its first attempt")
    _integer(run.get("id"), "workflow_run.id")
    _sha(run.get("head_sha"), "workflow_run.head_sha")
    return run


def _pull_request(run: Mapping[str, Any], default_branch: str, repository: str) -> Mapping[str, Any]:
    requests = run.get("pull_requests")
    if not isinstance(requests, list) or len(requests) != 1:
        raise PublishError("workflow run must identify exactly one pull request")
    request = _mapping(requests[0], "workflow_run.pull_requests[0]")
    head = _mapping(request.get("head"), "workflow_run.pull_requests[0].head")
    base = _mapping(request.get("base"), "workflow_run.pull_requests[0].base")
    # workflow_run の pull_requests 内の repository は最小表現で full_name を持たない。
    # フォークを拒否する判定は完全な head_repository を使う。
    head_repository = _mapping(run.get("head_repository"), "workflow_run.head_repository")
    if _string(head_repository.get("full_name"), "workflow_run.head_repository.full_name") != repository:
        raise PublishError("workflow run must come from a local pull request")
    if _sha(head.get("sha"), "pull request head.sha") != _sha(run.get("head_sha"), "workflow_run.head_sha"):
        raise PublishError("pull request head SHA does not match workflow run")
    if _string(base.get("ref"), "pull request base.ref") != default_branch:
        raise PublishError("pull request base is not the default branch")
    _sha(base.get("sha"), "pull request base.sha")
    _integer(request.get("number"), "pull request number")
    return request


def _canonical_job(jobs_document: object) -> Mapping[str, Any]:
    jobs_root = _mapping(jobs_document, "workflow jobs")
    jobs = jobs_root.get("jobs")
    if not isinstance(jobs, list):
        raise PublishError("workflow jobs must be an array")
    total_count = _integer(jobs_root.get("total_count"), "workflow jobs.total_count")
    if total_count != len(jobs) or total_count > 100:
        raise PublishError("workflow jobs response must contain every job in one page")
    candidates: list[Mapping[str, Any]] = []
    for index, raw_job in enumerate(jobs):
        job = _mapping(raw_job, f"workflow jobs[{index}]")
        if job.get("name") == CANONICAL_JOB_NAME:
            candidates.append(job)
    if len(candidates) != 1:
        raise PublishError("exactly one canonical Linux quality job is required")
    job = candidates[0]
    if job.get("status") != "completed" or job.get("conclusion") != "success":
        raise PublishError("canonical Linux quality job did not succeed")
    steps = job.get("steps")
    if not isinstance(steps, list):
        raise PublishError("canonical Linux quality job steps must be an array")
    matching_steps = []
    for index, raw_step in enumerate(steps):
        step = _mapping(raw_step, f"canonical job steps[{index}]")
        if step.get("name") == CANONICAL_STEP_NAME:
            matching_steps.append(step)
    if len(matching_steps) != 1:
        raise PublishError("exactly one shared release-quality step is required")
    step = matching_steps[0]
    if step.get("status") != "completed" or step.get("conclusion") != "success":
        raise PublishError("shared release-quality step did not succeed")
    return job


def _assert_actions_identity(response: object) -> int:
    result = _mapping(response, "check run response")
    check_id = _integer(result.get("id"), "check run response.id")
    app = _mapping(result.get("app"), "check run response.app")
    if _integer(app.get("id"), "check run response.app.id") != ACTIONS_APP_ID:
        raise PublishError("check run was not created by the GitHub Actions App")
    if app.get("slug") != "github-actions":
        raise PublishError("check run app slug was not github-actions")
    return check_id


def _profile(workflow_bytes: bytes) -> dict[str, object]:
    workflow_sha256 = hashlib.sha256(workflow_bytes).hexdigest()
    profile = {
        "check_id": CHECK_ID,
        "job_name": CANONICAL_JOB_NAME,
        "runner": "ubuntu-latest/linux64",
        "schema": PROFILE_SCHEMA,
        "step_name": CANONICAL_STEP_NAME,
        "workflow_path": WORKFLOW_PATH,
        "workflow_sha256": workflow_sha256,
    }
    return {"digest": hashlib.sha256(canonical_json_bytes(profile)).hexdigest(), "profile": profile}


def publish(event: object, repository: str, rest: RestAdapter) -> tuple[dict[str, object], int]:
    """Validate one CI run, write its canonical manifest, and make an Actions check."""

    if REPOSITORY_RE.fullmatch(repository) is None:
        raise PublishError("repository must be an owner/name pair")
    root = _mapping(event, "event")
    repo = _mapping(root.get("repository"), "event.repository")
    default_branch = _string(repo.get("default_branch"), "event.repository.default_branch")
    if not default_branch or "/" in default_branch:
        raise PublishError("event.repository.default_branch is invalid")
    run = _workflow_run(event)
    pull_request = _pull_request(run, default_branch, repository)
    head_sha = _sha(run.get("head_sha"), "workflow_run.head_sha")
    base_sha = _sha(
        _mapping(pull_request.get("base"), "pull request base").get("sha"),
        "pull request base.sha",
    )
    run_id = _integer(run.get("id"), "workflow_run.id")

    default_ref = _mapping(
        rest.request("GET", f"/repos/{repository}/git/ref/heads/{default_branch}"),
        "default branch ref",
    )
    default_object = _mapping(default_ref.get("object"), "default branch ref.object")
    if _sha(default_object.get("sha"), "default branch ref.object.sha") != base_sha:
        raise PublishError("pull request base SHA is stale relative to the default branch")
    default_workflow = _content_bytes(
        rest.request("GET", f"/repos/{repository}/contents/{WORKFLOW_PATH}?ref={base_sha}"),
        "default workflow content",
    )
    pull_workflow = _content_bytes(
        rest.request("GET", f"/repos/{repository}/contents/{WORKFLOW_PATH}?ref={head_sha}"),
        "pull request workflow content",
    )
    if default_workflow != pull_workflow:
        raise PublishError("default and pull request workflow bytes differ")

    jobs = rest.request("GET", f"/repos/{repository}/actions/runs/{run_id}/jobs?per_page=100")
    _canonical_job(jobs)
    try:
        input_tree = digest_input_tree(
            rest.request("GET", f"/repos/{repository}/git/trees/{head_sha}?recursive=1")
        )
    except TreeSchemaError as exc:
        raise PublishError(str(exc)) from exc
    profile = _profile(default_workflow)
    manifest: dict[str, object] = {
        "check_id": CHECK_ID,
        "input": input_tree,
        "profile": profile,
        "schema": SCHEMA,
        "source_run": {
            "head_sha": head_sha,
            "id": run_id,
            "pull_request": _integer(pull_request.get("number"), "pull request number"),
            "run_attempt": 1,
            "workflow_path": WORKFLOW_PATH,
        },
    }
    manifest_bytes = canonical_json_bytes(manifest)
    manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
    output = {
        "name": "KRR / CI evidence (linux release quality)",
        "head_sha": head_sha,
        "status": "in_progress",
        "output": {
            "title": CHECK_TITLE,
            "summary": f"manifest-sha256: {manifest_digest}",
            "text": (
                f"source-run: {run_id}\n"
                f"artifact: ci-evidence-linux-release-quality-{run_id}\n"
                f"input-tree-sha256: {input_tree['digest']}\n"
                f"profile-sha256: {profile['digest']}"
            ),
        },
    }
    check = rest.request("POST", f"/repos/{repository}/check-runs", output)
    return manifest, _assert_actions_identity(check)


def complete_check(repository: str, check_id: int, rest: RestAdapter) -> None:
    """Mark a previously identity-verified check successful after artifact upload."""

    if REPOSITORY_RE.fullmatch(repository) is None:
        raise PublishError("repository must be an owner/name pair")
    if check_id <= 0:
        raise PublishError("check run ID must be positive")
    response = rest.request(
        "PATCH",
        f"/repos/{repository}/check-runs/{check_id}",
        {
            "status": "completed",
            "conclusion": "success",
            "output": {"title": CHECK_TITLE},
        },
    )
    if _assert_actions_identity(response) != check_id:
        raise PublishError("check run update returned a different check run ID")


class GitHubAdapter:
    """Small urllib adapter; publisher logic never calls it directly."""

    def __init__(self, token: str) -> None:
        if not token:
            raise PublishError("GITHUB_TOKEN is required")
        self.token = token

    def request(self, method: str, path: str, body: object | None = None) -> object:
        if not path.startswith("/"):
            raise PublishError("REST path must start with /")
        payload = None if body is None else canonical_json_bytes(body)
        request = urllib.request.Request(
            f"https://api.github.com{path}",
            data=payload,
            method=method,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "Content-Type": "application/json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read()
        except (urllib.error.URLError, OSError) as exc:
            raise PublishError(f"GitHub REST request failed: {method} {path}") from exc
        try:
            return json.loads(raw.decode("utf-8", "strict"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise PublishError("GitHub REST response was not UTF-8 JSON") from exc


def _read_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PublishError(f"cannot read event JSON from {path}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--complete-check", type=int)
    args = parser.parse_args(argv)
    try:
        adapter = GitHubAdapter(os.environ.get("GITHUB_TOKEN", ""))
        if args.complete_check is not None:
            if args.output is not None:
                raise PublishError("--output cannot be used with --complete-check")
            complete_check(args.repository, args.complete_check, adapter)
        else:
            if args.output is None:
                raise PublishError("--output is required when publishing a manifest")
            manifest, check_id = publish(_read_json(args.event), args.repository, adapter)
            if args.output.exists() and not args.output.is_file():
                raise PublishError("output is not a regular file")
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(canonical_json_bytes(manifest) + b"\n")
            output_path = os.environ.get("GITHUB_OUTPUT")
            if output_path:
                with Path(output_path).open("a", encoding="utf-8", newline="\n") as handle:
                    handle.write(f"check_run_id={check_id}\n")
    except PublishError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
