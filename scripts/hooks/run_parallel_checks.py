from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import time

from local_review_lock import review_lock
from local_review_state import ReviewError, command, strict_json


VERIFY_COMMAND = ["bun", "scripts/html/harness-contract-cli.ts", "--verify-moon-run"]
REPORT_RELATIVE_PATH = Path(".moon/cache/runReport.json")
LOCK_RELATIVE_PATH = Path(".moon/cache/krr-terminal-report.lock")
AUTHORITY_REGISTRY_PATH = "scripts/html/moon-authority-inputs.json"
REQUIRED_AUTHORITY_BOOTSTRAP_PATHS = {
    AUTHORITY_REGISTRY_PATH,
    "scripts/html/harness-contract-cli.ts",
    "scripts/hooks/run_parallel_checks.py",
}
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}\Z")
HEAD_PATTERN = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")


def _stat_identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _read_stable_file(path: Path) -> tuple[bytes, os.stat_result]:
    try:
        before_path = path.lstat()
        if not stat.S_ISREG(before_path.st_mode):
            raise ReviewError(f"expected regular file: {path}")
        with path.open("rb") as source:
            before_fd = os.fstat(source.fileno())
            if _stat_identity(before_fd) != _stat_identity(before_path):
                raise ReviewError(f"file changed before read: {path}")
            contents = source.read()
            after_fd = os.fstat(source.fileno())
        after_path = path.lstat()
    except OSError as error:
        raise ReviewError(f"unable to read file: {path}") from error

    identity = _stat_identity(before_fd)
    if identity != _stat_identity(after_fd) or identity != _stat_identity(after_path):
        raise ReviewError(f"file changed during read: {path}")
    if len(contents) != before_fd.st_size:
        raise ReviewError(f"short file read: {path}")
    return contents, after_fd


def _authority_paths(root: Path) -> tuple[str, ...]:
    contents, _metadata = _read_stable_file(root / AUTHORITY_REGISTRY_PATH)
    try:
        envelope = strict_json(contents.decode("utf-8"))
    except (UnicodeDecodeError, ReviewError) as error:
        raise ReviewError("Moon authority registry is invalid JSON") from error
    if (
        not isinstance(envelope, dict)
        or set(envelope) != {"schema", "inputs"}
        or type(envelope.get("schema")) is not int
        or envelope["schema"] != 1
    ):
        raise ReviewError("Moon authority registry has an invalid schema")
    inputs = envelope["inputs"]
    if not isinstance(inputs, dict) or not inputs:
        raise ReviewError("Moon authority registry inputs must be a non-empty object")
    paths = tuple(inputs)
    if any(type(value) is not bool or value is not True for value in inputs.values()):
        raise ReviewError("Moon authority registry entries must be true")
    for name in paths:
        if (
            not isinstance(name, str)
            or not name
            or "\\" in name
            or PurePosixPath(name).is_absolute()
            or PurePosixPath(name).as_posix() != name
            or any(part in {"", ".", ".."} for part in name.split("/"))
        ):
            raise ReviewError("Moon authority registry contains an unsafe path")
    if not REQUIRED_AUTHORITY_BOOTSTRAP_PATHS.issubset(paths):
        raise ReviewError("Moon authority registry omits a bootstrap authority")
    return paths


def _authority_sha256(root: Path, authority_paths: tuple[str, ...] | None = None) -> dict[str, str]:
    paths = _authority_paths(root) if authority_paths is None else authority_paths
    return {
        name: hashlib.sha256(_read_stable_file(root / name)[0]).hexdigest()
        for name in paths
    }


def _validate_object_json(raw_text: str, label: str) -> dict[str, object]:
    value = strict_json(raw_text)
    if not isinstance(value, dict):
        raise ReviewError(f"invalid {label}: expected JSON object")
    return value


