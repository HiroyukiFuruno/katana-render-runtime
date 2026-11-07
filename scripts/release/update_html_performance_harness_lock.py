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
from typing import NamedTuple

from html_performance_harness_dependency_closure import (
    RETAINED_UTF8_ITER_VERSION,
    _harness_closure,
    _raw_packages,
    load_pinned_parent_lock,
    verify_utf8_iter_retention,
)

ROOT = Path(__file__).resolve().parents[2]
HARNESS_MANIFEST = Path("scripts/html/performance-harness/Cargo.toml")
ROOT_MANIFEST = Path("Cargo.toml")
GATE = Path("scripts/release/verify_html_performance_evidence.py")
LOCK_PIN = re.compile(r'(?m)^(LOCK_SHA\s*=\s*)"([0-9a-f]{64})"$')
SHA256 = re.compile(r"[0-9a-f]{64}")
# Python 3.9 でも型 aliasの評価時にPEP 604の演算を実行しない。
CommandRunner = Callable[..., "subprocess.CompletedProcess[bytes] | subprocess.CompletedProcess[str]"]
RETAINED_REASONS = {
    "utf8_iter": "immutable reference dependency closure",
    "crc32fast": "same SDK and measurement dependency as the immutable reference",
    "flate2": "same SDK and PNG measurement dependency as the immutable reference",
    "smallvec": "same SDK and URL measurement dependency as the immutable reference",
}


class RetainedPackage(NamedTuple):
    name: str
    version: str
    reason: str


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


def retention_plan(parent_lock: bytes) -> list[RetainedPackage]:
    packages = _raw_packages(parent_lock)
    _harness, _roots, closure = _harness_closure(parent_lock)
    plan: list[RetainedPackage] = []
    for name, reason in RETAINED_REASONS.items():
        identities = [identity for identity in packages if identity[0] == name]
        if (len(identities) != 1 or identities[0] not in closure
                or identities[0][2] != "registry+https://github.com/rust-lang/crates.io-index"):
            raise ValueError(f"immutable HTML parent must contain one retained {name} in its closure")
        version = identities[0][1]
        if name == "utf8_iter" and version != RETAINED_UTF8_ITER_VERSION:
            raise ValueError("immutable HTML parent utf8_iter version differs")
        plan.append(RetainedPackage(name, version, reason))
    return plan


def retention_command(cargo: Sequence[str], manifest: Path, package: RetainedPackage) -> list[str]:
    return [*cargo, "update", "--package", package.name, "--precise", package.version,
            "--manifest-path", str(manifest)]


def verify_retained_packages(parent_lock: bytes, locks: Sequence[bytes],
                             plan: Sequence[RetainedPackage]) -> None:
    parent = _raw_packages(parent_lock)
    for package in plan:
        expected = [(identity, value) for identity, value in parent.items() if identity[0] == package.name]
        if len(expected) != 1 or expected[0][0][1] != package.version:
            raise ValueError(f"immutable HTML parent retained {package.name} differs")
        for lock in locks:
            actual = [(identity, value) for identity, value in _raw_packages(lock).items()
                      if identity[0] == package.name]
            if actual != expected:
                raise ValueError(f"Cargo.lock does not retain immutable {package.name}")


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
    parent_lock = load_pinned_parent_lock(root)
    plan = retention_plan(parent_lock)
    for package in plan:
        runner(retention_command(cargo, ROOT_MANIFEST, package), cwd=root, check=True)
    runner(update_command(cargo), cwd=root, check=True)
    for package in plan:
        runner(retention_command(cargo, HARNESS_MANIFEST, package), cwd=root, check=True)
    root_lock_path = root / ROOT_MANIFEST.with_name("Cargo.lock")
    harness_lock_path = root / HARNESS_MANIFEST.with_name("Cargo.lock")
    _require_regular(root_lock_path, "root lock")
    _require_regular(harness_lock_path, "harness lock")
    root_lock, harness_lock = root_lock_path.read_bytes(), harness_lock_path.read_bytes()
    verify_retained_packages(parent_lock, (root_lock, harness_lock), plan)
    verify_utf8_iter_retention(root_lock, harness_lock, parent_lock)
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
