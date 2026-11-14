"""Owner-authenticated local HTML performance evidence; no private raw transport."""
from __future__ import annotations

import importlib.metadata
import json
import platform
import re
import subprocess
import time
from pathlib import Path
from typing import cast

import local_macos_evidence as mac
import html_performance_source_reuse_cli as reuse_cli
from html_performance_python_identity import installed_identity
import verify_html_performance_evidence as gate

MARKER = "<!-- krr-local-html-performance:v1 -->"
SCHEMA = "krr-local-html-performance-v2"
COMMAND = "verify_html_performance_evidence.verify"
REUSE_COMMAND = "html_performance_source_reuse_cli.verify_reuse_request"


def scope(workflow_sha: str) -> str:
    return mac.sha256_bytes(mac.canonical({"schema": SCHEMA, "command": COMMAND,
                                          "workflow_sha256": workflow_sha}))


def valid_macos_version(value: object) -> bool:
    if not isinstance(value, str) or re.fullmatch(r"[0-9]+(?:\.[0-9]+){1,2}", value) is None:
        return False
    return int(value.split(".", 1)[0]) > 0


def require_measurement_host(directory: Path, host_platform: str) -> None:
    bundle = gate.Bundle(directory)
    attempts = gate._objects(bundle.manifest.get("attempts"), "performance attempts")
    gate.require(len(attempts) == 84, "measurement host requires all 84 conditions")
    for attempt in attempts:
        expected = bundle.json(attempt["expected"])
        endpoint = gate._object(expected.get("endpointRefs"), "measurement endpoints")
        before = bundle.json(endpoint.get("before"))
        tools = gate._object(before.get("toolContext"), "measurement tool context")
        manifest = bundle.json(attempt["pairManifest"])
        gate.require(tools.get("platform") == host_platform and manifest.get("platform") == host_platform,
                     "measurement platform differs from current host")


def _payload(comment: object) -> dict[str, object] | None:
    if not isinstance(comment, dict) or not isinstance(comment.get("body"), str):
        return None
    body = comment["body"]
    if not body.startswith(MARKER + "\n"):
        return None
    try:
        value = json.loads(body[len(MARKER) + 1:], object_pairs_hook=mac.reject_duplicate_keys)
        return cast(dict[str, object], value) if isinstance(value, dict) else None
    except (ValueError, TypeError):
        return None


def valid(comment: object, repository: str, number: int, base: str, head: str,
          workflow_sha: str, owner: str, now: int) -> bool:
    value = _payload(comment)
    if not isinstance(comment, dict) or value is None:
        return False
    keys = {"schema", "repository", "pull_request", "base_sha", "head_sha", "workflow_sha256",
            "scope_sha256", "completed_at", "command", "bundle_manifest_sha256", "result_sha256",
            "conditions", "candidate_conditions", "checks", "formal_pass", "platform", "os_version",
            "tools", "memory_policy_version"}
    kind = value.get("verification_kind")
    source_reuse = isinstance(kind, str) and kind in (
        "documentation-source-reuse-v1", *reuse_cli.ARTIFACT_REUSE_KINDS
    )
    if source_reuse:
        keys |= {"verification_kind", "measured_head", "eligibility_sha256"}
    user = comment.get("user")
    login = user.get("login") if isinstance(user, dict) else None
    raw_tools = value.get("tools")
    if not isinstance(raw_tools, dict):
        return False
    tools = cast(dict[str, object], raw_tools)
    if (set(value) != keys or not isinstance(login, str)
            or login.lower() != owner.lower()
            or comment.get("author_association") != "OWNER"
            or value.get("schema") != SCHEMA or value.get("repository") != repository
            or value.get("memory_policy_version") != "html-memory-v5"
            or type(value.get("pull_request")) is not int or value["pull_request"] != number
            or value.get("base_sha") != base or value.get("head_sha") != head
            or value.get("workflow_sha256") != workflow_sha or value.get("scope_sha256") != scope(workflow_sha)
            or value.get("command") != (REUSE_COMMAND if source_reuse else COMMAND)
            or (source_reuse and (not isinstance(value.get("measured_head"), str)
                                  or mac.SHA_RE.fullmatch(cast(str, value["measured_head"])) is None
                                  or not mac.is_sha256(value.get("eligibility_sha256"))))
            or value.get("platform") != "macos-arm64"
            or not valid_macos_version(value.get("os_version"))
            or set(tools) != {"python", "scipy", "runtime_sha256"}
            or not isinstance(tools.get("python"), str)
            or not tools["python"]
            or tools.get("scipy") != "1.18.1"
            or not mac.is_sha256(tools.get("runtime_sha256"))
            or type(value.get("conditions")) is not int or value["conditions"] != 84
            or type(value.get("candidate_conditions")) is not int or value["candidate_conditions"] != 84
            or type(value.get("checks")) is not int or value["checks"] != 1680
            or value.get("formal_pass") is not True
            or not mac.is_sha256(value.get("bundle_manifest_sha256"))
            or not mac.is_sha256(value.get("result_sha256"))):
        return False
    try:
        completed = mac.iso_timestamp(value.get("completed_at"))
        created = mac.iso_timestamp(comment.get("created_at"))
        updated = mac.iso_timestamp(comment.get("updated_at"))
        return (created == updated and abs(completed - created) <= 300
                and 0 <= now - completed <= mac.FRESHNESS_SECONDS)
    except mac.EvidenceError:
        return False


