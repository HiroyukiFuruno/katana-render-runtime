#!/usr/bin/env python3
"""Default-branch-only arbiter for the KRR PR-governance Check Run.

The dispatcher intentionally does no decision making.  It invalidates every
current head and starts this program once; this program serializes all final
decisions.  Keeping the API boundary here makes the two token scopes explicit:
the ambient Actions token is read-only and the App token is used only for the
single Check Run PATCH helper.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import time
from contextlib import contextmanager
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, Sequence
from urllib.parse import parse_qs, urlencode, urlparse, urlunparse


sys.path.insert(0, str(Path(__file__).parents[1] / "hooks"))
import verify_push_issue as issue_contract


REPOSITORY = os.environ.get("GITHUB_REPOSITORY", "")
SERVER_URL = os.environ.get("GITHUB_SERVER_URL", "")
WRITER_RUN_ID = os.environ.get("GITHUB_RUN_ID", "")
CHECK_NAME = "KRR / PR governance (trusted check)"
CHECK_EXTERNAL_PREFIX = "krr-governance/v1/"
CHECK_WRITE_INTERVAL_SECONDS = 8.1
# Four continuation writers share the App installation rate limit with their
# registration and terminal polling.  20.5 seconds keeps each 150-write
# continuation inside the terminal window after bounded registration,
# bootstrap, and initial-evidence work.
# hour below the 4,500-request operational ceiling.
ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS = 20.5
# Every ``gh api`` child is bounded independently.  Initial evidence also uses
# a shared monotonic deadline so a sequence of individually successful slow
# reads cannot consume the terminal writer's await reserve.
GITHUB_API_TIMEOUT_SECONDS = 20.0
INITIAL_EVIDENCE_DEADLINE_SECONDS = 180.0
# The dispatched workflow has two default-token bootstrap/rebind phases,
# checkout, and two App-token creations before this script starts.  Reserve
# their realistic 120s upper bound separately from initial evidence, with a
# final 30s scheduler margin: 120 + 180 + 30 <= 330 seconds.  The first
# paced PATCH is already included in the 150-write interval below.
TERMINAL_WRITER_STARTUP_RESERVE_SECONDS = 120.0
TERMINAL_AWAIT_STARTUP_AND_EVIDENCE_RESERVE_SECONDS = 330.0
DISPATCHER_NAME = "PR governance dispatcher"
DISPATCHER_PATH = ".github/workflows/pr-governance.yml"
WRITER_WORKFLOW_PATH = ".github/workflows/pr-governance-status-writer.yml"
PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME = "Preflight workflow_run governance source"
RESOLVER_FAILURE_BARRIER_NAME = "Establish resolver-failure merge barrier"
PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME = "Record verified pull_request_target preflight no-op"
PREFLIGHT_ISSUE_NOOP_STEP_NAME = "Record verified Issue preflight no-op"
# Manual recovery dispatches are a trusted dispatcher generation too.  The
# workflow and its default-branch binding are still validated below; this is
# only the event-kind allowlist, not permission to accept an arbitrary replay.
DISPATCHER_EVENTS = frozenset({"pull_request_target", "issue_comment", "issues", "schedule", "workflow_dispatch", "workflow_run"})
DISPATCHER_ACTIVE_STATUSES = frozenset({"requested", "queued", "pending", "waiting", "in_progress"})
DISPATCHER_TERMINAL_CONCLUSIONS = frozenset({
    "action_required", "cancelled", "failure", "neutral", "skipped", "stale",
    "startup_failure", "success", "timed_out",
})
MAX_DISPATCHER_FENCE_RUNS = 100
# A terminal continuation handles at most 150 PRs.  Every sensor/CI listing is
# restricted by the immutable current PR head and a small bounded page chain,
# rather than paging arbitrary retained workflow history with the App token.
MAX_EVIDENCE_TARGETS = 150
# GitHub returns at most 100 workflow runs per REST page.  Keep a separate
# aggregate bound so an exact-head query can safely consume a small number of
# pages without turning retained run history into an unbounded read loop or
# installation-rate exhaustion denial of service.
MAX_EVIDENCE_RUNS_PER_PAGE = 100
MAX_EVIDENCE_RUNS_PER_QUERY = 300
# Open-pull and Check Run pagination is bounded independently of evidence
# pagination.  Six 100-item pages cover the repository's 600-item contract;
# a full sixth page fails closed instead of issuing an unbounded seventh read.
MAX_SHARED_SNAPSHOT_PAGES = 6
# The repository workflow token is limited to 1,000 REST reads/hour.  A
# 150-head slice spends 903 of them on CI pages/fences, workflow IDs, and the
# sensor's page-1 anchor.  Move page 2 for only this bounded prefix (never
# data-dependent selection) to retain 47 reads of headroom while lowering the
# shared App bucket's worst rolling window.
MAX_DEFAULT_INITIAL_SENSOR_PAGE_2_HEADS = 50
SHA = re.compile(r"[0-9a-fA-F]{40}")
BODY_SHA256 = re.compile(r"[0-9a-f]{64}")
NUMBER = re.compile(r"[1-9][0-9]*")
_last_check_write_at: float | None = None
_bound_check_runs: dict[tuple[str, str], int] = {}
_bound_check_ids_by_number: dict[int, int] = {}
# A completed workflow_run no-op can be observed by every terminal head in one
# all-open writer.  Retain only the fully validated immutable identity; active
# or otherwise untrusted observations never enter this cache.
_nonreconciling_dispatcher_generations: dict[int, DispatcherGeneration] = {}
_INVALID_REPOSITORY_IDENTITY = object()
_active_initial_evidence_deadline: float | None = None
_terminal_deadline_monotonic: float | None = None


def _cleanup_snapshot_ledger(snapshot_path: str) -> None:
    """Remove only the ledger belonging to an unchanged private snapshot."""
    snapshot = Path(snapshot_path)
    try:
        snapshot_stat = snapshot.stat()
        parent_stat = snapshot.parent.stat()
        if (
            snapshot.is_symlink() or not stat.S_ISREG(snapshot_stat.st_mode)
            or snapshot_stat.st_uid != os.getuid() or snapshot_stat.st_nlink != 1
            or stat.S_IMODE(snapshot_stat.st_mode) != 0o600
            or parent_stat.st_uid != os.getuid()
            or stat.S_IMODE(parent_stat.st_mode) & 0o077
        ):
            raise GovernanceError("Snapshot cleanup boundary is invalid.")
        snapshot_sha = hashlib.sha256(snapshot.read_bytes()).hexdigest()
        ledger = Path(str(snapshot) + ".krr-graphql-ledger-v1")
        try:
            ledger_stat = ledger.lstat()
        except FileNotFoundError:
            return
        if (
            stat.S_ISLNK(ledger_stat.st_mode) or not stat.S_ISREG(ledger_stat.st_mode)
            or ledger_stat.st_uid != os.getuid() or ledger_stat.st_nlink != 1
            or stat.S_IMODE(ledger_stat.st_mode) != 0o600
        ):
            raise GovernanceError("Snapshot ledger cleanup identity is invalid.")
        try:
            state = json.loads(ledger.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
            raise GovernanceError("Snapshot ledger cleanup state is malformed.") from error
        if not isinstance(state, dict) or state.get("snapshot_sha256") != snapshot_sha:
            raise GovernanceError("Snapshot ledger cleanup identity is invalid.")
        current_snapshot = snapshot.stat()
        current_ledger = ledger.lstat()
        if (
            (current_snapshot.st_dev, current_snapshot.st_ino, current_snapshot.st_mtime_ns)
            != (snapshot_stat.st_dev, snapshot_stat.st_ino, snapshot_stat.st_mtime_ns)
            or (current_ledger.st_dev, current_ledger.st_ino, current_ledger.st_mtime_ns)
            != (ledger_stat.st_dev, ledger_stat.st_ino, ledger_stat.st_mtime_ns)
        ):
            raise GovernanceError("Snapshot cleanup path was replaced.")
        ledger.unlink()
    except OSError as error:
        raise GovernanceError("Snapshot ledger cleanup failed.") from error


@contextmanager
def _snapshot_file() -> Any:
    with tempfile.TemporaryDirectory() as directory:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", suffix=".json", dir=directory
        ) as source:
            try:
                yield source
            finally:
                _cleanup_snapshot_ledger(source.name)


class GovernanceError(RuntimeError):
    pass


class NoPostGovernanceError(GovernanceError):
    """Fail a PR without adding another terminal status to an ambiguous latch."""


@dataclass(frozen=True)
class DispatcherSource:
    identifier: int
    event: str
    attempt: int


@dataclass(frozen=True)
class DispatcherGeneration:
    """A dispatcher run's immutable ordering and identity boundary."""

    identifier: int
    created_at: datetime
    event: str
    workflow_id: int
    status: str
    conclusion: str | None


def read_environment(*, default_token: bool = False) -> dict[str, str]:
    """The verifier and every GET see only the read token, never App secrets."""
    token = os.environ.get("DEFAULT_READ_TOKEN" if default_token else "GH_TOKEN", "")
    if not token:
        raise GovernanceError("Read token is missing.")
    return {"GH_TOKEN": token, "PATH": os.environ["PATH"]}


