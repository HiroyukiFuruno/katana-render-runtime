"""Verify the fixed observer is mandatory for fresh HTML performance captures."""
from __future__ import annotations

import unittest
from pathlib import Path

from local_macos_workflow_contract_test import workflow_job, workflow_steps


class ObserverWorkflowTest(unittest.TestCase):
    def test_fresh_capture_uses_verified_pinned_release_asset(self) -> None:
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/test-and-build.yml").read_text(encoding="utf8")
        job = workflow_job(workflow, "paired-performance")
        steps = workflow_steps(job)
        by_name = {step.get("name"): step for step in steps}
        fallback = "steps.html_local_proof.outcome != 'success' || steps.html_local_proof.outputs.reuse != 'true'"
        observer = by_name["Download and verify font observer"]
        capture = by_name["Capture all 84 HTML catalog conditions"]
        self.assertEqual(observer.get("if"), fallback)
        self.assertEqual(capture.get("if"), fallback)
        self.assertNotIn("continue-on-error", observer)
        self.assertLess(steps.index(by_name["Set up Python 3.13"]), steps.index(observer))
        self.assertLess(steps.index(observer), steps.index(capture))
        observer_text = job.split("      - name: Download and verify font observer\n", 1)[1].split("\n      - name:", 1)[0]
        capture_text = job.split("      - name: Capture all 84 HTML catalog conditions\n", 1)[1].split("\n      - name:", 1)[0]
        for required in (
            "GH_TOKEN: ${{ github.token }}",
            "set -euo pipefail",
            "mktemp -d",
            "repos/${GITHUB_REPOSITORY}/releases/assets/702068128",
            "Accept: application/octet-stream",
            "scripts/release/html_performance_observer_unpack.py",
            "--archive-sha256 92102a2dad3ef4c5f50e5581c7f12a2c7b4e291ee3c3a2804299690811e8a86f",
            "--manifest-sha256 74594a04e993c89f969d6b02c42a84fe4a787d818a0bd339d7d96445380ba6dc",
            "--executable-sha256 031c93c5e822ffd8d22121bb41ed139fb6fcc4e96f6aa1f43ea31ec36fb9242c",
            '--output-dir "$observer_work/observer"',
        ):
            self.assertIn(required, observer_text)
        self.assertIn('just html-performance-catalog-captures "$output_root" \'\' \'\'', capture_text)
        self.assertIn('${{ steps.font_observer.outputs.executable }}', capture_text)
        self.assertIn("031c93c5e822ffd8d22121bb41ed139fb6fcc4e96f6aa1f43ea31ec36fb9242c true", capture_text)
        self.assertIn("runs-on: macos-15", job)
        self.assertIn("needs: test", job)
        self.assertIn("attestations: write", job)
        self.assertIn("id-token: write", job)
        self.assertIn("needs: [test, paired-performance]", workflow_job(workflow, "preflight"))


if __name__ == "__main__":
    unittest.main()
