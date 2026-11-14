from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import html_performance_retention_capture as retention
import produce_html_performance_evidence as producer

_PROBE = r'''
import json, sys, time
mode = sys.argv[1]
if mode == "stalled":
    time.sleep(30)
for cycle in range(30):
    if mode == "incomplete":
        sys.stdout.write("SAMPLE 0 ")
        sys.stdout.flush()
        break
    print("SAMPLE", cycle, json.dumps({"sample": cycle % 5}), flush=True)
    print("READY", cycle if mode != "wrong" else cycle + 1, flush=True)
    if sys.stdin.readline() != f"ACK {cycle}\n":
        raise SystemExit(3)
'''


class RetentionCaptureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.python = str(Path(sys.executable).resolve(strict=True))
        self.env = dict(os.environ)
        self.env.update({key: "synthetic-unit-value" for key in producer.WORKER_METADATA_KEYS})
        self.seen: list[int] = []

    def validate_sample(self, cycle: int, raw: bytes) -> None:
        self.assertEqual(json.loads(raw), {"sample": cycle % 5})
        self.seen.append(cycle)

    def capture(self, mode: str, name: str) -> retention.RetentionCapture:
        return retention.capture([self.python, "-c", _PROBE, mode], cwd=self.root,
                                 env=self.env, output=self.root / name,
                                 validate_sample=self.validate_sample)

    def test_records_all_barriers_and_retains_worker_lifecycle(self) -> None:
        result = self.capture("normal", "normal")
        records = [json.loads(line) for line in (result.directory / "memory.jsonl").read_bytes().splitlines()]
        self.assertEqual(self.seen, list(range(30)))
        self.assertEqual([record["cycle"] for record in records], list(range(30)))
        self.assertEqual([record["warmup"] for record in records], [True] * 5 + [False] * 25)
        self.assertTrue(all(record["rss"] > 0 and record["vms"] >= record["rss"] for record in records))
        lifecycle = json.loads(result.lifecycle.read_bytes())
        self.assertEqual(lifecycle["exitCode"], 0)
        self.assertTrue(lifecycle["groupClosed"] and lifecycle["rawFdsClosed"])
        self.assertEqual(lifecycle["identity"]["pid"], records[0]["pid"])
        self.assertNotIn("observationErrorType", result.runtime_execution)

    def test_malformed_or_incomplete_barrier_closes_worker_and_keeps_raw(self) -> None:
        for mode in ("wrong", "incomplete"):
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                self.capture(mode, mode)
            lifecycle_path, = (self.root / mode).glob("worker-*/lifecycle.json")
            lifecycle = json.loads(lifecycle_path.read_bytes())
            self.assertTrue(lifecycle["groupClosed"] and lifecycle["rawFdsClosed"])
            self.assertTrue((lifecycle_path.parent / "stdout.raw").is_file())
            self.assertTrue((lifecycle_path.parent / "stderr.raw").is_file())
            self.assertIsNotNone(lifecycle["exitCode"])

    def test_reused_output_directory_is_rejected(self) -> None:
        (self.root / "existing").mkdir()
        with self.assertRaises(FileExistsError):
            self.capture("normal", "existing")

    def test_stalled_worker_timeout_retains_lifecycle_and_reaps_group(self) -> None:
        with mock.patch.dict(producer.MEASUREMENT_LIMITS, {"worker": 0.2}):
            with self.assertRaises(subprocess.TimeoutExpired):
                self.capture("stalled", "stalled")
        lifecycle_path, = (self.root / "stalled").glob("worker-*/lifecycle.json")
        lifecycle = json.loads(lifecycle_path.read_bytes())
        self.assertTrue(lifecycle["groupClosed"] and lifecycle["rawFdsClosed"])
        self.assertIsNotNone(lifecycle["exitCode"])
        self.assertEqual((self.root / "stalled" / "memory.jsonl").read_bytes(), b"")

    def test_interactive_owner_rejects_other_roles_and_fixed_input(self) -> None:
        def callback(process: subprocess.Popen[bytes], path: Path, deadline: float) -> None:
            del process, path, deadline

        cases: tuple[tuple[producer.MeasurementRole, bytes | None], ...] = (
            ("build", None), ("worker", b"input"),
        )
        for role, input_bytes in cases:
            with self.subTest(role=role), self.assertRaises(ValueError):
                producer._measurement_command([self.python], role=role, log_directory=self.root,
                                              input_bytes=input_bytes, interaction=callback)


if __name__ == "__main__":
    unittest.main()
