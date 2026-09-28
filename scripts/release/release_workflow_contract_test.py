from __future__ import annotations

import unittest
import os
import subprocess
import tempfile
from pathlib import Path


class ReleaseWorkflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.release = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.preflight = (root / ".github/workflows/release-preflight.yml").read_text(encoding="utf-8")
        self.retry = (root / ".github/workflows/release-publish-retry.yml").read_text(encoding="utf-8")
        self.publisher = (root / "scripts/release/publish-crates.sh").read_text(encoding="utf-8")

    def test_release_checks_install_the_bun_resolver_before_freshness_validation(self) -> None:
        for job in (self.release[self.release.index("  release-context:"):self.release.index("  release:\n")], self.release[self.release.index("  release:\n"):]):
            self.assertIn("uses: oven-sh/setup-bun@v2", job)
            self.assertIn("bun-version: 1.4.2", job)
            self.assertIn("run: bun install --frozen-lockfile", job)

    def test_release_target_check_requires_dependency_freshness_before_tagging(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        freshness = justfile.index("python3 scripts/release/verify_dependency_freshness.py")
        target = justfile.index("python3 scripts/release/verify-release-target.py")
        self.assertLess(freshness, target)
        self.assertIn('run: just VERSION="${{ steps.version.outputs.version }}" release-target-check', self.release)
        self.assertIn("- name: Release quality gate", self.preflight)
        self.assertIn("timeout-minutes: 65", self.preflight)
        self.assertIn("run: just release-quality", self.preflight)
        self.assertIn("- name: Release-specific verification", self.preflight)
        self.assertIn("timeout-minutes: 35", self.preflight)
        self.assertIn('run: just VERSION="${{ steps.version.outputs.version }}" release-specific', self.preflight)

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

    def run_plantuml_install(self, failures: int, checksum: str) -> tuple[subprocess.CompletedProcess[str], Path, int]:
        root = Path(__file__).parents[2]
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        temp_path = Path(temp.name)
        fake_bin = temp_path / "bin"
        fake_bin.mkdir()
        attempts = temp_path / "attempts"
        curl = fake_bin / "curl"
        curl.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            f"attempts={attempts!s}\n"
            "count=0\n"
            "if [ -f \"$attempts\" ]; then count=$(cat \"$attempts\"); fi\n"
            "count=$((count + 1))\n"
            "printf '%s\\n' \"$count\" > \"$attempts\"\n"
            f"if [ \"$count\" -le {failures} ]; then exit 22; fi\n"
            "while [ \"$#\" -gt 0 ]; do\n"
            "  if [ \"$1\" = \"--output\" ]; then printf 'fixture' > \"$2\"; exit 0; fi\n"
            "  shift\n"
            "done\n"
            "exit 64\n",
            encoding="utf-8",
        )
        sha256sum = fake_bin / "sha256sum"
        sha256sum.write_text(
            "#!/usr/bin/env bash\n"
            f"printf '%s  %s\\n' '{checksum}' \"$1\"\n",
            encoding="utf-8",
        )
        curl.chmod(0o755)
        sha256sum.chmod(0o755)
        output = temp_path / "cache" / "plantuml.jar"
        environment = os.environ | {
            "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
            "KRR_PLANTUML_DOWNLOAD_RETRY_DELAY_SECONDS": "0",
        }
        completed = subprocess.run(
            ["just", "plantuml-install", "1.2026.8", str(output)],
            cwd=root,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        return completed, output, int(attempts.read_text(encoding="utf-8"))

    def test_plantuml_install_recovers_from_a_transient_http_failure(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=1,
            checksum="1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(attempts, 2)
        self.assertEqual(output.read_text(encoding="utf-8"), "fixture")
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_fails_closed_after_bounded_retries(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=3,
            checksum="1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 3)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_keeps_checksum_validation_after_recovery(self) -> None:
        completed, output, attempts = self.run_plantuml_install(failures=1, checksum="0" * 64)
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 2)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())


if __name__ == "__main__":
    unittest.main()
