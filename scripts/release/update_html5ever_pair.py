#!/usr/bin/env python3
"""Update the HTML parser and its markup model as one compatibility unit."""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path


PACKAGE_NAMES = ("html5ever", "markup5ever")
ROOT = Path(__file__).resolve().parents[2]
CommandRunner = Callable[..., subprocess.CompletedProcess[bytes] | subprocess.CompletedProcess[str]]


def parse_cargo_command(value: str) -> list[str]:
    """Parse the configured cargo command without ever invoking a shell."""

    if "\0" in value:
        raise ValueError("--cargo must not contain a NUL byte")
    try:
        command = shlex.split(value, posix=True)
    except ValueError as error:
        raise ValueError(f"invalid --cargo command: {error}") from error
    if not command:
        raise ValueError("--cargo must not be empty")
    return command


def update_commands(cargo: Sequence[str]) -> tuple[list[str], list[str]]:
    """Keep html5ever and markup5ever on the same release line.

    ``markup5ever_rcdom`` is implemented by this workspace (see ``lib.rs``),
    so it has no registry package to upgrade.  Updating the parser and the
    registry-provided markup model together preserves that local adapter's
    compatibility boundary.
    """

    upgrade = [*cargo, "upgrade", "--incompatible", "allow"]
    for package in PACKAGE_NAMES:
        upgrade.extend(("--package", package))
    resolve = [*cargo, "update", "--recursive", *PACKAGE_NAMES]
    return upgrade, resolve


def run_updates(cargo: Sequence[str], runner: CommandRunner = subprocess.run) -> None:
    for command in update_commands(cargo):
        runner(command, cwd=ROOT, check=True)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo", required=True, help="cargo command, including an optional wrapper")
    parser.add_argument("--dry-run", action="store_true", help="print commands without changing files")
    args = parser.parse_args(argv)

    try:
        cargo = parse_cargo_command(args.cargo)
    except ValueError as error:
        parser.error(str(error))

    commands = update_commands(cargo)
    if args.dry_run:
        for command in commands:
            print(shlex.join(command))
        return 0

    try:
        run_updates(cargo)
    except subprocess.CalledProcessError as error:
        print(f"html5ever compatibility update failed: {shlex.join(error.cmd)}", file=sys.stderr)
        return error.returncode or 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
