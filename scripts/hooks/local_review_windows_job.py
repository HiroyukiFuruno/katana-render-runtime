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
from typing import Protocol


LAUNCHER_ARGUMENT = "--local-review-launcher"
JOB_OBJECT_EXTENDED_LIMIT_INFORMATION = 9
JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION = 1
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000


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
        self.kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
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
        self.handle = self.kernel32.CreateJobObjectW(None, None)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = _ExtendedLimitInformation()
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not self.kernel32.SetInformationJobObject(
            self.handle,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            ctypes.byref(limits),
            ctypes.sizeof(limits),
        ):
            error = ctypes.WinError(ctypes.get_last_error())
            self.close()
            raise error

    def assign(self, process_handle: int) -> None:
        if not self.kernel32.AssignProcessToJobObject(self.handle, process_handle):
            raise ctypes.WinError(ctypes.get_last_error())

    def terminate(self) -> None:
        if not self.kernel32.TerminateJobObject(self.handle, 1):
            raise ctypes.WinError(ctypes.get_last_error())

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
                raise ctypes.WinError(ctypes.get_last_error())
            if info.ActiveProcesses == 0:
                return
            time.sleep(0.01)

    def close(self) -> None:
        if self.handle:
            handle = self.handle
            if not self.kernel32.CloseHandle(handle):
                raise ctypes.WinError(ctypes.get_last_error())
            self.handle = None


class _Job(Protocol):
    def assign(self, process_handle: int) -> None: ...
    def terminate(self) -> None: ...
    def wait_empty(self) -> None: ...
    def close(self) -> None: ...


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
    diagnostic: Path, timeout: float, *, job_factory=_JobApi, popen_factory=subprocess.Popen,
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
            process = popen_factory(
                [sys.executable, str(Path(__file__).resolve()), LAUNCHER_ARGUMENT],
                cwd=root, env=environment, stdin=subprocess.PIPE,
                stdout=subprocess.DEVNULL, stderr=errors, text=True, encoding="utf-8",
            )
            job.assign(process._handle)
            assigned = True
            process.communicate(
                json.dumps({"arguments": arguments, "prompt": prompt}), timeout=timeout
            )
            return process.returncode
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
