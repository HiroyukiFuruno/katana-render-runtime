from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]
SPEC = importlib.util.spec_from_file_location(
    "pr_governance_status_writer", ROOT / "scripts/review/pr_governance_status_writer.py"
)
assert SPEC is not None and SPEC.loader is not None
WRITER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WRITER
SPEC.loader.exec_module(WRITER)


class StatusWriterUnitTest(unittest.TestCase):
    def setUp(self) -> None:
        WRITER._nonreconciling_dispatcher_generations.clear()
        self.addCleanup(WRITER._nonreconciling_dispatcher_generations.clear)
        self.dispatch_boundary = patch.dict(
            os.environ,
            {
                "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[]",
                "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[]", "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "0", "GOVERNANCE_CHECK_MANIFEST": "[]",
                "GOVERNANCE_TERMINAL_ORDER_NUMBERS": "[]", "GOVERNANCE_TERMINAL_BATCH_NUMBERS": "[]",
                "GOVERNANCE_CONTINUATION_INDEX": "0", "GOVERNANCE_COMPLETED_WRITER_RUN_IDS": "[]",
            },
        )
        self.dispatch_boundary.start()
        self.addCleanup(self.dispatch_boundary.stop)

    def identity(self):
        return patch.multiple(
            WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com", WRITER_RUN_ID="99"
        )

    @staticmethod
    def pull(number: int, body: object = "Fixes #64", *, draft: bool = False) -> dict[str, object]:
        return {
            "number": number, "state": "open", "draft": draft, "body": body,
            "base": {"sha": "b" * 40, "ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": "a" * 40, "ref": "governance", "repo": {"full_name": "owner/repository"}},
        }

    @staticmethod
    def generation(identifier: int = 900, attempt: int = 1, status: str = "completed", conclusion: object = "success") -> dict[str, object]:
        repository = {
            "id": 101,
            "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
        }
        return {
            "id": identifier, "run_number": 8, "run_attempt": attempt, "name": "CI",
            "path": ".github/workflows/test-and-build.yml", "event": "pull_request",
            "workflow_id": 44,
            "head_sha": "a" * 40, "status": status, "conclusion": conclusion,
            "repository": dict(repository),
            "pull_requests": [{"number": 72, "base": {"sha": "b" * 40, "ref": "master", "repo": dict(repository)}, "head": {"sha": "a" * 40, "repo": dict(repository)}}],
        }

    @staticmethod
    def rest_repository(identifier: int = 101) -> dict[str, object]:
        return {
            "id": identifier,
            "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
        }

    def with_rest_repository_identity(
        self, run: dict[str, object], *, run_identifier: int = 101,
        base_identifier: int = 101, head_identifier: int = 101,
    ) -> dict[str, object]:
        run["repository"] = self.rest_repository(run_identifier)
        pull_request = run["pull_requests"][0]  # type: ignore[index]
        pull_request["base"]["repo"] = self.rest_repository(base_identifier)  # type: ignore[index]
        pull_request["head"]["repo"] = self.rest_repository(head_identifier)  # type: ignore[index]
        return run

    @staticmethod
    def dispatcher_run(
        identifier: int = 88, *, event: str = "issues", status: str = "in_progress",
        conclusion: object = None, created_at: str = "2026-08-30T00:00:00Z",
    ) -> dict[str, object]:
        return {
            "id": identifier, "name": WRITER.DISPATCHER_NAME,
            "path": ".github/workflows/pr-governance.yml@master", "event": event,
            "head_sha": "d" * 40, "repository": {
                "id": 101, "name": "repository",
                "url": "https://api.github.com/repos/owner/repository",
            },
            "head_branch": "master", "workflow_id": 66, "run_number": 1, "run_attempt": 1, "status": status,
            "conclusion": conclusion, "created_at": created_at,
        }

    @staticmethod
    def dispatcher_page(*runs: dict[str, object], total_count: int | None = None) -> dict[str, object]:
        return {
            "total_count": len(runs) if total_count is None else total_count,
            "workflow_runs": list(runs),
        }

    @staticmethod
    def dispatcher_jobs(
        *,
        preflight_status: str = "completed", preflight_conclusion: object = "success",
        barrier_status: str = "completed", barrier_conclusion: object = "skipped",
        pull_request_target_noop_step: object = None,
        issue_noop_step: object = None,
        issue_noop_step_status: str = "completed",
        total_count: int | None = None,
    ) -> dict[str, object]:
        jobs = [
            {
                "id": 1, "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME,
                "status": preflight_status, "conclusion": preflight_conclusion,
                **(
                    {"steps": [{
                        "number": 1, "name": WRITER.PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME,
                        "status": "completed", "conclusion": pull_request_target_noop_step,
                    }]}
                    if pull_request_target_noop_step is not None else {}
                ),
                **(
                    {"steps": [{
                        "number": 1, "name": WRITER.PREFLIGHT_ISSUE_NOOP_STEP_NAME,
                        "status": issue_noop_step_status, "conclusion": issue_noop_step,
                    }]}
                    if issue_noop_step is not None else {}
                ),
            },
            {
                "id": 2, "name": WRITER.RESOLVER_FAILURE_BARRIER_NAME,
                "status": barrier_status, "conclusion": barrier_conclusion,
            },
        ]
        return {"total_count": len(jobs) if total_count is None else total_count, "jobs": jobs}

    @staticmethod
    def snapshot(numbers: tuple[int, ...], claimants: dict[str, frozenset[int]] | None = None, *, drafts: frozenset[int] = frozenset()) -> object:
        return WRITER.OpenSnapshot(
            numbers,
            {} if claimants is None else claimants,
            tuple({"number": number, "isDraft": number in drafts, "body": "Fixes #64", "head_sha": f"{number:040x}"[-40:]} for number in numbers),
        )

    def test_snapshot_file_cleans_ledger_after_normal_and_child_failure(self) -> None:
        for child_failure in (False, True):
            with self.subTest(child_failure=child_failure):
                ledger_path: Path | None = None
                if child_failure:
                    with self.assertRaises(RuntimeError):
                        with WRITER._snapshot_file() as source:
                            ledger_path = Path(source.name + ".krr-graphql-ledger-v1")
                            source.write("{}")
                            source.flush()
                            ledger_path.write_text(json.dumps({"snapshot_sha256": WRITER.hashlib.sha256(b"{}").hexdigest()}), encoding="utf-8")
                            ledger_path.chmod(0o600)
                            raise RuntimeError("child failure")
                else:
                    with WRITER._snapshot_file() as source:
                        ledger_path = Path(source.name + ".krr-graphql-ledger-v1")
                        source.write("{}")
                        source.flush()
                        ledger_path.write_text(json.dumps({"snapshot_sha256": WRITER.hashlib.sha256(b"{}").hexdigest()}), encoding="utf-8")
                        ledger_path.chmod(0o600)
                self.assertFalse(ledger_path.exists())
                self.assertIsNotNone(ledger_path)

    def test_snapshot_ledger_cleanup_rejects_malformed_or_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "snapshot.json"
            snapshot.write_text("{}", encoding="utf-8")
            snapshot.chmod(0o600)
            ledger = Path(str(snapshot) + ".krr-graphql-ledger-v1")
            ledger.write_text("{}", encoding="utf-8")
            ledger.chmod(0o600)
            with self.assertRaises(WRITER.GovernanceError):
                WRITER._cleanup_snapshot_ledger(str(snapshot))
            ledger.unlink()
            target = Path(directory) / "target"
            target.write_text("{}", encoding="utf-8")
            target.chmod(0o600)
            ledger.symlink_to(target)
            with self.assertRaises(WRITER.GovernanceError):
                WRITER._cleanup_snapshot_ledger(str(snapshot))
            self.assertTrue(ledger.is_symlink())

    def test_snapshot_ledger_cleanup_allows_absent_and_rejects_regular_path_swap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "snapshot.json"
            snapshot.write_text("{}", encoding="utf-8")
            snapshot.chmod(0o600)
            # No ledger is an ordinary post-child-failure state.
            WRITER._cleanup_snapshot_ledger(str(snapshot))
            ledger = Path(str(snapshot) + ".krr-graphql-ledger-v1")
            ledger.write_text(json.dumps({"snapshot_sha256": "bad"}), encoding="utf-8")
            ledger.chmod(0o600)
            replacement = Path(directory) / "replacement"
            replacement.write_text('{"snapshot_sha256":"bad"}', encoding="utf-8")
            replacement.chmod(0o600)
            ledger.unlink()
            replacement.rename(ledger)
            with self.assertRaises(WRITER.GovernanceError):
                WRITER._cleanup_snapshot_ledger(str(snapshot))

    def test_canonical_issue_requires_exactly_one_closer(self) -> None:
        self.assertEqual(WRITER.canonical_issue("Fixes #64"), "64")
        self.assertIsNone(WRITER.canonical_issue("Fixes #64; closes #65"))
        self.assertIsNone(WRITER.canonical_issue("No closer"))

    def test_canonical_issue_accepts_optional_colon_like_local_parser(self) -> None:
        with self.identity():
            self.assertEqual(WRITER.canonical_issue("Closes: #64"), "64")
            self.assertEqual(
                WRITER.canonical_issue(
                    "Fixes: https://github.com/owner/repository/issues/64"
                ),
                "64",
            )
            self.assertEqual(WRITER.canonical_issue("Resolves :\t#64"), "64")

    def test_canonical_issue_does_not_treat_colon_text_as_a_closing_reference(self) -> None:
        with self.identity():
            for body in (
                "encloses: #64",
                "Closes:: #64",
                "Closes #64x",
                "Closes: https://github.com/other/repository/issues/64",
            ):
                self.assertIsNone(WRITER.canonical_issue(body), body)

    def test_full_url_closer_must_target_the_current_repository(self) -> None:
        with self.identity():
            self.assertEqual(
                WRITER.canonical_issue("Fixes https://github.com/owner/repository/issues/64"),
                "64",
            )
            self.assertIsNone(
                WRITER.canonical_issue("Fixes https://github.com/other/repository/issues/64")
            )
            self.assertEqual(
                WRITER.canonical_issue(
                    "Fixes #64; fixes https://github.com/other/repository/issues/65"
                ),
                "64",
            )

    def test_closing_urls_share_the_push_contract_terminator(self) -> None:
        with self.identity():
            accepted = (
                "Closes: #64",
                "Fixes https://github.com/owner/repository/issues/64)",
                "Resolves : https://github.com/owner/repository/issues/64?source=pr",
            )
            for body in accepted:
                with self.subTest(body=body):
                    expected = {
                        str(number)
                        for number in WRITER.issue_contract.closing_issue_numbers(body, "owner/repository")
                    }
                    self.assertEqual(WRITER.closing_issues(body), expected)
                    self.assertEqual(WRITER.canonical_issue(body), "64")
            for body in (
                "Fixes https://github.com/owner/repository/issues/64/foo",
                "Fixes https://github.com/owner/repository/issues/64x",
            ):
                with self.subTest(body=body):
                    self.assertEqual(WRITER.closing_issues(body), set())
                    self.assertIsNone(WRITER.canonical_issue(body))

    def test_workflow_path_accepts_github_at_default_branch_not_arbitrary_suffix(self) -> None:
        expected = ".github/workflows/test-and-build.yml"
        self.assertTrue(WRITER.workflow_path_matches(expected, expected))
        self.assertTrue(WRITER.workflow_path_matches(expected + "@main", expected))
        self.assertTrue(WRITER.workflow_path_matches(expected + "@refs/heads/master", expected))
        for value in (expected + "@../main", expected + "@", expected + "@main//evil", expected + "@/main"):
            with self.subTest(value=value):
                self.assertFalse(WRITER.workflow_path_matches(value, expected))

    def test_sensor_blob_uses_default_base_and_head_api_routes(self) -> None:
        calls: list[str] = []
        def api(endpoint: str):
            calls.append(endpoint)
            return {"sha": "c" * 40}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", side_effect=api):
            WRITER.trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", "b" * 40, "a" * 40)
        self.assertEqual(calls, [
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "d" * 40,
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "b" * 40,
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "a" * 40,
        ])

    def test_sensor_blob_rejects_pr_modified_bytes(self) -> None:
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), \
             patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}, {"sha": "c" * 40}, {"sha": "d" * 40}]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", "b" * 40, "a" * 40)

    def test_blob_cache_reuses_default_and_base_bytes_across_pr_heads(self) -> None:
        cache: dict[tuple[str, str], str] = {}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", return_value={"sha": "c" * 40}) as api:
            WRITER.trusted_workflow_blob(".github/workflows/test-and-build.yml", "b" * 40, "a" * 40, cache)
            WRITER.trusted_workflow_blob(".github/workflows/test-and-build.yml", "b" * 40, "e" * 40, cache)
        self.assertEqual(api.call_count, 4)

    def test_malformed_sibling_is_a_canonical_issue_claimant(self) -> None:
        source = {
            "number": 72, "state": "open", "body": "Fixes #64",
            "base": {"sha": "b" * 40, "ref": "master", "repo": {"full_name": "owner/repository", "default_branch": "master"}},
            "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}},
        }
        sibling = {
            "number": 73, "state": "open", "body": "Fixes #64; closes #65",
            "base": {"sha": "b" * 40}, "head": {"sha": "c" * 40},
        }
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "b" * 40}), patch.object(WRITER, "pull", return_value=source):
            self.assertFalse(WRITER.final_closer_is_unique(
                72, "64", "b" * 40, "a" * 40, WRITER.pr_body_sha256("Fixes #64"),
                {"64": frozenset({72, 73})},
            ))

    def test_open_snapshot_rejects_two_governed_prs_with_the_same_head(self) -> None:
        shared = "a" * 40
        records = [
            self.pull(72),
            self.pull(73),
        ]
        for record in records:
            record["head"]["sha"] = shared  # type: ignore[index]
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "pages", return_value=[records]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_newer_check_fence_rejects_terminal_write(self) -> None:
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
                self.assertTrue(WRITER.check_changed_since("a" * 40, 101))

    def test_newer_dispatcher_generation_blocks_before_its_pending_check_exists(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(created_at="2026-08-30T00:00:00Z")
        # The newer run ID is intentionally lower.  The immutable creation
        # order, not numeric Actions IDs, decides which writer owns the head.
        newer = self.dispatcher_run(7, status="queued", created_at="2026-08-30T00:01:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(current, newer)):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_generation_order_uses_created_at_then_id_in_one_bounded_page(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        # Same timestamp uses the immutable ID only as the specified tie-break.
        newer_tie = self.dispatcher_run(89, status="queued", created_at="2026-08-30T00:00:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(current, newer_tie)):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_generation_does_not_use_a_larger_id_when_created_earlier(self) -> None:
        current = self.dispatcher_run(88, created_at="2026-08-30T00:01:00Z")
        older_with_larger_id = self.dispatcher_run(89, status="completed", conclusion="success", created_at="2026-08-30T00:00:00Z")
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}):
            current_generation = WRITER.dispatcher_generation(current, require_success=True)
            older_generation = WRITER.dispatcher_generation(older_with_larger_id)
        self.assertFalse(WRITER.dispatcher_generation_is_newer(older_generation, current_generation))

    def test_newer_active_dispatcher_generation_blocks_without_historical_check_scan(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        newer = self.dispatcher_run(7, status="in_progress", created_at="2026-08-30T00:01:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(current, newer)), \
             patch.object(WRITER, "checks") as checks:
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)
        checks.assert_not_called()

    def test_foreign_workflow_run_noop_does_not_preempt_a_local_writer(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        foreign_noop = self.dispatcher_run(
            7, event="workflow_run", status="completed", conclusion="success",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(
                 WRITER, "object_page",
                 side_effect=(
                     self.dispatcher_page(current, foreign_noop),
                     self.dispatcher_jobs(),
                 ),
             ) as page:
            WRITER.reject_newer_dispatcher_barrier(head)
        self.assertEqual(
            page.call_args_list[1].args[0],
            "repos/owner/repository/actions/runs/7/jobs?per_page=100",
        )

    def test_verified_pull_request_target_noop_does_not_preempt_a_local_writer(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        target_noop = self.dispatcher_run(
            7, event="pull_request_target", status="completed", conclusion="success",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(
                 WRITER, "object_page",
                 side_effect=(
                     self.dispatcher_page(current, target_noop),
                     self.dispatcher_jobs(pull_request_target_noop_step="success"),
                 ),
             ):
            WRITER.reject_newer_dispatcher_barrier(head)

        for label, jobs in (
            ("missing", self.dispatcher_jobs()),
            ("skipped", self.dispatcher_jobs(pull_request_target_noop_step="skipped")),
            ("failure", self.dispatcher_jobs(pull_request_target_noop_step="failure")),
            ("duplicate", {"total_count": 2, "jobs": [
                {"id": 1, "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME, "status": "completed", "conclusion": "success", "steps": [
                    {"number": 1, "name": WRITER.PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME, "status": "completed", "conclusion": "success"},
                    {"number": 2, "name": WRITER.PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME, "status": "completed", "conclusion": "success"},
                ]},
                {"id": 2, "name": WRITER.RESOLVER_FAILURE_BARRIER_NAME, "status": "completed", "conclusion": "skipped"},
            ]}),
            ("malformed", {"total_count": 2, "jobs": [
                {"id": 1, "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME, "status": "completed", "conclusion": "success", "steps": [{"number": True, "name": WRITER.PREFLIGHT_PULL_REQUEST_TARGET_NOOP_STEP_NAME, "status": "completed", "conclusion": "success"}]},
                {"id": 2, "name": WRITER.RESOLVER_FAILURE_BARRIER_NAME, "status": "completed", "conclusion": "skipped"},
            ]}),
        ):
            WRITER._nonreconciling_dispatcher_generations.clear()
            with self.subTest(evidence=label), self.identity(), patch.dict(os.environ, environment), \
                 patch.object(WRITER, "api_json", return_value=current), \
                 patch.object(WRITER, "object_page", side_effect=(self.dispatcher_page(current, target_noop), jobs)):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier(head)

    def test_verified_issue_preflight_noop_does_not_preempt_a_local_writer(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        for event in ("issues", "issue_comment"):
            WRITER._nonreconciling_dispatcher_generations.clear()
            candidate = self.dispatcher_run(
                7, event=event, status="completed", conclusion="success",
                created_at="2026-08-30T00:01:00Z",
            )
            with self.subTest(event=event), self.identity(), patch.dict(os.environ, environment), \
                 patch.object(WRITER, "api_json", return_value=current), \
                 patch.object(
                     WRITER, "object_page",
                     side_effect=(
                         self.dispatcher_page(current, candidate),
                         self.dispatcher_jobs(issue_noop_step="success"),
                     ),
                 ):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_issue_preflight_noop_requires_exact_success_step_evidence(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        candidate = self.dispatcher_run(
            7, event="issues", status="completed", conclusion="success",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        cases: list[tuple[str, dict[str, object]]] = [
            ("missing", self.dispatcher_jobs()),
            ("failure", self.dispatcher_jobs(issue_noop_step="failure")),
            ("pending", self.dispatcher_jobs(issue_noop_step="success", issue_noop_step_status="pending")),
            ("unknown", self.dispatcher_jobs(issue_noop_step="success", issue_noop_step_status="mystery")),
        ]
        duplicate = self.dispatcher_jobs(issue_noop_step="success")
        steps = duplicate["jobs"][0]["steps"]  # type: ignore[index]
        steps.append(dict(steps[0]))  # type: ignore[union-attr]
        cases.append(("duplicate", duplicate))
        malformed = self.dispatcher_jobs(issue_noop_step="success")
        malformed["jobs"][0]["steps"][0]["number"] = True  # type: ignore[index]
        cases.append(("malformed", malformed))
        for label, jobs in cases:
            WRITER._nonreconciling_dispatcher_generations.clear()
            with self.subTest(evidence=label), self.identity(), patch.dict(os.environ, environment), \
                 patch.object(WRITER, "api_json", return_value=current), \
                 patch.object(
                     WRITER, "object_page",
                     side_effect=(self.dispatcher_page(current, candidate), jobs),
                 ):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier(head)

    def test_verified_workflow_run_reconciliation_still_preempts_a_local_writer(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        newer = self.dispatcher_run(
            7, event="workflow_run", status="completed", conclusion="success",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(
                 WRITER, "object_page",
                 side_effect=(
                     self.dispatcher_page(current, newer),
                     self.dispatcher_jobs(barrier_conclusion="success"),
                 ),
             ):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_workflow_run_without_exact_noop_evidence_remains_a_fail_closed_fence(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        newer = self.dispatcher_run(
            7, event="workflow_run", status="in_progress",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        malformed_jobs = {"total_count": 1, "jobs": [{"id": 1, "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME, "status": "completed", "conclusion": "success"}]}
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(
                 WRITER, "object_page",
                 side_effect=(self.dispatcher_page(current, newer), malformed_jobs),
             ):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_fence_rejects_malformed_paginated_or_api_evidence(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run()
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        malformed = current | {"path": ".github/workflows/pr-governance.yml@evil"}
        cases = (
            ("malformed dispatcher identity", self.dispatcher_page(current, malformed)),
            ("current snapshot differs from direct source", self.dispatcher_page(current | {"created_at": "2026-08-30T00:00:01Z"})),
            ("truncated bounded page", self.dispatcher_page(current, total_count=2)),
            ("malformed response", {"total_count": 1, "workflow_runs": "not-a-list"}),
        )
        for label, page in cases:
            with self.subTest(label=label), self.identity(), patch.dict(os.environ, environment), \
                 patch.object(WRITER, "api_json", return_value=current), \
                 patch.object(WRITER, "object_page", return_value=page):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier(head)
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", side_effect=WRITER.GovernanceError("API failure")):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_fence_ignores_unrelated_workflows_but_blocks_ci_requested_race(self) -> None:
        head = "a" * 40
        current = self.dispatcher_run(88, event="workflow_run", created_at="2026-08-30T00:00:00Z")
        newer = self.dispatcher_run(7, event="workflow_run", status="queued", created_at="2026-08-30T00:01:00Z")
        unrelated = {"name": "CI", "id": False, "path": None}
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=current), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(unrelated, current, newer)):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_generation_accepts_only_the_canonical_status_and_conclusion_pairs(self) -> None:
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}):
            for status in WRITER.DISPATCHER_ACTIVE_STATUSES:
                with self.subTest(active_status=status):
                    self.assertEqual(
                        WRITER.dispatcher_generation(self.dispatcher_run(status=status), require_success=True).identifier,
                        88,
                    )
                    with self.assertRaises(WRITER.GovernanceError):
                        WRITER.dispatcher_generation(self.dispatcher_run(status=status, conclusion="success"))
            for conclusion in WRITER.DISPATCHER_TERMINAL_CONCLUSIONS:
                completed = self.dispatcher_run(status="completed", conclusion=conclusion)
                with self.subTest(terminal_conclusion=conclusion):
                    self.assertEqual(WRITER.dispatcher_generation(completed).identifier, 88)
                    if conclusion == "success":
                        self.assertEqual(WRITER.dispatcher_generation(completed, require_success=True).identifier, 88)
                    else:
                        with self.assertRaises(WRITER.GovernanceError):
                            WRITER.dispatcher_generation(completed, require_success=True)
            for invalid in (
                self.dispatcher_run(status="completed", conclusion=None),
                self.dispatcher_run(status="completed", conclusion="unknown"),
                self.dispatcher_run(status="unknown", conclusion=None),
            ):
                with self.subTest(invalid=invalid):
                    with self.assertRaises(WRITER.GovernanceError):
                        WRITER.dispatcher_generation(invalid)

    def test_active_and_failed_dispatchers_preempt_then_next_success_recovers(self) -> None:
        head = "a" * 40
        previous = self.dispatcher_run(88, status="completed", conclusion="success", created_at="2026-08-30T00:00:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        for status, conclusion in (
            ("waiting", None), ("completed", "failure"),
            ("completed", "cancelled"), ("completed", "startup_failure"),
        ):
            newer = self.dispatcher_run(7, status=status, conclusion=conclusion, created_at="2026-08-30T00:01:00Z")
            with self.subTest(status=status, conclusion=conclusion), self.identity(), patch.dict(os.environ, environment), \
                 patch.object(WRITER, "api_json", return_value=previous), \
                 patch.object(WRITER, "object_page", return_value=self.dispatcher_page(previous, newer)):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier(head)

        recovered = self.dispatcher_run(7, status="completed", conclusion="success", created_at="2026-08-30T00:01:00Z")
        recovered_environment = environment | {"GOVERNANCE_DISPATCHER_RUN_ID": "7"}
        with self.identity(), patch.dict(os.environ, recovered_environment), \
             patch.object(WRITER, "api_json", return_value=recovered), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(recovered)):
            WRITER.reject_newer_dispatcher_barrier(head)

    def test_dispatcher_fence_stops_at_one_full_page_and_400_terminal_heads_use_app_budget(self) -> None:
        head = "a" * 40
        source = self.dispatcher_run()
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", return_value=source), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(source, total_count=100)):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)

        transport = {"direct": 0, "page": 0}
        def direct(endpoint: str, *, default_token: bool = False) -> object:
            self.assertFalse(default_token)
            self.assertEqual(endpoint, "repos/owner/repository/actions/runs/88")
            transport["direct"] += 1
            return source
        def page(endpoint: str, *, default_token: bool = False) -> dict[str, object]:
            self.assertFalse(default_token)
            self.assertIn("actions/workflows/66/runs?", endpoint)
            self.assertIn("branch=master", endpoint)
            self.assertIn("created=%3E%3D2026-08-30T00%3A00%3A00Z", endpoint)
            self.assertIn("per_page=100", endpoint)
            transport["page"] += 1
            return self.dispatcher_page(source)
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", side_effect=direct), \
             patch.object(WRITER, "object_page", side_effect=page), \
             patch.object(WRITER, "checks") as checks:
            for _ in range(400):
                WRITER.reject_newer_dispatcher_barrier(head)
        self.assertEqual(transport, {"direct": 400, "page": 400})
        self.assertLess(sum(transport.values()), 5_000)
        checks.assert_not_called()

    def test_noop_workflow_run_generation_is_read_once_for_400_terminal_heads(self) -> None:
        current = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        foreign_noop = self.dispatcher_run(
            7, event="workflow_run", status="completed", conclusion="success",
            created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        reads = {"source": 0, "generations": 0, "jobs": 0}

        def direct(endpoint: str, *, default_token: bool = False) -> object:
            self.assertFalse(default_token)
            self.assertEqual(endpoint, "repos/owner/repository/actions/runs/88")
            reads["source"] += 1
            return current

        def page(endpoint: str, *, default_token: bool = False) -> dict[str, object]:
            self.assertFalse(default_token)
            if endpoint.startswith("repos/owner/repository/actions/workflows/66/runs?"):
                reads["generations"] += 1
                return self.dispatcher_page(current, foreign_noop)
            self.assertEqual(endpoint, "repos/owner/repository/actions/runs/7/jobs?per_page=100")
            reads["jobs"] += 1
            return self.dispatcher_jobs()

        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "api_json", side_effect=direct), \
             patch.object(WRITER, "object_page", side_effect=page):
            for sequence in range(400):
                WRITER.reject_newer_dispatcher_barrier(f"{sequence:040x}")
        self.assertEqual(reads["jobs"], 1)
        self.assertLessEqual(sum(reads.values()), 4_500)

    def test_noop_generation_cache_rejects_a_changed_identity(self) -> None:
        original = WRITER.DispatcherGeneration(
            7, WRITER.dispatcher_created_at("2026-08-30T00:01:00Z"), "workflow_run", 66,
            "completed", "success",
        )
        changed = WRITER.DispatcherGeneration(
            7, WRITER.dispatcher_created_at("2026-08-30T00:01:01Z"), "workflow_run", 66,
            "completed", "success",
        )
        WRITER._nonreconciling_dispatcher_generations[7] = original
        with self.assertRaises(WRITER.GovernanceError):
            WRITER.dispatcher_generation_reconciles(changed)

    def test_early_writer_refuses_new_pending_before_and_after_newer_barrier(self) -> None:
        head = "a" * 40
        current_dispatcher = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        newer_dispatcher = self.dispatcher_run(7, status="queued", created_at="2026-08-30T00:01:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "early",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
            "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER, "api_json", return_value=current_dispatcher), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(current_dispatcher, newer_dispatcher)), \
             patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.write_check(head, state="in_progress", description="pending", details_url="https://github.com/owner/repository/actions/runs/99")
        command.assert_not_called()

    def test_terminal_patch_refuses_newer_dispatcher_barrier_for_all_writer(self) -> None:
        head = "a" * 40
        prefix = f"krr-governance/v1/{head}/"
        existing = {
            "id": 202, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": prefix + "dispatcher-88", "updated_at": "now", "app": {"id": 42},
            "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/88",
        }
        current_dispatcher = self.dispatcher_run(88, created_at="2026-08-30T00:00:00Z")
        newer_dispatcher = self.dispatcher_run(7, status="queued", created_at="2026-08-30T00:01:00Z")
        environment = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
            "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "ensure_writer_run_is_active"), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER, "api_json", return_value=current_dispatcher), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(current_dispatcher, newer_dispatcher)), \
             patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.write_check(head, state="success", description="success", details_url=existing["details_url"], existing=existing)
        command.assert_not_called()

    def test_dispatcher_input_is_bound_to_one_default_branch_run(self) -> None:
        run = self.dispatcher_run()
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "api_json", return_value=run) as api:
            self.assertEqual(WRITER.trusted_dispatcher_source(88), WRITER.DispatcherSource(88, "issues", 1))
        self.assertTrue(api.call_args.kwargs["default_token"])
        for field, value in (("event", "push"), ("head_sha", "e" * 40), ("head_branch", "evil"), ("workflow_id", 0), ("path", ".github/workflows/pr-governance.yml.evil@master"), ("run_attempt", 2), ("run_attempt", 0), ("run_attempt", True), ("run_attempt", "1"), ("created_at", "2026-08-30T00:00:00+00:00")):
            invalid = {**run, field: value}
            with self.subTest(field=field), self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
                 patch.object(WRITER, "api_json", return_value=invalid):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.trusted_dispatcher_source(88)

    def test_dispatcher_run_accepts_the_live_plain_workflow_path(self) -> None:
        run = self.dispatcher_run() | {"path": ".github/workflows/pr-governance.yml"}
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "api_json", return_value=run):
            self.assertEqual(WRITER.trusted_dispatcher_source(88), WRITER.DispatcherSource(88, "issues", 1))

    def test_dispatcher_run_accepts_trusted_manual_recovery_dispatch(self) -> None:
        run = self.dispatcher_run(event="workflow_dispatch")
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "api_json", return_value=run):
            self.assertEqual(
                WRITER.trusted_dispatcher_source(88),
                WRITER.DispatcherSource(88, "workflow_dispatch", 1),
            )

    def test_dispatcher_run_rejects_manual_replay_from_another_workflow(self) -> None:
        run = self.dispatcher_run(event="workflow_dispatch") | {
            "name": "untrusted recovery workflow",
            "path": ".github/workflows/untrusted.yml@master",
        }
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "api_json", return_value=run):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.trusted_dispatcher_source(88)

    def test_dispatcher_fence_lists_only_the_exact_default_branch_workflow(self) -> None:
        run = self.dispatcher_run()
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(run)) as page:
            self.assertEqual(
                set(WRITER.dispatcher_generations(66, WRITER.dispatcher_created_at(run["created_at"]))), {88},
            )
        self.assertEqual(
            page.call_args.args[0],
            "repos/owner/repository/actions/workflows/66/runs?branch=master&created=%3E%3D2026-08-30T00%3A00%3A00Z&per_page=100",
        )
        self.assertFalse(page.call_args.kwargs["default_token"])
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(run | {"workflow_id": 67})):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.dispatcher_generations(66, WRITER.dispatcher_created_at(run["created_at"]))

    def test_writer_and_sensor_attempt_boundaries_reject_bool_zero_string_and_retry(self) -> None:
        head = "a" * 40
        writer_run = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": head,
            "repository": self.rest_repository(), "status": "in_progress", "run_attempt": 1,
        }
        with self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GITHUB_SHA": head}), \
             patch.object(WRITER, "api_json", return_value=writer_run) as api:
            WRITER.ensure_writer_run_is_active()
        self.assertFalse(api.call_args.kwargs.get("default_token", False))
        for attempt in (0, True, "1", 2):
            with self.subTest(writer_attempt=attempt), self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GITHUB_SHA": head}), \
                 patch.object(WRITER, "api_json", return_value=writer_run | {"run_attempt": attempt}):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.ensure_writer_run_is_active()

        sensor_run = self.generation(attempt=1) | {"name": "PR governance review sensor", "event": "pull_request_review", "head_sha": head, "path": ".github/workflows/pr-governance-review-events.yml@master"}
        with self.identity(), patch.object(WRITER, "trusted_workflow_blob"), patch.object(WRITER, "object_page", return_value={"total_count": 1, "workflow_runs": [sensor_run]}):
            self.assertEqual(WRITER.sensor(72, "b" * 40, head), 900)
        for attempt in (0, True, "1", 2):
            with self.subTest(sensor_attempt=attempt), self.identity(), patch.object(WRITER, "trusted_workflow_blob"), patch.object(WRITER, "object_page", return_value={"total_count": 1, "workflow_runs": [sensor_run | {"run_attempt": attempt}]}):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.sensor(72, "b" * 40, head)

    def test_repository_rest_identity_accepts_minimal_identity_and_rejects_mismatches(self) -> None:
        identity = self.rest_repository()
        with self.identity():
            self.assertTrue(WRITER.repository_boundary_matches(
                identity, (dict(identity), dict(identity)), "owner/repository",
            ))
            for label, run_identity, base_identity, head_identity in (
                ("full-name-only", {"full_name": "owner/repository"}, identity, identity),
                ("partial-rest", {"id": 101, "name": "repository"}, identity, identity),
                ("id-bool", {**identity, "id": True}, identity, identity),
                ("id-string", {**identity, "id": "101"}, identity, identity),
                ("id-mismatch", {**identity, "id": 202}, identity, identity),
                ("name-mismatch", {**identity, "name": "foreign"}, identity, identity),
                ("url-mismatch", {**identity, "url": "https://api.github.com/repos/foreign/repository"}, identity, identity),
                ("nested-id-mismatch", identity, {**identity, "id": 202}, identity),
                ("nested-name-mismatch", identity, {**identity, "name": "foreign"}, identity),
                ("nested-url-mismatch", identity, identity, {**identity, "url": "https://api.github.com/repos/foreign/repository"}),
                ("mixed-rest-and-legacy", identity, {"full_name": "owner/repository"}, identity),
            ):
                with self.subTest(label=label):
                    self.assertFalse(WRITER.repository_boundary_matches(
                        run_identity, (base_identity, head_identity), "owner/repository",
                    ))

    def test_dispatcher_and_writer_reject_full_name_only_repository_identity(self) -> None:
        writer = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": "d" * 40,
            "repository": self.rest_repository(), "run_attempt": 1,
            "status": "in_progress",
        }
        with self.identity(), patch.dict(os.environ, {
            "GITHUB_ACTIONS": "true", "GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40,
        }):
            WRITER.dispatcher_generation(self.dispatcher_run())
            with patch.object(WRITER, "api_json", return_value=writer):
                WRITER.ensure_writer_run_is_active()
            for subject in (self.dispatcher_run(), writer):
                subject["repository"] = {"full_name": "owner/repository"}
                with self.subTest(subject=subject["name"]):
                    if subject["name"] == WRITER.DISPATCHER_NAME:
                        with self.assertRaises(WRITER.GovernanceError):
                            WRITER.dispatcher_generation(subject)
                    else:
                        with patch.object(WRITER, "api_json", return_value=subject):
                            with self.assertRaises(WRITER.NoPostGovernanceError):
                                WRITER.ensure_writer_run_is_active()

    def test_observed_invalidations_returns_only_exact_current_carry_markers(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        with self.identity():
            carry_check = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 1)}
            fresh = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        terminal = {"status": "completed", "conclusion": "success", "details_url": WRITER.dispatcher_invalidation_url(source, 1)}
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[carry_check, fresh]):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", ())
            self.assertEqual(scoped.numbers, (72, 73))
            self.assertEqual(carry, frozenset({72}))
        with self.identity(), patch.object(WRITER, "check_run", return_value=terminal):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())
        draft_snapshot = WRITER.OpenSnapshot(
            (72,), {}, ({"number": 72, "isDraft": True, "head_sha": "a" * 40},)
        )
        with self.identity(), patch.object(WRITER, "check_run", return_value=carry_check):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(draft_snapshot, source, "all", ())
        # A writer-owned pending URL is not a dispatcher carry marker.
        for invalid in (None, fresh | {"details_url": "https://github.com/owner/repository/actions/runs/99"}):
            with self.subTest(invalid=invalid), self.identity(), patch.object(WRITER, "check_run", return_value=invalid):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.observed_invalidations(snapshot, source, "all", ())

    def test_early_scope_requires_the_exact_event_boundary_and_all_scope_accepts_ordered_priority(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "workflow_run", 1)
        with self.identity():
            marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        stale = {"status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/7"}
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "early", (72,))
        self.assertEqual(scoped.numbers, (72,))
        self.assertEqual(carry, frozenset())
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[marker, stale]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", (73, 72))
        self.assertEqual(scoped.numbers, (72, 73))
        self.assertEqual(carry, frozenset())
        self.assertEqual(WRITER.governance_order(scoped, carry, (73, 72)), (73, 72))
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, WRITER.DispatcherSource(88, "schedule", 1), "early", (72,))
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(
                    snapshot, WRITER.DispatcherSource(88, "workflow_dispatch", 1), "early", (72,)
                )
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", (72, 72))

    def test_all_scope_preserves_only_the_bound_early_success_and_skips_its_rewrite(self) -> None:
        head, base = "a" * 40, "b" * 40
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "body": "Fixes #64", "head_sha": head}, {"number": 73, "isDraft": False, "body": "Fixes #64", "head_sha": "c" * 40}),
        )
        source = WRITER.DispatcherSource(88, "workflow_run", 1)
        query = {
            "source_run_id": "1", "ci_workflow_id": "2", "ci_run_id": "3", "ci_run_number": "4", "ci_run_attempt": "1",
            "ci_status": "completed", "ci_conclusion": "success", "release_workflow_id": "5", "release_run_id": "6",
            "release_run_number": "7", "release_run_attempt": "1", "release_status": "completed", "release_conclusion": "success",
            "pr_base_sha": base, "pr_head_sha": head,
            "pr_body_sha256": WRITER.pr_body_sha256("Fixes #64"),
        }
        early_success = {
            "id": 711, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": f"krr-governance/v1/{head}/writer-71", "updated_at": "now", "app": {"id": 42},
            "status": "completed", "conclusion": "success",
            "details_url": "https://github.com/owner/repository/actions/runs/71?" + WRITER.urlencode(query),
        }
        with self.identity():
            marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        # The preserved source is located via its exact writer-71 external
        # generation, not the current all-writer dispatcher generation.
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "object_pages", return_value=[{"check_runs": [early_success]}]), \
             patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 71)
        self.assertEqual(scoped.numbers, (73,))
        self.assertEqual(carry, frozenset())
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "object_pages", return_value=[{"check_runs": [early_success]}]), \
             patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 72)
        for digest in (None, "d" * 64):
            stale_query = dict(query)
            if digest is None:
                del stale_query["pr_body_sha256"]
            else:
                stale_query["pr_body_sha256"] = digest
            stale = {
                **early_success,
                "details_url": "https://github.com/owner/repository/actions/runs/71?" + WRITER.urlencode(stale_query),
            }
            with self.subTest(digest=digest), self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
                 patch.object(WRITER, "object_pages", return_value=[{"check_runs": [stale]}]), \
                 patch.object(WRITER, "check_run", return_value=marker):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 71)

    def test_all_scope_fails_closed_when_a_new_open_pr_missed_the_all_open_invalidation(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[marker, None]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())

    def test_main_rejects_noncanonical_or_invalid_dispatch_target_inputs_before_api_reads(self) -> None:
        for scope, targets in (("all", "[ ]"), ("all", "[72,72]"), ("early", "[true]"), ("invalid", "[]")):
            with self.subTest(scope=scope, targets=targets), self.identity(), \
                 patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": scope, "GOVERNANCE_TARGET_NUMBERS": targets}), \
                 patch.object(WRITER, "trusted_dispatcher_source") as source:
                self.assertEqual(WRITER.main(), 1)
                source.assert_not_called()

    def test_early_scope_rejects_more_than_forty_targets_before_api_reads(self) -> None:
        targets = json.dumps(list(range(1, 42)), separators=(",", ":"))
        with self.identity(), patch.dict(os.environ, {
            "GOVERNANCE_SCOPE": "early", "GOVERNANCE_TARGET_NUMBERS": targets,
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": "[]", "GOVERNANCE_CONTINUATION_INDEX": "0",
        }), patch.object(WRITER, "trusted_dispatcher_source") as source:
            self.assertEqual(WRITER.main(), 1)
        source.assert_not_called()

    def test_all_scope_requires_the_exact_canonical_continuation_slice(self) -> None:
        numbers = tuple(range(1, 302))
        snapshot = self.snapshot(numbers)
        manifest = json.dumps([[number, 10_000 + number] for number in numbers], separators=(",", ":"))
        expected = tuple(range(151, 301))
        base = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[1]",
            "GOVERNANCE_CHECK_MANIFEST": manifest, "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(expected), separators=(",", ":")),
            "GOVERNANCE_CONTINUATION_INDEX": "2",
        }
        for batch in (
            expected[:-1], (150,) + expected[:-1], tuple(reversed(expected)),
        ):
            with self.subTest(batch=batch[:2]), self.identity(), patch.dict(os.environ, base | {"GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(batch), separators=(",", ":"))}), \
                 patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
                 patch.object(WRITER, "open_snapshot", return_value=snapshot), \
                 patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
                 patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
                 patch.object(WRITER, "process") as process:
                self.assertEqual(WRITER.main(), 1)
            process.assert_not_called()

    def test_four_all_segments_revalidate_and_terminalize_six_hundred_non_drafts(self) -> None:
        """Run four production all-writer processes through only a fake transport."""
        preserved = tuple(range(1, 41))
        terminal_numbers = tuple(range(41, 601))
        all_numbers = (*preserved, *terminal_numbers)
        default_head = "d" * 40
        heads = {number: f"{number:040x}"[-40:] for number in all_numbers}
        bodies = {number: f"Fixes #{number}" for number in all_numbers}
        pulls: dict[int, dict[str, object]] = {}
        checks: dict[int, dict[str, object]] = {}
        check_number_by_id: dict[int, int] = {}
        sensor_runs: list[dict[str, object]] = []
        ci_runs: list[dict[str, object]] = []
        release_runs: list[dict[str, object]] = []
        head_runs: dict[str, dict[str, object]] = {}
        source = self.dispatcher_run(88, event="issues", status="in_progress")
        writer_runs: dict[int, dict[str, object]] = {}
        clock = [0.0]
        current_segment = [0]
        transport = {
            index: {
                "app": [], "default": [], "write": [], "graphql": [], "rebind": [],
                "manifest": [], "dispatcher_source": [], "prior_writer": [], "registration": [], "await": [],
            }
            for index in range(1, 6)
        }
        source_reads = {index: 0 for index in range(1, 6)}
        snapshots = {index: 0 for index in range(1, 6)}
        check_read_count = {index: 0 for index in range(1, 6)}
        sleep_calls = {index: [] for index in range(1, 6)}
        terminal_patches: list[int] = []
        check_reads: dict[int, int] = {}
        drift_check_id = [0]

        def writer_run(identifier: int, segment: int, status: str = "in_progress") -> dict[str, object]:
            return {
                "id": identifier, "name": "PR governance status writer",
                "path": ".github/workflows/pr-governance-status-writer.yml@master",
                "event": "workflow_dispatch", "display_title": f"source=88 scope=all segment={segment}",
                "head_sha": default_head, "head_branch": "master",
                "repository": self.rest_repository(), "run_attempt": 1,
                "status": status, "conclusion": "success" if status == "completed" else None,
                "actor": {"login": "krr-governance[bot]", "type": "Bot"},
                "triggering_actor": {"login": "krr-governance[bot]", "type": "Bot"},
            }

        def page_chunks(values: list[dict[str, object]]) -> list[dict[str, object]]:
            return [
                {"workflow_runs": values[index:index + 100]}
                for index in range(0, len(values), 100)
            ] or [{"workflow_runs": []}]

        def record(kind: str, count: int = 1) -> None:
            transport[current_segment[0]][kind].extend([clock[0]] * count)

        def response(arguments: list[str], payload: object) -> subprocess.CompletedProcess[str]:
            body = json.dumps(payload)
            if "--include" in arguments:
                body = f"HTTP/2 200 OK\n\n{body}"
            return subprocess.CompletedProcess(arguments, 0, body, "")

        def sleep(delay: float) -> None:
            self.assertEqual(delay, WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS)
            sleep_calls[current_segment[0]].append(delay)
            clock[0] += delay

        def monotonic() -> float:
            return clock[0]

        def workflow_overhead(kind: str, count: int, start: float, duration: float) -> None:
            self.assertIn(kind, {"registration", "await", "bootstrap_default"})
            self.assertGreater(count, 0)
            self.assertGreater(duration, 0.0)
            for item in range(count):
                timestamp = start + duration * item / count
                if kind == "bootstrap_default":
                    transport[current_segment[0]]["default"].append(timestamp)
                else:
                    transport[current_segment[0]][kind].append(timestamp)

        with self.identity():
            invalidation = WRITER.dispatcher_invalidation_url(
                WRITER.DispatcherSource(88, "issues", 1), 0,
            )
            for number in all_numbers:
                pull = {
                    "number": number, "state": "open", "draft": False, "body": bodies[number],
                    "base": {
                        "sha": default_head, "ref": "master",
                        "repo": {"full_name": "owner/repository", "default_branch": "master"},
                    },
                    "head": {
                        "sha": heads[number], "ref": f"pr-{number}",
                        "repo": {"full_name": "owner/repository"},
                    },
                }
                pulls[number] = pull
                common = {
                    "run_number": number, "run_attempt": 1, "event": "pull_request",
                    "head_sha": heads[number], "status": "completed", "conclusion": "success",
                    "repository": self.rest_repository(),
                    "pull_requests": [{
                        "number": number,
                        "base": {**pull["base"], "repo": self.rest_repository()},
                        "head": {**pull["head"], "repo": self.rest_repository()},
                    }],
                }
                sensor = {
                    **common, "id": 1_000 + number, "name": "PR governance review sensor",
                    "path": ".github/workflows/pr-governance-review-events.yml@master",
                }
                ci = {
                    **common, "id": 2_000 + number, "name": "CI", "workflow_id": 44,
                    "path": ".github/workflows/test-and-build.yml@master",
                }
                release = {
                    **common, "id": 3_000 + number, "name": "release-preflight", "workflow_id": 45,
                    "path": ".github/workflows/release-preflight.yml@master",
                }
                sensor_runs.append(sensor)
                ci_runs.append(ci)
                release_runs.append(release)
                # Exercise the contractual worst case: initial review-sensor
                # pagination consumes 3 pages while each CI endpoint remains
                # a single page, and the terminal refresh consumes 3 pages.
                sensor_runs.extend({
                    "id": 10_000_000 + number * 400 + extra,
                    "head_sha": heads[number], "event": "workflow_dispatch",
                } for extra in range(1, 300))
                head_runs[heads[number]] = {
                    "total_count": 300,
                    "workflow_runs": [sensor, ci, release, *[
                        {
                            "id": 20_000_000 + number * 400 + extra,
                            "head_sha": heads[number], "event": "workflow_dispatch",
                        }
                        for extra in range(1, 298)
                    ]],
                }
                identifier = 10_000 + number
                check_number_by_id[identifier] = number
                if number in preserved:
                    early_query = {
                        "source_run_id": str(1_000 + number), "ci_workflow_id": "44",
                        "ci_run_id": str(2_000 + number), "ci_run_number": str(number),
                        "ci_run_attempt": "1", "ci_status": "completed", "ci_conclusion": "success",
                        "release_workflow_id": "45", "release_run_id": str(3_000 + number),
                        "release_run_number": str(number), "release_run_attempt": "1",
                        "release_status": "completed", "release_conclusion": "success",
                        "pr_base_sha": default_head, "pr_head_sha": heads[number],
                        "pr_body_sha256": WRITER.pr_body_sha256(bodies[number]),
                    }
                    checks[identifier] = {
                        "id": identifier, "name": WRITER.CHECK_NAME, "head_sha": heads[number],
                        "external_id": f"krr-governance/v1/{heads[number]}/writer-71",
                        "updated_at": f"early-{number}", "app": {"id": 42},
                        "status": "completed", "conclusion": "success",
                        "details_url": "https://github.com/owner/repository/actions/runs/71?" + WRITER.urlencode(early_query),
                    }
                else:
                    checks[identifier] = {
                        "id": identifier, "name": WRITER.CHECK_NAME, "head_sha": heads[number],
                        "external_id": f"krr-governance/v1/{heads[number]}/dispatcher-88",
                        "updated_at": f"pending-{number}", "app": {"id": 42},
                        "status": "in_progress", "conclusion": None, "details_url": invalidation,
                    }

            pull_pages = [
                [pulls[number] for number in all_numbers[start:start + 100]]
                for start in range(0, len(all_numbers), 100)
            ]

            def run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                environment = kwargs.get("env")
                self.assertIsInstance(environment, dict)
                assert isinstance(environment, dict)
                segment = current_segment[0]
                app_token = f"app-read-{segment}"
                write_token = f"check-write-{segment}"
                if arguments[0] == sys.executable:
                    self.assertEqual(environment, {"GH_TOKEN": app_token, "PATH": os.environ["PATH"]})
                    if arguments[1].endswith("verify_push_issue.py"):
                        record("app", 2)
                    elif arguments[1].endswith("verify_pr_ready.py"):
                        record("app", 3)
                        record("graphql", 2)
                    else:
                        self.fail(f"unexpected verifier command: {arguments}")
                    return subprocess.CompletedProcess(arguments, 0, "", "")

                self.assertEqual(arguments[:2], ["gh", "api"])
                api_arguments = arguments[2:]
                endpoint = next(
                    (item for item in api_arguments if isinstance(item, str) and item.startswith("repos/")),
                    None,
                )
                self.assertIsNotNone(endpoint)
                assert isinstance(endpoint, str)
                write = "--method" in api_arguments
                if write:
                    self.assertEqual(environment, {"GH_TOKEN": write_token, "PATH": os.environ["PATH"]})
                    self.assertIn("PATCH", api_arguments)
                    record("write")
                    identifier = int(endpoint.rsplit("/", 1)[1])
                    number = check_number_by_id[identifier]
                    self.assertIn(number, terminal_numbers)
                    self.assertEqual(checks[identifier]["status"], "in_progress")
                    details = next(
                        item.split("=", 1)[1]
                        for item in api_arguments
                        if isinstance(item, str) and item.startswith("details_url=")
                    )
                    checks[identifier] = checks[identifier] | {
                        "updated_at": f"terminal-{number}", "status": "completed",
                        "conclusion": "success", "details_url": details,
                    }
                    terminal_patches.append(number)
                    return response(arguments, checks[identifier])

                token = environment.get("GH_TOKEN")
                if token == "default-read":
                    record("default")
                    if endpoint == "repos/owner/repository/actions/runs/88":
                        record("dispatcher_source")
                        source_reads[segment] += 1
                    elif (
                        endpoint in {
                            "repos/owner/repository/actions/workflows/test-and-build.yml",
                            "repos/owner/repository/actions/workflows/release-preflight.yml",
                        }
                        or endpoint.startswith("repos/owner/repository/actions/workflows/pr-governance-review-events.yml/runs?")
                        or endpoint.startswith("repos/owner/repository/actions/workflows/44/runs?")
                        or endpoint.startswith("repos/owner/repository/actions/workflows/45/runs?")
                    ):
                        pass
                    else:
                        self.fail(f"unexpected default-token endpoint: {endpoint}")
                else:
                    self.assertEqual(environment, {"GH_TOKEN": app_token, "PATH": os.environ["PATH"]})
                    if endpoint in {"repos/owner/repository", "repos/owner/repository/git/ref/heads/master"}:
                        record("rebind")
                    if endpoint.startswith("repos/owner/repository/pulls?state=open"):
                        snapshots[segment] += 1
                    record("app")
                    if endpoint.startswith("repos/owner/repository/actions/runs/"):
                        identifier = int(endpoint.rsplit("/", 1)[1])
                        if identifier in writer_runs and str(identifier) != WRITER.WRITER_RUN_ID:
                            record("prior_writer")

                if endpoint == "repos/owner/repository":
                    payload: object = {"default_branch": "master"}
                elif endpoint == "repos/owner/repository/git/ref/heads/master":
                    payload = {"object": {"sha": default_head}}
                elif endpoint == "repos/owner/repository/actions/runs/88":
                    payload = source
                elif endpoint.startswith("repos/owner/repository/actions/runs/"):
                    payload = writer_runs[int(endpoint.rsplit("/", 1)[1])]
                elif endpoint.startswith("repos/owner/repository/pulls?state=open"):
                    page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query).get("page", ["1"])[0])
                    payload = pull_pages[page - 1] if page <= len(pull_pages) else []
                elif endpoint.startswith("repos/owner/repository/pulls/"):
                    payload = pulls[int(endpoint.rsplit("/", 1)[1])]
                elif endpoint == "repos/owner/repository/actions/workflows/test-and-build.yml":
                    payload = {"id": 44}
                elif endpoint == "repos/owner/repository/actions/workflows/release-preflight.yml":
                    payload = {"id": 45}
                elif endpoint.startswith("repos/owner/repository/actions/workflows/pr-governance-review-events.yml/runs?"):
                    query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
                    head = query["head_sha"][0]
                    self.assertNotIn("event", query)
                    values = [run for run in sensor_runs if run["head_sha"] == head]
                    page = int(query.get("page", ["1"])[0])
                    offset = (page - 1) * WRITER.MAX_EVIDENCE_RUNS_PER_PAGE
                    payload = {"total_count": len(values), "workflow_runs": values[offset:offset + WRITER.MAX_EVIDENCE_RUNS_PER_PAGE]}
                elif endpoint.startswith("repos/owner/repository/actions/workflows/44/runs?"):
                    query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
                    head = query["head_sha"][0]
                    values = [run for run in ci_runs if run["head_sha"] == head]
                    payload = {"total_count": len(values), "workflow_runs": values}
                elif endpoint.startswith("repos/owner/repository/actions/workflows/45/runs?"):
                    query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
                    head = query["head_sha"][0]
                    values = [run for run in release_runs if run["head_sha"] == head]
                    payload = {"total_count": len(values), "workflow_runs": values}
                elif endpoint.startswith("repos/owner/repository/actions/workflows/66/runs?"):
                    payload = self.dispatcher_page(source)
                elif endpoint.startswith("repos/owner/repository/actions/runs?head_sha="):
                    head = endpoint.split("head_sha=", 1)[1].split("&", 1)[0]
                    query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
                    page = int(query.get("page", ["1"])[0])
                    offset = (page - 1) * WRITER.MAX_EVIDENCE_RUNS_PER_PAGE
                    values = head_runs[head]["workflow_runs"]
                    assert isinstance(values, list)
                    payload = {"total_count": head_runs[head]["total_count"], "workflow_runs": values[offset:offset + WRITER.MAX_EVIDENCE_RUNS_PER_PAGE]}
                elif endpoint.startswith("repos/owner/repository/check-runs/"):
                    identifier = int(endpoint.rsplit("/", 1)[1])
                    check_read_count[segment] += 1
                    if check_read_count[segment] <= len(all_numbers):
                        record("manifest")
                    check_reads[identifier] = check_reads.get(identifier, 0) + 1
                    value = checks[identifier]
                    if identifier == drift_check_id[0] and check_reads[identifier] == 3:
                        payload = value | {"updated_at": "newer-check-fingerprint"}
                    else:
                        payload = value
                else:
                    self.fail(f"unexpected API endpoint: {endpoint}")
                return response(arguments, payload)

            manifest = json.dumps(
                [[number, 10_000 + number] for number in all_numbers], separators=(",", ":"),
            )
            common_environment = {
                "GITHUB_ACTIONS": "true", "GITHUB_SHA": default_head, "GITHUB_REF_NAME": "master",
                "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all",
                "GOVERNANCE_TARGET_NUMBERS": json.dumps(list(preserved), separators=(",", ":")),
                "GOVERNANCE_PRESERVED_TARGET_NUMBERS": json.dumps(list(preserved), separators=(",", ":")),
                "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71", "GOVERNANCE_CHECK_MANIFEST": manifest,
                "GOVERNANCE_TERMINAL_ORDER_NUMBERS": json.dumps(list(terminal_numbers), separators=(",", ":")),
                "KRR_GOVERNANCE_CHECK_APP_ID": "42", "KRR_GOVERNANCE_APP_BOT_LOGIN": "krr-governance[bot]",
                "DEFAULT_READ_TOKEN": "default-read",
            }
            durations: list[float] = []
            original_last_write = WRITER._last_check_write_at
            self.addCleanup(setattr, WRITER, "_last_check_write_at", original_last_write)
            with patch.object(WRITER.subprocess, "run", side_effect=run), \
                patch.object(WRITER.time, "sleep", side_effect=sleep), \
                patch.object(WRITER.time, "monotonic", side_effect=monotonic):
                for segment in range(1, 5):
                    current_segment[0] = segment
                    check_read_count[segment] = 0
                    start = clock[0]
                    workflow_overhead("registration", 60, start, 300.0)
                    clock[0] = start + 300.0
                    # The child workflow performs four bootstrap and four
                    # rebind github.token reads before its App-token work.
                    workflow_overhead("bootstrap_default", 8, clock[0], 120.0)
                    clock[0] += WRITER.TERMINAL_AWAIT_STARTUP_AND_EVIDENCE_RESERVE_SECONDS
                    writer_identifier = 98 + segment
                    writer_runs[writer_identifier] = writer_run(writer_identifier, segment)
                    expected = terminal_numbers[(segment - 1) * 150:segment * 150]
                    before = len(terminal_patches)
                    WRITER._last_check_write_at = None  # Separate workflow-dispatch process.
                    with self.subTest(segment=segment), patch.object(WRITER, "WRITER_RUN_ID", str(writer_identifier)), patch.dict(os.environ, common_environment | {
                        "GH_TOKEN": f"app-read-{segment}", "CHECK_WRITE_TOKEN": f"check-write-{segment}",
                        "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(expected), separators=(",", ":")),
                        "GOVERNANCE_CONTINUATION_INDEX": str(segment),
                        "GOVERNANCE_COMPLETED_WRITER_RUN_IDS": json.dumps(list(range(99, writer_identifier)), separators=(",", ":")),
                    }):
                        self.assertEqual(WRITER.main(), 0)
                    durations.append(clock[0] - start)
                    # Registration and completion polling share one absolute
                    # 3750s terminal window; no registration time is free.
                    workflow_overhead("await", 115, start + 300.0, 3_450.0)
                    clock[0] = max(clock[0], start + 3_750.0)
                    writer_runs[writer_identifier] = writer_run(writer_identifier, segment, "completed")
                    self.assertEqual(terminal_patches[before:], list(expected))
                    self.assertEqual(source_reads[segment], 1)
                    self.assertEqual(snapshots[segment], 7)
                    self.assertEqual(len(WRITER._bound_check_runs), len(all_numbers))
                    self.assertEqual(
                        WRITER._bound_check_runs[(heads[preserved[0]], f"krr-governance/v1/{heads[preserved[0]]}/writer-71")],
                        10_000 + preserved[0],
                    )
                    self.assertEqual(
                        WRITER._bound_check_runs[(heads[expected[0]], f"krr-governance/v1/{heads[expected[0]]}/dispatcher-88")],
                        10_000 + expected[0],
                    )
                    self.assertEqual(len(transport[segment]["default"]), 11 + 6 * len(expected) + 50)
                    self.assertGreater(len(transport[segment]["app"]), 0)
                    self.assertEqual(len(transport[segment]["write"]), len(expected))
                    self.assertEqual(len(transport[segment]["manifest"]), len(all_numbers))
                    self.assertEqual(len(transport[segment]["dispatcher_source"]), 1)
                    self.assertEqual(len(transport[segment]["prior_writer"]), segment - 1)
                    self.assertEqual(len(transport[segment]["rebind"]), 2 * len(expected))
                    self.assertEqual(len(sleep_calls[segment]), len(expected))

                self.assertEqual(terminal_patches, list(terminal_numbers))
                self.assertEqual(len(set(terminal_patches)), len(terminal_numbers))
                self.assertEqual(len(terminal_patches) + len(preserved), len(all_numbers))
                self.assertTrue(all(
                    checks[10_000 + number]["status"] == "completed"
                    and checks[10_000 + number]["conclusion"] == "success"
                    for number in all_numbers
                ))

                # A new fingerprint at the final fence returns False from the
                # production finalizer, so main must fail without publishing a
                # replacement terminal state.
                for number in terminal_numbers:
                    identifier = 10_000 + number
                    checks[identifier] = checks[identifier] | {
                        "updated_at": f"retry-{number}", "status": "in_progress",
                        "conclusion": None, "details_url": invalidation,
                    }
                check_reads.clear()
                drift_check_id[0] = 10_000 + terminal_numbers[0]
                current_segment[0] = 5
                WRITER._last_check_write_at = None
                before_drift = len(terminal_patches)
                writer_runs[103] = writer_run(103, 1)
                with patch.object(WRITER, "WRITER_RUN_ID", "103"), patch.dict(os.environ, common_environment | {
                    "GH_TOKEN": "app-read-5", "CHECK_WRITE_TOKEN": "check-write-5",
                    "GOVERNANCE_COMPLETED_WRITER_RUN_IDS": "[]",
                    "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(terminal_numbers[:150]), separators=(",", ":")),
                    "GOVERNANCE_CONTINUATION_INDEX": "1",
                }):
                    self.assertEqual(WRITER.main(), 1)
                self.assertEqual(len(terminal_patches), before_drift)

        def rolling_maximum(timestamps: list[float]) -> int:
            return max(
                (
                    sum(window_end - 3_600 < value <= window_end for value in timestamps)
                    for window_end in timestamps
                ),
                default=0,
            )

        installation_rest = [
            value
            for segment in range(1, 5)
            for value in (
                *transport[segment]["app"], *transport[segment]["write"],
                *transport[segment]["registration"], *transport[segment]["await"],
            )
        ]
        graphql = [
            value for segment in range(1, 5) for value in transport[segment]["graphql"]
        ]
        # A 3610s dispatch-start barrier keeps the four
        # 150-head sensor/final-page=3 segments below the shared limit.
        self.assertLessEqual(rolling_maximum(installation_rest), 4_500)
        self.assertLessEqual(rolling_maximum(graphql), 4_500)
        default_rest = [
            value for segment in range(1, 5) for value in transport[segment]["default"]
        ]
        self.assertLessEqual(rolling_maximum(default_rest), 1_000)
        self.assertEqual([round(duration, 6) for duration in durations], [3_705.0, 3_705.0, 3_705.0, 2_885.0])
        writer_runtime_upper_bound = 150 * WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS
        startup_and_initial_read_reserve = WRITER.TERMINAL_AWAIT_STARTUP_AND_EVIDENCE_RESERVE_SECONDS
        await_timeout = 125 * 30.0
        # The fake monotonic clock accumulates binary 20.5s steps; permit only
        # that sub-millisecond representation error, not another write slot.
        self.assertTrue(all(
            duration <= 300.0 + startup_and_initial_read_reserve + writer_runtime_upper_bound + 0.001
            for duration in durations
        ))
        self.assertLessEqual(
            300.0 + writer_runtime_upper_bound + startup_and_initial_read_reserve,
            await_timeout,
        )
        self.assertAlmostEqual(sum(durations), 14_000.0)
        self.assertEqual(clock[0], 15_000.0)
        # Conservative root bound serializes every awaited writer with its
        # startup/read reserve, then adds the independent 81m+15m hand-off.
        self.assertEqual(4 * await_timeout + 5_760.0, 20_760.0)
        self.assertLess(4 * await_timeout + 5_760.0, 6 * 3_600)
        # 81m all-open invalidation plus its bounded 15m priority/drain hand-off
        # still leaves the four registered/awaited writer segments below Actions' 6h job cap.
        self.assertLess(81 * 60 + 15 * 60 + clock[0], 6 * 3_600)

    def test_production_manifest_binds_exact_ids_in_preserved_event_unrelated_order(self) -> None:
        """The all writer receives the dispatcher POST IDs, never a name-only lookup."""
        snapshot = self.snapshot((1, 72, 73))
        scoped_snapshot = WRITER.OpenSnapshot(
            (1, 73), snapshot.claimants,
            tuple(item for item in snapshot.pull_requests if item["number"] != 72),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        environment = {
            "GITHUB_ACTIONS": "true",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all",
            "GOVERNANCE_TARGET_NUMBERS": "[72,73]",
            "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]",
            "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
            # preserved source, related claimant, then unrelated snapshot PR.
            "GOVERNANCE_CHECK_MANIFEST": "[[72,701],[73,702],[1,703]]",
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": "[73,1]",
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": "[73,1]", "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=source), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(scoped_snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=None):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(
            WRITER._bound_check_runs,
            {
                (f"{1:040x}", f"krr-governance/v1/{1:040x}/dispatcher-88"): 703,
                (f"{72:040x}", f"krr-governance/v1/{72:040x}/writer-71"): 701,
                (f"{73:040x}", f"krr-governance/v1/{73:040x}/dispatcher-88"): 702,
            },
        )

    def test_production_main_uses_manifest_ids_for_preserved_and_terminal_dispatcher_generations(self) -> None:
        snapshot = self.snapshot((1, 72, 73))
        scoped_snapshot = WRITER.OpenSnapshot(
            (1, 73), snapshot.claimants,
            tuple(item for item in snapshot.pull_requests if item["number"] != 72),
        )
        heads = {item["number"]: item["head_sha"] for item in snapshot.pull_requests}
        base = "b" * 40
        early_query = {
            "source_run_id": "1", "ci_workflow_id": "2", "ci_run_id": "3", "ci_run_number": "4", "ci_run_attempt": "1",
            "ci_status": "completed", "ci_conclusion": "success", "release_workflow_id": "5", "release_run_id": "6",
            "release_run_number": "7", "release_run_attempt": "1", "release_status": "completed", "release_conclusion": "success",
            "pr_base_sha": base, "pr_head_sha": heads[72], "pr_body_sha256": WRITER.pr_body_sha256("Fixes #64"),
        }
        values: dict[int, dict[str, object]] = {
            701: {"id": 701, "name": WRITER.CHECK_NAME, "head_sha": heads[72], "external_id": f"krr-governance/v1/{heads[72]}/writer-71", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "completed", "conclusion": "success", "details_url": f"https://github.com/owner/repository/actions/runs/71?{WRITER.urlencode(early_query)}"},
            702: {"id": 702, "name": WRITER.CHECK_NAME, "head_sha": heads[73], "external_id": f"krr-governance/v1/{heads[73]}/dispatcher-88", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/88?dispatcher_run_id=88&carry_pending=0"},
            703: {"id": 703, "name": WRITER.CHECK_NAME, "head_sha": heads[1], "external_id": f"krr-governance/v1/{heads[1]}/dispatcher-88", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/88?dispatcher_run_id=88&carry_pending=0"},
        }
        dispatcher = {"id": 88, "name": WRITER.DISPATCHER_NAME, "path": ".github/workflows/pr-governance.yml@master", "event": "issues", "head_sha": "d" * 40, "head_branch": "master", "workflow_id": 66, "repository": self.rest_repository(), "run_number": 1, "run_attempt": 1, "status": "in_progress", "conclusion": None, "created_at": "2026-08-30T00:00:00Z"}
        writer = {"id": 99, "name": "PR governance status writer", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "head_sha": "d" * 40, "repository": self.rest_repository(), "run_attempt": 1, "status": "in_progress"}
        reads: list[int] = []; terminal_ids: list[int] = []
        def api(endpoint: str, *, default_token: bool = False) -> object:
            if endpoint.endswith("/actions/runs/88"):
                return dispatcher
            if endpoint.endswith("/actions/runs/99"):
                return writer
            identifier = int(endpoint.rsplit("/", 1)[1]); reads.append(identifier)
            return values[identifier]
        def write(arguments: list[str], *, check_write: bool = False, default_token: bool = False) -> str:
            self.assertTrue(check_write)
            identifier = int(next(value.rsplit("/", 1)[1] for value in arguments if "/check-runs/" in value))
            details = next(value.split("=", 1)[1] for value in arguments if value.startswith("details_url="))
            terminal_ids.append(identifier); values[identifier] = values[identifier] | {"status": "completed", "conclusion": "failure", "details_url": details}
            return json.dumps(values[identifier])
        def terminal(number: int, _claimants: object, _path: str, _evidence: object, *, defer_terminal: bool) -> None:
            self.assertTrue(defer_terminal)
            value = WRITER.check_run(heads[number])
            WRITER.write_check(heads[number], state="failure", description="fixture", details_url="https://github.com/owner/repository/actions/runs/88", existing=value)
            return None
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[72,73]",
            "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]", "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
            "GOVERNANCE_CHECK_MANIFEST": "[[72,701],[73,702],[1,703]]",
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": "[73,1]",
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": "[73,1]", "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "api_json", side_effect=api), patch.object(WRITER, "command", side_effect=write), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "observed_invalidations", return_value=(scoped_snapshot, frozenset())), \
             patch.object(WRITER, "process", side_effect=terminal), patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER, "reject_newer_dispatcher_barrier"):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(terminal_ids, [702, 703])
        self.assertNotIn(701, reads)
        self.assertEqual(reads.count(702), 2)
        self.assertEqual(reads.count(703), 2)

    def test_production_manifest_rejects_reordered_or_duplicate_ids_before_revalidation(self) -> None:
        snapshot = self.snapshot((1, 72, 73))
        source = WRITER.DispatcherSource(88, "issues", 1)
        base = {
            "GITHUB_ACTIONS": "true",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all",
            "GOVERNANCE_TARGET_NUMBERS": "[72,73]", "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]",
            "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": "[73,1]",
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": "[73,1]", "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        for manifest in ("[[72,701],[73,702]]", "[[72,701],[73,702],[1,703],[74,704]]", "[[72,701],[1,703],[73,702]]", "[[72,701],[73,701],[1,703]]"):
            with self.subTest(manifest=manifest), self.identity(), patch.dict(os.environ, base | {"GOVERNANCE_CHECK_MANIFEST": manifest}), \
                 patch.object(WRITER, "trusted_dispatcher_source", return_value=source), \
                 patch.object(WRITER, "open_snapshot", return_value=snapshot), \
                 patch.object(WRITER, "observed_invalidations") as observed:
                self.assertEqual(WRITER.main(), 1)
                observed.assert_not_called()

    def test_bound_manifest_id_is_reread_by_id_before_an_immutable_write(self) -> None:
        head = "a" * 40
        external = f"krr-governance/v1/{head}/writer-71"
        value = {
            "id": 701, "name": WRITER.CHECK_NAME, "head_sha": head, "external_id": external,
            "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42},
        }
        WRITER._bound_check_runs.clear()
        WRITER._bound_check_runs[(head, external)] = 701
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "api_json", return_value=value) as api:
            self.assertEqual(WRITER.check_run_for_external_id(head, external), value)
        api.assert_called_once_with("repos/owner/repository/check-runs/701")

    def test_all_writer_never_posts_a_replacement_when_a_bound_generation_disappears(self) -> None:
        head = "a" * 40
        external = f"krr-governance/v1/{head}/dispatcher-88"
        WRITER._bound_check_runs.clear()
        WRITER._bound_check_runs[(head, external)] = 701
        with self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all"}), \
             patch.object(WRITER, "api_json", return_value=None), patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.write_check(
                    head, state="in_progress", description="missing", details_url="https://github.com/owner/repository/actions/runs/88",
                )
        command.assert_not_called()

    def test_terminal_rebind_rejects_default_branch_advance(self) -> None:
        responses = [
            {"default_branch": "master"},
            {"object": {"sha": "e" * 40}},
        ]
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "api_json", side_effect=responses):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.rebind_trusted_default_writer()

    def test_terminal_rebind_uses_app_read_transport_and_fails_closed(self) -> None:
        environment = {
            "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read",
        }
        for mode in ("success", "branch-changed", "ref-advanced", "transport-failure"):
            with self.subTest(mode=mode):
                seen: list[dict[str, str]] = []
                def run(arguments, **kwargs):
                    request_environment = kwargs.get("env")
                    self.assertEqual(request_environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
                    seen.append(request_environment)
                    endpoint = arguments[-1]
                    if endpoint == "repos/owner/repository":
                        if mode == "transport-failure":
                            return subprocess.CompletedProcess(arguments, 1, "", "sensitive upstream body")
                        branch = "release" if mode == "branch-changed" else "master"
                        return subprocess.CompletedProcess(arguments, 0, json.dumps({"default_branch": branch}), "")
                    if endpoint == "repos/owner/repository/git/ref/heads/master":
                        head = "e" * 40 if mode == "ref-advanced" else "d" * 40
                        return subprocess.CompletedProcess(arguments, 0, json.dumps({"object": {"sha": head}}), "")
                    self.fail(f"unexpected rebind transport: {arguments}")

                with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER.subprocess, "run", side_effect=run):
                    if mode == "success":
                        WRITER.rebind_trusted_default_writer()
                    else:
                        with self.assertRaises(WRITER.GovernanceError):
                            WRITER.rebind_trusted_default_writer()
                self.assertEqual(len(seen), 2 if mode in {"success", "ref-advanced"} else 1)

    def test_later_invalidator_details_fingerprint_blocks_terminal_patch(self) -> None:
        head = "a" * 40
        baseline = {
            "id": 12, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": WRITER.check_external_id(head), "updated_at": "one",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/88", "app": {"id": 42},
        }
        later = {**baseline, "updated_at": "two", "details_url": "https://github.com/owner/repository/actions/runs/89"}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=later):
            self.assertTrue(WRITER.check_changed_since(head, WRITER.check_fingerprint(baseline)))

    def test_main_skips_final_evidence_for_non_success_decisions(self) -> None:
        snapshot = self.snapshot((72, 73))
        first = WRITER.PendingDecision(72, "a" * 40, "b" * 40, (), "failure", "failed", None, None, None, "c" * 64)
        second = WRITER.PendingDecision(73, "c" * 40, "d" * 40, (), "failure", "failed", None, None, None, "c" * 64)
        calls: list[str] = []
        def process(number, *_args, **_kwargs):
            calls.append(f"process-{number}")
            return first if number == 72 else second
        def final(head, _evidence):
            calls.append(f"evidence-{head[0]}")
            return WRITER.EvidenceSnapshot({}, {}, {})
        def finalize(decision, *_args):
            calls.append(f"finalize-{decision.number}")
            return True
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=process), patch.object(WRITER, "final_evidence_for_pr", side_effect=final) as final_evidence, \
             patch.object(WRITER, "finalize_decision", side_effect=finalize):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(calls, ["process-72", "finalize-72", "process-73", "finalize-73"])
        final_evidence.assert_not_called()

    def test_main_continues_after_one_pr_fails(self) -> None:
        with patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), patch.object(WRITER, "REPOSITORY", "owner/repository"), \
             patch.object(WRITER, "SERVER_URL", "https://github.com"), \
             patch.object(WRITER, "WRITER_RUN_ID", "99"), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=self.snapshot((72, 73), {"64": frozenset({72, 73})})), \
             patch.object(WRITER, "observed_invalidations", return_value=(self.snapshot((72, 73), {"64": frozenset({72, 73})}), frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=[WRITER.GovernanceError("bad"), None]) as process:
            self.assertEqual(WRITER.main(), 1)
            self.assertEqual(process.call_count, 2)

    def test_open_pulls_collects_late_pages_without_a_target_limit(self) -> None:
        payload = [[{"number": number, "state": "open"} for number in range(1, 101)], [{"number": 301, "state": "open"}]]
        def api(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return payload[page - 1] if page <= len(payload) else []
        with self.identity(), patch.object(WRITER, "api_json", side_effect=api):
            self.assertEqual(WRITER.open_pulls()[-1], 301)

    def test_single_snapshot_indexes_300_prs_and_multi_issue_claimants_in_one_paged_read(self) -> None:
        pages = []
        for start in (1, 101, 201):
            pages.append([{"number": number, "state": "open", "draft": False, "body": "Fixes #64" if number != 250 else "Fixes #64; closes #65", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": f"{number:040x}"[-40:], "repo": {"full_name": "owner/repository"}}} for number in range(start, start + 100)])
        def api(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return pages[page - 1] if page <= len(pages) else []
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", side_effect=api) as command:
            snapshot = WRITER.open_snapshot()
        self.assertEqual(len(snapshot.numbers), 300)
        self.assertEqual(snapshot.claimants["64"], frozenset(range(1, 301)))
        self.assertEqual(snapshot.claimants["65"], frozenset({250}))
        self.assertEqual(command.call_count, 5)

    def test_snapshot_normalizes_missing_body_but_rejects_duplicate_pr(self) -> None:
        valid = {"number": 72, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        bodyless = {
            "number": 71, "state": "open", "draft": False, "body": None,
            "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}},
        }
        complete = {**valid, "head": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}}}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", return_value=[bodyless, complete]):
            snapshot = WRITER.open_snapshot()
        self.assertEqual(snapshot.numbers, (71, 72))
        self.assertEqual(snapshot.pull_requests[0]["body"], "")
        self.assertEqual(snapshot.claimants["64"], frozenset({72}))
        first = [{**complete, "number": number, "head": {"sha": f"{number:040x}"[-40:], "repo": {"full_name": "owner/repository"}}} for number in range(1, 101)]
        first[71] = complete
        def duplicate_page(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return first if page == 1 else [complete] if page == 2 else []
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", side_effect=duplicate_page):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_snapshot_excludes_fork_claimant_from_local_canonical_issue(self) -> None:
        local = {"number": 72, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}}}
        fork = {"number": 73, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "fork/repository"}}}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", return_value=[local, fork]):
            snapshot = WRITER.open_snapshot()
        self.assertEqual(snapshot.numbers, (72,))
        self.assertEqual(snapshot.claimants["64"], frozenset({72}))

    def test_snapshot_rejects_a_duplicate_fork_on_later_page(self) -> None:
        fork = {"number": 73, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "fork/repository"}}}
        first = [{**fork, "number": number, "head": {"sha": f"{number:040x}"[-40:], "repo": {"full_name": "fork/repository"}}} for number in range(1, 101)]
        first[72] = fork
        def duplicate_page(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return first if page == 1 else [fork] if page == 2 else []
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", side_effect=duplicate_page):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_300_pr_main_processes_all_after_one_failure_with_one_snapshot(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)), {"64": frozenset(range(1, 301))})
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), patch.object(WRITER, "open_snapshot", return_value=snapshot) as open_snapshot, \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=[WRITER.GovernanceError("one failed"), *([None] * 299)]) as process:
            self.assertEqual(WRITER.main(), 1)
        open_snapshot.assert_called_once_with()
        self.assertEqual(process.call_count, 300)

    def test_nonproduction_event_budget_remains_bounded(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, 99, "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision) as process, \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=False) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(process.call_count, 101)
        self.assertEqual(finalize.call_count, 100)

    def test_event_sourced_all_segment_terminalizes_all_one_hundred_fifty_successes(self) -> None:
        numbers = tuple(range(1, 151))
        snapshot = self.snapshot(numbers)
        manifest = json.dumps([[number, 10_000 + number] for number in numbers], separators=(",", ":"))
        environment = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[1]",
            "GOVERNANCE_CHECK_MANIFEST": manifest,
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        finalized: list[int] = []

        def decision(number: int, *_args, **_kwargs):
            return WRITER.PendingDecision(
                number, f"{number:040x}"[-40:], "b" * 40, (number,), "success", "ok",
                77, (), "64", "c" * 64,
            )

        def finalize(value, *_args):
            finalized.append(value.number)
            return True

        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=decision) as process, \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", side_effect=finalize):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(process.call_count, 150)
        self.assertEqual(finalized, list(numbers))

    def test_all_segment_budget_exhaustion_fails_closed_instead_of_skipping_tail(self) -> None:
        numbers = tuple(range(1, 151))
        snapshot = self.snapshot(numbers)
        manifest = json.dumps([[number, 10_000 + number] for number in numbers], separators=(",", ":"))
        environment = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[1]",
            "GOVERNANCE_CHECK_MANIFEST": manifest,
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        exhausted = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (), "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=exhausted) as process, \
             patch.object(WRITER, "finalize_decision", return_value=True) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(process.call_count, 76)
        self.assertEqual(finalize.call_count, 75)

    def test_all_segment_fails_closed_when_a_manifested_terminal_is_not_published(self) -> None:
        numbers = tuple(range(1, 151))
        snapshot = self.snapshot(numbers)
        manifest = json.dumps([[number, 10_000 + number] for number in numbers], separators=(",", ":"))
        environment = {
            "GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[1]",
            "GOVERNANCE_CHECK_MANIFEST": manifest,
            "GOVERNANCE_TERMINAL_ORDER_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS": json.dumps(list(numbers), separators=(",", ":")),
            "GOVERNANCE_CONTINUATION_INDEX": "1",
        }
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (1,), "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision) as process, \
             patch.object(WRITER, "finalize_decision", return_value=False) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(process.call_count, 1)
        finalize.assert_called_once()

    def test_all_open_event_priority_leads_the_local_terminal_queue(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 201)))
        finalized: list[int] = []

        def decision(number: int, *_args, **_kwargs):
            return WRITER.PendingDecision(number, f"{number:040x}"[-40:], "b" * 40, 99, "failure", "bad", None, None, None, "c" * 64)

        def finalize(value, *_args):
            finalized.append(value.number)
            return True

        with self.identity(), patch.dict(os.environ, {
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[150,149]",
        }), patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=decision) as process, \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", side_effect=finalize):
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(process.call_args_list[0].args[0], 150)
        self.assertEqual(process.call_args_list[1].args[0], 149)
        self.assertEqual(finalized[:2], [150, 149])
        self.assertEqual(len(finalized), 100)
        self.assertNotIn(100, finalized)

    def test_carry_precedes_fresh_targets_and_converges_with_any_dispatcher_gaps(self) -> None:
        numbers = tuple(range(1, 451))
        remaining = frozenset(numbers)
        # carryがあるheadは、後続dispatcherでも新規headより先に並ぶ。
        for _dispatcher_id in (7, 8, 91, 1042, 1043, 99999, 100003, 100004, 900000):
            ordered = WRITER.governance_order(self.snapshot(numbers), remaining)
            self.assertEqual(ordered[:len(remaining)], tuple(sorted(remaining)))
            completed = frozenset(ordered[:50])
            remaining = remaining - completed
        self.assertEqual(remaining, frozenset())

    def test_200_permanent_drafts_do_not_starve_carried_or_fresh_terminal_targets(self) -> None:
        numbers = tuple(range(1, 406))
        drafts = frozenset((*range(1, 201), *range(206, 406)))
        snapshot = self.snapshot(numbers, drafts=drafts)
        # PR #204/#205 were previously budget-deferred.  They must run first,
        # then fresh terminal PR #201-#203, before any of 400 Draft targets.
        ordered = WRITER.governance_order(snapshot, frozenset({204, 205}))
        self.assertEqual(ordered[:5], (204, 205, 201, 202, 203))
        self.assertEqual(set(ordered[5:]), set(drafts))

    def test_300_ready_pr_fails_closed_when_terminal_reservation_is_exhausted(self) -> None:
        numbers = tuple(range(1, 301))
        snapshot = WRITER.OpenSnapshot(numbers, {"64": frozenset(numbers)}, tuple({"number": number, "isDraft": False, "body": "Fixes #64"} for number in numbers))
        evidence = WRITER.EvidenceSnapshot({}, {}, {})
        def current(number: int):
            value = self.pull(number)
            value["head"]["sha"] = f"{number:040x}"[-40:]  # type: ignore[index]
            return value
        generation = WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success")
        release = WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")
        transport: dict[str, int] = {"app_rest": 0, "default_rest": 0, "graphql": 0}
        class Result:
            returncode = 0
        def verifier_transport(arguments, **kwargs):
            command, environment = arguments, kwargs["env"]
            self.assertEqual(environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
            self.assertEqual(command[0], sys.executable)
            if command[1].endswith("verify_push_issue.py"):
                transport["app_rest"] += 2  # pinned range and referenced Issue.
            elif command[1].endswith("verify_pr_ready.py"):
                transport["app_rest"] += 3  # PR, comments/reactions and cached closer inputs.
                transport["graphql"] += 2  # review threads and reviews.
            else:
                self.fail(f"unexpected verifier command: {command}")
            return Result()
        with self.identity(), patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=evidence), patch.object(WRITER, "pull", side_effect=current), \
             patch.object(WRITER, "write_governance_check", side_effect=range(1, 1000)) as post, patch.object(WRITER.subprocess, "run", side_effect=verifier_transport), \
             patch.object(WRITER, "check_baseline", return_value=0) as baseline, \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=[generation, release] * 900), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False), \
             patch.object(WRITER, "check_fence", return_value=(False, 0, False)) as check_run_fence, \
             patch.object(WRITER, "rebind_trusted_default_writer"), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=evidence) as final_evidence, \
             patch.dict(os.environ, {"GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read", "CHECK_WRITE_TOKEN": "check-write", "KRR_GOVERNANCE_CHECK_APP_ID": "42", "GOVERNANCE_DISPATCHER_RUN_ID": "88"}):
            self.assertEqual(WRITER.main(), 1)
        # The dispatcher owns event invalidation.  The writer emits one
        # terminal status per PR, never a duplicate pending status.
        self.assertEqual(post.call_count, 200)
        self.assertEqual(final_evidence.call_count, 200)
        self.assertEqual(check_run_fence.call_count, 200)
        self.assertEqual(baseline.call_count, 201)
        # terminalの実経路はsingle final evidence、check fence、closer、
        # writer/dispatcher/write fenceの合計9 App read/成功headである。
        observed_terminal_app_reads = 9 * post.call_count
        self.assertEqual(observed_terminal_app_reads, 1_800)
        max_terminal_app_reads = 9 * 400
        self.assertLessEqual(observed_terminal_app_reads, max_terminal_app_reads)
        self.assertLess(max_terminal_app_reads, 4_500)
        # Keep the small fixed dispatcher/bootstrap metadata allowance and
        # reserve the complete 400-success-terminal worst case. The only
        # default-token operation is the one-time trusted source binding.
        transport["app_rest"] += 100 + observed_terminal_app_reads
        transport["default_rest"] += 1
        transport["graphql"] += 1200
        self.assertLess(transport["app_rest"], 4500)
        self.assertLess(transport["default_rest"], 950)
        self.assertLess(transport["graphql"], 2500)

    def test_terminal_dispatcher_fence_uses_app_read_transport_fails_closed_and_recovers(self) -> None:
        """Fence reads must not spend the lower-rate default workflow token."""
        head = "a" * 40
        current = self.dispatcher_run(88, status="completed", conclusion="success")
        newer_failed = self.dispatcher_run(
            7, status="completed", conclusion="failure", created_at="2026-08-30T00:01:00Z",
        )
        recovered = self.dispatcher_run(
            7, status="completed", conclusion="success", created_at="2026-08-30T00:01:00Z",
        )
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
            "GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read",
        }

        def exercise(mode: str) -> list[dict[str, str]]:
            seen: list[dict[str, str]] = []
            def run(arguments, **kwargs):
                command = arguments
                request_environment = kwargs.get("env")
                self.assertEqual(request_environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
                seen.append(request_environment)
                endpoint = command[-1]
                if endpoint.endswith("/actions/runs/88"):
                    if mode == "source-http-400":
                        return subprocess.CompletedProcess(command, 1, "", "sensitive upstream body")
                    if mode == "source-invalid-json":
                        return subprocess.CompletedProcess(command, 0, "{", "")
                    return subprocess.CompletedProcess(command, 0, json.dumps(current), "")
                if "actions/workflows/66/runs?" in endpoint:
                    if mode == "page-http-400":
                        return subprocess.CompletedProcess(command, 1, "", "sensitive upstream body")
                    if mode == "page-invalid-json":
                        return subprocess.CompletedProcess(command, 0, "{", "")
                    if mode == "page-cap":
                        return subprocess.CompletedProcess(command, 0, json.dumps(self.dispatcher_page(current, total_count=100)), "")
                    return subprocess.CompletedProcess(command, 0, json.dumps(self.dispatcher_page(current)), "")
                self.fail(f"unexpected dispatcher fence transport: {command}")

            with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER.subprocess, "run", side_effect=run):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier(head)
            return seen

        for mode in ("source-http-400", "source-invalid-json", "page-http-400", "page-invalid-json", "page-cap"):
            with self.subTest(mode=mode):
                self.assertGreaterEqual(len(exercise(mode)), 1)

        phase = {"source": current, "page": self.dispatcher_page(current, newer_failed)}
        seen: list[dict[str, str]] = []
        def recover_transport(arguments, **kwargs):
            request_environment = kwargs.get("env")
            self.assertEqual(request_environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
            seen.append(request_environment)
            endpoint = arguments[-1]
            if endpoint.endswith("/actions/runs/88") or endpoint.endswith("/actions/runs/7"):
                return subprocess.CompletedProcess(arguments, 0, json.dumps(phase["source"]), "")
            if "actions/workflows/66/runs?" in endpoint:
                return subprocess.CompletedProcess(arguments, 0, json.dumps(phase["page"]), "")
            self.fail(f"unexpected dispatcher fence transport: {arguments}")

        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER.subprocess, "run", side_effect=recover_transport):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier(head)
            phase["source"] = recovered
            phase["page"] = self.dispatcher_page(recovered)
            os.environ["GOVERNANCE_DISPATCHER_RUN_ID"] = "7"
            WRITER.reject_newer_dispatcher_barrier(head)
        self.assertEqual(len(seen), 4)

    def test_400_distinct_terminal_failures_finalize_with_only_app_reads(self) -> None:
        """Exercise the real finalize/read fence path; fake only HTTP transport."""
        numbers = tuple(range(1, 401))
        checks: dict[str, dict[str, object]] = {}
        for number in numbers:
            head = f"{number:040x}"[-40:]
            checks[head] = {
                "id": number, "name": WRITER.CHECK_NAME, "head_sha": head,
                "external_id": f"krr-governance/v1/{head}/dispatcher-88", "updated_at": "initial",
                "app": {"id": 42}, "status": "in_progress", "conclusion": None,
                "details_url": "https://github.com/owner/repository/actions/runs/88",
            }
        dispatcher = self.dispatcher_run(88, status="completed", conclusion="success")
        writer = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": "d" * 40,
            "repository": self.rest_repository(), "run_attempt": 1, "status": "in_progress",
        }
        app_reads = 0
        terminal_read_tokens: set[str] = set()
        write_calls = 0

        def run(arguments, **kwargs):
            nonlocal app_reads, write_calls
            command = arguments
            api_arguments = command[2:]
            endpoint = next((item for item in api_arguments if isinstance(item, str) and item.startswith("repos/")), None)
            self.assertIsNotNone(endpoint)
            assert endpoint is not None
            environment = kwargs.get("env")
            if "--method" in api_arguments:
                self.assertEqual(environment, {"GH_TOKEN": "check-write", "PATH": os.environ["PATH"]})
                self.assertIn("PATCH", api_arguments)
                identifier = int(endpoint.rsplit("/", 1)[1])
                current = next(value for value in checks.values() if value["id"] == identifier)
                details = next(item.split("=", 1)[1] for item in api_arguments if item.startswith("details_url="))
                current.update({"updated_at": f"terminal-{identifier}", "status": "completed", "conclusion": "failure", "details_url": details})
                write_calls += 1
                return subprocess.CompletedProcess(command, 0, json.dumps(current), "")
            if environment == {"GH_TOKEN": "default-read", "PATH": os.environ["PATH"]}:
                terminal_read_tokens.add(environment["GH_TOKEN"])
                self.assertIn(endpoint, {"repos/owner/repository", "repos/owner/repository/git/ref/heads/master"})
            else:
                self.assertEqual(environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
                terminal_read_tokens.add(environment["GH_TOKEN"])
                app_reads += 1
            if endpoint == "repos/owner/repository":
                payload: object = {"default_branch": "master"}
            elif endpoint == "repos/owner/repository/git/ref/heads/master":
                payload = {"object": {"sha": "d" * 40}}
            elif endpoint == "repos/owner/repository/actions/runs/99":
                payload = writer
            elif endpoint == "repos/owner/repository/actions/runs/88":
                payload = dispatcher
            elif endpoint.startswith("repos/owner/repository/actions/workflows/66/runs?"):
                payload = self.dispatcher_page(dispatcher)
            elif endpoint.startswith("repos/owner/repository/check-runs/"):
                identifier = int(endpoint.rsplit("/", 1)[1])
                payload = next(value for value in checks.values() if value["id"] == identifier)
            elif endpoint.startswith("repos/owner/repository/commits/") and "/check-runs?" in endpoint:
                head = endpoint.split("/commits/", 1)[1].split("/check-runs?", 1)[0]
                payload = {"check_runs": [checks[head]]}
            else:
                self.fail(f"unexpected terminal transport: {command}")
            return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")

        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "88",
            "GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read", "CHECK_WRITE_TOKEN": "check-write",
            "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER.subprocess, "run", side_effect=run):
            for number in numbers:
                head = f"{number:040x}"[-40:]
                baseline = WRITER.check_fingerprint(checks[head])
                decision = WRITER.PendingDecision(
                    number, head, "b" * 40, baseline, "failure", "fixture", None, None, None, "c" * 64,
                )
                self.assertTrue(WRITER.finalize_decision(decision, {}, WRITER.EvidenceSnapshot({}, {}, {})))
        self.assertEqual(write_calls, 400)
        self.assertEqual(terminal_read_tokens, {"app-read"})
        # Each Check Run listing rereads page one as its race fence.
        self.assertEqual(app_reads, 4_800)
        failure_segment_reads = 12 * 150
        self.assertEqual(failure_segment_reads, 1_800)
        self.assertLessEqual(600 + 200 + (6 * 150) + failure_segment_reads + 150, 4_500)

    def test_event_sourced_terminal_success_segments_include_final_evidence_with_only_app_reads(self) -> None:
        """Run the success terminal fence without replacing any read helper."""
        numbers = tuple(range(1, 401))
        default_head = "d" * 40
        checks: dict[str, dict[str, object]] = {}
        pulls: dict[int, dict[str, object]] = {}
        run_pages: dict[str, dict[str, object]] = {}
        expected_generations: dict[str, tuple[WRITER.Generation, WRITER.Generation]] = {}
        for number in numbers:
            head = f"{number:040x}"[-40:]
            checks[head] = {
                "id": number, "name": WRITER.CHECK_NAME, "head_sha": head,
                "external_id": f"krr-governance/v1/{head}/dispatcher-88", "updated_at": "initial",
                "app": {"id": 42}, "status": "in_progress", "conclusion": None,
                "details_url": "https://github.com/owner/repository/actions/runs/88",
            }
            pull_request = {
                "number": number, "state": "open", "draft": False, "body": "Fixes #64",
                "base": {"sha": default_head, "ref": "master", "repo": {"full_name": "owner/repository", "default_branch": "master"}},
                "head": {"sha": head, "repo": {"full_name": "owner/repository"}},
            }
            pulls[number] = pull_request
            common = {
                "run_number": number, "run_attempt": 1, "event": "pull_request", "head_sha": head,
                "status": "completed", "conclusion": "success", "repository": self.rest_repository(),
                "pull_requests": [{
                    "number": number,
                    "base": {**pull_request["base"], "repo": self.rest_repository()},
                    "head": {**pull_request["head"], "repo": self.rest_repository()},
                }],
            }
            sensor_run = {
                **common, "id": 1_000 + number, "name": "PR governance review sensor",
                "path": ".github/workflows/pr-governance-review-events.yml@master",
            }
            ci_run = {
                **common, "id": 2_000 + number, "name": "CI", "workflow_id": 44,
                "path": ".github/workflows/test-and-build.yml@master",
            }
            release_run = {
                **common, "id": 3_000 + number, "name": "release-preflight", "workflow_id": 45,
                "path": ".github/workflows/release-preflight.yml@master",
            }
            run_pages[head] = {"total_count": 3, "workflow_runs": [sensor_run, ci_run, release_run]}
            expected_generations[head] = (
                WRITER.Generation("CI", ".github/workflows/test-and-build.yml", 44, 2_000 + number, number, 1, "completed", "success"),
                WRITER.Generation("release-preflight", ".github/workflows/release-preflight.yml", 45, 3_000 + number, number, 1, "completed", "success"),
            )
        dispatcher = self.dispatcher_run(88, status="completed", conclusion="success")
        writer = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": default_head,
            "repository": self.rest_repository(), "run_attempt": 1, "status": "in_progress",
        }
        app_reads = 0
        read_tokens: set[str] = set()
        write_calls = 0

        def run(arguments, **kwargs):
            nonlocal app_reads, write_calls
            command = arguments
            api_arguments = command[2:]
            endpoint = next((item for item in api_arguments if isinstance(item, str) and item.startswith("repos/")), None)
            self.assertIsNotNone(endpoint)
            assert endpoint is not None
            environment = kwargs.get("env")
            if "--method" in api_arguments:
                self.assertEqual(environment, {"GH_TOKEN": "check-write", "PATH": os.environ["PATH"]})
                self.assertIn("PATCH", api_arguments)
                identifier = int(endpoint.rsplit("/", 1)[1])
                current = next(value for value in checks.values() if value["id"] == identifier)
                details = next(item.split("=", 1)[1] for item in api_arguments if item.startswith("details_url="))
                current.update({"updated_at": f"terminal-{identifier}", "status": "completed", "conclusion": "success", "details_url": details})
                write_calls += 1
                return subprocess.CompletedProcess(command, 0, json.dumps(current), "")
            if environment == {"GH_TOKEN": "default-read", "PATH": os.environ["PATH"]}:
                read_tokens.add(environment["GH_TOKEN"])
                self.assertIn(endpoint, {"repos/owner/repository", "repos/owner/repository/git/ref/heads/master"})
            else:
                self.assertEqual(environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
                read_tokens.add(environment["GH_TOKEN"])
                app_reads += 1
            if endpoint == "repos/owner/repository":
                payload: object = {"default_branch": "master"}
            elif endpoint == "repos/owner/repository/git/ref/heads/master":
                payload = {"object": {"sha": default_head}}
            elif endpoint == "repos/owner/repository/actions/runs/99":
                payload = writer
            elif endpoint == "repos/owner/repository/actions/runs/88":
                payload = dispatcher
            elif endpoint.startswith("repos/owner/repository/actions/workflows/66/runs?"):
                payload = self.dispatcher_page(dispatcher)
            elif endpoint.startswith("repos/owner/repository/actions/runs?head_sha="):
                head = endpoint.split("head_sha=", 1)[1].split("&", 1)[0]
                payload = run_pages[head]
            elif endpoint.startswith("repos/owner/repository/pulls/"):
                payload = pulls[int(endpoint.rsplit("/", 1)[1])]
            elif endpoint.startswith("repos/owner/repository/check-runs/"):
                identifier = int(endpoint.rsplit("/", 1)[1])
                payload = next(value for value in checks.values() if value["id"] == identifier)
            elif endpoint.startswith("repos/owner/repository/commits/") and "/check-runs?" in endpoint:
                head = endpoint.split("/commits/", 1)[1].split("/check-runs?", 1)[0]
                payload = {"check_runs": [checks[head]]}
            else:
                self.fail(f"unexpected success terminal transport: {command}")
            return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")

        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": default_head, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "88",
            "GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read", "CHECK_WRITE_TOKEN": "check-write",
            "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        segment_reads: list[int] = []
        segment_writes: list[int] = []
        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER.subprocess, "run", side_effect=run):
            WRITER._bound_check_runs.clear()
            for start in range(0, len(numbers), 150):
                before_reads = app_reads
                before_writes = write_calls
                for number in numbers[start:start + 150]:
                    head = f"{number:040x}"[-40:]
                    evidence = WRITER.final_evidence_for_pr(head, initial)
                    baseline = WRITER.check_fingerprint(checks[head])
                    decision = WRITER.PendingDecision(
                        number, head, default_head, baseline, "success", "fixture", 1_000 + number,
                        expected_generations[head], "64", WRITER.pr_body_sha256("Fixes #64"),
                    )
                    self.assertTrue(WRITER.finalize_decision(decision, {"64": frozenset({number})}, evidence))
                segment_reads.append(app_reads - before_reads)
                segment_writes.append(write_calls - before_writes)
        self.assertEqual(write_calls, 400)
        self.assertEqual(read_tokens, {"app-read"})
        # final evidence, check fence, closer, and write fence(6)。
        # successは終端直前にdefault repo/refを各headで再確認する。
        self.assertEqual(app_reads, 6_000)
        self.assertEqual(segment_reads, [2_250, 2_250, 1_500])
        self.assertLessEqual(max(segment_reads), 2_250)
        self.assertEqual(segment_writes, [150, 150, 100])
        # full snapshot/manifest rereadを100 REST、verifierを5 REST/headと
        # 保守的に加算しても、installation共有limitのrolling window内に収まる。
        segment_seconds = 150 * WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS
        segment_rest_and_writes = 600 + 200 + (6 * 150) + 2_250 + 150
        self.assertEqual(segment_seconds, 3_075)
        self.assertLess(segment_seconds, 3_600)
        self.assertLessEqual(segment_rest_and_writes, 4_500)
        self.assertLessEqual(segment_rest_and_writes * 4, 4_500 * 4)
        self.assertLess(4 * segment_seconds, 6 * 3_600)

    def test_initial_and_final_closer_reads_use_the_app_token(self) -> None:
        current = self.pull(72)
        current["base"]["repo"]["default_branch"] = "master"  # type: ignore[index]
        calls: list[bool] = []
        def api(_endpoint: str, *, default_token: bool = False):
            calls.append(default_token)
            return current
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "b" * 40}), patch.object(WRITER, "api_json", side_effect=api):
            self.assertEqual(WRITER.pull(72)["number"], 72)
            self.assertTrue(WRITER.final_closer_is_unique(
                72, "64", "b" * 40, "a" * 40, WRITER.pr_body_sha256("Fixes #64"),
                {"64": frozenset({72})},
            ))
        self.assertEqual(calls, [False, False])

    def test_check_baseline_uses_the_app_read_path_before_default_fence(self) -> None:
        value = {"id": 12, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_baseline("a" * 40), WRITER.check_fingerprint(value))

    def test_evidence_snapshot_uses_exact_head_queries_with_fixed_page_budget(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 151)))
        calls: list[tuple[str, bool]] = []

        def page(endpoint: str, *, default_token: bool = False):
            calls.append((endpoint, default_token))
            return {"total_count": 0, "workflow_runs": []}

        def api(endpoint: str, *, default_token: bool = False):
            self.assertTrue(default_token)
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}

        with self.identity(), patch.object(WRITER, "object_page", side_effect=page), \
             patch.object(WRITER, "object_pages") as pages, patch.object(WRITER, "api_json", side_effect=api):
            evidence = WRITER.evidence_snapshot(snapshot, tuple(range(1, 151)))
        self.assertEqual(len(calls), 900)
        pages.assert_not_called()
        self.assertEqual(evidence.workflow_ids, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45})
        for endpoint, default_token in calls:
            query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
            self.assertEqual(query.get("head_sha"), [endpoint.split("head_sha=", 1)[1].split("&", 1)[0]])
            self.assertEqual(query.get("per_page"), ["100"])
            is_sensor = "pr-governance-review-events.yml/runs?" in endpoint
            if is_sensor:
                self.assertNotIn("event", query)
                self.assertTrue(default_token)
            else:
                self.assertEqual(query.get("event"), ["pull_request"])
                self.assertTrue(default_token)
        self.assertEqual(sum("pr-governance-review-events.yml/runs?" in endpoint for endpoint, _ in calls), 300)
        self.assertEqual(sum("/actions/workflows/44/runs?" in endpoint for endpoint, _ in calls), 300)
        self.assertEqual(sum("/actions/workflows/45/runs?" in endpoint for endpoint, _ in calls), 300)

    def test_terminal_workflow_enforces_conservative_3610_second_dispatch_barriers(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.assertLess(
            workflow.index("id: terminal-window-1"),
            workflow.index("id: dispatch-all-1"),
        )
        self.assertEqual(workflow.count("not_before = int(previous) + 3610"), 3)
        self.assertGreaterEqual(3_610, 3_610)
        self.assertEqual(workflow.count("for _ in range(125):"), 4)
        self.assertEqual(
            workflow.count("deadline = int(started) + 3750")
            + workflow.count("deadline=int(started)+3750"),
            12,
        )
        self.assertEqual(150 * WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS, 3_075)
        self.assertLessEqual(
            WRITER.TERMINAL_WRITER_STARTUP_RESERVE_SECONDS
            + WRITER.INITIAL_EVIDENCE_DEADLINE_SECONDS
            + WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS,
            WRITER.TERMINAL_AWAIT_STARTUP_AND_EVIDENCE_RESERVE_SECONDS,
        )
        self.assertLessEqual(
            300
            + 150 * WRITER.ALL_TERMINAL_CHECK_WRITE_INTERVAL_SECONDS
            + WRITER.TERMINAL_AWAIT_STARTUP_AND_EVIDENCE_RESERVE_SECONDS,
            125 * 30,
        )
        self.assertEqual(workflow.count("Terminal App REST rate-window barrier did not elapse."), 3)
        dispatch_names = (
            "Dispatch one repository-wide governance arbiter segment",
            "Dispatch second repository-wide governance arbiter segment",
            "Dispatch third repository-wide governance arbiter segment",
            "Dispatch fourth repository-wide governance arbiter segment",
        )
        dispatch_offsets = [workflow.index(f"      - name: {name}") for name in dispatch_names]
        dispatch_offsets.append(len(workflow))
        self.assertEqual(len(dispatch_offsets), 5)
        for index, (start, end) in enumerate(zip(dispatch_offsets, dispatch_offsets[1:]), start=1):
            segment = workflow[start:end]
            compact = "".join(segment.split())
            self.assertIn(
                f"TERMINAL_SEGMENT_STARTED_AT:${{{{steps.terminal-window-{index}.outputs.started_at}}}}",
                compact,
            )
            self.assertIn("defbounded_run(arguments,*args,**kwargs):", compact)
            self.assertIn('kwargs["timeout"]=min(20,remaining)', compact)
            self.assertIn("exceptsubprocess.TimeoutExpiredaserror:", compact)
            self.assertIn("iftime.time()>deadline:", compact)
            self.assertIn("subprocess.run=bounded_run", compact)
            self.assertIn("inputs[terminal_deadline_epoch]", compact)
            self.assertNotIn("--paginate", segment)
            self.assertNotIn("--slurp", segment)
            self.assertIn("pulls?state=open&per_page=100&page={page_number}", segment)
            self.assertIn('arguments.insert(2,"--include")', compact)
            self.assertIn("first page changed", segment)
            self.assertLess(
                compact.index("subprocess.run=bounded_run"),
                compact.index('subprocess.run(["gh","api"'),
            )
        for index in range(2, 5):
            previous = index - 1
            barrier = workflow.index(f"PREVIOUS_SEGMENT_STARTED_AT: ${{{{ steps.terminal-window-{previous}.outputs.started_at }}}}")
            record = workflow.index(f"id: terminal-window-{index}")
            dispatch = workflow.index(f"id: dispatch-all-{index}")
            self.assertLess(barrier, record)
            self.assertLess(record, dispatch)
        # Each recorded instant is rounded upward, so sleeping to its +3610s
        # bound cannot start the next dispatcher early because of truncation.
        self.assertEqual(workflow.count("started_at = math.ceil(time.time())"), 4)

    def test_each_terminal_registration_wrapper_bounds_and_fails_closed(self) -> None:
        """All four dispatch heredocs execute the same deadline contract."""
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        dispatch_names = (
            "Dispatch one repository-wide governance arbiter segment",
            "Dispatch second repository-wide governance arbiter segment",
            "Dispatch third repository-wide governance arbiter segment",
            "Dispatch fourth repository-wide governance arbiter segment",
        )
        offsets = [workflow.index(f"      - name: {name}") for name in dispatch_names]
        offsets.append(len(workflow))

        class Clock:
            value = 90.0

            @classmethod
            def time(cls) -> float:
                return cls.value

        for index, (start, end) in enumerate(zip(offsets, offsets[1:]), start=1):
            segment = workflow[start:end]
            definition_start = segment.index("          def bounded_run")
            definition_end = segment.index("\n          subprocess.run", definition_start)
            namespace: dict[str, object] = {
                "deadline": 100.0,
                "subprocess": subprocess,
                "time": Clock,
            }
            observed: list[float] = []

            def raw_run(*_args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
                timeout = kwargs.get("timeout")
                self.assertIsInstance(timeout, float)
                observed.append(timeout)
                return subprocess.CompletedProcess(["gh", "api"], 0, "{}", "")

            namespace["raw_run"] = raw_run
            exec(textwrap.dedent(segment[definition_start:definition_end]), namespace)
            bounded = namespace["bounded_run"]
            self.assertTrue(callable(bounded))
            with self.subTest(segment=index, case="bounded"):
                assert callable(bounded)
                self.assertEqual(bounded(["gh", "api", "repos/owner/repository"]).returncode, 0)
                self.assertEqual(observed, [10.0])
            with self.subTest(segment=index, case="post-call-deadline"):
                Clock.value = 90.0

                def crosses_deadline(*_args: object, **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    Clock.value = 101.0
                    return subprocess.CompletedProcess(["gh", "api"], 0, "{}", "")

                namespace["raw_run"] = crosses_deadline
                with self.assertRaisesRegex(SystemExit, "deadline elapsed"):
                    bounded(["gh", "api", "repos/owner/repository"])
                self.assertEqual(observed, [10.0])
            with self.subTest(segment=index, case="child-timeout"):
                Clock.value = 90.0

                def stalled(*_args: object, **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    raise subprocess.TimeoutExpired(["gh", "api"], 10)

                namespace["raw_run"] = stalled
                with self.assertRaisesRegex(SystemExit, "timed out"):
                    bounded(["gh", "api", "repos/owner/repository"])
        Clock.value = 90.0

    def test_each_terminal_await_bounds_its_child_call_and_deadline(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        await_names = tuple(
            f"Await {ordinal} repository-wide governance arbiter segment"
            for ordinal in ("first", "second", "third", "fourth")
        )
        offsets = [workflow.index(f"      - name: {name}") for name in await_names]
        offsets.append(len(workflow))
        for index, (start, end) in enumerate(zip(offsets, offsets[1:]), start=1):
            compact = "".join(workflow[start:end].split())
            with self.subTest(segment=index):
                self.assertIn("remaining=deadline-time.time()", compact)
                self.assertIn("timeout=min(20,remaining)", compact)
                self.assertIn("exceptsubprocess.TimeoutExpiredaserror:", compact)
                self.assertIn("iftime.time()>deadline:", compact)

    def test_writer_bootstrap_and_rebind_api_deadlines_fail_closed(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(
            encoding="utf-8"
        )
        bootstrap_start = workflow.index("      - name: Bind this dispatch to the repository default branch")
        rebind_start = workflow.index("      - name: Rebind trusted default branch before token creation")
        bootstrap = "".join(workflow[bootstrap_start:rebind_start].split())
        rebind = "".join(workflow[rebind_start:].split())
        for phase in (bootstrap, rebind):
            self.assertIn("deadline=time.monotonic()+120", phase)
            self.assertIn("timeout=min(20,remaining)", phase)
            self.assertIn("exceptsubprocess.TimeoutExpiredaserror:", phase)
            self.assertIn("iftime.monotonic()>deadline:", phase)
        self.assertEqual(bootstrap.count("request(f\"repos/"), 3)
        self.assertEqual(bootstrap.count("blob=request("), 1)
        self.assertEqual(rebind.count("=get(f\"repos/"), 4)

    def test_evidence_snapshot_keeps_old_current_head_runs_and_selects_latest_rerun(self) -> None:
        snapshot = self.snapshot((72,))
        old_sensor = self.generation(700, 1)
        old_sensor.update({"name": "PR governance review sensor", "path": ".github/workflows/pr-governance-review-events.yml@master"})
        latest_sensor = dict(old_sensor) | {"id": 701, "run_number": 9}
        old_ci = self.generation(900, 1)
        latest_ci = self.generation(901, 2)
        release = self.generation(902, 1)
        release.update({"name": "release-preflight", "path": ".github/workflows/release-preflight.yml@master", "workflow_id": 45})
        head = "0000000000000000000000000000000000000048"
        for run in (old_sensor, latest_sensor, old_ci, latest_ci, release):
            run["head_sha"] = head
            run["pull_requests"] = [dict(run["pull_requests"][0]) | {
                "head": {"sha": head, "repo": self.rest_repository()},
            }]

        def page(endpoint: str, *, default_token: bool = False):
            query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
            self.assertEqual(query.get("head_sha"), [head])
            if "pr-governance-review-events.yml/runs?" in endpoint:
                self.assertNotIn("event", query)
                return {"total_count": 2, "workflow_runs": [old_sensor, latest_sensor]}
            if "/actions/workflows/44/runs?" in endpoint:
                self.assertEqual(query.get("event"), ["pull_request"])
                return {"total_count": 2, "workflow_runs": [old_ci, latest_ci]}
            if "/actions/workflows/45/runs?" in endpoint:
                self.assertEqual(query.get("event"), ["pull_request"])
                return {"total_count": 1, "workflow_runs": [release]}
            self.fail(f"unexpected evidence endpoint: {endpoint}")

        def api(endpoint: str, *, default_token: bool = False):
            self.assertTrue(default_token)
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}

        with self.identity(), patch.object(WRITER, "object_page", side_effect=page), patch.object(WRITER, "api_json", side_effect=api):
            evidence = WRITER.evidence_snapshot(snapshot, (72,))
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, evidence), 701)
            self.assertEqual(
                WRITER.generation(72, "b" * 40, head, "CI", ".github/workflows/test-and-build.yml", evidence).identifier,
                901,
            )

    def test_evidence_snapshot_reuses_one_exact_head_query_for_two_open_prs(self) -> None:
        head = "a" * 40
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            (
                {"number": 72, "head_sha": head, "isDraft": False},
                {"number": 73, "head_sha": head, "isDraft": False},
            ),
        )

        def run(identifier: int, number: int, *, name: str, workflow_id: int | None = None) -> dict[str, object]:
            value = self.generation(identifier) | {
                "name": name, "path": (
                    ".github/workflows/pr-governance-review-events.yml@master"
                    if workflow_id is None else (
                        ".github/workflows/test-and-build.yml@master"
                        if workflow_id == 44 else ".github/workflows/release-preflight.yml@master"
                    )
                ),
                "head_sha": head,
                "pull_requests": [{
                    "number": number,
                    "base": {"sha": "b" * 40, "repo": self.rest_repository()},
                    "head": {"sha": head, "repo": self.rest_repository()},
                }],
            }
            if workflow_id is not None:
                value["workflow_id"] = workflow_id
            return value

        sensors = [run(700 + number, number, name="PR governance review sensor") for number in (72, 73)]
        ci = [run(900 + number, number, name="CI", workflow_id=44) for number in (72, 73)]
        release = [run(1_100 + number, number, name="release-preflight", workflow_id=45) for number in (72, 73)]
        calls: list[str] = []

        def page(endpoint: str, *, default_token: bool = False):
            calls.append(endpoint)
            if "pr-governance-review-events.yml/runs?" in endpoint:
                values = sensors
            elif "/actions/workflows/44/runs?" in endpoint:
                values = ci
            elif "/actions/workflows/45/runs?" in endpoint:
                values = release
            else:
                self.fail(f"unexpected evidence endpoint: {endpoint}")
            return {"total_count": len(values), "workflow_runs": values}

        def api(endpoint: str, *, default_token: bool = False):
            self.assertTrue(default_token)
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}

        with self.identity(), patch.object(WRITER, "object_page", side_effect=page), patch.object(WRITER, "api_json", side_effect=api):
            evidence = WRITER.evidence_snapshot(snapshot, (72, 73))
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, evidence), 772)
            self.assertEqual(WRITER.sensor(73, "b" * 40, head, evidence), 773)
            self.assertEqual(WRITER.generation(72, "b" * 40, head, "CI", ".github/workflows/test-and-build.yml", evidence).identifier, 972)
            self.assertEqual(WRITER.generation(73, "b" * 40, head, "CI", ".github/workflows/test-and-build.yml", evidence).identifier, 973)
        self.assertEqual(len(calls), 6)

    def test_evidence_snapshot_fails_closed_for_broad_or_incomplete_exact_head_page(self) -> None:
        snapshot = self.snapshot((72,))
        foreign = self.generation(900)
        foreign["head_sha"] = "f" * 40
        cases = (
            {"total_count": WRITER.MAX_EVIDENCE_RUNS_PER_QUERY + 1, "workflow_runs": []},
            {"total_count": 1, "workflow_runs": []},
            {"total_count": 1, "workflow_runs": [foreign]},
        )
        for page in cases:
            with self.subTest(page=page), self.identity(), \
                 patch.object(WRITER, "object_page", return_value=page), \
                 patch.object(WRITER, "api_json", return_value={"id": 44}):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.evidence_snapshot(snapshot, (72,))

    def test_bounded_exact_head_pages_accept_101_201_300_and_reject_301(self) -> None:
        head = "a" * 40
        endpoint = "repos/owner/repository/actions/workflows/pr-governance-review-events.yml/runs?head_sha=" + head + "&per_page=100"
        self.assertEqual(WRITER.MAX_EVIDENCE_RUNS_PER_QUERY, 300)
        for total in (101, 201, 300):
            runs = [{"id": index + 1, "head_sha": head} for index in range(total)]
            calls: list[str] = []

            def page(value: str) -> dict[str, object]:
                calls.append(value)
                query = WRITER.parse_qs(WRITER.urlparse(value).query)
                page_number = int(query.get("page", ["1"])[0])
                start = (page_number - 1) * WRITER.MAX_EVIDENCE_RUNS_PER_PAGE
                return {"total_count": total, "workflow_runs": runs[start:start + WRITER.MAX_EVIDENCE_RUNS_PER_PAGE]}

            with self.subTest(total=total), self.identity(), patch.object(WRITER, "object_page", side_effect=page):
                self.assertEqual(len(WRITER.bounded_head_runs(endpoint, head, "Review sensor")), total)
            self.assertEqual(len(calls), 1 + (total + WRITER.MAX_EVIDENCE_RUNS_PER_PAGE - 1) // WRITER.MAX_EVIDENCE_RUNS_PER_PAGE)

        with self.identity(), patch.object(
            WRITER, "object_page", return_value={"total_count": 301, "workflow_runs": []},
        ):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.bounded_head_runs(endpoint, head, "Review sensor")

    def test_bounded_exact_head_pages_reject_changed_or_duplicate_or_short_or_foreign_tail(self) -> None:
        head = "a" * 40
        endpoint = "repos/owner/repository/actions/workflows/pr-governance-review-events.yml/runs?head_sha=" + head + "&per_page=100"
        first = [{"id": index + 1, "head_sha": head} for index in range(100)]
        cases = (
            ("changed-total", {"total_count": 102, "workflow_runs": [{"id": 101, "head_sha": head}]}),
            ("duplicate-id", {"total_count": 101, "workflow_runs": [{"id": 1, "head_sha": head}]}),
            ("foreign-head", {"total_count": 101, "workflow_runs": [{"id": 101, "head_sha": "b" * 40}]}),
            ("short-intermediate", {"total_count": 201, "workflow_runs": [{"id": 101, "head_sha": head}]}),
        )
        for label, second in cases:
            with self.subTest(label=label), self.identity(), patch.object(
                WRITER, "object_page", side_effect=[{"total_count": second["total_count"], "workflow_runs": first}, second],
            ):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.bounded_head_runs(endpoint, head, "Review sensor")

    def test_bounded_exact_head_pages_reject_same_count_page_one_shift(self) -> None:
        """A same-count insert/delete during paging must not form a mixed snapshot."""
        head = "a" * 40
        endpoint = "repos/owner/repository/actions/runs?head_sha=" + head + "&per_page=100"
        initial_page_one = [{"id": index, "head_sha": head} for index in range(1, 101)]
        page_two = [{"id": 101, "head_sha": head}]
        shifted_page_one = [{"id": 102, "head_sha": head}, *initial_page_one[:-1]]
        with self.identity(), patch.object(WRITER, "object_page", side_effect=[
            {"total_count": 101, "workflow_runs": initial_page_one},
            {"total_count": 101, "workflow_runs": page_two},
            {"total_count": 101, "workflow_runs": shifted_page_one},
        ]):
            with self.assertRaisesRegex(WRITER.GovernanceError, "changed during pagination"):
                WRITER.bounded_head_runs(endpoint, head, "Final workflow-run")

    def test_initial_evidence_rejects_same_count_sensor_page_one_shift(self) -> None:
        snapshot = self.snapshot((72,))
        head = snapshot.pull_requests[0]["head_sha"]
        self.assertIsInstance(head, str)
        initial_page_one = [{"id": index, "head_sha": head} for index in range(1, 101)]
        with self.identity(), patch.object(WRITER, "object_page", side_effect=[
            {"total_count": 101, "workflow_runs": initial_page_one},
            {"total_count": 101, "workflow_runs": [{"id": 101, "head_sha": head}]},
            {"total_count": 101, "workflow_runs": [{"id": 102, "head_sha": head}, *initial_page_one[:-1]]},
        ]), patch.object(WRITER, "api_json") as api:
            with self.assertRaisesRegex(WRITER.GovernanceError, "changed during pagination"):
                WRITER.evidence_snapshot(snapshot, (72,))
        api.assert_not_called()

    def test_final_evidence_uses_one_complete_head_specific_repository_page(self) -> None:
        calls: list[str] = []
        def api(endpoint: str, *, default_token: bool = False):
            self.assertFalse(default_token)
            calls.append(endpoint)
            return {"total_count": 0, "workflow_runs": []}
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        with self.identity(), patch.object(WRITER, "api_json", side_effect=api):
            WRITER.final_evidence_for_pr("a" * 40, initial)
        self.assertEqual(calls, ["repos/owner/repository/actions/runs?head_sha=" + "a" * 40 + "&per_page=100"] * 2)

    def test_final_evidence_rejects_a_full_or_incomplete_head_page(self) -> None:
        older = [{"id": number, "head_sha": "a" * 40} for number in range(1, 101)]
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        for page in (
            {"total_count": WRITER.MAX_EVIDENCE_RUNS_PER_QUERY + 1, "workflow_runs": older},
            {"total_count": 98, "workflow_runs": older[:99]},
            {"total_count": 1, "workflow_runs": [{"id": 1, "head_sha": "b" * 40}]},
        ):
            with self.subTest(page=page), self.identity(), patch.object(WRITER, "api_json", return_value=page):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.final_evidence_for_pr("a" * 40, initial)

    def test_final_evidence_accepts_exactly_one_complete_page_of_100_runs(self) -> None:
        runs = [{"id": number, "head_sha": "a" * 40} for number in range(1, 101)]
        page = {"total_count": 100, "workflow_runs": runs}
        initial = WRITER.EvidenceSnapshot(
            {},
            {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45},
            {},
        )
        with self.identity(), patch.object(WRITER, "api_json", return_value=page):
            evidence = WRITER.final_evidence_for_pr("a" * 40, initial)
        self.assertEqual(len(evidence.workflow_runs[".github/workflows/test-and-build.yml"]), 0)

    def test_exact_head_pagination_keeps_latest_sensor_and_ci_generation_on_refresh(self) -> None:
        head = "a" * 40
        snapshot = WRITER.OpenSnapshot(
            (72,), {}, ({"number": 72, "head_sha": head, "isDraft": False, "body": "Fixes #64"},),
        )
        sensors: list[dict[str, object]] = []
        for index in range(101):
            run = self.generation(700 + index)
            run.update({
                "run_number": index + 1,
                "name": "PR governance review sensor",
                "path": ".github/workflows/pr-governance-review-events.yml@master",
                "event": "pull_request_review",
            })
            sensors.append(run)
        ci = self.generation(900)
        ci.update({"run_number": 1, "name": "CI", "path": ".github/workflows/test-and-build.yml@master"})
        release = self.generation(901)
        release.update({"run_number": 1, "name": "release-preflight", "path": ".github/workflows/release-preflight.yml@master", "workflow_id": 45})

        def initial_page(endpoint: str, *, default_token: bool = False) -> dict[str, object]:
            query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
            page = int(query.get("page", ["1"])[0])
            if "pr-governance-review-events.yml/runs?" in endpoint:
                self.assertTrue(default_token)
                values = sensors
            elif "/actions/workflows/44/runs?" in endpoint:
                self.assertTrue(default_token)
                values = [ci]
            elif "/actions/workflows/45/runs?" in endpoint:
                self.assertTrue(default_token)
                values = [release]
            else:
                self.fail(f"unexpected initial evidence endpoint: {endpoint}")
            return {"total_count": len(values), "workflow_runs": values[100 * (page - 1):100 * page]}

        def workflow_id(endpoint: str, *, default_token: bool = False) -> dict[str, int]:
            self.assertTrue(default_token)
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}

        with self.identity(), patch.object(WRITER, "object_page", side_effect=initial_page), patch.object(WRITER, "api_json", side_effect=workflow_id):
            initial = WRITER.evidence_snapshot(snapshot, (72,))
        with self.identity():
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, initial), 800)
            self.assertEqual(
                WRITER.generation(72, "b" * 40, head, "CI", ".github/workflows/test-and-build.yml", initial).identifier,
                900,
            )

        sensor_rerun = dict(sensors[-1])
        sensor_rerun.update({"id": 801, "run_number": 102})
        final_values = [*sensors, sensor_rerun, ci, release]

        def final_page(endpoint: str, *, default_token: bool = False) -> dict[str, object]:
            self.assertFalse(default_token)
            query = WRITER.parse_qs(WRITER.urlparse(endpoint).query)
            page = int(query.get("page", ["1"])[0])
            return {"total_count": len(final_values), "workflow_runs": final_values[100 * (page - 1):100 * page]}

        with self.identity(), patch.object(WRITER, "object_page", side_effect=final_page):
            refreshed = WRITER.final_evidence_for_pr(head, initial)
            # The page-2 rerun must invalidate the stale initial sensor fence.
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, refreshed), 801)
            self.assertNotEqual(WRITER.sensor(72, "b" * 40, head, refreshed), 800)
            self.assertEqual(
                WRITER.generation(72, "b" * 40, head, "CI", ".github/workflows/test-and-build.yml", refreshed).identifier,
                900,
            )

        malformed_tail = dict(sensors[-1])
        malformed_tail.update({"id": 802, "run_number": 102, "repository": self.rest_repository(202)})
        malformed_evidence = WRITER.EvidenceSnapshot(
            {"pull_request_review": tuple([*sensors, malformed_tail]), "pull_request": (), "pull_request_review_comment": ()},
            {}, {},
        )
        with self.identity():
            # A page-2 run with a foreign REST identity is not adopted as the
            # sensor generation; the prior trusted generation remains selected.
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, malformed_evidence), 800)

    def test_final_evidence_partitions_only_trusted_sensor_and_ci_workflows(self) -> None:
        sensor = self.generation(700)
        sensor.update({"name": "PR governance review sensor", "path": ".github/workflows/pr-governance-review-events.yml@master", "workflow_id": 43})
        ci = self.generation(900)
        release = self.generation(901)
        release.update({"name": "release-preflight", "path": ".github/workflows/release-preflight.yml@master", "workflow_id": 45})
        unrelated = {"id": 999, "head_sha": "a" * 40, "workflow_id": 999, "event": "push", "path": ".github/workflows/unrelated.yml@master"}
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        page = {"total_count": 4, "workflow_runs": [sensor, ci, release, unrelated]}
        with self.identity(), patch.object(WRITER, "api_json", return_value=page):
            evidence = WRITER.final_evidence_for_pr("a" * 40, initial)
        self.assertEqual(evidence.sensor_runs["pull_request"], (sensor,))
        self.assertEqual(evidence.workflow_runs[".github/workflows/test-and-build.yml"], (ci,))
        self.assertEqual(evidence.workflow_runs[".github/workflows/release-preflight.yml"], (release,))

    def test_final_closer_requires_current_default_branch_and_same_repository(self) -> None:
        current = self.pull(72)
        current["base"]["sha"] = "d" * 40  # type: ignore[index]
        current["base"]["repo"]["default_branch"] = "master"  # type: ignore[index]
        digest = WRITER.pr_body_sha256("Fixes #64")
        changes = (
            ("base ref", lambda value: value["base"].update({"ref": "release"})),
            ("default branch", lambda value: value["base"]["repo"].update({"default_branch": "release"})),
            ("base head", lambda value: value["base"].update({"sha": "e" * 40})),
            ("head repository", lambda value: value["head"].update({"repo": {"full_name": "fork/repository"}})),
        )
        for label, mutate in changes:
            changed = json.loads(json.dumps(current))
            mutate(changed)
            with self.subTest(label=label), self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "pull", return_value=changed):
                self.assertFalse(WRITER.final_closer_is_unique(72, "64", "d" * 40, "a" * 40, digest, {"64": frozenset({72})}))

    def test_final_head_pages_choose_the_101st_rerun_and_sensor(self) -> None:
        older_ci = self.generation(900, 1)
        newer_ci = self.generation(901, 2)
        sensor_template = self.generation(700, 1)
        sensor_template.update({"name": "PR governance review sensor", "path": ".github/workflows/pr-governance-review-events.yml@main", "event": "pull_request"})
        sensor_newer = dict(sensor_template); sensor_newer.update({"id": 701, "run_number": 9})
        cache = {
            (path, ref): "c" * 40
            for path in (".github/workflows/test-and-build.yml", ".github/workflows/pr-governance-review-events.yml")
            for ref in ("d" * 40, "b" * 40, "a" * 40)
        }
        evidence = WRITER.EvidenceSnapshot({"pull_request": tuple([sensor_template] * 100 + [sensor_newer]), "pull_request_review": (), "pull_request_review_comment": ()}, {".github/workflows/test-and-build.yml": 44}, {".github/workflows/test-and-build.yml": tuple([older_ci] * 100 + [newer_ci])}, cache)
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40}):
            selected = WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml", evidence)
            sensor = WRITER.sensor(72, "b" * 40, "a" * 40, evidence)
        self.assertEqual(selected.identifier, 901)
        self.assertEqual(sensor, 701)

    def test_open_pulls_rejects_duplicate_across_pages(self) -> None:
        first = [{"number": number, "state": "open"} for number in range(1, 101)]
        payload = [first, [{"number": 72, "state": "open"}]]
        def api(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return payload[page - 1] if page <= len(payload) else []
        with self.identity(), patch.object(WRITER, "api_json", side_effect=api):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_pulls()

    def test_open_pulls_rejects_malformed_or_nonopen_values(self) -> None:
        for payload in ({}, [{"number": "72", "state": "open"}], [{"number": 72, "state": "closed"}]):
            with self.subTest(payload=payload), self.identity(), patch.object(WRITER, "api_json", return_value=payload):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.open_pulls()

    def test_current_pull_rejects_foreign_base_or_head_repository(self) -> None:
        value = self.pull(72)
        value["head"]["repo"]["full_name"] = "foreign/repository"  # type: ignore[index]
        with self.identity(), patch.object(WRITER, "api_json", return_value=value):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.pull(72)

    def test_current_pull_rejects_a_nondefault_base_branch(self) -> None:
        value = self.pull(72); value["base"]["ref"] = "release/old"  # type: ignore[index]
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", return_value=value):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.pull(72)

    def test_pages_rejects_non_array_pages_and_non_object_items(self) -> None:
        for payload in ({}, ["bad"], [["bad"]]):
            with self.subTest(payload=payload), patch.object(WRITER, "api_json", return_value=payload):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.pages("ignored")

    def test_check_pages_reach_a_later_pending_fence(self) -> None:
        item = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        first = {"check_runs": [{"id": value} for value in range(1, 101)]}
        payload = [first, {"check_runs": [item]}]
        def api(endpoint: str, **_kwargs: object) -> object:
            page = int(WRITER.parse_qs(WRITER.urlparse(endpoint).query)["page"][0])
            return payload[page - 1] if page <= len(payload) else {"check_runs": []}
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "api_json", side_effect=api):
            self.assertEqual(WRITER.check_run("a" * 40), item)

    def test_check_run_ignores_foreign_and_historical_generations(self) -> None:
        for mutate in (lambda value: value["app"].update(id=7), lambda value: value.update(external_id="wrong")):
            value = {"id": 99, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
            mutate(value)
            with self.subTest(value=value), self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "object_pages", return_value=[{"check_runs": [value]}]):
                self.assertIsNone(WRITER.check_run("a" * 40))

    def test_check_fence_re_reads_same_id_and_evidence(self) -> None:
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}, "status": "completed", "conclusion": "success", "details_url": "https://github.test/run?source_run_id=77"}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_fence("a" * 40, (), 77), (True, 1, False))

    def test_check_fence_deduplicates_identical_in_progress_pending_evidence(self) -> None:
        details = "https://github.test/run"
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": details}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_fence("a" * 40, WRITER.check_fingerprint(value), 77, desired_state="pending", desired_target=details), (False, 0, True))

    def test_read_and_write_gh_boundaries_do_not_share_tokens(self) -> None:
        captured: list[dict[str, str]] = []
        class Result:
            returncode = 0
            stdout = "{}"
        def run(*_args, **kwargs):
            captured.append(kwargs["env"])
            return Result()
        previous = os.environ.get("CHECK_WRITE_TOKEN")
        previous_read = os.environ.get("GH_TOKEN")
        previous_default = os.environ.get("DEFAULT_READ_TOKEN")
        os.environ["CHECK_WRITE_TOKEN"] = "secret-check-token"; os.environ["GH_TOKEN"] = "read-token"; os.environ["DEFAULT_READ_TOKEN"] = "default-read-token"
        try:
            with patch.object(WRITER.subprocess, "run", side_effect=run):
                WRITER.command(["repos/owner/repository"])
                WRITER.command(["repos/owner/repository/pulls/72"], default_token=True)
                WRITER.command(["--method", "POST", "ignored"], check_write=True)
            self.assertEqual(captured[0], {"GH_TOKEN": "read-token", "PATH": os.environ["PATH"]})
            self.assertEqual(captured[1], {"GH_TOKEN": "default-read-token", "PATH": os.environ["PATH"]})
            self.assertEqual(captured[2], {"GH_TOKEN": "secret-check-token", "PATH": os.environ["PATH"]})
        finally:
            if previous is None: del os.environ["CHECK_WRITE_TOKEN"]
            else: os.environ["CHECK_WRITE_TOKEN"] = previous
            if previous_read is None: del os.environ["GH_TOKEN"]
            else: os.environ["GH_TOKEN"] = previous_read
            if previous_default is None: del os.environ["DEFAULT_READ_TOKEN"]
            else: os.environ["DEFAULT_READ_TOKEN"] = previous_default

    def test_command_bounds_each_gh_api_call_and_fails_closed_on_timeout(self) -> None:
        """A stalled gh child must not consume the terminal evidence reserve."""
        with patch.dict(os.environ, {"GH_TOKEN": "read-token"}, clear=False), \
             patch.object(WRITER.subprocess, "run", side_effect=subprocess.TimeoutExpired(["gh", "api"], 20)):
            with self.assertRaisesRegex(WRITER.GovernanceError, "timed out"):
                WRITER.command(["repos/owner/repository"])

    def test_command_caps_child_timeout_at_shared_evidence_deadline(self) -> None:
        captured: list[float] = []
        def run(*_args, **kwargs):
            captured.append(kwargs["timeout"])
            return subprocess.CompletedProcess(_args[0], 0, "{}", "")
        previous = WRITER._active_initial_evidence_deadline
        WRITER._active_initial_evidence_deadline = 103.0
        try:
            with patch.dict(os.environ, {"GH_TOKEN": "read-token"}, clear=False), \
                 patch.object(WRITER.time, "monotonic", return_value=100.0), \
                 patch.object(WRITER.subprocess, "run", side_effect=run):
                self.assertEqual(WRITER.command(["repos/owner/repository"]), "{}")
        finally:
            WRITER._active_initial_evidence_deadline = previous
        self.assertEqual(captured, [3.0])

    def test_initial_evidence_propagates_one_monotonic_deadline(self) -> None:
        snapshot = self.snapshot((1,))
        observed: list[float] = []
        def bounded(*_args, **kwargs):
            observed.append(WRITER._active_initial_evidence_deadline or -1.0)
            return ()
        def api(endpoint: str, *, default_token: bool = False):
            self.assertTrue(default_token)
            self.assertIn("actions/workflows/", endpoint)
            observed.append(WRITER._active_initial_evidence_deadline or -1.0)
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}
        with patch.object(WRITER.time, "monotonic", return_value=10.0), \
             patch.object(WRITER, "bounded_head_runs", side_effect=bounded), \
             patch.object(WRITER, "api_json", side_effect=api):
            WRITER.evidence_snapshot(snapshot, (1,))
        self.assertEqual(
            observed,
            [10.0 + WRITER.INITIAL_EVIDENCE_DEADLINE_SECONDS] * 5,
        )
        self.assertIsNone(WRITER._active_initial_evidence_deadline)

    def test_contract_verifiers_receive_only_the_read_token(self) -> None:
        class Result:
            returncode = 0
        captured: list[tuple[list[str], dict[str, str]]] = []
        previous = {key: os.environ.get(key) for key in ("GH_TOKEN", "CHECK_WRITE_TOKEN", "KRR_GOVERNANCE_APP_PRIVATE_KEY")}
        os.environ.update({"GH_TOKEN": "read-token", "CHECK_WRITE_TOKEN": "write-secret", "KRR_GOVERNANCE_APP_PRIVATE_KEY": "private-secret"})
        try:
            def run(*_args, **kwargs):
                captured.append((_args[0], kwargs["env"]))
                return Result()
            with self.identity(), patch.object(WRITER.subprocess, "run", side_effect=run):
                self.assertEqual(WRITER.contract(72, "b" * 40, "a" * 40, "branch", False, "/tmp/snapshot.json"), "success")
            self.assertEqual([value[1] for value in captured], [{"GH_TOKEN": "read-token", "PATH": os.environ["PATH"]}] * 2)
            self.assertNotIn("--open-pull-snapshot", captured[0][0])
            self.assertIn("--exclude-trusted-governance-check", captured[1][0])
            self.assertEqual(captured[1][0][-2:], ["--open-pull-snapshot", "/tmp/snapshot.json"])
        finally:
            for key, value in previous.items():
                if value is None: os.environ.pop(key, None)
                else: os.environ[key] = value

    def test_generation_selects_latest_same_head_attempt(self) -> None:
        first = self.generation(900, 1)
        second = self.generation(901, 2)
        second["path"] = ".github/workflows/test-and-build.yml@main"
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
             patch.object(WRITER, "object_page", return_value={"total_count": 2, "workflow_runs": [first, second]}):
            value = WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")
        self.assertEqual(value.identifier, 901)
        self.assertEqual(value.attempt, 2)

    def test_sensor_and_generation_accept_same_minimal_rest_repository_identity(self) -> None:
        sensor = self.with_rest_repository_identity(self.generation(700))
        sensor.update({
            "name": "PR governance review sensor",
            "path": ".github/workflows/pr-governance-review-events.yml@master",
        })
        ci = self.with_rest_repository_identity(self.generation(900))
        evidence = WRITER.EvidenceSnapshot(
            {"pull_request": (sensor,), "pull_request_review": (), "pull_request_review_comment": ()},
            {".github/workflows/test-and-build.yml": 44},
            {".github/workflows/test-and-build.yml": (ci,)},
        )
        with self.identity():
            self.assertEqual(WRITER.sensor(72, "b" * 40, "a" * 40, evidence), 700)
            self.assertEqual(
                WRITER.generation(
                    72, "b" * 40, "a" * 40, "CI",
                    ".github/workflows/test-and-build.yml", evidence,
                ).identifier,
                900,
            )

    def test_sensor_and_generation_reject_mixed_or_malformed_rest_repository_identity(self) -> None:
        for label, mutate in (
            ("top-id-bool", lambda run: run["repository"].update(id=True)),
            ("top-id-string", lambda run: run["repository"].update(id="101")),
            ("top-name", lambda run: run["repository"].update(name="foreign")),
            ("top-url", lambda run: run["repository"].update(url="https://api.github.com/repos/foreign/repository")),
            ("base-id", lambda run: run["pull_requests"][0]["base"]["repo"].update(id=202)),
            ("head-name", lambda run: run["pull_requests"][0]["head"]["repo"].update(name="foreign")),
            ("head-url", lambda run: run["pull_requests"][0]["head"]["repo"].update(url="https://api.github.com/repos/foreign/repository")),
        ):
            with self.subTest(label=label), self.identity():
                sensor = self.with_rest_repository_identity(self.generation(700))
                sensor.update({
                    "name": "PR governance review sensor",
                    "path": ".github/workflows/pr-governance-review-events.yml@master",
                })
                ci = self.with_rest_repository_identity(self.generation(900))
                mutate(sensor)
                mutate(ci)
                evidence = WRITER.EvidenceSnapshot(
                    {"pull_request": (sensor,), "pull_request_review": (), "pull_request_review_comment": ()},
                    {".github/workflows/test-and-build.yml": 44},
                    {".github/workflows/test-and-build.yml": (ci,)},
                )
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.sensor(72, "b" * 40, "a" * 40, evidence)
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.generation(
                        72, "b" * 40, "a" * 40, "CI",
                        ".github/workflows/test-and-build.yml", evidence,
                    )

    def test_generation_rejects_pr_modified_or_missing_workflow_blob(self) -> None:
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}, {"sha": "c" * 40}, {"sha": "d" * 40}]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", return_value={"sha": True}):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_generation_rejects_foreign_base_head_or_multiple_pr_binding(self) -> None:
        for mutate in (
            lambda run: run.update(repository={"full_name": "foreign/repository"}),
            lambda run: run["pull_requests"][0]["head"].update(sha="c" * 40),
            lambda run: run.update(pull_requests=[]),
        ):
            run = self.generation(); mutate(run)
            with self.subTest(run=run), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
                 patch.object(WRITER, "object_page", return_value={"total_count": 1, "workflow_runs": [run]}):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_generation_rejects_a_run_with_wrong_default_workflow_id(self) -> None:
        run = self.generation(); run["workflow_id"] = 45
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), \
             patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
             patch.object(WRITER, "object_page", return_value={"total_count": 1, "workflow_runs": [run]}):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_success_target_url_binds_workflow_and_generation_ids(self) -> None:
        values = (
            WRITER.Generation("CI", "x", 44, 101, 8, 2, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 102, 9, 1, "completed", "success"),
        )
        with self.identity():
            url = WRITER.target_url(
                source_run_id=77, generations=values, base="b" * 40, head="a" * 40,
                body_sha256="c" * 64,
            )
        self.assertIn("source_run_id=77", url)
        self.assertIn("ci_workflow_id=44", url)
        self.assertIn("ci_run_attempt=2", url)
        self.assertIn("ci_run_number=8", url)
        self.assertIn("ci_status=completed", url)
        self.assertIn("ci_conclusion=success", url)
        self.assertIn("release_workflow_id=45", url)
        self.assertIn("pr_base_sha=" + "b" * 40, url)
        self.assertIn("pr_head_sha=" + "a" * 40, url)
        self.assertIn("pr_body_sha256=" + "c" * 64, url)

    def test_body_digest_rejects_non_text_nul_and_invalid_utf8_scalars(self) -> None:
        self.assertEqual(
            WRITER.pr_body_sha256("Fixes #64"),
            "807aa69d375bfa66f74b64ac2143fa2c9511a011eb57ab8b4883f052d7ceb65f",
        )
        for value in (None, 64, "Fixes #64\0", "Fixes #64\ud800"):
            with self.subTest(value=repr(value)):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.pr_body_sha256(value)

    def test_success_evidence_requires_the_exact_body_digest(self) -> None:
        values = (
            WRITER.Generation("CI", "x", 44, 101, 8, 2, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 102, 9, 1, "completed", "success"),
        )
        with self.identity():
            desired = WRITER.target_url(
                source_run_id=77, generations=values, base="b" * 40, head="a" * 40,
                body_sha256="c" * 64,
            )
        self.assertTrue(WRITER._same_check_evidence(desired.replace("/99?", "/100?"), desired))
        self.assertFalse(WRITER._same_check_evidence(desired.replace("&pr_body_sha256=" + "c" * 64, ""), desired))
        self.assertFalse(WRITER._same_check_evidence(desired.replace("c" * 64, "d" * 64), desired))

    def test_verdict_handles_requested_success_and_terminal_failure(self) -> None:
        template = WRITER.Generation("CI", "p", 44, 1, 1, 1, "completed", "success")
        self.assertEqual(WRITER.verdict(template), "success")
        self.assertEqual(WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, "queued", None)), "pending")
        self.assertEqual(WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, "pending", None)), "pending")
        self.assertEqual(WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, "completed", "failure")), "failure")
        for status in ("", "unknown", "requested-but-invalid"):
            with self.subTest(status=status):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, status, None))

    def test_draft_process_stays_pending_without_sensor_or_ci(self) -> None:
        current = self.pull(72, draft=True)
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, \
             patch.object(WRITER, "contract", return_value="pending"), \
             patch.object(WRITER, "check_changed_since", return_value=False), \
             patch.object(WRITER, "sensor") as sensor, patch.object(WRITER, "generation") as generation:
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_count, 2)
        sensor.assert_not_called(); generation.assert_not_called()

    def test_bodyless_pr_defers_one_fail_closed_decision_to_the_budgeted_writer(self) -> None:
        current = self.pull(72)
        current["body"] = None
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "write_governance_check", return_value=(102,)) as post:
            decision = WRITER.process(72, {}, "/tmp/snapshot.json", defer_terminal=True)
        self.assertEqual(decision.state if decision is not None else None, "failure")
        self.assertEqual(decision.description if decision is not None else None, "Trusted PR governance failed closed.")
        post.assert_not_called()

    def test_deferred_success_carries_the_process_time_body_digest(self) -> None:
        current = self.pull(72)
        generations = (
            WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"),
        )
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), \
             patch.object(WRITER, "generation", side_effect=[*generations, *generations]), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True):
            decision = WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json", defer_terminal=True)
        self.assertEqual(decision.state if decision is not None else None, "success")
        self.assertEqual(
            decision.body_sha256 if decision is not None else None,
            WRITER.pr_body_sha256("Fixes #64"),
        )

    def test_finalize_refuses_success_when_the_pr_body_changes_after_the_decision(self) -> None:
        generations = (
            WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"),
        )
        decision = WRITER.PendingDecision(
            72, "a" * 40, "b" * 40, (101,), "success", "ok", 77, generations, "64",
            WRITER.pr_body_sha256("Fixes #64"),
        )
        with self.identity(), patch.object(WRITER, "final_closer_is_unique", return_value=False) as closer, \
             patch.object(WRITER, "sensor", return_value=77), \
             patch.object(WRITER, "generation", side_effect=generations), \
             patch.object(WRITER, "check_fence", return_value=(False, 0, False)), \
             patch.object(WRITER, "rebind_trusted_default_writer"), \
            patch.object(WRITER, "write_governance_check", return_value=(102,)) as post:
            self.assertTrue(WRITER.finalize_decision(decision, {"64": frozenset({72})}, WRITER.EvidenceSnapshot({}, {}, {})))
        self.assertEqual(post.call_args.args[1], "failure")
        closer.assert_called_once()

    def test_finalize_success_rebinds_default_ref_after_late_closer_before_terminal_patch(self) -> None:
        """A default-ref advance after the closer fence must suppress success PATCH."""
        default_head = "d" * 40
        advanced_head = "e" * 40
        head = "a" * 40
        base = {
            "sha": default_head, "ref": "master",
            "repo": {"full_name": "owner/repository", "default_branch": "master"},
        }
        pull_request = {
            "number": 72, "state": "open", "draft": False, "body": "Fixes #64",
            "base": base, "head": {"sha": head, "repo": {"full_name": "owner/repository"}},
        }
        common_run = {
            "run_number": 8, "run_attempt": 1, "event": "pull_request", "head_sha": head,
            "status": "completed", "conclusion": "success", "repository": self.rest_repository(),
            "pull_requests": [{
                "number": 72,
                "base": {**base, "repo": self.rest_repository()},
                "head": {**pull_request["head"], "repo": self.rest_repository()},
            }],
        }
        sensor_run = {
            **common_run, "id": 77, "name": "PR governance review sensor",
            "path": ".github/workflows/pr-governance-review-events.yml@master",
        }
        ci_run = {
            **common_run, "id": 900, "name": "CI", "workflow_id": 44,
            "path": ".github/workflows/test-and-build.yml@master",
        }
        release_run = {
            **common_run, "id": 901, "name": "release-preflight", "workflow_id": 45,
            "path": ".github/workflows/release-preflight.yml@master",
        }
        evidence = WRITER.EvidenceSnapshot(
            {"pull_request": (sensor_run,), "pull_request_review": (), "pull_request_review_comment": ()},
            {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45},
            {".github/workflows/test-and-build.yml": (ci_run,), ".github/workflows/release-preflight.yml": (release_run,)},
        )
        generations = (
            WRITER.Generation("CI", ".github/workflows/test-and-build.yml", 44, 900, 8, 1, "completed", "success"),
            WRITER.Generation("release-preflight", ".github/workflows/release-preflight.yml", 45, 901, 8, 1, "completed", "success"),
        )
        check = {
            "id": 101, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": f"krr-governance/v1/{head}/dispatcher-88", "updated_at": "initial",
            "app": {"id": 42}, "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/88",
        }
        decision = WRITER.PendingDecision(
            72, head, default_head, (
                101, "initial", "in_progress", None,
                "https://github.com/owner/repository/actions/runs/88", f"krr-governance/v1/{head}/dispatcher-88",
            ), "success", "fixture", 77,
            generations, "64", WRITER.pr_body_sha256("Fixes #64"),
        )
        dispatcher = self.dispatcher_run(88, status="completed", conclusion="success")
        writer = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": default_head,
            "repository": self.rest_repository(), "run_attempt": 1, "status": "in_progress",
        }
        calls: list[str] = []
        terminal_patches: list[list[str]] = []

        def run(arguments, **kwargs):
            api_arguments = arguments[2:]
            endpoint = next(
                (item for item in api_arguments if isinstance(item, str) and item.startswith("repos/")), None
            )
            self.assertIsNotNone(endpoint)
            assert endpoint is not None
            environment = kwargs.get("env")
            if "--method" in api_arguments:
                self.assertEqual(environment, {"GH_TOKEN": "check-write", "PATH": os.environ["PATH"]})
                self.assertEqual(api_arguments[api_arguments.index("--method") + 1], "PATCH")
                self.assertEqual(endpoint, "repos/owner/repository/check-runs/101")
                terminal_patches.append(api_arguments)
                details = next(item.split("=", 1)[1] for item in api_arguments if item.startswith("details_url="))
                check.update({"updated_at": "terminal", "status": "completed", "conclusion": "success", "details_url": details})
                return subprocess.CompletedProcess(arguments, 0, json.dumps(check), "")
            self.assertEqual(environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
            if endpoint == "repos/owner/repository/pulls/72":
                calls.append("closer")
                payload: object = pull_request
            elif endpoint == "repos/owner/repository":
                calls.append("default-branch")
                payload = {"default_branch": "master"}
            elif endpoint == "repos/owner/repository/git/ref/heads/master":
                calls.append("default-ref")
                payload = {"object": {"sha": advanced_head}}
            elif endpoint == "repos/owner/repository/actions/runs/99":
                calls.append("writer")
                payload = writer
            elif endpoint == "repos/owner/repository/actions/runs/88":
                calls.append("dispatcher")
                payload = dispatcher
            elif endpoint.startswith("repos/owner/repository/actions/workflows/66/runs?"):
                calls.append("dispatcher-page")
                payload = self.dispatcher_page(dispatcher)
            elif endpoint.startswith("repos/owner/repository/check-runs/"):
                calls.append("check-by-id")
                payload = check
            elif endpoint.startswith("repos/owner/repository/commits/") and "/check-runs?" in endpoint:
                calls.append("check-page")
                payload = {"check_runs": [check]}
            else:
                self.fail(f"unexpected late-closer race transport: {arguments}")
            return subprocess.CompletedProcess(arguments, 0, json.dumps(payload), "")

        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": default_head, "GITHUB_REF_NAME": "master",
            "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "88",
            "GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read", "CHECK_WRITE_TOKEN": "check-write",
            "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }
        WRITER._bound_check_runs.clear()
        self.addCleanup(WRITER._bound_check_runs.clear)
        failed_closed = False
        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER, "pace_check_write"), \
             patch.object(WRITER.subprocess, "run", side_effect=run):
            try:
                WRITER.finalize_decision(decision, {"64": frozenset({72})}, evidence)
            except WRITER.GovernanceError:
                failed_closed = True
        self.assertEqual((failed_closed, terminal_patches), (True, []))
        self.assertLess(calls.index("closer"), calls.index("default-ref"))

    def test_deferred_draft_reuses_the_dispatcher_pending_check_without_a_write(self) -> None:
        current = self.pull(72, draft=True)
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "contract", return_value="pending"), \
             patch.object(WRITER, "write_governance_check") as post:
            self.assertIsNone(WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json", defer_terminal=True))
        post.assert_not_called()

    def test_same_head_rerun_before_final_success_returns_pending(self) -> None:
        current = self.pull(72)
        previous = (WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"))
        latest = (WRITER.Generation("CI", "x", 44, 3, 1, 2, "completed", "success"), previous[1])
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=[*previous, *latest]), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False):
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_args_list[-1].args[1], "pending")

    def test_later_pending_fence_prevents_terminal_success_post(self) -> None:
        current = self.pull(72)
        generations = [WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")]
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=generations * 2), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=True):
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_count, 1)

    def test_sensor_terminal_binding_is_unique_and_ambiguous_history_posts_nothing(self) -> None:
        current = self.pull(72)
        generations = [WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")]
        for terminal_count, expected_posts in ((0, 2), (1, 2), (2, 1)):
            with self.subTest(terminal_count=terminal_count), self.identity(), patch.object(WRITER, "pull", return_value=current), \
                 patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
                 patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=generations * 2), \
                 patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False), \
                 patch.object(WRITER, "sensor_terminal_check_count", return_value=terminal_count):
                if terminal_count == 2:
                    with self.assertRaises(WRITER.NoPostGovernanceError):
                        WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
                else:
                    WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
            self.assertEqual(post.call_count, expected_posts)

    def test_terminal_write_refuses_a_later_invalidator_fingerprint(self) -> None:
        head = "a" * 40
        baseline = {
            "id": 102, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": WRITER.check_external_id(head), "updated_at": "one",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/88", "app": {"id": 42},
        }
        later = {**baseline, "updated_at": "two", "details_url": "https://github.com/owner/repository/actions/runs/89"}
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "check_run", side_effect=[baseline, later]), \
             patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.write_governance_check(
                    head, "success", "old decision", "https://github.com/owner/repository/actions/runs/88",
                    expected_fingerprint=WRITER.check_fingerprint(baseline),
                )
        command.assert_not_called()

    def test_missing_terminal_checks_fail_closed_when_the_reservation_is_exhausted(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (), "success", "ok", 77, (), "64", "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_INVALIDATED_COUNT": "spoofed"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=True) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(finalize.call_count, 200)

    def test_local_event_writer_fails_closed_after_its_terminal_reservation(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (102, "pending"), "success", "ok", 77, (), "64", "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_INVALIDATED_COUNT": "not-a-number"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=True) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(finalize.call_count, 100)

    def test_exceptional_terminal_decisions_cannot_exceed_their_reserved_write_budget(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (), "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", side_effect=WRITER.GovernanceError("failed closed")) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(finalize.call_count, 200)

    def test_production_first_check_write_waits_before_timestamping(self) -> None:
        WRITER._last_check_write_at = None
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "early"}), \
             patch.object(WRITER.time, "monotonic", return_value=108.1) as monotonic, \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.pace_check_write()
        sleep.assert_called_once_with(8.1)
        monotonic.assert_called_once_with()
        self.assertEqual(WRITER._last_check_write_at, 108.1)

    def test_production_check_writes_are_monotonically_paced(self) -> None:
        WRITER._last_check_write_at = 100.0
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "early"}), \
             patch.object(WRITER.time, "monotonic", side_effect=[100.0, 108.1, 108.1, 116.2]), \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.pace_check_write()
            WRITER.pace_check_write()
        self.assertEqual(sleep.call_args_list, [unittest.mock.call(8.1)] * 2)
        self.assertEqual(WRITER._last_check_write_at, 116.2)

    def test_production_all_segment_check_writes_use_the_shared_installation_interval(self) -> None:
        WRITER._last_check_write_at = None
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all"}), \
             patch.object(WRITER.time, "monotonic", return_value=116.2), \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.pace_check_write()
        sleep.assert_called_once_with(20.5)


if __name__ == "__main__":
    unittest.main()
