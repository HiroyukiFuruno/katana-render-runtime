#!/usr/bin/env python3
"""Update the isolated HTML performance harness lock and its explicit gate pin."""

from __future__ import annotations

import argparse
import hashlib
import re
import shlex
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HARNESS_MANIFEST = Path("scripts/html/performance-harness/Cargo.toml")
GATE = Path("scripts/release/verify_html_performance_evidence.py")
LOCK_PIN = re.compile(r'(?m)^(LOCK_SHA\s*=\s*)"([0-9a-f]{64})"$')
SHA256 = re.compile(r"[0-9a-f]{64}")
# Python 3.9 でも型 aliasの評価時にPEP 604の演算を実行しない。
CommandRunner = Callable[..., "subprocess.CompletedProcess[bytes] | subprocess.CompletedProcess[str]"]


def parse_cargo_command(value: str) -> list[str]:
    if "\0" in value:
        raise ValueError("--cargo must not contain a NUL byte")
    try:
        command = shlex.split(value, posix=True)
    except ValueError as error:
        raise ValueError(f"invalid --cargo command: {error}") from error
    if not command:
        raise ValueError("--cargo must not be empty")
    return command


def update_command(cargo: Sequence[str]) -> list[str]:
    return [*cargo, "update", "--manifest-path", str(HARNESS_MANIFEST)]


def lock_sha(root: Path) -> str:
    lock = root / HARNESS_MANIFEST.with_name("Cargo.lock")
    _require_regular(lock, "harness lock")
    return hashlib.sha256(lock.read_bytes()).hexdigest()


def _require_regular(path: Path, label: str) -> None:
    if path.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    if not path.is_file():
        raise ValueError(f"{label} must be a regular file")


def sync_lock_pin(root: Path, digest: str) -> None:
    if SHA256.fullmatch(digest) is None:
        raise ValueError("lock pin must be a lowercase SHA256")
    target = root / GATE
    _require_regular(target, "performance gate")
    source = target.read_text(encoding="utf-8")
    matches = list(LOCK_PIN.finditer(source))
    if len(matches) != 1:
        raise ValueError("performance gate must contain exactly one LOCK_SHA pin")
    updated = LOCK_PIN.sub(lambda match: f'{match.group(1)}"{digest}"', source, count=1)
    target.write_text(updated, encoding="utf-8")


def run_update(cargo: Sequence[str], root: Path = ROOT, runner: CommandRunner = subprocess.run) -> str:
    runner(update_command(cargo), cwd=root, check=True)
    digest = lock_sha(root)
    sync_lock_pin(root, digest)
    return digest


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo", required=True, help="cargo command, including an optional wrapper")
    args = parser.parse_args(argv)
    try:
        cargo = parse_cargo_command(args.cargo)
        print(run_update(cargo))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"performance harness lock update failed: {error}", file=sys.stderr)
        return getattr(error, "returncode", 1) or 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
