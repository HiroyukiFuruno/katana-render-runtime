"""Contracts for private HTML transport scratch ownership and cleanup."""
from __future__ import annotations

import os
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import html_performance_transport_scratch as scratch


@unittest.skipUnless(os.name == "posix", "HTML performance transport runs on macOS")
class TransportScratchTest(unittest.TestCase):
    def test_success_and_failure_preserve_other_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            retained = tmp / "bundle"
            retained.mkdir()
            (retained / "payload").write_bytes(b"evidence")
            for fail in (False, True):
                workspace: Path | None = None
                try:
                    with scratch.transport_workspace(tmp) as workspace:
                        (workspace / "blob").write_bytes(b"scratch")
                        if fail:
                            raise RuntimeError("failure")
                except RuntimeError:
                    self.assertTrue(fail)
                assert workspace is not None
                self.assertFalse(workspace.exists())
                self.assertEqual((retained / "payload").read_bytes(), b"evidence")

    def test_signal_cleanup_and_handler_restoration(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            for signum in (signal.SIGTERM, signal.SIGHUP):
                previous = signal.getsignal(signum)
                workspace: Path | None = None
                with self.assertRaises(InterruptedError):
                    with scratch.transport_workspace(tmp) as workspace:
                        (workspace / "blob").write_bytes(b"scratch")
                        os.kill(os.getpid(), signum)
                assert workspace is not None
                self.assertFalse(workspace.exists())
                self.assertEqual(signal.getsignal(signum), previous)

    def test_ignored_signal_stays_ignored(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            previous = signal.signal(signal.SIGHUP, signal.SIG_IGN)
            try:
                with scratch.transport_workspace(Path(temporary).resolve()):
                    os.kill(os.getpid(), signal.SIGHUP)
                self.assertEqual(signal.getsignal(signal.SIGHUP), signal.SIG_IGN)
            finally:
                signal.signal(signal.SIGHUP, previous)

    def test_recovery_removes_owned_work_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            namespace = scratch._namespace(tmp)
            bootstrap = namespace / ("bootstrap-" + "a" * 32)
            bootstrap.mkdir(mode=0o700)
            work = namespace / ("bootstrap-" + "b" * 32)
            work.mkdir(mode=0o700)
            info = work.stat()
            work = work.rename(namespace / f"work-{info.st_dev}-{info.st_ino}-{'b' * 32}")
            (work / "blob").write_bytes(b"scratch")
            foreign = namespace / ("work-1-2-" + "c" * 32)
            foreign.mkdir(mode=0o700)
            unknown = namespace / "other"
            unknown.mkdir()
            scratch.recover_transport_scratch(tmp)
            self.assertFalse(bootstrap.exists())
            self.assertFalse(work.exists())
            self.assertTrue(foreign.exists())
            self.assertTrue(unknown.exists())

    def test_recovery_after_forced_process_exit(self) -> None:
        child = (
            "import signal, sys\n"
            "from pathlib import Path\n"
            "from html_performance_transport_scratch import transport_workspace\n"
            "with transport_workspace(Path(sys.argv[1])) as workspace:\n"
            "    (workspace / 'pending.raw').write_bytes(b'partial output')\n"
            "    print(workspace, flush=True)\n"
            "    signal.pause()\n"
        )
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            with subprocess.Popen([sys.executable, "-c", child, str(tmp)],
                                  cwd=Path(__file__).resolve().parent,
                                  stdout=subprocess.PIPE, text=True) as process:
                assert process.stdout is not None
                try:
                    line = process.stdout.readline()
                    self.assertTrue(line)
                    workspace = Path(line.strip())
                    self.assertTrue((workspace / "pending.raw").is_file())
                finally:
                    process.kill()
                    self.assertEqual(process.wait(timeout=10), -signal.SIGKILL)
            self.assertTrue(workspace.is_dir())
            scratch.recover_transport_scratch(tmp)
            self.assertFalse(workspace.exists())

    def test_recovery_preserves_nonempty_bootstrap_and_rejects_symlink_namespace(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            namespace = scratch._namespace(tmp)
            bootstrap = namespace / ("bootstrap-" + "e" * 32)
            bootstrap.mkdir(mode=0o700)
            (bootstrap / "data").write_bytes(b"preserve")
            scratch.recover_transport_scratch(tmp)
            self.assertTrue(bootstrap.exists())
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            outside = tmp / "outside"
            outside.mkdir()
            (tmp / scratch._NAMESPACE).symlink_to(outside)
            with self.assertRaises(ValueError):
                scratch.recover_transport_scratch(tmp)

    def test_recovery_preserves_symlink_and_wrong_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp = Path(temporary).resolve()
            namespace = scratch._namespace(tmp)
            outside = tmp / "outside"
            outside.mkdir()
            work = namespace / ("bootstrap-" + "d" * 32)
            work.mkdir(mode=0o700)
            info = work.stat()
            work = work.rename(namespace / f"work-{info.st_dev}-{info.st_ino}-{'d' * 32}")
            (work / "link").symlink_to(outside)
            scratch.recover_transport_scratch(tmp)
            self.assertTrue(work.exists())
            (work / "link").unlink()
            work.chmod(0o755)
            scratch.recover_transport_scratch(tmp)
            self.assertTrue(work.exists())
