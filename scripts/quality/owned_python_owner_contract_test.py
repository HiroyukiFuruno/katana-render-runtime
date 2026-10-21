"""Moon owner AST contract regressions."""

from __future__ import annotations

import importlib.util
import ast
import argparse
import sys
import subprocess
import unittest
from pathlib import Path
from types import ModuleType
from typing import cast

SCRIPT = Path(__file__).with_name("owned_python_owner_contract.py")


def load_contract() -> ModuleType:
    spec = importlib.util.spec_from_file_location("owned_python_owner_contract", SCRIPT)
    if spec is None or spec.loader is None:
        raise AssertionError("owner AST contract module could not be loaded")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class MoonOwnerAstContractTests(unittest.TestCase):
    contract: ModuleType

    @classmethod
    def setUpClass(cls) -> None:
        cls.contract = load_contract()

    def setUp(self) -> None:
        self.owner_path = Path(__file__).with_name("moon_cache_contract_test.py")
        self.source = self.owner_path.read_text()

    def inspect(self, source: str) -> dict[str, object]:
        function = getattr(self.contract, "inspect_owner", None)
        if not callable(function):
            raise AssertionError("owner AST contract entry must be callable")
        result = function(source)
        if not isinstance(result, dict):
            raise AssertionError("owner AST contract must return an object")
        return result

    def scheduler_owner_phases(self) -> tuple[ModuleType, list[str]]:
        spec = importlib.util.spec_from_file_location("moon_cache_contract_test", self.owner_path)
        if spec is None or spec.loader is None:
            raise AssertionError("scheduler owner could not be loaded")
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        return module, list(module.SCHEDULER_EVENT_PHASES)

    def test_actual_owner_has_supported_entry_and_no_early_return(self) -> None:
        result = self.inspect(self.source)
        self.assertIs(result["valid"], True)
        self.assertEqual(result["contract"], "moon-owner-host-evidence-barrier-v3")

    def test_receipt_scope_distinguishes_fixture_subsets_from_full_run(self) -> None:
        owner, _ = self.scheduler_owner_phases()
        cases = (
            ("CAS subset", argparse.Namespace(cas_integrity_only=True, direct_inputs_only=False), "cas-integrity"),
            ("direct-input subset", argparse.Namespace(cas_integrity_only=False, direct_inputs_only=True), "direct-inputs"),
            ("full fixture", argparse.Namespace(cas_integrity_only=False, direct_inputs_only=False), "full"),
        )
        for label, args, expected in cases:
            with self.subTest(scope=label):
                self.assertEqual(owner.requested_scope(args), expected)

    def test_main_rejects_direct_early_success(self) -> None:
        mutated = self.source.replace(
            "def main() -> int:\n", "def main() -> int:\n    return 0\n", 1
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_main_rejects_conditional_early_success(self) -> None:
        mutated = self.source.replace(
            "def main() -> int:\n",
            "def main() -> int:\n    if True:\n        return 0\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_main_rejects_noop_control_wrapper(self) -> None:
        mutated = self.source.replace(
            "    parser = argparse.ArgumentParser(description=__doc__)\n",
            "    if False:\n        parser = argparse.ArgumentParser(description=__doc__)\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_main_rejects_success_exit_before_checks(self) -> None:
        mutated = self.source.replace(
            "    parser = argparse.ArgumentParser(description=__doc__)\n",
            "    raise SystemExit(0)\n    parser = argparse.ArgumentParser(description=__doc__)\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_main_guard_must_invoke_main(self) -> None:
        mutated = self.source.replace(
            "raise SystemExit(main())", "raise SystemExit(0)", 1
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_owner_invoke_rejects_early_success(self) -> None:
        mutated = self.source.replace(
            "        completed = subprocess.run(\n",
            "        return {}\n        completed = subprocess.run(\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_owner_invoke_rejects_conditional_bypass(self) -> None:
        mutated = self.source.replace(
            "        completed = subprocess.run(\n",
            "        if True:\n            return {}\n        completed = subprocess.run(\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_owner_miss_rejects_early_success(self) -> None:
        mutated = self.source.replace(
            "    def miss(label: str) -> None:\n",
            "    def miss(label: str) -> None:\n        return\n",
            1,
        )
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_owner_rejects_decorators_dead_calls_and_rebound_entry(self) -> None:
        mutations = [
            (
                "decorated-main",
                "def main() -> int:\n",
                "def skip_owner(function: object):\n    return lambda: 0\n\n\n@skip_owner\ndef main() -> int:\n",
            ),
            (
                "dead-subprocess",
                "        completed = subprocess.run(\n",
                "        completed = False and subprocess.run(\n",
            ),
            (
                "rebound-entry",
                'if __name__ == "__main__":',
                '__name__ = "not-main"\nif __name__ == "__main__":',
            ),
        ]
        for label, original, replacement in mutations:
            with self.subTest(label=label):
                self.assertEqual(self.source.count(original), 1)
                self.assertIs(
                    self.inspect(self.source.replace(original, replacement, 1))[
                        "valid"
                    ],
                    False,
                )

    def test_owner_rejects_bypassed_scheduler_failure_barrier(self) -> None:
        marker = "assert scheduler_failure_barrier_is_closed(failure_phases)"
        self.assertEqual(self.source.count(marker), 1)
        mutated = self.source.replace(marker, "assert True", 1)
        self.assertIs(self.inspect(mutated)["valid"], False)

    def test_owner_rejects_bypassed_acquisition_failure_barrier(self) -> None:
        marker = """assert all(
            phase not in acquisition_phases for phase in SCHEDULER_NATIVE_PHASES
        ), f"Consumer or native ran after failed acquisition: {acquisition_phases}"""
        self.assertEqual(self.source.count(marker), 1)
        self.assertIs(self.inspect(self.source.replace(marker, "assert True", 1))["valid"], False)

    def test_scheduler_projection_comes_from_reviewed_owner_literals(self) -> None:
        declarations = {
            node.target.id: ast.literal_eval(node.value)
            for node in ast.parse(self.source).body
            if isinstance(node, ast.AnnAssign)
            and isinstance(node.target, ast.Name)
            and node.value is not None
            and node.target.id in {"SCHEDULER_DEPENDENCIES", "SCHEDULER_TASK_OPTIONS"}
        }
        result = self.inspect(self.source)
        tasks = result["schedulerTasks"]
        self.assertIsInstance(tasks, dict)
        if not isinstance(tasks, dict):
            raise AssertionError("scheduler projection must be a dictionary")
        dependencies = declarations["SCHEDULER_DEPENDENCIES"]
        self.assertEqual(set(tasks), set(dependencies))
        for name, deps in dependencies.items():
            with self.subTest(task=name):
                self.assertEqual(tasks[name]["deps"], sorted(set(deps)))
                self.assertEqual(
                    tasks[name]["options"], declarations["SCHEDULER_TASK_OPTIONS"]
                )
        self.assertIsNone(tasks["static"]["leaf"])
        self.assertEqual(tasks["lint"]["leaf"], "lint")
        self.assertEqual(tasks["unit-test"]["deps"], ["runtime-asset-script-test"])
        self.assertEqual(
            tasks["runtime-asset-script-test"]["deps"],
            ["runtime-asset-script-other", "runtime-unused-script-regression"],
        )
        self.assertEqual(
            tasks["runtime-unused-script-regression"]["deps"],
            ["runtime-unused-rust-classic-producer"],
        )
        self.assertEqual(
            tasks["runtime-unused-fragment-loader-producer"]["deps"],
            ["runtime-classic-generation-evidence"],
        )
        self.assertEqual(
            tasks["runtime-asset-script-other"]["deps"],
            ["runtime-classic-generation-evidence"],
        )
        self.assertEqual(
            tasks["runtime-classic-generation-evidence"]["deps"],
            ["runtime-asset-script-preflight"],
        )
        self.assertEqual(
            tasks["runtime-asset-script-preflight"]["deps"],
            ["mathjax-owned-evidence"],
        )
        self.assertEqual(tasks["mathjax-owned-evidence"]["deps"], ["rust-facts-host-prebuild", "static"])
        self.assertEqual(tasks["rust-facts-host-prebuild"]["deps"], ["capacity-admission"])
        self.assertEqual(tasks["capacity-admission"]["deps"], ["source-static-foundation"])
        self.assertEqual(tasks["unit-test"]["leaf"], "unit-test")

    def test_scheduler_host_and_evidence_order_rejects_missing_duplicate_and_parallel_events(self) -> None:
        module, phases = self.scheduler_owner_phases()
        validate = module.scheduler_success_order_valid
        self.assertIs(validate(phases), True)
        phase_roles = {
            "static": "static",
            "host-preparation": "rust-facts-host-prebuild",
            "evidence": "mathjax-owned-evidence",
            "producer": "runtime-unused-fragment-loader-producer",
            "verify": "runtime-unused-script-regression",
            "other": "runtime-asset-script-other",
            "aggregate": "runtime-asset-script-test",
            "unit": "unit-test",
        }
        for role, phase in phase_roles.items():
            with self.subTest(role=role, phase=phase):
                self.assertIs(validate([item for item in phases if item != phase]), False)
                self.assertIs(validate([*phases, phase]), False)
                moved = [item for item in phases if item != phase]
                moved.insert(0, phase)
                self.assertIs(validate(moved), False)

    def test_foundation_format_cannot_follow_host_preparation(self) -> None:
        module, phases = self.scheduler_owner_phases()
        phases.remove("fmt-check")
        phases.append("fmt-check")
        self.assertIs(module.scheduler_success_order_valid(phases), False)

    def test_coverage_producers_require_disjoint_reports_and_local_receipts(self) -> None:
        module, _ = self.scheduler_owner_phases()
        valid = {
            "unit-test": {
                "command": ["just", "root-test-and-coverage"],
                "deps": ["runtime-asset-script-test"],
                "outputs": [
                    "tmp/quality/standalone-performance-coverage/root.json",
                    "tmp/quality/standalone-performance-coverage/root-local-result.json",
                ],
                "checks": [{
                    "check": "fingerprint", "script": "just native-coverage-cache-key root",
                    "hash": "stdout",
                }],
            },
            "coverage": {"command": ["just", "coverage-result-check"],
                         "deps": ["performance-coverage"]},
            "performance-coverage": {
                "command": ["just", "performance-harness-coverage"],
                "deps": ["heavy-native", "check"],
                "outputs": [
                    "tmp/quality/standalone-performance-coverage/baseline.json",
                    "tmp/quality/standalone-performance-coverage/candidate.json",
                    "tmp/quality/standalone-performance-coverage/performance-local-result.json",
                ],
                "checks": [{
                    "check": "fingerprint", "script": "just native-coverage-cache-key performance",
                    "hash": "stdout",
                }],
            },
        }
        module.validate_root_test_coverage_contract(valid)
        for task, field, replacement in [
            ("unit-test", "outputs", ["tmp/quality/standalone-performance-coverage/root.json"]),
            ("performance-coverage", "outputs", ["tmp/quality/standalone-performance-coverage/baseline.json"]),
            ("unit-test", "checks", [{"check": "fingerprint", "script": "true", "hash": "stdout"}]),
            ("performance-coverage", "checks", [{"check": "fingerprint", "script": "true", "hash": "stdout"}]),
        ]:
            changed = {name: dict(value) for name, value in
                       cast(dict[str, dict[str, object]], valid).items()}
            changed[task][field] = replacement
            with self.subTest(task=task, field=field):
                with self.assertRaises(AssertionError):
                    module.validate_root_test_coverage_contract(changed)

    def test_capacity_admission_cannot_follow_host_preparation(self) -> None:
        module, phases = self.scheduler_owner_phases()
        phases.remove("capacity-admission")
        host_index = phases.index("rust-facts-host-prebuild")
        phases.insert(host_index + 1, "capacity-admission")
        self.assertIs(module.scheduler_success_order_valid(phases), False)

    def test_bootstrap_failure_assertion_cannot_be_removed_or_made_optional(self) -> None:
        mutations = [
            (
                "blocked phase assertion",
                """assert all(
                phase not in bootstrap_failure_phases for phase in blocked_phases
            )""",
                "assert True",
            ),
            (
                "foundation failure",
                '''(
                "foundation",
                {"foundation_failure": True},''',
                '''(
                "foundation",
                {"foundation_failure": False},''',
            ),
            (
                "preparation failure",
                '''(
                "preparation",
                {"preparation_failure": True},''',
                '''(
                "preparation",
                {"preparation_failure": False},''',
            ),
        ]
        for label, original, replacement in mutations:
            with self.subTest(label=label):
                self.assertEqual(self.source.count(original), 1)
                changed = self.source.replace(original, replacement, 1)
                self.assertNotEqual(changed, self.source)
                self.assertIs(self.inspect(changed)["valid"], False)

    def test_changed_owner_option_has_no_scheduler_projection(self) -> None:
        original = '    "cache": False,\n'
        self.assertEqual(self.source.count(original), 1)
        result = self.inspect(self.source.replace(original, '    "cache": True,\n', 1))
        self.assertIs(result["valid"], False)
        self.assertIsNone(result["schedulerTasks"])

    def test_optimized_actual_owner_refuses_before_cli_or_fixture(self) -> None:
        completed = subprocess.run(
            [sys.executable, "-O", str(self.owner_path), "--help"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("Moon owner requires enabled Python assertions", completed.stderr)
        self.assertNotIn("usage:", completed.stdout)

    def test_owner_rejects_removed_debug_guard(self) -> None:
        guard = '    if not __debug__:\n        raise RuntimeError("Moon owner requires enabled Python assertions")\n'
        self.assertEqual(self.source.count(guard), 1)
        self.assertIs(self.inspect(self.source.replace(guard, "", 1))["valid"], False)

    def test_owner_accepts_comments_without_changing_ast(self) -> None:
        self.assertIs(
            self.inspect("# owner source annotation\n" + self.source)["valid"], True
        )


if __name__ == "__main__":
    unittest.main()
