from __future__ import annotations

import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from typing import cast
from unittest.mock import Mock
from unittest.mock import patch

import psutil

import html_open_memory_diagnostic as diagnostic


class OpenInteractionTest(unittest.TestCase):
    def test_expired_deadline_does_not_start_vmmap(self) -> None:
        fake_worker = Mock(spec=subprocess.Popen)
        fake_worker.poll.return_value = None
        fake_worker.pid = 42
        worker = cast(subprocess.Popen[bytes], fake_worker)
        observed = Mock()
        observed.create_time.return_value = 1.0
        observed.is_running.return_value = True
        observed.memory_info.return_value.rss = 100
        with (patch.object(time, "monotonic", side_effect=[0.0, 10.0]),
              patch.object(psutil, "Process", return_value=observed),
              patch.object(subprocess, "run") as run):
            with self.assertRaisesRegex(ValueError, "deadline expired"):
                diagnostic.OpenInteraction()._sample(worker, 5.0)
        run.assert_not_called()

    def test_malloc_region_extracts_resident_and_dirty_only(self) -> None:
        text = "Malloc Small (empty) 1000-2000 [ 4.0M 32K 16K 0K ] rw-/rwx\n"
        self.assertEqual(diagnostic.malloc_regions(text), {
            "Malloc Small (empty)": {"resident": 32768, "dirty": 16384},
        })

    def test_heap_summary_requires_one_unambiguous_total(self) -> None:
        self.assertEqual(diagnostic.heap_summary("All zones: 42 nodes (1024 bytes)\n"),
                         {"allocations": 42, "bytes": 1024})
        for invalid in ("", "All zones: 1 nodes (1 bytes)\n" * 2):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "unavailable or ambiguous"):
                diagnostic.heap_summary(invalid)

    def test_rejects_unterminated_oversized_line(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "stdout"
            output.write_bytes(b"x" * (diagnostic.MAX_LINE_BYTES + 1))
            worker = cast(subprocess.Popen[bytes], Mock(spec=subprocess.Popen))
            with self.assertRaisesRegex(ValueError, "line exceeds limit"):
                diagnostic.OpenInteraction()(worker, output, time.monotonic() + 10)

    def test_two_barriers_and_extra_barrier(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "stdout"
            output.write_bytes(b"READY\n")
            process = subprocess.Popen(
                [sys.executable, "-c", "import sys; sys.stdin.readline(); sys.stdin.readline(); sys.stdin.readline()"],
                stdin=subprocess.PIPE,
            )
            try:
                interaction = diagnostic.OpenInteraction()
                with patch.object(subprocess, "run", return_value=Mock(stdout=(
                    "Malloc Small (empty) 1000-2000 [ 4.0M 32K 16K 0K ] rw-/rwx\n"
                    "All zones: 42 nodes (1024 bytes)\n"
                ))):
                    interaction(process, output, time.monotonic() + 10)
                    self.assertEqual(len(interaction.samples), 1)
                    with output.open("ab") as stream:
                        stream.write(b"READY\n")
                    interaction(process, output, time.monotonic() + 10)
                self.assertEqual(len(interaction.samples), 2)
                with output.open("ab") as stream:
                    stream.write(b"READY\n")
                with self.assertRaisesRegex(ValueError, "invalid open diagnostic protocol"):
                    interaction(process, output, time.monotonic() + 10)
            finally:
                process.terminate()
                process.wait(timeout=10)
                if process.stdin is not None:
                    process.stdin.close()


if __name__ == "__main__":
    unittest.main()
