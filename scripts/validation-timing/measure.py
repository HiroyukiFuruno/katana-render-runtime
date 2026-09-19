#!/usr/bin/env python3
"""Measure one explicitly supplied command for audit purposes.

The JSON record is observational data only.  It is never a proof that a
future check may be skipped.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time


def current_head() -> str:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return "unknown"
    return result.stdout.strip()


def dirty_at_start() -> bool | None:
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return bool(result.stdout)


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="milliseconds")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Measure one explicit command and write a JSON audit record."
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--check-id",
        default="adhoc",
        help="stable identifier for the measured check",
    )
    parser.add_argument(
        "--input",
        dest="inputs",
        action="append",
        type=Path,
        default=[],
        help="input file to include in the content digest; may be repeated",
    )
    parser.add_argument(
        "command",
        nargs=argparse.REMAINDER,
        help="command and arguments after --; no shell parsing is performed",
    )
    args = parser.parse_args()
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if not args.command:
        parser.error("an explicit command is required after --")
    return args


def digest_inputs(inputs: list[Path], cwd: Path) -> tuple[list[str], str]:
    """Hash explicitly declared inputs without implicitly trusting the worktree."""
    digest = hashlib.sha256()
    names: list[str] = []
    for input_path in inputs:
        resolved = input_path.resolve()
        if not resolved.is_file():
            raise ValueError(f"input is not a regular file: {input_path}")
        try:
            name = resolved.relative_to(cwd.resolve()).as_posix()
        except ValueError:
            name = str(resolved)
        names.append(name)
        digest.update(name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(resolved.read_bytes())
        digest.update(b"\0")
    return names, digest.hexdigest()


def write_record(path: Path, record: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as temporary:
        temporary.write(json.dumps(record, ensure_ascii=True, indent=2) + "\n")
        temporary_path = Path(temporary.name)
    temporary_path.replace(path)


def main() -> int:
    args = parse_args()
    cwd_path = Path.cwd()
    cwd = str(cwd_path)
    head_at_start = current_head()
    dirty = dirty_at_start()
    try:
        input_names, input_digest = digest_inputs(args.inputs, cwd_path)
    except (OSError, ValueError) as error:
        print(f"validation-timing: {error}", file=sys.stderr)
        return 2
    started_at = utc_now()
    started_monotonic = time.monotonic()
    try:
        completed = subprocess.run(
            args.command,
            check=False,
            shell=False,
        )
        exit_code = completed.returncode
    except OSError:
        exit_code = 127
    ended_monotonic = time.monotonic()
    record = {
        "started_at": started_at,
        "ended_at": utc_now(),
        "duration_seconds": ended_monotonic - started_monotonic,
        "exit_code": exit_code,
        "check_id": args.check_id,
        "invocation_count": 1,
        "command": args.command,
        "inputs": input_names,
        "input_digest": input_digest,
        "head_at_start": head_at_start,
        "head_at_end": current_head(),
        "cwd": cwd,
        "dirty_at_start": dirty,
        "os": platform.system(),
        "arch": platform.machine(),
    }
    write_record(args.output, record)
    return 128 + (-exit_code) if exit_code < 0 else exit_code


if __name__ == "__main__":
    sys.exit(main())