def verify_from_api(repository: str, number: int, base: str, head: str) -> bool:
    try:
        if not mac.workflow_scope_supported():
            return False
        token = mac.gh_token()
        def snapshot() -> tuple[str, object, object, list[object]]:
            owner = mac.canonical_owner(mac.api_request(token, "GET", f"repos/{repository}"), repository)
            pr = mac.api_request(token, "GET", f"repos/{repository}/pulls/{number}")
            if mac.validate_pr(pr, repository, number) != (base, head):
                raise mac.EvidenceError("PR changed")
            compare = mac.api_request(token, "GET", f"repos/{repository}/compare/{base}...{head}")
            if not mac.base_is_merged(compare, base):
                raise mac.EvidenceError("base is not merged")
            comments = mac.read_comments_api(token, repository, number)
            return owner, pr, compare, comments
        first, second = snapshot(), snapshot()
        if mac.canonical(first) != mac.canonical(second):
            return False
        workflow_sha = mac.workflow_digest()
        now = int(time.time())
        marked = [item for item in first[3] if isinstance(item, dict)
                  and isinstance(item.get("body"), str) and item["body"].startswith(MARKER)]
        if any(_payload(item) is None for item in marked):
            return False
        current = [item for item in marked if (p := _payload(item)) is not None
                   and p.get("repository") == repository and p.get("pull_request") == number
                   and p.get("base_sha") == base and p.get("head_sha") == head
                   and p.get("workflow_sha256") == workflow_sha and p.get("scope_sha256") == scope(workflow_sha)
                   and not mac.is_expired_receipt(item, p, now)]
        return len(current) == 1 and valid(current[0], repository, number, base, head,
                                           workflow_sha, first[0], now)
    except (mac.EvidenceError, ValueError, TypeError, KeyError, OSError, RuntimeError,
            subprocess.SubprocessError):
        return False


