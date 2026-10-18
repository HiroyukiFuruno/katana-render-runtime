from __future__ import annotations

import os
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import native_coverage_local_aggregate as aggregate


class LocalCoverageAggregateUnitTests(unittest.TestCase):
    def test_authenticated_strict_success_skips_refresh(self) -> None:
        outputs = [subprocess.CompletedProcess[str]([], 0, '{"valid":true}', "")]
        with patch("native_coverage_local_aggregate.subprocess.run", side_effect=outputs) as run:
            row = aggregate.validate_or_refresh(Path("/fixture"), "root")
        self.assertTrue(row["valid"])
        self.assertFalse(row["refreshed"])
        self.assertIsNone(row["failure"])
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args_list[0].args[0][4], "local-validate")

    def test_authenticated_policy_failure_does_not_refresh(self) -> None:
        outputs = [
            subprocess.CompletedProcess[str]([], 1, '{"valid":false,"failure":"coverage-policy"}', ""),
        ]
        with patch("native_coverage_local_aggregate.subprocess.run", side_effect=outputs) as run:
            row = aggregate.validate_or_refresh(Path("/fixture"), "root")
        self.assertFalse(row["valid"])
        self.assertFalse(row["refreshed"])
        self.assertEqual(row["failure"], "coverage-policy")
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args_list[0].args[0][4], "local-validate")

    def test_invalid_collection_refreshes_then_requires_one_strict_readback(self) -> None:
        outputs = [
            subprocess.CompletedProcess[str]([], 1, '{"valid":false,"failure":"invalid-collection"}', ""),
            subprocess.CompletedProcess[str]([], 0, "", ""),
            subprocess.CompletedProcess[str]([], 0, '{"valid":true}', ""),
        ]
        with patch.dict(os.environ, {"MOON_TASK_ID": "coverage", "MOON_TASK_HASH": "cached-task"}), \
             patch("native_coverage_local_aggregate.subprocess.run", side_effect=outputs) as run:
            row = aggregate.validate_or_refresh(Path("/fixture"), "root")
        self.assertTrue(row["valid"])
        self.assertTrue(row["refreshed"])
        self.assertEqual(run.call_count, 3)
        self.assertEqual(run.call_args_list[0].args[0][4], "local-validate")
        self.assertEqual(run.call_args_list[1].args[0], ["just", "root-test-and-coverage"])
        self.assertEqual(run.call_args_list[2].args[0][4], "local-validate")
        self.assertNotIn("MOON_TASK_ID", run.call_args_list[1].kwargs["env"])

    def test_post_refresh_policy_failure_remains_a_policy_failure(self) -> None:
        outputs = [
            subprocess.CompletedProcess[str]([], 1, '{"valid":false,"failure":"invalid-collection"}', ""),
            subprocess.CompletedProcess[str]([], 0, "", ""),
            subprocess.CompletedProcess[str]([], 1, '{"valid":false,"failure":"coverage-policy"}', ""),
        ]
        with patch("native_coverage_local_aggregate.subprocess.run", side_effect=outputs) as run:
            row = aggregate.validate_or_refresh(Path("/fixture"), "root")
        self.assertFalse(row["valid"])
        self.assertTrue(row["refreshed"])
        self.assertEqual(row["failure"], "coverage-policy")
        self.assertEqual(run.call_count, 3)

    def test_unknown_producer_cannot_execute_an_arbitrary_recipe(self) -> None:
        with patch("native_coverage_local_aggregate.subprocess.run") as run, self.assertRaises(KeyError):
            aggregate.validate_or_refresh(Path("/fixture"), "unregistered")
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