def command(
    arguments: list[str], *, check_write: bool = False, default_token: bool = False,
    deadline: float | None = None,
) -> str:
    """Run one bounded GitHub API request without allowing a stalled child."""
    environment = os.environ.copy()
    if check_write:
        token = environment.get("CHECK_WRITE_TOKEN", "")
        if not token:
            raise GovernanceError("Check writer token is missing.")
        # Do not permit the read token to cross the write boundary.
        environment = {"GH_TOKEN": token, "PATH": environment["PATH"]}
    else:
        environment = read_environment(default_token=default_token)
    deadlines = [value for value in (deadline, _active_initial_evidence_deadline, _terminal_deadline_monotonic) if value is not None]
    effective_deadline = min(deadlines) if deadlines else None
    timeout = GITHUB_API_TIMEOUT_SECONDS
    if effective_deadline is not None:
        remaining = effective_deadline - time.monotonic()
        if remaining <= 0:
            raise GovernanceError("Governance terminal deadline exceeded.")
        timeout = min(timeout, remaining)
    try:
        result = subprocess.run(
            ["gh", "api", *arguments], capture_output=True, text=True,
            check=False, env=environment, timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise GovernanceError("GitHub API request timed out.") from error
    if effective_deadline is not None and time.monotonic() > effective_deadline:
        raise GovernanceError("Governance terminal deadline exceeded.")
    if result.returncode != 0:
        raise GovernanceError("GitHub API request failed.")
    return result.stdout


def api_json(endpoint: str, *, default_token: bool = False) -> Any:
    try:
        return json.loads(command([endpoint], default_token=default_token))
    except json.JSONDecodeError as error:
        raise GovernanceError("GitHub API response is not JSON.") from error


def _page_endpoint(endpoint: str, page: int) -> str:
    if not isinstance(endpoint, str) or not endpoint or "page" in parse_qs(urlparse(endpoint).query) or page < 1:
        raise GovernanceError("GitHub pagination endpoint is invalid.")
    return f"{endpoint}{'&' if '?' in endpoint else '?'}page={page}"


def _included_page(endpoint: str, *, default_token: bool) -> tuple[Any, bool]:
    """Read page six with headers, so a hidden seventh page is rejected."""
    raw = command(["--include", endpoint], default_token=default_token)
    normalized = raw.replace("\r\n", "\n")
    headers, separator, body = normalized.partition("\n\n")
    if not separator or not headers.startswith("HTTP/"):
        raise GovernanceError("GitHub pagination headers are invalid.")
    try:
        value = json.loads(body)
    except json.JSONDecodeError as error:
        raise GovernanceError("GitHub pagination response is not JSON.") from error
    return value, re.search(r"(?im)^link:.*rel=\"next\"", headers) is not None


def pages(endpoint: str, *, default_token: bool = False) -> list[list[dict[str, Any]]]:
    """Read at most six list pages and fence the complete first-page value."""
    values: list[list[dict[str, Any]]] = []
    for page_number in range(1, MAX_SHARED_SNAPSHOT_PAGES + 1):
        page_endpoint = _page_endpoint(endpoint, page_number)
        if page_number == MAX_SHARED_SNAPSHOT_PAGES:
            value, has_next = _included_page(page_endpoint, default_token=default_token)
            if has_next:
                raise GovernanceError("GitHub pagination exceeds the fixed page window.")
        else:
            value = api_json(page_endpoint, default_token=default_token)
        if not isinstance(value, list) or len(value) > MAX_EVIDENCE_RUNS_PER_PAGE or not all(isinstance(item, dict) for item in value):
            raise GovernanceError("GitHub pagination response is invalid.")
        values.append(value)
        if len(value) < MAX_EVIDENCE_RUNS_PER_PAGE or page_number == MAX_SHARED_SNAPSHOT_PAGES:
            break
    else:
        raise GovernanceError("GitHub pagination exceeds the fixed page window.")
    if api_json(_page_endpoint(endpoint, 1), default_token=default_token) != values[0]:
        raise GovernanceError("GitHub pagination first page changed.")
    return values


def object_pages(endpoint: str, *, default_token: bool = False) -> list[dict[str, Any]]:
    """Read bounded Check Run object pages and fence their first page exactly."""
    values: list[dict[str, Any]] = []
    for page_number in range(1, MAX_SHARED_SNAPSHOT_PAGES + 1):
        page_endpoint = _page_endpoint(endpoint, page_number)
        if page_number == MAX_SHARED_SNAPSHOT_PAGES:
            value, has_next = _included_page(page_endpoint, default_token=default_token)
            if has_next:
                raise GovernanceError("GitHub pagination exceeds the fixed page window.")
        else:
            value = api_json(page_endpoint, default_token=default_token)
        runs = value.get("check_runs") if isinstance(value, dict) else None
        if not isinstance(runs, list) or len(runs) > MAX_EVIDENCE_RUNS_PER_PAGE or not all(isinstance(item, dict) for item in runs):
            raise GovernanceError("GitHub pagination response is invalid.")
        values.append(value)
        if len(runs) < MAX_EVIDENCE_RUNS_PER_PAGE or page_number == MAX_SHARED_SNAPSHOT_PAGES:
            break
    else:
        raise GovernanceError("GitHub pagination exceeds the fixed page window.")
    if api_json(_page_endpoint(endpoint, 1), default_token=default_token) != values[0]:
        raise GovernanceError("GitHub pagination first page changed.")
    return values


def object_page(endpoint: str, *, default_token: bool = False) -> dict[str, Any]:
    """Read one API object page when the fence has a strict request budget."""
    value = api_json(endpoint, default_token=default_token)
    if not isinstance(value, dict):
        raise GovernanceError("GitHub object response is invalid.")
    return value


def open_pulls() -> list[int]:
    numbers: list[int] = []
    seen: set[int] = set()
    for page in pages(f"repos/{REPOSITORY}/pulls?state=open&per_page=100"):
        for pull in page:
            number = pull.get("number")
            if type(number) is not int or number < 1 or number in seen or pull.get("state") != "open":
                raise GovernanceError("Open pull request response is invalid.")
            seen.add(number)
            numbers.append(number)
    return numbers


@dataclass(frozen=True)
class OpenSnapshot:
    numbers: tuple[int, ...]
    claimants: dict[str, frozenset[int]]
    pull_requests: tuple[dict[str, object], ...]


def open_snapshot() -> OpenSnapshot:
    """Take one complete O(N) open-PR snapshot for a serialized writer run."""
    numbers: list[int] = []
    # Validate the complete API stream before applying the local-governance
    # scope.  A fork must not become an Issue claimant, but it also must not
    # hide a malformed duplicate response on a later page.
    seen_all: set[int] = set()
    governed_heads: set[str] = set()
    claimants: dict[str, set[int]] = {}
    pull_requests: list[dict[str, object]] = []
    for page in pages(f"repos/{REPOSITORY}/pulls?state=open&per_page=100"):
        for item in page:
            number = item.get("number")
            body = item.get("body")
            base = item.get("base") if isinstance(item, dict) else None
            head = item.get("head") if isinstance(item, dict) else None
            base_repository = base.get("repo") if isinstance(base, dict) else None
            head_repository = head.get("repo") if isinstance(head, dict) else None
            head_sha = head.get("sha") if isinstance(head, dict) else None
            draft = item.get("draft") if isinstance(item, dict) else None
            # GitHub represents an absent PR description as JSON null.  It is
            # an invalid closer for that individual PR, not a malformed
            # repository-wide snapshot which would strand every other head.
            if body is None:
                body = ""
            if type(number) is not int or number < 1 or number in seen_all or item.get("state") != "open" or not isinstance(body, str) or not isinstance(draft, bool):
                raise GovernanceError("Open pull request snapshot is invalid.")
            seen_all.add(number)
            # Forks and non-default-base PRs cannot be governed by this
            # default-branch App; do not let them claim a local canonical Issue.
            if (
                not isinstance(base_repository, dict) or not isinstance(head_repository, dict)
                or base_repository.get("full_name") != REPOSITORY or head_repository.get("full_name") != REPOSITORY
                or base.get("ref") != os.environ.get("GITHUB_REF_NAME") or not isinstance(head_sha, str) or not SHA.fullmatch(head_sha)
            ):
                continue
            normalized_head = head_sha.lower()
            if normalized_head in governed_heads:
                raise GovernanceError("Open pull request snapshot has duplicate governed head SHA.")
            governed_heads.add(normalized_head)
            numbers.append(number)
            pull_requests.append({"number": number, "isDraft": draft, "body": body, "head_sha": head_sha})
            for issue in closing_issues(body):
                claimants.setdefault(issue, set()).add(number)
    return OpenSnapshot(tuple(numbers), {issue: frozenset(values) for issue, values in claimants.items()}, tuple(pull_requests))


def pull(number: int, *, default_token: bool = False) -> dict[str, Any]:
    """Read one governed PR using the least-privileged read boundary.

    The initial decision can use the installation read token.  Only the
    terminal closer fence needs the repository workflow token: that isolates
    the small final-CAS budget from the 300-PR verifier/evidence scan.
    """
    value = api_json(f"repos/{REPOSITORY}/pulls/{number}", default_token=default_token)
    base = value.get("base") if isinstance(value, dict) else None
    head = value.get("head") if isinstance(value, dict) else None
    base_repository = base.get("repo") if isinstance(base, dict) else None
    head_repository = head.get("repo") if isinstance(head, dict) else None
    if (
        not isinstance(value, dict) or value.get("number") != number or value.get("state") != "open"
        or type(value.get("draft")) is not bool or not isinstance(base, dict) or not isinstance(head, dict)
        or not isinstance(base.get("sha"), str) or not isinstance(head.get("sha"), str)
        or base.get("ref") != os.environ.get("GITHUB_REF_NAME")
        or not SHA.fullmatch(base["sha"]) or not SHA.fullmatch(head["sha"])
        or not isinstance(base_repository, dict) or not isinstance(head_repository, dict)
        or base_repository.get("full_name") != REPOSITORY or head_repository.get("full_name") != REPOSITORY
    ):
        raise GovernanceError("Pull request is invalid.")
    return value


def canonical_issue(body: object) -> str | None:
    if not isinstance(body, str):
        raise GovernanceError("Pull request body is invalid.")
    issues = closing_issues(body)
    if len(issues) != 1:
        return None
    return next(iter(issues))


def pr_body_sha256(body: object) -> str:
    """Bind a decision to the exact valid UTF-8 PR description bytes."""
    if not isinstance(body, str) or "\0" in body:
        raise GovernanceError("Pull request body is invalid.")
    try:
        encoded = body.encode("utf-8", "strict")
    except UnicodeEncodeError as error:
        raise GovernanceError("Pull request body is invalid.") from error
    return hashlib.sha256(encoded).hexdigest()


def closing_issues(body: str) -> set[str]:
    """Use the push/PR contract's canonical same-repository closer parser."""
    try:
        return {str(number) for number in issue_contract.closing_issue_numbers(body, REPOSITORY)}
    except issue_contract.ContractViolation as error:
        raise GovernanceError("Pull request body is invalid.") from error


def workflow_path_matches(value: object, expected: str) -> bool:
    """Accept GitHub's documented ``path@ref`` representation safely."""
    if value == expected:
        return True
    if not isinstance(value, str) or not value.startswith(expected + "@"):
        return False
    ref = value[len(expected) + 1:]
    return bool(
        re.fullmatch(r"[A-Za-z0-9._/-]+", ref)
        and ref not in {".", ".."}
        and not ref.startswith("/")
        and "//" not in ref
        and all(part not in {"", ".", ".."} for part in ref.split("/"))
    )


def default_branch_name() -> str:
    """Return the trusted default branch name from the default-branch workflow."""
    branch = os.environ.get("GITHUB_REF_NAME", "")
    if (
        not re.fullmatch(r"[A-Za-z0-9._/-]+", branch)
        or branch in {".", ".."} or branch.startswith("/") or "//" in branch
        or any(part in {"", ".", ".."} for part in branch.split("/"))
    ):
        raise GovernanceError("Dispatcher default branch is invalid.")
    return branch


def workflow_path_is_default(value: object, expected: str) -> bool:
    """Require an Actions run to have executed the exact default workflow ref."""
    branch = default_branch_name()
    return value in {
        expected,
        f"{expected}@{branch}",
        f"{expected}@refs/heads/{branch}",
    }


def repository_rest_identity(
    value: object, repository: str,
) -> tuple[int, str, str] | object:
    """Validate the immutable REST repository identity for an Actions run."""
    if not isinstance(value, Mapping) or repository.count("/") != 1:
        return _INVALID_REPOSITORY_IDENTITY
    repository_name = repository.rsplit("/", 1)[1]
    canonical_url = f"https://api.github.com/repos/{repository}"
    identifier = value.get("id")
    name = value.get("name")
    url = value.get("url")
    if (
        type(identifier) is not int or identifier < 1
        or name != repository_name or url != canonical_url
        or ("full_name" in value and value.get("full_name") != repository)
    ):
        return _INVALID_REPOSITORY_IDENTITY
    return identifier, repository_name, canonical_url


def repository_boundary_matches(
    repository_value: object, nested_values: Sequence[object], repository: str,
) -> bool:
    """Bind every PR repository boundary to its Actions run identity."""
    run_identity = repository_rest_identity(repository_value, repository)
    if run_identity is _INVALID_REPOSITORY_IDENTITY:
        return False
    nested_identities = [
        repository_rest_identity(value, repository) for value in nested_values
    ]
    if any(identity is _INVALID_REPOSITORY_IDENTITY for identity in nested_identities):
        return False
    return all(identity == run_identity for identity in nested_identities)


def dispatcher_created_at(value: object) -> datetime:
    """Parse the API's canonical UTC timestamp without accepting local ambiguity."""
    if not isinstance(value, str) or re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", value) is None:
        raise GovernanceError("Dispatcher generation timestamp is invalid.")
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as error:
        raise GovernanceError("Dispatcher generation timestamp is invalid.") from error


def dispatcher_generation(value: object, *, expected_identifier: int | None = None, require_success: bool = False) -> DispatcherGeneration:
    """Validate one default-branch dispatcher generation completely."""
    expected_head = os.environ.get("GITHUB_SHA", "")
    branch = default_branch_name()
    repository = value.get("repository") if isinstance(value, dict) else None
    identifier = value.get("id") if isinstance(value, dict) else None
    workflow_id = value.get("workflow_id") if isinstance(value, dict) else None
    attempt = value.get("run_attempt") if isinstance(value, dict) else None
    status = value.get("status") if isinstance(value, dict) else None
    conclusion = value.get("conclusion") if isinstance(value, dict) else None
    if not (
        isinstance(value, dict) and SHA.fullmatch(expected_head) is not None
        and type(identifier) is int and identifier > 0
        and (expected_identifier is None or identifier == expected_identifier)
        and type(workflow_id) is int and workflow_id > 0
        and value.get("name") == DISPATCHER_NAME
        and workflow_path_is_default(value.get("path"), DISPATCHER_PATH)
        and value.get("event") in DISPATCHER_EVENTS and value.get("head_branch") == branch
        and value.get("head_sha") == expected_head
        and repository_rest_identity(repository, REPOSITORY) is not _INVALID_REPOSITORY_IDENTITY
        and type(value.get("run_number")) is int and value["run_number"] > 0
        and type(attempt) is int and attempt == 1
        and isinstance(status, str) and status in DISPATCHER_ACTIVE_STATUSES | {"completed"}
        and (status not in DISPATCHER_ACTIVE_STATUSES or conclusion is None)
        and (status != "completed" or conclusion in DISPATCHER_TERMINAL_CONCLUSIONS)
        and (not require_success or status != "completed" or conclusion == "success")
    ):
        raise GovernanceError("Dispatcher source is not a trusted default-branch run.")
    return DispatcherGeneration(
        identifier,
        dispatcher_created_at(value.get("created_at")),
        value["event"],
        workflow_id,
        status,
        conclusion,
    )


def dispatcher_generations(
    workflow_id: int, not_before: datetime, *, default_token: bool = False,
) -> dict[int, DispatcherGeneration]:
    """Read bounded, exact-workflow generations no older than the source."""
    if type(workflow_id) is not int or workflow_id < 1:
        raise GovernanceError("Dispatcher workflow ID is invalid.")
    if not isinstance(not_before, datetime) or not_before.tzinfo is None:
        raise GovernanceError("Dispatcher generation lower bound is invalid.")
    branch = default_branch_name()
    lower_bound = not_before.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    page = object_page(
        f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?"
        + urlencode({"branch": branch, "created": ">=" + lower_bound, "per_page": 100}),
        default_token=default_token,
    )
    runs = page.get("workflow_runs")
    total_count = page.get("total_count")
    if (
        not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs)
        or type(total_count) is not int or total_count != len(runs)
        or total_count >= MAX_DISPATCHER_FENCE_RUNS
    ):
        # One page is intentional: reaching capacity leaves a later generation
        # unobservable, so the writer must not publish a terminal result.
        raise GovernanceError("Dispatcher generation fence exceeds its bounded API window.")
    generations: dict[int, DispatcherGeneration] = {}
    for run in runs:
        if run.get("name") != DISPATCHER_NAME:
            continue
        generation = dispatcher_generation(run)
        if generation.workflow_id != workflow_id:
            raise GovernanceError("Dispatcher workflow ID differs from the selected source.")
        if generation.created_at < not_before:
            raise GovernanceError("Dispatcher generation predates the requested lower bound.")
        if generation.identifier in generations:
            raise GovernanceError("Dispatcher generation is duplicated within the bounded page.")
        generations[generation.identifier] = generation
    return generations


