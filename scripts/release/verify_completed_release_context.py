#!/usr/bin/env python3
"""完了CIとマージ済みrelease PRを公開対象のSHAへ固定する。"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from collections.abc import Callable
from datetime import datetime

CI_PATH = ".github/workflows/test-and-build.yml"
NATIVE_JOBS = (
    "Test and Build (ubuntu-latest, linux64)",
    "Test and Build (macos-15, mac-arm64)",
    "Test and Build (windows-latest, win64)",
)
PAIRED_JOB = "Native paired HTML time and RSS nonregression"
SHA = re.compile(r"[0-9a-f]{40}\Z")
RELEASE = re.compile(r"release/(v[0-9]+\.[0-9]+\.[0-9]+)\Z")
JsonObject = dict[str, object]
Fetch = Callable[[str], object]


def json_object(value: object, message: str) -> JsonObject:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ValueError(message)
    return {key: item for key, item in value.items()}


def json_object_rows(value: object, message: str) -> list[JsonObject]:
    if not isinstance(value, list):
        raise ValueError(message)
    return [json_object(item, message) for item in value]


def json_string(value: object, message: str) -> str:
    if not isinstance(value, str):
        raise ValueError(message)
    return value


def positive_integer(value: object, message: str) -> int:
    if type(value) is not int or value <= 0:
        raise ValueError(message)
    return value


def object_field(value: JsonObject, key: str, message: str) -> JsonObject:
    return json_object(value.get(key, {}), message)


def string_field(value: JsonObject, key: str, message: str, default: object = "") -> str:
    return json_string(value.get(key, default), message)


def integer_value(value: object, message: str) -> int:
    if type(value) is not int:
        raise ValueError(message)
    return value


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def api(path: str) -> object:
    value: object = json.loads(subprocess.check_output(["gh", "api", path], text=True))
    return value


def pages(fetch: Fetch, path: str, key: str | None = None) -> list[JsonObject]:
    rows: list[JsonObject] = []
    for page in range(1, 101):
        payload = fetch(f"{path}{'&' if '?' in path else '?'}per_page=100&page={page}")
        payload_object = json_object(payload, "invalid API page") if key else None
        batch = payload_object[key] if key and payload_object is not None else payload
        batch_rows = json_object_rows(batch, "invalid API page")
        rows.extend(batch_rows)
        if len(batch_rows) < 100:
            if key:
                require(payload_object is not None and payload_object.get("total_count") == len(rows), "incomplete API collection")
            return rows
    raise ValueError("API pagination exceeded bound")


def complete(value: JsonObject, label: str) -> None:
    require(value.get("status") == "completed" and value.get("conclusion") == "success", f"{label} is not completed successfully")


def latest_generation(rows: list[JsonObject], label: str) -> JsonObject:
    require(bool(rows), f"completed {label} is absent")
    start_times: list[str] = []
    for row in rows:
        positive_integer(row.get("id"), f"invalid {label} generation")
        positive_integer(row.get("run_attempt"), f"invalid {label} generation")
        started = json_string(row.get("run_started_at"), f"invalid {label} generation start time")
        require(re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", started) is not None, f"invalid {label} generation start time")
        try:
            datetime.strptime(started, "%Y-%m-%dT%H:%M:%SZ")
        except ValueError as error:
            raise ValueError(f"invalid {label} generation start time") from error
        start_times.append(started)
    latest_start = max(start_times)
    latest = [row for row in rows if row.get("run_started_at") == latest_start]
    require(len(latest) == 1, f"ambiguous latest {label} generation")
    complete(latest[0], label)
    return latest[0]


def merge_generation(fetch: Fetch, base: str, repository: str, head: str, workflow: JsonObject) -> JsonObject:
    runs = pages(fetch, f"{base}/actions/workflows/test-and-build.yml/runs?head_sha={head}", "workflow_runs")
    matching = [run for run in runs if run.get("head_sha") == head and object_field(run, "head_repository", "CI repository mismatch").get("full_name") == repository]
    run = latest_generation(matching, "exact-source CI")
    require(run.get("workflow_id") == workflow.get("id") and string_field(run, "path", "CI workflow identity mismatch").split("@")[0] == CI_PATH, "CI workflow identity mismatch")
    require(object_field(run, "repository", "CI repository mismatch").get("full_name") == repository, "CI repository mismatch")
    require(run.get("event") in ("push", "workflow_dispatch") and run.get("head_branch") == "master", "release CI must measure committed master")
    return run


def preflight_generation(fetch: Fetch, base: str, repository: str, pull: JsonObject, workflow: JsonObject) -> JsonObject:
    pull_head = object_field(pull, "head", "invalid release branch")
    branch = json_string(pull_head.get("ref"), "invalid release branch")
    runs = pages(fetch, f"{base}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch={branch}", "workflow_runs")
    pull_number = pull.get("number")
    pull_sha = pull_head.get("sha")
    bound = [item for item in runs if object_field(item, "repository", "PR preflight repository mismatch").get("full_name") == repository and object_field(item, "head_repository", "PR preflight repository mismatch").get("full_name") == repository and any(binding.get("number") == pull_number and object_field(binding, "head", "PR preflight head mismatch").get("sha") == pull_sha for binding in json_object_rows(item.get("pull_requests", []), "invalid PR preflight bindings"))]
    run = latest_generation(bound, "exact-PR preflight")
    require(run.get("workflow_id") == workflow.get("id") and string_field(run, "path", "PR preflight workflow identity mismatch").split("@")[0] == CI_PATH and run.get("event") == "pull_request" and run.get("head_branch") == branch, "PR preflight workflow identity mismatch")
    head_sha = json_string(run.get("head_sha"), "invalid PR preflight synthetic merge SHA")
    require(SHA.fullmatch(head_sha) is not None, "invalid PR preflight synthetic merge SHA")
    return run


def verify(fetch: Fetch, repository: str, head: str, run_id: int | None = None, run_attempt: int | None = None, automatic: bool = False) -> JsonObject:
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None, "invalid repository")
    require(SHA.fullmatch(head) is not None, "invalid source SHA")
    base = f"repos/{repository}"
    # workflow_runはforkのCIにも反応するため、APIでrepoとworkflowの実体を照合する。
    workflow = json_object(fetch(f"{base}/actions/workflows/test-and-build.yml"), "unexpected CI workflow")
    require(workflow.get("path") == CI_PATH and workflow.get("state") == "active", "unexpected CI workflow")
    run = merge_generation(fetch, base, repository, head, workflow)
    if run_id is not None:
        require((run["id"], run["run_attempt"]) == (run_id, run_attempt), "workflow_run event is stale")
    pulls = pages(fetch, f"{base}/commits/{head}/pulls")
    releases = [pull for pull in pulls if RELEASE.fullmatch(string_field(object_field(pull, "head", "invalid release branch"), "ref", "invalid release branch"))]
    if not releases and automatic:
        return {"eligible": "false"}
    require(len(releases) == 1, "expected one merged release PR for exact source")
    pull = json_object(fetch(f"{base}/pulls/{releases[0]['number']}"), "release PR is not merged and ready")
    require(pull.get("state") == "closed" and pull.get("merged") is True and pull.get("draft") is False and bool(pull.get("merged_at")), "release PR is not merged and ready")
    require(pull.get("merge_commit_sha") == head, "release PR merge SHA differs from measured source")
    pull_base = object_field(pull, "base", "release PR repository/base mismatch")
    pull_base_repo = object_field(pull_base, "repo", "release PR repository/base mismatch")
    pull_head = object_field(pull, "head", "release PR repository/base mismatch")
    pull_head_repo = object_field(pull_head, "repo", "release PR repository/base mismatch")
    require(pull_base.get("ref") == "master" and pull_base_repo.get("full_name") == repository and pull_head_repo.get("full_name") == repository, "release PR repository/base mismatch")
    branch = json_string(pull_head.get("ref"), "invalid release branch")
    match = RELEASE.fullmatch(branch)
    if match is None:
        raise ValueError("invalid release branch")
    jobs = pages(fetch, f"{base}/actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs", "jobs")
    for name in (*NATIVE_JOBS, PAIRED_JOB):
        matches = [job for job in jobs if job.get("name") == name]
        require(len(matches) == 1 and matches[0].get("run_id") == run["id"] and matches[0].get("head_sha") == head, f"required CI job absent/foreign/duplicate: {name}")
        complete(matches[0], name)
    # PR検査はGitHubのsynthetic merge SHAに付くため、PR headとの対応をrun metadataで確認する。
    preflight = preflight_generation(fetch, base, repository, pull, workflow)
    preflight_jobs = pages(fetch, f"{base}/actions/runs/{preflight['id']}/attempts/{preflight['run_attempt']}/jobs", "jobs")
    matching_jobs = [job for job in preflight_jobs if job.get("name") == "preflight"]
    require(len(matching_jobs) == 1 and matching_jobs[0].get("run_id") == preflight["id"], "required preflight job absent/duplicate")
    complete(matching_jobs[0], "release PR preflight")
    commit = json_object(fetch(f"{base}/git/commits/{head}"), "merge commit/tree identity mismatch")
    tree = string_field(object_field(commit, "tree", "merge commit/tree identity mismatch"), "sha", "merge commit/tree identity mismatch")
    require(commit.get("sha") == head and SHA.fullmatch(tree) is not None, "merge commit/tree identity mismatch")
    # 最新runのrerun開始やPR snapshot変更は公開権限へ昇格しない。
    require(fetch(f"{base}/actions/runs/{run['id']}") == run, "CI generation changed during verification")
    require(fetch(f"{base}/actions/runs/{preflight['id']}") == preflight, "PR preflight generation changed")
    require(fetch(f"{base}/pulls/{pull['number']}") == pull, "release PR changed during verification")
    # 選択済みIDのGETだけでは、新規runが開始された競合を観測できない。
    require(merge_generation(fetch, base, repository, head, workflow) == run, "latest CI generation changed during verification")
    require(preflight_generation(fetch, base, repository, pull, workflow) == preflight, "latest PR preflight generation changed during verification")
    return {"eligible": "true", "sha": head, "tree": tree, "version": match[1], "release_branch": branch, "pull_request": str(pull['number']), "run_id": str(run['id']), "run_attempt": str(run['run_attempt'])}


def bind_tree(root: Path, context: JsonObject, requested_version: str = "") -> None:
    if context["eligible"] != "true":
        return
    def git(*args: str) -> str:
        return subprocess.check_output(["git", "--no-replace-objects", "-C", str(root), *args], text=True).strip()
    sha = json_string(context.get("sha"), "invalid source SHA")
    tree = json_string(context.get("tree"), "invalid source tree SHA")
    version = json_string(context.get("version"), "invalid release version")
    require(git("rev-parse", f"{sha}^{{tree}}") == tree, "local tree differs from verified merge tree")
    subprocess.run(["git", "--no-replace-objects", "-C", str(root), "merge-base", "--is-ancestor", sha, "origin/master"], check=True)
    cargo = json_object(tomllib.loads(git("show", f"{sha}:Cargo.toml")), "invalid committed Cargo.toml")
    package = object_field(cargo, "package", "invalid committed Cargo.toml")
    cargo_version = json_string(package.get("version"), "invalid committed Cargo version")
    require("v" + cargo_version == version, "release branch and committed Cargo version differ")
    require(not requested_version or "v" + requested_version.removeprefix("v") == version, "requested release version differs")


def bind_acquisition(context: JsonObject, receipt: JsonObject, repository: str) -> None:
    require(context.get("eligible") == "true", "acquisition cannot authorize nonrelease context")
    run_id = integer_value(receipt.get("runId"), "acquired evidence generation differs from completed release context")
    run_attempt = integer_value(receipt.get("runAttempt"), "acquired evidence generation differs from completed release context")
    sha = json_string(context.get("sha"), "invalid source SHA")
    context_run_id = json_string(context.get("run_id"), "invalid completed release run ID")
    context_run_attempt = json_string(context.get("run_attempt"), "invalid completed release run attempt")
    require(receipt.get("repository") == repository and receipt.get("head") == sha and (str(run_id), str(run_attempt)) == (context_run_id, context_run_attempt), "acquired evidence generation differs from completed release context")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--event", type=Path, default=Path(os.environ.get("GITHUB_EVENT_PATH", "")))
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--head", default=os.environ.get("GITHUB_SHA", ""))
    parser.add_argument("--version", default="")
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--acquisition-receipt", type=Path)
    args = parser.parse_args()
    try:
        event = json_object(json.loads(args.event.read_text()), "invalid event")
        trigger = event.get("workflow_run")
        if trigger:
            trigger_object = json_object(trigger, "foreign workflow_run source")
            trigger_repository = object_field(trigger_object, "head_repository", "foreign workflow_run source")
            require(trigger_repository.get("full_name") == args.repository, "foreign workflow_run source")
            trigger_sha = json_string(trigger_object.get("head_sha"), "invalid workflow_run source SHA")
            trigger_id = positive_integer(trigger_object.get("id"), "invalid workflow_run source generation")
            trigger_attempt = positive_integer(trigger_object.get("run_attempt"), "invalid workflow_run source generation")
            context = verify(api, args.repository, trigger_sha, trigger_id, trigger_attempt, True)
        else:
            context = verify(api, args.repository, args.head)
        bind_tree(args.repo, context, args.version)
        if args.acquisition_receipt:
            receipt = json_object(json.loads(args.acquisition_receipt.read_text()), "invalid acquisition receipt")
            bind_acquisition(context, receipt, args.repository)
        if args.github_output:
            with args.github_output.open("a") as output:
                output.writelines(f"{key}={value}\n" for key, value in context.items())
        print(json.dumps(context, sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError) as error:
        print(f"completed release context rejected: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
