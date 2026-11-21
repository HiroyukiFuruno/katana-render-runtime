"""実入力の初回描画3秒と、closeを含むprobe全体60秒を検証する。"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import signal
import subprocess
import time
from typing import TypeGuard

DEADLINE_SECONDS = 60
FIRST_FRAME_LIMIT_MS = 3_000.0


class ProbeArguments(argparse.Namespace):
    input: Path
    binary: Path
    expected_sha256: str
    report: Path


def _string_keyed_record(value: object) -> dict[str, object] | None:
    if not isinstance(value, dict):
        return None
    record: dict[str, object] = {}
    for key, item in value.items():
        if not isinstance(key, str):
            return None
        record[key] = item
    return record


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def finite_nonnegative(value: object) -> TypeGuard[int | float]:
    if type(value) is int:
        return value >= 0
    if type(value) is float:
        return math.isfinite(value) and value >= 0
    return False


def valid_evidence(evidence: object) -> bool:
    record = _string_keyed_record(evidence)
    if record is None or record.get("closed") is not True:
        return False
    first_frame_ms = record.get("first_frame_ms")
    close_ms = record.get("close_ms")
    if not finite_nonnegative(first_frame_ms) or first_frame_ms >= FIRST_FRAME_LIMIT_MS:
        return False
    if not finite_nonnegative(close_ms) or close_ms >= DEADLINE_SECONDS * 1000:
        return False
    frame = _string_keyed_record(record.get("frame"))
    if frame is None:
        return False
    expected: dict[str, int | float | str] = {
        "width": 1280,
        "height": 900,
        "scale": 1.0,
        "pixel_format": "Rgba8",
        "pixel_bytes": 1280 * 900 * 4,
    }
    for key, expected_value in expected.items():
        value = frame.get(key)
        if type(value) is not type(expected_value) or value != expected_value:
            return False
    generation = frame.get("generation")
    return type(generation) is int and generation > 0


def group_exists(pid: int) -> bool:
    try:
        os.killpg(pid, 0)
    except ProcessLookupError:
        return False
    return True


def process_passed(result: object, evidence: object) -> bool:
    record = _string_keyed_record(result)
    if record is None:
        return False
    elapsed = record.get("elapsed_seconds")
    exit_code = record.get("exit_code")
    return (
        record.get("timed_out") is False
        and type(exit_code) is int
        and exit_code == 0
        and finite_nonnegative(elapsed)
        and elapsed < DEADLINE_SECONDS
        and record.get("remaining_process_group") is False
        and valid_evidence(evidence)
    )


def stop_group(pid: int) -> None:
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def execute(binary: Path, input_path: Path) -> dict[str, object]:
    started = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    process = subprocess.Popen([str(binary), str(input_path)], stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=DEADLINE_SECONDS)
    except subprocess.TimeoutExpired:
        timed_out = True
        stop_group(process.pid)
        stdout, stderr = process.communicate(timeout=5)
    elapsed = time.monotonic() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    remaining = group_exists(process.pid)
    if remaining:
        stop_group(process.pid)
    return {
        "elapsed_seconds": elapsed,
        "timed_out": timed_out,
        "exit_code": process.returncode,
        "remaining_process_group": remaining,
        "cpu_user_seconds": after.ru_utime - before.ru_utime,
        "cpu_system_seconds": after.ru_stime - before.ru_stime,
        "stdout": stdout.decode("utf-8", errors="strict"),
        "stderr": stderr.decode("utf-8", errors="strict"),
    }


def verify(args: ProbeArguments) -> dict[str, object]:
    input_path, binary = args.input.resolve(strict=True), args.binary.resolve(strict=True)
    input_sha = digest(input_path)
    if input_sha != args.expected_sha256:
        raise ValueError("input SHA-256 mismatch")
    binary_sha = digest(binary)
    result = execute(binary, input_path)
    if digest(binary) != binary_sha or digest(input_path) != input_sha:
        raise ValueError("input or executable changed during the probe")
    evidence: object = None
    stdout = result.get("stdout")
    if not isinstance(stdout, str):
        raise subprocess.SubprocessError("probe stdout is unavailable")
    try:
        evidence = json.loads(stdout)
    except json.JSONDecodeError:
        pass
    result.update({"input_sha256": input_sha, "input_bytes": input_path.stat().st_size,
                   "binary_sha256": binary_sha, "deadline_seconds": DEADLINE_SECONDS,
                   "evidence": evidence})
    result["passed"] = process_passed(result, evidence)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args: ProbeArguments = parser.parse_args(namespace=ProbeArguments())
    try:
        result = verify(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result = {"passed": False, "error": str(error), "deadline_seconds": DEADLINE_SECONDS}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in
                      ("passed", "elapsed_seconds", "exit_code", "timed_out", "error")}))
    return 0 if result.get("passed") is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
