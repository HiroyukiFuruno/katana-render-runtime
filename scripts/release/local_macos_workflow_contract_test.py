from __future__ import annotations

import importlib.util
import re
import tempfile
import unittest
from pathlib import Path

from draft_ci_contract_test import evaluate


ROOT = Path(__file__).parents[2]
CI_PATH = ROOT / ".github/workflows/test-and-build.yml"
EVIDENCE_PATH = ROOT / "scripts/release/local_macos_evidence.py"
EVIDENCE_SPEC = importlib.util.spec_from_file_location("local_macos_evidence", EVIDENCE_PATH)
assert EVIDENCE_SPEC and EVIDENCE_SPEC.loader
EVIDENCE = importlib.util.module_from_spec(EVIDENCE_SPEC)
EVIDENCE_SPEC.loader.exec_module(EVIDENCE)

def workflow_steps(workflow: str) -> list[dict[str, str]]:
    steps: list[dict[str, str]] = []
    current: list[str] = []
    step_section = workflow.split("    steps:\n", 1)[1]
    for line in step_section.splitlines():
        if line.startswith("      - "):
            if current:
                steps.append(parse_step(current))
            current = [line]
        elif current and (line.startswith("        ") or not line):
            current.append(line)
        elif current:
            steps.append(parse_step(current))
            current = []
    if current:
        steps.append(parse_step(current))
    return steps


def parse_step(lines: list[str]) -> dict[str, str]:
    step: dict[str, str] = {}
    marker = re.match(r"\s*-\s+(name|uses):\s*(.*)$", lines[0])
    if marker:
        step[marker.group(1)] = marker.group(2).strip()
    for line in lines:
        match = re.match(r"\s{8}(name|id|if|run|continue-on-error):\s*(.*)$", line)
        if match:
            step[match.group(1)] = match.group(2).strip()
    return step


def eval_step_if(expression: str, platform: str, os_name: str, outcome: str, reuse: str) -> bool:
    values = {
        "matrix.platform": platform,
        "matrix.os": os_name,
        "steps.mac_proof.outcome": outcome,
        "steps.mac_proof.outputs.reuse": reuse,
    }
    translated = re.sub(
        r"\b(?:matrix|steps)(?:\.[A-Za-z_][A-Za-z0-9_]*)+\b",
        lambda match: repr(values[match.group()]),
        expression,
    )
    translated = re.sub(r"\bgithub\.event_name\b", repr("pull_request"), translated)
    translated = translated.replace("success()", "true")
    translated = translated.replace("failure()", "false")
    translated = translated.replace("cancelled()", "false")
    return evaluate(translated, {})


class LocalMacOSWorkflowContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = CI_PATH.read_text(encoding="utf-8")
        cls.steps = workflow_steps(cls.workflow)

    def test_reuse_guard_truth_table_uses_actual_step_expressions(self) -> None:
        matrix = (("linux64", "ubuntu-latest"), ("mac-arm64", "macos-15"), ("win64", "windows-latest"))
        proof_states = (
            ("success", "true", True),
            ("success", "false", False),
            ("failure", "true", False),
            ("skipped", "true", False),
            ("", "true", False),
            ("success", "", False),
        )
        for step in self.steps:
            name = step.get("name", "")
            if name in {
                "Verify optional local macOS evidence",
                "Report reused local macOS evidence",
            } or step.get("uses", "").startswith("actions/checkout@"):
                continue
            expression = step.get("if", "true")
            baseline = eval_step_if(expression, "mac-arm64", "macos-15", "failure", "false")
            if not baseline:
                continue
            with self.subTest(step=name or step):
                for platform, os_name in matrix:
                    for outcome, reuse, should_run in proof_states:
                        original_run = eval_step_if(
                            expression, platform, os_name, "failure", "false"
                        )
                        expected = original_run and not (
                            platform == "mac-arm64" and should_run
                        )
                        self.assertEqual(
                            eval_step_if(expression, platform, os_name, outcome, reuse),
                            expected,
                            (name, platform, outcome, reuse),
                        )

    def test_mac_quality_steps_exactly_cover_the_evidence_command_map(self) -> None:
        mac_applicable_names = {
            step["name"]
            for step in self.steps
            if step.get("name")
            and any(
                eval_step_if(step.get("if", "true"), "mac-arm64", "macos-15", outcome, reuse)
                for outcome, reuse in (("failure", "false"), ("success", "true"))
            )
        }
        self.assertEqual(mac_applicable_names, set(EVIDENCE.MAC_WORKFLOW_STEP_COMMANDS))
        mac_quality_names = {
            name
            for name, command_ids in EVIDENCE.MAC_WORKFLOW_STEP_COMMANDS.items()
            if command_ids
        }
        self.assertTrue(mac_quality_names)
        mapped_commands = {
            command_id
            for command_ids in EVIDENCE.MAC_WORKFLOW_STEP_COMMANDS.values()
            for command_id in command_ids
        }
        self.assertEqual(mapped_commands, {command_id for command_id, _ in EVIDENCE.COMMANDS})

    def test_windows_review_process_tree_tests_run_only_on_windows(self) -> None:
        step = next(
            step for step in self.steps
            if step.get("name") == "Run Windows review process tree tests"
        )
        self.assertEqual(step.get("if"), "matrix.os == 'windows-latest'")
        self.assertEqual(
            step.get("run"),
            "python -m unittest discover -s scripts/hooks -p local_review_windows_job_test.py -v",
        )
        self.assertTrue(eval_step_if(step["if"], "win64", "windows-latest", "failure", "false"))
        self.assertFalse(eval_step_if(step["if"], "mac-arm64", "macos-15", "failure", "false"))

    def test_unknown_mac_quality_step_disables_evidence_scope_reuse(self) -> None:
        self.assertTrue(EVIDENCE.workflow_scope_supported(CI_PATH))
        injected_step = (
            "      - name: New Mac quality task\n"
            "        if: (matrix.platform != 'mac-arm64' || steps.mac_proof.outcome != 'success' || steps.mac_proof.outputs.reuse != 'true')\n"
            "        run: just new-quality-check\n\n"
        )
        mutated = self.workflow.replace("      - name: Run tests\n", injected_step + "      - name: Run tests\n", 1)
        self.assertNotEqual(mutated, self.workflow)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "workflow.yml"
            path.write_text(mutated, encoding="utf-8")
            self.assertFalse(EVIDENCE.workflow_scope_supported(path))

            extra_command = self.workflow.replace(
                "        run: just unit-test\n",
                "        run: just unit-test && just unreviewed-check\n",
                1,
            )
            self.assertNotEqual(extra_command, self.workflow)
            path.write_text(extra_command, encoding="utf-8")
            self.assertFalse(EVIDENCE.workflow_scope_supported(path))

    def test_proof_step_is_optional_and_report_remains_after_it(self) -> None:
        proof = next(step for step in self.steps if step.get("id") == "mac_proof")
        report = next(step for step in self.steps if step.get("name") == "Report reused local macOS evidence")
        self.assertEqual(proof.get("continue-on-error"), "true")
        self.assertIn("github.event_name == 'pull_request'", proof["if"])
        self.assertIn("--verify", self.workflow)
        self.assertIn("steps.mac_proof.outcome == 'success'", report["if"])
        self.assertIn("steps.mac_proof.outputs.reuse == 'true'", report["if"])

    def test_required_matrix_and_draft_job_gate_remain_intact(self) -> None:
        for entry in (
            "- os: ubuntu-latest\n            platform: linux64",
            "- os: macos-15\n            platform: mac-arm64",
            "- os: windows-latest\n            platform: win64",
        ):
            self.assertIn(entry, self.workflow)
        job = self.workflow.split("  test:\n", 1)[1].split("    runs-on:", 1)[0]
        self.assertIn("github.event.pull_request.draft == false", job)
        self.assertNotIn("needs:", self.workflow)
        self.assertNotIn("SKIP", self.workflow)


if __name__ == "__main__":
    unittest.main()
