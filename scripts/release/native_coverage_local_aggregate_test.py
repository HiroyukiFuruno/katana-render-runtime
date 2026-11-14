from __future__ import annotations

import io
import json
import os
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
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
            parent_environment = dict(os.environ)
        self.assertTrue(row["valid"])
        self.assertTrue(row["refreshed"])
        self.assertEqual(run.call_count, 3)
        self.assertEqual(run.call_args_list[0].args[0][4], "local-validate")
        self.assertEqual(run.call_args_list[1].args[0], ["just", "root-test-and-coverage"])
        self.assertEqual(run.call_args_list[2].args[0][4], "local-validate")
        for index in (0, 2):
            self.assertTrue(run.call_args_list[index].kwargs.get("env") == parent_environment)
        self.assertNotIn("MOON_TASK_ID", run.call_args_list[1].kwargs["env"])
        self.assertNotIn("MOON_TASK_HASH", run.call_args_list[1].kwargs["env"])

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

    def test_main_defaults_to_the_full_aggregate_in_existing_order(self) -> None:
        root = Path(__file__).resolve().parents[2]
        output = io.StringIO()
        rows = [
            {"producer": "root", "valid": True},
            {"producer": "performance", "valid": True},
        ]
        with patch.object(aggregate, "validate_or_refresh", side_effect=rows) as validate, \
             redirect_stdout(output):
            exit_code = aggregate.main(["--repo", str(root)])

        self.assertEqual(exit_code, 0)
        self.assertEqual([call.args[1] for call in validate.call_args_list], ["root", "performance"])
        self.assertEqual(json.loads(output.getvalue()), {
            "schema": aggregate.AGGREGATE_SCHEMA,
            "valid": True,
            "producers": rows,
        })

    def test_main_preserves_explicit_partial_scope_for_success_and_failure(self) -> None:
        root = Path(__file__).resolve().parents[2]
        cases = (
            ("success", {"producer": "performance", "valid": True}, 0),
            ("failure", {"producer": "performance", "valid": False,
                         "failure": "invalid-collection"}, 1),
        )
        for case, row, expected_exit in cases:
            with self.subTest(case=case):
                output = io.StringIO()
                with patch.object(aggregate, "validate_or_refresh", return_value=row) as validate, \
                     redirect_stdout(output):
                    exit_code = aggregate.main([
                        "--repo", str(root), "--producer", "performance",
                    ])

                self.assertEqual(exit_code, expected_exit)
                validate.assert_called_once_with(root, "performance")
                result = json.loads(output.getvalue())
                self.assertEqual(result["schema"], aggregate.PRODUCER_VALIDATION_SCHEMA)
                self.assertEqual(result["scope"], {
                    "kind": "producer-only", "producer": "performance", "fullAggregate": False,
                })
                self.assertEqual(result["valid"], expected_exit == 0)
                self.assertEqual(result["producers"], [row])

    def test_main_rejects_unknown_selector_before_validating_any_producer(self) -> None:
        root = Path(__file__).resolve().parents[2]
        error = io.StringIO()
        with patch.object(aggregate, "validate_or_refresh") as validate, \
             redirect_stderr(error), self.assertRaises(SystemExit) as raised:
            aggregate.main(["--repo", str(root), "--producer", "unregistered"])

        self.assertEqual(raised.exception.code, 2)
        self.assertIn("invalid choice", error.getvalue())
        validate.assert_not_called()


if __name__ == "__main__":
    unittest.main()
