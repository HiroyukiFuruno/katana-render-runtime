from __future__ import annotations

import ast
import re
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
AWAIT = "Await the captured review sensor terminal state"
ACK = "Require exact bound sensor acknowledgement"
DISPATCHER_AUDIT = "Stop only acknowledged obsolete heavy reconciliation"


def inline_program(workflow: str, name: str) -> ast.Module:
    match = re.search(
        r"- name: " + re.escape(name) + r".*?python3 - <<'PY'\n(.*?)\n          PY",
        workflow,
        re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"Missing sensor observer: {name}")
    return ast.parse(textwrap.dedent(match.group(1)))


def string_set(node, symbols):
    if isinstance(node, ast.Name):
        if node.id not in symbols:
            raise AssertionError(f"Unresolved status contract: {node.id}")
        return symbols[node.id]
    if isinstance(node, ast.Starred):
        return string_set(node.value, symbols)
    if isinstance(node, (ast.Set, ast.Tuple, ast.List)):
        result = set()
        for value in node.elts:
            if isinstance(value, ast.Constant) and isinstance(value.value, str):
                result.add(value.value)
            elif isinstance(value, ast.Starred):
                result.update(string_set(value, symbols))
            else:
                raise AssertionError("Status contract contains an unevaluated expression")
        return result
    raise AssertionError("Status contract is not a literal collection")


def literal_symbols(tree):
    result = {}
    status_domain = {"requested", "queued", "waiting", "pending", "in_progress", "completed"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and isinstance(node.value, (ast.Set, ast.Tuple, ast.List)):
            if all(isinstance(value, ast.Constant) and isinstance(value.value, str) for value in node.value.elts):
                for target in node.targets:
                    if isinstance(target, ast.Name):
                        values = string_set(node.value, {})
                        if not values or not values <= status_domain:
                            continue
                        if target.id in result and result[target.id] != values:
                            raise AssertionError(f"Ambiguous status symbol: {target.id}")
                        result[target.id] = values
    return result


def status_memberships(tree):
    symbols = literal_symbols(tree)
    result = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Compare) or len(node.ops) != 1 or not isinstance(node.ops[0], (ast.In, ast.NotIn)):
            continue
        left = node.left
        is_status = isinstance(left, ast.Name) and left.id == "status"
        if isinstance(left, ast.Call) and isinstance(left.func, ast.Attribute) and left.func.attr == "get":
            is_status = len(left.args) == 1 and isinstance(left.args[0], ast.Constant) and left.args[0].value == "status"
        if is_status:
            result.append(string_set(node.comparators[0], symbols))
    return result


class GovernanceSensorStateContractTest(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text()
        self.preflight = ast.parse((ROOT / "scripts/review/pr_governance_preflight.py").read_text())
        self.lifecycle = literal_symbols(self.preflight)["lifecycle"]
        self.active = self.lifecycle - {"completed"}

    def test_bound_sensor_producer_lifecycle_keeps_all_documented_states(self):
        self.assertEqual(self.lifecycle, {"requested", "queued", "waiting", "pending", "in_progress", "completed"})

    def test_initial_capture_covers_the_producer_active_domain(self):
        functions = [node for node in self.preflight.body if isinstance(node, ast.FunctionDef) and node.name == "active_local_review_sensors"]
        self.assertEqual(len(functions), 1)
        guards = status_memberships(functions[0])
        self.assertTrue(guards)
        for guard in guards:
            self.assertEqual(guard, self.active)

    def test_waiter_and_successor_observers_cover_the_producer_domain(self):
        guards = status_memberships(inline_program(self.workflow, AWAIT))
        self.assertGreaterEqual(len(guards), 3)
        for guard in guards:
            self.assertTrue(self.active <= guard)
            self.assertTrue(guard <= self.lifecycle)

    def test_acknowledgement_covers_the_same_producer_active_domain(self):
        guards = status_memberships(inline_program(self.workflow, ACK))
        self.assertEqual(guards, [self.active])

    def test_sensor_identity_observers_have_an_explicit_role_inventory(self):
        observers = set()
        for match in re.finditer(r"python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL):
            tree = ast.parse(textwrap.dedent(match.group(1)))
            if any(isinstance(node, ast.Constant) and node.value == "PR governance review sensor" for node in ast.walk(tree)):
                names = re.findall(r"- name: (.+)", self.workflow[:match.start()])
                self.assertTrue(names)
                observers.add(names[-1])
        # Dispatcherの完了ACK監査と、sensorを受動観測する経路は状態domainが異なる。
        self.assertEqual(observers, {AWAIT, ACK, DISPATCHER_AUDIT})


if __name__ == "__main__":
    unittest.main()
