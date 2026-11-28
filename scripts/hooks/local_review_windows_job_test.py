from __future__ import annotations

import json
import ctypes
import io
import errno
import os
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from typing import TYPE_CHECKING, Protocol, runtime_checkable
from types import SimpleNamespace
from unittest.mock import patch

import local_review_windows_job as job_runner
from local_review_lock import review_lock
from local_review_state import ReviewError


if TYPE_CHECKING:
    _Event = str | tuple[str, object] | tuple[str, object, object]


class _FakeJob:
    def __init__(self, events: list[_Event], assign_error: Exception | None = None) -> None:
        self.events = events
        self.assign_error = assign_error

    def assign(self, process_handle: object) -> None:
        self.events.append(("assign", process_handle))
        if self.assign_error:
            raise self.assign_error

    def terminate(self) -> None:
        self.events.append("terminate")

    def wait_empty(self) -> None:
        self.events.append("empty")

    def close(self) -> None:
        self.events.append("close")


class _FakeHandle:
    def __init__(self, events: list[_Event]) -> None:
        self.events = events

    def Close(self) -> None:
        self.events.append("handle-close")


class _FakeStdin:
    def __init__(self, events: list[_Event]) -> None:
        self.events = events

    def close(self) -> None:
        self.events.append("stdin-close")


class _FakeProcess:
    def __init__(self, events: list[_Event], communicate_results: list[object], exit_code: object = 0) -> None:
        self.events = events
        self._handle = _FakeHandle(events)
        self.stdin = _FakeStdin(events)
        self.communicate_results = communicate_results
        self.returncode: object = None
        self.exit_code = exit_code
        self.killed = False

    def communicate(self, input: str | None = None, timeout: float | None = None) -> object:
        self.events.append(("communicate", input, timeout))
        result = self.communicate_results.pop(0)
        if isinstance(result, BaseException):
            raise result
        self.returncode = self.exit_code
        return result

    def poll(self) -> object:
        return self.returncode

    def kill(self) -> None:
        self.events.append("kill")
        self.killed = True
        self.returncode = -9


class _FakeKernelFunction:
    def __init__(self, name: str, events: list[tuple[str, tuple[object, ...]]], result: object = 1) -> None:
        self.name = name
        self.events = events
        self.result = result
        self.argtypes: list[type[object]] | None = None
        self.restype: type[object] | None = None

    def __call__(self, *arguments: object) -> object:
        self.events.append((self.name, arguments))
        return self.result


class _FakeKernel32:
    def __init__(self) -> None:
        self.events: list[tuple[str, tuple[object, ...]]] = []
        self.CreateJobObjectW = _FakeKernelFunction("create", self.events, 73)
        self.SetInformationJobObject = _FakeKernelFunction("set", self.events)
        self.AssignProcessToJobObject = _FakeKernelFunction("assign", self.events)
        self.TerminateJobObject = _FakeKernelFunction("terminate", self.events)
        self.QueryInformationJobObject = _FakeKernelFunction("query", self.events)
        self.CloseHandle = _FakeKernelFunction("close", self.events)


class _LazyKernel32:
    def __init__(self, kernel32: _FakeKernel32) -> None:
        self.kernel32 = kernel32

    def __getattr__(self, name: str) -> object:
        function: object = getattr(self.kernel32, name)
        setattr(self, name, function)
        return function


@contextmanager
def windows_ctypes_fixture(
    kernel32: object, *, last_error: object = 5, error_result: object = OSError("native error"),
) -> Iterator[None]:
    def library(name: str, *, use_last_error: bool) -> object:
        if name != "kernel32" or not use_last_error:
            raise AssertionError("kernel32 must retain use_last_error")
        return kernel32

    def win_error(code: int) -> object:
        if code != last_error:
            raise AssertionError("WinError must use the native last-error code")
        return error_result

    with patch.object(os, "name", "nt"), \
            patch.object(ctypes, "WinDLL", library, create=True), \
            patch.object(ctypes, "get_last_error", lambda: last_error, create=True), \
            patch.object(ctypes, "WinError", win_error, create=True):
        yield


