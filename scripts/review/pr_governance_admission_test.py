from __future__ import annotations

import importlib.util
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]
SPEC = importlib.util.spec_from_file_location(
    "pr_governance_status_writer_admission",
    ROOT / "scripts/review/pr_governance_status_writer.py",
)
assert SPEC is not None and SPEC.loader is not None
WRITER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WRITER
SPEC.loader.exec_module(WRITER)


class AdmissionLaneSimulationTest(unittest.TestCase):
    def setUp(self) -> None:
        WRITER._nonreconciling_dispatcher_generations.clear()
        WRITER._dispatcher_admission_evidence_wait_remaining = None
        self.addCleanup(WRITER._nonreconciling_dispatcher_generations.clear)
        self.addCleanup(setattr, WRITER, "_dispatcher_admission_evidence_wait_remaining", None)

    @staticmethod
    def dispatcher_run(
        identifier: int, *, event: str = "workflow_run", status: str = "in_progress",
        conclusion: object = None, created_at: str,
    ) -> dict[str, object]:
        return {
            "id": identifier,
            "name": WRITER.DISPATCHER_NAME,
            "path": ".github/workflows/pr-governance.yml@master",
            "event": event,
            "head_sha": "d" * 40,
            "repository": {
                "id": 101,
                "name": "repository",
                "url": "https://api.github.com/repos/owner/repository",
            },
            "head_branch": "master",
            "workflow_id": 66,
            "run_number": identifier,
            "run_attempt": 1,
            "status": status,
            "conclusion": conclusion,
            "created_at": created_at,
        }

    @staticmethod
    def dispatcher_page(*runs: dict[str, object]) -> dict[str, object]:
        return {"total_count": len(runs), "workflow_runs": list(runs)}

    @staticmethod
    def jobs(
        *, sensor_marker: str, await_early: str | None = None,
        barrier: str = "success",
    ) -> dict[str, object]:
        def complete_step(number: int, name: str, conclusion: str) -> dict[str, object]:
            return {
                "number": number,
                "name": name,
                "status": "completed",
                "conclusion": conclusion,
            }

        preflight = {
            "id": 1,
            "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME,
            "status": "completed",
            "conclusion": "success",
            "steps": [
                complete_step(
                    1, WRITER.REVIEW_SENSOR_STAGED_ADMISSION_STEP_NAME,
                    sensor_marker,
                ),
            ],
        }
        values: list[dict[str, object]] = [
            preflight,
            {
                "id": 2,
                "name": WRITER.RESOLVER_FAILURE_BARRIER_NAME,
                "status": "completed",
                "conclusion": barrier,
            },
        ]
        if await_early is not None:
            await_status = "completed" if await_early in {"success", "failure", "cancelled"} else "in_progress"
            values.append({
                "id": 3,
                "name": WRITER.RECONCILE_ALL_OPEN_NAME,
                "status": await_status,
                "conclusion": await_early if await_status == "completed" else None,
                "steps": [{
                    "number": 1,
                    "name": WRITER.AWAIT_EARLY_WRITER_STEP_NAME,
                    "status": await_status,
                    "conclusion": await_early if await_status == "completed" else None,
                }],
            })
        return {"total_count": len(values), "jobs": values}

    def fence_environment(self) -> dict[str, str]:
        return {
            "GITHUB_ACTIONS": "true",
            "GITHUB_SHA": "d" * 40,
            "GITHUB_REF_NAME": "master",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88",
            "GOVERNANCE_SCOPE": "early",
            "KRR_GOVERNANCE_CHECK_APP_ID": "42",
        }

    def test_queued_sensor_is_reobserved_then_allows_old_early_until_its_await_succeeds(self) -> None:
        old = self.dispatcher_run(88, event="issues", created_at="2026-09-30T00:00:00Z")
        queued = self.dispatcher_run(7, status="queued", created_at="2026-09-30T00:01:00Z")
        staged = self.dispatcher_run(7, created_at="2026-09-30T00:01:00Z")
        # 通常のfenced writeが60秒以上前にあっても、最初のunknown candidate
        # を観測する有界budgetを消費してはならない。
        with patch.multiple(
            WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com",
            WRITER_RUN_ID="99",
        ), patch.dict(os.environ, self.fence_environment()), \
             patch.object(WRITER, "api_json", return_value=old), \
             patch.object(WRITER, "object_page", return_value=self.dispatcher_page(old)):
            WRITER.reject_newer_dispatcher_barrier("a" * 40)
        self.assertIsNone(WRITER._dispatcher_admission_evidence_wait_remaining)
        pages = (
            self.dispatcher_page(old, queued),
            {"total_count": 0, "jobs": []},
            self.dispatcher_page(old, staged),
            self.jobs(sensor_marker="success", await_early="in_progress"),
        )
        with patch.multiple(
            WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com",
            WRITER_RUN_ID="99",
        ), patch.dict(os.environ, self.fence_environment()), \
             patch.object(WRITER, "api_json", return_value=old), \
             patch.object(WRITER, "object_page", side_effect=pages), \
             patch.object(WRITER.time, "monotonic", return_value=120), \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.reject_newer_dispatcher_barrier("a" * 40)
        sleep.assert_called_once_with(WRITER.ADMISSION_EVIDENCE_RETRY_SECONDS)

        with patch.multiple(
            WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com",
            WRITER_RUN_ID="99",
        ), patch.dict(os.environ, self.fence_environment()), \
             patch.object(WRITER, "api_json", return_value=old), \
             patch.object(
                 WRITER, "object_page",
                 side_effect=(
                     self.dispatcher_page(old, staged),
                     self.jobs(sensor_marker="success", await_early="success"),
                 ),
             ):
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.reject_newer_dispatcher_barrier("a" * 40)

    def test_unclassified_ci_and_failed_sensor_remain_fail_closed(self) -> None:
        old = self.dispatcher_run(88, event="issues", created_at="2026-09-30T00:00:00Z")
        ci = self.dispatcher_run(7, created_at="2026-09-30T00:01:00Z")
        failed_sensor = self.dispatcher_run(
            7, status="completed", conclusion="failure",
            created_at="2026-09-30T00:01:00Z",
        )
        for candidate, jobs in (
            (ci, self.jobs(sensor_marker="skipped", barrier="success")),
            (failed_sensor, self.jobs(sensor_marker="success", await_early="in_progress")),
            (ci, self.jobs(sensor_marker="success", await_early="failure")),
            (ci, self.jobs(sensor_marker="success", await_early="cancelled")),
            (ci, {"total_count": 1, "jobs": [{"id": 1, "name": WRITER.PREFLIGHT_WORKFLOW_RUN_SOURCE_NAME, "status": "completed", "conclusion": "success"}]}),
        ):
            WRITER._dispatcher_admission_evidence_wait_remaining = None
            with self.subTest(candidate=candidate["status"], evidence=jobs["total_count"]), \
                 patch.multiple(
                     WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com",
                     WRITER_RUN_ID="99",
                 ), patch.dict(os.environ, self.fence_environment()), \
                 patch.object(WRITER, "api_json", return_value=old), \
                 patch.object(
                     WRITER, "object_page",
                     side_effect=(self.dispatcher_page(old, candidate), jobs),
                 ):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.reject_newer_dispatcher_barrier("a" * 40)

    def test_newer_same_pr_sensor_replaces_the_old_source_evidence(self) -> None:
        repository = {
            "id": 101,
            "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
        }
        head = "a" * 40

        def sensor_run(identifier: int, run_number: int) -> dict[str, object]:
            return {
                "id": identifier,
                "run_number": run_number,
                "run_attempt": 1,
                "name": "PR governance review sensor",
                "event": "pull_request_review",
                "path": ".github/workflows/pr-governance-review-events.yml@master",
                "head_sha": head,
                "repository": dict(repository),
                "pull_requests": [{
                    "number": 72,
                    "base": {"sha": "b" * 40, "repo": dict(repository)},
                    "head": {"sha": head, "repo": dict(repository)},
                }],
            }

        old_sensor = sensor_run(700, 1)
        newest_sensor = sensor_run(701, 2)
        evidence = WRITER.EvidenceSnapshot(
            {
                "pull_request": (),
                "pull_request_review": (old_sensor, newest_sensor),
                "pull_request_review_comment": (),
            },
            {},
            {},
        )
        with patch.multiple(WRITER, REPOSITORY="owner/repository"):
            self.assertEqual(WRITER.sensor(72, "b" * 40, head, evidence), 701)

    def test_default_workflow_orders_all_open_after_the_bound_early_await(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        await_early = workflow.index("Await the bound early event writer before all-open invalidation")
        all_open = workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertLess(await_early, all_open)


if __name__ == "__main__":
    unittest.main()
