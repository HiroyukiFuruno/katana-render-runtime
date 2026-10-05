from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

import local_review_windows_job as job_runner
from local_review_lock import review_lock
from local_review_state import ReviewError


class _FakeJob:
    def __init__(self, events: list[object], assign_error: Exception | None = None) -> None:
        self.events = events
        self.assign_error = assign_error

    def assign(self, handle: int) -> None:
        self.events.append(("assign", handle))
        if self.assign_error:
            raise self.assign_error

    def terminate(self) -> None:
        self.events.append("terminate")

    def wait_empty(self) -> None:
        self.events.append("empty")

    def close(self) -> None:
        self.events.append("close")


class _FakeHandle:
    def __init__(self, events: list[object]) -> None:
        self.events = events

    def Close(self) -> None:
        self.events.append("handle-close")


class _FakeStdin:
    def __init__(self, events: list[object]) -> None:
        self.events = events

    def close(self) -> None:
        self.events.append("stdin-close")


class _FakeProcess:
    def __init__(self, events: list[object], communicate_results: list[object]) -> None:
        self.events = events
        self._handle = _FakeHandle(events)
        self.stdin = _FakeStdin(events)
        self.communicate_results = communicate_results
        self.returncode = None
        self.killed = False

    def communicate(self, input: str | None = None, timeout: float | None = None):
        self.events.append(("communicate", input, timeout))
        result = self.communicate_results.pop(0)
        if isinstance(result, BaseException):
            raise result
        self.returncode = 0
        return result

    def poll(self):
        return self.returncode

    def kill(self) -> None:
        self.events.append("kill")
        self.killed = True
        self.returncode = -9




LOCK_PROBE_PROGRAM = (
    "import sys\nfrom pathlib import Path\n"
    "from local_review_lock import review_lock\n"
    "from local_review_state import ReviewError\n"
    "try:\n with review_lock(Path(sys.argv[1])): pass\n"
    "except ReviewError: print('locked')\n"
)
LOCK_RELEASE_PROGRAM = (
    "import sys\nfrom pathlib import Path\n"
    "from local_review_lock import review_lock\n"
    "with review_lock(Path(sys.argv[1])): pass\n"
)


def review_programs(root: Path) -> tuple[str, str]:
    parent_pid = root / "reviewer.pid"
    child_pid = root / "descendant.pid"
    descendant = (
        "import os,sys,time; from pathlib import Path; "
        "Path(sys.argv[1]).write_text(str(os.getpid())); time.sleep(3600)"
    )
    reviewer = (
        "import os,subprocess,sys,time\nfrom pathlib import Path\n"
        "assert sys.stdin.buffer.read().decode('utf-8') == '日本語 review'\n"
        f"subprocess.Popen([sys.executable,'-c',{descendant!r},{str(child_pid)!r}])\n"
        f"Path({str(parent_pid)!r}).write_text(str(os.getpid()))\n"
        "deadline=time.monotonic()+10\n"
        f"while not Path({str(child_pid)!r}).exists() and time.monotonic()<deadline: time.sleep(.01)\n"
        "if sys.argv[1] == 'timeout': time.sleep(3600)\n"
    )
    return descendant, reviewer


