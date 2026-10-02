from __future__ import annotations

import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from run_parallel_checks import LANES, Lane, run_lanes


class ParallelChecksTest(unittest.TestCase):
    def test_cargo_and_plantuml_work_share_one_serial_lane(self) -> None:
        cargo_lane = next(lane for lane in LANES if lane.name == "cargo")
        self.assertEqual(cargo_lane.recipe, "check-rust")
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        cargo_section = justfile.split("check-rust:", 1)[1].split("check-assets:", 1)[0]
        self.assertIn("just unit-test", cargo_section)
        self.assertIn("just runtime-asset-check", cargo_section)
        self.assertNotIn("just coverage", cargo_section)

    def test_waits_for_all_lanes_and_returns_failure(self) -> None:
        completed: list[str] = []

        def fake_run_lane(lane: Lane) -> tuple[str, int]:
            time.sleep(0.01)
            completed.append(lane.name)
            return lane.name, 17 if lane.name == "assets" else 0

        with mock.patch("run_parallel_checks.run_lane", side_effect=fake_run_lane):
            result = run_lanes(LANES, jobs=2)

        self.assertEqual(result, 17)
        self.assertCountEqual(completed, [lane.name for lane in LANES])

    def test_respects_configured_concurrency_limit(self) -> None:
        active = 0
        maximum_active = 0
        lock = threading.Lock()

        def fake_run_lane(lane: Lane) -> tuple[str, int]:
            nonlocal active, maximum_active
            with lock:
                active += 1
                maximum_active = max(maximum_active, active)
            time.sleep(0.02)
            with lock:
                active -= 1
            return lane.name, 0

        with mock.patch("run_parallel_checks.run_lane", side_effect=fake_run_lane):
            result = run_lanes(LANES, jobs=2)

        self.assertEqual(result, 0)
        self.assertEqual(maximum_active, 2)


if __name__ == "__main__":
    unittest.main()
