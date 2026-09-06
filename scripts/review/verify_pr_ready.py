#!/usr/bin/env python3
"""Verify that a pull request has completed the required review workflow."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import secrets
import stat
import subprocess
import sys
import time
from urllib.parse import parse_qs, quote, urlparse
from collections.abc import Mapping, Sequence
from datetime import datetime
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parents[1] / "hooks"))
import verify_push_issue as issue_contract


_MARKER_PATTERN = re.compile(
    r"(?m)^<!-- krr-review phase=(?P<phase>initial|final) "
    r"head=(?P<head>[0-9a-f]{40}) "
    r"body-sha256=(?P<body_sha256>[0-9a-f]{64}) -->$"
)
_SUCCESSFUL_CONCLUSIONS = {"SUCCESS", "NEUTRAL", "SKIPPED"}
_VALID_REVIEW_STATES = frozenset({"APPROVED", "CHANGES_REQUESTED", "COMMENTED"})
_SELF_CHECK_NAMES = frozenset(
    {
        "PR governance",
        "KRR / PR governance (trusted check)",
        "KRR / PR governance review latch",
    }
)
_CODEX_REVIEW_TRIGGER = re.compile(r"(?m)^\s*@codex\s+review\s*$")
_CODEX_NO_ISSUES_COMMENT = re.compile(
    r"\ACodex Review: Didn't find any major issues(?:\.\.\.|\. [^.!?\r\n]+[.!?])\r?\n\r?\n"
    r"\*\*Reviewed commit:\*\* `(?P<commit>[0-9a-f]{10,40})`"
    r"(?:\Z|\r?\n\Z|\r?\n\r?\n"
    r"<details>(?: <summary>|\r?\n<summary>)ℹ️ About Codex in GitHub</summary>\r?\n"
    r"(?:(?!<details>|</details>|Codex Review:|\*\*Reviewed commit:\*\*)[\s\S]){0,8192}</details>(?:\r?\n)?\Z)"
)
_SHA = re.compile(r"[0-9a-fA-F]{40}")
_TRUSTED_REPLY_ASSOCIATIONS = frozenset({"COLLABORATOR", "MEMBER", "OWNER"})
_TRUSTED_MARKER_ASSOCIATIONS = frozenset({"COLLABORATOR", "MEMBER", "OWNER"})
_TRUSTED_CHECK = "KRR / PR governance (trusted check)"
_LATCH_CHECK = "KRR / PR governance review latch"
# The initial thread query returns the first 100 comments; continuation is bounded.
_MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES = 10
# Keep the two outer review connections complete up to a bounded, explicit
# boundary.  A page that advertises another cursor at the boundary is rejected
# before that cursor is requested, so evidence is never silently truncated.
_MAX_REVIEW_CONNECTION_PAGES = 10
# The all-open writer can process at most 150 PRs in one terminal segment.  The
# review/reviewThreads connections therefore consume at most 3,000 GraphQL
# points when every page costs one point.  Keep the other GraphQL work and a
# small operational margin reserved in the shared installation bucket.
_MAX_ALL_WRITER_TARGETS = 150
_GRAPHQL_OTHER_WORK_RESERVE = 1_200
_GRAPHQL_OPERATIONAL_HEADROOM = 300
_GRAPHQL_REVIEW_BUDGET = 4_500
_GRAPHQL_REMAINING_FLOOR = (
    _GRAPHQL_OTHER_WORK_RESERVE + _GRAPHQL_OPERATIONAL_HEADROOM
)
# A GraphQL response reports cost only after the request.  Reserve the
# largest cost accepted by this verifier before issuing the next request, so a
# stale/slow server response cannot spend into the operational floor.
_GRAPHQL_MAX_QUERY_COST = 100
# GitHub CLI eager pagination executes all pages before returning.  Keep the
# response contract bounded even when a server advertises an unbounded cursor;
# callers fail closed before processing an over-cap result.
_MAX_REST_PAGES = 10
_MAX_REST_PAGE_ITEMS = 100
_MAX_REST_ITEMS = 1_000
_REST_BUDGET = 4_500
_REST_OTHER_WORK_RESERVE = 1_200
_REST_OPERATIONAL_HEADROOM = 300
_REST_REMAINING_FLOOR = _REST_OTHER_WORK_RESERVE + _REST_OPERATIONAL_HEADROOM
_ACTIVE_SENSOR_OR_LATCH_STATUSES = frozenset(
    {"queued", "in_progress", "waiting", "requested", "pending"}
)
_ReviewMarker = tuple[str, str, str, Mapping[str, object]]
_ReviewMarkerIdentity = tuple[int, str, str, str, str, str, str, str]
_RequiredStatusChecks = tuple[tuple[str, ...], tuple[tuple[str, int | None], ...]]
_RepositoryRestIdentity = tuple[int, str, str]
_INVALID_REPOSITORY_IDENTITY = object()
_ACTIVE_REST_BUDGET: _RestBudget | None = None
_ACTIVE_GRAPHQL_BUDGET: _GraphQLBudget | None = None
_ACTIVE_SHARED_GRAPHQL_LEDGER: _SharedGraphQLLedger | None = None
_GRAPHQL_PREFLIGHT_QUERY = "query { rateLimit { cost remaining resetAt } }"


class _RestBudget:
    """Track primary REST requests against one live installation bucket."""

    def __init__(self) -> None:
        self.remaining: int | None = None
        self.limit: int | None = None
        self.reset: int | None = None

    def observe_rate_limit(self, payload: object) -> None:
        if not isinstance(payload, Mapping):
            raise TypeError("REST rate_limit response must be an object")
        resources = payload.get("resources")
        if not isinstance(resources, Mapping):
            raise TypeError("REST rate_limit resources must be an object")
        core = resources.get("core")
        if not isinstance(core, Mapping):
            raise TypeError("REST rate_limit core must be an object")
        limit = core.get("limit")
        remaining = core.get("remaining")
        reset = core.get("reset")
        if type(limit) is not int or limit < 1:
            raise TypeError("REST rate_limit core limit must be a positive integer")
        if type(remaining) is not int or remaining < 0:
            raise TypeError(
                "REST rate_limit core remaining must be a non-negative integer"
            )
        if type(reset) is not int or reset < 1:
            raise TypeError("REST rate_limit core reset must be a positive integer")
        self.limit = limit
        self.remaining = min(remaining, _REST_BUDGET)
        self.reset = reset

    def before_request(self) -> None:
        if self.remaining is None:
            raise ValueError("REST budget was not initialized from rate_limit")
        if self.remaining <= _REST_REMAINING_FLOOR:
            raise ValueError("REST remaining budget is too low for the next request")

    def consume(self) -> None:
        self.before_request()
        assert self.remaining is not None
        self.remaining -= 1


class _GraphQLBudget:
    """Validate the shared installation budget with a bounded query-cost lease."""

    def __init__(self) -> None:
        self.remaining: int | None = None
        self._server_remaining: int | None = None
        self.max_cost = 0

    def before_request(self) -> None:
        if self.remaining is not None and self.remaining < (
            max(self.max_cost, _GRAPHQL_MAX_QUERY_COST)
            + _GRAPHQL_REMAINING_FLOOR
        ):
            raise ValueError("GraphQL remaining budget is too low for the next query")

    def observe(self, payload: object) -> None:
        if not isinstance(payload, Mapping):
            raise TypeError("GraphQL response must be an object")
        if "errors" in payload and payload.get("errors") != []:
            raise TypeError("GraphQL response contains errors")
        data = payload.get("data")
        if not isinstance(data, Mapping):
            raise TypeError("GraphQL data must be an object")
        rate_limit = data.get("rateLimit")
        if not isinstance(rate_limit, Mapping):
            raise TypeError("GraphQL rateLimit must be an object")
        cost = rate_limit.get("cost")
        remaining = rate_limit.get("remaining")
        reset_at = rate_limit.get("resetAt")
        if type(cost) is not int or cost < 1:
            raise TypeError("GraphQL rateLimit cost must be a positive integer")
        if type(remaining) is not int or remaining < 0:
            raise TypeError("GraphQL rateLimit remaining must be a non-negative integer")
        try:
            reset_time = _timestamp(reset_at, "GraphQL rateLimit resetAt")
        except (TypeError, ValueError) as error:
            raise TypeError("GraphQL rateLimit resetAt must be an ISO-8601 timestamp") from error
        if reset_time is None or reset_time.tzinfo is None:
            raise TypeError("GraphQL rateLimit resetAt must be an ISO-8601 timestamp")
        if remaining < _GRAPHQL_REMAINING_FLOOR:
            raise ValueError("GraphQL remaining budget is below the reserved floor")
        if cost > _GRAPHQL_MAX_QUERY_COST:
            raise ValueError("GraphQL rateLimit cost exceeds the query budget")
        self.max_cost = max(self.max_cost, cost)
        if self._server_remaining is None:
            self.remaining = min(remaining, _GRAPHQL_REVIEW_BUDGET)
        else:
            server_delta = self._server_remaining - remaining
            if server_delta < 0:
                raise ValueError("GraphQL rateLimit remaining increased unexpectedly")
            assert self.remaining is not None
            next_remaining = self.remaining - server_delta
            if next_remaining < _GRAPHQL_REMAINING_FLOOR:
                raise ValueError("GraphQL remaining budget is below the reserved floor")
            self.remaining = next_remaining
        self._server_remaining = remaining

    def consume(self, cost: int = 1) -> None:
        """Charge a GraphQL-backed read that cannot return rateLimit metadata."""

        if type(cost) is not int or cost < 1 or cost > _GRAPHQL_MAX_QUERY_COST:
            raise ValueError("GraphQL request cost is outside the query budget")
        self.before_request()
        if self.remaining is not None:
            self.remaining -= cost
        if self._server_remaining is not None:
            self._server_remaining -= cost


class _SharedGraphQLLedger:
    """Atomically lease the all-writer GraphQL budget across verifier processes."""

    _SCHEMA = 1
    _LEASE = _GRAPHQL_MAX_QUERY_COST
    # The all-writer phase starts up to 150 independent verifier processes.
    # Each reservation fsyncs the shared state, so a healthy serialized queue
    # can exceed two seconds on saturated macOS/CI storage.  Keep a finite
    # failure boundary while allowing that authorized fan-out to converge.
    _LOCK_WAIT_SECONDS = 15.0
    _LOCK_RETRY_SECONDS = 0.01

    def __init__(self, path: Path, snapshot_sha256: str) -> None:
        self.path = path
        self.snapshot_sha256 = snapshot_sha256

    @classmethod
    def from_open_pull_snapshot(cls, snapshot_path: str) -> _SharedGraphQLLedger:
        source = Path(snapshot_path)
        if source.is_symlink():
            raise ValueError("shared GraphQL ledger snapshot must not be a symlink")
        try:
            source_stat = source.stat()
            snapshot = source.read_bytes()
        except OSError as error:
            raise ValueError("shared GraphQL ledger snapshot is unavailable") from error
        if (
            not stat.S_ISREG(source_stat.st_mode)
            or source_stat.st_uid != os.getuid()
            or source_stat.st_nlink != 1
            or stat.S_IMODE(source_stat.st_mode) != 0o600
        ):
            raise ValueError("shared GraphQL ledger snapshot permissions are invalid")
        ledger = Path(str(source) + ".krr-graphql-ledger-v1")
        instance = cls(ledger, hashlib.sha256(snapshot).hexdigest())
        instance._initialize()
        return instance

    def _state(self, descriptor: int) -> dict[str, object]:
        os.lseek(descriptor, 0, os.SEEK_SET)
        raw = os.read(descriptor, 16_384)
        if len(raw) == 16_384:
            raise ValueError("shared GraphQL ledger exceeds its bounded size")
        try:
            state = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ValueError("shared GraphQL ledger is malformed") from error
        if not isinstance(state, dict) or set(state) != {
            "reservations", "schema", "snapshot_sha256", "spent"
        }:
            raise ValueError("shared GraphQL ledger schema is invalid")
        reservations = state.get("reservations")
        spent = state.get("spent")
        if (
            state.get("schema") != self._SCHEMA
            or state.get("snapshot_sha256") != self.snapshot_sha256
            or type(spent) is not int
            or spent < 0
            or spent > _GRAPHQL_REVIEW_BUDGET - _GRAPHQL_REMAINING_FLOOR
            or not isinstance(reservations, dict)
            or any(
                not isinstance(token, str)
                or re.fullmatch(r"[0-9a-f]{64}", token) is None
                or lease != self._LEASE
                for token, lease in reservations.items()
            )
        ):
            raise ValueError("shared GraphQL ledger state is invalid")
        if sum(reservations.values()) > spent:
            raise ValueError("shared GraphQL ledger reservations are invalid")
        return state

    @staticmethod
    def _encode(state: dict[str, object]) -> bytes:
        return json.dumps(state, sort_keys=True, separators=(",", ":")).encode("utf-8")

    def _write(self, descriptor: int, state: dict[str, object]) -> None:
        payload = self._encode(state)
        os.lseek(descriptor, 0, os.SEEK_SET)
        os.ftruncate(descriptor, 0)
        written = 0
        while written < len(payload):
            written += os.write(descriptor, payload[written:])
        os.fsync(descriptor)

    @staticmethod
    def _validate_descriptor(descriptor: int) -> None:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise ValueError("shared GraphQL ledger permissions are invalid")

    def _acquire_lock(self, descriptor: int, deadline: float | None = None) -> None:
        limit = deadline if deadline is not None else time.monotonic() + self._LOCK_WAIT_SECONDS
        while True:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return
            except BlockingIOError as error:
                if time.monotonic() >= limit:
                    raise ValueError("shared GraphQL ledger lock wait elapsed") from error
                time.sleep(min(self._LOCK_RETRY_SECONDS, max(0.0, limit - time.monotonic())))
            except OSError as error:
                raise ValueError("shared GraphQL ledger lock cannot be acquired") from error

    @staticmethod
    def _is_empty(descriptor: int) -> bool:
        os.lseek(descriptor, 0, os.SEEK_SET)
        return os.read(descriptor, 1) == b""

    def _wait_for_initialization(self) -> None:
        """Wait briefly for an O_EXCL creator to fsync its identity-bound state."""

        deadline = time.monotonic() + self._LOCK_WAIT_SECONDS
        flags = os.O_RDWR | getattr(os, "O_NOFOLLOW", 0)
        while True:
            try:
                descriptor = os.open(self.path, flags)
            except OSError as error:
                raise ValueError("shared GraphQL ledger is unavailable") from error
            locked = False
            try:
                self._validate_descriptor(descriptor)
                self._acquire_lock(descriptor, deadline)
                locked = True
                try:
                    self._state(descriptor)
                    return
                except ValueError:
                    if not self._is_empty(descriptor):
                        raise
            finally:
                if locked:
                    fcntl.flock(descriptor, fcntl.LOCK_UN)
                os.close(descriptor)
            if time.monotonic() >= deadline:
                raise ValueError("shared GraphQL ledger initialization wait elapsed")
            time.sleep(min(self._LOCK_RETRY_SECONDS, max(0.0, deadline - time.monotonic())))

    def _initialize(self) -> None:
        initial = {
            "reservations": {},
            "schema": self._SCHEMA,
            "snapshot_sha256": self.snapshot_sha256,
            "spent": 0,
        }
        flags = os.O_RDWR | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(self.path, flags, 0o600)
        except FileExistsError:
            self._wait_for_initialization()
            return
        except OSError as error:
            raise ValueError("shared GraphQL ledger cannot be created") from error
        locked = False
        try:
            self._validate_descriptor(descriptor)
            self._acquire_lock(descriptor)
            locked = True
            self._write(descriptor, initial)
        finally:
            if locked:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)

    def _update(self, mutate: Any) -> Any:
        flags = os.O_RDWR | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(self.path, flags)
        except OSError as error:
            raise ValueError("shared GraphQL ledger is unavailable") from error
        locked = False
        try:
            self._validate_descriptor(descriptor)
            self._acquire_lock(descriptor)
            locked = True
            state = self._state(descriptor)
            result = mutate(state)
            self._write(descriptor, state)
            return result
        finally:
            if locked:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)

    def reserve(self) -> str:
        def mutate(state: dict[str, object]) -> str:
            spent = state["spent"]
            reservations = state["reservations"]
            assert isinstance(spent, int) and isinstance(reservations, dict)
            if spent + self._LEASE > _GRAPHQL_REVIEW_BUDGET - _GRAPHQL_REMAINING_FLOOR:
                raise ValueError("shared GraphQL ledger budget is exhausted")
            token = secrets.token_hex(32)
            while token in reservations:
                token = secrets.token_hex(32)
            reservations[token] = self._LEASE
            state["spent"] = spent + self._LEASE
            return token

        return self._update(mutate)

    def settle(self, token: str, cost: int) -> None:
        if type(cost) is not int or cost < 1 or cost > self._LEASE:
            raise ValueError("shared GraphQL ledger query cost is invalid")

        def mutate(state: dict[str, object]) -> None:
            spent = state["spent"]
            reservations = state["reservations"]
            assert isinstance(spent, int) and isinstance(reservations, dict)
            lease = reservations.pop(token, None)
            if lease != self._LEASE or spent < lease - cost:
                raise ValueError("shared GraphQL ledger reservation is invalid")
            state["spent"] = spent - lease + cost

        self._update(mutate)


def _stable_graphql_payload(payload: object) -> object:
    """Remove only live rate-limit counters before comparing a race fence."""

    if not isinstance(payload, Mapping):
        return payload
    data = payload.get("data")
    if not isinstance(data, Mapping):
        return payload
    stable_data = dict(data)
    stable_data.pop("rateLimit", None)
    return {**payload, "data": stable_data}


def _safe_branch_name(value: object) -> bool:
    """writerと同じfail-closedなGit ref component境界を検証する。"""

    return (
        isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9._/-]+", value) is not None
        and value not in {".", ".."}
        and not value.startswith("/")
        and not value.endswith("/")
        and "//" not in value
        and all(part not in {"", ".", ".."} for part in value.split("/"))
    )


def _bot_login(value: object) -> str | None:
    if not isinstance(value, Mapping):
        return None
    login = value.get("login")
    return login if isinstance(login, str) else None


def _sensor_or_latch_state_is_valid(
    value: Mapping[str, object], *, internal_recalculation: bool
) -> bool:
    """Allow only a completed success, or an active current run during recalculation."""
    status = value.get("status")
    conclusion = value.get("conclusion")
    return (
        status == "completed" and conclusion == "success"
    ) or (
        internal_recalculation
        and isinstance(status, str)
        and status in _ACTIVE_SENSOR_OR_LATCH_STATUSES
        and conclusion is None
    )


def _is_review_bot(login: str | None, review_bot: str) -> bool:
    if login is None:
        return False
    return login.removesuffix("[bot]").casefold() == review_bot.removesuffix(
        "[bot]"
    ).casefold()


def _timestamp(value: object, field: str) -> datetime | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    return datetime.fromisoformat(normalized)


def _review_completion_times(
    reviews: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> list[datetime]:
    not_before_time = _timestamp(not_before, "marker updated_at")
    if not_before_time is None:
        raise TypeError("marker updated_at must be an ISO-8601 timestamp")
    before_time = _timestamp(before, "marker created_at")
    completion_times: list[datetime] = []
    for review in reviews:
        if not (
            _is_review_bot(_bot_login(review.get("author")), review_bot)
            and review.get("state") in _VALID_REVIEW_STATES
            and isinstance(review.get("commit"), Mapping)
            and review["commit"].get("oid") == head
        ):
            continue
        submitted_at = _timestamp(review.get("submittedAt"), "review submittedAt")
        if (
            submitted_at is not None
            and submitted_at > not_before_time
            and (before_time is None or submitted_at < before_time)
        ):
            completion_times.append(submitted_at)
    return completion_times


def _review_is_for(
    reviews: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    return bool(
        _review_completion_times(reviews, review_bot, head, not_before, before)
    )


def _no_issues_comment_completion_times(
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> list[datetime]:
    """Return trusted Codex no-issues comments for one reviewed commit.

    The review connector publishes no-issues results as Issue comments rather
    than formal Pull Request reviews.  Treat only its canonical, immutable
    result shape as evidence; reactions and arbitrary bot comments remain
    insufficient for the merge gate.
    """

    not_before_time = _timestamp(not_before, "marker updated_at")
    if not_before_time is None:
        raise TypeError("marker updated_at must be an ISO-8601 timestamp")
    before_time = _timestamp(before, "marker created_at")
    completion_times: list[datetime] = []
    for comment in comments:
        if not _is_review_bot(_bot_login(comment.get("user")), review_bot):
            continue
        body = comment.get("body")
        if not isinstance(body, str):
            continue
        # Reject an otherwise-valid line if the same result fields were
        # duplicated or malformed elsewhere in the body.
        if (
            body.count("Codex Review:") != 1
            or body.count("**Reviewed commit:**") != 1
        ):
            continue
        result = _CODEX_NO_ISSUES_COMMENT.match(body)
        if result is None or not head.startswith(result.group("commit")):
            continue
        try:
            created_at = _timestamp(
                comment.get("created_at"), "no-issues comment created_at"
            )
            updated_at = _timestamp(
                comment.get("updated_at"), "no-issues comment updated_at"
            )
        except (TypeError, ValueError):
            continue
        # Issue-comment bodies are mutable.  A no-issues result is durable
        # evidence only when the GitHub timestamps prove it was never edited.
        if (
            created_at is None
            or updated_at is None
            or created_at.tzinfo is None
            or updated_at.tzinfo is None
            or created_at != updated_at
        ):
            continue
        if (
            created_at > not_before_time
            and (before_time is None or created_at < before_time)
        ):
            completion_times.append(created_at)
    return completion_times


def _review_evidence_completion_times(
    reviews: Sequence[Mapping[str, object]],
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> list[datetime]:
    """Return formal-review and canonical no-issues Issue-comment evidence."""

    return [
        *_review_completion_times(reviews, review_bot, head, not_before, before),
        *_no_issues_comment_completion_times(
            comments, review_bot, head, not_before, before
        ),
    ]


def _no_issues_evidence_is_ambiguous(
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    """Require one canonical no-issues comment at most in each phase window."""

    return (
        len(
            _no_issues_comment_completion_times(
                comments, review_bot, head, not_before, before
            )
        )
        > 1
    )


def _review_evidence_is_for(
    reviews: Sequence[Mapping[str, object]],
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    return bool(
        _review_evidence_completion_times(
            reviews, comments, review_bot, head, not_before, before
        )
    )


def _no_issues_evidence_is_reusable_for_final(
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    body_sha256: str,
    initial: _ReviewMarker | None,
    final: _ReviewMarker | None,
    issue_updated_at: datetime | None,
    *,
    has_unresolved_threads: bool,
) -> bool:
    """Allow a duplicate-suppressed no-issues result to satisfy final review.

    Codex deliberately suppresses a second identical no-issues response for
    the same HEAD.  Reuse is safe only when both marker phases attest to that
    unchanged HEAD/body contract, no review thread is open, and the result was
    recorded after every relevant Issue update but before the final marker.
    """

    if (
        initial is None
        or final is None
        or has_unresolved_threads
        or initial[1] != head
        or final[1] != head
        or initial[2] != body_sha256
        or final[2] != body_sha256
    ):
        return False
    initial_time = _marker_updated_time(initial[3])
    final_time = _marker_updated_time(final[3])
    if initial_time is None or final_time is None or initial_time >= final_time:
        return False
    freshness_floor = max(initial_time, issue_updated_at or initial_time)
    return bool(
        _no_issues_comment_completion_times(
            comments,
            review_bot,
            head,
            freshness_floor.isoformat(),
            _marker_updated_at(final[3]),
        )
    )


def _marker_updated_at(comment: Mapping[str, object]) -> object:
    """Return the marker's latest edit time for evidence freshness checks."""

    if "updated_at" in comment:
        return comment["updated_at"]
    if "updatedAt" in comment:
        return comment["updatedAt"]
    # Keep accepting legacy fixtures/API projections that never exposed the
    # edit field; an explicitly present null or invalid value still fails closed.
    return comment.get("created_at")