def dispatcher_generation_is_newer(
    candidate: DispatcherGeneration, current: DispatcherGeneration,
) -> bool:
    """Compare only the documented immutable generation ordering fields."""
    return (candidate.created_at, candidate.identifier) > (
        current.created_at,
        current.identifier,
    )


def dispatcher_generation_reconciles(generation: DispatcherGeneration) -> bool:
    """Return whether a trusted generation can preempt writer ordering.

    A ``workflow_run`` fork no-op、明示的に記録された
    ``pull_request_target`` fork no-op、および Issue 系 prelock no-op は、
    検証済みの完了形だけ reconciliation を無効にする。これらは local
    writer に対する権限を持たない。証跡が欠落または不正な場合は
    reconciliation 対象として fail-closed の fence を維持する。
    """
    if generation.event not in {"workflow_run", "pull_request_target", "issues", "issue_comment"}:
        return True
    # 失敗中・実行中の Issue 系世代はそれ自体が有効な fence なので、
    # 追加の step 証跡を読むのは prelock 成功経路に限定する。
    if (
        generation.event in {"issues", "issue_comment"}
        and (generation.status != "completed" or generation.conclusion != "success")
    ):
        return True
    cached = _nonreconciling_dispatcher_generations.get(generation.identifier)
    if cached is not None:
        if cached != generation:
            raise GovernanceError("Cached dispatcher no-op generation changed.")
        return False
    page = object_page(
        f"repos/{REPOSITORY}/actions/runs/{generation.identifier}/jobs?per_page=100",
    )
    jobs = page.get("jobs")
    total_count = page.get("total_count")
    if (
        not isinstance(jobs, list) or not all(isinstance(job, dict) for job in jobs)
        or type(total_count) is not int or total_count != len(jobs)
        or total_count >= MAX_DISPATCHER_FENCE_RUNS
    ):
        raise GovernanceError("Dispatcher reconciliation evidence is invalid.")

    def named_job(name: str) -> dict[str, Any]:
        matches = [job for job in jobs if job.get("name") == name]
        if len(matches) != 1:
            raise GovernanceError("Dispatcher reconciliation evidence is ambiguous.")
        job = matches[0]
        status = job.get("status")
        conclusion = job.get("conclusion")
        if not (
            type(job.get("id")) is int and job["id"] > 0
            and isinstance(status, str) and status in {"queued", "in_progress", "completed"}
            and (status != "completed" or conclusion in DISPATCHER_TERMINAL_CONCLUSIONS)
            and (status == "completed" or conclusion is None)
        ):
            raise GovernanceError("Dispatcher reconciliation job is invalid.")
        return job

    preflight = named_job(PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME)
    barrier = named_job(RESOLVER_FAILURE_BARRIER_NAME)
    if barrier["status"] != "completed" or barrier["conclusion"] != "skipped":
        # The resolver barrier is enabled only for reconcile=true.  A queued,
        # running, failed, or cancelled barrier is still a newer generation
        # that must retain the fail-closed preemption fence.
        return True
    if not (
        generation.status == "completed" and generation.conclusion == "success"
        and preflight["status"] == "completed" and preflight["conclusion"] == "success"
    ):
        raise GovernanceError("Dispatcher no-op reconciliation evidence is incomplete.")
    if generation.event in {"pull_request_target", "issues", "issue_comment"}:
        step_name = (
            PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME
            if generation.event == "pull_request_target" else PREFLIGHT_ISSUE_NOOP_STEP_NAME
        )
        steps = preflight.get("steps")
        matches = (
            [step for step in steps if step.get("name") == step_name]
            if isinstance(steps, list) and all(isinstance(step, dict) for step in steps) else []
        )
        if len(matches) != 1:
            return True
        step = matches[0]
        if not (
            type(step.get("number")) is int and step["number"] > 0
            and step.get("status") == "completed" and step.get("conclusion") == "success"
        ):
            return True
    _nonreconciling_dispatcher_generations[generation.identifier] = generation
    return False


def trusted_dispatcher_source(identifier: int) -> DispatcherSource:
    """Bind dispatch input to one immutable default-branch dispatcher run."""
    if type(identifier) is not int or identifier < 1:
        raise GovernanceError("Dispatcher run ID is invalid.")
    value = api_json(f"repos/{REPOSITORY}/actions/runs/{identifier}", default_token=True)
    generation = dispatcher_generation(value, expected_identifier=identifier, require_success=True)
    attempt = value.get("run_attempt") if isinstance(value, dict) else None
    if type(attempt) is not int:
        raise GovernanceError("Dispatcher source attempt is invalid.")
    return DispatcherSource(generation.identifier, generation.event, attempt)


def rebind_trusted_default_writer() -> None:
    """Refuse a terminal Check Run write after the trusted default moved."""
    expected_head = os.environ.get("GITHUB_SHA", "")
    if not SHA.fullmatch(expected_head):
        raise GovernanceError("Writer default-branch SHA is invalid.")
    expected_branch = default_branch_name()
    # bootstrap-validation already bound the writer workflow bytes to the
    # trusted default ref before the environment exposed any App credential.
    # The terminal fence only needs to prove that ref is still this exact SHA.
    repository = api_json(f"repos/{REPOSITORY}")
    branch = repository.get("default_branch") if isinstance(repository, dict) else None
    if branch != expected_branch:
        raise GovernanceError("Repository default branch is invalid.")
    reference = api_json(f"repos/{REPOSITORY}/git/ref/heads/{branch}")
    current_head = reference.get("object", {}).get("sha") if isinstance(reference, dict) and isinstance(reference.get("object"), dict) else None
    if current_head != expected_head:
        raise GovernanceError("Trusted default branch advanced while governance was running.")


def trusted_workflow_blob(path: str, base: str, head: str, cache: dict[tuple[str, str], str] | None = None) -> None:
    """Require the source workflow bytes to equal base, PR head, and writer."""
    default_ref = os.environ.get("GITHUB_SHA", "")
    if not SHA.fullmatch(default_ref):
        raise GovernanceError("Writer default-branch SHA is invalid.")
    digests: list[str] = []
    for ref in (default_ref, base, head):
        cache_key = (path, ref)
        digest = cache.get(cache_key) if cache is not None else None
        if digest is None:
            blob = api_json(f"repos/{REPOSITORY}/contents/{path}?ref={ref}")
            digest = blob.get("sha") if isinstance(blob, dict) else None
            if not isinstance(digest, str) or not SHA.fullmatch(digest):
                raise GovernanceError("Default-branch workflow blob is invalid.")
            if cache is not None:
                cache[cache_key] = digest
        digests.append(digest)
    if len(set(digests)) != 1:
        raise GovernanceError("Workflow differs from the trusted default branch.")


def check_external_id(head: str) -> str:
    if not SHA.fullmatch(head):
        raise GovernanceError("Check Run head SHA is invalid.")
    scope = os.environ.get("GOVERNANCE_SCOPE", "")
    dispatcher = os.environ.get("GOVERNANCE_DISPATCHER_RUN_ID", "")
    if scope == "all" and NUMBER.fullmatch(dispatcher):
        generation = f"dispatcher-{dispatcher}"
    elif scope == "early" and NUMBER.fullmatch(WRITER_RUN_ID):
        generation = f"writer-{WRITER_RUN_ID}"
    else:
        # Unit-level helpers retain a stable synthetic generation; production
        # main rejects a missing/invalid scope before any network access.
        generation = "unit"
    return CHECK_EXTERNAL_PREFIX + head.lower() + "/" + generation


def check_app_id() -> int:
    value = os.environ.get("KRR_GOVERNANCE_CHECK_APP_ID", "")
    if not NUMBER.fullmatch(value):
        raise GovernanceError("Check Run App ID is invalid.")
    return int(value)


def _valid_check(value: object, head: str, *, external_id: str | None = None) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise GovernanceError("Check Run response is invalid.")
    app = value.get("app")
    if (
        type(value.get("id")) is not int or value.get("name") != CHECK_NAME
        or value.get("head_sha") != head or value.get("external_id") != (external_id if external_id is not None else check_external_id(head))
        or not isinstance(app, dict) or app.get("id") != check_app_id()
        or not isinstance(value.get("updated_at"), str)
    ):
        raise GovernanceError("Check Run identity is invalid.")
    return value


def checks(head: str, *, default_token: bool = False) -> list[dict[str, Any]]:
    values: list[dict[str, Any]] = []
    query = urlencode({"check_name": CHECK_NAME, "app_id": check_app_id(), "filter": "all", "per_page": 100})
    for page in object_pages(f"repos/{REPOSITORY}/commits/{head}/check-runs?{query}"):
        runs = page.get("check_runs")
        if not isinstance(runs, list) or not all(isinstance(item, dict) for item in runs):
            raise GovernanceError("Check Run pagination is invalid.")
        values.extend(runs)
    return values


def reject_newer_dispatcher_barrier(head: str) -> None:
    """Stop an older writer before a later dispatcher generation can be lost.

    The Actions run is the immutable generation order.  Check Runs are merely
    its visible fence and have no compare-and-swap update, so a newly queued
    dispatcher must block an older writer even before its pending Check Run is
    created.
    """

    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    if not SHA.fullmatch(head):
        raise NoPostGovernanceError("Check Run head is invalid for dispatcher fence.")
    current_value = os.environ.get("GOVERNANCE_DISPATCHER_RUN_ID", "")
    if not NUMBER.fullmatch(current_value):
        raise NoPostGovernanceError("Writer dispatcher source is invalid for dispatcher fence.")
    try:
        current_value_response = api_json(
            f"repos/{REPOSITORY}/actions/runs/{current_value}",
        )
        current_generation = dispatcher_generation(
            current_value_response,
            expected_identifier=int(current_value),
            require_success=True,
        )
        generations = dispatcher_generations(
            current_generation.workflow_id, current_generation.created_at,
        )
        snapshot_current = generations.get(current_generation.identifier)
        if snapshot_current != current_generation:
            raise GovernanceError("Current dispatcher generation is absent or changed in the paginated snapshot.")
        for candidate_generation in generations.values():
            if (
                dispatcher_generation_is_newer(candidate_generation, current_generation)
                and dispatcher_generation_reconciles(candidate_generation)
            ):
                raise NoPostGovernanceError("A newer dispatcher generation owns this Check Run head.")
    except GovernanceError as error:
        raise NoPostGovernanceError("Dispatcher barrier evidence is invalid.") from error


def check_run_for_external_id(head: str, external_id: str) -> dict[str, Any] | None:
    """Read exactly one immutable App Check Run generation, or fail closed."""
    if not isinstance(external_id, str) or not external_id:
        raise GovernanceError("Check Run generation external ID is invalid.")
    bound = _bound_check_runs.get((head, external_id))
    if bound is not None:
        value = api_json(f"repos/{REPOSITORY}/check-runs/{bound}")
        if not isinstance(value, dict) or value.get("id") != bound:
            raise GovernanceError("Bound Check Run ID changed.")
        return _valid_check(value, head, external_id=external_id)
    matching: list[dict[str, Any]] = []
    for item in checks(head):
        if item.get("name") != CHECK_NAME:
            continue
        app = item.get("app")
        # GitHub may return same-name checks from other Apps even with app_id
        # filtering. They are not governance candidates and cannot DoS ours.
        if not isinstance(app, dict) or app.get("id") != check_app_id():
            continue
        if item.get("head_sha") != head:
            raise GovernanceError("Check Run head mismatch.")
        if item.get("external_id") == external_id:
            matching.append(_valid_check(item, head, external_id=external_id))
    if len(matching) > 1:
        raise GovernanceError("Multiple trusted Check Runs exist for one immutable generation.")
    return matching[0] if matching else None