@runtime_checkable
class _WaitApi(Protocol):
    OpenProcess: job_runner._KernelFunction
    WaitForSingleObject: job_runner._KernelFunction
    CloseHandle: job_runner._KernelFunction




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
    def run_fake(self, process: _FakeProcess, job: _FakeJob) -> tuple[int, list[_Event]]:
        events = process.events

        def popen(args: list[str], **kwargs: object) -> _FakeProcess:
            events.append(("launch", args, kwargs))
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
        events: list[_Event] = []
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
        assert len(launch) == 3 and isinstance(launch[2], dict)
        self.assertEqual(launch[2]["stdin"], subprocess.PIPE)
        payload = next(event[1] for event in events if isinstance(event, tuple) and event[0] == "communicate")
        assert isinstance(payload, str)
        self.assertEqual(json.loads(payload), {"arguments": ["codex", "exec"], "prompt": "fixed prompt"})

    def test_timeout_terminates_job_before_reaping_launcher(self) -> None:
        events: list[_Event] = []
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
        events: list[_Event] = []
        process = _FakeProcess(events, [(None, None), (None, None)])

        def fail_job() -> _FakeJob:
            raise RuntimeError("job creation failed")

        def popen(args: list[str], **kwargs: object) -> _FakeProcess:
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
        events: list[_Event] = []
        process = _FakeProcess(events, [(None, None), (None, None)])
        job = _FakeJob(events)

        def fail_terminate() -> None:
            events.append("terminate")
            raise OSError("terminate failed")

        original_close = job.close

        def close_and_kill() -> None:
            original_close()
            process.returncode = -9

        with patch.object(job, "terminate", fail_terminate), patch.object(job, "close", close_and_kill), \
                self.assertRaisesRegex(OSError, "terminate failed"):
            self.run_fake(process, job)

        names = [event if isinstance(event, str) else event[0] for event in events]
        self.assertLess(names.index("terminate"), names.index("close"))
        self.assertLess(names.index("close"), max(i for i, name in enumerate(names) if name == "communicate"))
        self.assertNotIn("empty", names)
        self.assertIn("handle-close", names)

    def test_assignment_failure_kills_unassigned_launcher_without_sending_payload(self) -> None:
        events: list[_Event] = []
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

    def test_noninteger_launcher_exit_code_rejects_and_cleans_job(self) -> None:
        for code in (None, True, "0", object()):
            with self.subTest(code=code):
                events: list[_Event] = []
                process = _FakeProcess(events, [(None, None), (None, None)], code)
                with self.assertRaisesRegex(TypeError, "launcher exit code"):
                    self.run_fake(process, _FakeJob(events))
                self.assertEqual(events[-3:], ["handle-close", "empty", "close"])

    def test_launcher_rejects_invalid_request_without_starting_review(self) -> None:
        requests = ("[", "null", "{}", '{"arguments":"codex","prompt":"p"}',
                    '{"arguments":[1],"prompt":"p"}', '{"arguments":["codex"],"prompt":1}')
        for request in requests:
            with self.subTest(request=request), patch.object(sys, "stdin", io.StringIO(request)), \
                    patch("local_review_windows_job.subprocess.run") as launch:
                self.assertEqual(job_runner._launcher(), 2)
                launch.assert_not_called()

    def test_launcher_preserves_arguments_utf8_prompt_and_exit_code(self) -> None:
        args = ["codex", "exec"]
        request = json.dumps({"arguments": args, "prompt": "日本語 review"})
        with patch.object(sys, "stdin", io.StringIO(request)), \
                patch("local_review_windows_job.subprocess.run", return_value=subprocess.CompletedProcess(args, 7)) as launch:
            self.assertEqual(job_runner._launcher(), 7)
            launch.assert_called_once_with(
                args, input="日本語 review", text=True, encoding="utf-8", stdout=subprocess.DEVNULL,
            )


