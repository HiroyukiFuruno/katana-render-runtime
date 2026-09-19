from __future__ import annotations

import json
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("measure.py")


class MeasureCommandTests(unittest.TestCase):
    def run_measure(
        self,
        command: list[str],
        *options: str,
    ) -> tuple[subprocess.CompletedProcess[str], dict]:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "record.json"
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--output",
                    str(output),
                    *options,
                    "--",
                    *command,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            record = json.loads(output.read_text(encoding="utf-8"))
        return result, record

    def test_records_metadata_and_success(self) -> None:
        result, record = self.run_measure([sys.executable, "-c", "pass"])

        self.assertEqual(result.returncode, 0)
        self.assertEqual(record["exit_code"], 0)
        self.assertGreaterEqual(record["duration_seconds"], 0)
        self.assertEqual(record["command"][-1], "pass")
        self.assertEqual(record["check_id"], "adhoc")
        self.assertEqual(record["invocation_count"], 1)
        self.assertEqual(record["inputs"], [])
        self.assertEqual(len(record["input_digest"]), 64)
        for key in (
            "started_at",
            "ended_at",
            "head_at_start",
            "head_at_end",
            "os",
            "arch",
        ):
            self.assertTrue(record[key])
        self.assertTrue(record["cwd"])
        self.assertIsNotNone(record["dirty_at_start"])

    def test_inherits_child_output(self) -> None:
        result, _ = self.run_measure(
            [
                sys.executable,
                "-c",
                "import sys; print('stdout marker'); print('stderr marker', file=sys.stderr)",
            ]
        )

        self.assertEqual(result.returncode, 0)
        self.assertIn("stdout marker", result.stdout)
        self.assertIn("stderr marker", result.stderr)

    def test_preserves_command_failure_and_does_not_use_shell(self) -> None:
        result, record = self.run_measure(
            [sys.executable, "-c", "import sys; sys.exit(7)"]
        )

        self.assertEqual(result.returncode, 7)
        self.assertEqual(record["exit_code"], 7)

        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "marker"
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--output",
                    str(Path(directory) / "record.json"),
                    "--",
                    "printf",
                    "$(touch %s)" % marker,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)
            self.assertFalse(marker.exists())

    def test_converts_signal_return_code_after_recording_raw_value(self) -> None:
        result, record = self.run_measure(
            [
                sys.executable,
                "-c",
                "import os; os.kill(os.getpid(), __import__('signal').SIGTERM)",
            ]
        )

        self.assertEqual(result.returncode, 128 + signal.SIGTERM)
        self.assertEqual(record["exit_code"], -signal.SIGTERM)

    def test_records_check_id_and_declared_input_digest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            input_path = Path(directory) / "fixture.txt"
            input_path.write_text("before\n", encoding="utf-8")
            result, record = self.run_measure(
                [sys.executable, "-c", "pass"],
                "--check-id",
                "release-target",
                "--input",
                str(input_path),
            )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(record["check_id"], "release-target")
        self.assertEqual(record["inputs"], [str(input_path.resolve())])
        self.assertEqual(len(record["input_digest"]), 64)

    def test_rejects_missing_declared_input_before_running_command(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "record.json"
            marker = Path(directory) / "marker"
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--output",
                    str(output),
                    "--input",
                    str(Path(directory) / "missing"),
                    "--",
                    sys.executable,
                    "-c",
                    f"open({str(marker)!r}, 'w', encoding='utf-8').write('ran')",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 2)
            self.assertFalse(marker.exists())
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
