from __future__ import annotations

import ast
import base64
import copy
import hashlib
import io
import inspect
import json
import os
import re
import tempfile
import textwrap
import unittest
import urllib.parse
import zipfile
from contextlib import ExitStack
from collections import Counter, deque
from datetime import datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import pr_governance_cohort as cohort
import pr_governance_cohort_test as cohort_fixtures
import pr_governance_status_writer_test as writer_fixtures
import pr_governance_event_test as sensor_fixtures
from pr_governance_cohort_test import RawArtifactFixture


ROOT = Path(__file__).parents[2]
ADMISSION = "admit-cohort-backend"
ELECTION = "elect-cohort-owner"
PUBLISHER = "publish-cohort-journal"
SUCCESS = f"needs.{ADMISSION}.result == 'success'"
OWNER = f"needs.{ADMISSION}.outputs.owner == 'true'"


def split_boolean(expression: str, operator: str) -> list[str]:
    parts: list[str] = []
    depth = 0
    quoted = False
    start = 0
    index = 0
    while index < len(expression):
        character = expression[index]
        if character == "'":
            if quoted and expression[index:index + 2] == "''":
                index += 2
                continue
            quoted = not quoted
        elif not quoted:
            if character == "(":
                depth += 1
            elif character == ")":
                depth -= 1
                if depth < 0:
                    raise ValueError("Unbalanced governance condition")
            elif depth == 0 and expression.startswith(operator, index):
                parts.append(expression[start:index].strip())
                index += len(operator)
                start = index
                continue
        index += 1
    if quoted or depth != 0:
        raise ValueError("Unbalanced governance condition")
    parts.append(expression[start:].strip())
    if any(not part for part in parts):
        raise ValueError("Empty governance condition")
    return parts


def requires_positive_factor(expression: str, expected: str) -> bool:
    expression = expression.strip()
    if expression.startswith("${{") and expression.endswith("}}"):
        expression = expression[3:-2].strip()
    # OR の片側だけに admission を置く退行を許可しない。
    alternatives = split_boolean(expression, "||")
    if len(alternatives) > 1:
        return all(requires_positive_factor(part, expected) for part in alternatives)
    factors = split_boolean(expression, "&&")
    if len(factors) > 1:
        return any(requires_positive_factor(part, expected) for part in factors)
    if expression.startswith("(") and expression.endswith(")"):
        return requires_positive_factor(expression[1:-1], expected)
    return re.sub(r"\s+", " ", expression) == expected


def requires_admission_success(expression: str) -> bool:
    return requires_positive_factor(expression, SUCCESS)


def jobs(text: str) -> dict[str, str]:
    if text.count("\njobs:\n") != 1:
        raise ValueError("Missing or duplicate governance jobs section")
    text = text.split("\njobs:\n", 1)[1]
    blocks = list(re.finditer(r"^  ([a-z][a-z0-9_-]*):\s*$", text, re.MULTILINE))
    result: dict[str, str] = {}
    for index, match in enumerate(blocks):
        identifier = match.group(1)
        if identifier in result:
            raise ValueError("Duplicate governance job")
        end = blocks[index + 1].start() if index + 1 < len(blocks) else len(text)
        result[identifier] = text[match.end():end]
    return result


def job_needs(block: str) -> list[str]:
    matches = list(re.finditer(r"^    needs:([^\n]*)$", block, re.MULTILINE))
    if not matches:
        return []
    if len(matches) != 1:
        raise ValueError("Duplicate job dependencies")
    match = matches[0]
    value = match.group(1).strip()
    if value:
        if value.startswith("[") and value.endswith("]"):
            result = [part.strip() for part in value[1:-1].split(",")]
        else:
            result = [value]
    else:
        result = []
        for line in block[match.end():].splitlines()[1:]:
            item = re.fullmatch(r"(?:    |      )- ([a-z][a-z0-9_-]*)", line)
            if item is None:
                break
            result.append(item.group(1))
    if not result or any(re.fullmatch(r"[a-z][a-z0-9_-]*", item) is None for item in result):
        raise ValueError("Invalid job dependencies")
    return result


class CohortAdmissionConditionTest(unittest.TestCase):
    def test_dependencies_accept_scalar_inline_and_block_sequence(self) -> None:
        expected = ["preflight-workflow-run-source", PUBLISHER]
        for block in (
            "\n    needs: [preflight-workflow-run-source, publish-cohort-journal]\n",
            "\n    needs:\n    - preflight-workflow-run-source\n    - publish-cohort-journal\n    if: foo\n",
            "\n    needs:\n      - preflight-workflow-run-source\n      - publish-cohort-journal\n    if: foo\n",
        ):
            self.assertEqual(job_needs(block), expected)
        self.assertEqual(job_needs("\n    needs: publish-cohort-journal\n"), [PUBLISHER])

    def test_conjunction_and_nested_gate(self) -> None:
        for condition in (
            SUCCESS,
            "${{ always() && " + SUCCESS + " && github.run_attempt == 1 }}",
            "((" + SUCCESS + ") && (foo || bar))",
            "(" + SUCCESS + " && foo) || (bar && " + SUCCESS + ")",
        ):
            with self.subTest(condition=condition):
                self.assertTrue(requires_admission_success(condition))

    def test_disjunction_missing_negated_or_literal_gate_is_rejected(self) -> None:
        for condition in (
            "always()",
            SUCCESS + " || github.run_attempt == 1",
            "foo || (" + SUCCESS + " && bar)",
            "!(" + SUCCESS + ")",
            "needs.admit-cohort-backend.result != 'failure'",
            "contains(foo, '" + SUCCESS.replace("'", "''") + "')",
        ):
            with self.subTest(condition=condition):
                self.assertFalse(requires_admission_success(condition))

    def test_unbalanced_condition_is_rejected(self) -> None:
        for condition in ("(" + SUCCESS, SUCCESS + ")", SUCCESS + " &&", "'broken"):
            with self.subTest(condition=condition):
                with self.assertRaises(ValueError):
                    requires_admission_success(condition)


