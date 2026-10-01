from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[2]


class GovernanceAdminSerializationTest(unittest.TestCase):
    def setUp(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.jobs = dict(re.findall(
            r"(?ms)^  ([A-Za-z0-9_-]+):\n(.*?)(?=^  [A-Za-z0-9_-]+:|\Z)",
            workflow.split("\njobs:\n", 1)[1],
        ))

    def test_every_protection_writer_uses_one_non_preemptive_admin_queue(self) -> None:
        writers = {
            name: job for name, job in self.jobs.items()
            if "permission-administration: write" in job
        }
        self.assertGreaterEqual(len(writers), 3)
        groups = set()
        for name, job in writers.items():
            with self.subTest(job=name):
                match = re.search(r"(?m)^    concurrency:\n((?:      .*\n)+)", job)
                self.assertIsNotNone(match, "protection mutation must own the admin lock")
                if match is None:
                    continue
                policy = match.group(1)
                group = re.search(r"(?m)^      group: (.+)$", policy)
                self.assertIsNotNone(group)
                if group is not None:
                    groups.add(group.group(1))
                self.assertIn("      queue: max\n", policy)
                self.assertIn("      cancel-in-progress: false\n", policy)
                self.assertNotIn("github.run_id", policy)
                self.assertNotIn("github.event.workflow_run.id", policy)
        self.assertEqual(len(groups), 1, "arm and release must share the same lock")

    def test_long_sensor_and_writer_waits_never_hold_the_admin_lock(self) -> None:
        for name, job in self.jobs.items():
            if "permission-administration: write" not in job:
                continue
            with self.subTest(job=name):
                self.assertNotIn("Await the captured review sensor terminal state", job)
                self.assertNotIn("Await the bound early event writer", job)
                self.assertNotIn("Dispatch one repository-wide governance arbiter segment", job)
                limit = re.search(r"(?m)^    timeout-minutes: ([0-9]+)$", job)
                self.assertIsNotNone(limit)
                if limit is not None:
                    self.assertLessEqual(int(limit.group(1)), 15)

    def test_step_expressions_only_reference_steps_in_the_same_job(self) -> None:
        for name, job in self.jobs.items():
            with self.subTest(job=name):
                step_body = job.split("\n    steps:\n", 1)[1]
                identifiers = set(re.findall(r"(?m)^(?:      |        )id: ([A-Za-z0-9_-]+)$", step_body))
                expressions = re.findall(r"\$\{\{(.*?)\}\}", job, re.DOTALL)
                expressions.extend(re.findall(r"(?m)^(?:      |        )if: (.+)$", step_body))
                references = set(re.findall(
                    r"\bsteps\.([A-Za-z0-9_-]+)\.", "\n".join(expressions),
                ))
                self.assertEqual(references - identifiers, set(),
                                 "job split left references to another job's steps")


if __name__ == "__main__":
    unittest.main()
