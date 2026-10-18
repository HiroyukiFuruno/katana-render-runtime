"""macOSの同じ所有browser PIDだけを読取観測する。SOURCE案・実OS未実行。"""
from __future__ import annotations

import ctypes
import json
import os
import struct
import sys
from typing import TypedDict


class ProcessIdentity(TypedDict):
    pid: int
    parentPid: int
    startIdentity: str


class BrowserIdentity(ProcessIdentity):
    executablePath: str
    argv: list[str]


class Observation(TypedDict):
    schema: str
    platform: str
    browser: BrowserIdentity
    producer: ProcessIdentity
    parentChain: list[ProcessIdentity]
    collector: ProcessIdentity
    beforeEndEqual: bool


def native_integer(value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise RuntimeError("native process observation must return an integer")
    return value


class ProcBsdInfo(ctypes.Structure):
    # 現SDK sys/proc_info.hのproc_bsdinfoと同じ並び。ABI不一致は拒否する。
    _fields_ = [
        (name, ctypes.c_uint32) for name in (
            "flags", "status", "xstatus", "pid", "ppid", "uid", "gid", "ruid",
            "rgid", "svuid", "svgid", "reserved",
        )
    ] + [
        ("comm", ctypes.c_char * 16), ("name", ctypes.c_char * 32),
    ] + [
        (name, ctypes.c_uint32) for name in ("nfiles", "pgid", "jobc", "tdev", "tpgid")
    ] + [
        ("nice", ctypes.c_int32), ("start_seconds", ctypes.c_uint64),
        ("start_microseconds", ctypes.c_uint64),
    ]


def run(browser_pid: int, producer_pid: int) -> Observation:
    if sys.platform != "darwin" or ctypes.sizeof(ProcBsdInfo) != 136:
        raise RuntimeError("unsupported OS or proc_bsdinfo ABI")
    libproc = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    libc = ctypes.CDLL(None, use_errno=True)
    libproc.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
    libproc.proc_pidinfo.restype = ctypes.c_int
    libproc.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
    libproc.proc_pidpath.restype = ctypes.c_int
    libc.sysctl.argtypes = [ctypes.POINTER(ctypes.c_int), ctypes.c_uint, ctypes.c_void_p,
                           ctypes.POINTER(ctypes.c_size_t), ctypes.c_void_p, ctypes.c_size_t]
    libc.sysctl.restype = ctypes.c_int

    def info(pid: int) -> ProcessIdentity:
        row = ProcBsdInfo()
        if native_integer(libproc.proc_pidinfo(pid, 3, 0, ctypes.byref(row), ctypes.sizeof(row))) != ctypes.sizeof(row) or row.pid != pid:
            raise RuntimeError("owned process disappeared or proc_pidinfo failed")
        return {"pid": native_integer(row.pid), "parentPid": native_integer(row.ppid),
                "startIdentity": f"{native_integer(row.start_seconds)}.{native_integer(row.start_microseconds):06d}"}

    def executable(pid: int) -> str:
        buffer = ctypes.create_string_buffer(4096)
        if native_integer(libproc.proc_pidpath(pid, buffer, len(buffer))) <= 0:
            raise RuntimeError("actual executable path unavailable")
        return os.fsdecode(buffer.value)

    def argv(pid: int) -> list[str]:
        mib = (ctypes.c_int * 3)(1, 49, pid)  # CTL_KERN / KERN_PROCARGS2 / PID
        size = ctypes.c_size_t()
        if native_integer(libc.sysctl(mib, 3, None, ctypes.byref(size), None, 0)) != 0 or not 4 < native_integer(size.value) <= 1024 * 1024:
            raise RuntimeError("actual argv size unavailable")
        buffer = ctypes.create_string_buffer(native_integer(size.value))
        if native_integer(libc.sysctl(mib, 3, buffer, ctypes.byref(size), None, 0)) != 0:
            raise RuntimeError("actual argv unavailable")
        raw = buffer.raw[:native_integer(size.value)]
        argc = native_integer(struct.unpack_from("i", raw)[0])
        if not 0 < argc <= 8192:
            raise RuntimeError("invalid actual argc")
        offset = raw.index(b"\0", 4) + 1
        while offset < len(raw) and raw[offset] == 0:
            offset += 1
        result = []
        for _ in range(argc):
            end = raw.index(b"\0", offset)
            result.append(os.fsdecode(raw[offset:end]))
            offset = end + 1
        if not result or not os.path.isabs(result[0]):
            raise RuntimeError("actual argv executable missing")
        return result

    browser_identity = info(browser_pid)
    browser: BrowserIdentity = {**browser_identity, "executablePath": executable(browser_pid),
                                "argv": argv(browser_pid)}
    producer = info(producer_pid)
    chain: list[ProcessIdentity] = []
    visited, parent = {browser_pid}, browser["parentPid"]
    while parent != producer_pid:
        if parent <= 0 or parent in visited or len(chain) >= 63:
            raise RuntimeError("browser does not belong to producer")
        visited.add(parent)
        row = info(parent)
        chain.append(row)
        parent = row["parentPid"]
    chain.append(producer)
    # PID再利用・exec・親関係変更を同じ観測内でも拒否する。
    if any(info(row["pid"]) != row for row in chain):
        raise RuntimeError("owned parent chain changed")
    if info(browser_pid) != browser_identity or \
            executable(browser_pid) != browser["executablePath"] or argv(browser_pid) != browser["argv"]:
        raise RuntimeError("actual browser process changed during observation")
    return {"schema": "katana-macos-owned-browser-process-v1", "platform": "darwin",
            "browser": browser, "producer": producer, "parentChain": chain,
            "collector": info(os.getpid()), "beforeEndEqual": True}


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("exact browser/producer PID arguments required")
    browser_pid, producer_pid = map(int, sys.argv[1:])
    if browser_pid <= 0 or producer_pid <= 0 or browser_pid == producer_pid or os.getppid() != producer_pid:
        raise SystemExit("invalid owned browser/producer PID")
    print(json.dumps(run(browser_pid, producer_pid), sort_keys=True))