class GovernanceCohortDagContractTest(unittest.TestCase):
    def test_failed_owner_recovery_has_no_self_notification_loop_or_election_cycle(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        trigger = workflow.split("\njobs:\n", 1)[0]
        notification = trigger.split("  workflow_run:\n", 1)[1]
        self.assertNotIn("PR governance dispatcher", notification)
        graph = jobs(workflow)
        recovery = "dispatch-failed-cohort-recovery"
        self.assertIn(recovery, graph)
        block = graph[recovery]
        # 先行 job の終了後に通知を送り、通知先の完了を待たずに元 run を終了する。
        dependencies = job_needs(block)
        self.assertEqual(set(dependencies), set(graph) - {recovery})
        for identifier in dependencies:
            self.assertNotIn(recovery, job_needs(graph[identifier]))
        condition = re.findall(r"^    if:\s*(.+)$", block, re.MULTILINE)
        self.assertEqual(len(condition), 1)
        self.assertTrue(requires_admission_success(condition[0]))
        self.assertTrue(requires_positive_factor(condition[0], OWNER))
        failures = re.findall(r"needs\.([a-z][a-z0-9_-]*)\.result == 'failure'", condition[0])
        self.assertEqual(set(failures), set(dependencies))
        self.assertEqual(condition[0], "${{ always() && " + SUCCESS + " && " + OWNER + " && ("
                         + " || ".join(f"needs.{identifier}.result == 'failure'" for identifier in dependencies)
                         + ") }}")
        permissions = block.split("    permissions:\n", 1)[1].split("    steps:\n", 1)[0]
        self.assertEqual(dict(re.findall(r"^      ([a-z-]+): (read|write)$", permissions, re.MULTILINE)),
                         {"actions": "write", "contents": "read", "pull-requests": "read"})
        self.assertNotIn("uses: actions/create-github-app-token@", block)
        self.assertIn("GH_TOKEN: ${{ github.token }}", block)
        self.assertIn("COHORT_MODE: dispatch-recovery", block)
        self.assertIn("COHORT_ORIGINAL_ROOT_DEADLINE: ${{ needs.admit-cohort-backend.outputs.root_deadline_epoch }}", block)

    def test_sensor_static_mutation_inventory_matches_cohort_reader(self) -> None:
        sensor = (ROOT / ".github/workflows/pr-governance-review-events.yml").read_text(encoding="utf-8")
        payloads: list[str] = []
        for name in ("Prepare", "Complete", "Finalize"):
            marker = f"      - name: {name} trusted cohort proof reader\n"
            self.assertTrue(marker in sensor, f"Missing {name} proof preparation")
            block = sensor.split(marker, 1)[1].split("\n      - name:", 1)[0]
            payloads.extend(textwrap.dedent(payload) for payload in re.findall(
                r"<<'PY'\n(.*?)\n          PY(?:\n|$)", block, re.DOTALL,
            ))

        def inventory(code: str) -> set[str]:
            tree = ast.parse(code)
            values = [node.value for node in tree.body if isinstance(node, ast.Assign)
                      and any(isinstance(target, ast.Name) and target.id == "MUTATING_JOBS"
                              for target in node.targets)]
            self.assertEqual(len(values), 1, "Missing or duplicate static mutation inventory")
            self.assertIsInstance(values[0], ast.Call)
            self.assertEqual(len(values[0].args), 1)
            return set(ast.literal_eval(values[0].args[0]))

        code = "\n".join(payloads)
        tree = ast.parse(code)
        classifier = any(isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                         and node.name == "consumer_is_covered" for node in tree.body)
        has_inventory = any(isinstance(node, ast.Assign)
                            and any(isinstance(target, ast.Name) and target.id == "MUTATING_JOBS"
                                    for target in node.targets) for node in tree.body)
        if not classifier and not has_inventory:
            # mutation を観測しない reader profile には不要な inventory を要求しない。
            return
        self.assertEqual(inventory(code), inventory(
            (ROOT / "scripts/review/pr_governance_cohort.py").read_text(encoding="utf-8"),
        ))

    def test_reserved_sensor_early_service_precedes_sensor_wait(self) -> None:
        graph = jobs((ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8"))

        def ancestors(identifier: str) -> set[str]:
            pending = job_needs(graph[identifier])
            visited: set[str] = set()
            while pending:
                dependency = pending.pop()
                self.assertTrue(dependency in graph, f"Unknown dependency: {dependency}")
                if dependency in visited:
                    continue
                visited.add(dependency)
                pending.extend(job_needs(graph[dependency]))
            return visited

        early = "cohort-early-service"
        wait = "wait-for-active-review-sensor"
        self.assertTrue(early in graph, "Missing reserved sensor early service")
        self.assertTrue(wait in graph, "Missing strict sensor wait")
        before_early = ancestors(early)
        self.assertIn("arm-reserved-sensor-barrier", before_early)
        self.assertIn("establish-resolver-failure-barrier", before_early)
        self.assertNotIn(wait, before_early, "Early writer waits for the sensor that awaits its Check Run")
        self.assertIn(early, ancestors(wait))

    def test_cancelled_follower_proof_covers_every_mutating_job(self) -> None:
        graph = jobs((ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8"))
        expected: set[str] = set()
        for identifier, block in graph.items():
            if ("uses: actions/create-github-app-token@" not in block
                    and not re.search(r"--method[\s\"',]+(?:POST|PATCH|DELETE|PUT)", block)
                    and not re.search(r"^      actions: write$", block, re.MULTILINE)):
                continue
            names = re.findall(r"^    name:\s*(.+)$", block, re.MULTILINE)
            self.assertEqual(len(names), 1, f"Missing exact mutation job name: {identifier}")
            expected.add(names[0].strip("\"'"))
        tree = ast.parse((ROOT / "scripts/review/pr_governance_cohort.py").read_text(encoding="utf-8"))
        values = [node.value for node in tree.body if isinstance(node, ast.Assign)
                  and any(isinstance(target, ast.Name) and target.id == "MUTATING_JOBS"
                          for target in node.targets)]
        self.assertEqual(len(values), 1)
        value = values[0]
        self.assertIsInstance(value, ast.Call)
        self.assertEqual(len(value.args), 1)
        self.assertEqual(set(ast.literal_eval(value.args[0])), expected)

    def test_admission_does_not_wait_for_owner_election_completion(self) -> None:
        graph = jobs((ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8"))
        self.assertTrue(ADMISSION in graph, "Missing positive admission job")
        pending = [ADMISSION]
        visited: set[str] = set()
        while pending:
            identifier = pending.pop()
            if identifier in visited:
                continue
            visited.add(identifier)
            self.assertNotEqual(identifier, ELECTION, "Admission waits for the owner that awaits its backend")
            self.assertTrue(identifier in graph, f"Unknown admission dependency: {identifier}")
            pending.extend(job_needs(graph[identifier]))

    def test_sensor_reader_pins_actual_dispatcher_workflow_blob(self) -> None:
        sensor = (ROOT / ".github/workflows/pr-governance-review-events.yml").read_text(encoding="utf-8")
        preparation = sensor.split("      - name: Prepare trusted cohort sensor reader\n", 1)[1]
        preparation = preparation.split("\n      - name:", 1)[0]
        modules = re.findall(r"<<'PY'\n(.*?)\n          PY(?:\n|$)", preparation, re.DOTALL)
        self.assertEqual(len(modules), 1)
        tree = ast.parse(textwrap.dedent(modules[0]))
        assignments = [node for node in tree.body if isinstance(node, ast.Assign)
                       and any(isinstance(target, ast.Name)
                               and target.id == "PINNED_DISPATCHER_WORKFLOW_BLOB_SHA"
                               for target in node.targets)]
        self.assertEqual(len(assignments), 1, "Missing or duplicate immutable dispatcher pin")
        pin = ast.literal_eval(assignments[0].value)
        self.assertIsInstance(pin, str)
        self.assertRegex(pin, r"\A[0-9a-f]{40}\Z")
        payload = (ROOT / ".github/workflows/pr-governance.yml").read_bytes()
        # Git の blob 識別子へ固定し、別 workflow の証明を再利用させない。
        expected = hashlib.sha1(b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload).hexdigest()
        self.assertEqual(pin, expected)

    def test_all_app_jobs_require_positive_admission(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        graph = jobs(workflow)
        self.assertTrue(PUBLISHER in graph, "Missing durable publisher job")
        self.assertTrue(ELECTION in graph, "Missing read-only election job")
        self.assertTrue(ADMISSION in graph, "Missing positive admission job")
        protected = [identifier for identifier, block in graph.items()
                     if "uses: actions/create-github-app-token@" in block
                     or re.search(r"--method[\s\"',]+(?:POST|PATCH|DELETE|PUT)", block)
                     or re.search(r"^      actions: write$", block, re.MULTILINE)]
        self.assertTrue(protected)
        for identifier in protected:
            with self.subTest(job=identifier):
                block = graph[identifier]
                conditions = re.findall(r"^    if:\s*(.+)$", block, re.MULTILINE)
                self.assertIn(ADMISSION, job_needs(block))
                self.assertEqual(len(conditions), 1)
                self.assertTrue(requires_admission_success(conditions[0]), identifier)
                self.assertTrue(requires_positive_factor(conditions[0], OWNER), identifier)

    def test_election_has_no_privileged_mutation_and_follows_durable_publisher(self) -> None:
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        graph = jobs(workflow)
        self.assertTrue(ELECTION in graph, "Missing read-only election job")
        block = graph[ELECTION]
        self.assertNotIn("actions/create-github-app-token@", block)
        self.assertNotRegex(block, r"permission-[a-z-]+:\s*write")
        self.assertNotRegex(block, r"--method[\s\"',]+(?:POST|PATCH|DELETE|PUT)")
        self.assertIn(PUBLISHER, job_needs(block))


class GovernanceContextOwnerInventoryTest(unittest.TestCase):
    def fixture(self, *, phase="early"):
        with patch.object(cohort.time, "time", return_value=2_000_000_000):
            raw, _, _, _ = cohort_fixtures.CohortFollowerAndHandoffTests().follower()
        raw.steps[cohort.PRODUCER_JOB][:] = raw.steps[cohort.PRODUCER_JOB][:3]
        raw.run.update(status="in_progress", conclusion=None)
        binding = f"owner=100 artifact=1 digest={raw.header_ref['artifact_digest']}"
        raw.steps[cohort.ELECTION_JOB].append({"name": cohort.SELECTED_PREFIX + binding,
                                              "status": "completed", "conclusion": "success"})
        raw.jobs["jobs"].append({"id": 4, "run_id": 100, "head_sha": raw.sha, "name": cohort.ADMISSION_JOB,
            "status": "completed", "conclusion": "success", "steps": [{"name": cohort.ADMITTED_PREFIX + binding,
                                                                         "status": "completed", "conclusion": "success"}]})
        raw.jobs["total_count"] = len(raw.jobs["jobs"])
        if phase == "all":
            raw.jobs["jobs"][1].update(status="completed", conclusion="success")
            raw.steps[cohort.ELECTION_JOB].append({"name": cohort.HANDOFF_PREFIX + binding,
                                                  "status": "completed", "conclusion": "success"})
        values = [copy.deepcopy(raw.run)] + [dict(raw.run, id=1000 + index, run_number=1000 + index,
            status="queued", name=f"sensor={1000 + index} action=in_progress",
            display_title=f"sensor={1000 + index} action=in_progress") for index in range(100)]
        endpoint = "repos/" + raw.repository + "/actions/workflows/pr-governance.yml/runs?per_page=100&page=1"

        def read(path, *, timeout):
            if "/actions/workflows/pr-governance.yml/runs?" in path:
                page = int(urllib.parse.parse_qs(urllib.parse.urlsplit(path).query)["page"][0])
                return {"total_count": len(values), "workflow_runs": copy.deepcopy(values[(page - 1) * 100:page * 100])}
            return raw.read_json(path, timeout=timeout)

        budget = cohort.ReadBudget(900)
        context = cohort.DispatcherCohortFilter(1, raw.header_ref["artifact_digest"], 100,
            read_json=read, read_archive=raw.read_archive, budget=budget, deadline=21200, phase=phase)
        return raw, context, values, endpoint, read, budget

    def test_positive_context_owner_keeps_exact_hundred_pending_bound(self) -> None:
        for phase in ("early", "all"):
            with self.subTest(phase=phase), patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repository"}), \
                    patch.object(cohort.time, "time", return_value=200):
                raw, context, values, endpoint, read, budget = self.fixture(phase=phase)
                before = budget.used
                result = context.page(endpoint, read(endpoint, timeout=20))
                self.assertEqual(result["total_count"], 100)
                self.assertEqual([run["id"] for run in result["workflow_runs"]], list(range(1000, 1100)))
                self.assertNotIn(100, context.covered)
                self.assertNotIn(100, context.cohort["members_by_id"])
                self.assertLessEqual(budget.used - before, 60)
                values.append(dict(raw.run, id=1100, run_number=1100, status="queued"))
                with self.assertRaisesRegex(cohort.CohortError, "existing bound"):
                    context.page(endpoint, read(endpoint, timeout=20))

    def test_context_owner_requires_fresh_lease_artifact_source_and_type_sensitive_identity(self) -> None:
        cases = ("float-id", "float-run-number", "bool-attempt", "float-repository-id", "selected-marker",
                 "admission", "archive", "original-clock", "closed-early", "missing-handoff",
                 "owner-double-read", "expired-source", "expired-root")
        for case in cases:
            with self.subTest(case=case), patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repository"}), \
                    patch.object(cohort.time, "time", return_value=200):
                raw, context, values, endpoint, read, _ = self.fixture(phase="all" if case == "missing-handoff" else "early")
                if case == "float-id":
                    values[0]["id"] = 100.0
                elif case == "float-run-number":
                    values[0]["run_number"] = float(values[0]["run_number"])
                elif case == "bool-attempt":
                    values[0]["run_attempt"] = True
                elif case == "float-repository-id":
                    values[0]["repository"]["id"] = 101.0
                elif case == "selected-marker":
                    raw.steps[cohort.ELECTION_JOB][:] = [stage for stage in raw.steps[cohort.ELECTION_JOB]
                                                       if not stage["name"].startswith(cohort.SELECTED_PREFIX)]
                elif case == "admission":
                    raw.jobs["jobs"][-1]["conclusion"] = "failure"
                elif case == "archive":
                    raw.archives[1] = b"changed"
                elif case == "original-clock":
                    raw.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]["steps"][0]["started_at"] = "1970-01-01T00:01:41Z"
                elif case == "closed-early":
                    raw.run.update(status="completed", conclusion="success")
                    values[0].update(status="completed", conclusion="success")
                elif case == "missing-handoff":
                    raw.steps[cohort.ELECTION_JOB][:] = [stage for stage in raw.steps[cohort.ELECTION_JOB]
                                                       if not stage["name"].startswith(cohort.HANDOFF_PREFIX)]
                elif case == "owner-double-read":
                    owner_reads = [0]
                    def changed(path, *, timeout):
                        value = read(path, timeout=timeout)
                        if path == "repos/owner/repository/actions/runs/100":
                            owner_reads[0] += 1
                            if owner_reads[0] >= 6:
                                value["run_number"] = float(value["run_number"])
                        return value
                    context.reader.read_json = changed
                elif case in {"expired-source", "expired-root"}:
                    cohort.time.time.return_value = 6000 if case == "expired-source" else 21200
                with self.assertRaises(cohort.CohortError):
                    context.page(endpoint, read(endpoint, timeout=20))

    def test_inventory_anchor_later_count_and_cached_identity_preserve_json_types(self) -> None:
        for case in ("bool-anchor", "float-anchor", "float-page-count", "cached-float-repository",
                     "late-page-drift", "cached-nonmutating-float-repository"):
            with self.subTest(case=case), patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repository"}), \
                    patch.object(cohort.time, "time", return_value=200):
                raw, context, values, endpoint, read, _ = self.fixture()
                if case.endswith("anchor"):
                    values[:] = values[:1]
                first = read(endpoint, timeout=20)
                second_page_reads = [0]

                def changed(path, *, timeout):
                    response = read(path, timeout=timeout)
                    if "/actions/workflows/pr-governance.yml/runs?" in path:
                        page = urllib.parse.parse_qs(urllib.parse.urlsplit(path).query)["page"][0]
                        if case.endswith("anchor") and page == "1":
                            response["total_count"] = True if case == "bool-anchor" else 1.0
                        elif case == "float-page-count" and page == "2":
                            response["total_count"] = float(response["total_count"])
                        elif case == "late-page-drift" and page == "2":
                            second_page_reads[0] += 1
                            if second_page_reads[0] == 2:
                                response["workflow_runs"][0]["run_number"] = float(response["workflow_runs"][0]["run_number"])
                    return response

                if case in {"cached-float-repository", "cached-nonmutating-float-repository"}:
                    immutable = ("id", "run_attempt", "name", "display_title", "head_sha", "head_branch", "path",
                                 "event", "workflow_id", "run_number", "repository", "head_repository", "status", "conclusion")
                    cache = context.nonmutating if case.startswith("cached-nonmutating") else context.covered
                    cache[1000] = {key: copy.deepcopy(values[1].get(key)) for key in immutable}
                    values[1]["repository"] = dict(values[1]["repository"], id=101.0)
                    first = read(endpoint, timeout=20)
                context.reader.read_json = changed
                with self.assertRaises(cohort.CohortError):
                    context.page(endpoint, first)

    def test_terminal_inventory_cache_commits_only_after_full_snapshot_acceptance(self) -> None:
        with patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repository"}), patch.object(cohort.time, "time", return_value=200):
            raw, context, values, endpoint, read, _ = self.fixture()
            candidate = dict(raw.run, id=999, run_number=999, name="sensor=11 action=in_progress",
                display_title="sensor=11 action=in_progress", head_repository=copy.deepcopy(raw.run["repository"]),
                status="completed", conclusion="failure")
            values[-1] = candidate
            candidate_jobs = []
            for index, name in enumerate(sorted(cohort.MUTATING_JOBS | {cohort.PRODUCER_JOB,
                    cohort.ELECTION_JOB, cohort.ADMISSION_JOB, "Preflight workflow_run governance source"}), 1):
                conclusion = "skipped" if name in cohort.MUTATING_JOBS else "success"
                conclusion = {cohort.ELECTION_JOB: "cancelled", cohort.ADMISSION_JOB: "failure"}.get(name, conclusion)
                stages = ([{"number": index + 1, "name": stage, "status": "completed", "conclusion": "success"}
                    for index, stage in enumerate((cohort.PAYLOAD_PREFIX + "source sha256=" + "a" * 64,
                                                  "Upload immutable krr-governance-source-11-attempt-1"))]
                    if name == cohort.PRODUCER_JOB else [])
                candidate_jobs.append({"id": 99900 + index, "run_id": 999, "head_sha": raw.sha,
                    "name": name, "status": "completed", "conclusion": conclusion, "steps": stages})
            native = "repos/owner/repository/actions/runs/999/attempts/1/jobs?per_page=100&page=1"
            raw.data[native] = {"total_count": len(candidate_jobs), "jobs": candidate_jobs}
            page_reads, job_reads = [0], [0]
            def drifting(path, *, timeout):
                response = read(path, timeout=timeout)
                if path == native:
                    job_reads[0] += 1
                if "/actions/workflows/pr-governance.yml/runs?" in path and "page=2" in path:
                    page_reads[0] += 1
                    if page_reads[0] == 2:
                        response["workflow_runs"][0]["run_number"] = 999.0
                return response
            context.reader.read_json = drifting
            with self.assertRaisesRegex(cohort.CohortError, "after job proof"):
                context.page(endpoint, read(endpoint, timeout=20))
            self.assertFalse(context.nonmutating)
            self.assertFalse(context.covered)
            self.assertNotIn(("jobs", raw.repository, 999), context.reader.cache)
            context.page(endpoint, read(endpoint, timeout=20))
            self.assertEqual(job_reads[0], 2)
            self.assertIn(999, context.nonmutating)
            self.assertNotIn(999, context.covered)


class GovernanceCohortSchedulingReplayTest(unittest.TestCase):
    """単体simulation。通常は凍結時計、decision成功入力・admission即readyであり、live SLAではない。"""

    def replay(self, total: int, *, policy: str | None = None, enforce_raw_filter: bool = False,
               raw_filter_phase: str = "all", include_completed_callbacks: bool = True,
               enforce_writer_barrier: bool = False, drain_pending: bool = False,
               enforce_check_writes: bool = False, advance_pacing_clock: bool = False,
               enforce_installation_quota: bool = False, conditional_helper_transport: bool = False,
               cold_process: bool = False, conditional_writer_transport: bool = False,
               actual_sensor_loop: bool = False) -> dict:
        self.assertFalse(enforce_check_writes and not enforce_writer_barrier)
        self.assertFalse(advance_pacing_clock and not enforce_check_writes)
        self.assertFalse(enforce_installation_quota and not advance_pacing_clock)
        self.assertFalse(cold_process and not advance_pacing_clock)
        self.assertFalse(conditional_writer_transport and not cold_process)
        self.assertFalse(actual_sensor_loop and not (conditional_writer_transport and conditional_helper_transport))
        check_app_id = 4766933 if cold_process else 9001
        virtual_clock = [200.0]
        mutation_times, ack_times = [], []
        installation_requests, installation_window = [], deque()
        helper_requests, storage_requests = [], []
        metadata_requests = []
        writer_http_attempts = []
        installation_peak, installation_rejected_at = [0], [None]
        def app_request(function, endpoint=None, method="GET", component="writer"):
            while installation_window and installation_window[0] <= virtual_clock[0] - 3600:
                installation_window.popleft()
            if enforce_installation_quota and len(installation_window) >= 4500:
                installation_rejected_at[0] = virtual_clock[0]
                error = cohort.CohortError if component == "helper" else writer_fixtures.WRITER.NoPostGovernanceError
                raise error("Shared App installation rolling3600 quota is exhausted.")
            installation_requests.append({"epoch": virtual_clock[0], "function": function,
                                          "endpoint": endpoint, "method": method, "component": component})
            installation_window.append(virtual_clock[0])
            installation_peak[0] = max(installation_peak[0], len(installation_window))
        def native_timestamp():
            return datetime.fromtimestamp(virtual_clock[0], timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        def installation_metrics():
            reads = [value for value in installation_requests if value["method"] == "GET"]
            counts = Counter((value["function"], value["endpoint"]) for value in reads)
            attempts = ([value for value in writer_http_attempts if value["principal"] == "App"]
                        if conditional_writer_transport else
                        [dict(value, status=200) for value in installation_requests if value["component"] == "writer"])
            attempts += [dict(value, method="GET") for value in helper_requests if value["principal"] == "App"]
            return {"app_total": len(installation_requests), "app_rolling3600_peak": installation_peak[0],
                    "writer_transport": "actual-conditional" if conditional_writer_transport else "bare-get-diagnostic",
                    "app_rejected_epoch": installation_rejected_at[0],
                    "app_reads": len(reads), "app_mutations": len(installation_requests) - len(reads),
                    "app_by_component": dict(Counter(value["component"] for value in installation_requests)),
                    "app_primary_by_function": dict(Counter(value["function"] for value in installation_requests)),
                    "app_attempts": len(attempts),
                    "app_status_counts": dict(Counter(value.get("status", 200) for value in attempts)),
                    "app_secondary_points": sum(1 if value["method"] == "GET" else 5 for value in attempts),
                    "writer_http_status_counts": dict(Counter(value["status"] for value in writer_http_attempts)),
                    "writer_http_attempts": len(writer_http_attempts),
                    "writer_secondary_points": sum(1 if value["method"] == "GET" else 5 for value in writer_http_attempts),
                    "top_read_endpoints": [(function, endpoint, count) for (function, endpoint), count in counts.most_common(5)]}
        def helper_metrics():
            default_attempts = [value for value in helper_requests if value["principal"] == "GITHUB_TOKEN"]
            default = [value for value in default_attempts if value.get("status", 200) != 304]
            counts = Counter((value["function"], value["endpoint"]) for value in default)
            json_counts = Counter((value["function"], value["endpoint"]) for value in default
                                  if value.get("status", 200) == 200)
            window, peak = deque(), 0
            for value in default:
                while window and window[0] <= value["epoch"] - 3600:
                    window.popleft()
                window.append(value["epoch"])
                peak = max(peak, len(window))
            return {"github_token_reads": len(default), "github_token_rolling3600_peak": peak,
                    "helper_transport": "actual-conditional" if conditional_helper_transport else "bare-get-diagnostic",
                    "github_token_attempts": len(default_attempts),
                    "github_token_primary_by_role": dict(Counter(value["function"] for value in default)),
                    "github_token_json200_by_role": dict(Counter(value["function"] for value in default
                                                                  if value.get("status", 200) == 200)),
                    "github_token_status_counts": dict(Counter(value.get("status", 200) for value in default_attempts)),
                    "github_token_secondary_get_points": len(default_attempts),
                    "helper_status_counts": dict(Counter(value.get("status", 200) for value in helper_requests)),
                    "helper_secondary_get_points": len(helper_requests),
                    "top_github_token_endpoints": [(function, endpoint, count) for (function, endpoint), count in counts.most_common(5)],
                    "top_github_token_json200_endpoints": [(function, endpoint, count) for (function, endpoint), count in json_counts.most_common(5)],
                    "storage_attempts": len(storage_requests),
                    "github_token_graphql_requests": len(metadata_requests),
                    "github_token_graphql_primary_cost": sum(value["cost"] for value in metadata_requests),
                    "github_token_graphql_rolling3600_peak": max((sum(item["cost"] for item in metadata_requests
                        if value["epoch"] - 3600 < item["epoch"] <= value["epoch"]) for value in metadata_requests), default=0),
                    "github_token_graphql_secondary_points": sum(value["secondary_points"] for value in metadata_requests),
                    "github_token_total_rest_graphql_attempts": len(default_attempts) + len(metadata_requests)}
        workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        graph = jobs(workflow)
        election = graph[ELECTION]
        self.assertIn(PUBLISHER, job_needs(election))
        self.assertNotIn(ELECTION, job_needs(graph[ADMISSION]))
        self.assertIn("COHORT_MODE: select", election)
        self.assertIn("COHORT_MODE: await-handoff", election)
        self.assertIn("COHORT_MODE: admission", graph[ADMISSION])
        self.assertIn("COHORT_MODE: ack-members", graph["cohort-early-service"])
        queue = re.findall(r"^      queue: (single|max)$", election, re.MULTILINE)
        self.assertEqual(len(queue), 1)
        self.assertIn("      cancel-in-progress: false", election)
        queue = policy or queue[0]
        raw = RawArtifactFixture()
        prefix = "repos/" + raw.repository
        raw.code = workflow.encode("utf-8")
        raw.blob = hashlib.sha1(b"blob " + str(len(raw.code)).encode() + b"\0" + raw.code).hexdigest()
        raw.data[prefix].update(id=101, full_name=raw.repository)
        raw.data[prefix + "/contents/.github/workflows/pr-governance.yml?ref=" + raw.sha].update(
            sha=raw.blob, size=len(raw.code), content=base64.b64encode(raw.code).decode(),
        )
        writer_code = (ROOT / "scripts/review/pr_governance_status_writer.py").read_bytes()
        writer_blob = hashlib.sha1(b"blob " + str(len(writer_code)).encode() + b"\0" + writer_code).hexdigest()
        raw.data[prefix + "/contents/scripts/review/pr_governance_status_writer.py?ref=" + raw.sha] = {
            "path": "scripts/review/pr_governance_status_writer.py", "encoding": "base64", "sha": writer_blob,
            "size": len(writer_code), "content": base64.b64encode(writer_code).decode()}
        metadata, archives, runs, run_jobs, sources, pulls = {}, {}, {}, {}, {}, {}
        next_artifact, requests, roles, pending, cancelled = [1000], [], [], [], []
        own_journals, original_source_archives, acknowledged, owners, cohort_sizes, role_reads, raw_filter_reads = {}, {}, set(), [], [], [], []
        completed_callbacks = []
        writer_barriers = []
        writer_runs, checks, written_sources = {}, {}, set()
        writer_read_counts, sensor_binding_reads = {}, []
        sensor_reader = {}
        sensor_tasks, sensor_completion_times = {}, []
        sensor_namespaces = {}
        sensor_caches, sensor_primary = {}, {}
        def sensor_read(endpoint, *, timeout, source_id, budget):
            if conditional_helper_transport:
                def native_response(arguments, **options):
                    value = read(endpoint, timeout=options["timeout"])
                    body = json.dumps(value, sort_keys=True, allow_nan=False)
                    tag = '"' + hashlib.sha256(body.encode("utf-8")).hexdigest() + '"'
                    unchanged = "If-None-Match: " + tag in arguments
                    status = 304 if unchanged else 200
                    helper_requests.append({"epoch": virtual_clock[0], "function": "sensor-cohort-binding",
                                            "endpoint": endpoint, "principal": "GITHUB_TOKEN", "status": status})
                    return SimpleNamespace(returncode=1 if unchanged else 0,
                        stdout=f"HTTP/2.0 {status} {'Not Modified' if unchanged else 'OK'}\r\nETag: {tag}\r\n\r\n" + ("" if unchanged else body))
                return sensor_reader["sensor_json_request"](endpoint, budget=budget,
                    cache=sensor_caches.setdefault(source_id, {}), primary=sensor_primary.setdefault(source_id, {"hits": 0}),
                    precharged=True, request_timeout=timeout, deadline=5500, terminal_page=False, inspect_link=False,
                    run=native_response, authenticated=True)
            if advance_pacing_clock:
                helper_requests.append({"epoch": virtual_clock[0], "function": "sensor-cohort-binding",
                                        "endpoint": endpoint, "principal": "GITHUB_TOKEN"})
            return read(endpoint, timeout=timeout)
        def sensor_archive(endpoint, *, timeout, source_id, budget):
            if not conditional_helper_transport:
                return archives[int(endpoint.split("/")[-2])]
            def open_request(request, requested_timeout):
                if request.full_url.startswith("https://api.github.com/"):
                    helper_requests.append({"epoch": virtual_clock[0], "function": "sensor-archive",
                                            "endpoint": endpoint, "principal": "GITHUB_TOKEN", "status": 302})
                    return SimpleNamespace(code=302, headers={"Location": "https://fixture.blob.core.windows.net/archive"})
                self.assertFalse(request.has_header("Authorization"))
                storage_requests.append((virtual_clock[0], "sensor-archive", "GITHUB_TOKEN"))
                return SimpleNamespace(code=200, read=lambda limit: archives[int(endpoint.split("/")[-2])][:limit])
            with patch.dict(sensor_reader, {"_open_no_redirect": open_request}):
                return sensor_reader["sensor_archive_read"](endpoint, timeout=timeout, deadline=5500,
                    budget=budget, token="fixture-default", primary=sensor_primary.setdefault(source_id, {"hits": 0}))
        if enforce_check_writes:
            sensor_workflow = (ROOT / ".github/workflows/pr-governance-review-events.yml").read_text()
            private_chunks = re.findall(r'          cat >>? [^\n]*krr-governance-cohort-reader.py[^\n]*<<\x27PY\x27\n(.*?)^          PY$',
                                        sensor_workflow, re.MULTILINE | re.DOTALL)
            self.assertEqual(len(private_chunks), 6)
            for chunk in private_chunks:
                exec(compile(textwrap.dedent(chunk), "native-sensor-cohort-reader", "exec"), sensor_reader)
        repo = {"id": 101, "full_name": raw.repository, "name": "repository",
                "url": "https://api.github.com/repos/" + raw.repository}
        raw.data[prefix].update(repo)

        def step(name):
            return {"name": name, "status": "completed", "conclusion": "success"}
        def complete_callback(identifier):
            callback = 20000 + identifier
            if callback in runs:
                return
            title = f"sensor={identifier} action=completed"
            runs[callback] = dict(raw.run, id=callback, run_number=callback, name=title,
                display_title=title, repository=repo, head_repository=repo, created_at=native_timestamp(),
                status="completed", conclusion="success")
            run_jobs[callback] = [{"id": callback * 100 + index, "run_id": callback,
                "head_sha": raw.sha, "name": name, "status": "completed",
                "conclusion": "success" if name == "Preflight workflow_run governance source" else "skipped",
                "steps": [dict(step(cohort.COMPLETED_NOOP_PREFIX + str(identifier)), number=1)]
                    if name == "Preflight workflow_run governance source" else []}
                for index, name in enumerate(sorted(cohort.MUTATING_JOBS | {cohort.PRODUCER_JOB,
                    cohort.ELECTION_JOB, cohort.ADMISSION_JOB, "Preflight workflow_run governance source"}), 1)]
            completed_callbacks.append(callback)

        def upload(payload, producer, segment=None):
            identifier = next_artifact[0]
            next_artifact[0] += 1
            kind, member = payload["kind"], payload.get("member")
            name = (f"krr-governance-source-{member['source_run_id']}-attempt-1" if kind == "source"
                    else f"krr-governance-{kind}-{producer}-attempt-1" + (f"-{segment}" if segment else ""))
            packed = io.BytesIO()
            with zipfile.ZipFile(packed, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr(zipfile.ZipInfo("binding.json", date_time=(2000, 1, 1, 0, 0, 0)), cohort.canonical(payload))
            data = packed.getvalue()
            digest = "sha256:" + hashlib.sha256(data).hexdigest()
            archives[identifier] = data
            metadata[identifier] = {"id": identifier, "name": name, "expired": False, "digest": digest,
                "size_in_bytes": len(data), "workflow_run": {"id": producer, "repository_id": 101,
                "head_repository_id": 101, "head_sha": raw.sha}}
            creator = next(job for job in run_jobs[producer]
                           if job["name"] == (cohort.PRODUCER_JOB if kind == "source" else cohort.ELECTION_JOB))
            for name in (cohort.PAYLOAD_PREFIX + kind + " sha256=" + hashlib.sha256(cohort.canonical(payload)).hexdigest(),
                         "Upload immutable " + name):
                creator["steps"].append(dict(step(name), number=len(creator["steps"]) + 1))
            if kind == "source":
                creator["steps"].append(dict(step(cohort.JOURNAL_PREFIX + base64.b64encode(cohort.canonical(payload)).decode()),
                                             number=len(creator["steps"]) + 1))
                original_source_archives[identifier] = data
            return {"artifact_id": identifier, "artifact_digest": digest}

        for offset in range(total):
            identifier, number, dispatcher = 1000 + offset, offset + 1, 10000 + offset
            source_head = f"{number:040x}"
            pull = {"number": number, "state": "open", "draft": False, "body": f"Fixes #{number + 1000}" if cold_process else "body",
                    "base": {"ref": "master", "sha": "b" * 40, "repo": repo},
                    "head": {"sha": source_head, "ref": f"release/native-{number}", "repo": repo}}
            sources[identifier] = {"id": identifier, "run_attempt": 1, "workflow_id": 9, "run_number": number,
                "name": "PR governance review sensor", "event": "pull_request_review", "head_sha": source_head,
                "path": ".github/workflows/pr-governance-review-events.yml", "repository": repo, "head_repository": repo,
                "created_at": "1970-01-01T00:01:30Z", "status": "in_progress", "conclusion": None, "pull_requests": [pull]}
            pulls[number] = pull
            run_jobs[identifier] = [{"id": identifier * 100, "run_id": identifier, "head_sha": source_head,
                "name": "KRR / PR governance review latch", "status": "in_progress", "conclusion": None,
                "steps": [{"name": "Await matching trusted governance Check Run", "number": 5,
                           "started_at": "1970-01-01T00:01:40Z", "status": "in_progress", "conclusion": None}]}]
            title = f"sensor={identifier} action=in_progress"
            runs[dispatcher] = dict(raw.run, id=dispatcher, run_number=dispatcher, name=title,
                                    display_title=title, repository=repo, head_repository=repo,
                                    created_at="1970-01-01T00:01:40Z", status="in_progress", conclusion=None)
            run_jobs[dispatcher] = [{"id": dispatcher * 100 + index, "run_id": dispatcher, "head_sha": raw.sha,
                "name": name, "status": ("in_progress" if offset == 0 else "queued") if name == cohort.ELECTION_JOB else "completed",
                "conclusion": None if name == cohort.ELECTION_JOB else "success", "steps": []}
                for index, name in enumerate((cohort.PRODUCER_JOB, cohort.ELECTION_JOB,
                                             "Preflight workflow_run governance source"), 1)]
            run_jobs[dispatcher][-1]["steps"] = copy.deepcopy(raw.steps["Preflight workflow_run governance source"])
            for index, native_step in enumerate(run_jobs[dispatcher][-1]["steps"], 1):
                native_step["number"] = index
            run_jobs[dispatcher][-1]["steps"].append(dict(step("Record review sensor staged early admission"),
                number=len(run_jobs[dispatcher][-1]["steps"]) + 1))
            for name in sorted(cohort.MUTATING_JOBS | {cohort.ADMISSION_JOB}):
                run_jobs[dispatcher].append({"id": dispatcher * 100 + len(run_jobs[dispatcher]) + 1,
                    "run_id": dispatcher, "head_sha": raw.sha, "name": name,
                    "status": "in_progress" if name == cohort.ADMISSION_JOB else "queued",
                    "conclusion": None, "steps": []})

        def read(endpoint, *, timeout):
            requests.append(endpoint)
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, 20)
            if endpoint == prefix + "/rulesets/21909389":
                return {"id": 21909389, "name": "KRR PR governance merge authority", "source": raw.repository,
                    "source_type": "Repository", "target": "branch", "enforcement": "active",
                    "conditions": {"ref_name": {"include": ["refs/heads/master"], "exclude": []}},
                    "bypass_actors": [{"actor_id": 4766933, "actor_type": "Integration", "bypass_mode": "pull_request"}],
                    "rules": [{"type": "update", "parameters": {"update_allows_fetch_and_merge": False}}]}
            if endpoint == prefix + "/branches/master/protection":
                names = [writer_fixtures.WRITER.CHECK_NAME, "KRR / PR governance review latch", "KRR / PR governance affected-head barrier"]
                return {"url": "https://api.github.com/" + endpoint, "enforce_admins": {"enabled": True},
                    "required_conversation_resolution": {"enabled": True}, "required_status_checks": {
                        "url": "https://api.github.com/" + endpoint + "/required_status_checks",
                        "contexts_url": "https://api.github.com/" + endpoint + "/required_status_checks/contexts",
                        "strict": True, "contexts": names,
                        "checks": [{"context": name, "app_id": app} for name, app in zip(names, (4766933, 15368, 4766933))]}}
            if "/actions/artifacts/" in endpoint:
                return copy.deepcopy(metadata[int(endpoint.rsplit("/", 1)[1])])
            if "/actions/artifacts?" in endpoint:
                name = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["name"][0]
                entries = [item for item in metadata.values() if item["name"] == name]
                return {"total_count": len(entries), "artifacts": copy.deepcopy(entries)}
            if "/actions/runs/" in endpoint and "/artifacts?" in endpoint:
                identifier = int(endpoint.split("/runs/")[1].split("/")[0])
                entries = [item for item in metadata.values() if item["workflow_run"]["id"] == identifier]
                return {"total_count": len(entries), "artifacts": copy.deepcopy(entries)}
            if "/actions/workflows/" in endpoint and "/runs?" in endpoint:
                query = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)
                collection = sources if "pr-governance-review-events.yml" in endpoint else runs
                entries = [item for item in collection.values()
                           if query.get("status", [item["status"]])[0] == item["status"]
                           and query.get("branch", [item.get("head_branch")])[0] == item.get("head_branch")
                           and query.get("head_sha", [item["head_sha"]])[0] == item["head_sha"]
                           and ("created" not in query or query["created"][0].startswith(">=")
                                and item["created_at"] >= query["created"][0][2:])]
                page = int(query["page"][0])
                return {"total_count": len(entries), "workflow_runs": copy.deepcopy(entries[(page - 1) * 100:page * 100])}
            if "/attempts/1/jobs?" in endpoint or "/jobs?" in endpoint:
                identifier = int(endpoint.split("/runs/")[1].split("/")[0])
                return {"total_count": len(run_jobs[identifier]), "jobs": copy.deepcopy(run_jobs[identifier])}
            if endpoint.startswith(prefix + "/pulls?"):
                query = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)
                page = int(query["page"][0])
                return copy.deepcopy(list(pulls.values())[(page - 1) * 100:page * 100])
            if "/actions/runs/" in endpoint:
                return copy.deepcopy({**runs, **sources, **writer_runs}[int(endpoint.rsplit("/", 1)[1])])
            if "/check-runs/" in endpoint:
                return copy.deepcopy(checks[int(endpoint.rsplit("/", 1)[1])])
            if "/check-runs?" in endpoint:
                head = endpoint.split("/commits/")[1].split("/")[0]
                if urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query).get("check_name") == ["KRR / PR governance affected-head barrier"]:
                    return {"total_count": 0, "check_runs": []}
                entries = [check for check in checks.values() if check["head_sha"] == head][-1:]
                return {"total_count": len(entries), "check_runs": copy.deepcopy(entries)}
            if "/pulls/" in endpoint:
                return copy.deepcopy(pulls[int(endpoint.rsplit("/", 1)[1])])
            return raw.read_json(endpoint, timeout=timeout)

        sensor_case = None
        if actual_sensor_loop:
            sensor_case = sensor_fixtures.GovernanceReviewSensorIdentityContractTest()
            sensor_case.setUp()
            self.addCleanup(sensor_case.doCleanups)
            start = sensor_case.await_program.index("deadline = time.monotonic() + timeout")
            body = ast.parse(sensor_case.await_program[start:])
            class CooperativeSleep(ast.NodeTransformer):
                def visit_Expr(self, node):
                    if (isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Attribute)
                        and isinstance(node.value.func.value, ast.Name) and node.value.func.value.id == "time"
                        and node.value.func.attr == "sleep"):
                        return ast.copy_location(ast.Expr(value=ast.Yield(value=node.value.args[0])), node)
                    return self.generic_visit(node)
            body = CooperativeSleep().visit(body)
            bindings = sorted({node.id for node in ast.walk(body) if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Store)})
            coroutine = ast.Module(body=[ast.FunctionDef(name="await_native_sensor", args=ast.arguments(
                posonlyargs=[], args=[], kwonlyargs=[], kw_defaults=[], defaults=[]),
                body=[ast.Global(names=bindings), *body.body], decorator_list=[])], type_ignores=[])
            sensor_program = compile(ast.fix_missing_locations(coroutine), "actual-sensor-await-cooperative-sleep", "exec")

        def finish_sensor(source_id):
            sources[source_id].update(status="completed", conclusion="success")
            run_jobs[source_id][0].update(status="completed", conclusion="success")
            run_jobs[source_id][0]["steps"][0].update(status="completed", conclusion="success", completed_at=native_timestamp())
            if include_completed_callbacks:
                complete_callback(source_id)

        def resume_sensor(source_id):
            task = sensor_tasks[source_id]
            namespace = sensor_namespaces[source_id]
            state = (id(namespace["check_scan"]), id(namespace["api_response_cache"]), id(namespace["sensor_read_budget"]))
            self.assertEqual(task.setdefault("state", state), state)
            previous_attempts = namespace["api_read_count"]
            def native_response(arguments, **options):
                if arguments[2] == "graphql":
                    fields = dict(arguments[index + 1].split("=", 1) for index, value in enumerate(arguments[:-1]) if value == "-f")
                    self.assertEqual(fields, {"query": cohort.MEMBER_GRANT_METADATA_QUERY,
                        "owner": raw.repository.split("/")[0], "name": raw.repository.split("/")[1],
                        "writerExpression": raw.sha + ":scripts/review/pr_governance_status_writer.py"})
                    self.assertEqual(options["env"]["GH_TOKEN"], "fixture-default")
                    repository = read(prefix, timeout=options["timeout"])
                    reference = read(prefix + "/git/ref/heads/master", timeout=options["timeout"])
                    contents = read(prefix + "/contents/scripts/review/pr_governance_status_writer.py?ref=" + raw.sha, timeout=options["timeout"])
                    code = base64.b64decode(contents["content"]).decode("utf-8")
                    value = {"data": {"repository": {"id": "R_native_opaque", "databaseId": repository["id"],
                        "nameWithOwner": repository["full_name"], "url": "https://github.com/" + repository["full_name"],
                        "defaultBranchRef": {"name": repository["default_branch"], "target": {"__typename": "Commit", "oid": reference["object"]["sha"]}},
                        "writer": {"__typename": "Blob", "oid": contents["sha"], "text": code,
                            "byteSize": len(code.encode()), "isBinary": False, "isTruncated": False}}, "rateLimit": {"cost": 1}}}
                    metadata_requests.append({"epoch": virtual_clock[0], "function": "sensor-metadata",
                        "source_id": source_id, "principal": "GITHUB_TOKEN", "status": 200, "cost": value["data"]["rateLimit"]["cost"], "secondary_points": 1})
                    return SimpleNamespace(returncode=0, stdout="HTTP/2.0 200 OK\r\n\r\n" + json.dumps(value), stderr="")
                endpoint = next(value for value in arguments if value.startswith(prefix))
                value = read(endpoint, timeout=options["timeout"])
                body = json.dumps(value, sort_keys=True, allow_nan=False)
                tag = '\"' + hashlib.sha256(body.encode()).hexdigest() + '\"'
                unchanged = "If-None-Match: " + tag in arguments
                status = 304 if unchanged else 200
                helper_requests.append({"epoch": virtual_clock[0], "function": "sensor-await",
                                        "endpoint": endpoint, "principal": "GITHUB_TOKEN", "status": status})
                return SimpleNamespace(returncode=1 if unchanged else 0, stdout=
                    f"HTTP/2.0 {status} {'Not Modified' if unchanged else 'OK'}\r\nETag: {tag}\r\n\r\n" + ("" if unchanged else body), stderr="")
            try:
                with patch.dict(os.environ, {"GH_TOKEN": "fixture-default"}), patch("subprocess.run", side_effect=native_response):
                    duration = next(task["generator"])
                self.assertEqual(duration, 60)
                self.assertGreater(namespace["api_read_count"], previous_attempts)
                self.assertLessEqual(namespace["api_read_count"], 300)
                task["due"] = virtual_clock[0] + duration
            except SystemExit as error:
                if error.code != 0:
                    raise cohort.CohortError(f"Actual sensor source={source_id} epoch={virtual_clock[0]} reads={namespace['api_read_count']}/300 rejected") from error
                self.assertEqual(namespace["original_boundary"], 5500)
                sensor_binding_reads.append((source_id, namespace["api_read_count"]))
                sensor_completion_times.append((virtual_clock[0], source_id))
                written_sources.add(source_id)
                finish_sensor(source_id)
                del sensor_tasks[source_id]

        def advance_clock(duration):
            target = virtual_clock[0] + duration
            if actual_sensor_loop:
                while sensor_tasks:
                    due = min(task["due"] for task in sensor_tasks.values())
                    if due > target:
                        break
                    virtual_clock[0] = due
                    for source_id in sorted(list(sensor_tasks)):
                        if sensor_tasks[source_id]["due"] == due:
                            resume_sensor(source_id)
            virtual_clock[0] = target

        native_transport = cohort._Transport
        class Transport(native_transport):
            def __init__(self, token, budget):
                super().__init__(token, budget)
            def json(self, endpoint, *, timeout):
                if conditional_helper_transport:
                    def native_response(arguments, **kwargs):
                        value = read(endpoint, timeout=timeout)
                        body = json.dumps(value, sort_keys=True, allow_nan=False)
                        etag = '"' + hashlib.sha256(body.encode("utf-8")).hexdigest() + '"'
                        matching = [arguments[index + 1] for index, argument in enumerate(arguments[:-1]) if argument == "-H"]
                        unchanged = matching == ["If-None-Match: " + etag]
                        status = 304 if unchanged else 200
                        self.account(endpoint, status=status)
                        return SimpleNamespace(returncode=0, stdout=f"HTTP/2 {status} {'Not Modified' if unchanged else 'OK'}\netag: {etag}\n\n" + ("" if unchanged else body), stderr="")
                    with patch("subprocess.run", side_effect=native_response):
                        return super().json(endpoint, timeout=timeout)
                self.primary_reads += 1
                self.account(endpoint)
                return read(endpoint, timeout=timeout)
            def archive(self, endpoint, *, timeout):
                self.account(endpoint, status=302)
                self.budget.charge()
                storage_requests.append((virtual_clock[0], os.environ.get("COHORT_MODE", "raw-filter"), "helper"))
                return archives[int(endpoint.split("/")[-2])]
            def account(self, endpoint, *, status=200):
                if not advance_pacing_clock:
                    return
                mode = os.environ.get("COHORT_MODE", "raw-filter-" + raw_filter_phase)
                principal = "App" if mode in {"ack-members", "raw-filter-early"} else "GITHUB_TOKEN"
                helper_requests.append({"epoch": virtual_clock[0], "function": mode,
                                        "endpoint": endpoint, "principal": principal, "status": status})
                if principal == "App" and status != 304:
                    app_request(mode, endpoint, component="helper")

        env = {"GITHUB_REPOSITORY": raw.repository, "GITHUB_RUN_ATTEMPT": "1", "WORKFLOW_SHA": raw.sha,
               "WORKFLOW_REF": raw.repository + "/.github/workflows/pr-governance.yml@refs/heads/master",
               "DEFAULT_BRANCH": "master", "GH_TOKEN": "fixture", "COHORT_ORIGINAL_ROOT_DEADLINE": "21200",
               "COHORT_VALID": "true", "COHORT_SENSOR_SOURCE": "true"}
        initial_directory = os.getcwd()
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, env), ExitStack() as clocks, \
                patch.object(cohort.time, "time", side_effect=lambda: virtual_clock[0]), patch.object(cohort, "_Transport", Transport):
            if advance_pacing_clock:
                # 同一clockを全ownerで継続し、epoch期限をmonotonic pacingと同時に進める。
                clocks.enter_context(patch.object(cohort.time, "monotonic", side_effect=lambda: virtual_clock[0]))
                clocks.enter_context(patch.object(cohort.time, "sleep", side_effect=advance_clock))
            def run(mode, owner, directory, extra=None):
                directory.mkdir(exist_ok=True)
                os.chdir(directory)
                output = directory / "outputs"
                output.write_text("")
                with patch.dict(os.environ, {"COHORT_MODE": mode, "GITHUB_RUN_ID": str(owner),
                                             "GITHUB_OUTPUT": str(output)} | (extra or {})):
                    cohort.main()
                values = dict(line.split("=", 1) for line in output.read_text().splitlines())
                if "cohort_read_attempts" in values:
                    self.assertLessEqual(int(values["cohort_read_attempts"]), 300 if mode in {"producer", "admission"} else 900)
                    role_reads.append((owner, mode, int(values["cohort_read_attempts"])))
                roles.append((owner, mode))
                return values

            try:
                if actual_sensor_loop:
                    for source_id, source in sources.items():
                        namespace = sensor_case.namespace({"HEAD_SHA": source["head_sha"], "PR_NUMBER": str(source["pull_requests"][0]["number"]),
                            "SOURCE_RUN_ID": str(source_id)})
                        exec(sensor_program, namespace)
                        sensor_namespaces[source_id] = namespace
                        sensor_tasks[source_id] = {"generator": namespace["await_native_sensor"](), "due": virtual_clock[0]}
                    advance_clock(0)
                for offset, source in enumerate(sources.values()):
                    owner = 10000 + offset
                    directory = Path(temporary) / str(owner)
                    hint = dict(raw.member, source_run_id=source["id"], source_run_number=source["run_number"],
                                pr_number=source["pull_requests"][0]["number"], head_sha=source["head_sha"],
                                pr_body_sha256=hashlib.sha256(source["pull_requests"][0]["body"].encode()).hexdigest())
                    run("producer", owner, directory, {"COHORT_MEMBER_HINT": json.dumps(hint)})
                    own_journals[owner] = upload(json.loads((directory / "cohort-journal/binding.json").read_text()), owner)
                    if offset:
                        if queue == "single":
                            cancelled.extend(pending)
                            pending[:] = [owner]
                        elif len(pending) < 100:
                            pending.append(owner)
                        else:
                            cancelled.append(owner)
                for owner in cancelled:
                    runs[owner].update(status="completed", conclusion="failure")
                    run_jobs[owner][1].update(status="completed", conclusion="cancelled")
                    for name in cohort.MUTATING_JOBS | {cohort.ADMISSION_JOB}:
                        native_job = next(job for job in run_jobs[owner] if job["name"] == name)
                        native_job.update(status="completed", conclusion="failure" if name == cohort.ADMISSION_JOB else "skipped")
                scheduled = [10000, *pending]
                while scheduled and (len(acknowledged) < total or drain_pending):
                    owner = scheduled.pop(0)
                    run_jobs[owner][1].update(status="in_progress", conclusion=None)
                    owners.append(owner)
                    directory, own = Path(temporary) / str(owner), own_journals[owner]
                    run("select", owner, directory, {"COHORT_OWN_JOURNAL_ID": str(own["artifact_id"]),
                                                    "COHORT_OWN_JOURNAL_DIGEST": own["artifact_digest"]})
                    state = json.loads((directory / "cohort-selection/state.json").read_text())
                    cohort_sizes.append(len(state["member_ids"]))
                    if not drain_pending:
                        self.assertTrue(state["member_ids"])
                    self.assertLessEqual(len(state["member_ids"]), 200)
                    self.assertFalse(acknowledged.intersection(state["member_ids"]))
                    native = {}
                    for kind in ("members", "aliases", "batch"):
                        for index in range(1, state["batch_count"] + 1):
                            reference = upload(json.loads((directory / f"cohort-selection/{kind}-{index}/binding.json").read_text()), owner, index)
                            native[f"COHORT_{kind.upper()}_{index}_ID"] = str(reference["artifact_id"])
                            native[f"COHORT_{kind.upper()}_{index}_DIGEST"] = reference["artifact_digest"][7:]
                    run("build-header", owner, directory, native)
                    header_payload = json.loads((directory / "cohort-header/binding.json").read_text())
                    header = upload(header_payload, owner)
                    binding = f"owner={owner} artifact={header['artifact_id']} digest={header['artifact_digest']}"
                    run_jobs[owner][1]["steps"].append(dict(step(cohort.SELECTED_PREFIX + binding), number=99))
                    admitted = run("admission", owner, directory)
                    self.assertEqual(json.loads(admitted["source_ids"]), state["member_ids"])
                    self.assertEqual(admitted["root_deadline_epoch"], "21200")
                    admission_job = next(job for job in run_jobs[owner] if job["name"] == cohort.ADMISSION_JOB)
                    admission_job.update(status="completed", conclusion="success",
                        steps=[dict(step(cohort.ADMITTED_PREFIX + binding), number=1)])
                    if enforce_check_writes and state["member_ids"]:
                        registration_steps = []
                        writer = writer_fixtures.WRITER
                        for index in range(1, state["batch_count"] + 1):
                            identifier = owner * 10 + index
                            title = writer.writer_run_title(str(owner), "early", str(index))
                            writer_runs[identifier] = dict(raw.run, id=identifier, name=title, display_title=title,
                                repository=repo, head_repository=repo, path=writer.WRITER_WORKFLOW_PATH,
                                event="workflow_dispatch", status="in_progress", conclusion=None)
                            registration_steps.append(dict(step(cohort.REGISTER_PREFIX +
                                f"segment={index} run={identifier} artifact={header['artifact_id']} digest={header['artifact_digest']}"), number=index))
                        service_job = next(job for job in run_jobs[owner] if job["name"] == "Service bound cohort review sources")
                        service_job.update(status="in_progress", conclusion=None, steps=registration_steps)
                        if cold_process:
                            next(job for job in run_jobs[owner] if job["name"] == writer.RESOLVER_FAILURE_BARRIER_NAME).update(status="completed", conclusion="success")
                    def probe_writer(phase):
                        writer = writer_fixtures.WRITER
                        native_charge = writer.cohort_charge
                        native_api, native_object_page, native_command, native_read = writer.api_json, writer.object_page, writer.command, writer.cohort_read_json
                        def app_endpoint(endpoint, method="GET"):
                            if advance_pacing_clock and installation_requests and installation_requests[-1]["endpoint"] is None:
                                installation_requests[-1].update(endpoint=endpoint, method=method)
                        def charged(*, default_token=False):
                            if not advance_pacing_clock or default_token:
                                return native_charge(default_token=default_token)
                            native_charge(default_token=False)
                            if conditional_writer_transport:
                                return
                            frame, function = inspect.currentframe().f_back, "unknown"
                            while frame is not None:
                                if frame.f_code.co_filename == writer.__file__ and frame.f_code.co_name != "charge":
                                    function = frame.f_code.co_name
                                    break
                                frame = frame.f_back
                            app_request(function)
                        def app_read(endpoint, *, timeout):
                            app_endpoint(endpoint)
                            return read(endpoint, timeout=timeout)
                        def app_archive(endpoint, *, timeout):
                            if conditional_writer_transport:
                                app_request("cohort_read_archive", endpoint)
                                writer_http_attempts.append({"epoch": virtual_clock[0], "method": "GET", "status": 302, "principal": "App"})
                            app_endpoint(endpoint)
                            if writer._cold_native_observations is not None:
                                native_charge(default_token=False)
                            # productionと同じstorage attemptはrole予算だけを消費する。
                            native_charge(default_token=False)
                            storage_requests.append((virtual_clock[0], phase, "writer"))
                            return archives[int(endpoint.split("/")[-2])]
                        context = cohort.load_cohort(header["artifact_id"], header["artifact_digest"],
                            expected_owner=owner, expected_segment=None, read_json=read,
                            read_archive=lambda endpoint, timeout: archives[int(endpoint.split("/")[-2])],
                            budget=cohort.ReadBudget(900), deadline=21200, allow_empty=not state["member_ids"])
                        def api(endpoint, *, default_token=False):
                            writer.cohort_charge(default_token=default_token)
                            if not default_token:
                                app_endpoint(endpoint)
                            elif advance_pacing_clock:
                                helper_requests.append({"epoch": virtual_clock[0], "function": "writer-default",
                                                        "endpoint": endpoint, "principal": "GITHUB_TOKEN"})
                            return read(endpoint, timeout=20)
                        def command(arguments, **kwargs):
                            if not kwargs.get("cohort_precharged", False):
                                writer.cohort_charge(default_token=kwargs.get("default_token", False))
                            if arguments[0] == "--method":
                                self.assertTrue(enforce_check_writes)
                                method, endpoint = arguments[1:3]
                                app_endpoint(endpoint, method)
                                fields = dict(argument.split("=", 1) for argument in arguments[4::2])
                                if method == "POST":
                                    self.assertEqual(endpoint, prefix + "/check-runs")
                                    identifier = 50000 + len(checks)
                                    checks[identifier] = {"id": identifier, "name": fields["name"],
                                        "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                                        "app": {"id": check_app_id}, "conclusion": None, "updated_at": "1970-01-01T00:03:20Z",
                                        "started_at": native_timestamp(), "completed_at": None}
                                else:
                                    self.assertEqual(method, "PATCH")
                                    identifier = int(endpoint.rsplit("/", 1)[1])
                                checks[identifier].update({key: fields[key] for key in ("status", "conclusion", "details_url") if key in fields})
                                checks[identifier]["output"] = {key: fields["output[" + key + "]"] for key in ("title", "summary") if "output[" + key + "]" in fields}
                                if fields.get("status") == "completed":
                                    checks[identifier]["completed_at"] = native_timestamp()
                                if advance_pacing_clock:
                                    checks[identifier]["updated_at"] = native_timestamp()
                                    mutation_times.append((virtual_clock[0], method, fields["status"]))
                                return json.dumps(checks[identifier])
                            self.assertEqual(len(arguments), 1)
                            self.assertTrue(arguments[0].startswith(prefix))
                            app_endpoint(arguments[0])
                            return json.dumps(read(arguments[0], timeout=20))
                        def native_response(arguments, **options):
                            self.assertEqual(arguments[:2], ["gh", "api"])
                            request = arguments[2:]
                            endpoint = next(value for value in request if value.startswith(prefix))
                            method = request[request.index("--method") + 1] if "--method" in request else "GET"
                            default = options["env"]["GH_TOKEN"] == "fixture-default"
                            if method != "GET":
                                app_request("write_check", endpoint, method)
                                writer_http_attempts.append({"epoch": virtual_clock[0], "method": method, "status": 201, "principal": "App"})
                                return SimpleNamespace(returncode=0, stdout=command(request, cohort_precharged=True), stderr="")
                            value = read(endpoint, timeout=options["timeout"])
                            body = json.dumps(value, sort_keys=True, allow_nan=False)
                            tag = '"' + hashlib.sha256(body.encode("utf-8")).hexdigest() + '"'
                            headers = [request[index + 1] for index, argument in enumerate(request[:-1]) if argument == "-H"]
                            unchanged = "If-None-Match: " + tag in headers
                            status = 304 if unchanged else 200
                            writer_http_attempts.append({"epoch": virtual_clock[0], "method": method, "status": status,
                                                         "principal": "GITHUB_TOKEN" if default else "App"})
                            if default:
                                helper_requests.append({"epoch": virtual_clock[0], "function": "writer-default",
                                    "endpoint": endpoint, "principal": "GITHUB_TOKEN", "status": status})
                            elif status != 304:
                                frame, function = inspect.currentframe().f_back, "unknown"
                                while frame is not None:
                                    if frame.f_code.co_filename == writer.__file__ and frame.f_code.co_name not in {"command", "api_json", "object_page", "cohort_read_json"}:
                                        function = frame.f_code.co_name
                                        break
                                    frame = frame.f_back
                                app_request(function, endpoint)
                            if "--include" in request:
                                body = f"HTTP/2.0 {status} {'Not Modified' if unchanged else 'OK'}\r\nETag: {tag}\r\n\r\n" + ("" if unchanged else body)
                            return SimpleNamespace(returncode=1 if unchanged else 0, stdout=body, stderr="")
                        extra = {"GITHUB_ACTIONS": "true", "GITHUB_SHA": raw.sha, "GITHUB_REF_NAME": "master",
                            "GOVERNANCE_DISPATCHER_RUN_ID": str(owner), "GOVERNANCE_SCOPE": phase,
                            "GOVERNANCE_COHORT_ARTIFACT_ID": str(header["artifact_id"]),
                            "GOVERNANCE_COHORT_ARTIFACT_DIGEST": header["artifact_digest"],
                            "GH_TOKEN": "fixture-app-read", "DEFAULT_READ_TOKEN": "fixture-default", "CHECK_WRITE_TOKEN": "fixture-app-write",
                            "KRR_GOVERNANCE_CHECK_APP_ID": str(check_app_id)}
                        elapsed = virtual_clock if advance_pacing_clock else [200.0]
                        with ExitStack() as writer_patches:
                            writer_patches.enter_context(patch.dict(os.environ, extra))
                            writer_patches.enter_context(patch.multiple(writer, REPOSITORY=raw.repository,
                            SERVER_URL="https://github.com", WRITER_RUN_ID=str(owner), _cohort_context=context,
                            _terminal_deadline_monotonic=(21200 if advance_pacing_clock else writer.time.monotonic() + 21000),
                            _cohort_api_counts={"default": 0, "app": 0}, _cohort_alias_index=None,
                            _cohort_covered_generations={}, _cohort_inventory_noop_generations={},
                            _cohort_pending_generations={}, _cohort_pending_exemptions=frozenset(),
                            _cohort_cold_publication_guards={}, _cohort_members_by_target={}, _early_writer_admission=None,
                            _cold_native_observations=None, _cohort_member_grant_code_proof=None,
                            _cohort_member_grant_check_runs=set(),
                            _conditional_json_reads=conditional_writer_transport, _conditional_read_cache={},
                            _conditional_read_counts=dict.fromkeys(writer._conditional_read_counts, 0),
                            _cohort_generation_identities={}, _nonreconciling_dispatcher_generations={}))
                            for name, value in {"api_json": native_api if conditional_writer_transport else api,
                                    "object_page": native_object_page if conditional_writer_transport else api,
                                    "command": native_command if conditional_writer_transport else command,
                                    "cohort_charge": charged, "cohort_read_json": native_read if conditional_writer_transport else app_read,
                                    "cohort_read_archive": app_archive}.items():
                                writer_patches.enter_context(patch.object(writer, name, value))
                            writer_patches.enter_context(patch.object(writer.subprocess, "run", side_effect=native_response))
                            writer_patches.enter_context(patch.object(writer, "cohort_helper", return_value=cohort))
                            if not cold_process or phase == "all":
                                try:
                                    writer.reject_newer_dispatcher_barrier("c" * 40)
                                    outcome = "allowed"
                                except writer.NoPostGovernanceError:
                                    outcome = "preempted"
                                    if phase == "early":
                                        raise
                                writer_barriers.append((owner, phase, outcome, writer._cohort_api_counts["app"]))
                            if phase == "early" and enforce_check_writes:
                                with patch.object(writer.time, "monotonic", side_effect=lambda: elapsed[0]), \
                                        patch.object(writer.time, "sleep", side_effect=advance_clock if actual_sensor_loop else lambda duration: elapsed.__setitem__(0, elapsed[0] + duration)), \
                                        patch.object(writer, "_last_check_write_at", None), \
                                        patch.object(writer, "_bound_check_runs", {}):
                                    previous_segment = None
                                    for position, source_id in enumerate(state["member_ids"]):
                                        segment = int(state["member_segments"][position])
                                        if previous_segment is not None and previous_segment != segment:
                                            # segmentは独立processであり、読取予算やnative証拠cacheを共有しない。
                                            writer._cohort_api_counts = {"default": 0, "app": 0}
                                            writer._cohort_alias_index = None
                                            writer._cohort_covered_generations = {}
                                            writer._cohort_inventory_noop_generations = {}
                                            writer._cohort_pending_generations = {}
                                            writer._cohort_pending_exemptions = frozenset()
                                            writer._cohort_generation_identities = {}
                                            writer._nonreconciling_dispatcher_generations = {}
                                            writer._cold_native_observations = None
                                            writer._cohort_member_grant_code_proof = None
                                            writer._cohort_member_grant_check_runs = set()
                                            writer._conditional_read_cache = {}
                                            writer._conditional_read_counts = dict.fromkeys(writer._conditional_read_counts, 0)
                                        previous_segment = segment
                                        writer_id = owner * 10 + segment
                                        with patch.object(writer, "WRITER_RUN_ID", str(writer_id)), \
                                                patch.dict(os.environ, {"GOVERNANCE_CONTINUATION_INDEX": str(segment)}):
                                            member = state["members"][str(source_id)]
                                            generations = (writer.Generation("CI", ".github/workflows/ci.yml", 10, 500000, 1, 1, "completed", "success"),
                                                writer.Generation("Release", ".github/workflows/release-preflight.yml", 11, 500001, 1, 1, "completed", "success"))
                                            # review/CI decisionは単体入力。実sensorはnative member/writer/clockを検証する。
                                            details = writer.target_url(source_run_id=source_id, generations=generations,
                                                base=member["base_sha"], head=member["head_sha"], body_sha256=member["pr_body_sha256"])
                                            head = sources[source_id]["head_sha"]
                                            try:
                                                if cold_process:
                                                    if writer._early_writer_admission is None or writer._early_writer_admission.job_id != writer_id * 100:
                                                        started = native_timestamp()
                                                        native_writer_job = {"id": writer_id * 100, "run_id": writer_id, "run_attempt": 1,
                                                            "run_url": "https://api.github.com/" + prefix + f"/actions/runs/{writer_id}",
                                                            "url": "https://api.github.com/" + prefix + f"/actions/jobs/{writer_id * 100}",
                                                            "head_sha": raw.sha, "head_branch": "master", "workflow_name": writer.WRITER_WORKFLOW_NAME,
                                                            "name": "Write authoritative governance Check Runs", "status": "in_progress", "conclusion": None,
                                                            "started_at": started, "steps": [{"number": 1, "status": "in_progress", "conclusion": None,
                                                                "started_at": started, "name": "Revalidate every current open pull request and publish fenced states"}]}
                                                        run_jobs[writer_id] = [native_writer_job]
                                                        segment_targets = tuple(state["members"][str(identifier)]["pr_number"]
                                                            for offset, identifier in enumerate(state["member_ids"])
                                                            if int(state["member_segments"][offset]) == segment)
                                                        writer.prepare_cohort_writer(writer.DispatcherSource(owner, "workflow_run", 1),
                                                            header["artifact_id"], header["artifact_digest"], segment, segment_targets, phase="early")
                                                    with patch.object(writer, "contract", return_value="success"), \
                                                            patch.object(writer, "sensor", return_value=source_id), \
                                                            patch.object(writer, "generation", side_effect=[*generations, *generations]), \
                                                            patch.object(writer, "final_closer_is_unique", return_value=True):
                                                        writer.process(member["pr_number"], {str(member["pr_number"] + 1000): frozenset({member["pr_number"]})}, "unit-decision-input")
                                                    terminal = writer.check_run(head)
                                                    if position == 0:
                                                        writer_barriers.append((owner, phase, "allowed", writer._cohort_api_counts["app"]))
                                                else:
                                                    pending_check = writer.write_check(head, state="in_progress",
                                                        description="Fixture decision pending", details_url=details)
                                                    terminal = writer.write_check(head, state="success",
                                                        description="Fixture decision success", details_url=details, existing=pending_check)
                                            except writer.NoPostGovernanceError as error:
                                                raise writer.NoPostGovernanceError(f"Actual writer early owner={owner} "
                                                    f"completed={len(written_sources)} checks={len(checks)} "
                                                    f"app={writer._cohort_api_counts['app']}/4500: {error}") from error
                                            self.assertEqual(terminal["conclusion"], "success")
                                            if not actual_sensor_loop:
                                                sensor_budget = cohort.ReadBudget(300)
                                                boundary = sensor_reader["sensor_cohort_binding"](terminal,
                                                    writer_runs[writer_id], sources[source_id], pulls[member["pr_number"]],
                                                    read_json=lambda endpoint, timeout: sensor_read(endpoint, timeout=timeout, source_id=source_id, budget=sensor_budget),
                                                    read_archive=lambda endpoint, timeout: sensor_archive(endpoint, timeout=timeout, source_id=source_id, budget=sensor_budget),
                                                    budget=sensor_budget, deadline=21200, workflow_blob=raw.blob)
                                                self.assertEqual(boundary, 5500)
                                                sensor_binding_reads.append((source_id, sensor_budget.used))
                                                written_sources.add(source_id)
                                                if advance_pacing_clock:
                                                    # sensorの完了はbatch ACKを待たず、検証済みterminal直後に起きる。
                                                    sources[source_id].update(status="completed", conclusion="success")
                                                    run_jobs[source_id][0].update(status="completed", conclusion="success")
                                                    run_jobs[source_id][0]["steps"][0].update(status="completed", conclusion="success",
                                                                                            completed_at=native_timestamp())
                                                    if include_completed_callbacks:
                                                        complete_callback(source_id)
                                            writer_read_counts[writer_id] = writer._cohort_api_counts["app"]
                    if enforce_writer_barrier:
                        probe_writer("early")
                    if actual_sensor_loop:
                        while any(identifier in sensor_tasks for identifier in state["member_ids"]):
                            advance_clock(min(task["due"] for task in sensor_tasks.values()) - virtual_clock[0])
                    for identifier in state["member_ids"]:
                        if enforce_check_writes:
                            self.assertIn(identifier, written_sources)
                        self.assertEqual(state["members"][str(identifier)]["source_deadline_epoch"], 5500)
                        sources[identifier].update(status="completed", conclusion="success")
                        run_jobs[identifier][0].update(status="completed", conclusion="success")
                        if not advance_pacing_clock:
                            run_jobs[identifier][0]["steps"][0].update(status="completed", conclusion="success",
                                                                       completed_at=native_timestamp())
                        reader = cohort._Reader(read, lambda endpoint, timeout: archives[int(endpoint.split("/")[-2])],
                                                cohort.ReadBudget(300), 21200)
                        self.assertTrue(cohort.completed_sensor_noop(sources[identifier], pulls[sources[identifier]["pull_requests"][0]["number"]],
                            reader=reader, workflow_sha=raw.sha, workflow_blob=raw.blob))
                        if include_completed_callbacks:
                            complete_callback(identifier)
                    for index in range(1, state["batch_count"] + 1):
                        result = run("ack-members", owner, directory, {"COHORT_SEGMENT": str(index),
                            "COHORT_ARTIFACT_ID": str(header["artifact_id"]), "COHORT_ARTIFACT_DIGEST": header["artifact_digest"]})
                        acknowledged.update(json.loads(result["acknowledged_source_ids"]))
                        ack_times.append((virtual_clock[0], owner, len(acknowledged)))
                    for name in ("Service bound cohort review sources", "Preserve pending review sensor before direct dispatch",
                                 "Preserve admitted sources before obsolete heavy preemption"):
                        existing = next((job for job in run_jobs[owner] if job["name"] == name), None)
                        if existing is None:
                            run_jobs[owner].append({"id": max(job["id"] for job in run_jobs[owner]) + 1, "run_id": owner,
                                "head_sha": raw.sha, "name": name, "status": "completed", "conclusion": "success", "steps": []})
                        else:
                            existing.update(status="completed", conclusion="success")
                    def probe_raw_filter(phase):
                        budget = cohort.ReadBudget(900)
                        transport = Transport("fixture", budget)
                        context = cohort.DispatcherCohortFilter(header["artifact_id"], header["artifact_digest"], owner,
                            read_json=transport.json, read_archive=transport.archive, budget=budget, deadline=21200, phase=phase)
                        try:
                            if phase == "early":
                                queries = [{"status": status, "branch": "master", "head_sha": raw.sha, "per_page": 100}
                                           for _ in range(2) for status in ("queued", "in_progress")]
                            else:
                                queries = [{"branch": "master", "head_sha": raw.sha,
                                            "created": ">=" + runs[owner]["created_at"], "per_page": 100}]
                            for query in queries:
                                endpoint = prefix + "/actions/workflows/pr-governance.yml/runs?" + urllib.parse.urlencode(query | {"page": 1})
                                result = context.page(endpoint, transport.json(endpoint, timeout=20))
                                self.assertLessEqual(result["total_count"], 100)
                            self.assertFalse(set(context.nonmutating).intersection(context.covered))
                            raw_filter_reads.append((owner, phase, budget.used))
                        except cohort.CohortError as error:
                            raise cohort.CohortError(f"Raw lifecycle phase={phase} owner={owner} selected={len(state['member_ids'])} "
                                f"cancelled={len(cancelled)} inventory={len(runs)} reads={budget.used}/900: {error}") from error
                    if enforce_raw_filter and raw_filter_phase == "early":
                        probe_raw_filter("early")
                    run("await-handoff", owner, directory, {"COHORT_ARTIFACT_ID": str(header["artifact_id"]),
                                                           "COHORT_ARTIFACT_DIGEST": header["artifact_digest"]})
                    run_jobs[owner][1]["steps"].append(dict(step(cohort.HANDOFF_PREFIX + binding), number=100))
                    run_jobs[owner][1].update(status="completed", conclusion="success")
                    reconcile = next(job for job in run_jobs[owner] if job["name"] == "Reconcile all current governance pull requests")
                    reconcile.update(status="in_progress", conclusion=None)
                    if scheduled:
                        # election mutexはhandoffで解放され、前ownerのall barrierと後継が重なる。
                        run_jobs[scheduled[0]][1].update(status="in_progress", conclusion=None)
                    if enforce_raw_filter and raw_filter_phase == "all":
                        probe_raw_filter("all")
                    if enforce_writer_barrier:
                        probe_writer("all")
                    runs[owner].update(status="completed", conclusion="success")
                    reconcile.update(status="completed", conclusion="success")
                    for native_job in run_jobs[owner]:
                        if native_job["status"] == "queued":
                            native_job.update(status="completed", conclusion="skipped")
                    self.assertEqual({identifier: archives[identifier] for identifier in original_source_archives}, original_source_archives)
            except (cohort.CohortError, writer_fixtures.WRITER.GovernanceError) as error:
                error.replay_metrics = {"epoch": virtual_clock[0], "monotonic": virtual_clock[0],
                    "owners": list(owners), "cohort_sizes": list(cohort_sizes),
                    "written": len(written_sources), "published_terminal": sum(value.get("conclusion") == "success" for value in checks.values()),
                    "acknowledged": len(acknowledged),
                    "mutations": list(mutation_times), "ack_times": list(ack_times),
                    "source_deadline_epoch": 5500, "root_deadline_epoch": 21200,
                    "expired_sources": sum(source["status"] != "completed" and virtual_clock[0] >= 5500 for source in sources.values()),
                    "initial_callback_count": total, "completed_callback_count": len(completed_callbacks),
                    "callback_inventory_count": len(runs), "installation_quota": installation_metrics(),
                    "helper_resources": helper_metrics()}
                raise
            finally:
                os.chdir(initial_directory)
        return {"acknowledged": acknowledged, "owners": owners, "cancelled": cancelled, "roles": roles,
                "cohort_sizes": cohort_sizes, "role_reads": role_reads,
                "raw_filter_reads": raw_filter_reads,
                "initial_callback_count": total, "completed_callback_count": len(completed_callbacks),
                "callback_inventory_count": len(runs),
                "writer_barriers": writer_barriers,
                "written_sources": written_sources,
                "writer_read_counts": writer_read_counts, "sensor_binding_reads": sensor_binding_reads,
                "virtual_epoch": virtual_clock[0], "mutation_times": mutation_times, "ack_times": ack_times,
                "sensor_completion_times": sensor_completion_times,
                "sensor_attempts": {source: namespace["api_read_count"] for source, namespace in sensor_namespaces.items()},
                "installation_quota": installation_metrics(),
                "helper_resources": helper_metrics(),
                "remaining": {identifier for identifier, source in sources.items() if source["status"] != "completed"}}

    def test_fifo_role_simulation_preserves_successor_for_401_and_600_sources(self) -> None:
        for total in (401, 600):
            with self.subTest(total=total):
                result = self.replay(total)
                self.assertEqual(result["acknowledged"], set(range(1000, 1000 + total)))
                self.assertFalse(result["remaining"])
                self.assertGreaterEqual(len(result["owners"]), 3)
                self.assertEqual(result["owners"], list(range(10000, 10000 + len(result["owners"]))))
                self.assertEqual(len(result["cancelled"]), total - 101)
                self.assertEqual(result["initial_callback_count"], total)
                self.assertEqual(result["completed_callback_count"], total)
                self.assertEqual(result["callback_inventory_count"], 2 * total)
                for owner in result["owners"]:
                    modes = [mode for identifier, mode in result["roles"] if identifier == owner]
                    self.assertEqual(modes[:4], ["producer", "select", "build-header", "admission"])
                    self.assertIn("ack-members", modes)
                    self.assertEqual(modes[-1], "await-handoff")

    def test_bounded_raw_600_native_callbacks_use_terminal_inventory_job_proof(self) -> None:
        # 新completed通知を除く容量対照であり、600originalの全lifecycle受入ではない。
        result = self.replay(600, enforce_raw_filter=True, include_completed_callbacks=False)
        self.assertEqual(result["callback_inventory_count"], 600)
        self.assertEqual(len(result["acknowledged"]), 600)
        self.assertGreaterEqual(len(result["owners"]), 3)
        self.assertLessEqual(max(reads for _, _, reads in result["raw_filter_reads"]), 900)

    def test_original_600_plus_native_completed_callbacks_exceeds_fixed_raw_cap(self) -> None:
        with self.assertRaisesRegex(cohort.CohortError, "inventory=620.*Raw dispatcher page incomplete"):
            self.replay(600, enforce_raw_filter=True)

    def test_bounded_200_both_notifications_native_writer_and_ack_liveness(self) -> None:
        result = self.replay(200, enforce_raw_filter=True, enforce_writer_barrier=True,
                             drain_pending=True, enforce_check_writes=True)
        self.assertEqual(result["written_sources"], set(range(1000, 1200)))
        self.assertEqual(result["acknowledged"], result["written_sources"])
        self.assertEqual(result["callback_inventory_count"], 400)
        self.assertGreaterEqual(sum(bool(size) for size in result["cohort_sizes"]), 3)
        old_all = [outcome for _, phase, outcome, _ in result["writer_barriers"][:-1] if phase == "all"]
        self.assertEqual(set(old_all), {"preempted"})
        self.assertEqual(result["writer_barriers"][-1][1:3], ("all", "allowed"))

    def test_real_pacing_401_and_600_expire_original_source_clock(self) -> None:
        # callback増加を除く容量対照でも期限不足となる。native transportの時間は0秒の下限対照。
        self.pacing_failures = {}
        for total, first_batch in ((401, 35), (600, 20)):
            with self.subTest(total=total):
                with self.assertRaisesRegex(writer_fixtures.WRITER.NoPostGovernanceError,
                                             "Actual writer early.*Dispatcher barrier evidence is invalid") as failure:
                    self.replay(total, enforce_raw_filter=True, enforce_writer_barrier=True,
                                enforce_check_writes=True, include_completed_callbacks=False,
                                advance_pacing_clock=True)
                metrics = failure.exception.replay_metrics
                self.pacing_failures[total] = metrics
                self.assertEqual(metrics["cohort_sizes"][0], first_batch)
                self.assertEqual(metrics["source_deadline_epoch"], 100 + 5400)
                self.assertEqual(metrics["root_deadline_epoch"], 200 + 21000)
                self.assertEqual(metrics["epoch"], metrics["monotonic"])
                self.assertGreaterEqual(metrics["epoch"], metrics["source_deadline_epoch"])
                self.assertLess(metrics["epoch"], metrics["source_deadline_epoch"] + 8.1)
                self.assertEqual(len(metrics["mutations"]), 2 * metrics["written"])
                for before, after in zip(metrics["mutations"], metrics["mutations"][1:]):
                    self.assertAlmostEqual(after[0] - before[0], 8.1)
                self.assertLess(metrics["mutations"][-1][0], metrics["source_deadline_epoch"])
                self.assertLess(metrics["acknowledged"], total)
                self.assertGreater(metrics["expired_sources"], 0)
                self.assertLess(metrics["ack_times"][-1][0], metrics["source_deadline_epoch"])
                self.assertGreater(total * 2 * writer_fixtures.WRITER.CHECK_WRITE_INTERVAL_SECONDS, 5400)

    def test_shared_installation_quota_stops_bounded_200_and_401_before_deadline(self) -> None:
        self.quota_failures = {}
        for total, callbacks in ((200, True), (401, True), (401, False)):
            with self.subTest(total=total, completed_callbacks=callbacks):
                with self.assertRaises((cohort.CohortError, writer_fixtures.WRITER.NoPostGovernanceError)) as failure:
                    self.replay(total, enforce_raw_filter=True, enforce_writer_barrier=True,
                                enforce_check_writes=True, include_completed_callbacks=callbacks,
                                advance_pacing_clock=True, enforce_installation_quota=True)
                metrics = failure.exception.replay_metrics
                self.quota_failures[total, callbacks] = metrics
                quota = metrics["installation_quota"]
                self.assertEqual(quota["app_total"], 4500)
                self.assertEqual(quota["app_rolling3600_peak"], 4500)
                self.assertEqual(quota["app_reads"] + quota["app_mutations"], 4500)
                self.assertEqual(set(quota["app_by_component"]), {"writer", "helper"})
                self.assertEqual(quota["app_rejected_epoch"], metrics["epoch"])
                self.assertEqual(metrics["source_deadline_epoch"], 5500)
                self.assertEqual(metrics["root_deadline_epoch"], 21200)
                self.assertLess(metrics["epoch"], 5500)
                self.assertLess(metrics["acknowledged"], total)
                self.assertGreater(metrics["helper_resources"]["github_token_reads"], 1000)
                self.assertEqual(metrics["callback_inventory_count"], total + metrics["completed_callback_count"])
                if not callbacks:
                    self.assertEqual(metrics["completed_callback_count"], 0)
                else:
                    self.assertGreater(metrics["completed_callback_count"], 0)

    def test_actual_grant_sensor_cooperative_poll_preserves_state_clock_and_role_budget(self) -> None:
        result = self.replay(1, enforce_raw_filter=True, enforce_writer_barrier=True,
            enforce_check_writes=True, advance_pacing_clock=True, enforce_installation_quota=True,
            conditional_helper_transport=True, cold_process=True, conditional_writer_transport=True, actual_sensor_loop=True)
        self.assertEqual(result["written_sources"], {1000})
        self.assertEqual(result["acknowledged"], {1000})
        self.assertEqual(result["virtual_epoch"], 320)
        self.assertEqual(result["sensor_completion_times"], [(320, 1000)])
        self.assertEqual(result["sensor_attempts"], {1000: 20})
        self.assertEqual(result["callback_inventory_count"], 2)
        self.assertEqual(result["helper_resources"]["github_token_primary_by_role"]["sensor-await"], 8)
        self.assertEqual(result["helper_resources"]["github_token_graphql_requests"], 2)
        self.assertEqual(result["helper_resources"]["github_token_graphql_primary_cost"], 2)
        self.assertEqual(result["helper_resources"]["github_token_graphql_secondary_points"], 2)
        self.assertTrue(result["writer_read_counts"])
        self.assertLess(max(result["writer_read_counts"].values()), 4500)

    def test_actual_conditional_transports_cold_20_native_posts_and_acks(self) -> None:
        result = self.replay(20, enforce_raw_filter=True, enforce_writer_barrier=True,
            enforce_check_writes=True, advance_pacing_clock=True, enforce_installation_quota=True,
            conditional_helper_transport=True, cold_process=True, conditional_writer_transport=True, actual_sensor_loop=True)
        self.assertEqual(result["written_sources"], set(range(1000, 1020)))
        self.assertEqual(result["acknowledged"], result["written_sources"])
        self.assertEqual(result["callback_inventory_count"], 40)
        self.assertEqual([(method, status) for _, method, status in result["mutation_times"]], [("POST", "completed")] * 20)
        self.assertGreaterEqual(result["virtual_epoch"], 200 + 20 * 8.1)
        self.assertLess(result["virtual_epoch"], 5500)
        self.assertGreater(result["installation_quota"]["writer_http_status_counts"][304], 0)
        self.assertGreater(result["helper_resources"]["helper_status_counts"][304], 0)
        self.assertLess(result["installation_quota"]["app_rolling3600_peak"], 4500)
        self.assertLess(result["helper_resources"]["github_token_rolling3600_peak"], 1000)

    def test_old_single_policy_exhausts_real_owners_before_overflow_ack(self) -> None:
        for total in (401, 600):
            with self.subTest(total=total):
                result = self.replay(total, policy="single")
                self.assertEqual(len(result["owners"]), 2)
                self.assertLess(len(result["acknowledged"]), total)
                self.assertTrue(result["remaining"])


if __name__ == "__main__":
    unittest.main()
