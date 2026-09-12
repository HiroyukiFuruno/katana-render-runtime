from __future__ import annotations

import unittest
from pathlib import Path


class ReleaseWorkflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.release = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.preflight = (root / ".github/workflows/release-preflight.yml").read_text(encoding="utf-8")
        self.retry = (root / ".github/workflows/release-publish-retry.yml").read_text(encoding="utf-8")
        self.publisher = (root / "scripts/release/publish-crates.sh").read_text(encoding="utf-8")

    def test_release_target_check_requires_dependency_freshness_before_tagging(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        freshness = justfile.index("python3 scripts/release/verify_dependency_freshness.py")
        target = justfile.index("python3 scripts/release/verify-release-target.py")
        self.assertLess(freshness, target)
        self.assertIn('run: just VERSION="${{ steps.version.outputs.version }}" release-target-check', self.release)
        self.assertIn('run: just VERSION="${{ steps.version.outputs.version }}" release-target-check', self.preflight)

    def test_public_release_and_cleanup_follow_both_registry_publications(self) -> None:
        publish = self.release.index("- name: Publish crates.io")
        public = self.release.index("- name: Publish complete GitHub Release")
        cleanup = self.release.index("- name: Cleanup published release state")
        self.assertLess(publish, public)
        self.assertLess(public, cleanup)
        conditional = "if: github.event_name == 'pull_request' || inputs.publish_crates == true"
        self.assertGreaterEqual(self.release.count(conditional), 3)
        self.assertIn("publish_if_needed katana-render-runtime-cli\nwait_for_crate katana-render-runtime-cli", self.publisher)

    def test_retry_rejoins_the_same_publication_completion_path(self) -> None:
        publish = self.retry.index("- name: Publish crates.io from release tag")
        public = self.retry.index("- name: Publish complete GitHub Release")
        cleanup = self.retry.index("- name: Cleanup published release state")
        self.assertLess(publish, public)
        self.assertLess(public, cleanup)
        self.assertIn("contents: write", self.retry)
        retry_publish = self.retry[public:cleanup]
        self.assertIn('gh release edit "${TAG}" --repo "${GITHUB_REPOSITORY}" --draft=false --verify-tag', retry_publish)
        self.assertNotIn("--latest", retry_publish)
        self.assertIn("python3 scripts/release/cleanup_release_state.py", self.retry)


if __name__ == "__main__":
    unittest.main()
