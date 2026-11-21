#!/usr/bin/env python3
"""Windows Job Objectでローカルレビューの子孫プロセスを管理する。"""
from __future__ import annotations

import ctypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from collections.abc import Callable
from typing import Protocol, TextIO, runtime_checkable


LAUNCHER_ARGUMENT = "--local-review-launcher"
JOB_OBJECT_EXTENDED_LIMIT_INFORMATION = 9
JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION = 1
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000


@runtime_checkable
class _KernelFunction(Protocol):
    argtypes: list[type[object]] | None
    restype: type[object] | None

    def __call__(self, *arguments: object) -> object: ...


@runtime_checkable
class _Kernel32(Protocol):
    CreateJobObjectW: _KernelFunction
    SetInformationJobObject: _KernelFunction
    AssignProcessToJobObject: _KernelFunction
    TerminateJobObject: _KernelFunction
    QueryInformationJobObject: _KernelFunction
    CloseHandle: _KernelFunction


@runtime_checkable
class _WindowsCtypes(Protocol):
    def WinDLL(self, name: str, *, use_last_error: bool) -> object: ...
    def WinError(self, code: int) -> object: ...
    def get_last_error(self) -> object: ...


class _BasicLimitInformation(ctypes.Structure):
    _fields_ = [
        ("PerProcessUserTimeLimit", ctypes.c_longlong),
        ("PerJobUserTimeLimit", ctypes.c_longlong),
        ("LimitFlags", ctypes.c_uint32),
        ("MinimumWorkingSetSize", ctypes.c_size_t),
        ("MaximumWorkingSetSize", ctypes.c_size_t),
        ("ActiveProcessLimit", ctypes.c_uint32),
        ("Affinity", ctypes.c_size_t),
        ("PriorityClass", ctypes.c_uint32),
        ("SchedulingClass", ctypes.c_uint32),
    ]


class _IoCounters(ctypes.Structure):
    _fields_ = [(field, ctypes.c_ulonglong) for field in (
        "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
        "ReadTransferCount", "WriteTransferCount", "OtherTransferCount",
    )]


class _ExtendedLimitInformation(ctypes.Structure):
    _fields_ = [
        ("BasicLimitInformation", _BasicLimitInformation),
        ("IoInfo", _IoCounters),
        ("ProcessMemoryLimit", ctypes.c_size_t),
        ("JobMemoryLimit", ctypes.c_size_t),
        ("PeakProcessMemoryUsed", ctypes.c_size_t),
        ("PeakJobMemoryUsed", ctypes.c_size_t),
    ]


class _BasicAccountingInformation(ctypes.Structure):
    _fields_ = [
        ("TotalUserTime", ctypes.c_longlong),
        ("TotalKernelTime", ctypes.c_longlong),
        ("ThisPeriodTotalUserTime", ctypes.c_longlong),
        ("ThisPeriodTotalKernelTime", ctypes.c_longlong),
        ("TotalPageFaultCount", ctypes.c_uint32),
        ("TotalProcesses", ctypes.c_uint32),
        ("ActiveProcesses", ctypes.c_uint32),
        ("TotalTerminatedProcesses", ctypes.c_uint32),
    ]


class _JobApi:
    def __init__(self) -> None:
        if os.name != "nt":
            raise OSError("Windows Job Objects are unavailable")
        windows_ctypes: object = ctypes
        if not isinstance(windows_ctypes, _WindowsCtypes) or not all((
            callable(windows_ctypes.WinDLL), callable(windows_ctypes.WinError),
            callable(windows_ctypes.get_last_error),
        )):
            raise OSError("Windows ctypes API is unavailable")
        self.windows_ctypes = windows_ctypes
        kernel32 = windows_ctypes.WinDLL("kernel32", use_last_error=True)
        # ctypesはAPIを遅延生成するため、静的なProtocol照合より先に実関数を取得する。
        for name in (
            "CreateJobObjectW", "SetInformationJobObject", "AssignProcessToJobObject",
            "TerminateJobObject", "QueryInformationJobObject", "CloseHandle",
        ):
            function: object = getattr(kernel32, name, None)
            if not isinstance(function, _KernelFunction) or not callable(function):
                raise TypeError("invalid kernel32 function")
        if not isinstance(kernel32, _Kernel32):
            raise TypeError("invalid kernel32 Job Object API")
        self.kernel32 = kernel32
        self.kernel32.CreateJobObjectW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
        self.kernel32.CreateJobObjectW.restype = ctypes.c_void_p
        self.kernel32.SetInformationJobObject.argtypes = [
            ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32,
        ]
        self.kernel32.SetInformationJobObject.restype = ctypes.c_int
        self.kernel32.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.kernel32.AssignProcessToJobObject.restype = ctypes.c_int
        self.kernel32.TerminateJobObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        self.kernel32.TerminateJobObject.restype = ctypes.c_int
        self.kernel32.QueryInformationJobObject.argtypes = [
            ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32,
            ctypes.POINTER(ctypes.c_uint32),
        ]
        self.kernel32.QueryInformationJobObject.restype = ctypes.c_int
        self.kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
        self.kernel32.CloseHandle.restype = ctypes.c_int
        handle = self.kernel32.CreateJobObjectW(None, None)
        if handle is not None and type(handle) is not int:
            raise TypeError("invalid Job Object handle")
        self.handle: int | None = handle
        if not self.handle:
            raise self.windows_error()
        limits = _ExtendedLimitInformation()
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not self.kernel32.SetInformationJobObject(
            self.handle,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            ctypes.byref(limits),
            ctypes.sizeof(limits),
        ):
            try:
                error = self.windows_error()
            finally:
                self.close()
            raise error

    def windows_error(self) -> OSError:
        code = self.windows_ctypes.get_last_error()
        if type(code) is not int:
            raise TypeError("invalid Windows last-error code")
        error = self.windows_ctypes.WinError(code)
        if not isinstance(error, OSError):
            raise TypeError("invalid Windows error")
        return error

    def assign(self, process_handle: object) -> None:
        if not self.kernel32.AssignProcessToJobObject(self.handle, process_handle):
            raise self.windows_error()

    def terminate(self) -> None:
        if not self.kernel32.TerminateJobObject(self.handle, 1):
            raise self.windows_error()

    def wait_empty(self) -> None:
        while True:
            info = _BasicAccountingInformation()
            if not self.kernel32.QueryInformationJobObject(
                self.handle,
                JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
                ctypes.byref(info),
                ctypes.sizeof(info),
                None,
            ):
                raise self.windows_error()
            if info.ActiveProcesses == 0:
                return
            time.sleep(0.01)

    def close(self) -> None:
        if self.handle:
            handle = self.handle
            if not self.kernel32.CloseHandle(handle):
                raise self.windows_error()
            self.handle = None