def _marker_updated_time(comment: Mapping[str, object]) -> datetime | None:
    try:
        return _timestamp(_marker_updated_at(comment), "marker updated_at")
    except (TypeError, ValueError):
        return None


def _body_sha256(body: object) -> str | None:
    """Return the exact UTF-8 PR-body digest used by review evidence."""

    if not isinstance(body, str) or "\x00" in body:
        return None
    try:
        encoded = body.encode("utf-8", "strict")
    except UnicodeEncodeError:
        return None
    return hashlib.sha256(encoded).hexdigest()


def _latest_issue_updated_time(
    referenced_issues: Sequence[issue_contract.Issue],
) -> datetime | None:
    """Return the newest referenced Issue edit time, failing closed per item."""

    if not referenced_issues:
        return None
    timestamps: list[datetime] = []
    for issue in referenced_issues:
        try:
            updated_at = _timestamp(issue.updated_at, f"Issue #{issue.number} updated_at")
        except (TypeError, ValueError):
            return None
        if updated_at is None:
            return None
        timestamps.append(updated_at)
    return max(timestamps)


def closing_reference_errors(
    *,
    repository: str,
    body: object,
    referenced_issues: Sequence[issue_contract.Issue],
) -> list[str]:
    """Require one canonical open Issue in both commits and the PR body."""

    if not isinstance(body, str):
        raise TypeError("pull request body must be a string")
    errors: list[str] = []
    if len(referenced_issues) != 1:
        errors.append(
            "commit範囲の参照Issueはちょうど1件のOPEN Issueである必要があります: "
            f"{len(referenced_issues)}件"
        )
    else:
        issue = referenced_issues[0]
        if type(issue.number) is not int or issue.number < 1:
            errors.append("commit範囲のcanonical Issue番号が不正です")
        elif issue.state != "OPEN":
            errors.append(
                "commit範囲の参照IssueはOPENである必要があります: "
                f"#{issue.number}={issue.state}"
            )
        elif (
            not isinstance(issue.url, str)
            or issue.url.casefold()
            != f"https://github.com/{repository}/issues/{issue.number}".casefold()
        ):
            errors.append(
                f"Issue #{issue.number}は対象repositoryのcanonical Issue URLではありません"
            )
    referenced_numbers = {issue.number for issue in referenced_issues}
    closing_numbers = issue_contract.closing_issue_numbers(body, repository)
    missing = sorted(referenced_numbers - closing_numbers)
    extra = sorted(closing_numbers - referenced_numbers)
    if missing or extra:
        details: list[str] = []
        if missing:
            details.append(
                "不足=" + ", ".join(f"#{number}" for number in missing)
            )
        if extra:
            details.append(
                "余分=" + ", ".join(f"#{number}" for number in extra)
            )
        errors.append(
            "PR本文のGitHub closing Issue集合がcommit範囲参照Issue集合と一致しません: "
            + "; ".join(details)
        )
    return errors