def _preflight_authority(
    raw_text: str,
    current: dict[str, str],
    authority_paths: tuple[str, ...],
) -> None:
    envelope = _validate_object_json(raw_text, "preflight")
    if set(envelope) != {"schema", "staticContractOnly", "errors", "moonRunContract"}:
        raise ReviewError("preflight fields differ from the accepted schema")
    if type(envelope["schema"]) is not int or envelope["schema"] != 1:
        raise ReviewError("preflight schema is unsupported")
    if envelope["staticContractOnly"] is not True:
        raise ReviewError("preflight is not a static contract result")
    errors = envelope["errors"]
    if not isinstance(errors, list) or not all(isinstance(error, str) for error in errors):
        raise ReviewError("preflight errors field is invalid")
    if errors:
        raise ReviewError(f"preflight contract failed: {'; '.join(errors)}")

    contract = envelope["moonRunContract"]
    if not isinstance(contract, dict):
        raise ReviewError("Moon run contract is invalid")
    input_sha256 = contract.get("inputSha256")
    if not isinstance(input_sha256, dict) or set(input_sha256) != set(authority_paths):
        raise ReviewError("preflight authority set differs from the exact required inputs")
    if any(not isinstance(value, str) or not SHA256_PATTERN.fullmatch(value) for value in input_sha256.values()):
        raise ReviewError("preflight authority contains an invalid SHA-256")
    if input_sha256 != current:
        raise ReviewError("preflight authority SHA-256 differs from current files")


def _report_snapshot(path: Path) -> tuple[str, tuple[int, int, int, int, int, int]] | None:
    try:
        contents, metadata = _read_stable_file(path)
    except ReviewError as error:
        if isinstance(error.__cause__, FileNotFoundError):
            return None
        raise
    return hashlib.sha256(contents).hexdigest(), _stat_identity(metadata)


def _decimal(value: int) -> str:
    if value < 0:
        raise ReviewError("wall clock returned a negative nanosecond value")
    return str(value)


def _head(root: Path) -> str:
    value = command(["git", "rev-parse", "HEAD"], root).strip()
    if not HEAD_PATTERN.fullmatch(value):
        raise ReviewError("Git HEAD has an unexpected format")
    return value


def _report_text(contents: bytes) -> str:
    try:
        value = contents.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReviewError("Moon report is not UTF-8") from error
    _validate_object_json(value, "Moon report")
    return value


def _make_verification_request(
    preflight_text: str,
    report_text: str,
    report_sha256: str,
    input_sha256_before: dict[str, str],
    input_sha256_after: dict[str, str],
    head_before: str,
    head_after: str,
    started_ns: int,
    finished_ns: int,
    report_mtime_ns: int,
) -> str:
    request = {
        "schema": 1,
        "preflightText": preflight_text,
        "reportText": report_text,
        "reportSha256": report_sha256,
        "inputSha256Before": input_sha256_before,
        "inputSha256After": input_sha256_after,
        "headBefore": head_before,
        "headAfter": head_after,
        "startedNs": _decimal(started_ns),
        "finishedNs": _decimal(finished_ns),
        "reportMtimeNs": _decimal(report_mtime_ns),
    }
    return json.dumps(request, ensure_ascii=False, separators=(",", ":"))


def _validate_success_proof(
    raw_text: str,
    report_sha256: str,
    input_sha256: dict[str, str],
    head: str,
    started_ns: int,
    finished_ns: int,
    report_mtime_ns: int,
) -> None:
    proof = _validate_object_json(raw_text, "Moon success proof")
    expected = {
        "schema": 1,
        "status": "verified",
        "reportSha256": report_sha256,
        "inputSha256": input_sha256,
        "head": head,
        "startedNs": _decimal(started_ns),
        "finishedNs": _decimal(finished_ns),
        "reportMtimeNs": _decimal(report_mtime_ns),
    }
    if set(proof) != set(expected) or type(proof.get("schema")) is not int or proof != expected:
        raise ReviewError("Moon verification proof does not match this invocation")


