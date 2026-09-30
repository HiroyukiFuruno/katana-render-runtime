"""Execute the workflow waiter against deterministic GitHub response sequences."""
import copy
import json
import os
from pathlib import Path
import re
import subprocess
import textwrap
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
REPO = {"id": 7, "full_name": "owner/repo", "default_branch": "master"}


def sensor(identifier, *, branch="master", fork=False, status="completed", conclusion="success"):
    head_repo = {"id": 8, "full_name": "fork/repo"} if fork else REPO
    pull = {"number": identifier, "state": "open", "draft": False,
            "base": {"ref": branch, "repo": REPO},
            "head": {"sha": "a" * 40, "repo": head_repo}}
    return {"id": identifier, "name": "PR governance review sensor",
            "event": "pull_request_review", "run_attempt": 1,
            "path": ".github/workflows/pr-governance-review-events.yml",
            "status": status, "conclusion": conclusion,
            "repository": REPO, "head_repository": head_repo,
            "head_sha": "a" * 40, "pull_requests": [pull]}


class SensorWaitExecutionTest(unittest.TestCase):
    def execute_waiter(self, runs, listings):
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text()
        job = workflow.split("  wait-for-active-review-sensor:", 1)[1].split(
            "  establish-resolver-failure-barrier:", 1)[0]
        program = textwrap.dedent(re.search(
            r"python3 - <<'PY'\n(.*?)\n          PY", job, re.S).group(1))
        reads = []
        queue_reads = 0
        def response(arguments, **kwargs):
            nonlocal queue_reads
            endpoint = arguments[-1]
            reads.append(endpoint)
            if len(reads) > 100:
                raise AssertionError("Waiter failed to converge")
            if endpoint == "repos/owner/repo":
                value = REPO
            elif "/actions/workflows/" in endpoint:
                if "status=queued" in endpoint:
                    value = listings[min(queue_reads, len(listings)-1)]
                    queue_reads += 1
                else:
                    value = []
                value = {"total_count": len(value), "workflow_runs": value}
            elif "/actions/runs/" in endpoint:
                value = runs[int(endpoint.rsplit("/", 1)[1])]
            elif "/pulls/" in endpoint:
                value = runs[int(endpoint.rsplit("/", 1)[1])]["pull_requests"][0]
            else:
                raise AssertionError(endpoint)
            return subprocess.CompletedProcess(arguments, 0, json.dumps(value), "")
        with patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repo", "SENSOR_RUN_IDS": "[1]"}), \
             patch("subprocess.run", side_effect=response), patch("time.sleep") as sleep:
            exec(compile(program, "sensor-wait-workflow", "exec"), {"__name__": "__main__"})
            self.assertLess(sleep.call_count, 5)
        return reads

    def test_success_rescans_and_waits_for_new_sensor(self):
        runs = {1: sensor(1), 2: sensor(2)}
        reads = self.execute_waiter(runs, [[sensor(2, status="queued", conclusion=None)], []])
        self.assertIn("repos/owner/repo/actions/runs/2", reads)
        self.assertEqual(sum("status=queued" in path for path in reads), 2)

    def test_repeated_arrivals_are_drained_before_success(self):
        runs = {identifier: sensor(identifier) for identifier in (1, 2, 3)}
        reads = self.execute_waiter(runs, [
            [sensor(2, status="queued", conclusion=None)],
            [sensor(3, status="queued", conclusion=None)], [],
        ])
        self.assertIn("repos/owner/repo/actions/runs/3", reads)
        self.assertEqual(sum("status=queued" in path for path in reads), 3)

    def test_cancelled_sensor_skips_verified_fork_and_other_branch(self):
        runs = {1: sensor(1, conclusion="cancelled"), 2: sensor(2),
                3: sensor(3, fork=True), 4: sensor(4, branch="feature")}
        active = [sensor(2, status="queued", conclusion=None),
                  sensor(3, fork=True, status="queued", conclusion=None),
                  sensor(4, branch="feature", status="queued", conclusion=None)]
        reads = self.execute_waiter(runs, [active, [], []])
        self.assertIn("repos/owner/repo/actions/runs/2", reads)
        self.assertNotIn("repos/owner/repo/actions/runs/3", reads)
        self.assertNotIn("repos/owner/repo/actions/runs/4", reads)

    def test_malformed_foreign_identity_is_not_skipped(self):
        broken = sensor(3, fork=True, status="queued", conclusion=None)
        broken["head_repository"] = copy.deepcopy(REPO)
        with self.assertRaisesRegex(SystemExit, "changed identity"):
            self.execute_waiter({1: sensor(1), 3: sensor(3, fork=True)}, [[broken]])

    def test_captured_sensor_moving_out_of_scope_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "changed identity"):
            self.execute_waiter({1: sensor(1, fork=True)}, [[]])


if __name__ == "__main__":
    unittest.main()
