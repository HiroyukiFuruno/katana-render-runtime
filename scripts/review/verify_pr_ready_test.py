from __future__ import annotations

import fcntl
import json
import hashlib
import multiprocessing
import os
import queue
import re
import subprocess
import sys
import tempfile
import time
import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import mock_open, patch
from urllib.parse import parse_qs, urlparse

sys.path.insert(0, str(Path(__file__).parent))

import verify_pr_ready as subject


HEAD = "a" * 40
INITIAL_HEAD = "b" * 40
BOT = "chatgpt-codex-connector"
BODY = "Closes #64"
BODY_SHA256 = hashlib.sha256(BODY.encode("utf-8")).hexdigest()
NO_ISSUES_PREFIX = "Codex Review: Didn't find any major issues"
NO_ISSUES_FIXTURE = (
    Path(__file__).parent / "fixtures" / "no_issues_comment_contract.v1.json"
)


def marker(
    comment_id: int,
    phase: str,
    head: str,
    updated_at: str | None = None,
    body: str = BODY,
    login: str = "HiroyukiFuruno",
    author_association: object = "OWNER",
) -> dict[str, object]:
    created_at = f"2026-08-29T03:0{comment_id}:00Z"
    return {
        "id": comment_id,
        "body": (
            f"<!-- krr-review phase={phase} head={head} "
            f"body-sha256={hashlib.sha256(body.encode('utf-8')).hexdigest()} -->\n"
            "@codex review"
        ),
        "created_at": created_at,
        "updated_at": updated_at or created_at,
        "user": {"login": login},
        "author_association": author_association,
    }


def no_issues_review(
    comment_id: int,
    head: str,
    *,
    login: str = BOT,
    prefix_length: int = 10,
    created_at: str = "2026-08-29T03:01:30Z",
    updated_at: str | None = None,
    body: str | None = None,
    result: str = "Hooray!",
    footer: str = "",
) -> dict[str, object]:
    return {
        "id": comment_id,
        "body": body or (
            f"{NO_ISSUES_PREFIX}. {result}\n\n"
            f"**Reviewed commit:** `{head[:prefix_length]}`{footer}"
        ),
        "created_at": created_at,
        "updated_at": updated_at or created_at,
        "user": {"login": login},
    }


def _shared_ledger_worker(
    snapshot_path: str,
    start: multiprocessing.synchronize.Event,
    results: multiprocessing.queues.Queue[tuple[object, ...]],
    operations: int,
) -> None:
    """Exercise creation and reservation through a separate verifier process."""

    try:
        if not start.wait(5):
            raise RuntimeError("shared ledger start barrier timed out")
        ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(snapshot_path)
        consumed = 0
        for operation in range(operations):
            reservation = ledger.reserve()
            cost = 1 + operation % 2
            ledger.settle(reservation, cost)
            consumed += cost
        results.put(("ok", consumed))
    except BaseException as error:
        results.put(("error", type(error).__name__, str(error)))


def _shared_ledger_lease_worker(
    snapshot_path: str,
    start: multiprocessing.synchronize.Event,
    results: multiprocessing.queues.Queue[tuple[object, ...]],
) -> None:
    """Reserve once to model one high-quota verifier process without settling."""

    try:
        if not start.wait(10):
            raise RuntimeError("shared ledger start barrier timed out")
        ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(snapshot_path)
        results.put(("reserved", ledger.reserve()))
    except ValueError as error:
        if "budget is exhausted" in str(error):
            results.put(("exhausted",))
        else:
            results.put(("error", type(error).__name__, str(error)))
    except BaseException as error:
        results.put(("error", type(error).__name__, str(error)))


def _paused_shared_ledger_creator(
    snapshot_path: str,
    initialized: multiprocessing.synchronize.Event,
    release: multiprocessing.synchronize.Event,
    results: multiprocessing.queues.Queue[tuple[object, ...]],
) -> None:
    """Pause after O_EXCL + LOCK_EX and before the creator writes its first byte."""

    original_write = subject._SharedGraphQLLedger._write

    def paused_write(
        ledger: subject._SharedGraphQLLedger,
        descriptor: int,
        state: dict[str, object],
    ) -> None:
        initialized.set()
        if not release.wait(5):
            raise RuntimeError("creator release barrier timed out")
        original_write(ledger, descriptor, state)

    try:
        with patch.object(subject._SharedGraphQLLedger, "_write", paused_write):
            subject._SharedGraphQLLedger.from_open_pull_snapshot(snapshot_path)
        results.put(("creator", "ok"))
    except BaseException as error:
        results.put(("creator", type(error).__name__, str(error)))


def _initializing_shared_ledger_peer(
    snapshot_path: str,
    blocked: multiprocessing.synchronize.Event,
    results: multiprocessing.queues.Queue[tuple[object, ...]],
) -> None:
    """Report the exact nonblocking flock contention before retrying normally."""

    original_flock = subject.fcntl.flock

    def observing_flock(descriptor: int, operation: int) -> None:
        try:
            original_flock(descriptor, operation)
        except BlockingIOError:
            if operation & fcntl.LOCK_NB:
                blocked.set()
            raise

    try:
        with patch.object(subject.fcntl, "flock", side_effect=observing_flock):
            subject._SharedGraphQLLedger.from_open_pull_snapshot(snapshot_path)
        results.put(("peer", "ok"))
    except BaseException as error:
        results.put(("peer", type(error).__name__, str(error)))


def _hold_shared_ledger_lock(
    ledger_path: str,
    acquired: multiprocessing.synchronize.Event,
    release: multiprocessing.synchronize.Event,
) -> None:
    descriptor = os.open(ledger_path, os.O_RDWR)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        acquired.set()
        release.wait(5)
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def _contending_shared_ledger_reserver(
    snapshot_path: str,
    blocked: multiprocessing.synchronize.Event,
    results: multiprocessing.queues.Queue[tuple[object, ...]],
) -> None:
    """Prove a real verifier can outwait a healthy, serialized ledger owner."""

    original_flock = subject.fcntl.flock

    def observing_flock(descriptor: int, operation: int) -> None:
        try:
            original_flock(descriptor, operation)
        except BlockingIOError:
            if operation & fcntl.LOCK_NB:
                blocked.set()
            raise

    try:
        with patch.object(subject.fcntl, "flock", side_effect=observing_flock):
            ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(snapshot_path)
            ledger.reserve()
        results.put(("reserved",))
    except BaseException as error:
        results.put(("error", type(error).__name__, str(error)))


def successful_state() -> tuple[
    dict[str, object],
    list[dict[str, object]],
    list[dict[str, object]],
]:
    pull_request = {
        "isDraft": True,
        "baseRefOid": "c" * 40,
        "baseRefName": "master",
        "headRefOid": HEAD,
        "body": BODY,
        "author": {"login": "HiroyukiFuruno"},
        "updatedAt": "2026-08-29T03:03:00Z",
        "statusCheckRollup": [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            }
        ],
        "reviews": [
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:01:30Z",
            },
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:02:30Z",
            },
        ],
    }
    threads = [
        {
            "id": "thread-1",
            "isResolved": True,
            "comments": [
                {"author": {"login": "reviewer"}},
                {"author": {"login": "HiroyukiFuruno"}},
            ],
        }
    ]
    comments = [
        marker(1, "initial", HEAD),
        marker(2, "final", HEAD),
    ]
    return pull_request, threads, comments


def current_canonical_closer() -> list[dict[str, object]]:
    return [{"number": 72, "isDraft": True, "body": "Closes #64"}]


def rate_limited(
    payload: dict[str, object], *, remaining: int = 4_500, cost: int = 1
) -> dict[str, object]:
    data = payload.get("data")
    if not isinstance(data, dict):
        raise AssertionError("GraphQL fixture data must be an object")
    data["rateLimit"] = {
        "cost": cost,
        "remaining": remaining,
        "resetAt": "2026-08-29T04:00:00Z",
    }
    return payload


def rest_rate_limited(
    *, remaining: int = 4_500, limit: int = 5_000, reset: int = 1_800_000_000
) -> dict[str, object]:
    return {
        "resources": {
            "core": {"limit": limit, "remaining": remaining, "reset": reset}
        }
    }


