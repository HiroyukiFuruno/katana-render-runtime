from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import automation_release_test_identity as identity


class AutomationReleaseTestIdentityTest(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.environment = {"PATH": "/usr/bin", "HOME": "/tmp"}
        self.installed = "a" * 64
        (self.root / "node_modules").mkdir()

    def test_digest_is_private_and_stable(self) -> None:
        with patch.object(identity, "_source_state", return_value="a" * 64), patch.object(
            identity, "_quality_git", return_value="b" * 64
        ), patch.object(identity, "_executable_sha256", return_value="c" * 64):
            first = identity.input_digest(self.root, self.environment, self.installed)
            second = identity.input_digest(self.root, self.environment, self.installed)
        self.assertRegex(first or "", r"^[0-9a-f]{64}$")
        self.assertEqual(first, second)

    def test_missing_or_unsupported_identity_fails_closed_and_stable_env_is_bound(self) -> None:
        with patch.object(identity, "_source_state", return_value="a" * 64), patch.object(
            identity, "_quality_git", return_value="b" * 64
        ), patch.object(identity, "_executable_sha256", return_value="c" * 64):
            self.assertIsNone(identity.input_digest(self.root, self.environment, "invalid"))
            baseline = identity.input_digest(self.root, self.environment, self.installed)
            changed = identity.input_digest(
                self.root, {**self.environment, "UNKNOWN": "x"}, self.installed
            )
            self.assertNotEqual(baseline, changed)
            self.assertIsNone(identity.input_digest(
                self.root, {**self.environment, "LD_PRELOAD": "/tmp/lib.so"}, self.installed
            ))


if __name__ == "__main__":
    unittest.main()
