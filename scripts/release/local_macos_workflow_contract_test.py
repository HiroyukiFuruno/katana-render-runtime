from __future__ import annotations

import importlib.util
import re
import tempfile
import unittest
from pathlib import Path
from typing import ClassVar

from draft_ci_contract_test import evaluate


ROOT = Path(__file__).parents[2]
CI_PATH = ROOT / ".github/workflows/test-and-build.yml"
EVIDENCE_PATH = ROOT / "scripts/release/local_macos_evidence.py"
EVIDENCE_SPEC = importlib.util.spec_from_file_location("local_macos_evidence", EVIDENCE_PATH)
assert EVIDENCE_SPEC and EVIDENCE_SPEC.loader
EVIDENCE = importlib.util.module_from_spec(EVIDENCE_SPEC)
EVIDENCE_SPEC.loader.exec_module(EVIDENCE)

def workflow_job(workflow: str, job_name: str) -> str:
    jobs = workflow.split("jobs:\n", 1)[1]
    marker = f"  {job_name}:\n"
    start = jobs.index(marker)
    remainder = jobs[start + len(marker) :]
    next_job = re.search(r"^  [A-Za-z0-9_-]+:\s*$", remainder, re.MULTILINE)
    return remainder[: next_job.start()] if next_job else remainder


def workflow_steps(job: str) -> list[dict[str, str]]:
    steps: list[dict[str, str]] = []
    current: list[str] = []
    step_section = job.split("    steps:\n", 1)[1]
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


def workflow_job_condition(job: str) -> str:
    lines = job.splitlines()
    condition = lines.index("    if: >-")
    expression: list[str] = []
    for line in lines[condition + 1 :]:
        if not line.startswith("      "):
            break
        expression.append(line.strip())
    return " ".join(expression)


def parse_step(lines: list[str]) -> dict[str, str]:
    step: dict[str, str] = {}
    marker = re.match(r"\s*-\s+(name|uses):\s*(.*)$", lines[0])
    if marker:
        step[marker.group(1)] = marker.group(2).strip()
    for index, line in enumerate(lines):
        match = re.match(r"\s{8}(name|id|if|run|continue-on-error):\s*(.*)$", line)
        if match:
            key, value = match.group(1), match.group(2).strip()
            if key == "if" and value in {">-", ">"}:
                folded: list[str] = []
                for continuation in lines[index + 1 :]:
                    if not continuation.startswith("          "):
                        break
                    folded.append(continuation.strip())
                value = " ".join(folded)
            step[key] = value
    return step


def eval_step_if(
    expression: str,
    platform: str,
    os_name: str,
    outcome: str,
    reuse: str,
    html_phase: str = "final",
    *,
    event_name: str = "pull_request",
    ref: str | None = None,
) -> bool:
    actual_ref = ref or (
        "refs/pull/123/merge" if event_name == "pull_request" else "refs/heads/master"
    )
    mac_job = platform == "mac-arm64" and event_name == "pull_request"
    linux_job = (
        platform == "linux64"
        and os_name == "ubuntu-latest"
        and event_name in {"push", "workflow_dispatch"}
        and actual_ref == "refs/heads/master"
    )
    values = {
        "matrix.platform": platform,
        "matrix.os": os_name,
        "steps.mac_proof.outcome": outcome if mac_job else "skipped",
        "steps.mac_proof.outputs.reuse": reuse if mac_job else "",
        "steps.html_policy.outputs.phase": html_phase,
        "steps.native_coverage_context.outcome": "success" if linux_job else "skipped",
        "steps.native_coverage_context.outputs.context": "context.json" if linux_job else "",
        "steps.native_coverage_context.outputs.key": "coverage-key" if linux_job else "",
        "steps.native_coverage_cache_restore.outcome": "success" if linux_job else "skipped",
        "steps.native_coverage_cache_restore.outputs.cache-hit": "false" if linux_job else "",
        "steps.native_coverage_cache_verify.outcome": "skipped",
        "steps.native_coverage_cache_verify.outputs.verified": "",
        "steps.coverage.outcome": "success" if linux_job else "skipped",
    }
    translated = re.sub(
        r"\b(?:matrix|steps)(?:\.[A-Za-z_][A-Za-z0-9_-]*)+\b",
        lambda match: repr(_required_condition_value(values, match.group())),
        expression,
    )
    environment: dict[str, object] = {
        "github": {
            "event_name": event_name,
            "ref": actual_ref,
            "event": {"pull_request": {"draft": False}},
        }
    }
    translated = translated.replace("success()", "true")
    translated = translated.replace("failure()", "false")
    translated = translated.replace("cancelled()", "false")
    return evaluate(translated, environment)


