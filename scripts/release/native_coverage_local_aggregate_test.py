from __future__ import annotations

import os
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import native_coverage_local_aggregate as aggregate


class LocalCoverageAggregateUnitTests(unittest.TestCase):
    def test_cache_failure_requires_successful_producer_and_readback(self) -> None:
        cases = (
            ("valid-cache", [0], True, False),
            ("fresh-valid", [1, 0, 0], True, True),
            ("fresh-failed", [1, 7], False, True),
            ("fresh-invalid", [1, 0, 1], False, True),
        )
        for label, exits, valid, refreshed in cases:
            for producer, recipe in aggregate.PRODUCERS.items():
                with self.subTest(case=label, producer=producer):
                    results = [subprocess.CompletedProcess[str]([], code) for code in exits]
                    with (patch.dict(os.environ, {"MOON_TASK_ID": "coverage",
                                                  "MOON_TASK_HASH": "cached-task",
                                                  "RUSTFLAGS": "-D warnings"}),
                          patch("native_coverage_local_aggregate.subprocess.run", side_effect=results) as run):
                        row = aggregate.validate_or_refresh(Path("/fixture"), producer)
                    self.assertEqual(row["valid"], valid)
                    self.assertEqual(row["refreshed"], refreshed)
                    self.assertEqual(run.call_count, len(exits))
                    if refreshed:
                        self.assertEqual(run.call_args_list[1].args[0], ["just", recipe])
                        environment = run.call_args_list[1].kwargs["env"]
                        self.assertNotIn("MOON_TASK_ID", environment)
                        self.assertNotIn("MOON_TASK_HASH", environment)
                        self.assertEqual(environment["RUSTFLAGS"], "-D warnings")

    def test_unknown_producer_cannot_execute_an_arbitrary_recipe(self) -> None:
        with patch("native_coverage_local_aggregate.subprocess.run") as run:
            with self.assertRaises(KeyError):
                aggregate.validate_or_refresh(Path("/fixture"), "unregistered")
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
