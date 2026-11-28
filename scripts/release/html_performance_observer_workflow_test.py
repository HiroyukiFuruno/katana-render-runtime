"""Verify the fixed observer is mandatory for fresh HTML performance captures."""
from __future__ import annotations

import unittest
from pathlib import Path

from local_macos_workflow_contract_test import workflow_job, workflow_steps


class ObserverWorkflowTest(unittest.TestCase):
    def test_v0424_uses_local_visual_evidence_on_pull_requests_only(self) -> None:
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/test-and-build.yml").read_text(encoding="utf8")
        job = workflow_job(workflow, "paired-performance")
        steps = {step.get("name"): step for step in workflow_steps(job)}
        proof = steps["Verify optional local HTML performance evidence"]
        self.assertEqual(
            proof.get("if"),
            "github.event_name == 'pull_request'",
        )
        proof_text = job.split("      - name: Verify optional local HTML performance evidence\n", 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn('if [[ "${{ steps.source.outputs.version }}" == \'0.4.24\' ]]; then', proof_text)
        self.assertIn("evidence_args+=(--visual)", proof_text)
        self.assertIn('"${evidence_args[@]}" --verify --pr', proof_text)
        self.assertNotIn('"${evidence_args[@]}" --verify-merged-head "$GITHUB_SHA"', proof_text)
        fallback = steps["Capture all 84 HTML catalog conditions"]["if"]
        self.assertEqual(
            steps["Verify all 84 HTML images and font identities for v0.4.24"]["if"],
            f"steps.source.outputs.version == '0.4.24' && ({fallback})",
        )
        report_text = job.split("      - name: Report local HTML performance evidence\n", 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn("HTML visual evidence reused from verified maintainer evidence.", report_text)
        self.assertIn("HTML performance reused from verified maintainer evidence.", report_text)

    def test_fresh_capture_uses_verified_pinned_observer_from_release_or_ci(self) -> None:
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
            'gh run download 44800513985 --repo "$GITHUB_REPOSITORY"',
            '--name krr-font-observer-v0.4.24-92102a2d --dir "$observer_work/staged"',
            'mv "$observer_work/staged/font-observer-macos-arm64.tar.gz" "$archive"',
            "scripts/release/html_performance_observer_unpack.py",
            "--archive-sha256 92102a2dad3ef4c5f50e5581c7f12a2c7b4e291ee3c3a2804299690811e8a86f",
            "--manifest-sha256 74594a04e993c89f969d6b02c42a84fe4a787d818a0bd339d7d96445380ba6dc",
            "--executable-sha256 031c93c5e822ffd8d22121bb41ed139fb6fcc4e96f6aa1f43ea31ec36fb9242c",
            '--output-dir "$observer_work/observer"',
        ):
            self.assertIn(required, observer_text)
        self.assertLess(
            observer_text.index("gh run download 44800513985"),
            observer_text.index("scripts/release/html_performance_observer_unpack.py"),
        )
        self.assertIn('just html-performance-catalog-captures "$output_root" \'\' \'\'', capture_text)
        self.assertIn('${{ steps.font_observer.outputs.executable }}', capture_text)
        self.assertIn("031c93c5e822ffd8d22121bb41ed139fb6fcc4e96f6aa1f43ea31ec36fb9242c true", capture_text)
        self.assertIn("runs-on: macos-latest", job)
        self.assertIn("needs: test", job)
        self.assertIn("actions: read", job)
        self.assertIn("contents: read", job)
        self.assertNotIn("contents: write", job)
        self.assertIn("attestations: write", job)
        self.assertIn("id-token: write", job)
        self.assertIn("needs: [test, paired-performance]", workflow_job(workflow, "preflight"))


if __name__ == "__main__":
    unittest.main()