class WindowsApiBoundaryUnitTest(unittest.TestCase):
    def test_nonwindows_job_api_is_unavailable_without_loading_library(self) -> None:
        with patch.object(os, "name", "posix"), patch.object(ctypes, "WinDLL", create=True) as library:
            with self.assertRaisesRegex(OSError, "Job Objects are unavailable"):
                job_runner._JobApi()
            library.assert_not_called()

    def test_kernel_binding_preserves_types_arguments_and_cleanup_order(self) -> None:
        kernel32 = _FakeKernel32()
        lazy_kernel32 = _LazyKernel32(kernel32)
        with windows_ctypes_fixture(lazy_kernel32):
            job = job_runner._JobApi()
            self.assertEqual(kernel32.CreateJobObjectW.argtypes, [ctypes.c_void_p, ctypes.c_wchar_p])
            self.assertIs(kernel32.CreateJobObjectW.restype, ctypes.c_void_p)
            signatures = (
                (kernel32.SetInformationJobObject, [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]),
                (kernel32.AssignProcessToJobObject, [ctypes.c_void_p, ctypes.c_void_p]),
                (kernel32.TerminateJobObject, [ctypes.c_void_p, ctypes.c_uint32]),
                (kernel32.QueryInformationJobObject, [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p,
                                                    ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32)]),
                (kernel32.CloseHandle, [ctypes.c_void_p]),
            )
            for function, arguments in signatures:
                self.assertEqual(function.argtypes, arguments)
                self.assertIs(function.restype, ctypes.c_int)
            limits: object = getattr(kernel32.events[1][1][2], "_obj", None)
            assert isinstance(limits, job_runner._ExtendedLimitInformation)
            self.assertEqual(limits.BasicLimitInformation.LimitFlags, job_runner.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
            job.assign(91)
            job.terminate()
            job.wait_empty()
            job.close()
            job.close()
        self.assertEqual([name for name, _ in kernel32.events], ["create", "set", "assign", "terminate", "query", "close"])
        self.assertEqual(kernel32.events[0][1], (None, None))
        self.assertEqual(kernel32.events[1][1][:2], (73, job_runner.JOB_OBJECT_EXTENDED_LIMIT_INFORMATION))
        self.assertEqual(kernel32.events[1][1][3], ctypes.sizeof(job_runner._ExtendedLimitInformation))
        self.assertEqual(kernel32.events[2][1], (73, 91))
        self.assertEqual(kernel32.events[3][1], (73, 1))
        self.assertEqual(kernel32.events[4][1][:2], (73, job_runner.JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION))
        self.assertEqual(kernel32.events[4][1][3:], (ctypes.sizeof(job_runner._BasicAccountingInformation), None))
        self.assertEqual(kernel32.events[5][1], (73,))
        self.assertIsNone(job.handle)

    def test_invalid_handles_reject_before_configuring_limits(self) -> None:
        for handle in (True, "73", object()):
            kernel32 = _FakeKernel32()
            kernel32.CreateJobObjectW.result = handle
            with self.subTest(handle=handle), windows_ctypes_fixture(kernel32):
                with self.assertRaisesRegex(TypeError, "Job Object handle"):
                    job_runner._JobApi()
                self.assertEqual([name for name, _ in kernel32.events], ["create"])

    def test_invalid_last_error_or_error_object_is_rejected(self) -> None:
        scenarios = ((True, OSError(), "last-error code"), ("5", OSError(), "last-error code"),
                     (object(), OSError(), "last-error code"), (5, None, "Windows error"),
                     (5, "error", "Windows error"))
        for field, expected_calls in (("CreateJobObjectW", ["create"]),
                                      ("SetInformationJobObject", ["create", "set", "close"])):
            for code, error, message in scenarios:
                kernel32 = _FakeKernel32()
                function: object = getattr(kernel32, field)
                assert isinstance(function, _FakeKernelFunction)
                function.result = 0
                with self.subTest(field=field, code=code, error=error), \
                        windows_ctypes_fixture(kernel32, last_error=code, error_result=error):
                    with self.assertRaisesRegex(TypeError, message):
                        job_runner._JobApi()
                    self.assertEqual([name for name, _ in kernel32.events], expected_calls)

    def test_native_error_identity_and_failed_setup_cleanup_are_preserved(self) -> None:
        for field, expected_calls in (("CreateJobObjectW", ["create"]),
                                      ("SetInformationJobObject", ["create", "set", "close"])):
            kernel32 = _FakeKernel32()
            failure = OSError(5, "native denied")
            with self.subTest(field=field), windows_ctypes_fixture(kernel32, error_result=failure):
                function: object = getattr(kernel32, field)
                assert isinstance(function, _FakeKernelFunction)
                function.result = 0
                with self.assertRaises(OSError) as rejected:
                    job_runner._JobApi()
                self.assertIs(rejected.exception, failure)
                self.assertEqual([name for name, _ in kernel32.events], expected_calls)

    def test_noncallable_windows_api_or_kernel_function_is_rejected(self) -> None:
        for field in ("WinDLL", "WinError", "get_last_error"):
            kernel32 = _FakeKernel32()
            with self.subTest(field=field), windows_ctypes_fixture(kernel32), \
                    patch.object(ctypes, field, object()):
                with self.assertRaisesRegex(OSError, "ctypes API is unavailable"):
                    job_runner._JobApi()
                self.assertFalse(kernel32.events)
        kernel32 = _FakeKernel32()
        with windows_ctypes_fixture(kernel32), patch.object(kernel32, "CloseHandle", object()):
            with self.assertRaisesRegex(TypeError, "invalid kernel32 function"):
                job_runner._JobApi()
            self.assertFalse(kernel32.events)

    def test_windows_lock_keeps_native_arguments_and_closes_descriptor_on_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "review.lock"
            calls: list[tuple[int, int, int]] = []

            def locking(descriptor: int, mode: int, count: int) -> None:
                calls.append((descriptor, mode, count))

            module = SimpleNamespace(LK_NBLCK=2, locking=locking)
            with patch.dict(sys.modules, {"msvcrt": module}), patch.object(os, "name", "nt"):
                with review_lock(lock):
                    os.fstat(calls[0][0])
                self.assertEqual(calls[0][1:], (2, 1))
                with self.assertRaises(OSError):
                    os.fstat(calls[0][0])
            invalid_modules = (SimpleNamespace(LK_NBLCK=True, locking=locking),
                               SimpleNamespace(LK_NBLCK="2", locking=locking),
                               SimpleNamespace(LK_NBLCK=2, locking=object()))
            for invalid in invalid_modules:
                with self.subTest(module=invalid), patch.dict(sys.modules, {"msvcrt": invalid}), \
                        patch.object(os, "name", "nt"), patch("local_review_lock.os.close", wraps=os.close) as close:
                    with self.assertRaisesRegex(OSError, "file-lock API is unavailable"):
                        with review_lock(lock):
                            self.fail("invalid lock API admitted review")
                    close.assert_called_once()

    def test_windows_lock_reports_contention_and_closes_descriptor(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "review.lock"
            for code in (errno.EACCES, errno.EAGAIN, errno.EBADF):
                def locking(descriptor: int, mode: int, count: int, error_code: int = code) -> None:
                    raise OSError(error_code, "native locking failure")

                module = SimpleNamespace(LK_NBLCK=2, locking=locking)
                expected = OSError if code == errno.EBADF else ReviewError
                with self.subTest(code=code), patch.dict(sys.modules, {"msvcrt": module}), \
                        patch.object(os, "name", "nt"), patch("local_review_lock.os.close", wraps=os.close) as close:
                    with self.assertRaises(expected):
                        with review_lock(lock):
                            self.fail("failed native locking admitted review")
                    close.assert_called_once()


class WindowsJobRunnerIntegrationTest(unittest.TestCase):
    @staticmethod
    def wait_process_exit(pid: int) -> None:
        windows_ctypes: object = ctypes
        if not isinstance(windows_ctypes, job_runner._WindowsCtypes):
            raise AssertionError("Windows ctypes API is unavailable")
        kernel32 = windows_ctypes.WinDLL("kernel32", use_last_error=True)
        for name in ("OpenProcess", "WaitForSingleObject", "CloseHandle"):
            function: object = getattr(kernel32, name, None)
            if not isinstance(function, job_runner._KernelFunction) or not callable(function):
                raise AssertionError("Windows process-wait function is unavailable")
        if not isinstance(kernel32, _WaitApi):
            raise AssertionError("Windows process-wait API is unavailable")
        kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel32.OpenProcess.restype = ctypes.c_void_p
        kernel32.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel32.WaitForSingleObject.restype = ctypes.c_uint32
        kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel32.OpenProcess(0x00100000, False, pid)
        if not handle:
            error = windows_ctypes.get_last_error()
            if type(error) is not int:
                raise AssertionError("invalid Windows last-error code")
            if error == 87:  # 87はERROR_INVALID_PARAMETERで、終了済みPIDだけを許容する。
                return
            exception = windows_ctypes.WinError(error)
            if not isinstance(exception, OSError):
                raise AssertionError("invalid Windows error")
            raise exception
        try:
            result = kernel32.WaitForSingleObject(handle, 10_000)
            if result != 0:
                raise AssertionError(f"process {pid} remained active (wait={result})")
        finally:
            kernel32.CloseHandle(handle)

    def execute_review(
        self, root: Path, *, timeout: float, job_factory: Callable[[], job_runner._Job] = job_runner._JobApi,
        expected_exception: type[Exception] = subprocess.TimeoutExpired,
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