class WindowsJobRunnerUnitTest(unittest.TestCase):
    def run_fake(self, process: _FakeProcess, job: _FakeJob):
        events = process.events

        def popen(arguments, **kwargs):
            events.append(("launch", arguments, kwargs))
            return process

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            diagnostic = root / "stderr.log"
            result = job_runner.run_review_process(
                ["codex", "exec"], root, {}, "fixed prompt", diagnostic, 17,
                job_factory=lambda: job, popen_factory=popen,
            )
        return result, events

    def test_embedded_windows_python_programs_compile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            descendant, reviewer = review_programs(Path(temporary))
        for program in (descendant, reviewer, LOCK_PROBE_PROGRAM, LOCK_RELEASE_PROGRAM):
            compile(program, "<windows integration -c>", "exec")

    def test_assigns_launcher_before_sending_review_request_then_reaps_job(self) -> None:
        events: list[object] = []
        process = _FakeProcess(events, [(None, None), (None, None)])
        result, events = self.run_fake(process, _FakeJob(events))

        self.assertEqual(result, 0)
        names = [event if isinstance(event, str) else event[0] for event in events]
        self.assertLess(names.index("launch"), names.index("assign"))
        self.assertLess(names.index("assign"), names.index("communicate"))
        self.assertLess(names.index("communicate"), names.index("terminate"))
        self.assertLess(names.index("terminate"), names.index("empty"))
        self.assertLess(names.index("handle-close"), names.index("empty"))
        self.assertLess(names.index("empty"), names.index("close"))
        launch = next(event for event in events if isinstance(event, tuple) and event[0] == "launch")
        self.assertEqual(launch[2]["stdin"], subprocess.PIPE)
        payload = next(event[1] for event in events if isinstance(event, tuple) and event[0] == "communicate")
        self.assertEqual(json.loads(payload), {"arguments": ["codex", "exec"], "prompt": "fixed prompt"})

    def test_timeout_terminates_job_before_reaping_launcher(self) -> None:
        events: list[object] = []
        timeout = subprocess.TimeoutExpired("launcher", 17)
        process = _FakeProcess(events, [timeout, (None, None)])
        job = _FakeJob(events)
        with self.assertRaises(subprocess.TimeoutExpired):
            self.run_fake(process, job)

        names = [event if isinstance(event, str) else event[0] for event in events]
        self.assertEqual(names.count("communicate"), 2)
        self.assertLess(names.index("terminate"), max(i for i, name in enumerate(names) if name == "communicate"))
        self.assertLess(names.index("handle-close"), names.index("empty"))
        self.assertLess(names.index("empty"), names.index("close"))

    def test_job_initialization_failure_does_not_start_launcher(self) -> None:
        events: list[object] = []
        process = _FakeProcess(events, [(None, None), (None, None)])

        def fail_job():
            raise RuntimeError("job creation failed")

        def popen(arguments, **kwargs):
            events.append("launch")
            return process

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(RuntimeError, "job creation failed"):
                job_runner.run_review_process(
                    ["codex"], root, {}, "prompt", root / "stderr.log", 1,
                    job_factory=fail_job, popen_factory=popen,
                )
        self.assertNotIn("launch", events)

    def test_terminate_failure_closes_job_before_reaping_launcher(self) -> None:
        events: list[object] = []
        process = _FakeProcess(events, [(None, None), (None, None)])
        job = _FakeJob(events)

        def fail_terminate() -> None:
            events.append("terminate")
            raise OSError("terminate failed")

        job.terminate = fail_terminate
        original_close = job.close

        def close_and_kill() -> None:
            original_close()
            process.returncode = -9

        job.close = close_and_kill
        with self.assertRaisesRegex(OSError, "terminate failed"):
            self.run_fake(process, job)

        names = [event if isinstance(event, str) else event[0] for event in events]
        self.assertLess(names.index("terminate"), names.index("close"))
        self.assertLess(names.index("close"), max(i for i, name in enumerate(names) if name == "communicate"))
        self.assertNotIn("empty", names)
        self.assertIn("handle-close", names)

    def test_assignment_failure_kills_unassigned_launcher_without_sending_payload(self) -> None:
        events: list[object] = []
        process = _FakeProcess(events, [(None, None), (None, None)])
        job = _FakeJob(events, RuntimeError("assignment failed"))
        with self.assertRaisesRegex(RuntimeError, "assignment failed"):
            self.run_fake(process, job)

        names = [event if isinstance(event, str) else event[0] for event in events]
        self.assertIn("kill", names)
        self.assertEqual(names.count("communicate"), 1)
        communicate = next(event for event in events if isinstance(event, tuple) and event[0] == "communicate")
        self.assertIsNone(communicate[1])
        self.assertNotIn("payload", str(events))
        self.assertIn("close", names)
        self.assertLess(names.index("handle-close"), names.index("empty"))


class WindowsJobRunnerIntegrationTest(unittest.TestCase):
    @staticmethod
    def wait_process_exit(pid: int) -> None:
        import ctypes

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel32.OpenProcess.restype = ctypes.c_void_p
        kernel32.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel32.WaitForSingleObject.restype = ctypes.c_uint32
        kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel32.OpenProcess(0x00100000, False, pid)
        if not handle:
            error = ctypes.get_last_error()
            if error == 87:  # 87はERROR_INVALID_PARAMETERで、終了済みPIDだけを許容する。
                return
            raise ctypes.WinError(error)
        try:
            result = kernel32.WaitForSingleObject(handle, 10_000)
            if result != 0:
                raise AssertionError(f"process {pid} remained active (wait={result})")
        finally:
            kernel32.CloseHandle(handle)

    def execute_review(
        self, root: Path, *, timeout: float, job_factory=job_runner._JobApi,
        expected_exception= subprocess.TimeoutExpired,
    ) -> tuple[Path, Path]:
        parent_pid = root / "reviewer.pid"
        child_pid = root / "descendant.pid"
        _descendant, reviewer = review_programs(root)
        compile(reviewer, "<windows review command>", "exec")
        returncode = None
        if timeout < 10:
            with self.assertRaises(expected_exception):
                job_runner.run_review_process(
                    [sys.executable, "-c", reviewer, "timeout"], root, dict(os.environ),
                    "日本語 review", root / "stderr.log", timeout, job_factory=job_factory,
                )
        else:
            returncode = job_runner.run_review_process(
                [sys.executable, "-c", reviewer, "success"], root, dict(os.environ),
                "日本語 review", root / "stderr.log", timeout, job_factory=job_factory,
            )
            self.assertEqual(returncode, 0)
        deadline = time.monotonic() + 10
        while (not parent_pid.exists() or not child_pid.exists()) and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(parent_pid.exists())
        self.assertTrue(child_pid.exists())
        self.wait_process_exit(int(parent_pid.read_text()))
        self.wait_process_exit(int(child_pid.read_text()))
        return parent_pid, child_pid

    @unittest.skipUnless(os.name == "nt", "Windows Job Object integration")
    def test_job_cleans_descendants_after_successful_wrapper_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            self.execute_review(Path(temporary), timeout=10)

    @unittest.skipUnless(os.name == "nt", "Windows Job Object integration")
    def test_job_cleans_descendants_on_timeout_before_lock_release(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            lock = root / "review.lock"
            with review_lock(lock):
                self.execute_review(root, timeout=3)
                probe = subprocess.run(
                    [sys.executable, "-c", LOCK_PROBE_PROGRAM, str(lock)],
                    cwd=Path(__file__).parent, capture_output=True, text=True, check=True,
                )
                self.assertEqual(probe.stdout.strip(), "locked")
            subprocess.run(
                [sys.executable, "-c", LOCK_RELEASE_PROGRAM, str(lock)],
                cwd=Path(__file__).parent, check=True,
            )


if __name__ == "__main__":
    unittest.main()