def check_run(head: str) -> dict[str, Any] | None:
    return check_run_for_external_id(head, check_external_id(head))


def pace_check_write() -> None:
    """Pace Check Run mutations within the shared installation REST envelope."""
    global _last_check_write_at
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    interval = (
        ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS
        if os.environ.get("GOVERNANCE_SCOPE") == "all"
        else CHECK_WRITE_INTERVAL_SECONDS
    )
    if _last_check_write_at is None:
        # installation tokenを再発行してもREST limitは共有されるため、all
        # segmentはdispatcher/read overhead込みのより長い間隔で開始する。
        time.sleep(interval)
        _last_check_write_at = time.monotonic()
        return
    now = time.monotonic()
    delay = interval - (now - _last_check_write_at)
    if delay > 0:
        time.sleep(delay)
        now = time.monotonic()
    _last_check_write_at = now


def write_check(
    head: str,
    *,
    state: str,
    description: str,
    details_url: str,
    existing: dict[str, Any] | None = None,
    expected_fingerprint: tuple[object, ...] | None = None,
) -> dict[str, Any]:
    if state not in {"in_progress", "success", "failure"}:
        raise GovernanceError("Check Run state is invalid.")
    early_pending = (
        state == "in_progress"
        and os.environ.get("GOVERNANCE_SCOPE") == "early"
    )
    if expected_fingerprint is not None:
        current = check_run(head)
        if check_fingerprint(current) != expected_fingerprint:
            raise NoPostGovernanceError("Check Run changed before terminal write.")
        existing = current
    if existing is None:
        # A production all-open writer is bound to the exact IDs returned by
        # the invalidator POSTs.  Falling back to a new same-name generation
        # would turn a missing/changed manifest entry into an unprotected
        # terminal decision.
        if os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("GOVERNANCE_SCOPE") == "all":
            raise NoPostGovernanceError("Bound all-open Check Run is missing.")
        arguments = ["--method", "POST", f"repos/{REPOSITORY}/check-runs", "-f", f"name={CHECK_NAME}", "-f", f"head_sha={head}", "-f", f"external_id={check_external_id(head)}", "-f", "status=in_progress", "-f", f"details_url={details_url}", "-f", f"output[title]={CHECK_NAME}", "-f", f"output[summary]={description}"]
    else:
        identifier = _valid_check(existing, head)["id"]
        arguments = ["--method", "PATCH", f"repos/{REPOSITORY}/check-runs/{identifier}", "-f", f"details_url={details_url}", "-f", f"output[title]={CHECK_NAME}", "-f", f"output[summary]={description}"]
        if state == "in_progress":
            arguments.extend(["-f", "status=in_progress"])
        else:
            arguments.extend(["-f", "status=completed", "-f", f"conclusion={state}"])
    # Check Runs provide no compare-and-swap PATCH.  A priority writer first
    # acquires the workflow singleton with cancellation enabled; this final
    # read is the matching writer-side fence, so a cancelled older generation
    # cannot publish a terminal value after that hand-off.
    if state != "in_progress":
        ensure_writer_run_is_active()
        if os.environ.get("GITHUB_ACTIONS") == "true":
            source = os.environ.get("GOVERNANCE_DISPATCHER_RUN_ID", "")
            if not NUMBER.fullmatch(source):
                raise NoPostGovernanceError("Writer dispatcher source is invalid.")
    pace_check_write()
    # This read must be adjacent to the mutation. A newer dispatcher may have
    # arrived while pacing or while the old writer revalidated its source.
    if early_pending or state != "in_progress":
        reject_newer_dispatcher_barrier(head)
    try:
        value = json.loads(command(arguments, check_write=True))
    except json.JSONDecodeError as error:
        raise GovernanceError("Check Run write response is not JSON.") from error
    checked = _valid_check(value, head)
    _bound_check_runs[(head, checked["external_id"])] = checked["id"]
    expected_status = "in_progress" if state == "in_progress" else "completed"
    expected_conclusion = None if state == "in_progress" else state
    if checked.get("status") != expected_status or checked.get("conclusion") != expected_conclusion or checked.get("details_url") != details_url:
        raise GovernanceError("Check Run write state is invalid.")
    reread = check_run(head)
    if reread is None or check_fingerprint(reread) != check_fingerprint(checked):
        raise GovernanceError("Check Run changed after write.")
    # Leave an already-created old early generation pending when the new
    # barrier appeared between its POST and this exact-generation reread.
    if early_pending:
        reject_newer_dispatcher_barrier(head)
    return reread


def ensure_writer_run_is_active() -> None:
    """Reject a terminal mutation once this workflow generation was cancelled."""
    # Unit callers are not Actions generations.  Production always supplies
    # this marker, and must prove the current default-branch workflow run.
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    expected_head = os.environ.get("GITHUB_SHA", "")
    if not NUMBER.fullmatch(WRITER_RUN_ID) or not SHA.fullmatch(expected_head):
        raise NoPostGovernanceError("Writer generation identity is invalid.")
    value = api_json(f"repos/{REPOSITORY}/actions/runs/{WRITER_RUN_ID}")
    repository = value.get("repository") if isinstance(value, dict) else None
    if not (
        isinstance(value, dict) and value.get("id") == int(WRITER_RUN_ID)
        and value.get("name") == "PR governance status writer"
        and workflow_path_matches(value.get("path"), WRITER_WORKFLOW_PATH)
        and value.get("event") == "workflow_dispatch" and value.get("head_sha") == expected_head
        and repository_rest_identity(repository, REPOSITORY) is not _INVALID_REPOSITORY_IDENTITY
        and value.get("status") == "in_progress" and type(value.get("run_attempt")) is int and value["run_attempt"] == 1
    ):
        raise NoPostGovernanceError("Writer generation is no longer active.")


def _same_check_evidence(current: str, desired: str) -> bool:
    """Compare immutable Check Run evidence, ignoring the writer run URL."""
    current_query = parse_qs(urlparse(current).query, keep_blank_values=True)
    desired_query = parse_qs(urlparse(desired).query, keep_blank_values=True)
    required = {
        "source_run_id", "ci_workflow_id", "ci_run_id", "ci_run_number",
        "ci_run_attempt", "ci_status", "ci_conclusion", "release_workflow_id",
        "release_run_id", "release_run_number", "release_run_attempt",
        "release_status", "release_conclusion", "pr_base_sha", "pr_head_sha",
        "pr_body_sha256",
    }
    current_digest = current_query.get("pr_body_sha256")
    desired_digest = desired_query.get("pr_body_sha256")
    if current_digest is None and desired_digest is None:
        return current_query == desired_query
    for digest in (current_digest, desired_digest):
        if digest is None or len(digest) != 1 or BODY_SHA256.fullmatch(digest[0]) is None:
            return False
    return {key: current_query.get(key) for key in required} == {key: desired_query.get(key) for key in required}


def check_fingerprint(value: dict[str, Any] | None) -> tuple[object, ...]:
    if value is None:
        return ()
    checked = _valid_check(value, value.get("head_sha", ""))
    return (checked["id"], checked["updated_at"], checked.get("status"), checked.get("conclusion"), checked.get("details_url"), checked["external_id"])


def write_governance_check(
    head: str,
    state: str,
    description: str,
    target_url: str,
    *,
    expected_fingerprint: tuple[object, ...] | None = None,
) -> tuple[object, ...]:
    """Compatibility entry point backed exclusively by one external Check Run."""
    mapped = {"pending": "in_progress", "success": "success", "failure": "failure"}.get(state)
    if mapped is None:
        raise GovernanceError("Governance Check Run state is invalid.")
    existing = check_run(head)
    if expected_fingerprint is not None and check_fingerprint(existing) != expected_fingerprint:
        raise NoPostGovernanceError("Check Run changed before governance write.")
    if existing is None and mapped != "in_progress":
        # Check Runs cannot be created directly in a completed state.
        existing = write_check(
            head,
            state="in_progress",
            description="Trusted governance revalidation is running.",
            details_url=target_url,
            expected_fingerprint=expected_fingerprint,
        )
        expected_fingerprint = check_fingerprint(existing)
    value = write_check(
        head,
        state=mapped,
        description=description,
        details_url=target_url,
        existing=existing,
        expected_fingerprint=expected_fingerprint,
    )
    return check_fingerprint(value)


def check_baseline(head: str) -> tuple[object, ...]:
    return check_fingerprint(check_run(head))


def check_changed_since(head: str, baseline: tuple[object, ...]) -> bool:
    return check_fingerprint(check_run(head)) != baseline


def sensor_terminal_check_count(head: str, sensor_id: int) -> int:
    value = check_run(head)
    if value is None:
        return 0
    details = value.get("details_url")
    source = parse_qs(urlparse(details).query).get("source_run_id") if isinstance(details, str) else None
    return int(value.get("status") == "completed" and source == [str(sensor_id)])


def check_fence(head: str, baseline: tuple[object, ...], sensor_id: int, *, desired_state: str | None = None, desired_target: str | None = None) -> tuple[bool, int, bool]:
    value = check_run(head)
    if value is None:
        return baseline != (), 0, False
    newer = check_fingerprint(value) != baseline
    details = value.get("details_url")
    expected_status = "in_progress" if desired_state == "pending" else "completed"
    expected_conclusion = None if desired_state == "pending" else desired_state
    exact = bool(desired_state and value.get("status") == expected_status and value.get("conclusion") == expected_conclusion and isinstance(details, str) and desired_target and _same_check_evidence(details, desired_target))
    source = parse_qs(urlparse(details).query).get("source_run_id") if isinstance(details, str) else None
    terminal_count = int(value.get("status") == "completed" and source == [str(sensor_id)])
    return newer, terminal_count, exact


def dispatcher_invalidation_url(source: DispatcherSource, carry_pending: int) -> str:
    if carry_pending not in {0, 1}:
        raise GovernanceError("Dispatcher carry marker is invalid.")
    return (
        f"{SERVER_URL}/{REPOSITORY}/actions/runs/{source.identifier}?"
        + urlencode({"dispatcher_run_id": str(source.identifier), "carry_pending": str(carry_pending)})
    )


def preserved_early_success(value: dict[str, Any], head: str, writer_run_id: int, *, body_sha256: str) -> bool:
    """Accept only the exact early-writer success that this all-pass preserves."""
    details = value.get("details_url")
    if (
        value.get("status") != "completed" or value.get("conclusion") != "success"
        or not isinstance(details, str) or type(writer_run_id) is not int or writer_run_id < 1
    ):
        return False
    expected = urlparse(f"{SERVER_URL}/{REPOSITORY}/actions/runs/{writer_run_id}")
    parsed = urlparse(details)
    if (
        parsed.scheme != expected.scheme or parsed.netloc != expected.netloc
        or parsed.path != expected.path or parsed.params or parsed.fragment
    ):
        return False
    query = parse_qs(parsed.query, keep_blank_values=True)
    required = {
        "source_run_id", "ci_workflow_id", "ci_run_id", "ci_run_number",
        "ci_run_attempt", "ci_status", "ci_conclusion", "release_workflow_id",
        "release_run_id", "release_run_number", "release_run_attempt",
        "release_status", "release_conclusion", "pr_base_sha", "pr_head_sha",
        "pr_body_sha256",
    }
    expected_base = os.environ.get("GITHUB_SHA", "")
    return (
        set(query) == required and query.get("pr_head_sha") == [head]
        and query.get("pr_body_sha256") == [body_sha256]
        and (not expected_base or query.get("pr_base_sha") == [expected_base])
        and query.get("ci_status") == ["completed"] and query.get("ci_conclusion") == ["success"]
        and query.get("release_status") == ["completed"] and query.get("release_conclusion") == ["success"]
        and BODY_SHA256.fullmatch(body_sha256) is not None
        and all(len(query[key]) == 1 for key in required)
        and all(NUMBER.fullmatch(query[key][0]) for key in (
            "source_run_id", "ci_workflow_id", "ci_run_id", "ci_run_number",
            "ci_run_attempt", "release_workflow_id", "release_run_id",
            "release_run_number", "release_run_attempt",
        ))
        and SHA.fullmatch(query["pr_base_sha"][0]) is not None
    )


