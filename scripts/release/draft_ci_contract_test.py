from __future__ import annotations

import ast
import importlib.util
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
PR_EVENTS = ["opened", "synchronize", "reopened", "ready_for_review", "converted_to_draft"]
VERIFIER_PATH = ROOT / "scripts/release/verify_ci_quality_evidence.py"
VERIFIER_SPEC = importlib.util.spec_from_file_location("ci_evidence_verifier", VERIFIER_PATH)
assert VERIFIER_SPEC and VERIFIER_SPEC.loader
VERIFIER = importlib.util.module_from_spec(VERIFIER_SPEC)
VERIFIER_SPEC.loader.exec_module(VERIFIER)


def evaluate(expression: str, environment: dict[str, object]) -> bool:
    def resolve(path: str) -> str:
        value: object = environment
        for part in path.split("."):
            if not isinstance(value, dict):
                raise AssertionError(f"invalid workflow context path: {path}")
            value = value[part]
        return repr(value)

    translated = re.sub(
        r"\b(?:github|needs)(?:\.[A-Za-z_][A-Za-z0-9_]*)+\b",
        lambda match: resolve(match.group()),
        expression,
    )
    translated = re.sub(r"\btrue\b", "True", translated)
    translated = re.sub(r"\bfalse\b", "False", translated)
    translated = translated.replace("&&", " and ").replace("||", " or ")
    translated = re.sub(r"(?<!!)!(?!=)", " not ", translated)
    parsed = ast.parse(translated, mode="eval")
    allowed = (
        ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp, ast.Not, ast.Compare,
        ast.Eq, ast.NotEq, ast.Constant, ast.Call, ast.Name, ast.Load,
    )
    if any(not isinstance(node, allowed) for node in ast.walk(parsed)):
        raise AssertionError(f"unsupported workflow condition syntax: {translated}")
    for node in ast.walk(parsed):
        if isinstance(node, ast.Call) and not isinstance(node.func, ast.Name):
            raise AssertionError(f"unsupported workflow condition function: {translated}")
        if isinstance(node, ast.Name) and node.id not in {"always", "startsWith", "contains"}:
            raise AssertionError(f"unsupported workflow condition name: {node.id}")
    return bool(
        eval(
            compile(parsed, "<workflow-if>", "eval"),
            {"__builtins__": {}},
            {
                "always": lambda: True,
                "startsWith": lambda value, prefix: str(value).startswith(str(prefix)),
                "contains": lambda value, item: str(item) in str(value),
            },
        )
    )


def workflow_job_if(path: str, job_id: str) -> str:
    lines = (ROOT / path).read_text(encoding="utf-8").splitlines()
    marker = f"  {job_id}:"
    start = lines.index(marker) + 1
    job_lines = []
    for line in lines[start:]:
        if line.startswith("  ") and not line.startswith("    "):
            break
        job_lines.append(line)
    condition = next(index for index, line in enumerate(job_lines) if line.strip() == "if: >-")
    expression = []
    for line in job_lines[condition + 1 :]:
        if line.startswith("      "):
            expression.append(line.strip())
        else:
            break
    return " ".join(expression)


def trigger_types(path: str) -> list[str]:
    lines = (ROOT / path).read_text(encoding="utf-8").splitlines()
    start = lines.index("  pull_request:")
    trigger = next(line.strip() for line in lines[start + 1 :] if line.startswith("    types:"))
    return re.findall(r"[a-z_]+", trigger.partition(":")[2])


def context(
    event_name: str,
    *,
    draft: bool = False,
    head_repo: str = "owner/repo",
    repository: str = "owner/repo",
    head_ref: str = "release/v1.2.3",
    push_message: str = "fix: correction",
    evidence_result: str = "success",
    action: str = "opened",
) -> dict[str, object]:
    return {
        "github": {
            "event_name": event_name,
            "event": {
                "action": action,
                "pull_request": {
                    "draft": draft,
                    "head": {"repo": {"full_name": head_repo}},
                },
                "head_commit": {"message": push_message},
            },
            "repository": repository,
            "head_ref": head_ref,
        },
        "needs": {"evidence": {"result": evidence_result}},
    }