def _required_condition_value(values: dict[str, str], path: str) -> str:
    if path not in values:
        raise AssertionError(f"workflow condition uses an unmodeled context path: {path}")
    return values[path]


class LocalMacOSWorkflowContractTest(unittest.TestCase):
    workflow: ClassVar[str]
    test_job: ClassVar[str]
    steps: ClassVar[list[dict[str, str]]]

    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = CI_PATH.read_text(encoding="utf-8")
        cls.test_job = workflow_job(cls.workflow, "test")
        cls.steps = workflow_steps(cls.test_job)

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

    def test_condition_context_models_real_pr_and_skipped_linux_steps(self) -> None:
        self.assertTrue(
            eval_step_if(
                "github.event_name == 'pull_request' && github.ref == 'refs/pull/123/merge' "
                "&& steps.mac_proof.outcome == 'success' "
                "&& steps.mac_proof.outputs.reuse == 'true'",
                "mac-arm64",
                "macos-15",
                "success",
                "true",
            )
        )
        skipped_linux_only_steps = (
            "matrix.os != 'ubuntu-latest' "
            "&& steps.native_coverage_context.outcome == 'skipped' "
            "&& steps.native_coverage_cache_restore.outputs.cache-hit == '' "
            "&& steps.native_coverage_cache_verify.outputs.verified == '' "
            "&& steps.coverage.outcome == 'skipped'"
        )
        for platform, os_name in (("mac-arm64", "macos-15"), ("win64", "windows-latest")):
            with self.subTest(platform=platform):
                self.assertTrue(
                    eval_step_if(skipped_linux_only_steps, platform, os_name, "skipped", "")
                )
        pr_linux_context = (
            "github.event_name == 'pull_request' && github.ref == 'refs/pull/123/merge' "
            "&& steps.native_coverage_context.outcome == 'skipped' "
            "&& steps.native_coverage_context.outputs.key == '' "
            "&& steps.native_coverage_cache_restore.outcome == 'skipped' "
            "&& steps.coverage.outcome == 'skipped'"
        )
        self.assertTrue(
            eval_step_if(pr_linux_context, "linux64", "ubuntu-latest", "skipped", "")
        )
        master_linux_context = (
            "github.event_name == 'push' && github.ref == 'refs/heads/master' "
            "&& steps.native_coverage_context.outcome == 'success' "
            "&& steps.native_coverage_cache_restore.outputs.cache-hit == 'false' "
            "&& steps.native_coverage_cache_verify.outcome == 'skipped' "
            "&& steps.coverage.outcome == 'success'"
        )
        self.assertTrue(
            eval_step_if(
                master_linux_context,
                "linux64",
                "ubuntu-latest",
                "skipped",
                "",
                event_name="push",
                ref="refs/heads/master",
            )
        )
        with self.assertRaisesRegex(AssertionError, "unmodeled context path"):
            eval_step_if("steps.future_step.outcome == 'success'", "mac-arm64", "macos-15", "failure", "false")

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

    def test_html_evidence_uploads_follow_the_selected_phase(self) -> None:
        final_upload = next(
            step for step in self.steps
            if step.get("name") == "Upload independent HTML comparison evidence"
        )
        interim_upload = next(
            step for step in self.steps
            if step.get("name") == "Upload interim HTML comparison evidence"
        )
        for platform, os_name, phase, expected_final, expected_interim in (
            ("linux64", "ubuntu-latest", "final", True, False),
            ("linux64", "ubuntu-latest", "interim", False, True),
            ("mac-arm64", "macos-15", "final", False, False),
            ("win64", "windows-latest", "interim", False, False),
        ):
            with self.subTest(platform=platform, phase=phase):
                self.assertEqual(
                    eval_step_if(
                        final_upload["if"], platform, os_name, "failure", "false", phase
                    ),
                    expected_final,
                )
                self.assertEqual(
                    eval_step_if(
                        interim_upload["if"], platform, os_name, "failure", "false", phase
                    ),
                    expected_interim,
                )

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

    def test_source_quality_is_required_before_native_checks_and_independent_of_mac_proof(self) -> None:
        static_job = workflow_job(self.workflow, "static-quality")
        self.assertIn("    runs-on: ubuntu-latest\n", static_job)
        self.assertNotIn("    needs:", static_job)
        self.assertNotIn("continue-on-error:", static_job)
        self.assertNotIn("mac_proof", static_job)
        ready_condition = (
            "github.event_name != 'pull_request' || github.event.pull_request.draft == false"
        )
        self.assertEqual(workflow_job_condition(static_job), ready_condition)
        self.assertEqual(workflow_job_condition(self.test_job), ready_condition)
        self.assertIn("    needs: static-quality\n", self.test_job)
        self.assertNotIn("continue-on-error:", self.test_job.split("    steps:\n", 1)[0])

        static_steps = workflow_steps(static_job)
        registration = next(
            step for step in static_steps
            if step.get("name") == "Verify validation registration before scheduling checks"
        )
        checks = next(
            step for step in static_steps
            if step.get("name") == "Run independent source checks"
        )
        self.assertLess(static_steps.index(registration), static_steps.index(checks))
        self.assertEqual(registration.get("run"), "bun scripts/html/harness-contract-cli.ts")
        self.assertEqual(
            checks.get("run"),
            "bun run moon --concurrency 3 exec krr:source-static --on-failure continue --no-actions",
        )
        for step in static_steps:
            self.assertNotIn("if", step)
            self.assertNotIn("continue-on-error", step)
            for outcome, reuse in (("success", "true"), ("failure", "false")):
                with self.subTest(step=step.get("name", step), outcome=outcome):
                    self.assertTrue(
                        eval_step_if(step.get("if", "true"), "mac-arm64", "macos-15", outcome, reuse)
                    )
        for event, draft, expected in (
            ("pull_request", True, False),
            ("pull_request", False, True),
            ("push", True, True),
            ("workflow_dispatch", True, True),
        ):
            with self.subTest(event=event, draft=draft):
                environment: dict[str, object] = {
                    "github": {"event_name": event, "event": {"pull_request": {"draft": draft}}}
                }
                self.assertEqual(evaluate(workflow_job_condition(static_job), environment), expected)
                self.assertEqual(evaluate(workflow_job_condition(self.test_job), environment), expected)

    def test_source_quality_bypass_changes_fail_closed_for_mac_reuse(self) -> None:
        changes = (
            ("    needs: static-quality\n", "    needs: []\n"),
            (
                "      - name: Run independent source checks\n",
                "      - name: Run independent source checks\n"
                "        if: steps.mac_proof.outputs.reuse != 'true'\n",
            ),
            (
                "      - name: Run independent source checks\n",
                "      - name: Run independent source checks\n        continue-on-error: true\n",
            ),
            (
                "  test:\n    name: Test and Build\n    needs: static-quality\n    if: >-\n",
                "  test:\n    name: Test and Build\n    needs: static-quality\n    if: >-\n      always() &&\n",
            ),
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "workflow.yml"
            for before, after in changes:
                with self.subTest(change=after):
                    mutated = self.workflow.replace(before, after, 1)
                    self.assertNotEqual(mutated, self.workflow)
                    path.write_text(mutated, encoding="utf-8")
                    self.assertFalse(EVIDENCE.workflow_scope_supported(path))

    def test_previous_workflow_hash_cannot_satisfy_the_current_scope(self) -> None:
        previous_hashes = (
            "44d135c35f89f5865af191b12cead8c3f45749272268a12efd1394c3a290b36a",
            "a632de2fa6c15e435dd820bcdd0eca7f2f8c08d8ec7a5a6ecbd4055d55de8290",
            "b7226b9b6c7cc4d5b7e4e43d198fd934fff68e165d4f148202f9d1ba86ded672",
        )
        current_hash = EVIDENCE.workflow_digest(CI_PATH)
        self.assertTrue(EVIDENCE.workflow_scope_supported(CI_PATH))
        self.assertEqual(current_hash, EVIDENCE.SUPPORTED_WORKFLOW_SHA256)
        for previous_hash in previous_hashes:
            with self.subTest(previous_hash=previous_hash):
                self.assertNotEqual(current_hash, previous_hash)
                self.assertNotEqual(EVIDENCE.scope_digest(previous_hash), EVIDENCE.scope_digest(current_hash))

    def test_rust_quality_checks_precede_native_build_and_tests(self) -> None:
        names = [step.get("name", "") for step in self.steps]
        quality_checks = (
            "Check formatting",
            "Run Windows format check",
            "Check Rust types",
            "Run linter",
            "Run AST linter",
            "Check dependency boundary",
        )
        quality_positions = [names.index(name) for name in quality_checks]
        self.assertEqual(quality_positions, sorted(quality_positions))
        for name in (
            "Rebuild V8 static archive on macOS arm64",
            "Rebuild V8 static archive on Windows",
            "Run tests",
        ):
            with self.subTest(native_step=name):
                self.assertLess(max(quality_positions), names.index(name))

    def test_linux_imagemagick_installation_is_outside_mac_reuse_scope(self) -> None:
        install = next(
            step for step in self.steps
            if step.get("name") == "Install pinned ImageMagick for HTML comparison"
        )
        self.assertEqual(install.get("if"), "matrix.os == 'ubuntu-latest'")
        self.assertNotIn(install["name"], EVIDENCE.MAC_WORKFLOW_STEP_COMMANDS)
        self.assertNotIn("continue-on-error", install)
        for platform, os_name, expected in (
            ("linux64", "ubuntu-latest", True),
            ("mac-arm64", "macos-15", False),
            ("win64", "windows-latest", False),
        ):
            for outcome, reuse in (("success", "true"), ("failure", "false")):
                with self.subTest(platform=platform, outcome=outcome):
                    self.assertEqual(
                        eval_step_if(install["if"], platform, os_name, outcome, reuse), expected
                    )
        names = [step.get("name", "") for step in self.steps]
        self.assertLess(names.index(install["name"]), names.index("Run runtime asset script tests"))

    def test_required_matrix_and_draft_job_gate_remain_intact(self) -> None:
        for entry in (
            "- os: ubuntu-latest\n            platform: linux64",
            "- os: macos-15\n            platform: mac-arm64",
            "- os: windows-latest\n            platform: win64",
        ):
            self.assertIn(entry, self.workflow)
        self.assertIn("github.event.pull_request.draft == false", self.test_job)
        self.assertIn("    needs: static-quality\n", self.test_job)
        paired_job = workflow_job(self.workflow, "paired-performance")
        self.assertIn("needs: test", paired_job)
        preflight_job = workflow_job(self.workflow, "preflight")
        self.assertIn("needs: [test, paired-performance]", preflight_job)
        self.assertIn("needs.test.result == 'success'", preflight_job)
        self.assertIn("github.event.pull_request.draft == false", preflight_job)
        self.assertNotIn("SKIP", self.workflow)


if __name__ == "__main__":
    unittest.main()
