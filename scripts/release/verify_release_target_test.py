#!/usr/bin/env python3
"""Regression tests for the consecutive KRR release target guard."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("verify-release-target.py")
REQUIRED_COMMIT = "02a73d293c04f9635fd3a822ac865bf81d4c8745"


class VerifyReleaseTargetTests(unittest.TestCase):
    def run_check(self, target: str, latest: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                target,
                "--latest-version",
                latest,
                "--head-ref",
                REQUIRED_COMMIT,
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_v0420_after_published_v0419(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.19")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_allows_idempotent_retry_after_v0420(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.20")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_v0420_before_v0419_is_published(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.18")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_skipped_target(self) -> None:
        result = self.run_check("v0.4.21", "v0.4.19")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_old_target(self) -> None:
        result = self.run_check("v0.4.19", "v0.4.19")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
