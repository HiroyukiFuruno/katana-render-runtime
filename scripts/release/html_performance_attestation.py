"""Verify immutable GitHub certificate identities for a performance artifact."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

from native_coverage_reuse import _parse_attestation_output
from verify_html_performance_evidence import JsonObject, _integer, _object, _objects, _text, git, require, sha256, unique_object

WORKFLOW = ".github/workflows/test-and-build.yml"


def _github_json(root: Path, api_path: str) -> JsonObject:
    raw = subprocess.check_output(["gh", "api", api_path], cwd=root)
    return _object(json.loads(raw, object_pairs_hook=unique_object), "GitHub API response")


def _attested_commit_matches_run(root: Path, repository: str, run: JsonObject,
                                 checkout_head: str, source_sha: str) -> str:
    event = run.get("event")
    if event == "pull_request":
        pulls = _objects(run.get("pull_requests"), "attested pull requests")
        require(len(pulls) == 1, "attested pull request identity differs")
        pull = pulls[0]
        number = _integer(pull.get("number"), "attested pull request number")
        base = _text(_object(pull.get("base"), "attested PR base").get("sha"), "attested base SHA")
        head = _text(_object(pull.get("head"), "attested PR head").get("sha"), "attested head SHA")
        require(number > 0 and head == checkout_head, "attested PR source differs")
        commit = _github_json(root, f"repos/{repository}/git/commits/{source_sha}")
        parents = _objects(commit.get("parents"), "attested merge parents")
        require(commit.get("sha") == source_sha and [row.get("sha") for row in parents] == [base, head],
                "attested PR merge parents differ")
        return f"refs/pull/{number}/merge"
    branch = _text(run.get("head_branch"), "attested source branch")
    require(event in {"push", "workflow_dispatch"} and run.get("head_sha") == checkout_head == source_sha
            and re.fullmatch(r"[A-Za-z0-9._/-]+", branch) is not None, "attested source ref differs")
    return "refs/heads/" + branch


def _require_reviewed_workflow_blob(root: Path, repository: str, revision: str, checkout_head: str) -> None:
    require(re.fullmatch(r"[0-9a-f]{40}", revision) is not None, "attested workflow SHA differs")
    expected = git(root, "rev-parse", f"{checkout_head}:{WORKFLOW}").strip()
    item = _github_json(root, f"repos/{repository}/contents/{WORKFLOW}?ref={revision}")
    require(item.get("type") == "file" and item.get("path") == WORKFLOW and item.get("sha") == expected,
            "attested workflow differs from reviewed source")


def verify_remote_attestation(archive: Path, root: Path, repository: str, run: JsonObject,
                              subject_name: str, checkout_head: str) -> None:
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None,
            "attestation repository differs")
    run_id = _integer(run.get("id"), "attested run id")
    attempt = _integer(run.get("run_attempt"), "attested run attempt")
    require(run_id > 0 and attempt > 0 and re.fullmatch(r"[0-9a-f]{40}", checkout_head) is not None,
            "attested workflow identity differs")
    checksum = sha256(archive)
    command = ["gh", "attestation", "verify", str(archive), "--repo", repository,
               "--signer-workflow", f"{repository}/{WORKFLOW}", "--deny-self-hosted-runners", "--format", "json"]
    try:
        completed = subprocess.run(command, cwd=root, text=True, capture_output=True, check=False, timeout=120)
    except subprocess.TimeoutExpired as error:
        raise ValueError("GitHub attestation verification timed out") from error
    require(completed.returncode == 0 and sha256(archive) == checksum, "GitHub attestation verification failed")
    verified = _parse_attestation_output(completed.stdout)
    invocation = f"https://github.com/{repository}/actions/runs/{run_id}/attempts/{attempt}"
    matched = 0
    for row in verified:
        verification = _object(row.get("verificationResult"), "attestation verification")
        certificate = _object(_object(verification.get("signature"), "attestation signature").get("certificate"),
                              "attestation certificate")
        if certificate.get("runInvocationURI") != invocation:
            continue
        source_sha = _text(certificate.get("sourceRepositoryDigest"), "attested source SHA")
        signer_sha = _text(certificate.get("buildSignerDigest"), "attested signer SHA")
        config_sha = _text(certificate.get("buildConfigDigest"), "attested workflow configuration SHA")
        require(all(re.fullmatch(r"[0-9a-f]{40}", value) is not None
                    for value in (source_sha, signer_sha, config_sha)), "attested commit SHA differs")
        source_ref = _attested_commit_matches_run(root, repository, run, checkout_head, source_sha)
        workflow_prefix = f"https://github.com/{repository}/{WORKFLOW}@"
        allowed_refs = {source_ref}
        branch = run.get("head_branch")
        if isinstance(branch, str) and re.fullmatch(r"[A-Za-z0-9._/-]+", branch) is not None:
            allowed_refs.add("refs/heads/" + branch)
        signer_uri = _text(certificate.get("buildSignerURI"), "attested signer URI")
        config_uri = _text(certificate.get("buildConfigURI"), "attested workflow configuration URI")
        require(certificate.get("sourceRepositoryURI") == f"https://github.com/{repository}"
                and certificate.get("sourceRepositoryRef") == source_ref
                and certificate.get("runnerEnvironment") == "github-hosted"
                and signer_uri.startswith(workflow_prefix) and signer_uri[len(workflow_prefix):] in allowed_refs
                and config_uri.startswith(workflow_prefix) and config_uri[len(workflow_prefix):] in allowed_refs,
                "attestation certificate identity differs")
        for revision in {source_sha, signer_sha, config_sha}:
            _require_reviewed_workflow_blob(root, repository, revision, checkout_head)
        statement = _object(verification.get("statement"), "attestation statement")
        subjects = _objects(statement.get("subject"), "attestation subjects")
        if (statement.get("predicateType") == "https://slsa.dev/provenance/v1"
                and any(item.get("name") == subject_name
                        and _object(item.get("digest"), "attestation digest").get("sha256") == checksum
                        for item in subjects)):
            matched += 1
    require(matched == 1, "attestation certificate run or subject differs")
