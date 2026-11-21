"""Run the explicit KRR runtime-script Moon CAS diagnostic task."""

from __future__ import annotations

import argparse
import os
import shutil
import time
from pathlib import Path

from runtime_script_moon_cas import (
    ASSESSMENT,
    RUNTIME_CAS_OPERATION_ENV,
    RuntimeScriptCasDiagnostic,
    run_runtime_cas_operation,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--moon", nargs="+", required=True)
    parser.add_argument("--bun", type=Path, default=Path(shutil.which("bun") or "bun"))
    args = parser.parse_args()
    bun = args.bun.resolve()
    if not bun.is_file():
        raise RuntimeError("Bun executable is required")
    path_bun = shutil.which("bun", path=os.environ.get("PATH"))
    if path_bun is None or Path(path_bun).resolve() != bun:
        raise RuntimeError("diagnostic Bun path must match the current PATH tool")
    operation = os.environ.get(RUNTIME_CAS_OPERATION_ENV)
    if operation is not None:
        return run_runtime_cas_operation(operation)
    run_dir = ASSESSMENT / f"moon-contract-{time.time_ns()}"
    run_dir.mkdir(parents=True)
    return RuntimeScriptCasDiagnostic(args.moon, run_dir).execute()


if __name__ == "__main__":
    raise SystemExit(main())
