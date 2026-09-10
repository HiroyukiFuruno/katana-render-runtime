#!/usr/bin/env python3
"""Regression tests for the consecutive KRR release target guard."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("verify-release-target.py")
REQUIRED_COMMITS = (
    "02a73d293c04f9635fd3a822ac865bf81d4c8745",
    "8552c63457480379922c7076bcff604b5401ae20",
    "694ac82a85d555485e46eb46cf882c8db11b2fe5",
    "007ab829df39ed40bbfcfc19205ad21f3da32fe8",
    "91f07699b26567658f76021050ca7ec7b5c10df1",
)


class VerifyReleaseTargetTests(unittest.TestCase):
    def run_check(
        self, target: str, latest: str, head_ref: str = "HEAD"
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--target-version",
                target,
                "--latest-version",
                latest,
                "--head-ref",
                head_ref,
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

    def test_accepts_head_containing_every_v0420_issue_commit(self) -> None:
        result = self.run_check("v0.4.20", "v0.4.19")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_head_missing_a_v0420_issue_commit(self) -> None:
        for commit in REQUIRED_COMMITS[1:]:
            parent = subprocess.run(
                ["git", "rev-parse", f"{commit}^"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            result = self.run_check("v0.4.20", "v0.4.19", parent)
            self.assertNotEqual(result.returncode, 0, commit)


if __name__ == "__main__":
    unittest.main()