def collect(repository: str, number: int, directory: Path,
            reuse_request: Path | None = None) -> dict[str, object]:
    os_version = mac.macos_product_version()
    if platform.system() != "Darwin" or platform.machine() != "arm64" or not valid_macos_version(os_version):
        raise mac.EvidenceError("local collection requires macOS Apple Silicon")
    host_platform = platform.platform()
    mac.require_standard_index_flags()
    if mac.git_status() or not mac.workflow_scope_supported():
        raise mac.EvidenceError("source or workflow is not eligible")
    head = mac.run_git("rev-parse", "HEAD")
    source = mac.head_worktree_bytes_digest(head)
    workflow_sha = mac.workflow_digest()
    token = mac.gh_token()
    owner = mac.canonical_owner(mac.api_request(token, "GET", f"repos/{repository}"), repository)
    login = mac.api_request(token, "GET", "user")
    login_name = login.get("login") if isinstance(login, dict) else None
    if not isinstance(login_name, str) or login_name.lower() != owner.lower():
        raise mac.EvidenceError("repository owner required")
    pr_path = f"repos/{repository}/pulls/{number}"
    base, pr_head = mac.validate_pr(mac.api_request(token, "GET", pr_path), repository, number)
    if head != pr_head or mac.run_git("merge-base", base, head) != base:
        raise mac.EvidenceError("PR source differs")
    manifest_sha = gate.sha256(directory / "bundle.json")
    runtime_identity = installed_identity("1.18.1", "2.5.4")
    eligibility_sha: str | None = None
    verification_kind: str | None = None
    measured_head = head
    if reuse_request is None:
        result = gate.verify(directory, mac.ROOT, memory_policy_version="html-memory-v5")
    else:
        eligibility_sha = gate.sha256(reuse_request)
        reuse_result = reuse_cli.verify_reuse_request(
            reuse_request, mac.ROOT, expected_measured_dir=directory,
            expected_measured_manifest_sha256=manifest_sha,
            expected_request_sha256=eligibility_sha,
            memory_policy_version="html-memory-v5",
        )
        gate.require(reuse_result.get("reuseEligibility") is True
                     and reuse_result.get("eligibleHead") == head,
                     "reuse eligibility does not match PR head")
        measured_head = gate._text(reuse_result.get("measuredHead"), "measured HEAD")
        gate.require(mac.SHA_RE.fullmatch(measured_head) is not None
                     and gate.Bundle(directory).manifest.get("head") == measured_head,
                     "reuse measured bundle head differs")
        reuse_kind = reuse_result.get("reuseKind")
        gate.require(reuse_kind in (None, *reuse_cli.ARTIFACT_REUSE_KINDS),
                     "unsupported reuse kind")
        verification_kind = ("documentation-source-reuse-v1" if reuse_kind is None
                             else cast(str, reuse_kind))
        result = gate._object(reuse_result.get("measuredResult"), "measured formal result")
        gate._require_formal_release_result(result)
        gate.require(gate.sha256(reuse_request) == eligibility_sha, "reuse request changed")
    gate._require_cli_memory_v5(result, reused=False)
    if installed_identity("1.18.1", "2.5.4") != runtime_identity:
        raise mac.EvidenceError("scientific runtime changed during verification")
    require_measurement_host(directory, host_platform)
    if (result.get("head") != measured_head or result.get("conditions") != 84
            or result.get("candidateConditions") != 84 or result.get("checks") != 1680):
        raise mac.EvidenceError("formal HTML84 result differs")
    mac.require_standard_index_flags()
    if (mac.git_status() or mac.run_git("rev-parse", "HEAD") != head
            or mac.head_worktree_bytes_digest(head) != source or mac.workflow_digest() != workflow_sha
            or not mac.workflow_scope_supported() or gate.sha256(directory / "bundle.json") != manifest_sha
            or mac.macos_product_version() != os_version or platform.platform() != host_platform
            or (reuse_request is not None and gate.sha256(reuse_request) != eligibility_sha)
            or mac.validate_pr(mac.api_request(token, "GET", pr_path), repository, number) != (base, head)
            or not mac.base_is_merged(mac.api_request(token, "GET", f"repos/{repository}/compare/{base}...{head}"), base)):
        raise mac.EvidenceError("source or PR changed during verification")
    projection: dict[str, object] = {"schema": SCHEMA, "repository": repository, "pull_request": number,
            "base_sha": base, "head_sha": head, "workflow_sha256": workflow_sha,
            "scope_sha256": scope(workflow_sha), "completed_at": mac.utc_now(),
            "command": REUSE_COMMAND if reuse_request is not None else COMMAND,
            "bundle_manifest_sha256": manifest_sha, "result_sha256": mac.sha256_bytes(mac.canonical(result)),
            "conditions": 84, "candidate_conditions": 84, "checks": 1680,
            "memory_policy_version": result["memoryPolicyVersion"],
            "formal_pass": True, "platform": "macos-arm64", "os_version": os_version,
            "tools": {"python": platform.python_version(), "scipy": importlib.metadata.version("scipy"),
                      "runtime_sha256": runtime_identity}}
    if reuse_request is not None:
        projection.update({"verification_kind": verification_kind, "measured_head": measured_head,
                           "eligibility_sha256": eligibility_sha})
    return projection