def prior_writer_url_matches(value: dict[str, Any], writer_run_id: int) -> bool:
    """Require a non-success prefix result to point only to its writer run."""
    details = value.get("details_url")
    if not isinstance(details, str) or type(writer_run_id) is not int or writer_run_id < 1:
        return False
    expected = urlparse(f"{SERVER_URL}/{REPOSITORY}/actions/runs/{writer_run_id}")
    parsed = urlparse(details)
    return (
        parsed.scheme == expected.scheme and parsed.netloc == expected.netloc
        and parsed.path == expected.path and not parsed.params and not parsed.query and not parsed.fragment
    )


def trusted_completed_terminal_writers(
    source: DispatcherSource, continuation_index: int, completed_writer_run_ids: tuple[int, ...],
) -> None:
    """Bind every consumed terminal prefix to one completed App writer run."""
    if continuation_index < 2 or len(completed_writer_run_ids) != continuation_index - 1:
        raise GovernanceError("Completed terminal writer boundary is invalid.")
    bot_login = os.environ.get("KRR_GOVERNANCE_APP_BOT_LOGIN", "")
    expected_head = os.environ.get("GITHUB_SHA", "")
    expected_branch = default_branch_name()
    if not bot_login or SHA.fullmatch(expected_head) is None:
        raise GovernanceError("Completed terminal writer identity is invalid.")
    for segment, identifier in enumerate(completed_writer_run_ids, start=1):
        value = api_json(f"repos/{REPOSITORY}/actions/runs/{identifier}")
        repository = value.get("repository") if isinstance(value, dict) else None
        actor = value.get("actor") if isinstance(value, dict) else None
        triggering_actor = value.get("triggering_actor") if isinstance(value, dict) else None
        if not (
            isinstance(value, dict) and value.get("id") == identifier
            and value.get("name") == "PR governance status writer"
            and workflow_path_is_default(value.get("path"), WRITER_WORKFLOW_PATH)
            and value.get("event") == "workflow_dispatch"
            and value.get("display_title") == f"source={source.identifier} scope=all segment={segment}"
            and value.get("head_branch") == expected_branch and value.get("head_sha") == expected_head
            and repository_rest_identity(repository, REPOSITORY) is not _INVALID_REPOSITORY_IDENTITY
            and type(value.get("run_attempt")) is int and value["run_attempt"] == 1
            and value.get("status") == "completed" and value.get("conclusion") == "success"
            and isinstance(actor, dict) and actor.get("login") == bot_login and actor.get("type") == "Bot"
            and isinstance(triggering_actor, dict) and triggering_actor.get("login") == bot_login
            and triggering_actor.get("type") == "Bot"
        ):
            raise GovernanceError("Completed terminal writer is not a trusted App run.")


def observed_invalidations(
    snapshot: OpenSnapshot, source: DispatcherSource, scope: str, targets: tuple[int, ...],
    preserved: tuple[int, ...] = (), preserved_writer_run_id: int = 0,
    terminal_order: tuple[int, ...] = (), continuation_index: int = 1,
    completed_writer_run_ids: tuple[int, ...] = (),
) -> tuple[OpenSnapshot, frozenset[int]]:
    """Bind the writer scope to current App invalidations from one dispatcher."""
    if scope not in {"early", "all"}:
        raise GovernanceError("Writer scope is invalid.")
    if scope == "all":
        expected_numbers = snapshot.numbers
        if len(set(targets)) != len(targets) or any(type(number) is not int or number < 1 for number in targets):
            raise GovernanceError("All-open priority target boundary is invalid.")
        if not set(targets).issubset(expected_numbers):
            raise GovernanceError("All-open priority target is outside the open snapshot.")
        if (
            len(set(preserved)) != len(preserved)
            or any(type(number) is not int or number < 1 for number in preserved)
            or not set(preserved).issubset(targets)
            or (bool(preserved) != (preserved_writer_run_id > 0))
        ):
            raise GovernanceError("All-open preserved target boundary is invalid.")
        if continuation_index < 1:
            raise GovernanceError("All-open continuation boundary is invalid.")
    else:
        if source.event in {"schedule", "workflow_dispatch"} or not targets or len(set(targets)) != len(targets):
            raise GovernanceError("Early writer target boundary is invalid.")
        if any(type(number) is not int or number < 1 for number in targets):
            raise GovernanceError("Early writer target boundary is invalid.")
        if not set(targets).issubset(snapshot.numbers):
            raise GovernanceError("Early writer target is outside the open snapshot.")
        if preserved or preserved_writer_run_id != 0:
            raise GovernanceError("Early writer cannot preserve an all-open target.")
        expected_numbers = targets
        # The early workflow acquired the repository writer singleton with
        # cancellation before this program starts.  It therefore owns the
        # source pending mutation itself; requiring an external dispatcher
        # marker would reintroduce the old GET/PATCH hand-off race.
        selected = {
            pull_request.get("number"): pull_request
            for pull_request in snapshot.pull_requests
            if isinstance(pull_request, dict) and pull_request.get("number") in expected_numbers
        }
        if set(selected) != set(expected_numbers):
            raise GovernanceError("Early writer target set changed.")
        return (
            OpenSnapshot(
                tuple(expected_numbers), snapshot.claimants,
                tuple(selected[number] for number in expected_numbers),
            ),
            frozenset(),
        )
    prefix_length = (continuation_index - 1) * 150 if scope == "all" else 0
    prefix = terminal_order[:prefix_length]
    if len(prefix) != prefix_length or len(set(prefix)) != len(prefix):
        raise GovernanceError("All-open terminal prefix boundary is invalid.")
    prefix_writer_ids = {
        number: completed_writer_run_ids[index // 150]
        for index, number in enumerate(prefix)
    }
    expected_fresh = dispatcher_invalidation_url(source, 0)
    expected_carry = dispatcher_invalidation_url(source, 1)
    carry: set[int] = set()
    selected: dict[int, dict[str, Any]] = {}
    for pull_request in snapshot.pull_requests:
        number = pull_request.get("number")
        head = pull_request.get("head_sha")
        draft = pull_request.get("isDraft")
        if type(number) is not int or number < 1 or type(draft) is not bool or not isinstance(head, str) or not SHA.fullmatch(head):
            raise GovernanceError("Open pull request head is invalid.")
        if number not in expected_numbers:
            continue
        if number in preserved:
            preserved_value = check_run_for_external_id(
                head,
                CHECK_EXTERNAL_PREFIX + head.lower() + f"/writer-{preserved_writer_run_id}",
            )
            if preserved_value is None:
                raise GovernanceError("Preserved early governance Check Run is missing.")
            if not preserved_early_success(
                preserved_value, head, preserved_writer_run_id,
                body_sha256=pr_body_sha256(pull_request.get("body")),
            ):
                raise GovernanceError("Preserved early governance success is invalid.")
            continue
        value = check_run(head)
        if value is None:
            raise GovernanceError("Dispatcher invalidation Check Run is missing.")
        if number in prefix_writer_ids:
            if draft:
                if (
                    value.get("status") != "in_progress" or value.get("conclusion") is not None
                    or value.get("details_url") not in {expected_fresh, expected_carry}
                ):
                    raise GovernanceError("Prior terminal Draft Check Run is invalid.")
                continue
            writer_run_id = prefix_writer_ids[number]
            if value.get("status") == "completed" and value.get("conclusion") == "success":
                if not preserved_early_success(
                    value, head, writer_run_id, body_sha256=pr_body_sha256(pull_request.get("body")),
                ):
                    raise GovernanceError("Prior terminal success Check Run is invalid.")
            elif (
                (value.get("status") == "completed" and value.get("conclusion") == "failure")
                or (value.get("status") == "in_progress" and value.get("conclusion") is None)
            ):
                if not prior_writer_url_matches(value, writer_run_id):
                    raise GovernanceError("Prior terminal Check Run writer evidence is invalid.")
            else:
                raise GovernanceError("Prior terminal Check Run state is invalid.")
            continue
        if value.get("status") != "in_progress" or value.get("conclusion") is not None:
            raise GovernanceError("Dispatcher invalidation Check Run state is invalid.")
        details = value.get("details_url")
        if details not in {expected_fresh, expected_carry}:
            raise GovernanceError("Dispatcher invalidation Check Run evidence is stale or foreign.")
        if details == expected_carry:
            if draft:
                raise GovernanceError("Draft pull request cannot carry a terminal governance decision.")
            carry.add(number)
        selected[number] = pull_request
    if set(selected) != set(expected_numbers) - set(preserved) - set(prefix):
        raise GovernanceError("Dispatcher invalidation target set changed.")
    return (
        OpenSnapshot(
            tuple(number for number in expected_numbers if number not in preserved and number not in prefix), snapshot.claimants,
            tuple(selected[number] for number in expected_numbers if number not in preserved and number not in prefix),
        ),
        frozenset(carry),
    )


def sensor(number: int, base: str, head: str, evidence: EvidenceSnapshot | None = None) -> int:
    if evidence is None:
        trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", base, head)
        query = urlencode({"head_sha": head, "per_page": 100})
        direct_runs = bounded_head_runs(
            f"repos/{REPOSITORY}/actions/workflows/pr-governance-review-events.yml/runs?{query}",
            head, "Review sensor",
        )
        direct_by_event = {
            event: tuple(run for run in direct_runs if run.get("event") == event)
            for event in ("pull_request", "pull_request_review", "pull_request_review_comment")
        }
    candidates: list[tuple[int, int, int]] = []
    for event in ("pull_request", "pull_request_review", "pull_request_review_comment"):
        if evidence is None:
            run_pages = [{"workflow_runs": list(direct_by_event[event])}]
        else:
            run_pages = [{"workflow_runs": list(evidence.sensor_runs.get(event, ()))}]
        for page in run_pages:
            runs = page.get("workflow_runs")
            if not isinstance(runs, list):
                raise GovernanceError("Review sensor response is invalid.")
            for run in runs:
                if not isinstance(run, dict):
                    raise GovernanceError("Review sensor run is invalid.")
                pulls = run.get("pull_requests")
                if not isinstance(pulls, list) or len(pulls) != 1 or not isinstance(pulls[0], dict):
                    continue
                current = pulls[0]
                run_base, run_head = current.get("base"), current.get("head")
                repository = run.get("repository")
                base_repository = run_base.get("repo") if isinstance(run_base, dict) else None
                head_repository = run_head.get("repo") if isinstance(run_head, dict) else None
                if not (
                    run.get("name") == "PR governance review sensor" and run.get("event") == event
                    and workflow_path_matches(run.get("path"), ".github/workflows/pr-governance-review-events.yml")
                    and run.get("head_sha") == head and type(run.get("run_attempt")) is int and run.get("run_attempt") == 1
                    and current.get("number") == number and isinstance(run_base, dict) and isinstance(run_head, dict)
                    and run_base.get("sha") == base and run_head.get("sha") == head
                    and repository_boundary_matches(
                        repository, (base_repository, head_repository), REPOSITORY,
                    )
                    and type(run.get("id")) is int and type(run.get("run_number")) is int
                ):
                    continue
                candidates.append((run["run_number"], run["id"], run["id"]))
    if not candidates:
        raise GovernanceError("No current trusted review sensor exists.")
    return max(candidates)[2]


@dataclass(frozen=True)
class Generation:
    name: str
    path: str
    workflow_id: int
    identifier: int
    number: int
    attempt: int
    status: str
    conclusion: object


@dataclass(frozen=True)
class EvidenceSnapshot:
    """A complete, run-wide cache of workflow-run pages.

    The arbiter deliberately lists each governed workflow once, rather than
    making every one of 300 PR decisions page the same run history again.
    Per-PR workflow-byte and head/base checks remain fail-closed below.
    """
    sensor_runs: dict[str, tuple[dict[str, Any], ...]]
    workflow_ids: dict[str, int]
    workflow_runs: dict[str, tuple[dict[str, Any], ...]]
    workflow_blobs: dict[tuple[str, str], str] = field(default_factory=dict, compare=False)


@dataclass(frozen=True)
class PendingDecision:
    number: int
    head: str
    base: str
    pending_check_fingerprint: tuple[object, ...]
    state: str
    description: str
    sensor_id: int | None
    generations: tuple[Generation, Generation] | None
    issue: str | None
    body_sha256: str


def _page_endpoint(endpoint: str, page: int) -> str:
    parsed = urlparse(endpoint)
    query = parse_qs(parsed.query, keep_blank_values=True)
    query["page"] = [str(page)]
    return urlunparse(parsed._replace(query=urlencode(query, doseq=True)))


def bounded_head_runs(
    endpoint: str, head: str, response_name: str, *, max_runs: int = MAX_EVIDENCE_RUNS_PER_QUERY,
    default_token: bool = False, anchor_default_token: bool = False,
    additional_default_pages: frozenset[int] = frozenset(),
) -> tuple[dict[str, Any], ...]:
    """Read a bounded exact-head snapshot and fence its first page against races."""
    if (
        SHA.fullmatch(head) is None or type(max_runs) is not int or max_runs < 1
        or not isinstance(additional_default_pages, frozenset)
        or any(type(page) is not int or page < 1 for page in additional_default_pages)
    ):
        raise GovernanceError("Evidence PR head is invalid.")
    values: list[dict[str, Any]] = []
    seen_ids: set[int] = set()
    expected_total: int | None = None
    page_number = 1
    while True:
        current_endpoint = endpoint if page_number == 1 else _page_endpoint(endpoint, page_number)
        use_default_token = (
            default_token or (anchor_default_token and page_number == 1)
            or page_number in additional_default_pages
        )
        page = object_page(current_endpoint, default_token=True) if use_default_token else object_page(current_endpoint)
        runs = page.get("workflow_runs")
        total_count = page.get("total_count")
        if (
            not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs)
            or type(total_count) is not int or total_count < 0
            or total_count > max_runs
            or len(runs) > MAX_EVIDENCE_RUNS_PER_PAGE
        ):
            raise GovernanceError(f"{response_name} exact-head evidence is incomplete or mismatched.")
        if expected_total is None:
            expected_total = total_count
        elif total_count != expected_total:
            # A run appearing/disappearing while following the page cursor
            # makes the generation ordering ambiguous; retry on a later pass.
            raise GovernanceError(f"{response_name} exact-head evidence changed during pagination.")
        for run in runs:
            run_id = run.get("id")
            if (
                run.get("head_sha") != head
                or type(run_id) is not int or run_id < 1
                or run_id in seen_ids
            ):
                raise GovernanceError(f"{response_name} exact-head evidence is incomplete or mismatched.")
            seen_ids.add(run_id)
        values.extend(runs)
        if len(values) > expected_total:
            raise GovernanceError(f"{response_name} exact-head evidence is incomplete or mismatched.")
        if len(values) == expected_total:
            # ``total_count`` alone cannot detect a same-count insertion and
            # deletion (or page-order shift) while we follow the cursor.  A
            # second immutable page-1 read proves the cursor's anchor has not
            # changed; otherwise selecting a latest run is unsafe.
            first = object_page(endpoint, default_token=True) if (default_token or anchor_default_token) else object_page(endpoint)
            first_runs = first.get("workflow_runs") if isinstance(first, dict) else None
            if (
                first.get("total_count") != expected_total
                or not isinstance(first_runs, list)
                or len(first_runs) != min(expected_total, MAX_EVIDENCE_RUNS_PER_PAGE)
                or any(not isinstance(run, dict) or run.get("head_sha") != head for run in first_runs)
                or [run.get("id") for run in first_runs] != [run.get("id") for run in values[:len(first_runs)]]
            ):
                raise GovernanceError(f"{response_name} exact-head evidence changed during pagination.")
            return tuple(values)
        # Any non-final short page proves that the response did not provide a
        # complete cursor chain for the advertised total.
        if len(runs) != MAX_EVIDENCE_RUNS_PER_PAGE:
            raise GovernanceError(f"{response_name} exact-head evidence is incomplete or mismatched.")
        page_number += 1


