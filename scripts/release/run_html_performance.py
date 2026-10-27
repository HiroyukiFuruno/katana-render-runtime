"""Run the fixed HTML performance programs in their pinned Python environment."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from collections.abc import Sequence

from html_performance_python_identity import installed_identity


ROOT = Path(__file__).resolve().parents[2]
SCIPY_VERSION = "1.18.1"
NUMPY_VERSION = "2.5.4"
IDENTITY_ENV = "KRR_HTML_PERFORMANCE_PYTHON_IDENTITY"
VERSION_ENV = "KRR_HTML_PERFORMANCE_SCIPY_VERSION"
ENVIRONMENT_ENV = "KRR_HTML_PERFORMANCE_ENVIRONMENT_SHA256"


def _environment_sha256(environment: dict[str, str]) -> str:
    payload = json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return hashlib.sha256(payload).hexdigest()


def _command(mode: str, arguments: Sequence[str]) -> list[str]:
    if mode == "verify":
        program = ROOT / "scripts/release/verify_html_performance_evidence.py"
    elif mode == "produce":
        program = ROOT / "scripts/release/produce_html_performance_evidence.py"
    elif mode == "test":
        return [sys.executable, "-m", "unittest", "discover", "-s", "scripts/release", "-p", "*_test.py", "--failfast", *arguments]
    else:
        raise ValueError(f"unsupported HTML performance mode: {mode}")
    return [sys.executable, str(program), *arguments]


def run_mode(mode: str, arguments: Sequence[str]) -> int:
    environment = dict(os.environ)
    before = installed_identity(SCIPY_VERSION, NUMPY_VERSION)
    environment[IDENTITY_ENV] = before
    environment[VERSION_ENV] = SCIPY_VERSION
    environment[ENVIRONMENT_ENV] = _environment_sha256(environment)
    completed = subprocess.run(_command(mode, arguments), cwd=ROOT, env=environment, check=False)
    try:
        after = installed_identity(SCIPY_VERSION, NUMPY_VERSION)
    except Exception as error:
        print(f"HTML performance Python identity changed or became unavailable: {error}", file=sys.stderr)
        return completed.returncode if completed.returncode != 0 else 1
    if after != before:
        print("HTML performance Python identity changed while the child process ran", file=sys.stderr)
        return completed.returncode if completed.returncode != 0 else 1
    return completed.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("verify", "produce", "test"))
    args, arguments = parser.parse_known_args()
    return run_mode(args.mode, arguments)


if __name__ == "__main__":
    raise SystemExit(main())