class VerifyPrReadyTest(unittest.TestCase):
    def setUp(self) -> None:
        self.required_checks = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        self.required_checks_patcher = patch.object(
            subject,
            "_required_status_check_snapshot",
            return_value=self.required_checks,
        )
        self.required_checks_patcher.start()
        self.addCleanup(self.required_checks_patcher.stop)

    def errors(
        self,
        pull_request: dict[str, object] | None = None,
        threads: list[dict[str, object]] | None = None,
        comments: list[dict[str, object]] | None = None,
        referenced_issues: tuple[subject.issue_contract.Issue, ...] = (),
    ) -> list[str]:
        default_pr, default_threads, default_comments = successful_state()
        return subject.readiness_errors(
            pull_request or default_pr,
            threads if threads is not None else default_threads,
            comments if comments is not None else default_comments,
            review_bot=BOT,
            require_draft=True,
            referenced_issues=referenced_issues,
        )

    def issue(self, number: int, updated_at: str) -> subject.issue_contract.Issue:
        return subject.issue_contract.Issue(
            number=number,
            state="OPEN",
            body="Issue body",
            url=f"https://github.com/owner/repo/issues/{number}",
            updated_at=updated_at,
        )

    def test_accepts_two_phase_review_on_current_head(self) -> None:
        self.assertEqual(self.errors(), [])

    def test_ignores_newer_valid_looking_marker_from_an_untrusted_user(self) -> None:
        for phase in ("initial", "final"):
            with self.subTest(phase=phase):
                pull_request, threads, comments = successful_state()
                comments.append(
                    marker(
                        3,
                        phase,
                        INITIAL_HEAD,
                        login="external-user",
                        author_association="NONE",
                    )
                )
                self.assertEqual(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    ),
                    [],
                )

    def test_accepts_markers_from_the_pr_author(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        for comment in comments:
            comment["author_association"] = "NONE"
        threads[0]["comments"] = [
            {"author": {"login": "reviewer"}},
            {"author": {"login": "HiroyukiFuruno"}},
        ]
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
            ),
            [],
        )

    def test_accepts_markers_from_a_trusted_maintainer(self) -> None:
        pull_request, threads, comments = successful_state()
        for comment in comments:
            comment["author"] = {"login": "trusted-maintainer"}
            comment["authorAssociation"] = "MEMBER"
            comment.pop("user")
            comment.pop("author_association")
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
            ),
            [],
        )

    def test_ignores_newer_marker_with_missing_or_malformed_identity(self) -> None:
        variants = {
            "missing author": lambda comment: comment.pop("user"),
            "invalid author": lambda comment: comment.update({"user": {"login": 42}}),
            "missing association": lambda comment: comment.pop("author_association"),
            "invalid association": lambda comment: comment.update({"author_association": []}),
        }
        for name, mutate in variants.items():
            with self.subTest(name=name):
                _, _, comments = successful_state()
                untrusted_marker = marker(3, "final", INITIAL_HEAD)
                mutate(untrusted_marker)
                comments.append(untrusted_marker)
                self.assertEqual(self.errors(comments=comments), [])

    def test_rejects_missing_or_malformed_pr_author_despite_trusted_markers(self) -> None:
        variants = {
            "missing": lambda pull_request: pull_request.pop("author"),
            "null": lambda pull_request: pull_request.update({"author": None}),
            "non-mapping": lambda pull_request: pull_request.update({"author": []}),
            "missing login": lambda pull_request: pull_request.update({"author": {}}),
            "non-string login": lambda pull_request: pull_request.update({"author": {"login": 42}}),
            "empty login": lambda pull_request: pull_request.update({"author": {"login": ""}}),
        }
        for name, mutate in variants.items():
            with self.subTest(name=name):
                pull_request, _, _ = successful_state()
                pull_request["author"] = {"login": "HiroyukiFuruno"}
                mutate(pull_request)
                with self.assertRaisesRegex(TypeError, "pull request author"):
                    self.errors(pull_request=pull_request)

    def test_rejects_pr_author_marker_without_an_association(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        threads[0]["comments"] = [
            {"author": {"login": "reviewer"}},
            {"author": {"login": "HiroyukiFuruno"}},
        ]
        for comment in comments:
            comment["author_association"] = "NONE"
            comment.pop("author_association")
        errors = " ".join(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
            )
        )
        self.assertIn("initial review marker がありません", errors)
        self.assertIn("final review marker がありません", errors)

    def test_rejects_latest_initial_marker_for_old_head_even_with_current_final_review(self) -> None:
        pull_request, threads, comments = successful_state()
        comments[0] = marker(1, "initial", INITIAL_HEAD)
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["commit"] = {"oid": INITIAL_HEAD}
        errors = self.errors(
            pull_request=pull_request,
            threads=threads,
            comments=comments,
        )
        self.assertTrue(
            errors,
            "an old-head initial marker must not authorize current final evidence",
        )

    def test_accepts_trusted_no_issues_review_in_both_phases(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        comments = [
            marker(1, "initial", HEAD),
            no_issues_review(
                3, HEAD, created_at="2026-08-29T03:01:30Z"
            ),
            marker(2, "final", HEAD),
        ]
        self.assertEqual(
            self.errors(
                pull_request=pull_request, threads=threads, comments=comments
            ),
            [],
        )

    def test_no_issues_comment_contract_fixture_matches_completion_parser(self) -> None:
        with NO_ISSUES_FIXTURE.open(encoding="utf-8") as fixture_file:
            fixture = json.load(fixture_file)
        self.assertIsInstance(fixture, dict)
        self.assertEqual(set(fixture), {"version", "cases"})
        self.assertEqual(fixture["version"], 1)
        cases = fixture["cases"]
        self.assertIsInstance(cases, list)
        self.assertGreater(len(cases), 0)

        for case in cases:
            with self.subTest(case=case.get("name")):
                self.assertIsInstance(case, dict)
                self.assertEqual(
                    set(case), {"name", "head", "after", "before", "accepted", "comment"}
                )
                self.assertIsInstance(case["name"], str)
                head = case["head"]
                self.assertIsInstance(head, str)
                self.assertRegex(head, r"^[0-9a-f]{40}$")
                after = case["after"]
                before = case["before"]
                self.assertIsInstance(after, str)
                self.assertIsInstance(before, str)
                after_time = subject._timestamp(after, "fixture after")
                before_time = subject._timestamp(before, "fixture before")
                self.assertIsNotNone(after_time)
                self.assertIsNotNone(before_time)
                assert after_time is not None and before_time is not None
                self.assertIsNotNone(after_time.tzinfo)
                self.assertIsNotNone(before_time.tzinfo)
                self.assertLess(after_time, before_time)
                accepted = case["accepted"]
                self.assertIs(type(accepted), bool)

                comment = case["comment"]
                self.assertIsInstance(comment, dict)
                self.assertEqual(
                    set(comment), {"body", "created_at", "updated_at", "user"}
                )
                self.assertIsInstance(comment["body"], str)
                self.assertIsInstance(comment["created_at"], str)
                self.assertIsInstance(comment["updated_at"], str)
                self.assertIsInstance(comment["user"], dict)
                self.assertEqual(set(comment["user"]), {"login"})
                self.assertIsInstance(comment["user"]["login"], str)

                completion_times = subject._no_issues_comment_completion_times(
                    [comment], BOT, head, after, before
                )
                self.assertEqual(bool(completion_times), accepted)

    def test_accepts_no_issues_evidence_after_first_api_page(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        filler = [
            {
                "id": index,
                "body": f"unrelated comment {index}",
                "created_at": "2026-08-29T03:00:00Z",
                "updated_at": "2026-08-29T03:00:00Z",
                "user": {"login": "reviewer"},
            }
            for index in range(1, 101)
        ]
        comments = filler + [
            marker(1, "initial", HEAD),
            no_issues_review(3, HEAD, created_at="2026-08-29T03:01:30Z"),
            marker(2, "final", HEAD),
        ]
        self.assertEqual(
            self.errors(
                pull_request=pull_request, threads=threads, comments=comments
            ),
            [],
        )

    def test_accepts_no_issues_evidence_after_final_marker_too(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        comments = [
            marker(1, "initial", HEAD),
            no_issues_review(3, HEAD, created_at="2026-08-29T03:01:30Z"),
            marker(2, "final", HEAD),
            no_issues_review(4, HEAD, created_at="2026-08-29T03:02:30Z"),
        ]
        self.assertEqual(
            self.errors(
                pull_request=pull_request, threads=threads, comments=comments
            ),
            [],
        )

    def test_accepts_live_no_issues_result_variants(self) -> None:
        for result in ("Hooray!", "Delightful!"):
            with self.subTest(result=result):
                pull_request, threads, _ = successful_state()
                pull_request["reviews"] = []
                comments = [
                    marker(1, "initial", HEAD),
                    no_issues_review(3, HEAD, result=result),
                    marker(2, "final", HEAD),
                ]
                self.assertEqual(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    ),
                    [],
                )

    def test_accepts_live_no_issues_footer_and_trailing_newline(self) -> None:
        footer = (
            "\n\n<details> <summary>ℹ️ About Codex in GitHub</summary>\n"
            "Generated by Codex.\n</details>"
        )
        for suffix in (footer, footer + "\n"):
            with self.subTest(suffix=repr(suffix)):
                pull_request, threads, _ = successful_state()
                pull_request["reviews"] = []
                comments = [
                    marker(1, "initial", HEAD),
                    no_issues_review(3, HEAD, footer=suffix),
                    marker(2, "final", HEAD),
                ]
                self.assertEqual(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    ),
                    [],
                )

    def test_rejects_unbounded_or_noncanonical_no_issues_footer(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        variants = {
            "nested details": (
                "\n\n<details> <summary>ℹ️ About Codex in GitHub</summary>\n"
                "<details>nested</details>\n</details>"
            ),
            "sentinel": (
                "\n\n<details> <summary>ℹ️ About Codex in GitHub</summary>\n"
                "Codex Review: injected\n</details>"
            ),
            "oversize": (
                "\n\n<details> <summary>ℹ️ About Codex in GitHub</summary>\n"
                + "x" * 8193
                + "</details>"
            ),
            "wrong summary": (
                "\n\n<details> <summary>Review details</summary>\n"
                "Generated by Codex.\n</details>"
            ),
        }
        for name, footer in variants.items():
            with self.subTest(name=name):
                comments = [
                    marker(1, "initial", HEAD),
                    no_issues_review(3, HEAD, footer=footer),
                    marker(2, "final", HEAD),
                ]
                self.assertTrue(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    )
                )

    def test_rejects_edited_no_issues_comment_timestamp(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        evidence = no_issues_review(3, HEAD, created_at="2026-08-29T03:00:30Z")
        evidence["updated_at"] = "2026-08-29T03:01:30Z"
        comments = [
            marker(1, "initial", HEAD),
            evidence,
            marker(2, "final", HEAD),
        ]
        self.assertTrue(
            self.errors(pull_request=pull_request, threads=threads, comments=comments)
        )

    def test_rejects_duplicate_no_issues_evidence_in_initial_final_window(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        comments = [
            marker(1, "initial", HEAD),
            no_issues_review(3, HEAD, created_at="2026-08-29T03:01:30Z"),
            no_issues_review(4, HEAD, created_at="2026-08-29T03:01:31Z"),
            marker(2, "final", HEAD),
        ]
        self.assertTrue(
            self.errors(pull_request=pull_request, threads=threads, comments=comments)
        )

    def test_rejects_duplicate_no_issues_evidence_after_final_marker(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        comments = [
            marker(1, "initial", HEAD),
            no_issues_review(3, HEAD, created_at="2026-08-29T03:01:30Z"),
            marker(2, "final", HEAD),
            no_issues_review(4, HEAD, created_at="2026-08-29T03:02:30Z"),
            no_issues_review(5, HEAD, created_at="2026-08-29T03:02:31Z"),
        ]
        self.assertTrue(
            self.errors(pull_request=pull_request, threads=threads, comments=comments)
        )

    def test_no_issues_evidence_requires_matching_strict_timestamps(self) -> None:
        variants = {
            "missing created_at": {"created_at": "__missing__"},
            "missing updated_at": {"updated_at": "__missing__"},
            "null created_at": {"created_at": None},
            "null updated_at": {"updated_at": None},
            "mismatch": {"updated_at": "2026-08-29T03:01:31Z"},
            "reverse": {"created_at": "2026-08-29T03:02:00Z"},
        }
        for name, changes in variants.items():
            with self.subTest(name=name):
                pull_request, threads, _ = successful_state()
                pull_request["reviews"] = []
                evidence = no_issues_review(3, HEAD)
                evidence.update(changes)
                for field in ("created_at", "updated_at"):
                    if evidence.get(field) == "__missing__":
                        evidence.pop(field)
                comments = [
                    marker(1, "initial", HEAD),
                    evidence,
                    marker(2, "final", HEAD),
                ]
                self.assertTrue(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    )
                )

    def test_rejects_untrusted_or_malformed_no_issues_evidence(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        valid = no_issues_review(3, HEAD)
        variants = {
            "other author": no_issues_review(3, HEAD, login="reviewer"),
            "different head": no_issues_review(3, INITIAL_HEAD),
            "short prefix": no_issues_review(3, HEAD, prefix_length=9),
            "before marker": no_issues_review(
                3, HEAD, created_at="2026-08-29T03:00:59Z"
            ),
            "marker boundary": no_issues_review(
                3, HEAD, created_at="2026-08-29T03:01:00Z"
            ),
            "phase confusion": no_issues_review(
                3, HEAD, created_at="2026-08-29T03:02:30Z"
            ),
            "malformed body": no_issues_review(
                3,
                HEAD,
                body=(
                    "Codex Review: Didn't find any major issues\n"
                    "**Reviewed commit:** `aaaaaaaaa`"
                ),
            ),
            "prefix only": no_issues_review(
                3, HEAD, body=NO_ISSUES_PREFIX
            ),
            "missing reviewed commit": no_issues_review(
                3, HEAD, body=f"{NO_ISSUES_PREFIX}. Hooray!"
            ),
            "duplicate result": no_issues_review(
                3, HEAD, body=(
                    f"{NO_ISSUES_PREFIX}. Hooray! Delightful!\n\n"
                    f"**Reviewed commit:** `{HEAD[:10]}`"
                ),
            ),
            "duplicate review line": no_issues_review(
                3, HEAD, body=(
                    f"{NO_ISSUES_PREFIX}. Hooray!\n"
                    f"{NO_ISSUES_PREFIX}. Hooray!\n\n"
                    f"**Reviewed commit:** `{HEAD[:10]}`"
                ),
            ),
            "duplicate reviewed commit": no_issues_review(
                3, HEAD, body=(
                    f"{NO_ISSUES_PREFIX}. Hooray!\n\n"
                    f"**Reviewed commit:** `{HEAD[:10]}`\n"
                    f"**Reviewed commit:** `{HEAD[:10]}`"
                ),
            ),
            "sentence injection": no_issues_review(
                3, HEAD, body=(
                    f"{NO_ISSUES_PREFIX}.\nInjected sentence.\n\n"
                    f"**Reviewed commit:** `{HEAD[:10]}`"
                ),
            ),
            "reaction only": {**valid, "body": "LGTM", "reactions": {"+1": 1}},
        }
        for name, evidence in variants.items():
            with self.subTest(name=name):
                comments = [
                    marker(1, "initial", HEAD),
                    evidence,
                    marker(2, "final", HEAD),
                ]
                self.assertTrue(
                    self.errors(
                        pull_request=pull_request,
                        threads=threads,
                        comments=comments,
                    )
                )

    def test_rejects_no_issues_evidence_for_wrong_phase_after_final_marker(self) -> None:
        pull_request, threads, _ = successful_state()
        pull_request["reviews"] = []
        comments = [
            marker(1, "initial", HEAD),
            no_issues_review(3, HEAD, created_at="2026-08-29T03:02:30Z"),
            marker(2, "final", HEAD),
        ]
        self.assertTrue(
            self.errors(pull_request=pull_request, threads=threads, comments=comments)
        )

    def test_rejects_same_head_pr_body_edit_with_stale_marker_digests(self) -> None:
        pull_request, _, comments = successful_state()
        pull_request["body"] = "Closes #65"
        errors = " ".join(self.errors(pull_request=pull_request, comments=comments))
        self.assertIn("initial review marker のPR本文digest", errors)
        self.assertIn("final review marker のPR本文digest", errors)

    def test_accepts_exact_utf8_unicode_body_digest(self) -> None:
        body = "Closes #64\n本文"
        pull_request, _, comments = successful_state()
        pull_request["body"] = body
        comments[0] = marker(1, "initial", HEAD, body=body)
        comments[1] = marker(2, "final", HEAD, body=body)
        self.assertEqual(self.errors(pull_request=pull_request, comments=comments), [])

    def test_rejects_invalid_pr_body_for_review_evidence(self) -> None:
        for body in (None, "Closes #64\0", "Closes #64\ud800"):
            with self.subTest(body=repr(body)):
                pull_request, _, comments = successful_state()
                pull_request["body"] = body
                self.assertIn(
                    "PR本文の review marker digest",
                    " ".join(
                        self.errors(pull_request=pull_request, comments=comments)
                    ),
                )

    def test_rejects_malformed_or_wrong_marker_body_digest(self) -> None:
        pull_request, _, comments = successful_state()
        for comment in comments:
            marker_body = comment["body"]
            assert isinstance(marker_body, str)
            comment["body"] = marker_body.replace(BODY_SHA256, "A" * 64)
        errors = " ".join(self.errors(comments=comments))
        self.assertIn("initial review marker がありません", errors)
        self.assertIn("final review marker がありません", errors)

    def test_review_marker_requires_canonical_spacing_order_and_lowercase_hex(self) -> None:
        canonical = marker(1, "initial", INITIAL_HEAD)
        body = canonical["body"]
        assert isinstance(body, str)
        malformed = {
            "opening-space": body.replace("<!-- krr-review", "<!--\tkrr-review"),
            "field-space": body.replace("phase=initial head=", "phase=initial  head="),
            "closing-space": body.replace(" -->", "  -->"),
            "uppercase-head": body.replace(f"head={INITIAL_HEAD}", f"head={INITIAL_HEAD.upper()}"),
            "prefix": body.replace("<!--", "prefix<!--"),
            "suffix": body.replace(" -->", " -->suffix"),
            "line-break": body.replace(" head=", "\nhead="),
            "attribute-order": body.replace(
                f"phase=initial head={INITIAL_HEAD} body-sha256={BODY_SHA256}",
                f"phase=initial body-sha256={BODY_SHA256} head={INITIAL_HEAD}",
            ),
        }
        self.assertEqual(
            subject._review_markers([canonical], "HiroyukiFuruno"),
            [("initial", INITIAL_HEAD, BODY_SHA256, canonical)],
        )
        for name, value in malformed.items():
            with self.subTest(name=name):
                rejected = {**canonical, "body": value}
                self.assertEqual(
                    subject._review_markers([rejected], "HiroyukiFuruno"), []
                )

    def test_rejects_final_evidence_before_referenced_issue_edit(self) -> None:
        pull_request, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[1]["submittedAt"] = "2026-08-29T03:04:01Z"
        errors = self.errors(
            pull_request=pull_request,
            referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),)
        )
        self.assertIn("marker が参照Issue更新後", " ".join(errors))

    def test_accepts_final_evidence_after_referenced_issue_edit(self) -> None:
        pull_request, _, comments = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:01Z"
        )
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[1]["submittedAt"] = "2026-08-29T03:04:02Z"
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                comments=comments,
                referenced_issues=(self.issue(64, "2026-08-29T03:04:00Z"),),
            ),
            [],
        )

    def test_uses_latest_referenced_issue_edit_for_final_evidence(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[1]["submittedAt"] = "2026-08-29T03:04:01Z"
        errors = self.errors(
            pull_request=pull_request,
            comments=comments,
            referenced_issues=(
                self.issue(64, "2026-08-29T03:03:00Z"),
                self.issue(65, "2026-08-29T03:05:00Z"),
            ),
        )
        self.assertIn("参照Issue更新後", " ".join(errors))

    def test_rejects_malformed_referenced_issue_timestamp(self) -> None:
        errors = self.errors(
            referenced_issues=(self.issue(64, "not-a-timestamp"),)
        )
        self.assertIn("snapshot", " ".join(errors))

    def test_rejects_missing_referenced_issue_timestamp(self) -> None:
        errors = self.errors(referenced_issues=(self.issue(64, ""),))
        self.assertIn("snapshot", " ".join(errors))

    def test_closing_contract_accepts_pr_72_style_body(self) -> None:
        self.assertEqual(
            subject.closing_reference_errors(
                repository="owner/repo",
                body="Closes #64",
                referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),),
            ),
            [],
        )

    def test_closing_contract_rejects_zero_canonical_open_issues(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="",
            referenced_issues=(),
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_extra_closing_reference_without_a_canonical_issue(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Fixes #64",
            referenced_issues=(),
        )
        self.assertIn("余分=#64", " ".join(errors))

    def test_closing_contract_rejects_missing_wrong_and_refs_only_references(self) -> None:
        referenced_issues = (self.issue(64, "2026-08-29T03:03:00Z"),)
        for body in ("", "Closes #65", "Refs #64"):
            with self.subTest(body=body):
                errors = subject.closing_reference_errors(
                    repository="owner/repo",
                    body=body,
                    referenced_issues=referenced_issues,
                )
                self.assertIn("#64", " ".join(errors))

    def test_closing_contract_rejects_extra_same_repo_closing_reference(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64\nFixes #65",
            referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),),
        )
        self.assertIn("余分=#65", " ".join(errors))

    def test_closing_contract_rejects_multiple_matching_issues(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64\nFixes https://github.com/owner/repo/issues/65",
            referenced_issues=(
                self.issue(64, "2026-08-29T03:03:00Z"),
                self.issue(65, "2026-08-29T03:03:00Z"),
            ),
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_multiple_keyword_variants_and_same_repo_url(self) -> None:
        referenced_issues = (
            self.issue(64, "2026-08-29T03:03:00Z"),
            self.issue(65, "2026-08-29T03:03:00Z"),
            self.issue(66, "2026-08-29T03:03:00Z"),
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body=(
                "fixed #64\n"
                "Resolve: https://github.com/owner/repo/issues/65\n"
                "CLOSED #66\n"
                "Fixes https://github.com/other/repo/issues/67"
            ),
            referenced_issues=referenced_issues,
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_closed_canonical_issue(self) -> None:
        closed_issue = subject.issue_contract.Issue(
            number=64,
            state="CLOSED",
            body="Issue body",
            url="https://github.com/owner/repo/issues/64",
            updated_at="2026-08-29T03:03:00Z",
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64",
            referenced_issues=(closed_issue,),
        )
        self.assertIn("OPEN", " ".join(errors))

    def test_closing_contract_rejects_noncanonical_issue_url(self) -> None:
        issue = subject.issue_contract.Issue(
            number=64,
            state="OPEN",
            body="Issue body",
            url="https://github.com/example/other/issues/64",
            updated_at="2026-08-29T03:03:00Z",
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64",
            referenced_issues=(issue,),
        )
        self.assertIn("canonical", " ".join(errors))

    def test_open_pull_request_contract_allows_only_the_current_canonical_closer(self) -> None:
        issue = self.issue(64, "2026-08-29T03:03:00Z")

        def pull_requests(count: int, *, sibling_is_draft: bool) -> list[dict[str, object]]:
            return current_canonical_closer() + [
                {
                    "number": 1_000 + index,
                    "isDraft": sibling_is_draft,
                    "body": "Closes #64",
                }
                for index in range(count)
            ]

        self.assertEqual(
            subject.closing_open_pull_request_errors(
                repository="owner/repo",
                current_pull_request=72,
                referenced_issues=(issue,),
                open_pull_requests=pull_requests(0, sibling_is_draft=False),
            ),
            [],
        )
        for sibling_is_draft in (False, True):
            with self.subTest(sibling_is_draft=sibling_is_draft):
                open_pull_requests = pull_requests(1, sibling_is_draft=sibling_is_draft)
                errors = subject.closing_open_pull_request_errors(
                    repository="owner/repo",
                    current_pull_request=72,
                    referenced_issues=(issue,),
                    open_pull_requests=open_pull_requests,
                )
                self.assertIn("open PRは自身だけ", " ".join(errors))
                self.assertIn("#72, #1000", " ".join(errors))

    def test_open_closer_snapshot_excludes_fork_and_nondefault_base_pull_requests(self) -> None:
        def node(number: int, *, base: str, head_repository: str) -> dict[str, object]:
            return {
                "number": number, "isDraft": True, "body": "Closes #64",
                "baseRefName": base, "headRefOid": f"{number:040x}",
                "headRepository": {"nameWithOwner": head_repository},
            }

        payload = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [
                            node(72, base="master", head_repository="owner/repo"),
                            node(73, base="master", head_repository="fork/repo"),
                            node(74, base="release/v0.4", head_repository="owner/repo"),
                        ],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            snapshot = subject._open_pull_requests("owner/repo")
        self.assertEqual([item["number"] for item in snapshot], [72])
        self.assertEqual(
            subject._open_pull_request_closers(
                repository="owner/repo", issue_number=64, open_pull_requests=snapshot,
            ),
            {72},
        )

    def test_open_closer_snapshot_rejects_missing_or_invalid_governance_metadata(self) -> None:
        def response(node: dict[str, object]) -> dict[str, object]:
            return {
                "data": {
                    "repository": {
                        "defaultBranchRef": {"name": "master"},
                        "pullRequests": {
                            "nodes": [node],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        },
                    }
                }
            }

        valid = {
            "number": 72, "isDraft": True, "body": "Closes #64",
            "baseRefName": "master", "headRefOid": "a" * 40,
            "headRepository": {"nameWithOwner": "owner/repo"},
        }
        variants = {
            "base": {**valid, "baseRefName": None},
            "base-unsafe": {**valid, "baseRefName": "master\n"},
            "head": {**valid, "headRefOid": "not-a-sha"},
            "repository-name": {**valid, "headRepository": {"nameWithOwner": ""}},
        }
        for name, node in variants.items():
            with self.subTest(name=name), patch.object(subject, "_gh_json", return_value=response(node)):
                with self.assertRaises(TypeError):
                    subject._open_pull_requests("owner/repo")

    def test_open_closer_snapshot_skips_unavailable_head_repository(self) -> None:
        payload = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [
                            {
                                "number": 71, "isDraft": True, "body": "Closes #64",
                                "baseRefName": "master", "headRefOid": "not-a-sha",
                                "headRepository": None,
                            },
                            {
                                "number": 72, "isDraft": True, "body": "Closes #64",
                                "baseRefName": "master", "headRefOid": "a" * 40,
                                "headRepository": {"nameWithOwner": "owner/repo"},
                            },
                        ],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            snapshot = subject._open_pull_requests("owner/repo")
        self.assertEqual([item["number"] for item in snapshot], [72])

    def test_open_closer_snapshot_rejects_duplicate_ungoverned_pull_request_numbers(self) -> None:
        node = {
            "number": 73, "isDraft": True, "body": "Closes #64",
            "baseRefName": "master", "headRefOid": "a" * 40,
            "headRepository": {"nameWithOwner": "fork/repo"},
        }
        payload = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [node, dict(node)],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "duplicate"):
                subject._open_pull_requests("owner/repo")

    def test_open_closer_snapshot_rejects_default_branch_drift_between_pages(self) -> None:
        def page(
            *, default_branch: str, number: int, has_next_page: bool, cursor: str | None
        ) -> dict[str, object]:
            return {
                "data": {
                    "repository": {
                        "defaultBranchRef": {"name": default_branch},
                        "pullRequests": {
                            "nodes": [{
                                "number": number, "isDraft": True, "body": "Closes #64",
                                "baseRefName": default_branch, "headRefOid": f"{number:040x}",
                                "headRepository": {"nameWithOwner": "owner/repo"},
                            }],
                            "pageInfo": {"hasNextPage": has_next_page, "endCursor": cursor},
                        },
                    }
                }
            }

        with patch.object(
            subject, "_gh_json", side_effect=[
                page(default_branch="master", number=72, has_next_page=True, cursor="next"),
                page(default_branch="release/v0.4", number=73, has_next_page=False, cursor=None),
            ]
        ):
            with self.assertRaisesRegex(TypeError, "default branch changed"):
                subject._open_pull_requests("owner/repo")

    def test_open_closer_snapshot_uses_strict_git_ref_names_for_default_and_base(self) -> None:
        def response(default_branch: str, base_branch: str) -> dict[str, object]:
            return {
                "data": {
                    "repository": {
                        "defaultBranchRef": {"name": default_branch},
                        "pullRequests": {
                            "nodes": [{
                                "number": 72, "isDraft": True, "body": "Closes #64",
                                "baseRefName": base_branch, "headRefOid": "a" * 40,
                                "headRepository": {"nameWithOwner": "owner/repo"},
                            }],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        },
                    }
                }
            }

        invalid = (".", "..", "feature//x", "/x", "x/")
        for value in invalid:
            with self.subTest(field="defaultBranchRef.name", value=value), patch.object(
                subject, "_gh_json", return_value=response(value, value)
            ):
                with self.assertRaises(TypeError):
                    subject._open_pull_requests("owner/repo")
            with self.subTest(field="baseRefName", value=value), patch.object(
                subject, "_gh_json", return_value=response("master", value)
            ):
                with self.assertRaises(TypeError):
                    subject._open_pull_requests("owner/repo")

        valid = "feature/x.y-z_1"
        with patch.object(subject, "_gh_json", return_value=response(valid, valid)):
            self.assertEqual(
                [item["number"] for item in subject._open_pull_requests("owner/repo")],
                [72],
            )

    def test_open_pull_requests_reads_all_pages(self) -> None:
        def payload(nodes: list[dict[str, object]], has_next_page: bool, cursor: str | None) -> dict[str, object]:
            return {
                "data": {
                    "repository": {
                        "defaultBranchRef": {"name": "master"},
                        "pullRequests": {
                            "nodes": nodes,
                            "pageInfo": {
                                "hasNextPage": has_next_page,
                                "endCursor": cursor,
                            },
                        }
                    }
                }
            }

        def node(number: int, draft: bool) -> dict[str, object]:
            return {
                "number": number, "isDraft": draft, "body": "Closes #64",
                "baseRefName": "master", "headRefOid": f"{number:040x}",
                "headRepository": {"nameWithOwner": "owner/repo"},
            }

        first_page = payload([node(71, False)], True, "next-page")
        second_page = payload([node(72, True)], False, None)

        def gh_json(*arguments: str) -> object:
            if "cursor=next-page" in arguments:
                return second_page
            return first_page

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                [item["number"] for item in subject._open_pull_requests("owner/repo")],
                [71, 72],
            )

    def test_open_pull_requests_fails_closed_when_initial_page_changes_at_race_fence(self) -> None:
        def payload(number: int) -> dict[str, object]:
            return {
                "data": {"repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [{
                            "number": number,
                            "isDraft": True,
                            "body": "Closes #64",
                            "baseRefName": "master",
                            "headRefOid": f"{number:040x}",
                            "headRepository": {"nameWithOwner": "owner/repo"},
                        }],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }},
            }

        with patch.object(
            subject,
            "_gh_json",
            side_effect=[rate_limited(payload(72)), rate_limited(payload(73))],
        ):
            with self.assertRaisesRegex(ValueError, "open pull requests changed"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_fails_closed_on_malformed_response(self) -> None:
        malformed = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [{"number": 72, "isDraft": True, "body": None}],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=malformed):
            with self.assertRaisesRegex(TypeError, "body"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_fails_closed_on_duplicate_numbers(self) -> None:
        node = {
            "number": 72, "isDraft": True, "body": "Closes #64",
            "baseRefName": "master", "headRefOid": "a" * 40,
            "headRepository": {"nameWithOwner": "owner/repo"},
        }
        first_page = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [node],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        duplicate_page = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [node],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", side_effect=[first_page, duplicate_page]):
            with self.assertRaisesRegex(TypeError, "duplicate"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_fails_closed_on_repeated_cursor(self) -> None:
        first_node = {
            "number": 72, "isDraft": True, "body": "Closes #64",
            "baseRefName": "master", "headRefOid": "a" * 40,
            "headRepository": {"nameWithOwner": "owner/repo"},
        }
        second_node = {
            "number": 73, "isDraft": True, "body": "Closes #64",
            "baseRefName": "master", "headRefOid": "b" * 40,
            "headRepository": {"nameWithOwner": "owner/repo"},
        }
        first_page = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [first_node],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        repeated_cursor_page = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [second_node],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        with patch.object(
            subject, "_gh_json", side_effect=[first_page, repeated_cursor_page]
        ):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_rejects_duplicate_live_governed_heads(self) -> None:
        duplicate_head = "d" * 40
        response = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [
                            {
                                "number": 72,
                                "isDraft": True,
                                "body": "Closes #64",
                                "baseRefName": "master",
                                "headRefOid": duplicate_head,
                                "headRepository": {"nameWithOwner": "owner/repo"},
                            },
                            {
                                "number": 73,
                                "isDraft": True,
                                "body": "Closes #64",
                                "baseRefName": "master",
                                "headRefOid": duplicate_head.upper(),
                                "headRepository": {"nameWithOwner": "owner/repo"},
                            },
                        ],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=response) as gh_json:
            with self.assertRaisesRegex(TypeError, "duplicate governed head SHA"):
                subject._open_pull_requests("owner/repo")
        query = gh_json.call_args.args[3]
        self.assertIn("rateLimit { cost remaining resetAt }", query)
        self.assertIn("defaultBranchRef { name }", query)
        self.assertIn("baseRefName headRefOid", query)
        self.assertIn("headRepository { nameWithOwner }", query)

    def test_open_pull_request_snapshot_rejects_duplicate_governed_heads(self) -> None:
        snapshot = json.dumps(
            [
                {"number": 72, "isDraft": True, "body": "Closes #64", "head_sha": "d" * 40},
                {"number": 73, "isDraft": True, "body": "Closes #64", "head_sha": "D" * 40},
            ]
        )
        with patch("builtins.open", mock_open(read_data=snapshot)):
            with self.assertRaisesRegex(TypeError, "duplicate governed head SHA"):
                subject._open_pull_request_snapshot("/immutable/open-pulls.json")

    def test_rejects_ready_pr_before_the_gate(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["isDraft"] = False
        self.assertIn("Draft", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_final_review_marker_for_an_old_head(self) -> None:
        _, _, comments = successful_state()
        comments[1] = marker(2, "final", INITIAL_HEAD)
        self.assertIn("最新HEAD", " ".join(self.errors(comments=comments)))

    def test_rejects_newer_final_review_marker_for_an_old_head(self) -> None:
        _, _, comments = successful_state()
        comments.append(marker(3, "final", INITIAL_HEAD))
        self.assertIn("最新HEAD", " ".join(self.errors(comments=comments)))

    def test_rejects_ambiguous_latest_initial_or_final_marker_timestamp(self) -> None:
        for phase, head in (("initial", INITIAL_HEAD), ("final", HEAD)):
            with self.subTest(phase=phase):
                _, _, comments = successful_state()
                duplicate = marker(3, phase, head)
                duplicate["created_at"] = comments[0 if phase == "initial" else 1]["created_at"]
                duplicate["updated_at"] = comments[0 if phase == "initial" else 1]["updated_at"]
                comments.append(duplicate)
                errors = " ".join(self.errors(comments=comments))
                self.assertIn("最新時刻が曖昧", errors)

    def test_rejects_unresolved_review_threads(self) -> None:
        threads = [{"id": "thread-1", "isResolved": False}]
        self.assertIn("未resolve", " ".join(self.errors(threads=threads)))

    def test_rejects_failed_or_pending_checks(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": "",
            }
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_ignores_trusted_governance_pending_check(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance (trusted check)",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_ignores_review_latch_failure_when_regular_ci_succeeds(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance review latch",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_rejects_regular_ci_failure_alongside_review_latch_failure(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance review latch",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_pending_regular_ci_alongside_trusted_check(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance (trusted check)",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_required_status_check_snapshot_rejects_invalid_contracts(self) -> None:
        self.required_checks_patcher.stop()
        valid_checks = [
            {"context": "CI", "app_id": None},
            {"context": subject._TRUSTED_CHECK, "app_id": 42},
            {"context": subject._LATCH_CHECK, "app_id": 15368},
        ]
        variants: dict[str, object] = {
            "not_strict": {"strict": False, "contexts": ["CI"], "checks": valid_checks},
            "missing_contexts": {"strict": True, "checks": valid_checks},
            "missing_checks": {"strict": True, "contexts": ["CI"]},
            "duplicate_context": {"strict": True, "contexts": ["CI", "CI"], "checks": valid_checks},
            "contexts_disagree": {"strict": True, "contexts": ["CI"], "checks": valid_checks},
            "duplicate_check_context": {
                "strict": True,
                "contexts": ["CI", subject._TRUSTED_CHECK, subject._LATCH_CHECK],
                "checks": [*valid_checks, {"context": "CI", "app_id": 7}],
            },
            "invalid_app": {
                "strict": True,
                "contexts": ["CI", subject._TRUSTED_CHECK, subject._LATCH_CHECK],
                "checks": [
                    {"context": "CI", "app_id": True},
                    *valid_checks[1:],
                ],
            },
        }
        for name, response in variants.items():
            with self.subTest(name=name), patch.object(
                subject, "_gh_json", return_value=response
            ):
                with self.assertRaises((TypeError, ValueError)):
                    subject._required_status_check_snapshot("owner/repo", "master")

    def test_required_non_self_contexts_must_have_one_completed_success(self) -> None:
        required = (
            ("CI", "Lint", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                ("Lint", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        completed_ci = {
            "__typename": "CheckRun",
            "name": "CI",
            "status": "COMPLETED",
            "conclusion": "SUCCESS",
        }
        completed_lint = {
            "__typename": "CheckRun",
            "name": "Lint",
            "status": "COMPLETED",
            "conclusion": "SUCCESS",
        }
        variants = {
            "missing": [completed_ci],
            "duplicate": [completed_ci, {**completed_ci}, completed_lint],
            "pending": [completed_ci, {**completed_lint, "status": "IN_PROGRESS", "conclusion": None}],
            "non_required_cannot_substitute": [
                completed_ci,
                {**completed_lint, "name": "Unrequired"},
            ],
        }
        for name, rollup in variants.items():
            with self.subTest(name=name):
                errors = subject._status_check_rollup_errors(rollup, required)
                self.assertTrue(errors)
        self.assertEqual(
            subject._status_check_rollup_errors(
                [completed_ci, completed_lint], required
            ),
            [],
        )

    def test_null_app_required_context_accepts_terminal_success_status_context(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        rollup = [
            {"__typename": "StatusContext", "context": "CI", "state": "SUCCESS"},
            {
                "__typename": "CheckRun",
                "name": subject._LATCH_CHECK,
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
            {
                "__typename": "CheckRun",
                "name": subject._TRUSTED_CHECK,
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(subject._status_check_rollup_errors(rollup, required), [])

    def test_status_context_is_never_accepted_for_app_bound_required_context(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        rollup = [
            {"__typename": "StatusContext", "context": "CI", "state": "SUCCESS"},
            {
                "__typename": "CheckRun",
                "name": subject._LATCH_CHECK,
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
            {
                "__typename": "CheckRun",
                "name": subject._TRUSTED_CHECK,
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        errors = subject._status_check_rollup_errors(rollup, required)
        self.assertIn("CI", " ".join(errors))

    def test_null_app_status_context_requires_exact_success_state(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        for state in ("PENDING", "FAILURE", "success", None):
            with self.subTest(state=state):
                rollup = [
                    {"__typename": "StatusContext", "context": "CI", "state": state},
                    {
                        "__typename": "CheckRun",
                        "name": subject._LATCH_CHECK,
                        "status": "COMPLETED",
                        "conclusion": "SUCCESS",
                    },
                    {
                        "__typename": "CheckRun",
                        "name": subject._TRUSTED_CHECK,
                        "status": "COMPLETED",
                        "conclusion": "SUCCESS",
                    },
                ]
                self.assertTrue(subject._status_check_rollup_errors(rollup, required))

    def test_app_bound_required_context_binds_rollup_to_exact_check_run(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        details_url = "https://github.com/owner/repo/actions/runs/123/job/456"
        rollup = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
                "databaseId": 99,
                "detailsUrl": details_url,
            }
        ]
        producer = {
            "id": 99,
            "name": "CI",
            "head_sha": HEAD,
            "app": {"id": 7},
            "details_url": details_url,
            "status": "completed",
            "conclusion": "success",
        }
        historical_failure = {
            **producer,
            "id": 98,
            "details_url": "https://github.com/owner/repo/actions/runs/122/job/455",
            "conclusion": "failure",
        }
        with patch.object(
            subject,
            "_gh_json",
            return_value=[
                {"check_runs": [historical_failure]},
                {"check_runs": [producer]},
            ],
        ) as gh_json:
            self.assertEqual(
                subject._required_check_run_producer_errors(
                    "owner/repo", HEAD, rollup, required
                ),
                [],
            )
        arguments = gh_json.call_args.args
        self.assertNotIn("--paginate", arguments)
        self.assertNotIn("--slurp", arguments)
        self.assertIn("page=1", arguments[-1])
        self.assertIn("app_id=7", arguments[-1])

    def test_app_bound_required_context_rejects_unbound_or_malformed_producer(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        details_url = "https://github.com/owner/repo/actions/runs/123/job/456"
        rollup = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
                "databaseId": 99,
                "detailsUrl": details_url,
            }
        ]
        producer = {
            "id": 99,
            "name": "CI",
            "head_sha": HEAD,
            "app": {"id": 7},
            "details_url": details_url,
            "status": "completed",
            "conclusion": "success",
        }
        variants = {
            "missing": [],
            "wrong_app": [{**producer, "app": {"id": 8}}],
            "duplicate": [producer, {**producer}],
            "malformed_app": [{**producer, "app": {"id": True}}],
            "malformed_id": [{**producer, "id": True}],
            "malformed_url": [{**producer, "details_url": "not-a-url"}],
            "pending": [{**producer, "status": "in_progress", "conclusion": None}],
            "failure": [{**producer, "conclusion": "failure"}],
            "cancelled": [{**producer, "conclusion": "cancelled"}],
            "malformed_terminal": [{**producer, "status": None, "conclusion": 1}],
        }
        for name, runs in variants.items():
            with self.subTest(name=name), patch.object(
                subject, "_gh_json", return_value=[{"check_runs": runs}]
            ):
                errors = subject._required_check_run_producer_errors(
                    "owner/repo", HEAD, rollup, required
                )
                self.assertTrue(errors)
                self.assertIn("CI", " ".join(errors))

        malformed_rollup = [{**rollup[0], "__typename": "StatusContext"}]
        with patch.object(subject, "_gh_json") as gh_json:
            errors = subject._required_check_run_producer_errors(
                "owner/repo", HEAD, malformed_rollup, required
            )
        self.assertTrue(errors)
        gh_json.assert_not_called()
        for invalid_rollup in [
            [{**rollup[0], "databaseId": True}],
            [{**rollup[0], "id": True}],
            [{**rollup[0], "detailsUrl": "not-a-url"}],
        ]:
            with self.subTest(rollup=invalid_rollup), patch.object(
                subject, "_gh_json"
            ) as gh_json:
                errors = subject._required_check_run_producer_errors(
                    "owner/repo", HEAD, invalid_rollup, required
                )
            self.assertTrue(errors)
            gh_json.assert_not_called()

    def test_legacy_null_app_required_context_does_not_require_a_check_run_producer(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        rollup = [
            {
                "__typename": "StatusContext",
                "context": "CI",
                "state": "SUCCESS",
            }
        ]
        with patch.object(subject, "_gh_json") as gh_json:
            self.assertEqual(
                subject._required_check_run_producer_errors(
                    "owner/repo", HEAD, rollup, required
                ),
                [],
            )
        gh_json.assert_not_called()

    def test_final_snapshot_rejects_stale_app_bound_check_run_producer(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        details_url = "https://github.com/owner/repo/actions/runs/123/job/456"
        final_snapshot, _, _ = successful_state()
        final_snapshot["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
                "databaseId": 99,
                "detailsUrl": details_url,
            }
        ]
        pending_producer = {
            "id": 99,
            "name": "CI",
            "head_sha": HEAD,
            "app": {"id": 7},
            "details_url": details_url,
            "status": "in_progress",
            "conclusion": None,
        }
        with patch.object(
            subject,
            "_gh_json",
            side_effect=[final_snapshot, [{"check_runs": [pending_producer]}]],
        ), patch.object(
            subject, "_required_status_check_snapshot", return_value=required
        ):
            with self.assertRaisesRegex(ValueError, "Check Run producer changed"):
                subject._verify_final_readiness_snapshot_unchanged(
                    repository="owner/repo",
                    pull_request=72,
                    initial_base="c" * 40,
                    initial_head=HEAD,
                    initial_base_branch="master",
                    initial_body=BODY,
                    initial_updated_at="2026-08-29T03:03:00Z",
                    initial_required_checks=required,
                    initial_issue_identity=(),
                    initial_closers=frozenset(),
                )

    def test_main_rechecks_app_bound_producer_before_and_after_readiness(self) -> None:
        required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        details_url = "https://github.com/owner/repo/actions/runs/123/job/456"
        pull_request, threads, comments = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
                "databaseId": 99,
                "detailsUrl": details_url,
            }
        ]
        producer = {
            "id": 99,
            "name": "CI",
            "head_sha": HEAD,
            "app": {"id": 7},
            "details_url": details_url,
            "status": "completed",
            "conclusion": "success",
        }
        check_run_calls = 0

        def gh_json(*arguments: str) -> object:
            nonlocal check_run_calls
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[:1] == ("api",) and "check-runs?" in arguments[-1]:
                check_run_calls += 1
                return [{"check_runs": [producer]}]
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(
            subject, "_required_status_check_snapshot", return_value=required
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)
        self.assertEqual(check_run_calls, 2)

    def test_draft_rejects_invalid_required_governance_app_binding(self) -> None:
        pull_request, threads, comments = successful_state()
        invalid_required = (
            ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                (subject._LATCH_CHECK, 15369),
                (subject._TRUSTED_CHECK, None),
            ),
        )
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
        ), patch.object(subject, "closing_reference_errors", return_value=[]), patch.object(
            subject,
            "_required_status_check_snapshot",
            return_value=invalid_required,
        ):
            self.assertEqual(
                subject.main(["--pr", "72", "--repository", "owner/repo"]), 1
            )

    def test_keeps_legacy_governance_context_compatibility(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "PR governance",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_rejects_final_marker_without_bot_completion(self) -> None:
        pull_request, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        pull_request["reviews"] = [reviews[0]]
        self.assertIn(
            "final",
            " ".join(self.errors(pull_request=pull_request)),
        )

    def test_rejects_initial_marker_without_post_marker_bot_review(self) -> None:
        pull_request, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        pull_request["reviews"] = [reviews[1]]
        self.assertIn("initial", " ".join(self.errors(pull_request=pull_request)))

    def test_accepts_initial_review_on_current_head_before_final_marker(self) -> None:
        pull_request, _, comments = successful_state()
        comments[0] = marker(1, "initial", HEAD)
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["commit"] = {"oid": HEAD}
        self.assertEqual(self.errors(pull_request=pull_request, comments=comments), [])

    def test_rejects_latest_initial_marker_after_final_marker(self) -> None:
        _, _, comments = successful_state()
        comments.append(marker(3, "initial", HEAD))
        errors = self.errors(comments=comments)
        self.assertIn("initial review marker は最新 final", " ".join(errors))

    def test_rejects_stale_initial_evidence_for_a_newer_initial_marker(self) -> None:
        _, _, comments = successful_state()
        latest_initial = marker(3, "initial", INITIAL_HEAD)
        latest_initial["created_at"] = "2026-08-29T03:01:45Z"
        latest_initial["updated_at"] = "2026-08-29T03:01:45Z"
        comments.append(latest_initial)
        errors = self.errors(comments=comments)
        self.assertIn("initial review に", " ".join(errors))

    def test_accepts_edited_marker_with_new_bot_review(self) -> None:
        pull_request, _, comments = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:00Z"
        )
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[1]["submittedAt"] = "2026-08-29T03:04:30Z"
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                comments=comments,
            ),
            [],
        )

    def test_rejects_edited_initial_marker_reused_as_final_evidence(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["submittedAt"] = "2026-08-29T03:00:30Z"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:03:00Z",
            }
        )
        comments[0]["body"] = (
            f"<!-- krr-review phase=final head={HEAD} body-sha256={BODY_SHA256} -->\n"
            "@codex review"
        )
        comments[0]["updated_at"] = "2026-08-29T03:04:00Z"
        comments.append(
            {
                "id": 3,
                "body": (
                    f"<!-- krr-review phase=initial head={INITIAL_HEAD} body-sha256={BODY_SHA256} -->\n"
                    "@codex review"
                ),
                "created_at": "2026-08-29T03:00:00Z",
                "updated_at": "2026-08-29T03:00:00Z",
                "user": {"login": "HiroyukiFuruno"},
            }
        )
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments)),
        )

    def test_rejects_review_submitted_in_same_second_as_marker_edit(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        comments[1]["updated_at"] = "2026-08-29T03:04:00Z"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:04:00Z",
            }
        )
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments)),
        )

    def test_accepts_review_submitted_after_marker_edit_second(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        comments[1]["updated_at"] = "2026-08-29T03:04:00Z"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:04:01Z",
            }
        )
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                comments=comments,
            ),
            [],
        )

    def test_rejects_review_completed_before_its_marker(self) -> None:
        pull_request, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["submittedAt"] = "2026-08-29T03:00:00Z"
        self.assertIn(
            "initial",
            " ".join(self.errors(pull_request=pull_request)),
        )

    def test_rejects_dismissed_review_as_final_evidence(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["state"] = "DISMISSED"
        reviews[1]["state"] = "DISMISSED"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "DISMISSED",
                "submittedAt": "2026-08-29T03:03:30Z",
            }
        )
        errors = self.errors(
            pull_request=pull_request,
            comments=comments,
        )
        self.assertIn("initial", " ".join(errors))
        self.assertIn("final", " ".join(errors))

    def test_accepts_approved_review_as_valid_evidence(self) -> None:
        pull_request, _, comments = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "APPROVED",
                "submittedAt": "2026-08-29T03:03:30Z",
            }
        )
        self.assertEqual(
            [],
            self.errors(
                pull_request=pull_request,
                comments=comments,
            ),
        )

    def test_rejects_marker_without_the_codex_review_trigger(self) -> None:
        _, _, comments = successful_state()
        for comment in comments:
            body = comment["body"]
            assert isinstance(body, str)
            comment["body"] = body.replace("\n@codex review", "")
        errors = " ".join(self.errors(comments=comments))
        self.assertIn("initial", errors)
        self.assertIn("final", errors)

    def test_rejects_missing_ci_results(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["statusCheckRollup"] = []
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_one_review_used_for_both_review_phases(self) -> None:
        pull_request, _, comments = successful_state()
        comments[0] = marker(1, "initial", HEAD)
        pull_request["reviews"] = [
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:03:00Z",
            }
        ]
        self.assertIn(
            "initial",
            " ".join(
                self.errors(
                    pull_request=pull_request,
                    comments=comments,
                )
            ),
        )

    def test_rejects_resolved_thread_without_author_reply(self) -> None:
        pull_request, _, comments = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}, "createdAt": "2026-08-29T03:00:30Z"},
                ],
            }
        ]

        errors = self.errors(
            pull_request=pull_request,
            threads=threads,
            comments=comments,
        )
        self.assertIn("reply", " ".join(errors).lower())

    def test_accepts_resolved_thread_with_author_reply_after_root(self) -> None:
        pull_request, _, comments = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}, "createdAt": "2026-08-29T03:00:30Z"},
                    {"author": {"login": "HiroyukiFuruno"}, "createdAt": "2026-08-29T03:00:45Z"},
                ],
            }
        ]

        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
            ),
            [],
        )

    def test_accepts_resolved_thread_with_trusted_maintainer_reply_to_bot_pr(self) -> None:
        pull_request, _, comments = successful_state()
        pull_request["author"] = {"login": "dependabot[bot]"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}},
                    {
                        "author": {"login": "maintainer"},
                        "authorAssociation": "MEMBER",
                    },
                ],
            }
        ]

        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
            ),
            [],
        )

    def test_rejects_resolved_thread_with_untrusted_reply_to_bot_pr(self) -> None:
        pull_request, _, comments = successful_state()
        pull_request["author"] = {"login": "dependabot[bot]"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}},
                    {
                        "author": {"login": "contributor"},
                        "authorAssociation": "CONTRIBUTOR",
                    },
                ],
            }
        ]

        errors = self.errors(
            pull_request=pull_request,
            threads=threads,
            comments=comments,
        )
        self.assertIn("reply", " ".join(errors).lower())

    def test_reads_issue_comments_past_the_first_api_page(self) -> None:
        pull_request = {
            "isDraft": True,
            "baseRefOid": "c" * 40,
            "baseRefName": "master",
            "headRefOid": HEAD,
            "body": "Closes #64",
            "author": {"login": "HiroyukiFuruno"},
            "updatedAt": "2026-08-29T03:03:00Z",
            "statusCheckRollup": [
                {
                    "__typename": "CheckRun",
                    "name": "CI",
                    "status": "COMPLETED",
                    "conclusion": "SUCCESS",
                }
            ],
            "reviews": [
                {
                    "author": {"login": BOT},
                    "commit": {"oid": HEAD},
                    "state": "COMMENTED",
                    "submittedAt": "2026-08-29T03:01:30Z",
                },
                {
                    "author": {"login": BOT},
                    "commit": {"oid": HEAD},
                    "state": "COMMENTED",
                    "submittedAt": "2026-08-29T03:03:30Z",
                },
            ],
        }
        filler = [
            {
                "id": index,
                "body": f"filler-{index}",
                "created_at": "2026-08-29T03:00:00Z",
                "user": {"login": "reviewer"},
            }
            for index in range(1, 101)
        ]
        all_comments = filler + [
            {
                "id": 31,
                "body": (
                    f"<!-- krr-review phase=initial head={HEAD} "
                    f"body-sha256={BODY_SHA256} -->\n@codex review"
                ),
                "created_at": "2026-08-29T03:01:00Z",
                "user": {"login": "HiroyukiFuruno"},
                "author_association": "OWNER",
            },
            {
                "id": 32,
                "body": (
                    f"<!-- krr-review phase=final head={HEAD} "
                    f"body-sha256={BODY_SHA256} -->\n@codex review"
                ),
                "created_at": "2026-08-29T03:03:00Z",
                "user": {"login": "HiroyukiFuruno"},
                "author_association": "OWNER",
            },
        ]

        def gh_json(*arguments: str) -> object:
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[0] == "api" and arguments[1].startswith(
                "repos/owner/repo/issues/72/comments"
            ):
                return filler if "page=1&" in arguments[-1] else all_comments[100:]
            if arguments[0:2] == ("api", "graphql"):
                return rate_limited({
                    "data": {
                        "repository": {
                            "pullRequest": {
                                "reviewThreads": {
                                    "nodes": [
                                        {
                                            "id": "thread-1",
                                            "isResolved": True,
                                            "comments": {
                                                "nodes": [
                                                    {"author": {"login": "reviewer"}},
                                                    {"author": {"login": "HiroyukiFuruno"}},
                                                ],
                                                "pageInfo": {
                                                    "hasNextPage": False,
                                                    "endCursor": None,
                                                },
                                            },
                                        }
                                    ],
                                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                                }
                            }
                        }
                    }
                })
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_reads_review_past_the_first_graphql_page(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request["reviews"] = []

        def gh_json(*arguments: str) -> object:
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[0] == "api" and arguments[1].startswith(
                "repos/owner/repo/issues/72/comments"
            ):
                return comments
            if arguments[0:2] == ("api", "graphql"):
                query = next((arg for arg in arguments if "reviews" in arg), "")
                if query:
                    has_cursor = any(
                        argument in {"reviews-cursor", "cursor=reviews-cursor"}
                        for argument in arguments
                    )
                    if not has_cursor:
                        review_nodes = [
                            {
                                "author": {"login": "reviewer"},
                                "commit": {"oid": INITIAL_HEAD},
                                "state": "COMMENTED",
                                "submittedAt": "2026-08-29T03:00:00Z",
                            }
                            for _ in range(100)
                        ]
                        return rate_limited({
                            "data": {
                                "repository": {
                                    "pullRequest": {
                                        "reviews": {
                                            "nodes": review_nodes,
                                            "pageInfo": {
                                                "hasNextPage": True,
                                                "endCursor": "reviews-cursor",
                                            },
                                        }
                                    }
                                }
                            }
                        })
                    return rate_limited({
                        "data": {
                            "repository": {
                                "pullRequest": {
                                    "reviews": {
                                        "nodes": [
                                            {
                                                "author": {"login": BOT},
                                                "commit": {"oid": HEAD},
                                                "state": "COMMENTED",
                                                "submittedAt": "2026-08-29T03:01:30Z",
                                            },
                                            {
                                                "author": {"login": BOT},
                                                "commit": {"oid": HEAD},
                                                "state": "COMMENTED",
                                                "submittedAt": "2026-08-29T03:03:30Z",
                                            },
                                        ],
                                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                                    }
                                }
                            }
                        }
                    })
                return rate_limited({
                    "data": {
                        "repository": {
                            "pullRequest": {
                                "reviewThreads": {
                                    "nodes": [
                                        {
                                            **thread,
                                            "comments": {
                                                "nodes": thread["comments"],
                                                "pageInfo": {
                                                    "hasNextPage": False,
                                                    "endCursor": None,
                                                },
                                            },
                                        }
                                        for thread in threads
                                    ],
                                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                                }
                            }
                        }
                    }
                })
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_outer_review_connections_accept_the_page_boundary(self) -> None:
        for connection_name, reader in (
            ("reviews", subject._reviews),
            ("reviewThreads", subject._review_threads),
        ):
            with self.subTest(connection=connection_name):
                calls: list[tuple[str, ...]] = []

                def gh_json(*arguments: str) -> object:
                    calls.append(arguments)
                    page_number = 1 if not any(
                        argument.startswith("cursor=") for argument in arguments
                    ) else len(calls)
                    if connection_name == "reviews":
                        nodes: list[dict[str, object]] = [{"id": page_number}]
                    else:
                        nodes = [
                            {
                                "id": f"thread-{page_number}",
                                "isResolved": True,
                                "comments": {
                                    "nodes": [],
                                    "pageInfo": {
                                        "hasNextPage": False,
                                        "endCursor": None,
                                    },
                                },
                            }
                        ]
                    return rate_limited(
                        {
                            "data": {
                                "repository": {
                                    "pullRequest": {
                                        connection_name: {
                                            "nodes": nodes,
                                            "pageInfo": {
                                                "hasNextPage": page_number
                                                < subject._MAX_REVIEW_CONNECTION_PAGES,
                                                "endCursor": f"cursor-{page_number}",
                                            },
                                        }
                                    }
                                }
                            }
                        },
                        # race fence の再読も実リクエストなので、接続のページは
                        # 初回と同じでも server remaining は通算で減少させる。
                        remaining=subject._GRAPHQL_REVIEW_BUDGET - len(calls),
                        cost=page_number % 3 + 1,
                    )

                with patch.object(subject, "_gh_json", side_effect=gh_json):
                    values = reader("owner/repo", 72)

                self.assertEqual(
                    len(values), subject._MAX_REVIEW_CONNECTION_PAGES
                )
                self.assertEqual(
                    len(calls), subject._MAX_REVIEW_CONNECTION_PAGES + 1
                )

    def test_outer_review_connections_refuse_a_cursor_after_the_page_boundary(self) -> None:
        for connection_name, reader in (
            ("reviews", subject._reviews),
            ("reviewThreads", subject._review_threads),
        ):
            with self.subTest(connection=connection_name):
                calls: list[tuple[str, ...]] = []

                def gh_json(*arguments: str) -> object:
                    calls.append(arguments)
                    page_number = 1 if not any(
                        argument.startswith("cursor=") for argument in arguments
                    ) else len(calls)
                    nodes: list[dict[str, object]] = (
                        [{"id": page_number}]
                        if connection_name == "reviews"
                        else [
                            {
                                "id": f"thread-{page_number}",
                                "isResolved": True,
                                "comments": {
                                    "nodes": [],
                                    "pageInfo": {
                                        "hasNextPage": False,
                                        "endCursor": None,
                                    },
                                },
                            }
                        ]
                    )
                    return rate_limited(
                        {
                            "data": {
                                "repository": {
                                    "pullRequest": {
                                        connection_name: {
                                            "nodes": nodes,
                                            "pageInfo": {
                                                "hasNextPage": True,
                                                "endCursor": f"cursor-{page_number}",
                                            },
                                        }
                                    }
                                }
                            }
                        },
                        remaining=subject._GRAPHQL_REVIEW_BUDGET - page_number,
                    )

                with patch.object(subject, "_gh_json", side_effect=gh_json):
                    with self.assertRaisesRegex(ValueError, "outer page limit"):
                        reader("owner/repo", 72)

                self.assertEqual(
                    len(calls), subject._MAX_REVIEW_CONNECTION_PAGES
                )

    def test_outer_review_connections_reject_a_raced_invalid_second_cursor(self) -> None:
        for connection_name, reader in (
            ("reviews", subject._reviews),
            ("reviewThreads", subject._review_threads),
        ):
            with self.subTest(connection=connection_name):
                def page(end_cursor: object, has_next_page: bool) -> dict[str, object]:
                    nodes: list[dict[str, object]] = (
                        [{"id": 1}]
                        if connection_name == "reviews"
                        else [
                            {
                                "id": "thread-1",
                                "isResolved": True,
                                "comments": {
                                    "nodes": [],
                                    "pageInfo": {
                                        "hasNextPage": False,
                                        "endCursor": None,
                                    },
                                },
                            }
                        ]
                    )
                    return rate_limited(
                        {
                            "data": {
                                "repository": {
                                    "pullRequest": {
                                        connection_name: {
                                            "nodes": nodes,
                                            "pageInfo": {
                                                "hasNextPage": has_next_page,
                                                "endCursor": end_cursor,
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    )

                with patch.object(
                    subject,
                    "_gh_json",
                    side_effect=[page("cursor-1", True), page(None, True)],
                ) as request:
                    with self.assertRaisesRegex(TypeError, "endCursor"):
                        reader("owner/repo", 72)
                self.assertEqual(request.call_count, 2)

    def test_reviews_fails_closed_when_initial_page_changes_at_race_fence(self) -> None:
        first = rate_limited({
            "data": {"repository": {"pullRequest": {"reviews": {
                "nodes": [],
                "pageInfo": {"hasNextPage": False, "endCursor": None},
            }}}}
        })
        changed = rate_limited(deepcopy(first))
        changed["data"]["repository"]["pullRequest"]["reviews"]["nodes"] = [
            {"id": "replacement"}
        ]  # type: ignore[index]
        with patch.object(subject, "_gh_json", side_effect=[first, changed]):
            with self.assertRaisesRegex(ValueError, "reviews changed"):
                subject._reviews("owner/repo", 72)

    def test_review_threads_fails_closed_when_initial_page_changes_at_race_fence(self) -> None:
        first = rate_limited({
            "data": {"repository": {"pullRequest": {"reviewThreads": {
                "nodes": [],
                "pageInfo": {"hasNextPage": False, "endCursor": None},
            }}}}
        })
        changed = rate_limited(deepcopy(first))
        changed["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"] = [
            {"id": "replacement"}
        ]  # type: ignore[index]
        with patch.object(subject, "_gh_json", side_effect=[first, changed]):
            with self.assertRaisesRegex(ValueError, "reviewThreads changed"):
                subject._review_threads("owner/repo", 72)

    def test_nested_comments_fail_closed_when_initial_page_changes_at_race_fence(self) -> None:
        first = rate_limited({
            "data": {"node": {"__typename": "PullRequestReviewThread", "comments": {
                "nodes": [{"author": {"login": "reviewer"}}],
                "pageInfo": {"hasNextPage": False, "endCursor": None},
            }}}
        })
        changed = rate_limited(deepcopy(first))
        changed["data"]["node"]["comments"]["nodes"] = []  # type: ignore[index]
        with patch.object(subject, "_gh_json", side_effect=[first, changed]):
            with self.assertRaisesRegex(ValueError, "comments changed"):
                subject._review_thread_comments("thread-1", "initial-cursor")

    def test_rest_pages_fail_closed_when_initial_page_changes_at_race_fence(self) -> None:
        first = [{"id": 1}]
        changed = [{"id": 2}]
        with patch.object(subject, "_gh_json", side_effect=[first, changed]):
            with self.assertRaisesRegex(ValueError, "REST page changed"):
                subject._paginated_api_array("repos/owner/repo/issues/72/comments")

    def test_review_graphql_page_cap_fits_the_all_writer_budget(self) -> None:
        review_pages = 2 * subject._MAX_REVIEW_CONNECTION_PAGES
        bounded_review_points = subject._MAX_ALL_WRITER_TARGETS * review_pages
        self.assertEqual(bounded_review_points, 3_000)
        nested_overflow_points = subject._MAX_ALL_WRITER_TARGETS * (
            review_pages + subject._MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES
        )
        self.assertEqual(nested_overflow_points, 4_500)
        self.assertGreater(
            nested_overflow_points,
            subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR,
        )
        self.assertLessEqual(
            bounded_review_points
            + subject._GRAPHQL_OTHER_WORK_RESERVE
            + subject._GRAPHQL_OPERATIONAL_HEADROOM,
            subject._GRAPHQL_REVIEW_BUDGET,
        )

    def test_graphql_server_budget_accumulates_nested_overflow_across_150_verifiers(self) -> None:
        """The real reader path stops before the shared server floor is spent."""

        server_remaining = subject._GRAPHQL_REVIEW_BUDGET
        server_requests = 0
        violations: list[tuple[int, int]] = []
        process_phase = {"reviews": 0, "threads": 0}

        def server(*arguments: str) -> object:
            nonlocal server_remaining, server_requests
            query = next(argument for argument in arguments if argument.startswith("query="))
            if "reviews(first: 100" in query:
                process_phase["reviews"] += 1
                page = process_phase["reviews"]
                connection = {
                    "nodes": [{"id": f"review-{page}"}],
                    "pageInfo": {
                        "hasNextPage": page < subject._MAX_REVIEW_CONNECTION_PAGES,
                        "endCursor": f"reviews-{page}",
                    },
                }
            elif "node(id: $threadId)" in query:
                connection = {
                    "nodes": [{"author": {"login": "reviewer"}}],
                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                }
                payload = rate_limited(
                    {"data": {"node": {
                        "__typename": "PullRequestReviewThread",
                        "comments": connection,
                    }}},
                    remaining=server_remaining - 1,
                )
                if server_remaining - 1 < subject._GRAPHQL_REMAINING_FLOOR:
                    violations.append((server_remaining, 1))
                server_remaining -= 1
                server_requests += 1
                return payload
            else:
                process_phase["threads"] += 1
                page = process_phase["threads"]
                connection = {
                    "nodes": [
                        {
                            "id": "thread-1",
                            "isResolved": True,
                            "comments": {
                                "nodes": [{"author": {"login": "reviewer"}}],
                                "pageInfo": {
                                    "hasNextPage": page == 1,
                                    "endCursor": "comments-1" if page == 1 else None,
                                },
                            },
                        }
                    ] if page == 1 else [],
                    "pageInfo": {
                        "hasNextPage": page < subject._MAX_REVIEW_CONNECTION_PAGES,
                        "endCursor": f"threads-{page}",
                    },
                }
            if server_remaining - 1 < subject._GRAPHQL_REMAINING_FLOOR:
                violations.append((server_remaining, 1))
            server_remaining -= 1
            server_requests += 1
            return rate_limited(
                {"data": {"repository": {"pullRequest": {
                    "reviews" if "reviews(first: 100" in query else "reviewThreads": connection
                }}}},
                remaining=server_remaining,
            )

        failed_process: int | None = None
        with patch.object(subject, "_gh_json", side_effect=server):
            for process in range(1, subject._MAX_ALL_WRITER_TARGETS + 1):
                process_phase["reviews"] = 0
                process_phase["threads"] = 0
                budget = subject._GraphQLBudget()
                try:
                    subject._reviews("owner/repo", process, budget=budget)
                    subject._review_threads("owner/repo", process, budget=budget)
                except ValueError:
                    failed_process = process
                    break

        self.assertIsNotNone(failed_process)
        self.assertLess(failed_process, subject._MAX_ALL_WRITER_TARGETS)
        self.assertGreaterEqual(server_remaining, subject._GRAPHQL_REMAINING_FLOOR)
        self.assertEqual(violations, [])
        self.assertLessEqual(
            subject._GRAPHQL_REVIEW_BUDGET - server_remaining,
            subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR,
        )
        self.assertLess(server_requests, subject._MAX_ALL_WRITER_TARGETS * 30)

    def test_paginated_rest_response_rejects_page_and_item_overflow(self) -> None:
        full_page = [{"id": number} for number in range(subject._MAX_REST_PAGE_ITEMS)]
        with patch.object(
            subject,
            "_gh_json",
            side_effect=[full_page for _ in range(subject._MAX_REST_PAGES)],
        ) as request:
            with self.assertRaisesRegex(ValueError, "page limit"):
                subject._paginated_api_array("repos/owner/repo/issues/72/comments")
        self.assertEqual(request.call_count, subject._MAX_REST_PAGES)

        with patch.object(
            subject,
            "_gh_json",
            return_value=[[] for _ in range(subject._MAX_REST_PAGES + 1)],
        ):
            with self.assertRaisesRegex(ValueError, "page limit"):
                subject._paginated_api_array("repos/owner/repo/issues/72/comments")

        oversized_page = [{"id": number} for number in range(subject._MAX_REST_PAGE_ITEMS + 1)]
        with patch.object(
            subject,
            "_gh_json",
            return_value=[oversized_page],
        ):
            with self.assertRaisesRegex(ValueError, "item limit"):
                subject._paginated_api_array("repos/owner/repo/issues/72/comments")
        with patch.object(
            subject,
            "_gh_json",
            return_value=[
                {"id": number} for number in range(subject._MAX_REST_ITEMS + 1)
            ],
        ):
            with self.assertRaisesRegex(ValueError, "item limit"):
                subject._paginated_api_array("repos/owner/repo/issues/72/comments")

    def test_paginated_rest_object_response_rejects_page_and_item_overflow(self) -> None:
        with self.assertRaisesRegex(ValueError, "page limit"):
            subject._bounded_rest_object_pages(
                [{} for _ in range(subject._MAX_REST_PAGES + 1)],
                label="check-runs",
                item_key="check_runs",
            )

    def test_production_rest_readers_do_not_use_eager_cli_pagination(self) -> None:
        source = (Path(__file__).parent / "verify_pr_ready.py").read_text(
            encoding="utf-8"
        )
        self.assertNotIn('"--paginate"', source)
        self.assertNotIn('"--slurp"', source)
        with self.assertRaisesRegex(ValueError, "item limit"):
            subject._bounded_rest_object_pages(
                [{"check_runs": [{} for _ in range(subject._MAX_REST_PAGE_ITEMS + 1)]}],
                label="check-runs",
                item_key="check_runs",
            )

    def test_graphql_rate_limit_metadata_is_required_and_strict(self) -> None:
        payload = {
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviews": {
                            "nodes": [],
                            "pageInfo": {
                                "hasNextPage": False,
                                "endCursor": None,
                            },
                        }
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "rateLimit"):
                subject._reviews("owner/repo", 72)

        for field, value, error_type in (
            ("cost", 0, TypeError),
            ("cost", subject._GRAPHQL_MAX_QUERY_COST + 1, ValueError),
            ("remaining", -1, TypeError),
            ("resetAt", "not-a-timestamp", TypeError),
        ):
            with self.subTest(field=field):
                invalid = rate_limited(deepcopy(payload), remaining=4_500)
                rate_limit = invalid["data"]["rateLimit"]
                assert isinstance(rate_limit, dict)
                rate_limit[field] = value
                with patch.object(subject, "_gh_json", return_value=invalid):
                    with self.assertRaisesRegex(error_type, "rateLimit"):
                        subject._reviews("owner/repo", 72)

        below_floor = rate_limited(deepcopy(payload), remaining=subject._GRAPHQL_REMAINING_FLOOR - 1)
        with patch.object(subject, "_gh_json", return_value=below_floor):
            with self.assertRaisesRegex(ValueError, "reserved floor"):
                subject._reviews("owner/repo", 72)

    def test_graphql_budget_uses_reported_cost_before_follow_up(self) -> None:
        budget = subject._GraphQLBudget()
        first = rate_limited({"data": {}}, cost=11, remaining=4_489)
        second = rate_limited({"data": {}}, cost=97, remaining=4_392)
        budget.observe(first)
        budget.before_request()
        budget.observe(second)
        self.assertEqual(budget.max_cost, 97)
        self.assertEqual(budget.remaining, 4_392)

        exhausted = rate_limited(
            {"data": {}},
            cost=1,
            remaining=subject._GRAPHQL_REMAINING_FLOOR,
        )
        budget.observe(exhausted)
        with self.assertRaisesRegex(ValueError, "next query"):
            budget.before_request()

    def test_graphql_budget_caps_a_high_server_remaining_to_the_local_lease(self) -> None:
        budget = subject._GraphQLBudget()
        budget.observe(rate_limited({"data": {}}, remaining=10_000))
        self.assertEqual(budget.remaining, subject._GRAPHQL_REVIEW_BUDGET)
        budget.observe(rate_limited({"data": {}}, remaining=9_999))
        self.assertEqual(budget.remaining, subject._GRAPHQL_REVIEW_BUDGET - 1)

        # server 側の大きな残量を、verifier の有限な local lease へ拡張しない。
        consumed = 0
        while True:
            try:
                budget.consume()
            except ValueError:
                break
            consumed += 1

        self.assertLessEqual(
            consumed,
            subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR,
        )
        assert budget.remaining is not None
        self.assertGreaterEqual(budget.remaining, subject._GRAPHQL_REMAINING_FLOOR)

    def test_shared_graphql_ledger_bounds_150_verifiers_despite_high_server_quota(self) -> None:
        """Separate verifier processes share one 4,500-point lease, not 150 leases."""
        maximum = subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR
        server_remaining = 10_000
        consumed = 0
        rejected = 0
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            for verifier in range(subject._MAX_ALL_WRITER_TARGETS):
                ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))
                for reread in range(30):
                    try:
                        reservation = ledger.reserve()
                    except ValueError:
                        rejected += 1
                        break
                    cost = 2 if (verifier + reread) % 2 else 1
                    ledger.settle(reservation, cost)
                    consumed += cost
                    server_remaining -= cost
                if rejected:
                    continue
        self.assertGreater(rejected, 0)
        self.assertLessEqual(consumed, maximum)
        self.assertGreaterEqual(server_remaining, 10_000 - maximum)

    def test_shared_graphql_ledger_caps_150_real_verifier_processes_at_3000(self) -> None:
        """A server quota of 10,000 cannot turn 150 verifier leases into 15,000 points."""

        maximum = subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR
        context = multiprocessing.get_context("fork")
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            start = context.Event()
            results = context.Queue()
            workers = [
                context.Process(
                    target=_shared_ledger_lease_worker,
                    args=(str(snapshot), start, results),
                )
                for _ in range(subject._MAX_ALL_WRITER_TARGETS)
            ]
            for worker in workers:
                worker.start()
            start.set()
            for worker in workers:
                worker.join(15)
                self.assertEqual(worker.exitcode, 0)
            outcomes: list[tuple[object, ...]] = []
            for _ in workers:
                try:
                    outcomes.append(results.get(timeout=5))
                except queue.Empty as error:
                    self.fail(f"shared ledger worker did not report: {error}")
            self.assertFalse(
                [outcome for outcome in outcomes if outcome[0] == "error"], outcomes
            )
            self.assertEqual(
                sum(outcome[0] == "reserved" for outcome in outcomes),
                maximum // subject._GRAPHQL_MAX_QUERY_COST,
            )
            self.assertEqual(
                sum(outcome[0] == "exhausted" for outcome in outcomes),
                subject._MAX_ALL_WRITER_TARGETS - maximum // subject._GRAPHQL_MAX_QUERY_COST,
            )
            ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))
            self.assertEqual(ledger._update(lambda state: state["spent"]), maximum)

    def test_shared_graphql_ledger_serializes_real_process_creation_and_reservations(self) -> None:
        """An empty O_EXCL inode is never observable as a malformed ledger by peers."""

        maximum = subject._GRAPHQL_REVIEW_BUDGET - subject._GRAPHQL_REMAINING_FLOOR
        context = multiprocessing.get_context("fork")
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            start = context.Event()
            results = context.Queue()
            workers = [
                context.Process(
                    target=_shared_ledger_worker,
                    args=(str(snapshot), start, results, 30),
                )
                for _ in range(12)
            ]
            for worker in workers:
                worker.start()
            start.set()
            for worker in workers:
                worker.join(10)
                self.assertEqual(worker.exitcode, 0)
            outcomes: list[tuple[object, ...]] = []
            for _ in workers:
                try:
                    outcomes.append(results.get(timeout=5))
                except queue.Empty as error:
                    self.fail(f"shared ledger worker did not report: {error}")
            self.assertTrue(all(outcome[0] == "ok" for outcome in outcomes), outcomes)
            self.assertLessEqual(sum(int(outcome[1]) for outcome in outcomes), maximum)

    def test_shared_graphql_ledger_waits_for_a_locked_empty_creator_inode(self) -> None:
        """A peer retries the creation window instead of rejecting the empty inode."""

        context = multiprocessing.get_context("fork")
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            initialized = context.Event()
            blocked = context.Event()
            release = context.Event()
            results = context.Queue()
            creator = context.Process(
                target=_paused_shared_ledger_creator,
                args=(str(snapshot), initialized, release, results),
            )
            creator.start()
            self.assertTrue(initialized.wait(5))
            peer = context.Process(
                target=_initializing_shared_ledger_peer,
                args=(str(snapshot), blocked, results),
            )
            peer.start()
            self.assertTrue(blocked.wait(5))
            release.set()
            for worker in (creator, peer):
                worker.join(5)
                self.assertEqual(worker.exitcode, 0)
            outcomes = [results.get(timeout=5), results.get(timeout=5)]
            self.assertCountEqual(outcomes, [("creator", "ok"), ("peer", "ok")])

    def test_shared_graphql_ledger_rejects_malformed_rollback_and_concurrent_updates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))
            reservation = ledger.reserve()
            with self.assertRaisesRegex(ValueError, "query cost"):
                ledger.settle(reservation, subject._GRAPHQL_MAX_QUERY_COST + 1)
            self.assertEqual(ledger._update(lambda state: state["spent"]), 100)

            context = multiprocessing.get_context("fork")
            acquired = context.Event()
            release = context.Event()
            holder = context.Process(
                target=_hold_shared_ledger_lock,
                args=(str(ledger.path), acquired, release),
            )
            holder.start()
            try:
                self.assertTrue(acquired.wait(5))
                with patch.object(subject._SharedGraphQLLedger, "_LOCK_WAIT_SECONDS", 0.02):
                    with self.assertRaisesRegex(ValueError, "lock wait elapsed"):
                        ledger.reserve()
            finally:
                release.set()
                holder.join(5)
                self.assertEqual(holder.exitcode, 0)

            ledger.path.write_text("{}", encoding="utf-8")
            os.chmod(ledger.path, 0o600)
            with self.assertRaisesRegex(ValueError, "malformed|schema"):
                subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))

    def test_shared_graphql_ledger_outwaits_a_healthy_lock_queue_beyond_two_seconds(self) -> None:
        """A 150-process verifier burst must not fail merely because fsync serialization is slow."""
        self.assertGreater(subject._SharedGraphQLLedger._LOCK_WAIT_SECONDS, 2.0)
        context = multiprocessing.get_context("fork")
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))
            acquired = context.Event()
            release = context.Event()
            blocked = context.Event()
            results = context.Queue()
            holder = context.Process(
                target=_hold_shared_ledger_lock,
                args=(str(ledger.path), acquired, release),
            )
            holder.start()
            try:
                self.assertTrue(acquired.wait(5))
                reserver = context.Process(
                    target=_contending_shared_ledger_reserver,
                    args=(str(snapshot), blocked, results),
                )
                reserver.start()
                self.assertTrue(blocked.wait(5))
                time.sleep(2.1)
                release.set()
                reserver.join(10)
                self.assertEqual(reserver.exitcode, 0)
                self.assertEqual(results.get(timeout=5), ("reserved",))
            finally:
                release.set()
                holder.join(5)
                self.assertEqual(holder.exitcode, 0)

    def test_graphql_preflight_and_reread_settle_the_active_shared_ledger(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "open-pulls.json"
            snapshot.write_text("[]", encoding="utf-8")
            os.chmod(snapshot, 0o600)
            ledger = subject._SharedGraphQLLedger.from_open_pull_snapshot(str(snapshot))
            previous_budget = subject._ACTIVE_GRAPHQL_BUDGET
            previous_ledger = subject._ACTIVE_SHARED_GRAPHQL_LEDGER
            subject._ACTIVE_GRAPHQL_BUDGET = None
            subject._ACTIVE_SHARED_GRAPHQL_LEDGER = ledger

            def response(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                if arguments[1:3] == ["api", "graphql"]:
                    payload = rate_limited({"data": {}}, remaining=10_000)
                    return subprocess.CompletedProcess(arguments, 0, json.dumps(payload), "")
                return subprocess.CompletedProcess(arguments, 0, "{}", "")

            try:
                with patch.object(subject.subprocess, "run", side_effect=response):
                    subject._gh_json("pr", "view", "72", "--json", "reviews")
                self.assertEqual(ledger._update(lambda state: state["spent"]), 2)
            finally:
                subject._ACTIVE_GRAPHQL_BUDGET = previous_budget
                subject._ACTIVE_SHARED_GRAPHQL_LEDGER = previous_ledger

    def test_rest_budget_accumulates_primary_cost_across_150_verifiers(self) -> None:
        server_remaining = subject._REST_BUDGET
        requests = 0
        for _process in range(subject._MAX_ALL_WRITER_TARGETS):
            budget = subject._RestBudget()
            budget.observe_rate_limit(rest_rate_limited(remaining=server_remaining))
            for _request in range(20):
                try:
                    budget.consume()
                except ValueError:
                    break
                server_remaining -= 1
                requests += 1
            if server_remaining <= subject._REST_REMAINING_FLOOR:
                break

        self.assertEqual(requests, subject._REST_BUDGET - subject._REST_REMAINING_FLOOR)
        self.assertEqual(server_remaining, subject._REST_REMAINING_FLOOR)
        self.assertLessEqual(requests, subject._MAX_ALL_WRITER_TARGETS * 20)
        exhausted = subject._RestBudget()
        exhausted.observe_rate_limit(rest_rate_limited(remaining=server_remaining))
        with self.assertRaisesRegex(ValueError, "next request"):
            exhausted.before_request()

    def test_rest_budget_rejects_malformed_or_low_rate_limit(self) -> None:
        budget = subject._RestBudget()
        for payload in (
            {},
            {"resources": {}},
            {"resources": {"core": {"limit": 5_000, "remaining": 4_500}}},
            rest_rate_limited(remaining=-1),
            rest_rate_limited(limit=0),
            rest_rate_limited(reset=0),
        ):
            with self.subTest(payload=payload):
                with self.assertRaises(TypeError):
                    budget.observe_rate_limit(payload)

        budget.observe_rate_limit(
            rest_rate_limited(remaining=subject._REST_REMAINING_FLOOR)
        )
        with self.assertRaisesRegex(ValueError, "next request"):
            budget.before_request()

    def test_gh_json_bootstraps_and_charges_rest_budget_but_excludes_rate_limit(self) -> None:
        subject._ACTIVE_REST_BUDGET = None
        self.addCleanup(setattr, subject, "_ACTIVE_REST_BUDGET", None)
        responses = [
            subprocess.CompletedProcess(
                ["gh"], 0, json.dumps(rest_rate_limited()), ""
            ),
            subprocess.CompletedProcess(["gh"], 0, '{"ok":true}', ""),
        ]
        with patch.object(subject.subprocess, "run", side_effect=responses) as run:
            self.assertEqual(subject._gh_json("api", "repos/owner/repo/issues/72"), {"ok": True})

        self.assertEqual(run.call_count, 2)
        self.assertEqual(run.call_args_list[0].args[0], ["gh", "api", "rate_limit"])
        self.assertEqual(
            run.call_args_list[1].args[0],
            ["gh", "api", "repos/owner/repo/issues/72"],
        )
        assert subject._ACTIVE_REST_BUDGET is not None
        self.assertEqual(
            subject._ACTIVE_REST_BUDGET.remaining, subject._REST_BUDGET - 1
        )

    def test_gh_json_preflights_and_charges_graphql_backed_pr_view(self) -> None:
        subject._ACTIVE_GRAPHQL_BUDGET = None
        self.addCleanup(setattr, subject, "_ACTIVE_GRAPHQL_BUDGET", None)
        pull = {"baseRefOid": "c" * 40, "headRefOid": "a" * 40}
        responses = [
            subprocess.CompletedProcess(
                ["gh"],
                0,
                json.dumps(rate_limited({"data": {}}, remaining=10_000)),
                "",
            ),
            subprocess.CompletedProcess(["gh"], 0, json.dumps(pull), ""),
        ]
        with patch.object(subject.subprocess, "run", side_effect=responses) as run:
            self.assertEqual(
                subject._gh_json(
                    "pr",
                    "view",
                    "72",
                    "--repo",
                    "owner/repo",
                    "--json",
                    "baseRefOid,headRefOid",
                ),
                pull,
            )

        self.assertEqual(run.call_count, 2)
        preflight_args = run.call_args_list[0].args[0]
        self.assertEqual(preflight_args[:2], ["gh", "api"])
        self.assertIn("rateLimit", preflight_args[4])
        self.assertEqual(run.call_args_list[1].args[0][:3], ["gh", "pr", "view"])
        assert subject._ACTIVE_GRAPHQL_BUDGET is not None
        self.assertEqual(subject._ACTIVE_GRAPHQL_BUDGET.remaining, 4_499)

    def test_repo_view_is_also_preflighted_and_charged(self) -> None:
        subject._ACTIVE_GRAPHQL_BUDGET = None
        self.addCleanup(setattr, subject, "_ACTIVE_GRAPHQL_BUDGET", None)
        responses = [
            subprocess.CompletedProcess(
                ["gh"],
                0,
                json.dumps(rate_limited({"data": {}}, remaining=4_499)),
                "",
            ),
            subprocess.CompletedProcess(
                ["gh"], 0, '{"nameWithOwner":"owner/repo"}', ""
            ),
        ]
        with patch.object(subject.subprocess, "run", side_effect=responses) as run:
            self.assertEqual(subject._repository_name(None), "owner/repo")
        self.assertEqual(run.call_count, 2)
        self.assertEqual(run.call_args_list[0].args[0][2], "graphql")
        self.assertEqual(run.call_args_list[1].args[0][:3], ["gh", "repo", "view"])
        assert subject._ACTIVE_GRAPHQL_BUDGET is not None
        self.assertEqual(subject._ACTIVE_GRAPHQL_BUDGET.remaining, 4_498)

    def test_open_pull_requests_graphql_query_includes_rate_limit_contract(self) -> None:
        subject._ACTIVE_GRAPHQL_BUDGET = None
        self.addCleanup(setattr, subject, "_ACTIVE_GRAPHQL_BUDGET", None)
        open_pulls = {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "master"},
                    "pullRequests": {
                        "nodes": [],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        }
        responses = [
            subprocess.CompletedProcess(
                ["gh"],
                0,
                json.dumps(rate_limited({"data": {}}, remaining=4_499)),
                "",
            ),
            subprocess.CompletedProcess(
                ["gh"], 0, json.dumps(rate_limited(open_pulls, remaining=4_498)), ""
            ),
            subprocess.CompletedProcess(
                ["gh"], 0, json.dumps(rate_limited(open_pulls, remaining=4_497)), ""
            ),
        ]
        with patch.object(subject.subprocess, "run", side_effect=responses) as run:
            self.assertEqual(subject._open_pull_requests("owner/repo"), [])
        self.assertEqual(run.call_count, 3)
        query = run.call_args_list[1].args[0][4]
        self.assertIn("rateLimit { cost remaining resetAt }", query)

    def test_reads_author_reply_past_first_review_thread_comment_page(self) -> None:
        initial_page = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "thread-1",
                                    "isResolved": True,
                                    "comments": {
                                        "nodes": [
                                            {"author": {"login": "reviewer"}}
                                            for _ in range(100)
                                        ],
                                        "pageInfo": {
                                            "hasNextPage": True,
                                            "endCursor": "comments-cursor",
                                        },
                                    },
                                }
                            ],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        }
                    }
                }
            }
        })
        reply_page = rate_limited({
            "data": {
                "node": {
                    "__typename": "PullRequestReviewThread",
                    "comments": {
                        "nodes": [{"author": {"login": "HiroyukiFuruno"}}],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        })

        def gh_json(*arguments: str) -> object:
            query = next(argument for argument in arguments if argument.startswith("query="))
            if "node(id: $threadId)" in query:
                self.assertIn("threadId=thread-1", arguments)
                self.assertIn("commentsCursor=comments-cursor", arguments)
                return reply_page
            self.assertNotIn("commentsCursor=comments-cursor", arguments)
            return initial_page

        pull_request, _, comments = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        with patch.object(subject, "_gh_json", side_effect=gh_json):
            threads = subject._review_threads("owner/repo", 72)

        self.assertEqual(len(threads[0]["comments"]), 101)
        self.assertEqual(
            self.errors(pull_request=pull_request, threads=threads, comments=comments),
            [],
        )

    def test_review_thread_comment_cursor_is_distinct_from_threads_cursor(self) -> None:
        first_threads_page = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [],
                            "pageInfo": {
                                "hasNextPage": True,
                                "endCursor": "threads-cursor",
                            },
                        }
                    }
                }
            }
        })
        second_threads_page = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "thread-2",
                                    "isResolved": True,
                                    "comments": {
                                        "nodes": [],
                                        "pageInfo": {
                                            "hasNextPage": True,
                                            "endCursor": "comments-cursor",
                                        },
                                    },
                                }
                            ],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        }
                    }
                }
            }
        })
        comment_page = rate_limited({
            "data": {
                "node": {
                    "__typename": "PullRequestReviewThread",
                    "comments": {
                        "nodes": [{"author": {"login": "reviewer"}}],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    },
                }
            }
        })
        calls: list[tuple[str, ...]] = []

        def gh_json(*arguments: str) -> object:
            calls.append(arguments)
            query = next(argument for argument in arguments if argument.startswith("query="))
            if "node(id: $threadId)" in query:
                self.assertIn("threadId=thread-2", arguments)
                self.assertIn("commentsCursor=comments-cursor", arguments)
                self.assertNotIn("cursor=threads-cursor", arguments)
                return comment_page
            if "cursor=threads-cursor" in arguments:
                self.assertNotIn("commentsCursor=comments-cursor", arguments)
                return second_threads_page
            return first_threads_page

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            threads = subject._review_threads("owner/repo", 72)

        self.assertEqual(threads[0]["id"], "thread-2")
        self.assertEqual(len(threads[0]["comments"]), 1)
        self.assertEqual(len(calls), 5)

    def test_fails_closed_when_review_thread_comment_cursor_is_invalid(self) -> None:
        for name, page_info in (
            ("missing", {"hasNextPage": True}),
            ("empty", {"hasNextPage": True, "endCursor": ""}),
            ("wrong-type", {"hasNextPage": True, "endCursor": 1}),
            ("non-boolean", {"hasNextPage": None, "endCursor": None}),
        ):
            with self.subTest(name=name):
                payload = rate_limited({
                    "data": {
                        "node": {
                            "__typename": "PullRequestReviewThread",
                            "comments": {"nodes": [], "pageInfo": page_info},
                        }
                    }
                })
                with patch.object(subject, "_gh_json", return_value=payload):
                    with self.assertRaisesRegex(TypeError, "review thread comments"):
                        subject._review_thread_comments("thread-1", "initial-cursor")

    def test_fails_closed_when_review_thread_comment_cursor_repeats(self) -> None:
        payload = rate_limited({
            "data": {
                "node": {
                    "__typename": "PullRequestReviewThread",
                    "comments": {
                        "nodes": [],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    },
                }
            }
        })
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._review_thread_comments("thread-1", "again")

    def test_reads_all_review_thread_comment_pages_within_follow_up_limit(self) -> None:
        calls: list[tuple[str, ...]] = []

        def gh_json(*arguments: str) -> object:
            calls.append(arguments)
            page_number = 1 if "commentsCursor=initial-cursor" in arguments else len(calls)
            return rate_limited({
                "data": {
                    "node": {
                        "__typename": "PullRequestReviewThread",
                        "comments": {
                            "nodes": [{"page": page_number}],
                            "pageInfo": {
                                "hasNextPage": (
                                    page_number
                                    < subject._MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES
                                ),
                                "endCursor": f"cursor-{page_number}",
                            },
                        },
                    }
                }
            }, remaining=4_500 - len(calls))

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            comments = subject._review_thread_comments("thread-1", "initial-cursor")

        self.assertEqual(
            len(comments), subject._MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES
        )
        self.assertEqual(
            len(calls), subject._MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES + 1
        )

    def test_rejects_review_thread_comments_beyond_follow_up_limit(self) -> None:
        calls: list[tuple[str, ...]] = []

        def gh_json(*arguments: str) -> object:
            calls.append(arguments)
            page_number = len(calls)
            return rate_limited({
                "data": {
                    "node": {
                        "__typename": "PullRequestReviewThread",
                        "comments": {
                            "nodes": [{"page": page_number}],
                            "pageInfo": {
                                "hasNextPage": True,
                                "endCursor": f"cursor-{page_number}",
                            },
                        },
                    }
                }
            }, remaining=4_500 - page_number)

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(ValueError, "follow-up page limit"):
                subject._review_thread_comments("thread-1", "initial-cursor")

        self.assertEqual(
            len(calls), subject._MAX_REVIEW_THREAD_COMMENT_FOLLOW_UP_PAGES
        )

    def test_fails_closed_when_review_thread_comment_request_fails(self) -> None:
        initial_page = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "thread-1",
                                    "isResolved": True,
                                    "comments": {
                                        "nodes": [],
                                        "pageInfo": {
                                            "hasNextPage": True,
                                            "endCursor": "comments-cursor",
                                        },
                                    },
                                }
                            ],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        }
                    }
                }
            }
        })
        failure = subprocess.CalledProcessError(1, ["gh", "api", "graphql"])
        with patch.object(subject, "_gh_json", side_effect=[initial_page, failure]):
            with self.assertRaises(subprocess.CalledProcessError):
                subject._review_threads("owner/repo", 72)

    def test_fails_closed_when_review_thread_cursor_repeats(self) -> None:
        payload = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [],
                            "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                        }
                    }
                }
            }
        })
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._review_threads("owner/repo", 72)

    def test_fails_closed_when_review_cursor_repeats(self) -> None:
        payload = rate_limited({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviews": {
                            "nodes": [],
                            "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                        }
                    }
                }
            }
        })
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._reviews("owner/repo", 72)

    def test_rejects_boundary_changed_during_readiness_check(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})
        changed_pull_request = dict(pull_request)
        changed_pull_request["headRefOid"] = "d" * 40

        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_malformed_expected_snapshot_sha(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected base and head"):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "not-a-sha",
                    "--expected-head-sha",
                    HEAD,
                ]
            )

    def test_rejects_malformed_expected_head_snapshot_sha(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected base and head"):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "c" * 40,
                    "--expected-head-sha",
                    "not-a-sha",
                ]
            )

    def test_rejects_missing_expected_snapshot_sha(self) -> None:
        with self.assertRaises(SystemExit):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "c" * 40,
                ]
            )

    def test_rejects_initial_snapshot_different_from_expected(self) -> None:
        pull_request, _, _ = successful_state()
        pull_request["baseRefOid"] = "d" * 40
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=[]
        ), patch.object(subject, "_review_threads", return_value=[]
        ):
            with self.assertRaisesRegex(ValueError, "initial base/head"):
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                )

    def test_rejects_success_final_snapshot_different_from_expected(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["headRefOid"] = "d" * 40
        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed|snapshot"):
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                )

    def test_accepts_matching_start_and_final_snapshots(self) -> None:
        pull_request, threads, comments = successful_state()
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                ),
                0,
            )

    def test_rejects_trusted_marker_added_before_the_final_fence(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_comments = [
            *comments,
            marker(3, "final", HEAD),
        ]
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject,
            "_paginated_api_array",
            side_effect=[comments, changed_comments],
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "trusted review marker.*changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_ignores_untrusted_marker_added_before_the_final_fence(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_comments = [
            *comments,
            marker(
                3,
                "final",
                INITIAL_HEAD,
                login="external-user",
                author_association="NONE",
            ),
        ]
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject,
            "_paginated_api_array",
            side_effect=[comments, changed_comments],
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_rejects_issue_edit_between_identical_pr_boundaries(self) -> None:
        pull_request, threads, comments = successful_state()
        initial_issue = self.issue(64, "2026-08-29T03:00:00Z")
        edited_issue = self.issue(64, "2026-08-29T03:03:00Z")

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            side_effect=[(initial_issue,), (edited_issue,)],
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "canonical Issue snapshot changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_issue_body_change_with_the_same_updated_at(self) -> None:
        pull_request, threads, comments = successful_state()
        initial_issue = self.issue(64, "2026-08-29T03:00:00Z")
        changed_issue = subject.issue_contract.Issue(
            number=64,
            state="OPEN",
            body="Changed Issue body",
            url="https://github.com/owner/repo/issues/64",
            updated_at="2026-08-29T03:00:00Z",
        )

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            side_effect=[(initial_issue,), (changed_issue,)],
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "canonical Issue snapshot changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_pr_updated_at_change_after_an_aba_body_mutation(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["updatedAt"] = "2026-08-29T03:04:00Z"

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, changed_pull_request]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "updatedAt changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_base_branch_retarget_with_the_same_base_sha(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["baseRefName"] = "release/v0.4"

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, changed_pull_request]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base branch changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rechecks_ci_status_rollup_immediately_before_success(self) -> None:
        pull_request, threads, comments = successful_state()
        final_snapshot = deepcopy(pull_request)
        final_snapshot["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": None,
            }
        ]

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, final_snapshot]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "CI status changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_required_status_check_configuration_change_before_success(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_required_checks = (
            ("CI", "Lint", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
            (
                ("CI", None),
                ("Lint", 7),
                (subject._LATCH_CHECK, 15368),
                (subject._TRUSTED_CHECK, 42),
            ),
        )
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(
            subject,
            "_required_status_check_snapshot",
            side_effect=[self.required_checks, changed_required_checks],
        ):
            with self.assertRaisesRegex(ValueError, "required status checks changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_body_change_after_initial_closing_contract(self) -> None:
        pull_request, threads, comments = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["body"] = "Closes #65"

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, changed_pull_request]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "pull request body changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_new_open_closer_after_initial_contract(self) -> None:
        pull_request, threads, comments = successful_state()
        initial_open_pull_requests = current_canonical_closer()
        changed_open_pull_requests = initial_open_pull_requests + [
            {"number": 73, "isDraft": True, "body": "Fixes #64"}
        ]

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject,
            "_open_pull_requests",
            side_effect=[initial_open_pull_requests, changed_open_pull_requests],
        ):
            with self.assertRaisesRegex(ValueError, "open PR closer set changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_existing_readiness_error_is_not_masked_by_snapshot_refence(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request["isDraft"] = False
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged") as verify_snapshot:
            self.assertEqual(
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                ),
                1,
            )
            verify_snapshot.assert_not_called()

    def test_pr_ready_check_wires_one_snapshot_to_both_readiness_gates(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        check_start = justfile.index("pr-ready-check pr:")
        check_end = justfile.index("\n\n# ", check_start)
        check = justfile[check_start:check_end]
        self.assertIn("verify_push_issue.py --pr-number \"$pr\" --pr-base-sha \"$base_sha\" --pr-head-sha \"$head_sha\"", check)
        self.assertIn("baseRefOid,headRefOid,headRefName,baseRefName,isDraft", check)
        self.assertIn('pull.get("baseRefName")', check)
        self.assertIn('fields[3] == default["name"]', check)
        self.assertIn('"require-draft" if fields[6] else "allow-ready"', check)
        self.assertIn('gh repo view --json nameWithOwner', check)
        self.assertIn('gh api graphql -f query=', check)
        self.assertIn('target{__typename ... on Commit {oid}}', check)
        self.assertIn('target.get("__typename") == "Commit"', check)
        self.assertIn('--trusted-default-sha "$trusted_default_sha"', check)
        self.assertIn("verify_pr_ready.py --pr \"$pr\" --repository \"$repository\" \"--$readiness_mode\" --expected-base-sha \"$base_sha\" --expected-head-sha \"$head_sha\"", check)
        self.assertLess(check.index("verify_push_issue.py"), check.index("verify_pr_ready.py"))

    def test_pr_ready_metadata_parser_requires_default_branch_base(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        check_start = justfile.index("pr-ready-check pr:")
        check_end = justfile.index("\n\n# ", check_start)
        check = justfile[check_start:check_end]
        match = re.search(
            r"read -r base_sha head_sha branch base_branch parsed_repository "
            r"trusted_default_sha readiness_mode extra < <\(python3 -c '(.+?)' "
            r'\"\$pr_metadata\" \"\$repository\" \"\$default_metadata\"\)',
            check,
        )
        self.assertIsNotNone(match)
        assert match is not None
        parser = match.group(1)
        pull_request = {
            "baseRefOid": "c" * 40,
            "headRefOid": "a" * 40,
            "headRefName": "feature/pr-ready",
            "baseRefName": "master",
            "isDraft": True,
        }
        default_metadata = {
            "data": {
                "repository": {
                    "defaultBranchRef": {
                        "name": "master",
                        "target": {"__typename": "Commit", "oid": "d" * 40},
                    }
                }
            }
        }

        def run_parser(pull: dict[str, object], default: dict[str, object]) -> subprocess.CompletedProcess[str]:
            return subprocess.run(
                [sys.executable, "-c", parser, json.dumps(pull), "owner/repo", json.dumps(default)],
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertEqual(run_parser(pull_request, default_metadata).returncode, 0)
        for invalid_base in ("release/v1", None, "master\n"):
            with self.subTest(baseRefName=invalid_base):
                invalid_pull = dict(pull_request)
                invalid_pull["baseRefName"] = invalid_base
                result = run_parser(invalid_pull, default_metadata)
                self.assertNotEqual(result.returncode, 0)

        for invalid_default in (None, "release/v1", "master\n"):
            with self.subTest(defaultBranch=invalid_default):
                invalid_default_metadata = json.loads(json.dumps(default_metadata))
                invalid_default_metadata["data"]["repository"]["defaultBranchRef"]["name"] = invalid_default
                result = run_parser(pull_request, invalid_default_metadata)
                self.assertNotEqual(result.returncode, 0)

    def test_rejects_base_changed_with_head_unchanged_during_readiness_check(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})
        changed_pull_request = dict(pull_request)
        changed_pull_request["baseRefOid"] = "e" * 40

        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_fails_closed_when_final_boundary_response_is_not_an_object(self) -> None:
        with patch.object(subject, "_gh_json", return_value=[]):
            with self.assertRaisesRegex(TypeError, "boundary response"):
                subject._verify_pr_boundary_unchanged(
                    "owner/repo", 72, "c" * 40, "a" * 40
                )

    def test_accepts_unchanged_boundary_after_readiness_check(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(
                subject.main(["--pr", "72", "--repository", "owner/repo"]), 0
            )

    def test_does_not_fetch_final_boundary_when_readiness_has_errors(self) -> None:
        pull_request, threads, comments = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64", "isDraft": False})

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged") as verify_snapshot:
            self.assertEqual(
                subject.main(["--pr", "72", "--repository", "owner/repo"]), 1
            )
            verify_snapshot.assert_not_called()


class StrictGovernanceCheckRunTest(unittest.TestCase):
    """Contract fixtures for the non-Draft trusted Check Run gate."""

    repository = "owner/repo"
    pull_request = 72
    base = "c" * 40
    head = HEAD
    branch = "master"
    app_id = 42
    source_run_id = 901

    @property
    def body_sha256(self) -> str:
        return hashlib.sha256(BODY.encode("utf-8")).hexdigest()

    def _source(self, *, event: str = "pull_request_review") -> dict[str, object]:
        return {
            "id": self.source_run_id,
            "name": "PR governance review sensor",
            "path": ".github/workflows/pr-governance-review-events.yml@refs/pull/72/merge",
            "event": event,
            "run_number": 1,
            "run_attempt": 1,
            "head_sha": self.head,
            "status": "completed",
            "conclusion": "success",
            "repository": {"full_name": self.repository},
            "pull_requests": [
                {
                    "number": self.pull_request,
                    "base": {
                        "sha": self.base,
                        "ref": self.branch,
                        "repo": {"full_name": self.repository},
                    },
                    "head": {
                        "sha": self.head,
                        "repo": {"full_name": self.repository},
                    },
                }
            ],
        }

    def _source_with_rest_repository_identity(
        self, *, include_run_identity: bool = True
    ) -> dict[str, object]:
        source = self._source()
        repository = {
            "id": 101,
            "name": "repo",
            "url": "https://api.github.com/repos/owner/repo",
        }
        if include_run_identity:
            source["repository"] = dict(repository)
        pull = source["pull_requests"][0]  # type: ignore[index]
        pull["base"]["repo"] = dict(repository)  # type: ignore[index]
        pull["head"]["repo"] = dict(repository)  # type: ignore[index]
        return source

    def _run(self) -> dict[str, object]:
        return {
            "id": 101,
            "created_at": "2026-08-30T00:00:00Z",
            "name": subject._TRUSTED_CHECK,
            "head_sha": self.head,
            "app": {"id": self.app_id},
            "status": "completed",
            "conclusion": "success",
            "external_id": f"krr-governance/v1/{self.head}/writer-101",
            "details_url": (
                "https://github.com/owner/repo/actions/runs/123"
                f"?source_run_id={self.source_run_id}&pr_body_sha256={self.body_sha256}"
            ),
        }

    def _latch_run(self) -> dict[str, object]:
        return {
            "id": 201,
            "name": subject._LATCH_CHECK,
            "head_sha": self.head,
            "app": {"id": 15368},
            "status": "completed",
            "conclusion": "success",
            "details_url": (
                f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}"
            ),
        }

    def _protection(self, checks: list[dict[str, object]] | None = None) -> dict[str, object]:
        current_checks = (
            checks
            if checks is not None
            else [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ]
        )
        return {
            "strict": True,
            "contexts": [item["context"] for item in current_checks],
            "checks": current_checks,
        }

    def _gate(
        self,
        *,
        pages: list[dict[str, object]] | None = None,
        latch_pages: list[dict[str, object]] | None = None,
        protection: dict[str, object] | None = None,
        source: dict[str, object] | None = None,
        source_history: list[dict[str, object]] | None = None,
        source_history_pages: list[list[dict[str, object]]] | None = None,
        exclude_trusted_governance_check: bool = False,
    ) -> str | None:
        check_pages = pages if pages is not None else [{"check_runs": [self._run()]}]
        latch_check_pages = (
            latch_pages
            if latch_pages is not None
            else [{"check_runs": [self._latch_run()]}]
        )
        source_run = source if source is not None else self._source()
        history_runs = source_history if source_history is not None else [source_run]
        history_pages = (
            source_history_pages
            if source_history_pages is not None
            else [history_runs]
        )
        required = protection if protection is not None else self._protection()

        def gh_json(*arguments: str) -> object:
            endpoint = next(
                (
                    argument
                    for argument in arguments
                    if argument.startswith(f"repos/{self.repository}/")
                ),
                "",
            )
            if endpoint.endswith("/protection/required_status_checks"):
                return required
            if "/check-runs?" in endpoint:
                if "review" in endpoint and "latch" in endpoint:
                    return latch_check_pages
                if exclude_trusted_governance_check:
                    raise AssertionError("internal mode must not read the trusted Check Run")
                return check_pages
            if endpoint in {
                f"repos/{self.repository}/actions/runs/{self.source_run_id}",
                f"repos/{self.repository}/actions/runs/{source_run['id']}",
            }:
                return source_run
            if endpoint.startswith(
                f"repos/{self.repository}/actions/workflows/"
                "pr-governance-review-events.yml/runs?"
            ):
                query = parse_qs(urlparse(endpoint).query)
                requested_events = query.get("event", [])
                if query.get("head_sha") != [self.head]:
                    raise AssertionError("sensor history query must bind to the fixed head")
                return [
                    {
                        "workflow_runs": [
                            run
                            for run in page
                            if not requested_events or run.get("event") in requested_events
                        ]
                    }
                    for page in history_pages
                ]
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            return subject._governance_check_error(
                self.repository,
                self.pull_request,
                self.branch,
                self.base,
                self.head,
                self.body_sha256,
                exclude_trusted_governance_check=exclude_trusted_governance_check,
            )

    def test_rejects_trusted_check_with_stale_pr_body_digest(self) -> None:
        run = self._run()
        run["details_url"] = (
            "https://github.com/owner/repo/actions/runs/123"
            f"?source_run_id={self.source_run_id}&pr_body_sha256={'0' * 64}"
        )
        error = self._gate(pages=[{"check_runs": [run]}])
        self.assertEqual(
            error,
            "trusted Check Run details_url lacks exact current PR body digest evidence",
        )

    def test_accepts_current_newest_generation_with_stale_body_history(self) -> None:
        historical = self._run()
        historical.update(
            {
                "id": 100,
                "created_at": "2026-08-29T23:59:59Z",
                "external_id": f"krr-governance/v1/{self.head}/writer-100",
                "details_url": (
                    "https://github.com/owner/repo/actions/runs/123"
                    f"?source_run_id={self.source_run_id}&pr_body_sha256={'0' * 64}"
                ),
            }
        )
        self.assertIsNone(
            self._gate(pages=[{"check_runs": [historical, self._run()]}])
        )

    def test_accepts_authoritative_generation_when_history_lacks_its_source_evidence(self) -> None:
        historical = self._run()
        historical.update(
            {
                "id": 100,
                "created_at": "2026-08-29T23:59:59Z",
                "external_id": f"krr-governance/v1/{self.head}/writer-100",
                "details_url": None,
            }
        )
        self.assertIsNone(
            self._gate(pages=[{"check_runs": [historical, self._run()]}])
        )

    def test_rejects_trusted_check_without_pr_body_digest(self) -> None:
        run = self._run()
        run["details_url"] = (
            "https://github.com/owner/repo/actions/runs/123"
            f"?source_run_id={self.source_run_id}"
        )
        error = self._gate(pages=[{"check_runs": [run]}])
        self.assertEqual(
            error,
            "trusted Check Run details_url lacks exact current PR body digest evidence",
        )

    def test_rejects_trusted_check_with_duplicate_pr_body_digest(self) -> None:
        run = self._run()
        run["details_url"] = (
            "https://github.com/owner/repo/actions/runs/123"
            f"?source_run_id={self.source_run_id}&pr_body_sha256={self.body_sha256}"
            f"&pr_body_sha256={self.body_sha256}"
        )
        error = self._gate(pages=[{"check_runs": [run]}])
        self.assertEqual(
            error,
            "trusted Check Run details_url lacks exact current PR body digest evidence",
        )

    def _allow_ready(
        self, sources: list[dict[str, object]], *, exclude_trusted_governance_check: bool = False
    ) -> int:
        pull = {
            "isDraft": False,
            "baseRefOid": self.base,
            "headRefOid": self.head,
            "baseRefName": self.branch,
            "body": "Closes #64",
            "author": {"login": "HiroyukiFuruno"},
            "updatedAt": "2026-08-29T03:03:00Z",
            "statusCheckRollup": [],
            "reviews": [{}],
        }
        source_queue = list(sources)
        latest_source = source_queue[0]

        def gh_json(*arguments: str) -> object:
            nonlocal latest_source
            if arguments[:2] == ("pr", "view"):
                return pull
            endpoint = next(
                (
                    argument
                    for argument in arguments
                    if argument.startswith(f"repos/{self.repository}/")
                ),
                "",
            )
            if endpoint.endswith("/protection/required_status_checks"):
                return self._protection()
            if "/check-runs?" in endpoint:
                if "review" in endpoint and "latch" in endpoint:
                    return [{"check_runs": [self._latch_run()]}]
                if exclude_trusted_governance_check:
                    raise AssertionError("internal mode must not read the trusted Check Run")
                return [{"check_runs": [self._run()]}]
            if endpoint == f"repos/{self.repository}/actions/runs/{self.source_run_id}":
                if not source_queue:
                    raise AssertionError("unexpected extra source-run read")
                latest_source = source_queue.pop(0)
                return latest_source
            if endpoint.startswith(
                f"repos/{self.repository}/actions/workflows/"
                "pr-governance-review-events.yml/runs?"
            ):
                query = parse_qs(urlparse(endpoint).query)
                requested_events = query.get("event", [])
                if query.get("head_sha") != [self.head]:
                    raise AssertionError("sensor history query must bind to the fixed head")
                return [{
                    "workflow_runs": [
                        latest_source
                    ] if not requested_events or latest_source.get("event") in requested_events else []
                }]
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject, "_paginated_api_array", return_value=[]
        ), patch.object(subject, "_review_threads", return_value=[]
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
        ), patch.object(subject, "closing_reference_errors", return_value=[]), patch.object(
            subject, "readiness_errors", return_value=[]
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged"):
            return subject.main(
                [
                    "--pr",
                    str(self.pull_request),
                    "--repository",
                    self.repository,
                    "--allow-ready",
                    *(
                        ["--exclude-trusted-governance-check"]
                        if exclude_trusted_governance_check
                        else []
                    ),
                ]
            )

    def test_draft_gate_does_not_require_trusted_or_latch_check_runs(self) -> None:
        for state, governance_checks in (
            ("absent", []),
            (
                "pending",
                [
                    {
                        "__typename": "CheckRun",
                        "name": subject._TRUSTED_CHECK,
                        "status": "IN_PROGRESS",
                        "conclusion": None,
                    },
                    {
                        "__typename": "CheckRun",
                        "name": subject._LATCH_CHECK,
                        "status": "COMPLETED",
                        "conclusion": "FAILURE",
                    },
                ],
            ),
        ):
            with self.subTest(state=state):
                pull, threads, comments = successful_state()
                pull["statusCheckRollup"] = [
                    {
                        "__typename": "CheckRun",
                        "name": "CI",
                        "status": "COMPLETED",
                        "conclusion": "SUCCESS",
                    },
                    *governance_checks,
                ]
                with patch.object(subject, "_gh_json", return_value=pull), patch.object(
                    subject, "_paginated_api_array", return_value=comments
                ), patch.object(subject, "_review_threads", return_value=threads
                ), patch.object(
                    subject.issue_contract, "referenced_issue_snapshot", return_value=()
                ), patch.object(subject, "closing_reference_errors", return_value=[]), patch.object(
                    subject, "_verify_final_readiness_snapshot_unchanged"
                ), patch.object(subject, "_governance_check_error") as governance, patch.object(
                    subject,
                    "_required_status_check_snapshot",
                    return_value=(
                        ("CI", subject._LATCH_CHECK, subject._TRUSTED_CHECK),
                        (
                            ("CI", None),
                            (subject._LATCH_CHECK, 15368),
                            (subject._TRUSTED_CHECK, self.app_id),
                        ),
                    ),
                ):
                    self.assertEqual(
                        subject.main(["--pr", "72", "--repository", self.repository]), 0
                    )
                governance.assert_not_called()

    def test_allow_ready_accepts_exact_trusted_check_and_all_supported_sensor_events(self) -> None:
        for event in (
            "pull_request",
            "pull_request_review",
            "pull_request_review_comment",
        ):
            with self.subTest(event=event):
                self.assertIsNone(self._gate(source=self._source(event=event)))
        self.assertEqual(self._allow_ready([self._source(), self._source()]), 0)

    def test_internal_writer_mode_excludes_only_the_trusted_check_output(self) -> None:
        self.assertIsNone(self._gate(exclude_trusted_governance_check=True))
        self.assertEqual(
            self._allow_ready(
                [self._source(), self._source()],
                exclude_trusted_governance_check=True,
            ),
            0,
        )

    def test_internal_writer_mode_allows_only_active_current_sensor_and_latch(self) -> None:
        for status in subject._ACTIVE_SENSOR_OR_LATCH_STATUSES:
            with self.subTest(status=status):
                source = {**self._source(), "status": status, "conclusion": None}
                latch = {**self._latch_run(), "status": status, "conclusion": None}
                self.assertIsNone(
                    self._gate(
                        source=source,
                        source_history=[source],
                        latch_pages=[{"check_runs": [latch]}],
                        exclude_trusted_governance_check=True,
                    )
                )
        self.assertIsNotNone(
            self._gate(
                source={**self._source(), "status": "in_progress", "conclusion": None},
                source_history=[{**self._source(), "status": "in_progress", "conclusion": None}],
            )
        )

    def test_internal_writer_mode_rejects_invalid_sensor_or_latch_states(self) -> None:
        invalid_states = {
            "completed_failure": ("completed", "failure"),
            "unknown": ("unrecognized", None),
            "active_with_conclusion": ("in_progress", "success"),
        }
        for target in ("sensor", "latch"):
            for name, (status, conclusion) in invalid_states.items():
                with self.subTest(target=target, state=name):
                    source = (
                        {**self._source(), "status": status, "conclusion": conclusion}
                        if target == "sensor"
                        else self._source()
                    )
                    latch = (
                        {**self._latch_run(), "status": status, "conclusion": conclusion}
                        if target == "latch"
                        else self._latch_run()
                    )
                    self.assertIsNotNone(
                        self._gate(
                            source=source,
                            source_history=[source],
                            latch_pages=[{"check_runs": [latch]}],
                            exclude_trusted_governance_check=True,
                        )
                    )

    def test_internal_writer_mode_requires_allow_ready(self) -> None:
        with self.assertRaises(SystemExit):
            subject.main(
                [
                    "--pr",
                    str(self.pull_request),
                    "--repository",
                    self.repository,
                    "--exclude-trusted-governance-check",
                ]
            )

    def test_governance_check_rejects_missing_invalid_or_ambiguous_trusted_check_runs(self) -> None:
        invalid_runs: dict[str, list[dict[str, object]]] = {
            "absent": [],
            "pending": [{**self._run(), "status": "in_progress", "conclusion": None}],
            "failure": [{**self._run(), "conclusion": "failure"}],
            "duplicate": [self._run(), {**self._run(), "id": 102}],
            "foreign_app": [{**self._run(), "app": {"id": self.app_id + 1}}],
            "wrong_head": [{**self._run(), "head_sha": "b" * 40}],
            "wrong_external": [{**self._run(), "external_id": "krr-governance/v1/wrong"}],
            "duplicate_source_query": [
                {
                    **self._run(),
                    "details_url": "https://github.com/owner/repo/actions/runs/123?source_run_id=901&source_run_id=902",
                }
            ],
        }
        for name, runs in invalid_runs.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(pages=[{"check_runs": runs}]))

    def test_newest_immutable_generation_controls_the_trusted_gate(self) -> None:
        old = self._run()
        newest = {
            **old,
            "id": 102,
            "created_at": "2026-08-30T00:00:01Z",
            "external_id": f"krr-governance/v1/{self.head}/writer-102",
            "status": "in_progress",
            "conclusion": None,
        }
        # A previous success must not mask pending/failure in the generation
        # with the greatest immutable Check Run ID.
        self.assertIsNotNone(self._gate(pages=[{"check_runs": [old, newest]}]))
        newest_failure = {**newest, "status": "completed", "conclusion": "failure"}
        self.assertIsNotNone(self._gate(pages=[{"check_runs": [old, newest_failure]}]))

    def test_newest_immutable_generation_parses_fractional_timestamps(self) -> None:
        old_success = {**self._run(), "created_at": "2026-08-30T00:00:00Z"}
        newer_pending = {
            **old_success,
            "id": 102,
            "created_at": "2026-08-30T00:00:00.1Z",
            "external_id": f"krr-governance/v1/{self.head}/writer-102",
            "status": "in_progress",
            "conclusion": None,
        }
        # Lexicographic ordering puts the non-fractional `Z` after `.1Z`.
        # The parsed instant must instead select the pending generation.
        self.assertIsNotNone(self._gate(pages=[{"check_runs": [old_success, newer_pending]}]))

    def test_newest_immutable_generation_rejects_invalid_timestamp(self) -> None:
        invalid = {**self._run(), "created_at": "2026-02-30T00:00:00Z"}
        self.assertIsNotNone(self._gate(pages=[{"check_runs": [invalid]}]))

    def test_governance_check_requires_exact_branch_protection_app_bindings(self) -> None:
        variants = {
            "trusted_missing": [{"context": subject._LATCH_CHECK, "app_id": 15368}],
            "trusted_duplicate": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
            "trusted_unbound": [
                {"context": subject._TRUSTED_CHECK, "app_id": None},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
            "latch_wrong_app": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15369},
            ],
            "latch_duplicate": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
        }
        for name, checks in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(protection=self._protection(checks)))

    def test_allow_ready_binds_latch_to_the_trusted_source_run(self) -> None:
        invalid_latches: dict[str, list[dict[str, object]]] = {
            "absent": [],
            "pending": [{**self._latch_run(), "status": "in_progress", "conclusion": None}],
            "failure": [{**self._latch_run(), "conclusion": "failure"}],
            "foreign_app": [{**self._latch_run(), "app": {"id": 15369}}],
            "wrong_head": [{**self._latch_run(), "head_sha": "b" * 40}],
            "wrong_source": [
                {
                    **self._latch_run(),
                    "details_url": f"https://github.com/{self.repository}/actions/runs/902",
                }
            ],
            "duplicate_for_source": [self._latch_run(), {**self._latch_run(), "id": 202}],
        }
        for name, pages in invalid_latches.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(latch_pages=[{"check_runs": pages}]))
        older_latch = {
            **self._latch_run(),
            "id": 202,
            "details_url": f"https://github.com/{self.repository}/actions/runs/900",
        }
        self.assertIsNone(
            self._gate(latch_pages=[{"check_runs": [older_latch, self._latch_run()]}])
        )

    def test_latch_url_accepts_exact_run_and_job_urls_only(self) -> None:
        run_url = f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}"
        self.assertEqual(
            subject._latch_source_run_id(run_url, self.repository),
            str(self.source_run_id),
        )
        self.assertEqual(
            subject._latch_source_run_id(
                f"{run_url}/job/123", self.repository
            ),
            str(self.source_run_id),
        )
        for url in (
            f"https://github.com/other/repo/actions/runs/{self.source_run_id}",
            f"https://evil.example/{self.repository}/actions/runs/{self.source_run_id}",
            f"https://github.com@evil.example/{self.repository}/actions/runs/{self.source_run_id}",
            f"https://github.com:443/{self.repository}/actions/runs/{self.source_run_id}",
            f"{run_url}?next=1",
            f"{run_url}#fragment",
            f"{run_url}/job/0",
            f"{run_url}/extra",
        ):
            with self.subTest(url=url):
                self.assertIsNone(subject._latch_source_run_id(url, self.repository))

    def test_latch_url_uses_the_configured_github_server_exactly(self) -> None:
        url = f"https://ghe.example/{self.repository}/actions/runs/{self.source_run_id}"
        with patch.dict(subject.os.environ, {"GITHUB_SERVER_URL": "https://ghe.example"}):
            self.assertEqual(
                subject._latch_source_run_id(url, self.repository),
                str(self.source_run_id),
            )
            self.assertIsNone(
                subject._latch_source_run_id(
                    f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}",
                    self.repository,
                )
            )

    def test_latch_job_url_accepts_only_the_current_trusted_source(self) -> None:
        current = {
            **self._latch_run(),
            "details_url": (
                f"https://github.com/{self.repository}/actions/runs/"
                f"{self.source_run_id}/job/456"
            ),
        }
        old = {
            **self._latch_run(),
            "id": 202,
            "details_url": f"https://github.com/{self.repository}/actions/runs/900/job/456",
        }
        self.assertIsNone(self._gate(latch_pages=[{"check_runs": [old, current]}]))

    def test_internal_mode_binds_latch_to_the_unique_latest_sensor_generation(self) -> None:
        old = self._source()
        latest = self._source(event="pull_request_review_comment")
        latest.update({"id": 902, "run_number": 2})
        latch = {
            **self._latch_run(),
            "details_url": f"https://github.com/{self.repository}/actions/runs/902/job/456",
        }
        self.assertIsNone(
            self._gate(
                source=latest,
                source_history=[old, latest],
                latch_pages=[{"check_runs": [latch]}],
                exclude_trusted_governance_check=True,
            )
        )

    def test_internal_mode_rejects_ambiguous_latest_sensor_generation(self) -> None:
        latest_a = self._source(event="pull_request_review")
        latest_b = self._source(event="pull_request_review_comment")
        latest_a.update({"id": 902, "run_number": 2})
        latest_b.update({"id": 902, "run_number": 2})
        with self.assertRaisesRegex(
            TypeError, "sensor workflow run generation is duplicated"
        ):
            self._gate(
                source_history=[latest_a, latest_b],
                exclude_trusted_governance_check=True,
            )

    def test_sensor_history_fails_closed_on_a_truncated_page(self) -> None:
        with patch.object(
            subject,
            "_gh_json",
            return_value=[{"workflow_runs": [], "truncated": True}],
        ):
            with self.assertRaisesRegex(TypeError, "sensor workflow run page is truncated"):
                subject._latest_sensor_generation(
                    repository=self.repository,
                    pull_request=self.pull_request,
                    base_branch=self.branch,
                    base_sha=self.base,
                    head=self.head,
                )

    def test_sensor_history_fails_closed_on_a_malformed_page_marker(self) -> None:
        with patch.object(
            subject,
            "_gh_json",
            return_value=[{"workflow_runs": [], "truncated": "false"}],
        ):
            with self.assertRaisesRegex(TypeError, "sensor workflow run page is invalid"):
                subject._latest_sensor_generation(
                    repository=self.repository,
                    pull_request=self.pull_request,
                    base_branch=self.branch,
                    base_sha=self.base,
                    head=self.head,
                )

    def test_sensor_history_rejects_a_rerun_before_reusing_an_old_success(self) -> None:
        rerun = self._source()
        rerun["run_attempt"] = 2
        with self.assertRaisesRegex(TypeError, "sensor workflow run generation is invalid"):
            self._gate(source=self._source(), source_history=[rerun])

    def test_sensor_history_rejects_invalid_attempt_encodings(self) -> None:
        for attempt in (True, None, "2", 0):
            with self.subTest(attempt=attempt):
                candidate = self._source()
                candidate["run_attempt"] = attempt
                with self.assertRaisesRegex(
                    TypeError, "sensor workflow run generation is invalid"
                ):
                    self._gate(source_history=[candidate])

    def test_sensor_history_rejects_duplicate_fixed_boundary_generation(self) -> None:
        source = self._source()
        with self.assertRaisesRegex(
            TypeError, "sensor workflow run generation is duplicated"
        ):
            self._gate(source_history=[source, deepcopy(source)])

    def test_sensor_history_queries_are_bounded_to_fixed_head(self) -> None:
        endpoints: list[str] = []

        def gh_json(*arguments: str) -> object:
            endpoint = next(
                argument
                for argument in arguments
                if argument.startswith(f"repos/{self.repository}/")
            )
            endpoints.append(endpoint)
            return [{"workflow_runs": []}]

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertIsNone(
                subject._latest_sensor_generation(
                    repository=self.repository,
                    pull_request=self.pull_request,
                    base_branch=self.branch,
                    base_sha=self.base,
                    head=self.head,
                )
            )

        self.assertEqual(len(endpoints), 3)
        for event_name, endpoint in zip(
            (
                "pull_request",
                "pull_request_review",
                "pull_request_review_comment",
            ),
            endpoints,
        ):
            self.assertEqual(
                endpoint,
                f"repos/{self.repository}/actions/workflows/"
                f"pr-governance-review-events.yml/runs?event={event_name}"
                f"&head_sha={self.head}&per_page=100&page=1",
            )

    def test_governance_check_rejects_mismatched_source_run_identity(self) -> None:
        variants: dict[str, dict[str, object]] = {}
        for field, value in (
            ("id", self.source_run_id + 1),
            ("name", "other workflow"),
            ("path", ".github/workflows/other.yml@master"),
            ("event", "workflow_dispatch"),
            ("run_attempt", 2),
            ("status", "in_progress"),
            ("conclusion", "failure"),
            ("repository", {"full_name": "other/repo"}),
            ("head_sha", "b" * 40),
        ):
            source = self._source()
            source[field] = value
            variants[field] = source
        for name, source in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(source=source))

    def test_governance_check_rejects_mismatched_source_pr_binding(self) -> None:
        variants: dict[str, dict[str, object]] = {}
        for name, path, value in (
            ("number", ("number",), self.pull_request + 1),
            ("base_sha", ("base", "sha"), "d" * 40),
            ("base_ref", ("base", "ref"), "release"),
            ("base_repo", ("base", "repo", "full_name"), "other/repo"),
            ("head_sha", ("head", "sha"), "b" * 40),
            ("head_repo", ("head", "repo", "full_name"), "other/repo"),
        ):
            source = self._source()
            current: dict[str, object] = source["pull_requests"][0]  # type: ignore[index]
            for key in path[:-1]:
                current = current[key]  # type: ignore[assignment,index]
            current[path[-1]] = value
            variants[name] = source
        for name, source in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(source=source))

    def test_governance_check_accepts_rest_repository_identity_without_full_name(self) -> None:
        source = self._source_with_rest_repository_identity(include_run_identity=False)
        self.assertIsNone(self._gate(source=source, source_history=[source]))

    def test_governance_check_accepts_rest_identity_at_every_repository_boundary(self) -> None:
        source = self._source_with_rest_repository_identity()
        self.assertIsNone(self._gate(source=source, source_history=[source]))

    def test_governance_check_rejects_run_repository_id_drift(self) -> None:
        source = self._source_with_rest_repository_identity()
        source["repository"]["id"] = 202  # type: ignore[index]
        self.assertIsNotNone(self._gate(source=source, source_history=[source]))

    def test_governance_check_rejects_malformed_rest_repository_identity(self) -> None:
        for name, field, value in (
            ("id-bool", "id", True),
            ("id-string", "id", "101"),
            ("id-foreign", "id", 202),
            ("id-missing", "id", None),
            ("name-foreign", "name", "other-repository"),
            ("name-missing", "name", None),
            ("url-foreign", "url", "https://api.github.com/repos/other/repository"),
            ("url-missing", "url", None),
        ):
            with self.subTest(name=name):
                source = self._source_with_rest_repository_identity()
                pull = source["pull_requests"][0]  # type: ignore[index]
                pull["head"]["repo"][field] = value  # type: ignore[index]
                self.assertIsNotNone(self._gate(source=source, source_history=[source]))

    def test_governance_check_reads_a_matching_run_from_page_two(self) -> None:
        self.assertIsNone(
            self._gate(pages=[{"check_runs": []}, {"check_runs": [self._run()]}])
        )

    def test_governance_check_reads_a_matching_latch_from_page_two(self) -> None:
        self.assertIsNone(
            self._gate(latch_pages=[{"check_runs": []}, {"check_runs": [self._latch_run()]}])
        )

    def test_governance_check_rejects_a_trusted_source_that_is_not_latest(self) -> None:
        for name, status, conclusion in (
            ("requested", "queued", None),
            ("in_progress", "in_progress", None),
            ("completed", "completed", "success"),
        ):
            with self.subTest(name=name):
                newer = self._source(event="pull_request_review_comment")
                newer.update({
                    "id": self.source_run_id + 1,
                    "run_number": 2,
                    "status": status,
                    "conclusion": conclusion,
                })
                self.assertIsNotNone(
                    self._gate(source_history_pages=[[self._source()], [newer]])
                )

    def test_governance_check_rejects_a_non_success_latest_snapshot_for_the_same_run(self) -> None:
        stale = self._source()
        stale.update({"status": "completed", "conclusion": "failure"})
        self.assertIsNotNone(self._gate(source_history=[stale]))

    def test_allow_ready_rejects_source_base_change_between_initial_and_final_evidence(self) -> None:
        changed = deepcopy(self._source())
        changed["pull_requests"][0]["base"]["sha"] = "d" * 40  # type: ignore[index]
        with self.assertRaisesRegex(ValueError, "governance evidence changed"):
            self._allow_ready([self._source(), changed])


if __name__ == "__main__":
    unittest.main()