class _Job(Protocol):
    def assign(self, process_handle: object) -> None: ...
    def terminate(self) -> None: ...
    def wait_empty(self) -> None: ...
    def close(self) -> None: ...


class _Closable(Protocol):
    def close(self) -> None: ...


class _ProcessHandle(Protocol):
    def Close(self) -> None: ...


@runtime_checkable
class _ReviewProcess(Protocol):
    _handle: _ProcessHandle
    stdin: _Closable
    returncode: object

    def communicate(self, input: str | None = None, timeout: float | None = None) -> object: ...
    def poll(self) -> object: ...
    def kill(self) -> None: ...


class _PopenFactory(Protocol):
    def __call__(
        self, args: list[str], *, cwd: Path, env: dict[str, str], stdin: int,
        stdout: int, stderr: TextIO, text: bool, encoding: str,
    ) -> object: ...


def _launcher() -> int:
    try:
        request = json.load(sys.stdin)
        arguments = request["arguments"]
        prompt = request["prompt"]
        if not isinstance(arguments, list) or not all(isinstance(arg, str) for arg in arguments):
            return 2
        if not isinstance(prompt, str):
            return 2
        completed = subprocess.run(
            arguments, input=prompt, text=True, encoding="utf-8", stdout=subprocess.DEVNULL
        )
        return completed.returncode
    except (EOFError, json.JSONDecodeError, KeyError, TypeError, OSError):
        return 2


def run_review_process(
    arguments: list[str], root: Path, environment: dict[str, str], prompt: str,
    diagnostic: Path, timeout: float, *, job_factory: Callable[[], _Job] = _JobApi,
    popen_factory: _PopenFactory = subprocess.Popen,
) -> int:
    """Job割当より前にレビュー本体を起動しないlauncherを実行する。

    Popen._handleはAssignProcessToJobObjectへ渡すlauncherのprocess HANDLE。
    primary thread handleやsuspended processは操作しない。
    """
    job: _Job = job_factory()
    process = None
    assigned = False
    try:
        with diagnostic.open("w") as errors:
            candidate = popen_factory(
                [sys.executable, str(Path(__file__).resolve()), LAUNCHER_ARGUMENT],
                cwd=root, env=environment, stdin=subprocess.PIPE,
                stdout=subprocess.DEVNULL, stderr=errors, text=True, encoding="utf-8",
            )
            if not isinstance(candidate, _ReviewProcess):
                raise TypeError("invalid Windows review launcher process")
            process = candidate
            job.assign(process._handle)
            assigned = True
            process.communicate(
                json.dumps({"arguments": arguments, "prompt": prompt}), timeout=timeout
            )
            returncode = process.returncode
            if type(returncode) is not int:
                raise TypeError("invalid Windows review launcher exit code")
            return returncode
    finally:
        try:
            job.terminate()
        except BaseException as terminate_error:
            close_error = None
            try:
                # terminateに失敗した場合はkill-on-close handleを閉じて子孫を終了する。
                job.close()
            except BaseException as error:
                close_error = error
            if process is not None:
                try:
                    if process.poll() is None:
                        process.kill()
                    if not assigned:
                        process.stdin.close()
                    process.communicate()
                except BaseException as error:
                    if close_error is None:
                        close_error = error
                try:
                    process._handle.Close()
                except BaseException as error:
                    if close_error is None:
                        close_error = error
            if close_error is not None:
                raise terminate_error from close_error
            raise
        else:
            try:
                if process is not None:
                    try:
                        if process.poll() is None and not assigned:
                            process.kill()
                        process.communicate()
                    finally:
                        process._handle.Close()
            finally:
                try:
                    job.wait_empty()
                finally:
                    job.close()



if __name__ == "__main__" and sys.argv[1:] == [LAUNCHER_ARGUMENT]:
    raise SystemExit(_launcher())
