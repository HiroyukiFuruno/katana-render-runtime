#!/usr/bin/env python3
"""Initialize CI capacity state from the repository's pinned reference policy."""
from __future__ import annotations

import argparse
import json
import os
import stat
import sys
from pathlib import Path

import cargo_capacity_guard as guard
import cargo_capacity_guard_native_cli_observation as observation
import cargo_capacity_guard_state as capacity_state
from cargo_capacity_guard_lock import GuardLock


def _regular_or_absent(path: Path, label: str) -> bool:
    """Return whether path exists, rejecting links and non-regular entries."""
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return False
    if not stat.S_ISREG(mode):
        raise RuntimeError(f"{label} must be a regular file")
    return True


def _validate_tmp(root: Path) -> Path:
    temporary = root / "tmp"
    if temporary.is_symlink():
        raise RuntimeError("root/tmp must not be a symlink")
    if temporary.exists() and not temporary.is_dir():
        raise RuntimeError("root/tmp must be a directory")
    return temporary


def _existing_state(state_path: Path, root: Path, target: Path) -> bool:
    if not _regular_or_absent(state_path, "capacity state"):
        return False
    try:
        state = capacity_state.validate_state(
            json.loads(state_path.read_text(encoding="utf-8")), root, target
        )
        baseline = guard.validate_recorded_baseline(state, target)
        if baseline is None:
            raise RuntimeError("existing capacity state has no calibrated baseline")
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, RuntimeError, ValueError) as error:
        raise RuntimeError("existing capacity state is invalid") from error
    return True


def initialize(root_argument: str) -> str:
    if os.environ.get("CI") != "true" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("CI=true and GITHUB_ACTIONS=true are required")

    root = Path(root_argument)
    if not root.is_absolute() or ".." in root.parts:
        raise RuntimeError("root must be canonical and absolute")
    target = root / "target"
    # 既存factoryでroot、target、policyを検証する。
    reference_state = capacity_state.create_repository_reference(root, target)
    temporary = _validate_tmp(root)
    state_path = root / "tmp/.cargo-capacity-guard.json"
    write_path = state_path.with_suffix(state_path.suffix + ".tmp")
    lock_path = root / "tmp/.cargo-capacity-guard.lock"
    if temporary.exists():
        _regular_or_absent(lock_path, "capacity guard lock")
    if write_path.exists() or write_path.is_symlink():
        raise RuntimeError("capacity state temporary path already exists")

    # CI markerとpolicyの確認後にtmpを作成する。
    temporary.mkdir(parents=True, exist_ok=True)
    with GuardLock(lock_path):
        if _existing_state(state_path, root, target):
            return "capacity state already exists; preserved"
        if write_path.exists() or write_path.is_symlink():
            raise RuntimeError("capacity state temporary path already exists")
        guard.save_state(state_path, reference_state)
        try:
            saved = json.loads(state_path.read_text(encoding="utf-8"))
            capacity_state.validate_state(saved, root, target)
        except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
            raise RuntimeError("initialized capacity state failed validation") from error
    return "initialized historical repository capacity reference"


def _protected_report_identity(report_path: Path) -> dict[str, object]:
    report_record = observation.file_record(report_path)
    report_stat = report_path.stat()
    return observation.protected_identity(
        {
            **report_record,
            "dev": report_stat.st_dev,
            "ino": report_stat.st_ino,
            "mode": report_stat.st_mode,
        }
    )


def _write_performance_binding(root: Path, state_path: Path,
                                protected_report: dict[str, object], *,
                                directory: Path | None = None) -> str:
    temporary = _validate_tmp(root)
    binding_directory = directory or temporary / "html-performance-capacity"
    if (not binding_directory.is_relative_to(temporary)
            or binding_directory.parent.resolve(strict=True) != binding_directory.parent):
        raise RuntimeError("capacity binding directory is not canonical repository temporary storage")
    binding_directory.mkdir()
    registry_path = binding_directory / "protected-refs.json"
    binding_path = binding_directory / "binding.json"
    registry = {
        "schema": "krr-protected-refs-successor-v1",
        "physicalRefCount": 1,
        "refs": [protected_report],
    }
    with registry_path.open("x", encoding="utf-8") as stream:
        json.dump(registry, stream, sort_keys=True, separators=(",", ":"))
        stream.write("\n")
    binding = {
        "schema": "krr-html-performance-capacity-binding-v1",
        "state": observation.file_record(state_path),
        "protectedRegistry": observation.file_record(registry_path),
    }
    with binding_path.open("x", encoding="utf-8") as stream:
        json.dump(binding, stream, sort_keys=True, separators=(",", ":"))
        stream.write("\n")
    digest = observation.file_record(binding_path)["sha256"]
    if not isinstance(digest, str):
        raise RuntimeError("binding digest was not recorded")
    return digest


def write_html_performance_binding(root_argument: str, report_argument: Path, *,
                                   directory: Path | None = None) -> str:
    root = Path(root_argument)
    expected_report = root / "tmp/html-interaction-suite/krr.json"
    if report_argument != expected_report:
        raise RuntimeError("interaction report must be the producer's canonical current-run report")
    state_path = root / "tmp/.cargo-capacity-guard.json"
    if not _existing_state(state_path, root, root / "target"):
        raise RuntimeError("current-run capacity state is missing")
    return _write_performance_binding(root, state_path, _protected_report_identity(report_argument),
                                      directory=directory)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="canonical absolute repository root")
    parser.add_argument(
        "--interaction-report",
        type=Path,
        help="write a fresh binding for the exact producer interaction report",
    )
    args = parser.parse_args()
    try:
        message = (
            write_html_performance_binding(args.root, args.interaction_report)
            if args.interaction_report is not None
            else initialize(args.root)
        )
    except (OSError, RuntimeError, ValueError) as error:
        print(f"capacity state initialization failed: {error}", file=sys.stderr)
        return 1
    print(message)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
