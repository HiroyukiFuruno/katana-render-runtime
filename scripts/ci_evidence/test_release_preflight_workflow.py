from __future__ import annotations

import unittest
from pathlib import Path


WORKFLOW = Path(__file__).parents[2] / ".github/workflows/release-preflight.yml"


class ReleasePreflightWorkflowTests(unittest.TestCase):
    def test_evidence_check_polls_with_a_bounded_retry_budget(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        start = workflow.index("- name: Check trusted Linux CI evidence")
        end = workflow.index("\n      - name: Run shared release quality gate", start)
        evidence_step = workflow[start:end]

        self.assertIn("timeout-minutes: 110", evidence_step)
        self.assertIn("poll_max_attempts=120", evidence_step)
        self.assertIn("poll_interval_seconds=45", evidence_step)
        self.assertIn("while (( poll_attempt < poll_max_attempts ))", evidence_step)
        self.assertIn('sleep "$poll_interval_seconds"', evidence_step)
        self.assertIn('              10)', evidence_step)
        self.assertIn('              11)', evidence_step)
        self.assertIn('"decision"])' , evidence_step)
        self.assertIn(')" = rerun', evidence_step)
        self.assertIn(')" = pending', evidence_step)
        self.assertIn("Trusted evidence cannot be reused; running the complete Linux quality gate.", evidence_step)
        self.assertIn("Trusted evidence publisher remained pending after", evidence_step)
        self.assertIn("Trusted evidence consumer failed (exit $status)", evidence_step)

    def test_quality_gate_runs_only_after_polling_falls_back(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        start = workflow.index("- name: Run shared release quality gate")
        end = workflow.index("\n      - name: Release-specific verification", start)
        quality_step = workflow[start:end]

        self.assertIn(
            "steps.reusable_quality.outputs.reuse != 'true'",
            quality_step,
        )


if __name__ == "__main__":
    unittest.main()
