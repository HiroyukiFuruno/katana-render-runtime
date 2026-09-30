"""Execute the workflow waiter against deterministic GitHub response sequences."""
import ast
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
REPO = {"id": 7, "name": "repo", "url": "https://api.github.com/repos/owner/repo", "full_name": "owner/repo", "default_branch": "master"}


def sensor(identifier, *, branch="master", fork=False, status="completed", conclusion="success"):
    head_repo = {"id": 8, "name": "repo", "url": "https://api.github.com/repos/fork/repo", "full_name": "fork/repo"} if fork else REPO
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

    def test_waiter_accepts_canonical_path_and_rest_summary(self):
        for path in (".github/workflows/pr-governance-review-events.yml", ".github/workflows/pr-governance-review-events.yml@refs/heads/master"):
            with self.subTest(path=path):
                run = copy.deepcopy(sensor(1))
                run["path"] = path
                summary = {"id": 7, "name": "repo", "url": "https://api.github.com/repos/owner/repo"}
                run["pull_requests"][0]["base"]["repo"] = summary
                run["pull_requests"][0]["head"]["repo"] = summary
                self.execute_waiter({1: run}, [[]])

    def test_scanner_and_waiter_share_the_canonical_identity_contract(self):
        preflight = ast.parse((ROOT / "scripts/review/pr_governance_preflight.py").read_text())
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text()
        job = workflow.split("  wait-for-active-review-sensor:", 1)[1].split("  establish-resolver-failure-barrier:", 1)[0]
        program = textwrap.dedent(re.search(r"python3 - <<'PY'\n(.*?)\n          PY", job, re.S).group(1))
        waiter = ast.parse(program)
        for name in ("canonical_repository_binding", "workflow_path_matches"):
            first = next(node for node in preflight.body if isinstance(node, ast.FunctionDef) and node.name == name)
            second = next(node for node in waiter.body if isinstance(node, ast.FunctionDef) and node.name == name)
            self.assertEqual(ast.dump(first, include_attributes=False), ast.dump(second, include_attributes=False))
        helper = next(node for node in preflight.body if isinstance(node, ast.FunctionDef) and node.name == "canonical_repository_binding")
        namespace = {"re": re}
        matcher = next(node for node in preflight.body if isinstance(node, ast.FunctionDef) and node.name == "workflow_path_matches")
        exec(compile(ast.Module(body=[helper, matcher], type_ignores=[]), "canonical-identity-contract", "exec"), namespace)
        binding = namespace["canonical_repository_binding"]
        path_matches = namespace["workflow_path_matches"]
        expected = ".github/workflows/pr-governance-review-events.yml"
        for path in (expected, expected + "@main", expected + "@refs/heads/master"):
            self.assertTrue(path_matches(path, expected))
        for path in (expected + "@", expected + "@../master", expected + "@main/", expected + "@main?x=1", expected + "/other@main"):
            self.assertFalse(path_matches(path, expected))
        summary = {"id": 7, "name": "repo", "url": "https://api.github.com/repos/owner/repo"}
        self.assertEqual(binding(summary), (7, "owner/repo"))
        self.assertEqual(binding(REPO), (7, "owner/repo"))
        self.assertEqual(binding({"id": 7, "full_name": "owner/repo"}), (7, "owner/repo"))
        invalid = [dict(summary, id=True), dict(summary, id=8, full_name="other/repo"),
                   dict(summary, name="other"), dict(summary, full_name=None),
                   dict(summary, owner={"login": "other"}), dict(summary, html_url="https://github.com/other/repo")]
        invalid.extend(dict(summary, url=url) for url in (
            "https://api.github.com.evil/repos/owner/repo", "http://api.github.com/repos/owner/repo",
            "https://api.github.com/repos/owner/repo?x=1", "https://api.github.com/repos/owner/repo/"))
        invalid.extend({key: value for key, value in summary.items() if key != missing} for missing in ("id", "name", "url"))
        for value in invalid:
            with self.subTest(value=value):
                self.assertIsNone(binding(value))

    def test_waiter_rejects_typed_identity_and_provided_field_conflicts(self):
        changes = [lambda run: run.update(run_attempt=True), lambda run: run.update(id=True), lambda run: run.update(id=2**63),
                   lambda run: run["pull_requests"][0].update(number=True),
                   lambda run: run["pull_requests"][0].update(draft="false"),
                   lambda run: run["pull_requests"][0]["base"]["repo"].update(full_name="other/repo"),
                   lambda run: run["pull_requests"][0]["head"]["repo"].update(url="https://api.github.com.evil/repos/owner/repo"),
                   lambda run: run["pull_requests"][0]["head"]["repo"].update(id=8)]
        for change in changes:
            run = copy.deepcopy(sensor(1))
            change(run)
            with self.subTest(change=change), self.assertRaises(SystemExit):
                self.execute_waiter({1: run}, [[]])

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