class DraftCiContractTest(unittest.TestCase):
    ci_path = ".github/workflows/test-and-build.yml"
    preflight_path = ".github/workflows/release-preflight.yml"

    @classmethod
    def setUpClass(cls) -> None:
        cls.ci_test = workflow_job_if(cls.ci_path, "test")
        cls.evidence = workflow_job_if(cls.preflight_path, "evidence")
        cls.preflight = workflow_job_if(cls.preflight_path, "preflight")

    def test_pull_request_triggers_cover_ready_and_head_updates(self) -> None:
        self.assertEqual(trigger_types(self.ci_path), PR_EVENTS)
        self.assertEqual(trigger_types(self.preflight_path), PR_EVENTS)

    def test_ci_job_truth_table_preserves_draft_and_push_behavior(self) -> None:
        draft_events = ("opened", "synchronize", "converted_to_draft")
        ready_events = ("opened", "synchronize", "reopened", "ready_for_review")
        cases = tuple(
            (context("pull_request", draft=True, action=action), False)
            for action in draft_events
        ) + tuple(
            (context("pull_request", draft=False, action=action), True)
            for action in ready_events
        ) + (
            (context("push"), True),
            (context("push", push_message="Merge pull request #12 from owner/topic"), False),
            (context("push", push_message="fix: merged (#12)"), False),
        )
        for event, expected in cases:
            with self.subTest(event=event):
                self.assertEqual(evaluate(self.ci_test, event), expected)

        mutant = self.ci_test.replace(
            "(github.event_name != 'pull_request' || github.event.pull_request.draft == false) && ",
            "",
        )
        self.assertTrue(evaluate(mutant, context("pull_request", draft=True)))

    def test_evidence_and_preflight_truth_table_gates_draft_and_release_eligibility(self) -> None:
        draft_events = ("opened", "synchronize", "converted_to_draft")
        ready_events = ("opened", "synchronize", "reopened", "ready_for_review")
        evidence_cases = tuple(
            (context("pull_request", draft=True, action=action), False)
            for action in draft_events
        ) + tuple(
            (context("pull_request", draft=False, action=action), True)
            for action in ready_events
        ) + (
            (context("pull_request", head_repo="fork/repo"), False),
            (context("pull_request", head_ref="feature/work"), False),
            (context("workflow_dispatch"), False),
        )
        for event, expected in evidence_cases:
            with self.subTest(job="evidence", event=event):
                self.assertEqual(evaluate(self.evidence, event), expected)

        release_cases = (
            (context("pull_request", draft=True), False),
            (context("pull_request", draft=False), True),
            (context("pull_request", head_repo="fork/repo"), False),
            (context("pull_request", head_ref="feature/work"), False),
            (context("pull_request", evidence_result="failure"), False),
            (context("pull_request", evidence_result="skipped"), False),
            (context("workflow_dispatch"), True),
        )
        for event, expected in release_cases:
            with self.subTest(job="preflight", event=event):
                self.assertEqual(evaluate(self.preflight, event), expected)

        draft_guard = "github.event.pull_request.draft == false && "
        evidence_mutant = self.evidence.replace(draft_guard, "")
        preflight_mutant = self.preflight.replace(draft_guard, "")
        draft_event = context("pull_request", draft=True)
        self.assertTrue(evaluate(evidence_mutant, draft_event))
        self.assertTrue(evaluate(preflight_mutant, draft_event))

    def test_skipped_or_missing_ubuntu_job_cannot_satisfy_quality_evidence(self) -> None:
        skipped_job = {
            "run_id": 42,
            "name": VERIFIER.JOB_NAME,
            "status": "completed",
            "conclusion": "skipped",
            "steps": [],
        }
        with self.assertRaisesRegex(VERIFIER.EvidenceError, "completed unsuccessfully"):
            VERIFIER._verify_jobs({"total_count": 1, "jobs": [skipped_job]}, 42, ())
        with self.assertRaisesRegex(VERIFIER.EvidenceError, "expected exactly one Ubuntu CI job"):
            VERIFIER._verify_jobs({"total_count": 0, "jobs": []}, 42, ())


if __name__ == "__main__":
    unittest.main()
