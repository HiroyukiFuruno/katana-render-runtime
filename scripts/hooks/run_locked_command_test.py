from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from local_review_lock import review_lock
from run_locked_command import run_locked_command


class LockedCommandTest(unittest.TestCase):
    def _fake_rtk(self, root: Path, marker: Path) -> Path:
        executable = root / "bin/rtk"
        executable.parent.mkdir(parents=True)
        executable.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            "from pathlib import Path\n"
            "if sys.argv[1:] != ['proxy', 'cargo', 'clean']:\n"
            "    raise SystemExit(99)\n"
            f"Path({str(marker)!r}).write_text('ran', encoding='utf-8')\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)
        return executable

    def _run_protected_clean(self, root: Path, rtk: Path, producer_lock: Path) -> int:
        return run_locked_command(
            ["--protect-root-producer", "--", str(rtk), "proxy", "cargo", "clean"],
            root / "outer/review.lock",
            producer_lock,
            root,
        )

    def test_protected_clean_without_references_preserves_relative_target_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {"PATH": str(rtk.parent), "CARGO_TARGET_DIR": "relative-target"}):
                os.environ.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 0)
            self.assertTrue(marker.exists())

    def test_protected_clean_rejects_target_reference_before_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            reserved = target / "debug/krr"
            reserved.parent.mkdir()
            reserved.write_text("binary", encoding="utf-8")
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {
                "PATH": str(rtk.parent),
                "CARGO_TARGET_DIR": str(target),
                "CARGO_CAPACITY_PROTECTED_REFS": json.dumps([str(reserved)]),
            }):
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 1)
            self.assertFalse(marker.exists())
            self.assertEqual(reserved.read_text(encoding="utf-8"), "binary")

    def test_protected_clean_allows_reference_outside_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            external = root / "reserved/krr"
            external.parent.mkdir()
            external.write_text("binary", encoding="utf-8")
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {
                "PATH": str(rtk.parent),
                "CARGO_TARGET_DIR": str(target),
                "CARGO_CAPACITY_PROTECTED_REFS": json.dumps([str(external)]),
            }):
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 0)
            self.assertTrue(marker.exists())
            self.assertEqual(external.read_text(encoding="utf-8"), "binary")

    def test_lock_is_held_for_child_and_released_after_success(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            code = """from pathlib import Path
import sys
sys.path.insert(0, sys.argv[2])
from local_review_lock import review_lock
from local_review_state import ReviewError
try:
    with review_lock(Path(sys.argv[1])):
        raise SystemExit(9)
except ReviewError:
    pass
"""
            status = run_locked_command(
                ["--", sys.executable, "-c", code, str(lock), str(Path(__file__).parent)], lock
            )
            self.assertEqual(status, 0)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_parent_lock_rejects_child_until_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            sentinel = Path(directory) / "child-ran"
            hooks = str(Path(__file__).parent)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[2])
from local_review_lock import ReviewError
from run_locked_command import run_locked_command
command = ["--", sys.executable, "-c", "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", sys.argv[3]]
try:
    status = run_locked_command(command, Path(sys.argv[1]))
except ReviewError:
    raise SystemExit(0)
raise SystemExit(9 if status == 0 else 8)
"""
            with review_lock(lock):
                attempt = subprocess.run(
                    [sys.executable, "-c", code, str(lock), hooks, str(sentinel)],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(attempt.returncode, 0, attempt.stderr)
                self.assertFalse(sentinel.exists())
            status = run_locked_command(
                ["--", sys.executable, "-c", "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", str(sentinel)],
                lock,
            )
            self.assertEqual(status, 0)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "ran")

    def test_child_failure_status_is_preserved_and_lock_released(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            status = run_locked_command(["--", sys.executable, "-c", "raise SystemExit(23)"], lock)
            self.assertEqual(status, 23)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_protected_child_holds_both_locks_until_exit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outer_lock = root / "outer/review.lock"
            producer_lock = root / "producer/root-nextest-producer.lock"
            hooks = str(Path(__file__).parent)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[3])
from local_review_lock import review_lock
from local_review_state import ReviewError
for lock in (Path(sys.argv[1]), Path(sys.argv[2])):
    try:
        with review_lock(lock):
            pass
    except ReviewError:
        continue
    raise SystemExit(9)
raise SystemExit(0)
"""
            status = run_locked_command(
                ["--protect-root-producer", "--", sys.executable, "-c", code,
                 str(producer_lock), str(outer_lock), hooks],
                outer_lock,
                producer_lock,
            )
            self.assertEqual(status, 0)
            with review_lock(outer_lock), review_lock(producer_lock):
                self.assertTrue(outer_lock.exists())
                self.assertTrue(producer_lock.exists())

    def test_busy_root_producer_rejects_child_and_releases_outer_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outer_lock = root / "outer/review.lock"
            producer_lock = root / "producer/root-nextest-producer.lock"
            sentinel = root / "child-ran"
            hooks = str(Path(__file__).parent)
            producer_lock.parent.mkdir(parents=True)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from local_review_state import ReviewError
from run_locked_command import run_locked_command
command = ["--protect-root-producer", "--", sys.executable, "-c",
           "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", sys.argv[4]]
try:
    run_locked_command(command, Path(sys.argv[2]), Path(sys.argv[3]))
except ReviewError:
    raise SystemExit(0)
raise SystemExit(9)
"""
            with review_lock(producer_lock):
                attempt = subprocess.run(
                    [sys.executable, "-c", code, hooks, str(outer_lock),
                     str(producer_lock), str(sentinel)],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(attempt.returncode, 0, attempt.stderr)
                self.assertFalse(sentinel.exists())
            with review_lock(outer_lock):
                self.assertTrue(outer_lock.exists())

    def test_child_launch_failure_releases_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            missing = Path(directory) / "missing-command"
            self.assertEqual(run_locked_command(["--", str(missing)], lock), 1)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_invalid_command_is_rejected_without_locking(self) -> None:
        self.assertEqual(run_locked_command([]), 2)
        self.assertEqual(run_locked_command(["--"]), 2)
        self.assertEqual(run_locked_command(["--protect-root-producer"]), 2)
        self.assertEqual(run_locked_command(["--protect-root-producer", "--"]), 2)


if __name__ == "__main__":
    unittest.main()