def evidence_snapshot(snapshot: OpenSnapshot, target_numbers: tuple[int, ...]) -> EvidenceSnapshot:
    """Cache only bounded, server-filtered evidence for this terminal slice."""
    if (
        not isinstance(target_numbers, tuple) or not target_numbers
        or len(target_numbers) > MAX_EVIDENCE_TARGETS
        or len(set(target_numbers)) != len(target_numbers)
        or any(type(number) is not int or number < 1 for number in target_numbers)
    ):
        raise GovernanceError("Evidence target slice is invalid.")
    global _active_initial_evidence_deadline
    if _active_initial_evidence_deadline is not None:
        raise GovernanceError("Initial evidence deadline is already active.")
    _active_initial_evidence_deadline = time.monotonic() + INITIAL_EVIDENCE_DEADLINE_SECONDS
    try:
        return _evidence_snapshot_with_deadline(snapshot, target_numbers)
    finally:
        _active_initial_evidence_deadline = None


def _evidence_snapshot_with_deadline(snapshot: OpenSnapshot, target_numbers: tuple[int, ...]) -> EvidenceSnapshot:
    """Build initial evidence while ``command`` enforces the active deadline."""
    heads: dict[int, str] = {}
    for pull_request in snapshot.pull_requests:
        number = pull_request.get("number")
        head = pull_request.get("head_sha")
        if number in target_numbers:
            if type(number) is not int or not isinstance(head, str) or SHA.fullmatch(head) is None or number in heads:
                raise GovernanceError("Evidence target head is invalid.")
            heads[number] = head.lower()
    if set(heads) != set(target_numbers):
        raise GovernanceError("Evidence target snapshot is incomplete.")
    # Multiple open PRs may intentionally share an immutable commit.  Preserve
    # every number-to-head binding above, but read each exact head only once.
    unique_heads = tuple(dict.fromkeys(heads.values()))

    # The review sensor can legitimately retain three exact-head pages, while
    # each CI generation is deliberately limited to one.  Do not coalesce
    # these endpoints: doing so would permit a CI history larger than the
    # initial-read contract and make its generation selection ambiguous.
    sensor_values: dict[str, tuple[dict[str, Any], ...]] = {}
    for index, head in enumerate(unique_heads):
        query = urlencode({"head_sha": head, "per_page": 100})
        sensor_values[head] = bounded_head_runs(
            f"repos/{REPOSITORY}/actions/workflows/pr-governance-review-events.yml/runs?{query}",
            head,
            "Initial review sensor",
            anchor_default_token=True,
            additional_default_pages=(frozenset({2}) if index < MAX_DEFAULT_INITIAL_SENSOR_PAGE_2_HEADS else frozenset()),
        )
    sensor_runs = {
        event: tuple(
            run for values in sensor_values.values() for run in values
            if run.get("event") == event
            and workflow_path_matches(run.get("path"), ".github/workflows/pr-governance-review-events.yml")
        )
        for event in ("pull_request", "pull_request_review", "pull_request_review_comment")
    }
    workflow_ids: dict[str, int] = {}
    workflow_runs: dict[str, tuple[dict[str, Any], ...]] = {}
    for path in (".github/workflows/test-and-build.yml", ".github/workflows/release-preflight.yml"):
        workflow = api_json(
            f"repos/{REPOSITORY}/actions/workflows/{path.rsplit('/', 1)[-1]}", default_token=True,
        )
        workflow_id = workflow.get("id") if isinstance(workflow, dict) else None
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Default-branch CI workflow ID is invalid.")
        values: list[dict[str, Any]] = []
        for head in unique_heads:
            query = urlencode({"event": "pull_request", "head_sha": head, "per_page": 100})
            values.extend(bounded_head_runs(
                f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?{query}",
                head,
                "Initial CI generation",
                max_runs=MAX_EVIDENCE_RUNS_PER_PAGE,
                default_token=True,
            ))
        workflow_ids[path] = workflow_id
        workflow_runs[path] = tuple(values)
    return EvidenceSnapshot(sensor_runs, workflow_ids, workflow_runs)


def final_evidence_for_pr(head: str, initial: EvidenceSnapshot) -> EvidenceSnapshot:
    """Read every relevant current-head run through bounded repository pages."""
    if SHA.fullmatch(head) is None:
        raise GovernanceError("Final evidence head is invalid.")
    runs = bounded_head_runs(
        f"repos/{REPOSITORY}/actions/runs?" + urlencode({"head_sha": head, "per_page": 100}),
        head,
        "Final workflow-run",
    )
    sensor_runs = {
        event: tuple(
            run for run in runs
            if run.get("event") == event
            and workflow_path_matches(run.get("path"), ".github/workflows/pr-governance-review-events.yml")
        )
        for event in ("pull_request", "pull_request_review", "pull_request_review_comment")
    }
    workflow_runs = {
        path: tuple(run for run in runs if run.get("workflow_id") == workflow_id)
        for path, workflow_id in initial.workflow_ids.items()
    }
    # Reuse only immutable default workflow IDs; byte guards have their own
    # per-head cache and still check each source workflow before use.
    return EvidenceSnapshot(sensor_runs, initial.workflow_ids, workflow_runs, initial.workflow_blobs)


def generation(number: int, base: str, head: str, name: str, path: str, evidence: EvidenceSnapshot | None = None) -> Generation:
    # A workflow run can originate from a PR-modified YAML file.
    if evidence is None:
        trusted_workflow_blob(path, base, head)
    if evidence is None:
        workflow = api_json(f"repos/{REPOSITORY}/actions/workflows/{path.rsplit('/', 1)[-1]}")
        workflow_id = workflow.get("id") if isinstance(workflow, dict) else None
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Default-branch CI workflow ID is invalid.")
        query = urlencode({"event": "pull_request", "head_sha": head, "per_page": 100})
        run_pages = [{"workflow_runs": list(bounded_head_runs(
            f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?{query}", head, "CI generation",
            max_runs=MAX_EVIDENCE_RUNS_PER_PAGE,
        ))}]
    else:
        workflow_id = evidence.workflow_ids.get(path)
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Cached CI workflow ID is invalid.")
        run_pages = [{"workflow_runs": list(evidence.workflow_runs.get(path, ()))}]
    matches: list[Generation] = []
    for page in run_pages:
        runs = page.get("workflow_runs")
        if not isinstance(runs, list):
            raise GovernanceError("CI generation response is invalid.")
        for run in runs:
            if not isinstance(run, dict):
                raise GovernanceError("CI run is invalid.")
            pulls = run.get("pull_requests")
            if not isinstance(pulls, list) or len(pulls) != 1 or not isinstance(pulls[0], dict):
                continue
            item = pulls[0]
            run_base, run_head, repository = item.get("base"), item.get("head"), run.get("repository")
            base_repository = run_base.get("repo") if isinstance(run_base, dict) else None
            head_repository = run_head.get("repo") if isinstance(run_head, dict) else None
            if not (
                run.get("name") == name and workflow_path_matches(run.get("path"), path) and run.get("event") == "pull_request"
                and run.get("head_sha") == head and item.get("number") == number
                and isinstance(run_base, dict) and isinstance(run_head, dict)
                and run_base.get("sha") == base and run_head.get("sha") == head
                and repository_boundary_matches(
                    repository, (base_repository, head_repository), REPOSITORY,
                )
                and run.get("workflow_id") == workflow_id and type(run.get("id")) is int and type(run.get("run_number")) is int
                and type(run.get("run_attempt")) is int and isinstance(run.get("status"), str)
            ):
                continue
            matches.append(Generation(name, path, workflow_id, run["id"], run["run_number"], run["run_attempt"], run["status"], run.get("conclusion")))
    if not matches:
        raise GovernanceError("Current CI generation is missing.")
    return max(matches, key=lambda item: (item.number, item.attempt, item.identifier))


def verdict(value: Generation) -> str:
    if value.status in {"queued", "in_progress", "pending", "waiting", "requested"}:
        return "pending"
    if value.status == "completed" and value.conclusion == "success":
        return "success"
    if value.status == "completed":
        return "failure"
    raise GovernanceError("CI generation status is invalid.")


