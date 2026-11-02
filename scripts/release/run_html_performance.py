"""Run the fixed HTML performance programs in their pinned Python environment."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
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
BASE_PYTHON_ENV = "KRR_HTML_PERFORMANCE_BASE_PYTHON_SHA256"
BASE_PYTHON_PATH_ENV = "KRR_HTML_PERFORMANCE_BASE_PYTHON_PATH"
UV_ENV = "KRR_HTML_PERFORMANCE_UV_SHA256"
UV_PATH_ENV = "KRR_HTML_PERFORMANCE_UV_PATH"
VENV = ROOT / "tmp/html-performance-python-venv"
REQUIREMENTS = ROOT / "scripts/release/html-performance-requirements.lock"


def _environment_sha256(environment: dict[str, str]) -> str:
    payload = json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return hashlib.sha256(payload).hexdigest()


def _command(mode: str, arguments: Sequence[str]) -> list[str]:
    if mode == "verify":
        program = ROOT / "scripts/release/verify_html_performance_evidence.py"
    elif mode == "produce":
        program = ROOT / "scripts/release/produce_html_performance_evidence.py"
    elif mode == "reuse":
        program = ROOT / "scripts/release/html_performance_source_reuse_cli.py"
    elif mode == "promote":
        program = ROOT / "scripts/release/html_performance_promotion.py"
    elif mode == "test":
        return [sys.executable, "-m", "unittest", "discover", "-s", "scripts/release", "-p", "*_test.py", "--failfast", *arguments]
    else:
        raise ValueError(f"unsupported HTML performance mode: {mode}")
    return [sys.executable, str(program), *arguments]


def _file_sha256(path: Path) -> str:
    return hashlib.sha256(path.resolve(strict=True).read_bytes()).hexdigest()


def _venv_python() -> Path:
    return VENV / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def _require_private_venv_path() -> None:
    if VENV.is_symlink() or VENV.resolve() != VENV or not VENV.is_relative_to(ROOT):
        raise RuntimeError("HTML performance virtual environment path is invalid")


def bootstrap(arguments: Sequence[str]) -> int:
    uv_path = shutil.which("uv")
    if uv_path is None:
        raise RuntimeError("uv executable unavailable")
    uv = str(Path(uv_path).resolve(strict=True))
    python = str(Path(sys.executable).resolve(strict=True))
    python_sha = _file_sha256(Path(python))
    uv_sha = _file_sha256(Path(uv))
    _require_private_venv_path()
    if not (VENV / "pyvenv.cfg").is_file():
        subprocess.run([uv, "venv", "--python", python, str(VENV)], cwd=ROOT, check=True)
    _require_private_venv_path()
    environment = dict(os.environ)
    environment[BASE_PYTHON_ENV] = python_sha
    environment[BASE_PYTHON_PATH_ENV] = python
    environment[UV_ENV] = uv_sha
    environment[UV_PATH_ENV] = uv
    subprocess.run(
        [str(_venv_python()), str(Path(__file__).resolve()), "--check-base"],
        cwd=ROOT, env=environment, check=True,
    )
    subprocess.run(
        [uv, "pip", "sync", "--python", str(_venv_python()), "--require-hashes", str(REQUIREMENTS)],
        cwd=ROOT,
        check=True,
    )
    if _file_sha256(Path(python)) != python_sha or _file_sha256(Path(uv)) != uv_sha:
        raise RuntimeError("HTML performance bootstrap tool identity changed")
    command = [str(_venv_python()), str(Path(__file__).resolve()), "--prepared", *arguments]
    return subprocess.run(command, cwd=ROOT, env=environment, check=False).returncode


def _verify_prepared_python() -> None:
    base = getattr(sys, "_base_executable", None)
    if (
        not isinstance(base, str)
        or not base
        or str(Path(base).resolve(strict=True)) != os.environ.get(BASE_PYTHON_PATH_ENV)
        or _file_sha256(Path(base)) != os.environ.get(BASE_PYTHON_ENV)
    ):
        raise RuntimeError("HTML performance base Python identity differs")


def _child_environment(identity: str) -> dict[str, str]:
    environment = dict(os.environ)
    environment[IDENTITY_ENV] = identity
    environment[VERSION_ENV] = SCIPY_VERSION
    environment[ENVIRONMENT_ENV] = _environment_sha256(environment)
    return environment


def run_mode(mode: str, arguments: Sequence[str]) -> int:
    before = installed_identity(SCIPY_VERSION, NUMPY_VERSION)
    environment = _child_environment(before)
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
    if sys.argv[1:] == ["--check-base"]:
        _verify_prepared_python()
        return 0
    if sys.argv[1:2] == ["--bootstrap"]:
        return bootstrap(sys.argv[2:])
    if sys.argv[1:2] == ["--prepared"]:
        _verify_prepared_python()
        del sys.argv[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("verify", "produce", "reuse", "promote", "test"))
    args, arguments = parser.parse_known_args()
    return run_mode(args.mode, arguments)


if __name__ == "__main__":
    raise SystemExit(main())