def run_checks(jobs: int) -> int:
    if jobs < 1:
        raise ValueError("jobs must be at least 1")

    try:
        root = Path(__file__).resolve().parents[2]
        if Path.cwd().resolve() != root:
            raise ReviewError("run the KRR checks from the repository root")

        authority_paths = _authority_paths(root)
        input_sha256_before = _authority_sha256(root, authority_paths)
        head_before = _head(root)
        preflight = subprocess.run(
            ["bun", "scripts/html/harness-contract-cli.ts"],
            check=False,
            shell=False,
            cwd=root,
            capture_output=True,
            text=True,
        )
        if preflight.stdout:
            sys.stdout.write(preflight.stdout)
            sys.stdout.flush()
        if preflight.stderr:
            sys.stderr.write(preflight.stderr)
            sys.stderr.flush()
        if preflight.returncode != 0:
            return preflight.returncode
        _preflight_authority(preflight.stdout, input_sha256_before, authority_paths)

        lock_path = root / LOCK_RELATIVE_PATH
        lock_path.parent.mkdir(parents=True, exist_ok=True)
        with review_lock(lock_path):
            report_path = root / REPORT_RELATIVE_PATH
            prior_report = _report_snapshot(report_path)
            started_ns = time.time_ns()
            if prior_report is not None and prior_report[1][4] >= started_ns:
                raise ReviewError("previous Moon report mtime overlaps this invocation")
            moon_command = [
                "bun",
                "run",
                "moon",
                "--concurrency",
                str(jobs),
                "exec",
                "krr:coverage",
                "--on-failure",
                "continue",
                "--no-actions",
            ]
            result = subprocess.run(
                moon_command,
                check=False,
                shell=False,
                cwd=root,
            )
            finished_ns = time.time_ns()
            if result.returncode != 0:
                return result.returncode

            input_sha256_after = _authority_sha256(root, authority_paths)
            head_after = _head(root)
            if input_sha256_after != input_sha256_before or head_after != head_before:
                raise ReviewError("repository authority files or HEAD changed during Moon execution")

            contents, report_metadata = _read_stable_file(report_path)
            report_text = _report_text(contents)
            report_sha256 = hashlib.sha256(contents).hexdigest()
            if prior_report is not None and report_sha256 == prior_report[0]:
                raise ReviewError("Moon report bytes did not change during this invocation")
            if not started_ns < report_metadata.st_mtime_ns <= finished_ns:
                raise ReviewError("Moon report mtime is outside this invocation")

            verification_request = _make_verification_request(
                preflight.stdout,
                report_text,
                report_sha256,
                input_sha256_before,
                input_sha256_after,
                head_before,
                head_after,
                started_ns,
                finished_ns,
                report_metadata.st_mtime_ns,
            )
            before_verify_sha256 = _authority_sha256(root, authority_paths)
            before_verify_head = _head(root)
            if before_verify_sha256 != input_sha256_before or before_verify_head != head_before:
                raise ReviewError("repository authority changed before Moon verification")
            verification = subprocess.run(
                VERIFY_COMMAND,
                input=verification_request,
                check=False,
                shell=False,
                cwd=root,
                capture_output=True,
                text=True,
            )
            if verification.returncode != 0:
                if verification.stderr:
                    sys.stderr.write(verification.stderr)
                return verification.returncode
            if verification.stderr:
                sys.stderr.write(verification.stderr)
            _validate_success_proof(
                verification.stdout,
                report_sha256,
                input_sha256_before,
                head_before,
                started_ns,
                finished_ns,
                report_metadata.st_mtime_ns,
            )

            after_verify_sha256 = _authority_sha256(root, authority_paths)
            after_verify_head = _head(root)
            if after_verify_sha256 != input_sha256_before or after_verify_head != head_before:
                raise ReviewError("repository authority changed during Moon verification")
            verified_contents, verified_metadata = _read_stable_file(report_path)
            if (
                hashlib.sha256(verified_contents).hexdigest() != report_sha256
                or _stat_identity(verified_metadata) != _stat_identity(report_metadata)
            ):
                raise ReviewError("Moon report changed during verification")
    except ReviewError as error:
        message = str(error)
        if message == "another local review is active; do not start recursive reviews":
            message = "another KRR terminal-report check is active"
        print(f"quality-check report rejected: {message}", file=sys.stderr)
        return 1
    except (OSError, subprocess.SubprocessError) as error:
        print(f"quality-check execution failed: {error}", file=sys.stderr)
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the fixed KRR Moon check project.")
    parser.add_argument("--jobs", type=int, default=3)
    arguments = parser.parse_args()
    if arguments.jobs < 1:
        parser.error("--jobs must be at least 1")
    return run_checks(arguments.jobs)


if __name__ == "__main__":
    raise SystemExit(main())
