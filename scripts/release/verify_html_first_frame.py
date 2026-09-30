"""実入力の初期描画・closeを、ビルドを除いた60秒の上限で検証する。"""

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

DEADLINE_SECONDS = 60


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def finite_nonnegative(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def valid_evidence(evidence):
    if not isinstance(evidence, dict) or evidence.get("closed") is not True:
        return False
    for key in ("first_frame_ms", "close_ms"):
        value = evidence.get(key)
        if not finite_nonnegative(value) or value >= DEADLINE_SECONDS * 1000:
            return False
    frame = evidence.get("frame")
    if not isinstance(frame, dict):
        return False
    expected = {"width": 1280, "height": 900, "scale": 1.0,
                "pixel_format": "Rgba8", "pixel_bytes": 1280 * 900 * 4}
    return all(type(frame.get(key)) is type(value) and frame[key] == value
               for key, value in expected.items()) and (
        type(frame.get("generation")) is int and frame["generation"] > 0
    )


def group_exists(pid):
    try:
        os.killpg(pid, 0)
    except ProcessLookupError:
        return False
    return True


def process_passed(result, evidence):
    elapsed = result.get("elapsed_seconds")
    return (result.get("timed_out") is False and type(result.get("exit_code")) is int
            and result["exit_code"] == 0 and finite_nonnegative(elapsed)
            and elapsed < DEADLINE_SECONDS and result.get("remaining_process_group") is False
            and valid_evidence(evidence))


def stop_group(pid):
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def execute(binary, input_path):
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
    return {"elapsed_seconds": elapsed, "timed_out": timed_out,
            "exit_code": process.returncode, "remaining_process_group": remaining,
            "cpu_user_seconds": after.ru_utime - before.ru_utime,
            "cpu_system_seconds": after.ru_stime - before.ru_stime,
            "stdout": stdout.decode("utf-8", errors="strict"),
            "stderr": stderr.decode("utf-8", errors="strict")}


def verify(args):
    input_path, binary = args.input.resolve(strict=True), args.binary.resolve(strict=True)
    input_sha = digest(input_path)
    if input_sha != args.expected_sha256:
        raise ValueError("input SHA-256 mismatch")
    binary_sha = digest(binary)
    result = execute(binary, input_path)
    if digest(binary) != binary_sha or digest(input_path) != input_sha:
        raise ValueError("input or executable changed during the probe")
    evidence = None
    try:
        evidence = json.loads(result["stdout"])
    except json.JSONDecodeError:
        pass
    result.update({"input_sha256": input_sha, "input_bytes": input_path.stat().st_size,
                   "binary_sha256": binary_sha, "deadline_seconds": DEADLINE_SECONDS,
                   "evidence": evidence})
    result["passed"] = process_passed(result, evidence)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = verify(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result = {"passed": False, "error": str(error), "deadline_seconds": DEADLINE_SECONDS}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in
                      ("passed", "elapsed_seconds", "exit_code", "timed_out", "error")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
