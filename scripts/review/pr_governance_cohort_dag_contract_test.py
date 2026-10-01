from __future__ import annotations

import ast
import hashlib
import re
import textwrap
import unittest
from pathlib import Path


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


if __name__ == "__main__":
    unittest.main()