def contract(number: int, base: str, head: str, branch: str, draft: bool, snapshot_path: str) -> str:
    timeout = GITHUB_API_TIMEOUT_SECONDS
    if _terminal_deadline_monotonic is not None:
        timeout = min(timeout, _terminal_deadline_monotonic - time.monotonic())
        if timeout <= 0:
            raise GovernanceError("Governance terminal deadline exceeded.")
    try:
        issue = subprocess.run(
        [sys.executable, "scripts/hooks/verify_push_issue.py", "--pr-number", str(number),
         "--pr-base-sha", base, "--pr-head-sha", head, "--pr-branch", branch,
         "--repository", REPOSITORY, "--trusted-default-sha", os.environ.get("GITHUB_SHA", "")], check=False, env=read_environment(), timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise GovernanceError("Governance contract verifier timed out.") from error
    if _terminal_deadline_monotonic is not None and time.monotonic() > _terminal_deadline_monotonic:
        raise GovernanceError("Governance terminal deadline exceeded.")
    if issue.returncode != 0:
        return "failure"
    if draft:
        return "pending"
    if _terminal_deadline_monotonic is not None:
        timeout = min(GITHUB_API_TIMEOUT_SECONDS, _terminal_deadline_monotonic - time.monotonic())
        if timeout <= 0:
            raise GovernanceError("Governance terminal deadline exceeded.")
    try:
        ready = subprocess.run(
        [sys.executable, "scripts/review/verify_pr_ready.py", "--pr", str(number),
         "--expected-base-sha", base, "--expected-head-sha", head, "--allow-ready",
         # This writer is producing the trusted Check Run itself.  The
         # verifier still checks its App binding, latch/source, CI, Issue and
         # review evidence, but must not require this output to already be
         # completed/success while it is deliberately in_progress.
         "--exclude-trusted-governance-check", "--open-pull-snapshot", snapshot_path], check=False,
        env=read_environment(), timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise GovernanceError("Governance contract verifier timed out.") from error
    if _terminal_deadline_monotonic is not None and time.monotonic() > _terminal_deadline_monotonic:
        raise GovernanceError("Governance terminal deadline exceeded.")
    return "success" if ready.returncode == 0 else "failure"


def final_closer_is_unique(
    number: int, issue: str, base: str, head: str, body_sha256: str,
    claimants: dict[str, frozenset[int]],
) -> bool:
    current = pull(number)
    expected_branch = default_branch_name()
    expected_default_head = os.environ.get("GITHUB_SHA", "")
    current_base = current.get("base") if isinstance(current, dict) else None
    current_head = current.get("head") if isinstance(current, dict) else None
    base_repository = current_base.get("repo") if isinstance(current_base, dict) else None
    head_repository = current_head.get("repo") if isinstance(current_head, dict) else None
    actual_issue = canonical_issue(current.get("body"))
    if (
        not isinstance(current, dict) or BODY_SHA256.fullmatch(body_sha256) is None
        or SHA.fullmatch(expected_default_head) is None
        or pr_body_sha256(current.get("body")) != body_sha256
        or actual_issue != issue or not isinstance(current_base, dict) or not isinstance(current_head, dict)
        or current_base.get("sha") != base or current_head.get("sha") != head
        or current_base.get("ref") != expected_branch or current_base.get("sha") != expected_default_head
        or not isinstance(base_repository, dict) or not isinstance(head_repository, dict)
        or base_repository.get("full_name") != REPOSITORY or head_repository.get("full_name") != REPOSITORY
        or base_repository.get("default_branch") != expected_branch
    ):
        return False
    # A malformed multi-Issue closer is a claimant for every Issue it names;
    # the one complete snapshot prevents O(N^2) GETs for a 300+ PR run.
    return claimants.get(issue) == frozenset({number})


def target_url(
    *, source_run_id: int | None = None, generations: tuple[Generation, Generation] | None = None,
    base: str | None = None, head: str | None = None, body_sha256: str | None = None,
) -> str:
    url = f"{SERVER_URL}/{REPOSITORY}/actions/runs/{WRITER_RUN_ID}"
    if source_run_id is None and generations is None and base is None and head is None and body_sha256 is None:
        return url
    if body_sha256 is not None and BODY_SHA256.fullmatch(body_sha256) is None:
        raise GovernanceError("Pull request body digest is invalid.")
    parts = urlparse(url)
    query: dict[str, str] = {}
    if source_run_id is not None:
        query["source_run_id"] = str(source_run_id)
    if generations is not None:
        for prefix, item in zip(("ci", "release"), generations, strict=True):
            query[f"{prefix}_workflow_id"] = str(item.workflow_id)
            query[f"{prefix}_run_id"] = str(item.identifier)
            query[f"{prefix}_run_number"] = str(item.number)
            query[f"{prefix}_run_attempt"] = str(item.attempt)
            query[f"{prefix}_status"] = item.status
            query[f"{prefix}_conclusion"] = item.conclusion if isinstance(item.conclusion, str) else ""
    if base is not None:
        query["pr_base_sha"] = base
    if head is not None:
        query["pr_head_sha"] = head
    if body_sha256 is not None:
        query["pr_body_sha256"] = body_sha256
    return urlunparse(parts._replace(query=urlencode(query)))


def process(number: int, claimants: dict[str, frozenset[int]], snapshot_path: str, evidence: EvidenceSnapshot | None = None, *, defer_terminal: bool = False) -> PendingDecision | None:
    initial = pull(number)
    head, base, branch, draft = initial["head"]["sha"], initial["base"]["sha"], initial["head"].get("ref"), initial["draft"]
    if not isinstance(branch, str):
        raise GovernanceError("Pull request branch is invalid.")
    # The dispatcher is the only event-path invalidator.  Main always defers
    # terminal publication, so it reuses that pending status (or a scheduled
    # baseline) rather than creating a second pending status for every PR.
    pending = check_baseline(head) if defer_terminal else write_governance_check(head, "pending", "Trusted governance revalidation is running.", target_url())
    body_sha256 = ""
    try:
        body_sha256 = pr_body_sha256(initial.get("body"))
        issue = canonical_issue(initial.get("body"))
        result = contract(number, base, head, branch, draft, snapshot_path)
        # A Draft is deliberately non-terminal. It must not require a final
        # review sensor or release-preflight run merely to stay pending.
        if draft:
            if defer_terminal:
                # The dispatcher already invalidated this head.  Do not add
                # an unbudgeted pending mutation while an all-open writer is
                # deliberately conserving its Check Run write allowance.
                return None
            if not check_changed_since(head, pending):
                write_governance_check(head, "pending", "Draft PR governance remains pending.", target_url())
            return
        # verify_push_issue rejects every workflow-file change in the PR
        # range.  Once that contract fails, no untrusted workflow evidence is
        # needed to publish a failure; once it succeeds, the shared
        # default-branch run index is the trust chain and avoids O(N) blob
        # reads for the same immutable workflow paths.
        if result != "success":
            if defer_terminal:
                return PendingDecision(number, head, base, pending, "failure", "Trusted PR governance failed.", None, None, issue, body_sha256)
            if not check_changed_since(head, pending):
                write_governance_check(
                    head, "failure", "Trusted PR governance failed.", target_url(),
                    expected_fingerprint=pending,
                )
            return
        sensor_id = sensor(number, base, head, evidence)
        current_generations = (
            generation(number, base, head, "CI", ".github/workflows/test-and-build.yml", evidence),
            generation(number, base, head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
        )
        ci = "failure" if "failure" in {verdict(item) for item in current_generations} else "pending" if "pending" in {verdict(item) for item in current_generations} else "success"
        if draft or result == "pending" or ci == "pending":
            state, description = "pending", "Trusted governance revalidation is pending."
        elif result != "success" or ci != "success" or issue is None:
            state, description = "failure", "Trusted PR governance failed."
        elif not final_closer_is_unique(number, issue, base, head, body_sha256, claimants):
            state, description = "failure", "Canonical Issue closer set changed."
        else:
            # A second read immediately before success rejects same-head reruns and attempts.
            latest = (
                generation(number, base, head, "CI", ".github/workflows/test-and-build.yml", evidence),
                generation(number, base, head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
            )
            if latest != current_generations:
                state, description = "pending", "CI generation changed during governance revalidation."
            elif not defer_terminal and check_changed_since(head, pending):
                return
            else:
                state, description = "success", "Trusted PR governance passed."
        if defer_terminal:
            return PendingDecision(number, head, base, pending, state, description, sensor_id, current_generations, issue, body_sha256)
        terminal_count = 0
        if state == "success" and not defer_terminal:
            terminal_count = sensor_terminal_check_count(head, sensor_id)
            if terminal_count >= 2:
                raise NoPostGovernanceError("Review latch already has multiple terminal statuses.")
            # Preserve the one existing sensor-bound terminal status.  A new
            # success intentionally omits source_run_id so the latch remains
            # unambiguous.
        if check_changed_since(head, pending):
            return
        if state == "success" and not defer_terminal and not final_closer_is_unique(number, issue, base, head, body_sha256, claimants):
            state, description = "failure", "Pull request body changed during governance revalidation."
        if os.environ.get("GITHUB_ACTIONS") == "true":
            rebind_trusted_default_writer()
        write_governance_check(head, state, description, target_url(
            source_run_id=sensor_id if state == "success" and terminal_count == 0 else None,
            generations=current_generations if state == "success" else None,
            base=base if state == "success" else None,
            head=head if state == "success" else None,
            body_sha256=body_sha256 if state == "success" else None,
        ), expected_fingerprint=pending)
    except NoPostGovernanceError:
        raise
    except GovernanceError:
        if defer_terminal:
            # Main owns the terminal-write reservation.  Publishing a
            # fail-closed result here would let malformed tail PRs bypass the
            # segment単位のterminal write予算を迂回してrate-limit burstを起こす。
            return PendingDecision(
                number, head, base, pending, "failure",
                "Trusted PR governance failed closed.", None, None, None, body_sha256,
            )
        if not check_changed_since(head, pending):
            write_governance_check(
                head, "failure", "Trusted PR governance failed closed.", target_url(),
                expected_fingerprint=pending,
            )
        raise


def finalize_decision(decision: PendingDecision, claimants: dict[str, frozenset[int]], evidence: EvidenceSnapshot) -> bool:
    """Write one terminal state after a bounded final head-specific refresh."""
    state, description = decision.state, decision.description
    generations = decision.generations
    terminal_count = 0
    rebind_started = False
    terminal_write_started = False
    try:
        if state == "success":
            if (
                decision.sensor_id is None or generations is None or decision.issue is None
                or BODY_SHA256.fullmatch(decision.body_sha256) is None
            ):
                raise GovernanceError("Successful governance decision is incomplete.")
            if sensor(decision.number, decision.base, decision.head, evidence) != decision.sensor_id:
                state, description = "pending", "Review sensor changed during governance revalidation."
            else:
                latest = (
                    generation(decision.number, decision.base, decision.head, "CI", ".github/workflows/test-and-build.yml", evidence),
                    generation(decision.number, decision.base, decision.head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
                )
                if latest != generations:
                    state, description = "pending", "CI generation changed during governance revalidation."
        desired = target_url(
            source_run_id=decision.sensor_id if state == "success" else None,
            generations=generations if state == "success" else None,
            base=decision.base if state == "success" else None,
            head=decision.head if state == "success" else None,
            body_sha256=decision.body_sha256 if state == "success" else None,
        )
        if decision.sensor_id is not None:
            newer, observed_terminal_count, exact_current = check_fence(
                decision.head, decision.pending_check_fingerprint, decision.sensor_id,
                desired_state=state, desired_target=desired,
            )
            if state == "success":
                terminal_count = observed_terminal_count
                if terminal_count >= 2:
                    raise NoPostGovernanceError("Review latch already has multiple terminal statuses.")
        else:
            newer = check_changed_since(decision.head, decision.pending_check_fingerprint)
            exact_current = False
        if newer:
            return False
        # The body is mutable without a head change.  Read it again after all
        # review/CI fences and before honoring an existing or new success.
        if state == "success" and not final_closer_is_unique(
            decision.number, decision.issue, decision.base, decision.head,
            decision.body_sha256, claimants,
        ):
            state, description, exact_current = (
                "failure", "Pull request body changed during governance revalidation.", False,
            )
        # A scheduled all-open pass must not manufacture a fresh status when
        # the currently trusted App status already has identical immutable
        # CI/review/base/head evidence.
        if exact_current:
            return False
        # closerの読取後にもdefault refは進み得る。stateにかかわらず、
        # terminal PATCH直前にwriterのdefault refを再束縛する。
        rebind_started = True
        rebind_trusted_default_writer()
        terminal_write_started = True
        write_governance_check(decision.head, state, description, target_url(
            source_run_id=decision.sensor_id if state == "success" and terminal_count == 0 else None,
            generations=generations if state == "success" else None,
            base=decision.base if state == "success" else None,
            head=decision.head if state == "success" else None,
            body_sha256=decision.body_sha256 if state == "success" else None,
        ), expected_fingerprint=decision.pending_check_fingerprint)
        return True
    except NoPostGovernanceError:
        raise
    except GovernanceError:
        # default refの再束縛そのものが失敗した場合、同じ読取を繰り返して
        # rate budgetを消費したり失敗CheckをPATCHしたりしてはならない。
        if rebind_started and not terminal_write_started:
            raise
        rebind_trusted_default_writer()
        if not check_changed_since(decision.head, decision.pending_check_fingerprint):
            write_governance_check(
                decision.head, "failure", "Trusted PR governance failed closed.", target_url(),
                expected_fingerprint=decision.pending_check_fingerprint,
            )
        raise


def decision_write_cost(decision: PendingDecision) -> int:
    """Reserve every Check Run mutation before attempting a terminal decision."""
    if decision.pending_check_fingerprint:
        return 1
    return 1 if decision.state == "pending" else 2


def governance_order(snapshot: OpenSnapshot, carry: frozenset[int], priority: tuple[int, ...] = ()) -> tuple[int, ...]:
    """Prioritize affected event claimants, then carry, terminal work and Drafts."""
    numbers = snapshot.numbers
    if not carry.issubset(numbers):
        raise GovernanceError("Dispatcher carry target is outside the open snapshot.")
    if len(set(priority)) != len(priority) or not set(priority).issubset(numbers):
        raise GovernanceError("Dispatcher priority target is outside the open snapshot.")
    drafts: dict[int, bool] = {}
    for pull_request in snapshot.pull_requests:
        number = pull_request.get("number")
        draft = pull_request.get("isDraft")
        if type(number) is not int or number not in numbers or type(draft) is not bool or number in drafts:
            raise GovernanceError("Open pull request draft state is invalid.")
        drafts[number] = draft
    if set(drafts) != set(numbers):
        raise GovernanceError("Open pull request draft snapshot is incomplete.")
    if any(drafts[number] for number in carry):
        raise GovernanceError("Draft pull request cannot carry a terminal governance decision.")
    # The dispatcher puts every PR that can be affected by the triggering
    # Issue/PR event first.  This preserves the resolver's source-first
    # closure ordering even when REST pagination has unrelated PRs ahead of
    # it, and makes every continuation segment preserve that closure order.
    return (
        priority
        + tuple(number for number in numbers if number in carry and number not in priority)
        + tuple(number for number in numbers if not drafts[number] and number not in carry and number not in priority)
        + tuple(number for number in numbers if drafts[number] and number not in priority)
    )


def main() -> int:
    global _terminal_deadline_monotonic
    if not REPOSITORY or not SERVER_URL or not NUMBER.fullmatch(WRITER_RUN_ID):
        print("Writer runtime identity is invalid.", file=sys.stderr)
        return 1
    dispatcher_run_id = os.environ.get("GOVERNANCE_DISPATCHER_RUN_ID", "")
    scope = os.environ.get("GOVERNANCE_SCOPE", "")
    raw_targets = os.environ.get("GOVERNANCE_TARGET_NUMBERS", "")
    raw_preserved = os.environ.get("GOVERNANCE_PRESERVED_TARGET_NUMBERS", "")
    preserved_writer_run_id = os.environ.get("GOVERNANCE_PRESERVED_WRITER_RUN_ID", "")
    raw_manifest = os.environ.get("GOVERNANCE_CHECK_MANIFEST", "")
    raw_terminal_order = os.environ.get("GOVERNANCE_TERMINAL_ORDER_NUMBERS", "")
    raw_terminal_batch = os.environ.get("GOVERNANCE_TERMINAL_BATCH_NUMBERS", "")
    raw_continuation_index = os.environ.get("GOVERNANCE_CONTINUATION_INDEX", "")
    raw_completed_writer_run_ids = os.environ.get("GOVERNANCE_COMPLETED_WRITER_RUN_IDS", "")
    raw_terminal_deadline = os.environ.get("GOVERNANCE_TERMINAL_DEADLINE_EPOCH", "0")
    _terminal_deadline_monotonic = None
    _bound_check_runs.clear()
    _bound_check_ids_by_number.clear()
    _nonreconciling_dispatcher_generations.clear()
    if not NUMBER.fullmatch(dispatcher_run_id) or scope not in {"early", "all"} or re.fullmatch(r"0|[1-9][0-9]*", preserved_writer_run_id) is None:
        print("Writer dispatch boundary is invalid.", file=sys.stderr)
        return 1
    if scope == "all" and os.environ.get("GOVERNANCE_TERMINAL_DEADLINE_REQUIRED") == "true":
        if re.fullmatch(r"[1-9][0-9]*", raw_terminal_deadline) is None:
            print("Writer terminal deadline is invalid.", file=sys.stderr)
            return 1
        remaining = int(raw_terminal_deadline) - time.time()
        if remaining <= 0:
            print("Writer terminal deadline elapsed.", file=sys.stderr)
            return 1
        _terminal_deadline_monotonic = time.monotonic() + remaining
    try:
        decoded_targets = json.loads(raw_targets)
        decoded_preserved = json.loads(raw_preserved)
        decoded_manifest = json.loads(raw_manifest)
        decoded_terminal_order = json.loads(raw_terminal_order)
        decoded_terminal_batch = json.loads(raw_terminal_batch)
        decoded_completed_writer_run_ids = json.loads(raw_completed_writer_run_ids)
        if (
            not isinstance(decoded_targets, list) or any(type(number) is not int for number in decoded_targets)
            or not isinstance(decoded_preserved, list) or any(type(number) is not int for number in decoded_preserved)
            or not isinstance(decoded_manifest, list) or not isinstance(decoded_terminal_order, list)
            or any(type(number) is not int for number in decoded_terminal_order)
            or not isinstance(decoded_terminal_batch, list) or any(type(number) is not int for number in decoded_terminal_batch)
            or not isinstance(decoded_completed_writer_run_ids, list)
            or any(type(identifier) is not int for identifier in decoded_completed_writer_run_ids)
        ):
            raise ValueError
        # Dispatcher output is a compact canonical JSON array.  Accepting
        # alternate spellings here would make an out-of-band dispatch
        # indistinguishable from the event boundary that was invalidated.
        if (
            json.dumps(decoded_targets, separators=(",", ":")) != raw_targets
            or json.dumps(decoded_preserved, separators=(",", ":")) != raw_preserved
            or json.dumps(decoded_manifest, separators=(",", ":")) != raw_manifest
            or json.dumps(decoded_terminal_order, separators=(",", ":")) != raw_terminal_order
            or json.dumps(decoded_terminal_batch, separators=(",", ":")) != raw_terminal_batch
            or json.dumps(decoded_completed_writer_run_ids, separators=(",", ":")) != raw_completed_writer_run_ids
        ):
            raise ValueError
        targets = tuple(decoded_targets)
        preserved = tuple(decoded_preserved)
        terminal_order = tuple(decoded_terminal_order)
        terminal_batch = tuple(decoded_terminal_batch)
        completed_writer_run_ids = tuple(decoded_completed_writer_run_ids)
    except (json.JSONDecodeError, ValueError):
        print("Writer target boundary is invalid.", file=sys.stderr)
        return 1
    if (
        len(set(targets)) != len(targets)
        or any(number < 1 for number in targets)
        or len(set(preserved)) != len(preserved)
        or any(number < 1 for number in preserved)
        or (scope == "all" and (not set(preserved).issubset(targets) or bool(preserved) != (preserved_writer_run_id != "0")))
        or (scope == "early" and not targets)
        or (scope == "early" and (preserved or preserved_writer_run_id != "0"))
        or (scope == "early" and (decoded_manifest or terminal_order or completed_writer_run_ids))
        or (scope == "early" and (len(targets) > 40 or terminal_batch or raw_continuation_index != "0"))
        or (scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true" and (
            not re.fullmatch(r"[1-4]", raw_continuation_index)
            or not terminal_order or len(terminal_order) > 600 or len(set(terminal_order)) != len(terminal_order)
            or any(number < 1 for number in terminal_order)
            or len(terminal_batch) > 150 or len(set(terminal_batch)) != len(terminal_batch)
            or any(number < 1 for number in terminal_batch)
            or len(completed_writer_run_ids) != int(raw_continuation_index) - 1
            or len(set(completed_writer_run_ids)) != len(completed_writer_run_ids)
            or any(identifier < 1 for identifier in completed_writer_run_ids)
        ))
    ):
        print("Writer target boundary is invalid.", file=sys.stderr)
        return 1
    manifest_numbers: list[int] = []
    manifest_check_ids: set[int] = set()
    for item in decoded_manifest:
        if (
            not isinstance(item, list) or len(item) != 2 or type(item[0]) is not int
            or item[0] in manifest_numbers or type(item[1]) is not int or item[1] < 1
            or item[1] in manifest_check_ids
        ):
            print("Writer Check Run manifest is invalid.", file=sys.stderr)
            return 1
        manifest_numbers.append(item[0])
        manifest_check_ids.add(item[1])
        _bound_check_ids_by_number[item[0]] = item[1]
    continuation_index = int(raw_continuation_index) if (
        scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true"
    ) else 0
    try:
        dispatcher_source = trusted_dispatcher_source(int(dispatcher_run_id))
        snapshot = open_snapshot()
        if scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true":
            event_tail = tuple(number for number in targets if number not in preserved)
            expected_manifest = preserved + event_tail + tuple(
                number for number in snapshot.numbers
                if number not in preserved and number not in event_tail
            )
            if tuple(manifest_numbers) != expected_manifest:
                raise GovernanceError("Writer Check Run manifest does not match the current open snapshot.")
            for pull_request in snapshot.pull_requests:
                number = pull_request.get("number")
                head = pull_request.get("head_sha")
                if number in _bound_check_ids_by_number and isinstance(head, str) and SHA.fullmatch(head):
                    external = CHECK_EXTERNAL_PREFIX + head.lower() + (
                        f"/writer-{preserved_writer_run_id}" if number in preserved else f"/dispatcher-{dispatcher_run_id}"
                    )
                    _bound_check_runs[(head, external)] = _bound_check_ids_by_number[number]
            if continuation_index > 1:
                trusted_completed_terminal_writers(
                    dispatcher_source, continuation_index, completed_writer_run_ids,
                )
        scoped_snapshot, carry = observed_invalidations(
            snapshot, dispatcher_source, scope, targets, preserved, int(preserved_writer_run_id),
            terminal_order, continuation_index, completed_writer_run_ids,
        )
        if scope == "all" and set(preserved).intersection(scoped_snapshot.numbers):
            raise GovernanceError("Preserved early success reappeared in an all-open terminal segment.")
    except GovernanceError as error:
        print(str(error), file=sys.stderr)
        return 1
    ordered_numbers = governance_order(
        scoped_snapshot, carry,
        tuple(number for number in targets if number not in preserved) if scope == "all" else (),
    )
    if scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true":
        if len(terminal_order) > 600:
            print("Writer terminal segment boundary exceeds four batches.", file=sys.stderr)
            return 1
        start = (continuation_index - 1) * 150
        expected_terminal_order = terminal_order[:start] + ordered_numbers
        expected_terminal_batch = terminal_order[start:start + 150]
        if (
            terminal_order != expected_terminal_order or not expected_terminal_batch
            or terminal_batch != expected_terminal_batch
        ):
            print("Writer terminal segment boundary is invalid.", file=sys.stderr)
            return 1
        numbers_to_process = terminal_batch
    else:
        numbers_to_process = ordered_numbers
    try:
        initial_evidence = evidence_snapshot(scoped_snapshot, numbers_to_process)
    except GovernanceError as error:
        print(str(error), file=sys.stderr)
        return 1
    failures = 0
    # Do not make one malformed/changed PR leave other open PRs stale.
    with _snapshot_file() as source:
        json.dump(list(snapshot.pull_requests), source)
        source.flush()
        # Complete each PR before starting the next one.  Retaining every
        # decision and finalizing only after all contracts ran created an
        # avoidable window for Issue/review/CI state to become stale.
        # dispatcherは全headのpending Check Runを先に作るため、all segmentは
        # 最大150 PATCHで完結する。all terminalの20.5秒paceは共有rolling上限を守る。
        # manifest欠落などで追加mutationが必要ならsegmentを成功扱いにしない。
        terminal_write_budget = 150 if scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true" else (
            400 if dispatcher_source.event == "schedule" else 100
        )
        for number in numbers_to_process:
            decision: PendingDecision | None = None
            try:
                if scope == "early":
                    # The early scope holds the writer singleton.  It writes
                    # source pending and the final result in one generation,
                    # rather than trusting a dispatcher-side pre-write.
                    process(number, snapshot.claimants, source.name, initial_evidence, defer_terminal=False)
                    continue
                decision = process(number, snapshot.claimants, source.name, initial_evidence, defer_terminal=True)
                if decision is not None:
                    cost = decision_write_cost(decision)
                    if cost > terminal_write_budget:
                        failures += 1
                        print(f"PR #{number}: terminal segment write budget is exhausted.", file=sys.stderr)
                        break
                    terminal_write_budget -= cost
                    final_evidence = (
                        final_evidence_for_pr(decision.head, initial_evidence)
                        if decision.state == "success" else initial_evidence
                    )
                    terminalized = finalize_decision(decision, snapshot.claimants, final_evidence)
                    if (
                        scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true"
                        and not terminalized
                    ):
                        failures += 1
                        print(f"PR #{number}: terminal Check Run was not published.", file=sys.stderr)
                        break
            except GovernanceError as error:
                failures += 1
                print(f"PR #{number}: {error}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