def require_fresh_measurement_if_local_expired(jobs: list[dict[str, object]]) -> None:
    paired = [job for job in jobs if job.get("name") == "Native paired HTML time and RSS nonregression"]
    if len(paired) == 1:
        steps = gate._objects(paired[0].get("steps", []), "paired job steps")
        reused = [step for step in steps if step.get("name") == "Report local HTML performance evidence"
                  and step.get("conclusion") == "success"]
        gate.require(not reused, "local HTML performance evidence expired or changed; rerun the required CI job for fresh measurement")


def main() -> int:
    import argparse
    import sys
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--pr", type=int, required=True)
    parser.add_argument("--base")
    parser.add_argument("--head")
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--reuse-request", type=Path)
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()
    try:
        if args.verify:
            if args.publish or args.bundle or args.reuse_request or not args.base or not args.head:
                raise mac.EvidenceError("invalid verification arguments")
            accepted = verify_from_api(args.repository, args.pr, args.base, args.head)
            mac.output_reuse(accepted, __import__("os").environ.get("GITHUB_OUTPUT"))
            return 0
        if args.bundle is None:
            raise mac.EvidenceError("bundle is required")
        payload = collect(args.repository, args.pr, args.bundle.resolve(),
                          args.reuse_request.absolute() if args.reuse_request is not None else None)
        if args.publish:
            token = mac.gh_token()
            base = gate._text(payload.get("base_sha"), "PR base")
            head = gate._text(payload.get("head_sha"), "PR head")
            if verify_from_api(args.repository, args.pr, base, head):
                print("A fresh local HTML performance proof already exists")
                return 0
            comments = mac.read_comments_api(token, args.repository, args.pr)
            for comment in comments:
                if not isinstance(comment, dict) or not isinstance(comment.get("body"), str):
                    continue
                if not comment["body"].startswith(MARKER):
                    continue
                prior = _payload(comment)
                if prior is None:
                    raise mac.EvidenceError("malformed HTML evidence marker")
                if (all(prior.get(key) == payload[key] for key in
                        ("repository", "pull_request", "base_sha", "head_sha", "workflow_sha256", "scope_sha256"))
                        and not mac.is_expired_receipt(comment, prior, int(time.time()))):
                    raise mac.EvidenceError("ambiguous current HTML evidence")
            posted = mac.api_request(token, "POST", f"repos/{args.repository}/issues/{args.pr}/comments",
                                     {"body": MARKER + "\n" + mac.canonical(payload).decode()})
            user = posted.get("user") if isinstance(posted, dict) else None
            owner = mac.canonical_owner(mac.api_request(token, "GET", f"repos/{args.repository}"), args.repository)
            posted_login = user.get("login") if isinstance(user, dict) else None
            if (not isinstance(posted_login, str) or posted_login.lower() != owner.lower()
                    or posted.get("author_association") != "OWNER"
                    or not verify_from_api(args.repository, args.pr, base, head)):
                raise mac.EvidenceError("published HTML evidence was not independently verified")
        print("Local HTML performance evidence validated" + (" and published" if args.publish else ""))
        return 0
    except (mac.EvidenceError, ValueError, TypeError, KeyError, OSError, RuntimeError,
            subprocess.SubprocessError) as error:
        print(f"local HTML performance evidence failed: {type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