def _open_pull_requests(repository: str) -> list[dict[str, object]]:
    """Read every open PR body through GraphQL cursor pagination."""

    owner, name = repository.split("/", maxsplit=1)
    pull_requests: list[dict[str, object]] = []
    seen_numbers: set[int] = set()
    governed_heads: set[str] = set()
    snapshot_default_branch: str | None = None
    seen_cursors: set[str] = set()
    cursor: str | None = None
    page_count = 0
    first_payload: object | None = None
    first_arguments: list[str] | None = None
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            "query="
            "query($owner: String!, $name: String!, $cursor: String) {\n"
            "  rateLimit { cost remaining resetAt }\n"
            "  repository(owner: $owner, name: $name) {\n"
            "    defaultBranchRef { name }\n"
            "    pullRequests(first: 100, states: OPEN, after: $cursor) {\n"
            "      nodes { number isDraft body baseRefName headRefOid headRepository { nameWithOwner } }\n"
            "      pageInfo { hasNextPage endCursor }\n"
            "    }\n"
            "  }\n"
            "}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        payload = _gh_json(*arguments)
        if page_count == 0:
            first_payload = payload
            first_arguments = list(arguments)
        if not isinstance(payload, Mapping):
            raise TypeError("open pull requests response must be an object")
        data = payload.get("data")
        if not isinstance(data, Mapping):
            raise TypeError("open pull requests data must be an object")
        repository_data = data.get("repository")
        if not isinstance(repository_data, Mapping):
            raise TypeError("open pull requests repository must be an object")
        default_ref = repository_data.get("defaultBranchRef")
        default_branch = default_ref.get("name") if isinstance(default_ref, Mapping) else None
        if not _safe_branch_name(default_branch):
            raise TypeError("open pull requests default branch is invalid")
        if snapshot_default_branch is None:
            snapshot_default_branch = default_branch
        elif default_branch != snapshot_default_branch:
            raise TypeError("open pull requests default branch changed during pagination")
        connection = repository_data.get("pullRequests")
        if not isinstance(connection, Mapping):
            raise TypeError("open pull requests connection must be an object")
        nodes = connection.get("nodes")
        if not isinstance(nodes, list):
            raise TypeError("open pull requests nodes must be an array")
        if len(nodes) > _MAX_REST_PAGE_ITEMS:
            raise ValueError(
                "open pull requests page exceeds the item limit "
                f"({_MAX_REST_PAGE_ITEMS})"
            )
        page_count += 1
        if page_count > _MAX_REST_PAGES:
            raise ValueError(
                "open pull requests exceed the page limit "
                f"({_MAX_REST_PAGES})"
            )
        for node in nodes:
            if not isinstance(node, Mapping):
                raise TypeError("open pull request must be an object")
            number = node.get("number")
            is_draft = node.get("isDraft")
            body = node.get("body")
            base_ref = node.get("baseRefName")
            head = node.get("headRefOid")
            head_repository = node.get("headRepository")
            if type(number) is not int or number < 1:
                raise TypeError("open pull request number must be a positive integer")
            if number in seen_numbers:
                raise TypeError("open pull requests contain a duplicate number")
            if not isinstance(is_draft, bool):
                raise TypeError("open pull request isDraft must be a boolean")
            if not isinstance(body, str):
                raise TypeError("open pull request body must be a string")
            if "headRepository" not in node:
                raise TypeError("open pull request headRepository is missing")
            seen_numbers.add(number)
            # GitHub can return null when the head repository was deleted or
            # is otherwise unavailable.  It cannot be a governed PR, so
            # scope it out before validating governance-only metadata.
            if head_repository is None:
                continue
            if not _safe_branch_name(base_ref):
                raise TypeError("open pull request baseRefName is invalid")
            if not isinstance(head, str) or _SHA.fullmatch(head) is None:
                raise TypeError("open pull request headRefOid is invalid")
            if not isinstance(head_repository, Mapping):
                raise TypeError("open pull request headRepository is invalid")
            head_repository_name = head_repository.get("nameWithOwner")
            if not isinstance(head_repository_name, str) or not head_repository_name:
                raise TypeError("open pull request headRepository name is invalid")
            # Only same-repository PRs targeting the default branch are
            # governed.  Two such PRs sharing a head would share one Check
            # Run namespace, so readiness must stop before a stale success
            # can satisfy either PR.
            if base_ref != default_branch or head_repository_name != repository:
                continue
            normalized_head = head.lower()
            if normalized_head in governed_heads:
                raise TypeError("open pull requests have duplicate governed head SHA")
            governed_heads.add(normalized_head)
            pull_requests.append(
                {"number": number, "isDraft": is_draft, "body": body, "head_sha": head}
            )
        page_info = connection.get("pageInfo")
        if not isinstance(page_info, Mapping):
            raise TypeError("open pull requests pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            if first_payload is None or first_arguments is None:
                raise TypeError("open pull requests initial page is missing")
            reread = _gh_json(*first_arguments)
            if _stable_graphql_payload(reread) != _stable_graphql_payload(first_payload):
                raise ValueError("open pull requests changed during pagination")
            return pull_requests
        if has_next_page is not True:
            raise TypeError("open pull requests hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("open pull requests endCursor must be a unique string")
        seen_cursors.add(cursor)


def _open_pull_request_snapshot(path: str) -> list[dict[str, object]]:
    """Load the immutable single-arbiter open-PR snapshot fail-closed."""
    try:
        with open(path, encoding="utf-8") as source:
            payload = json.load(source)
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("open pull request snapshot is unavailable") from error
    if not isinstance(payload, list):
        raise TypeError("open pull request snapshot must be an array")
    values: list[dict[str, object]] = []
    seen: set[int] = set()
    seen_heads: set[str] = set()
    for item in payload:
        if not isinstance(item, Mapping):
            raise TypeError("open pull request snapshot item must be an object")
        number, draft, body, head = item.get("number"), item.get("isDraft"), item.get("body"), item.get("head_sha")
        if type(number) is not int or number < 1 or number in seen:
            raise TypeError("open pull request snapshot number is invalid")
        if not isinstance(draft, bool) or not isinstance(body, str) or not isinstance(head, str) or _SHA.fullmatch(head) is None:
            raise TypeError("open pull request snapshot item is invalid")
        normalized_head = head.lower()
        if normalized_head in seen_heads:
            raise TypeError("open pull request snapshot has duplicate governed head SHA")
        seen.add(number)
        seen_heads.add(normalized_head)
        values.append({"number": number, "isDraft": draft, "body": body, "head_sha": head})
    return values


def closing_open_pull_request_errors(
    *,
    repository: str,
    current_pull_request: int,
    referenced_issues: Sequence[issue_contract.Issue],
    open_pull_requests: Sequence[Mapping[str, object]],
) -> list[str]:
    """Require the canonical Issue to have this single open PR as its closer.

    Draft PRs are included deliberately: waiting until a sibling is Ready
    leaves a check-to-Ready race in which two Draft PRs can both pass.
    """

    errors: list[str] = []
    for issue in referenced_issues:
        closers = _open_pull_request_closers(
            repository=repository,
            issue_number=issue.number,
            open_pull_requests=open_pull_requests,
        )
        if closers != {current_pull_request}:
            rendered = ", ".join(f"#{number}" for number in sorted(closers)) or "なし"
            errors.append(
                f"Issue #{issue.number}をclosingするopen PRは自身だけである必要があります: {rendered}"
            )
    return errors


def _canonical_issue_identity(
    *,
    repository: str,
    referenced_issues: Sequence[issue_contract.Issue],
) -> tuple[tuple[int, str, str, str, str], ...]:
    """Return the immutable fields used by the final canonical-Issue fence."""

    identity: list[tuple[int, str, str, str, str]] = []
    for issue in referenced_issues:
        number = issue.number
        state = issue.state
        body = issue.body
        url = issue.url
        updated_at = issue.updated_at
        if type(number) is not int or number < 1:
            raise TypeError("canonical Issue number must be a positive integer")
        if not isinstance(state, str) or not state:
            raise TypeError("canonical Issue state must be a non-empty string")
        if not isinstance(body, str):
            raise TypeError("canonical Issue body must be a string")
        if not isinstance(url, str) or not url:
            raise TypeError("canonical Issue url must be a non-empty string")
        canonical_url = f"https://github.com/{repository}/issues/{number}"
        if url.casefold() != canonical_url.casefold():
            raise ValueError("canonical Issue url does not match the repository")
        if not isinstance(updated_at, str) or not updated_at:
            raise TypeError("canonical Issue updated_at must be a non-empty string")
        identity.append((number, state, body, url, updated_at))
    return tuple(identity)


def _required_timestamp_text(value: object, field: str) -> str:
    """Validate and preserve an immutable GitHub timestamp string."""

    if not isinstance(value, str):
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    if _timestamp(value, field) is None:
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    return value


def _final_review_evidence_is_fresh(
    reviews: Sequence[Mapping[str, object]],
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    comment: Mapping[str, object],
    issue_updated_at: datetime | None,
) -> bool:
    """Require a submitted bot review after the final marker and Issue edits.

    Reactions are mutable and therefore cannot serve as durable final-review
    evidence for a required context.
    """

    marker_time = _marker_updated_time(comment)
    if marker_time is None:
        return False
    try:
        marker_created_at = _timestamp(comment.get("created_at"), "marker created_at")
    except (TypeError, ValueError):
        return False
    if marker_created_at is None or marker_time < marker_created_at:
        return False
    freshness_floor = max(
        marker_time,
        issue_updated_at or marker_time,
    )
    review_times = _review_evidence_completion_times(
        reviews, comments, review_bot, head, _marker_updated_at(comment)
    )
    return any(submitted_at > freshness_floor for submitted_at in review_times)


def _pull_request_author_login(pull_request: Mapping[str, object]) -> str:
    """Return the required PR-author login or fail closed."""

    author_login = _bot_login(pull_request.get("author"))
    if author_login is None or not author_login:
        raise TypeError("pull request author.login must be a non-empty string")
    return author_login


def _marker_comment_identity(comment: Mapping[str, object]) -> tuple[str, str] | None:
    """Read a marker author's REST or GraphQL identity without trusting malformed data."""

    if "user" in comment:
        author = comment["user"]
        association = comment.get("author_association")
    elif "author" in comment:
        author = comment["author"]
        association = comment.get("authorAssociation")
    else:
        return None
    login = _bot_login(author)
    if login is None or not login or not isinstance(association, str):
        return None
    return login, association.upper()


def _review_markers(
    comments: Sequence[Mapping[str, object]], pull_request_author: str
) -> list[_ReviewMarker]:
    """Return only review markers controlled by the PR author or a maintainer."""

    if not isinstance(pull_request_author, str) or not pull_request_author:
        raise TypeError("pull request author.login must be a non-empty string")
    markers: list[_ReviewMarker] = []
    for comment in comments:
        body = comment["body"]
        if not isinstance(body, str):
            raise TypeError("comment body must be a string")
        if _CODEX_REVIEW_TRIGGER.search(body) is None:
            continue
        for match in _MARKER_PATTERN.finditer(body):
            identity = _marker_comment_identity(comment)
            if identity is None:
                continue
            login, association = identity
            if (
                login.casefold() != pull_request_author.casefold()
                and association not in _TRUSTED_MARKER_ASSOCIATIONS
            ):
                continue
            markers.append(
                (
                    match.group("phase"),
                    match.group("head").lower(),
                    match.group("body_sha256"),
                    comment,
                )
            )
    return markers


def _review_marker_identities(
    markers: Sequence[_ReviewMarker],
) -> tuple[_ReviewMarkerIdentity, ...]:
    """Make the accepted marker set comparable at the final readiness fence."""

    identities: list[_ReviewMarkerIdentity] = []
    for phase, head, body_sha256, comment in markers:
        comment_id = comment.get("id")
        body = comment.get("body")
        created_at = comment.get("created_at")
        updated_at = _marker_updated_at(comment)
        identity = _marker_comment_identity(comment)
        if (
            type(comment_id) is not int
            or not isinstance(body, str)
            or not isinstance(created_at, str)
            or not isinstance(updated_at, str)
            or identity is None
        ):
            raise TypeError("trusted review marker identity is malformed")
        login, association = identity
        identities.append(
            (
                comment_id,
                phase,
                head,
                body_sha256,
                login.casefold(),
                association,
                created_at,
                updated_at,
            )
        )
    return tuple(sorted(identities))


def _resolved_thread_has_author_reply(
    thread: Mapping[str, object], author_login: str
) -> bool:
    """Return whether a resolved review thread has a reply from the PR author.

    The first thread comment is the review root.  A response on the root itself
    is not evidence that its author addressed the finding, so only later
    comments count.
    """

    comments = thread.get("comments")
    if not isinstance(comments, Sequence) or isinstance(comments, (str, bytes)):
        return False
    if not comments:
        return False
    for comment in comments[1:]:
        if not isinstance(comment, Mapping):
            raise TypeError("review thread comment must be an object")
        comment_author = comment.get("author")
        comment_login = _bot_login(comment_author)
        if comment_login is None:
            continue
        if comment_login.casefold() == author_login.casefold():
            return True
        association = comment.get("authorAssociation")
        if (
            isinstance(association, str)
            and association.upper() in _TRUSTED_REPLY_ASSOCIATIONS
        ):
            return True
    return False


def _is_self_check(check: Mapping[str, object]) -> bool:
    """Exclude only the governance contexts that would otherwise self-cycle."""

    return check.get("name", check.get("context")) in _SELF_CHECK_NAMES


def _required_status_check_snapshot(
    repository: str, base_branch: object
) -> _RequiredStatusChecks:
    """Read one strict, internally consistent required-check configuration."""

    if not _safe_branch_name(base_branch):
        raise TypeError("baseRefName must be a safe branch name")
    protection = _gh_json(
        "api",
        f"repos/{repository}/branches/{base_branch}/protection/required_status_checks",
    )
    if not isinstance(protection, Mapping):
        raise TypeError("required status checks response must be an object")
    if protection.get("strict") is not True:
        raise ValueError("branch protection required status checks strict must be true")
    raw_contexts = protection.get("contexts")
    raw_checks = protection.get("checks")
    if not isinstance(raw_contexts, list) or not isinstance(raw_checks, list):
        raise TypeError("required status checks contexts and checks must be arrays")
    contexts: list[str] = []
    for context in raw_contexts:
        if not isinstance(context, str) or not context:
            raise TypeError("required status check context must be a non-empty string")
        contexts.append(context)
    checks: list[tuple[str, int | None]] = []
    for raw_check in raw_checks:
        if not isinstance(raw_check, Mapping):
            raise TypeError("required status check must be an object")
        context = raw_check.get("context")
        app_id = raw_check.get("app_id")
        if not isinstance(context, str) or not context:
            raise TypeError("required status check context must be a non-empty string")
        if app_id is not None and (type(app_id) is not int or app_id < 1):
            raise TypeError("required status check app_id must be null or a positive integer")
        checks.append((context, app_id))
    check_contexts = [context for context, _app_id in checks]
    if len(contexts) != len(set(contexts)) or len(check_contexts) != len(set(check_contexts)):
        raise ValueError("branch protection required status check contexts contain duplicates")
    if set(contexts) != set(check_contexts):
        raise ValueError("branch protection required status check contexts and checks differ")
    return tuple(sorted(contexts)), tuple(sorted(checks))


def _governance_app_binding_error(required_checks: _RequiredStatusChecks) -> str | None:
    """Validate the two required governance contexts and their immutable Apps."""

    _contexts, checks = required_checks
    bindings = dict(checks)
    trusted_app = bindings.get(_TRUSTED_CHECK)
    if type(trusted_app) is not int or trusted_app < 1:
        return "branch protection trusted Check Run App binding is missing or ambiguous"
    if bindings.get(_LATCH_CHECK) != 15368:
        return "branch protection review latch App binding is not exact"
    return None


def _status_check_context(check: Mapping[str, object]) -> str:
    """Return one unambiguous CheckRun or StatusContext name."""

    name = check.get("name")
    context = check.get("context")
    if name is not None and context is not None and name != context:
        raise TypeError("status check name and context must agree")
    value = name if name is not None else context
    if not isinstance(value, str) or not value:
        raise TypeError("status check name must be a non-empty string")
    return value


def _check_error(check: Mapping[str, object]) -> str | None:
    name = _status_check_context(check)
    if _is_self_check(check):
        return None
    if not isinstance(name, str) or not name:
        raise TypeError("status check name must be a string")

    status = check.get("status")
    conclusion = check.get("conclusion")
    if status == "COMPLETED" and isinstance(conclusion, str):
        if conclusion.upper() in _SUCCESSFUL_CONCLUSIONS:
            return None
    # StatusContext values are returned as a terminal state instead of a
    # CheckRun status/conclusion pair by some gh versions.
    state = check.get("state")
    if isinstance(state, str) and state.upper() in _SUCCESSFUL_CONCLUSIONS:
        return None
    return f"CI check が未完了または失敗しています: {name}"


def _status_check_rollup_errors(
    status_rollup: object,
    required_checks: _RequiredStatusChecks | None = None,
) -> list[str]:
    """Validate required independent CI evidence from one immutable API read."""

    if not isinstance(status_rollup, Sequence) or isinstance(
        status_rollup, (str, bytes)
    ):
        raise TypeError("statusCheckRollup must be a sequence")
    typed_rollup: list[Mapping[str, object]] = []
    for check in status_rollup:
        if not isinstance(check, Mapping):
            raise TypeError("status check must be an object")
        typed_rollup.append(check)
    if required_checks is None:
        errors: list[str] = []
        for check in typed_rollup:
            if _is_self_check(check):
                continue
            error = _check_error(check)
            if error is not None:
                errors.append(error)
        if not any(not _is_self_check(check) for check in typed_rollup):
            errors.append("CI check を取得できません")
        return errors

    required_contexts = {
        context
        for context, _app_id in required_checks[1]
        if context not in _SELF_CHECK_NAMES
    }
    matching: dict[str, list[Mapping[str, object]]] = {
        context: [] for context in required_contexts
    }
    for check in typed_rollup:
        context = _status_check_context(check)
        if context in matching:
            matching[context].append(check)
    errors = []
    required_app_ids = dict(required_checks[1])
    for context in sorted(required_contexts):
        checks = matching[context]
        if len(checks) != 1:
            errors.append(
                f"required CI check がちょうど1件ではありません: {context} ({len(checks)}件)"
            )
            continue
        check = checks[0]
        app_id = required_app_ids[context]
        is_legacy_status = check.get("__typename") == "StatusContext"
        if app_id is None and is_legacy_status:
            # Classic required contexts have no producer App binding. GitHub
            # exposes their terminal commit status as a StatusContext rather
            # than a CheckRun in statusCheckRollup; accept only that exact
            # terminal-success representation. App-bound contexts remain
            # CheckRun-only and are validated by the producer fence below.
            if check.get("state") == "SUCCESS":
                continue
        if is_legacy_status or check.get("status") != "COMPLETED" or check.get("conclusion") != "SUCCESS":
            errors.append(f"required CI check がcompleted-successではありません: {context}")
    return errors


def _check_run_details_url(value: object, field: str) -> str | None:
    """Accept a canonical HTTPS details URL or an omitted optional URL."""

    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise TypeError(f"{field} must be a non-empty HTTPS URL")
    parsed = urlparse(value)
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ValueError(f"{field} must be a canonical HTTPS URL")
    return value


def _rollup_check_run_identity(check: Mapping[str, object]) -> tuple[int | None, str | None]:
    """Return immutable Check Run identity exposed by one rollup entry."""

    if check.get("__typename") != "CheckRun":
        raise TypeError("app-bound required status context must be a CheckRun")
    database_id = check.get("databaseId")
    raw_id = check.get("id")
    if database_id is not None and (type(database_id) is not int or database_id < 1):
        raise TypeError("rollup CheckRun databaseId must be a positive integer")
    if raw_id is not None and not (
        type(raw_id) is int or (isinstance(raw_id, str) and raw_id)
    ):
        raise TypeError("rollup CheckRun id must be a positive integer or opaque ID")
    if type(raw_id) is int and raw_id < 1:
        raise TypeError("rollup CheckRun id must be a positive integer")
    if database_id is not None and type(raw_id) is int and raw_id != database_id:
        raise ValueError("rollup CheckRun immutable IDs disagree")
    immutable_id = database_id if database_id is not None else raw_id
    if immutable_id is not None and type(immutable_id) is not int:
        immutable_id = None

    details_url = check.get("detailsUrl")
    alternate_details_url = check.get("details_url")
    if details_url is not None and alternate_details_url is not None and details_url != alternate_details_url:
        raise ValueError("rollup CheckRun details URLs disagree")
    if details_url is None:
        details_url = alternate_details_url
    details_url = _check_run_details_url(details_url, "rollup CheckRun details URL")
    if immutable_id is None and details_url is None:
        raise ValueError("rollup CheckRun lacks immutable ID and details URL")
    return immutable_id, details_url


def _required_check_run_producer_errors(
    repository: str,
    head: str,
    status_rollup: object,
    required_checks: _RequiredStatusChecks,
) -> list[str]:
    """Bind every App-bound required rollup success to its exact producer."""

    if not isinstance(status_rollup, Sequence) or isinstance(status_rollup, (str, bytes)):
        raise TypeError("statusCheckRollup must be a sequence")
    if re.fullmatch(r"[0-9a-fA-F]{40}", head) is None:
        raise ValueError("required Check Run producer head must be a 40-character SHA")
    typed_rollup: list[Mapping[str, object]] = []
    for check in status_rollup:
        if not isinstance(check, Mapping):
            raise TypeError("status check must be an object")
        typed_rollup.append(check)

    errors: list[str] = []
    for context, app_id in required_checks[1]:
        if context in _SELF_CHECK_NAMES or app_id is None:
            continue
        matches = [
            check for check in typed_rollup if _status_check_context(check) == context
        ]
        if len(matches) != 1:
            errors.append(
                f"required CI Check Run producer がちょうど1件ではありません: {context} ({len(matches)}件)"
            )
            continue
        try:
            rollup_id, rollup_url = _rollup_check_run_identity(matches[0])
            pages = _rest_pages(
                f"repos/{repository}/commits/{head}/check-runs?"
                f"check_name={quote(context, safe='')}&app_id={app_id}&filter=all&per_page=100",
                item_key="check_runs",
            )
            candidates: list[Mapping[str, object]] = []
            for page in pages:
                page_runs = page.get("check_runs")
                if not isinstance(page_runs, list) or not all(
                    isinstance(run, Mapping) for run in page_runs
                ):
                    raise TypeError("required Check Run page must contain an array")
                for run in page_runs:
                    if run.get("name") != context:
                        continue
                    run_head = run.get("head_sha")
                    run_app = run.get("app")
                    run_id = run.get("id")
                    run_url = _check_run_details_url(
                        run.get("details_url"), "required Check Run details URL"
                    )
                    if (
                        not isinstance(run_head, str)
                        or re.fullmatch(r"[0-9a-fA-F]{40}", run_head) is None
                        or run_head.casefold() != head.casefold()
                        or not isinstance(run_app, Mapping)
                        or type(run_app.get("id")) is not int
                        or run_app.get("id") != app_id
                        or type(run_id) is not int
                        or run_id < 1
                    ):
                        raise ValueError("required Check Run producer binding is malformed")
                    candidates.append(run)
            bound = [
                run
                for run in candidates
                if (rollup_id is None or run.get("id") == rollup_id)
                and (rollup_url is None or run.get("details_url") == rollup_url)
            ]
            if len(bound) != 1:
                raise ValueError(
                    "required Check Run producer binding is missing or ambiguous"
                )
            if (
                bound[0].get("status") != "completed"
                or bound[0].get("conclusion") != "success"
            ):
                raise ValueError("required Check Run producer is not completed-success")
        except (TypeError, ValueError) as error:
            errors.append(f"required CI Check Run producer is invalid for {context}: {error}")
    return errors


def readiness_errors(
    pull_request: Mapping[str, object],
    threads: Sequence[Mapping[str, object]],
    comments: Sequence[Mapping[str, object]],
    review_bot: str,
    require_draft: bool,
    referenced_issues: Sequence[issue_contract.Issue] = (),
    required_checks: _RequiredStatusChecks | None = None,
) -> list[str]:
    """Return every unmet PR readiness condition without changing GitHub state."""

    errors: list[str] = []
    head = pull_request["headRefOid"]
    if not isinstance(head, str) or re.fullmatch(r"[0-9a-fA-F]{40}", head) is None:
        raise ValueError("pull request headRefOid must be a 40-character SHA")
    head = head.lower()

    is_draft = pull_request["isDraft"]
    if not isinstance(is_draft, bool):
        raise TypeError("pull request isDraft must be a boolean")
    if require_draft and not is_draft:
        errors.append("Draft PR でのみ readiness gate を実行できます")

    status_rollup = pull_request["statusCheckRollup"]
    errors.extend(_status_check_rollup_errors(status_rollup, required_checks))

    unresolved = [thread for thread in threads if thread.get("isResolved") is not True]
    if unresolved:
        errors.append(f"未resolve review thread が {len(unresolved)} 件あります")

    author_login = _pull_request_author_login(pull_request)
    missing_replies = [
        thread
        for thread in threads
        if thread.get("isResolved") is True
        and not _resolved_thread_has_author_reply(thread, author_login)
    ]
    if missing_replies:
        errors.append(
            f"resolve 済み review thread に PR author の reply が {len(missing_replies)} 件ありません"
        )

    reviews = pull_request["reviews"]
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise TypeError("reviews must be a sequence")
    typed_reviews: list[Mapping[str, object]] = []
    for review in reviews:
        if not isinstance(review, Mapping):
            raise TypeError("review must be an object")
        typed_reviews.append(review)

    issue_updated_at = _latest_issue_updated_time(referenced_issues)
    if referenced_issues and issue_updated_at is None:
        errors.append("参照Issue snapshotのupdated_atが不正です")
    body_sha256 = _body_sha256(pull_request.get("body"))
    if body_sha256 is None:
        errors.append("PR本文の review marker digestを検証できません")

    markers = _review_markers(comments, author_login)
    initial_markers = [marker for marker in markers if marker[0] == "initial"]
    final_markers = [marker for marker in markers if marker[0] == "final"]

    latest_initial: _ReviewMarker | None = None
    if not initial_markers:
        errors.append("initial review marker がありません")
    else:
        initial_times = [
            _marker_updated_time(marker[3]) for marker in initial_markers
        ]
        if any(marker_time is None for marker_time in initial_times):
            errors.append("initial review marker の時刻が不正です")
        else:
            latest_initial_time = max(initial_times)
            latest_initial_candidates = [
                marker
                for marker, marker_time in zip(initial_markers, initial_times)
                if marker_time == latest_initial_time
            ]
            if len(latest_initial_candidates) != 1:
                errors.append("initial review marker の最新時刻が曖昧です")
            else:
                latest_initial = latest_initial_candidates[0]

    if latest_initial is not None:
        if latest_initial[1] != head:
            errors.append("initial review marker が最新HEADを指していません")
        if body_sha256 is None or latest_initial[2] != body_sha256:
            errors.append("initial review marker のPR本文digestが現在値と一致しません")

    latest_final: _ReviewMarker | None = None
    if final_markers:
        # The newest final marker is authoritative even when it names an old
        # head.  Ignoring a later old-head marker would let an earlier current
        # marker continue to authorize evidence after the review contract was
        # changed.
        marker_times = [
            _marker_updated_time(marker[3]) for marker in final_markers
        ]
        if any(marker_time is None for marker_time in marker_times):
            final_markers = []
        else:
            latest_final_time = max(marker_times)
            latest_final_candidates = [
                marker
                for marker, marker_time in zip(final_markers, marker_times)
                if marker_time == latest_final_time
            ]
            if len(latest_final_candidates) == 1:
                latest_final = latest_final_candidates[0]

    if latest_initial is not None and latest_final is not None:
        if _no_issues_evidence_is_ambiguous(
            comments,
            review_bot,
            latest_initial[1],
            _marker_updated_at(latest_initial[3]),
            _marker_updated_at(latest_final[3]),
        ):
            errors.append("initial-final間の no-issues review evidence が曖昧です")
    if latest_final is not None and _no_issues_evidence_is_ambiguous(
        comments,
        review_bot,
        head,
        _marker_updated_at(latest_final[3]),
    ):
        errors.append("final review後の no-issues review evidence が曖昧です")

    if not final_markers:
        errors.append("final review marker がありません")
    elif latest_final is None:
        errors.append("final review marker の最新時刻が曖昧です")
    elif latest_final[1] != head:
        errors.append("final review marker が最新HEADを指していません")
    elif body_sha256 is None or latest_final[2] != body_sha256:
        errors.append("final review marker のPR本文digestが現在値と一致しません")
    elif issue_updated_at is not None and (
        _marker_updated_time(latest_final[3]) is None
        or _marker_updated_time(latest_final[3]) <= issue_updated_at
    ):
        errors.append("final review marker が参照Issue更新後ではありません")
    elif not _final_review_evidence_is_fresh(
            typed_reviews,
            comments,
            review_bot,
            head,
            latest_final[3],
            issue_updated_at,
        ) and not _no_issues_evidence_is_reusable_for_final(
            comments,
            review_bot,
            head,
            body_sha256,
            latest_initial,
            latest_final,
            issue_updated_at,
            has_unresolved_threads=bool(unresolved),
        ):
        errors.append("final review に参照Issue更新後の review bot 完了記録がありません")

    if latest_initial is not None and latest_final is not None:
        initial_time = _marker_updated_time(latest_initial[3])
        final_time = _marker_updated_time(latest_final[3])
        if initial_time is None or final_time is None or initial_time >= final_time:
            errors.append("initial review marker は最新 final review marker より前である必要があります")
        elif not _review_evidence_is_for(
            typed_reviews,
            comments,
            review_bot,
            latest_initial[1],
            _marker_updated_at(latest_initial[3]),
            _marker_updated_at(latest_final[3]),
        ):
            errors.append("initial review に review bot の完了記録がありません")

    return errors


def _graphql_reported_cost(payload: object) -> int:
    if not isinstance(payload, Mapping):
        raise TypeError("GraphQL response must be an object")
    data = payload.get("data")
    rate_limit = data.get("rateLimit") if isinstance(data, Mapping) else None
    cost = rate_limit.get("cost") if isinstance(rate_limit, Mapping) else None
    if type(cost) is not int or cost < 1 or cost > _GRAPHQL_MAX_QUERY_COST:
        raise ValueError("GraphQL rateLimit cost is outside the query budget")
    return cost


def _gh_json(*arguments: str) -> Any:
    global _ACTIVE_REST_BUDGET, _ACTIVE_GRAPHQL_BUDGET, _ACTIVE_SHARED_GRAPHQL_LEDGER
    rest_get = len(arguments) >= 2 and arguments[0] == "api" and arguments[1] not in {
        "graphql",
        "rate_limit",
    }
    if rest_get and _ACTIVE_REST_BUDGET is None:
        budget = _RestBudget()
        _ACTIVE_REST_BUDGET = budget
        budget.observe_rate_limit(_gh_json("api", "rate_limit"))
    if rest_get and _ACTIVE_REST_BUDGET is not None:
        _ACTIVE_REST_BUDGET.consume()
    graphql_entry = len(arguments) >= 2 and (
        arguments[:2] == ("pr", "view")
        or arguments[:2] == ("repo", "view")
        or arguments[:2] == ("api", "graphql")
    )
    is_preflight = f"query={_GRAPHQL_PREFLIGHT_QUERY}" in arguments
    if graphql_entry and _ACTIVE_GRAPHQL_BUDGET is None:
        budget = _GraphQLBudget()
        _ACTIVE_GRAPHQL_BUDGET = budget
        if not is_preflight:
            preflight = _gh_json(
                "api",
                "graphql",
                "-f",
                f"query={_GRAPHQL_PREFLIGHT_QUERY}",
            )
            budget.observe(preflight)
    if graphql_entry and _ACTIVE_GRAPHQL_BUDGET is not None:
        _ACTIVE_GRAPHQL_BUDGET.before_request()
    shared_reservation = (
        _ACTIVE_SHARED_GRAPHQL_LEDGER.reserve()
        if graphql_entry and _ACTIVE_SHARED_GRAPHQL_LEDGER is not None
        else None
    )
    completed = subprocess.run(
        ["gh", *arguments],
        check=True,
        capture_output=True,
        text=True,
        timeout=20,
    )
    payload = json.loads(completed.stdout)
    if arguments[:2] == ("api", "graphql") and _ACTIVE_GRAPHQL_BUDGET is not None:
        _ACTIVE_GRAPHQL_BUDGET.observe(payload)
        if shared_reservation is not None and _ACTIVE_SHARED_GRAPHQL_LEDGER is not None:
            _ACTIVE_SHARED_GRAPHQL_LEDGER.settle(
                shared_reservation, _graphql_reported_cost(payload)
            )
    elif arguments[:2] in {("pr", "view"), ("repo", "view")} and _ACTIVE_GRAPHQL_BUDGET is not None:
        # These ``gh`` projections are GraphQL-backed reads but do not expose
        # rateLimit metadata in its JSON projection; charge its conservative
        # one-point primary query cost after the preflight reservation.
        _ACTIVE_GRAPHQL_BUDGET.consume()
        if shared_reservation is not None and _ACTIVE_SHARED_GRAPHQL_LEDGER is not None:
            _ACTIVE_SHARED_GRAPHQL_LEDGER.settle(shared_reservation, 1)
    return payload


def _repository_name(repository: str | None) -> str:
    if repository is not None:
        return repository
    payload = _gh_json("repo", "view", "--json", "nameWithOwner")
    return payload["nameWithOwner"]


def _verify_pr_boundary_unchanged(
    repository: str,
    pull_request: int,
    initial_base: str,
    initial_head: str,
) -> None:
    """Fail closed if the PR boundary moved while readiness was evaluated."""

    payload = _gh_json(
        "pr",
        "view",
        str(pull_request),
        "--repo",
        repository,
        "--json",
        "baseRefOid,headRefOid",
    )
    if not isinstance(payload, dict):
        raise TypeError("pull request boundary response must be an object")
    current_base = payload.get("baseRefOid")
    current_head = payload.get("headRefOid")
    if not isinstance(current_base, str) or not isinstance(current_head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    if (current_base, current_head) != (initial_base, initial_head):
        raise ValueError("pull request base/head changed during readiness check")


def _verify_final_readiness_snapshot_unchanged(
    *,
    repository: str,
    pull_request: int,
    initial_base: str,
    initial_head: str,
    initial_base_branch: str,
    initial_body: str,
    initial_updated_at: str,
    initial_required_checks: _RequiredStatusChecks,
    initial_issue_identity: tuple[tuple[int, str, str, str, str], ...],
    initial_closers: frozenset[int],
    open_pull_requests: Sequence[Mapping[str, object]] | None = None,
) -> None:
    """Fence every mutable input that justified a successful readiness result."""

    payload = _gh_json(
        "pr",
        "view",
        str(pull_request),
        "--repo",
        repository,
        "--json",
        "baseRefOid,headRefOid,baseRefName,body,updatedAt,statusCheckRollup",
    )
    if not isinstance(payload, dict):
        raise TypeError("pull request final snapshot response must be an object")
    current_base = payload.get("baseRefOid")
    current_head = payload.get("headRefOid")
    current_base_branch = payload.get("baseRefName")
    current_body = payload.get("body")
    current_updated_at = _required_timestamp_text(
        payload.get("updatedAt"), "pull request updatedAt"
    )
    if (
        not isinstance(current_base, str)
        or not isinstance(current_head, str)
        or not isinstance(current_base_branch, str)
    ):
        raise TypeError("pull request baseRefOid/headRefOid/baseRefName must be strings")
    if not isinstance(current_body, str):
        raise TypeError("pull request body must be a string")
    if (current_base, current_head) != (initial_base, initial_head):
        raise ValueError("pull request base/head changed during readiness check")
    if current_body != initial_body:
        raise ValueError("pull request body changed during readiness check")
    if current_updated_at != initial_updated_at:
        raise ValueError("pull request updatedAt changed during readiness check")
    if current_base_branch != initial_base_branch:
        raise ValueError("pull request base branch changed during readiness check")
    final_required_checks = _required_status_check_snapshot(
        repository, initial_base_branch
    )
    if final_required_checks != initial_required_checks:
        raise ValueError("required status checks changed during readiness check")
    final_ci_errors = _status_check_rollup_errors(
        payload.get("statusCheckRollup"), final_required_checks
    )
    if final_ci_errors:
        raise ValueError(
            "CI status changed during readiness check: " + "; ".join(final_ci_errors)
        )
    final_producer_errors = _required_check_run_producer_errors(
        repository,
        initial_head,
        payload.get("statusCheckRollup"),
        final_required_checks,
    )
    if final_producer_errors:
        raise ValueError(
            "required CI Check Run producer changed during readiness check: "
            + "; ".join(final_producer_errors)
        )

    current_issues = issue_contract.referenced_issue_snapshot(
        repository=repository,
        base_sha=initial_base,
        head_sha=initial_head,
    )
    current_issue_identity = _canonical_issue_identity(
        repository=repository,
        referenced_issues=current_issues,
    )
    if current_issue_identity != initial_issue_identity:
        raise ValueError("canonical Issue snapshot changed during readiness check")
    if len(current_issues) != 1:
        raise ValueError("canonical Issue snapshot is no longer exactly one Issue")

    current_closers = frozenset(
        number
        for number in _open_pull_request_closers(
            repository=repository,
            issue_number=current_issues[0].number,
            open_pull_requests=(
                list(open_pull_requests)
                if open_pull_requests is not None
                else _open_pull_requests(repository)
            ),
        )
    )
    if current_closers != initial_closers:
        raise ValueError("open PR closer set changed during readiness check")


def _open_pull_request_closers(
    *,
    repository: str,
    issue_number: int,
    open_pull_requests: Sequence[Mapping[str, object]],
) -> set[int]:
    """Return every open PR that closes one Issue, including Draft PRs."""

    closers: set[int] = set()
    for pull_request in open_pull_requests:
        number = pull_request.get("number")
        is_draft = pull_request.get("isDraft")
        body = pull_request.get("body")
        if type(number) is not int or number < 1:
            raise TypeError("open pull request number must be a positive integer")
        if not isinstance(is_draft, bool):
            raise TypeError("open pull request isDraft must be a boolean")
        if not isinstance(body, str):
            raise TypeError("open pull request body must be a string")
        if issue_number in issue_contract.closing_issue_numbers(body, repository):
            closers.add(number)
    return closers


def _expected_boundary(
    expected_base: str | None, expected_head: str | None
) -> tuple[str, str] | None:
    """Validate an optional caller-supplied immutable PR boundary."""

    if (expected_base is None) != (expected_head is None):
        raise ValueError("expected base and head SHA must be provided together")
    if expected_base is None:
        return None
    if (
        re.fullmatch(r"[0-9a-fA-F]{40}", expected_base) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", expected_head) is None
    ):
        raise ValueError("expected base and head SHA must be 40-character SHAs")
    return expected_base.lower(), expected_head.lower()


def _require_boundary(
    base: object, head: object, expected: tuple[str, str] | None
) -> tuple[str, str]:
    if not isinstance(base, str) or not isinstance(head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    if (
        re.fullmatch(r"[0-9a-fA-F]{40}", base) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", head) is None
    ):
        raise ValueError("pull request baseRefOid/headRefOid must be 40-character SHAs")
    boundary = base.lower(), head.lower()
    if expected is not None and boundary != expected:
        raise ValueError("pull request initial base/head does not match expected boundary")
    return boundary


def _review_thread_comments(
    thread_id: object,
    initial_cursor: object,
    *,
    budget: _GraphQLBudget | None = None,
) -> list[dict[str, object]]:
    """Read every comment on one review thread with its own cursor."""

    if not isinstance(thread_id, str) or not thread_id:
        raise TypeError("review thread id must be a non-empty string")
    if not isinstance(initial_cursor, str) or not initial_cursor:
        raise TypeError("review thread comments endCursor must be a unique string")
    query = """
query($threadId: ID!, $commentsCursor: String) {
  rateLimit { cost remaining resetAt }
  node(id: $threadId) {
    __typename
    ... on PullRequestReviewThread {
      comments(first: 100, after: $commentsCursor) {
        nodes { author { login } authorAssociation }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    comments: list[dict[str, object]] = []
    cursor = initial_cursor
    seen_cursors = {cursor}
    follow_up_pages = 0
    active_budget = budget or _GraphQLBudget()
    first_payload: object | None = None
    first_arguments: list[str] | None = None
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-F",
            f"threadId={thread_id}",
        ]
        arguments.extend(("-F", f"commentsCursor={cursor}"))
        active_budget.before_request()
        payload = _gh_json(*arguments)
        active_budget.observe(payload)
        if follow_up_pages == 0:
            first_payload = payload
            first_arguments = list(arguments)
        if not isinstance(payload, Mapping):
            raise TypeError("review thread comments response must be an object")
        data = payload.get("data")
        if not isinstance(data, Mapping):
            raise TypeError("review thread comments data must be an object")
        node = data.get("node")
        if not isinstance(node, Mapping):
            raise TypeError("review thread comments node must be an object")
        if node.get("__typename") != "PullRequestReviewThread":
            raise TypeError("review thread comments node has an invalid type")
        connection = node.get("comments")
        if not isinstance(connection, Mapping):
            raise TypeError("review thread comments must be an object")
        nodes = connection.get("nodes")
        if not isinstance(nodes, list):
            raise TypeError("review thread comments nodes must be an array")
        for comment in nodes:
            if not isinstance(comment, dict):
                raise TypeError("review thread comment must be an object")
            comments.append(comment)
        follow_up_pages += 1
        page_info = connection.get("pageInfo")
        if not isinstance(page_info, Mapping):
            raise TypeError("review thread comments pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            if first_payload is None or first_arguments is None:
                raise TypeError("review thread comments initial page is missing")
            active_budget.before_request()
            reread = _gh_json(*first_arguments)
            active_budget.observe(reread)
            if _stable_graphql_payload(reread) != _stable_graphql_payload(first_payload):
                raise ValueError("review thread comments changed during pagination")
            return comments
        if has_next_page is not True:
            raise TypeError("review thread comments hasNextPage must be a boolean")
        if follow_up_pages >= _MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES:
            raise ValueError(
                "review thread comments exceed the follow-up page limit (10)"
            )
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("review thread comments endCursor must be a unique string")
        seen_cursors.add(cursor)


def _review_threads(
    repository: str,
    pull_request: int,
    *,
    budget: _GraphQLBudget | None = None,
) -> list[dict[str, object]]:
    query = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  rateLimit { cost remaining resetAt }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $cursor) {
        nodes {
          id
          isResolved
          comments(first: 100) {
            nodes {
              author { login }
              authorAssociation
            }
            pageInfo { hasNextPage endCursor }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    owner, name = repository.split("/", maxsplit=1)
    threads: list[dict[str, object]] = []
    cursor: str | None = None
    seen_cursors: set[str] = set()
    active_budget = budget or _GraphQLBudget()
    page_count = 0
    first_payload: object | None = None
    first_arguments: list[str] | None = None
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
            "-F",
            f"number={pull_request}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        active_budget.before_request()
        payload = _gh_json(*arguments)
        active_budget.observe(payload)
        if page_count == 0:
            first_payload = payload
            first_arguments = list(arguments)
        connection = payload["data"]["repository"]["pullRequest"]["reviewThreads"]
        if not isinstance(connection, Mapping):
            raise TypeError("reviewThreads must be an object")
        nodes = connection["nodes"]
        if not isinstance(nodes, list):
            raise TypeError("reviewThreads nodes must be an array")
        for node in nodes:
            if not isinstance(node, dict):
                raise TypeError("review thread must be an object")
            thread_comments = node.get("comments")
            # Compatibility for old gh fixtures.  In a live GraphQL response
            # this key is always present because it is explicitly selected.
            if thread_comments is not None:
                if not isinstance(thread_comments, Mapping):
                    raise TypeError("review thread comments must be an object")
                page_info = thread_comments.get("pageInfo")
                if not isinstance(page_info, Mapping):
                    raise TypeError("review thread comments pageInfo must be an object")
                comment_nodes = thread_comments.get("nodes")
                if not isinstance(comment_nodes, list):
                    raise TypeError("review thread comments nodes must be an array")
                has_next_page = page_info.get("hasNextPage")
                if has_next_page is True:
                    comments_cursor = page_info.get("endCursor")
                    if (
                        not isinstance(comments_cursor, str)
                        or not comments_cursor
                    ):
                        raise TypeError(
                            "review thread comments endCursor must be a unique string"
                        )
                    comment_nodes = [
                        *comment_nodes,
                        *_review_thread_comments(
                            node.get("id"),
                            comments_cursor,
                            budget=active_budget,
                        ),
                    ]
                elif has_next_page is not False:
                    raise TypeError("review thread comments hasNextPage must be a boolean")
                node = {**node, "comments": comment_nodes}
            threads.append(node)
        page_count += 1
        page_info = connection["pageInfo"]
        if not isinstance(page_info, Mapping):
            raise TypeError("reviewThreads pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            if first_payload is None or first_arguments is None:
                raise TypeError("reviewThreads initial page is missing")
            active_budget.before_request()
            reread = _gh_json(*first_arguments)
            active_budget.observe(reread)
            if _stable_graphql_payload(reread) != _stable_graphql_payload(first_payload):
                raise ValueError("reviewThreads changed during pagination")
            return threads
        if has_next_page is not True:
            raise TypeError("reviewThreads hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("reviewThreads endCursor must be a string")
        if page_count >= _MAX_REVIEW_CONNECTION_PAGES:
            raise ValueError(
                "reviewThreads exceed the outer page limit "
                f"({_MAX_REVIEW_CONNECTION_PAGES})"
            )
        seen_cursors.add(cursor)


def _reviews(
    repository: str,
    pull_request: int,
    *,
    budget: _GraphQLBudget | None = None,
) -> list[dict[str, object]]:
    """Read every pull-request review page, failing closed on invalid cursors."""

    query = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  rateLimit { cost remaining resetAt }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviews(first: 100, after: $cursor) {
        nodes { author { login } commit { oid } state submittedAt }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    owner, name = repository.split("/", maxsplit=1)
    reviews: list[dict[str, object]] = []
    cursor: str | None = None
    seen_cursors: set[str] = set()
    active_budget = budget or _GraphQLBudget()
    page_count = 0
    first_payload: object | None = None
    first_arguments: list[str] | None = None
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
            "-F",
            f"number={pull_request}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        active_budget.before_request()
        payload = _gh_json(*arguments)
        active_budget.observe(payload)
        if page_count == 0:
            first_payload = payload
            first_arguments = list(arguments)
        connection = payload["data"]["repository"]["pullRequest"]["reviews"]
        if not isinstance(connection, Mapping):
            raise TypeError("reviews must be an object")
        nodes = connection["nodes"]
        if not isinstance(nodes, list):
            raise TypeError("reviews nodes must be an array")
        for node in nodes:
            if not isinstance(node, dict):
                raise TypeError("review must be an object")
            reviews.append(node)
        page_count += 1
        page_info = connection["pageInfo"]
        if not isinstance(page_info, Mapping):
            raise TypeError("reviews pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            if first_payload is None or first_arguments is None:
                raise TypeError("reviews initial page is missing")
            active_budget.before_request()
            reread = _gh_json(*first_arguments)
            active_budget.observe(reread)
            if _stable_graphql_payload(reread) != _stable_graphql_payload(first_payload):
                raise ValueError("reviews changed during pagination")
            return reviews
        if has_next_page is not True:
            raise TypeError("reviews hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("reviews endCursor must be a string")
        if page_count >= _MAX_REVIEW_CONNECTION_PAGES:
            raise ValueError(
                "reviews exceed the outer page limit "
                f"({_MAX_REVIEW_CONNECTION_PAGES})"
            )
        seen_cursors.add(cursor)


def _rest_page_endpoint(endpoint: str, page: int) -> str:
    """Build one bounded REST request without eager CLI pagination."""

    separator = "&" if "?" in endpoint else "?"
    per_page = "" if "per_page=" in endpoint else "&per_page=100"
    return f"{endpoint}{separator}page={page}{per_page}"


def _rest_pages(endpoint: str, *, item_key: str | None = None) -> list[object]:
    """Fetch REST pages one at a time and stop before an 11th request."""

    pages: list[object] = []
    item_count = 0
    first_payload: object | None = None
    for page_number in range(1, _MAX_REST_PAGES + 1):
        payload = _gh_json("api", _rest_page_endpoint(endpoint, page_number))
        # Keep old unit fixtures that model gh's slurp output compatible while
        # production always uses the explicit single-page request above.
        if item_key is None and isinstance(payload, list) and payload and all(
            isinstance(page, list) for page in payload
        ):
            if len(payload) > _MAX_REST_PAGES:
                raise ValueError(
                    "paginated GitHub API response exceeds the page limit "
                    f"({_MAX_REST_PAGES})"
                )
            if any(len(page) > _MAX_REST_PAGE_ITEMS for page in payload):
                raise ValueError(
                    "paginated GitHub API page exceeds the item limit "
                    f"({_MAX_REST_PAGE_ITEMS})"
                )
            flattened = [item for page in payload for item in page]
            if len(flattened) > _MAX_REST_ITEMS:
                raise ValueError(
                    "paginated GitHub API response exceeds the item limit "
                    f"({_MAX_REST_ITEMS})"
                )
            if not all(isinstance(item, Mapping) for item in flattened):
                raise TypeError("GitHub API page item must be an object")
            return payload
        if item_key is not None and isinstance(payload, list) and all(
            isinstance(page, Mapping) for page in payload
        ):
            return _bounded_rest_object_pages(
                payload, label="REST", item_key=item_key
            )
        if item_key is None:
            if not isinstance(payload, list):
                raise TypeError("paginated GitHub API response must be an array")
            values = payload
        else:
            if not isinstance(payload, Mapping):
                raise TypeError(
                    "paginated GitHub API response must contain page objects"
                )
            values = payload.get(item_key)
            if not isinstance(values, list):
                raise TypeError("paginated GitHub API page must contain an array")
        if len(values) > _MAX_REST_PAGE_ITEMS:
            raise ValueError(
                "paginated GitHub API page exceeds the item limit "
                f"({_MAX_REST_PAGE_ITEMS})"
            )
        if not all(isinstance(item, Mapping) for item in values):
            raise TypeError("GitHub API page item must be an object")
        if page_number == 1:
            first_payload = payload
        pages.append(payload)
        item_count += len(values)
        if item_count > _MAX_REST_ITEMS:
            raise ValueError(
                "paginated GitHub API response exceeds the item limit "
                f"({_MAX_REST_ITEMS})"
            )
        # A short page proves there is no next page.  A full final page is
        # deliberately treated as overflow because requesting page 11 would
        # spend the REST budget before this verifier could inspect it.
        if len(values) < _MAX_REST_PAGE_ITEMS:
            if first_payload is None:
                raise TypeError("REST initial page is missing")
            reread = _gh_json("api", _rest_page_endpoint(endpoint, 1))
            if reread != first_payload:
                raise ValueError("REST page changed during pagination")
            return pages
    raise ValueError(
        "paginated GitHub API response exceeds the page limit "
        f"({_MAX_REST_PAGES})"
    )


def _paginated_api_array(endpoint: str) -> list[dict[str, object]]:
    """Fetch every REST page and normalize it to one bounded array."""

    pages = _rest_pages(endpoint)
    flattened: list[dict[str, object]] = []
    for page in pages:
        if not isinstance(page, list):
            raise TypeError("paginated GitHub API page must be an array")
        flattened.extend(page)
    return flattened


def _bounded_rest_object_pages(
    payload: object, *, label: str, item_key: str
) -> list[Mapping[str, object]]:
    """Validate a bounded object-page fixture (kept for unit contracts)."""

    if not isinstance(payload, list) or not all(
        isinstance(page, Mapping) for page in payload
    ):
        raise TypeError(f"{label} pagination response must contain page objects")
    if len(payload) > _MAX_REST_PAGES:
        raise ValueError(
            f"{label} pagination response exceeds the page limit "
            f"({_MAX_REST_PAGES})"
        )
    pages = list(payload)
    item_count = 0
    for page in pages:
        values = page.get(item_key)
        if not isinstance(values, list):
            raise TypeError(f"{label} page must contain an array")
        if len(values) > _MAX_REST_PAGE_ITEMS:
            raise ValueError(
                f"{label} page exceeds the item limit ({_MAX_REST_PAGE_ITEMS})"
            )
        item_count += len(values)
        if item_count > _MAX_REST_ITEMS:
            raise ValueError(
                f"{label} pagination response exceeds the item limit "
                f"({_MAX_REST_ITEMS})"
            )
    return pages


def _latch_source_run_id(details_url: object, repository: str) -> str | None:
    """Accept only the canonical Actions run or job URL for this repository."""

    if not isinstance(details_url, str):
        return None
    expected_url = os.environ.get("GITHUB_SERVER_URL", "https://github.com")
    expected = urlparse(expected_url)
    parsed = urlparse(details_url)
    try:
        expected_port = expected.port
        parsed_port = parsed.port
    except ValueError:
        return None
    if (
        expected.scheme != "https"
        or expected.netloc != expected.hostname
        or expected_port is not None
        or expected.path not in {"", "/"}
        or expected.params
        or expected.query
        or expected.fragment
        or parsed.scheme != expected.scheme
        or parsed.netloc != expected.netloc
        or parsed.username is not None
        or parsed.password is not None
        or parsed_port is not None
        or parsed.params
        or parsed.query
        or parsed.fragment
    ):
        return None
    owner, name = repository.split("/", 1)
    path = parsed.path.split("/")
    expected_prefix = ["", owner, name, "actions", "runs"]
    if path[:5] != expected_prefix or len(path) not in {6, 8}:
        return None
    run_id = path[5]
    if re.fullmatch(r"[1-9][0-9]*", run_id) is None:
        return None
    if len(path) == 8 and (
        path[6] != "job" or re.fullmatch(r"[1-9][0-9]*", path[7]) is None
    ):
        return None
    return run_id


def _repository_rest_identity(
    value: object, repository: str
) -> _RepositoryRestIdentity | None | object:
    """REST の id/name/url を検証し、既存の full_name fixture も維持する。"""

    if not isinstance(value, Mapping) or repository.count("/") != 1:
        return _INVALID_REPOSITORY_IDENTITY
    repository_name = repository.rsplit("/", 1)[1]
    canonical_url = f"https://api.github.com/repos/{repository}"
    rest_fields = ("id", "name", "url")
    if not any(field in value for field in rest_fields):
        return (
            None
            if value.get("full_name") == repository
            else _INVALID_REPOSITORY_IDENTITY
        )
    identifier = value.get("id")
    name = value.get("name")
    url = value.get("url")
    if (
        type(identifier) is not int
        or identifier < 1
        or name != repository_name
        or url != canonical_url
        or ("full_name" in value and value.get("full_name") != repository)
    ):
        return _INVALID_REPOSITORY_IDENTITY
    return identifier, repository_name, canonical_url


def _repository_boundary_matches(
    repository_value: object,
    nested_values: Sequence[object],
    repository: str,
) -> bool:
    """PR 内の repository を workflow run の REST identity に束縛する。"""

    run_identity = _repository_rest_identity(repository_value, repository)
    if run_identity is _INVALID_REPOSITORY_IDENTITY:
        return False
    nested_identities = [
        _repository_rest_identity(value, repository) for value in nested_values
    ]
    if any(identity is _INVALID_REPOSITORY_IDENTITY for identity in nested_identities):
        return False
    if run_identity is not None:
        return all(identity == run_identity for identity in nested_identities)
    if not nested_identities or all(identity is None for identity in nested_identities):
        return True
    # 既存の top-level fixture と REST 形式の nested identity が混在しても、
    # nested 側は同一の不変 identity を全て保持していなければならない。
    return all(
        identity is not None and identity == nested_identities[0]
        for identity in nested_identities
    )


def _latest_sensor_generation(
    *,
    repository: str,
    pull_request: int,
    base_branch: str,
    base_sha: str,
    head: str,
) -> Mapping[str, object] | None:
    """Return one unambiguous latest sensor run for the fixed PR boundary."""

    candidates: list[Mapping[str, object]] = []
    candidate_ids: set[int] = set()
    for event_name in (
        "pull_request",
        "pull_request_review",
        "pull_request_review_comment",
    ):
        payload = _rest_pages(
            f"repos/{repository}/actions/workflows/"
            f"pr-governance-review-events.yml/runs?event={event_name}"
            f"&head_sha={head}&per_page=100",
            item_key="workflow_runs",
        )
        for page in payload:
            if "truncated" in page and type(page["truncated"]) is not bool:
                raise TypeError("sensor workflow run page is invalid")
            if page.get("truncated") is True:
                raise TypeError("sensor workflow run page is truncated")
            values = page.get("workflow_runs")
            if not isinstance(values, list) or not all(
                isinstance(value, Mapping) for value in values
            ):
                raise TypeError("sensor workflow run page is invalid")
            for value in values:
                if (
                    value.get("name") != "PR governance review sensor"
                    or value.get("event") != event_name
                ):
                    continue
                run_head = value.get("head_sha")
                if not isinstance(run_head, str) or run_head.lower() != head.lower():
                    continue
                runs_pr = value.get("pull_requests")
                repo = value.get("repository")
                path = value.get("path", "")
                if (
                    not isinstance(runs_pr, list)
                    or len(runs_pr) != 1
                    or not isinstance(runs_pr[0], Mapping)
                ):
                    raise TypeError("sensor workflow run PR identity is invalid")
                source_pr = runs_pr[0]
                source_number = source_pr.get("number")
                if type(source_number) is not int or source_number < 1:
                    raise TypeError("sensor workflow run PR identity is invalid")
                if source_number != pull_request:
                    continue
                source_base = source_pr.get("base")
                source_head = source_pr.get("head")
                if not isinstance(source_base, Mapping) or not isinstance(source_head, Mapping):
                    raise TypeError("sensor workflow run PR boundary is invalid")
                source_base_sha = source_base.get("sha")
                source_head_sha = source_head.get("sha")
                source_base_repo = source_base.get("repo")
                source_head_repo = source_head.get("repo")
                if (
                    not isinstance(source_base_sha, str)
                    or not isinstance(source_head_sha, str)
                    or not isinstance(source_base_repo, Mapping)
                    or not isinstance(source_head_repo, Mapping)
                ):
                    raise TypeError("sensor workflow run PR boundary is invalid")
                if not _repository_boundary_matches(
                    repo, (source_base_repo, source_head_repo), repository
                ):
                    raise TypeError("sensor workflow run repository is invalid")
                if (
                    source_base_sha.lower() != base_sha.lower()
                    or source_base.get("ref") != base_branch
                    or source_head_sha.lower() != head.lower()
                ):
                    continue
                if (
                    type(value.get("id")) is not int
                    or value["id"] < 1
                    or type(value.get("run_number")) is not int
                    or value["run_number"] < 1
                    or type(value.get("run_attempt")) is not int
                    or value["run_attempt"] != 1
                ):
                    raise TypeError("sensor workflow run generation is invalid")
                if (
                    not isinstance(path, str)
                    or path.split("@", 1)[0]
                    != ".github/workflows/pr-governance-review-events.yml"
                    or (
                        "@" in path
                        and (
                            re.fullmatch(r"[A-Za-z0-9._/-]+", path.split("@", 1)[1])
                            is None
                            or not _safe_branch_name(path.split("@", 1)[1])
                        )
                    )
                ):
                    raise TypeError("sensor workflow run path is invalid")
                if value["id"] in candidate_ids:
                    raise TypeError("sensor workflow run generation is duplicated")
                candidate_ids.add(value["id"])
                candidates.append(value)
    if not candidates:
        return None
    key = max((value["run_number"], value["id"]) for value in candidates)
    latest = [
        value
        for value in candidates
        if (value["run_number"], value["id"]) == key
    ]
    return latest[0] if len(latest) == 1 else None


def _governance_check_error(
    repository: str,
    pull_request: int,
    base_branch: object,
    base_sha: str,
    head: str,
    body_sha256: object,
    evidence: dict[str, object] | None = None,
    *,
    exclude_trusted_governance_check: bool = False,
    required_checks: _RequiredStatusChecks | None = None,
) -> str | None:
    """Require exactly one terminal trusted Check Run and its Actions evidence."""
    if not isinstance(body_sha256, str) or re.fullmatch(r"[0-9a-f]{64}", body_sha256) is None:
        raise TypeError("pull request body SHA-256 must be lowercase hexadecimal")
    if required_checks is None:
        try:
            required_checks = _required_status_check_snapshot(repository, base_branch)
        except (TypeError, ValueError):
            return "branch protection required status checks configuration is invalid"
    binding_error = _governance_app_binding_error(required_checks)
    if binding_error is not None:
        return binding_error
    app_id = dict(required_checks[1])[_TRUSTED_CHECK]
    assert type(app_id) is int
    run: Mapping[str, object] | None = None
    source_ids: list[str] = []
    if not exclude_trusted_governance_check:
        raw_pages = _rest_pages(
            f"repos/{repository}/commits/{head}/check-runs?check_name={_TRUSTED_CHECK.replace(' ', '%20')}&app_id={app_id}&filter=all&per_page=100",
            item_key="check_runs",
        )
        runs: list[Mapping[str, object]] = []
        for page in raw_pages:
            page_runs = page.get("check_runs")
            if not isinstance(page_runs, list) or not all(
                isinstance(run, Mapping) for run in page_runs
            ):
                raise TypeError("check-runs page must contain an array")
            runs.extend(page_runs)
        matches = [item for item in runs if item.get("name") == _TRUSTED_CHECK and item.get("head_sha", "").lower() == head.lower() and isinstance(item.get("app"), Mapping) and item["app"].get("id") == app_id]
        # Each dispatcher/early writer owns an immutable Check Run generation.
        # Check Run PATCH has no CAS, so the newest immutable ID is the only
        # authoritative generation; an old success must never mask a newer
        # pending or failure.  Duplicating one external-id is ambiguous.
        generations: dict[str, tuple[Mapping[str, object], datetime]] = {}
        external_pattern = re.compile(rf"krr-governance/v1/{re.escape(head.lower())}/(?:dispatcher|writer)-[1-9][0-9]*$")
        for item in matches:
            identifier = item.get("id")
            external = item.get("external_id")
            created_at = item.get("created_at")
            if type(identifier) is not int or identifier < 1 or not isinstance(external, str) or external_pattern.fullmatch(external) is None or not isinstance(created_at, str) or not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+Z", created_at):
                return "trusted Check Run immutable generation is invalid"
            try:
                created = datetime.fromisoformat(created_at.replace("Z", "+00:00"))
            except ValueError:
                return "trusted Check Run immutable generation is invalid"
            if external in generations:
                return "trusted Check Run immutable generation is ambiguous"
            generations[external] = (item, created)
        if not generations:
            return "trusted Check Run immutable generation is missing"
        run = max(
            generations.values(), key=lambda item: (item[1], int(item[0]["id"]))
        )[0]
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            return "trusted Check Run is not completed successfully"
        external_id = run.get("external_id")
        details = run.get("details_url")
        if not isinstance(external_id, str) or external_pattern.fullmatch(external_id) is None:
            return "trusted Check Run external_id is invalid"
        if not isinstance(details, str):
            return "trusted Check Run details_url lacks exact source_run_id evidence"
        query = parse_qs(urlparse(details).query, keep_blank_values=True)
        source_ids = query.get("source_run_id", [])
        if len(source_ids) != 1 or re.fullmatch(r"[1-9][0-9]*", source_ids[0]) is None:
            return "trusted Check Run details_url lacks exact source_run_id evidence"
        if query.get("pr_body_sha256", []) != [body_sha256]:
            return "trusted Check Run details_url lacks exact current PR body digest evidence"
    else:
        latest = _latest_sensor_generation(
            repository=repository,
            pull_request=pull_request,
            base_branch=base_branch,
            base_sha=base_sha,
            head=head,
        )
        if latest is None:
            return "trusted source Actions run is not the latest sensor generation"
        source_ids = [str(latest["id"])]
    latch_payload = _rest_pages(
        f"repos/{repository}/commits/{head}/check-runs?check_name={_LATCH_CHECK.replace(' ', '%20')}&app_id=15368&filter=all&per_page=100",
        item_key="check_runs",
    )
    latch_runs = [item for page in latch_payload for item in page["check_runs"]]
    if not all(isinstance(item, Mapping) for item in latch_runs):
        raise TypeError("latch Check Run page must contain an array")
    latch_candidates = [item for item in latch_runs if item.get("name") == _LATCH_CHECK and item.get("head_sha", "").lower() == head.lower() and isinstance(item.get("app"), Mapping) and item["app"].get("id") == 15368]
    same_source: list[Mapping[str, object]] = []
    for item in latch_candidates:
        source_id = _latch_source_run_id(item.get("details_url"), repository)
        if source_id == source_ids[0]:
            same_source.append(item)
    if len(same_source) != 1:
        return "review latch Check Run for the trusted source must have exactly one matching run"
    latch = same_source[0]
    if not _sensor_or_latch_state_is_valid(
        latch, internal_recalculation=exclude_trusted_governance_check
    ):
        return "review latch Check Run is not completed successfully"
    source = _gh_json("api", f"repos/{repository}/actions/runs/{source_ids[0]}")
    if not isinstance(source, Mapping):
        return "trusted source Actions run response is invalid"
    if (
        type(source.get("id")) is not int
        or source.get("id") != int(source_ids[0])
        or source.get("name") != "PR governance review sensor"
        or source.get("event") not in {"pull_request", "pull_request_review", "pull_request_review_comment"}
        or type(source.get("run_attempt")) is not int
        or source.get("run_attempt") != 1
        or source.get("head_sha", "").lower() != head.lower()
        or not isinstance(source.get("path"), str)
        or source.get("path", "").split("@", 1)[0] != ".github/workflows/pr-governance-review-events.yml"
        or ("@" in source.get("path", "") and (
            re.fullmatch(r"[A-Za-z0-9._/-]+", source.get("path", "").split("@", 1)[1]) is None
            or source.get("path", "").split("@", 1)[1].startswith("/")
            or "//" in source.get("path", "").split("@", 1)[1]
            or any(part in {".", ".."} for part in source.get("path", "").split("@", 1)[1].split("/"))
        ))
    ):
        return "trusted source Actions run evidence does not match"
    if not _sensor_or_latch_state_is_valid(
        source, internal_recalculation=exclude_trusted_governance_check
    ):
        return "trusted source Actions run evidence does not match"
    pull_requests = source.get("pull_requests")
    if not isinstance(pull_requests, list) or len(pull_requests) != 1 or not isinstance(pull_requests[0], Mapping) or pull_requests[0].get("number") != pull_request:
        return "trusted source Actions run PR identity does not match"
    source_pr = pull_requests[0]
    source_repo = source.get("repository")
    source_base = source_pr.get("base")
    source_head = source_pr.get("head")
    source_base_repo = source_base.get("repo") if isinstance(source_base, Mapping) else None
    source_head_repo = source_head.get("repo") if isinstance(source_head, Mapping) else None
    if (
        not _repository_boundary_matches(
            source_repo, (source_base_repo, source_head_repo), repository
        )
        or not isinstance(source_base, Mapping) or source_base.get("sha", "").lower() != base_sha.lower()
        or source_base.get("ref") != base_branch
        or not isinstance(source_head, Mapping) or source_head.get("sha", "").lower() != head.lower()
    ):
        return "trusted source Actions run PR boundary does not match"
    latest = _latest_sensor_generation(
        repository=repository,
        pull_request=pull_request,
        base_branch=base_branch,
        base_sha=base_sha,
        head=head,
    )
    if latest is None:
        return "trusted source Actions run is not the latest sensor generation"
    if latest.get("id") != int(source_ids[0]):
        return "trusted source Actions run is not the latest sensor generation"
    if not _sensor_or_latch_state_is_valid(
        latest, internal_recalculation=exclude_trusted_governance_check
    ):
        return "trusted source Actions run is not the latest sensor generation"
    if evidence is not None:
        evidence.update({
            "protection": required_checks,
            "check": (
                ("excluded", app_id, head.lower())
                if run is None
                else tuple(run.get(key) for key in ("id", "name", "head_sha", "external_id", "status", "conclusion", "details_url"))
            ),
            "latch": tuple(latch.get(key) for key in ("id", "name", "head_sha", "status", "conclusion", "details_url")),
            "source": tuple(source.get(key) for key in ("id", "name", "path", "event", "run_attempt", "head_sha", "status", "conclusion")),
            "source_pr": (source_pr.get("number"), source_base.get("sha"), source_base.get("ref"), source_base["repo"].get("full_name"), source_head.get("sha"), source_head["repo"].get("full_name")),
        })
    return None


def main(argv: Sequence[str] | None = None) -> int:
    global _ACTIVE_REST_BUDGET, _ACTIVE_GRAPHQL_BUDGET, _ACTIVE_SHARED_GRAPHQL_LEDGER
    # A verifier invocation is one process-level budget scope.  Resetting here
    # also keeps embedded/unit callers from carrying a prior invocation's
    # remaining value into the next readiness check.
    _ACTIVE_REST_BUDGET = None
    _ACTIVE_GRAPHQL_BUDGET = None
    _ACTIVE_SHARED_GRAPHQL_LEDGER = None
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, required=True, help="pull request number")
    parser.add_argument("--repository", help="GitHub repository as OWNER/REPOSITORY")
    parser.add_argument("--expected-base-sha")
    parser.add_argument("--expected-head-sha")
    parser.add_argument("--open-pull-snapshot")
    parser.add_argument(
        "--exclude-trusted-governance-check",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    draft_group = parser.add_mutually_exclusive_group()
    draft_group.add_argument(
        "--require-draft", dest="require_draft", action="store_true", default=True
    )
    draft_group.add_argument(
        "--allow-ready", dest="require_draft", action="store_false"
    )
    arguments = parser.parse_args(argv)
    if (arguments.expected_base_sha is None) != (arguments.expected_head_sha is None):
        parser.error("--expected-base-sha and --expected-head-sha must be provided together")
    if arguments.exclude_trusted_governance_check and arguments.require_draft:
        parser.error("--exclude-trusted-governance-check requires --allow-ready")
    # The all/early writer passes one private temporary open-PR snapshot to
    # every verifier process. Its immutable bytes name a single ledger, while
    # ordinary local invocations retain their existing per-process lease.
    if (
        arguments.exclude_trusted_governance_check
        and arguments.open_pull_snapshot is not None
    ):
        _ACTIVE_SHARED_GRAPHQL_LEDGER = _SharedGraphQLLedger.from_open_pull_snapshot(
            arguments.open_pull_snapshot
        )
    expected_boundary = _expected_boundary(
        arguments.expected_base_sha, arguments.expected_head_sha
    )

    repository = _repository_name(arguments.repository)
    pull_request = _gh_json(
        "pr",
        "view",
        str(arguments.pr),
        "--repo",
        repository,
        "--json",
        "isDraft,baseRefOid,headRefOid,baseRefName,body,updatedAt,statusCheckRollup,reviews,author",
    )
    if not isinstance(pull_request, dict):
        raise TypeError("pull request response must be an object")
    marker_author_login = _pull_request_author_login(pull_request)
    current_reviews = pull_request.get("reviews")
    if not isinstance(current_reviews, list):
        raise TypeError("pull request reviews must be an array")
    graphql_budget = _ACTIVE_GRAPHQL_BUDGET or _GraphQLBudget()
    # gh pr view exposes a bounded connection.  A full boundary (or an empty
    # compatibility response) requires explicit GraphQL cursor pagination.
    if not current_reviews or len(current_reviews) >= 100:
        pull_request["reviews"] = _reviews(
            repository, arguments.pr, budget=graphql_budget
        )
    comments = _paginated_api_array(
        f"repos/{repository}/issues/{arguments.pr}/comments"
    )
    initial_marker_identities = _review_marker_identities(
        _review_markers(comments, marker_author_login)
    )
    threads = _review_threads(repository, arguments.pr, budget=graphql_budget)
    base, head = _require_boundary(
        pull_request.get("baseRefOid"), pull_request.get("headRefOid"), expected_boundary
    )
    base_branch = pull_request.get("baseRefName")
    initial_required_checks = _required_status_check_snapshot(repository, base_branch)
    initial_updated_at = _required_timestamp_text(
        pull_request.get("updatedAt"), "pull request updatedAt"
    )
    referenced_issues = issue_contract.referenced_issue_snapshot(
        repository=repository,
        base_sha=base,
        head_sha=head,
    )
    initial_body = pull_request.get("body")
    initial_body_sha256 = _body_sha256(initial_body)
    errors = closing_reference_errors(
        repository=repository,
        body=initial_body,
        referenced_issues=referenced_issues,
    )
    binding_error = _governance_app_binding_error(initial_required_checks)
    if binding_error is not None:
        errors.append(binding_error)
    errors.extend(
        _required_check_run_producer_errors(
            repository,
            head,
            pull_request.get("statusCheckRollup"),
            initial_required_checks,
        )
    )
    governance_evidence: dict[str, object] = {}
    if not arguments.require_draft:
        governance_error = _governance_check_error(
            repository,
            arguments.pr,
            base_branch,
            base,
            head,
            initial_body_sha256,
            governance_evidence,
            exclude_trusted_governance_check=arguments.exclude_trusted_governance_check,
            required_checks=initial_required_checks,
        )
        if governance_error is not None:
            errors.append(governance_error)
    initial_issue_identity: tuple[tuple[int, str, str, str, str], ...] = ()
    initial_closers: frozenset[int] = frozenset()
    if referenced_issues and not errors:
        open_pull_requests = (
            _open_pull_request_snapshot(arguments.open_pull_snapshot)
            if arguments.open_pull_snapshot is not None
            else _open_pull_requests(repository)
        )
        errors.extend(
            closing_open_pull_request_errors(
                repository=repository,
                current_pull_request=arguments.pr,
                referenced_issues=referenced_issues,
                open_pull_requests=open_pull_requests,
            )
        )
        if not errors:
            initial_issue_identity = _canonical_issue_identity(
                repository=repository,
                referenced_issues=referenced_issues,
            )
            initial_closers = frozenset(
                _open_pull_request_closers(
                    repository=repository,
                    issue_number=referenced_issues[0].number,
                    open_pull_requests=open_pull_requests,
                )
            )
    errors.extend(
        readiness_errors(
            pull_request,
            threads,
            comments,
            review_bot="chatgpt-codex-connector",
            require_draft=arguments.require_draft,
            referenced_issues=referenced_issues,
            required_checks=initial_required_checks,
        )
    )
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if not isinstance(initial_body, str):
        raise TypeError("pull request body must be a string")
    final_comments = _paginated_api_array(
        f"repos/{repository}/issues/{arguments.pr}/comments"
    )
    if (
        _review_marker_identities(
            _review_markers(final_comments, marker_author_login)
        )
        != initial_marker_identities
    ):
        raise ValueError("trusted review marker identities changed during readiness check")
    _verify_final_readiness_snapshot_unchanged(
        repository=repository,
        pull_request=arguments.pr,
        initial_base=base,
        initial_head=head,
        initial_base_branch=base_branch,
        initial_body=initial_body,
        initial_updated_at=initial_updated_at,
        initial_required_checks=initial_required_checks,
        initial_issue_identity=initial_issue_identity,
        initial_closers=initial_closers,
        open_pull_requests=(
            open_pull_requests
            if arguments.open_pull_snapshot is not None and referenced_issues and not errors
            else None
        ),
    )
    if not arguments.require_draft:
        final_required_checks = _required_status_check_snapshot(repository, base_branch)
        if final_required_checks != initial_required_checks:
            raise ValueError("required status checks changed during readiness check")
        final_governance_evidence: dict[str, object] = {}
        governance_error = _governance_check_error(
            repository,
            arguments.pr,
            base_branch,
            base,
            head,
            initial_body_sha256,
            final_governance_evidence,
            exclude_trusted_governance_check=arguments.exclude_trusted_governance_check,
            required_checks=final_required_checks,
        )
        if governance_error is not None or final_governance_evidence != governance_evidence:
            raise ValueError("governance evidence changed during readiness check")
    print(f"PR #{arguments.pr} is ready")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
