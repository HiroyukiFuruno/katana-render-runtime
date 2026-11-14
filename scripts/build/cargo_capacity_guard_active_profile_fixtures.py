from __future__ import annotations

from collections.abc import Sequence
import hashlib
import itertools
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import TYPE_CHECKING, TypedDict

import cargo_capacity_guard as guard

if TYPE_CHECKING:
    from typing import TypeGuard

CALLS = itertools.count()
FileIdentity = tuple[str, int, int, int, str]


class WorkspaceMetadata(TypedDict):
    packages: list[guard.JsonObject]
    workspace_members: list[str]


class CompilerMessage(TypedDict, total=False):
    reason: str
    fresh: bool


class _CleanupFields(TypedDict):
    deleted_files: int


class CleanupReport(_CleanupFields, total=False):
    error: str


class _GuardFields(TypedDict):
    threshold_bytes: int | float | None
    current_bytes: int
    post_clean_bytes: int | None
    actual_removed_bytes: int
    cleanup_status: str


class GuardReport(_GuardFields, total=False):
    finite_cleanup: CleanupReport


class FixtureState(TypedDict):
    baseline_bytes: int


def _metadata_shape(value: object) -> TypeGuard[WorkspaceMetadata]:
    metadata = guard._object(value, "fixture Cargo metadata")
    for package in guard._array(metadata["packages"], "fixture Cargo packages"):
        guard._object(package, "fixture Cargo package")
    for member in guard._array(metadata["workspace_members"], "fixture workspace members"):
        guard._text(member, "fixture workspace member")
    return True


def workspace_metadata(value: object) -> WorkspaceMetadata:
    if not _metadata_shape(value):
        raise AssertionError("actual Cargo metadata shape is required")
    return value


def _message_shape(value: object) -> TypeGuard[CompilerMessage]:
    message = guard._object(value, "fixture Cargo message")
    reason = guard._text(message["reason"], "fixture Cargo message reason")
    if "fresh" in message or reason == "compiler-artifact":
        if not isinstance(message.get("fresh"), bool):
            raise RuntimeError("fixture Cargo artifact freshness must be a boolean")
    return True


def cargo_messages(stdout: str) -> list[CompilerMessage]:
    rows = []
    for line in stdout.splitlines():
        value: object = json.loads(line)
        if not _message_shape(value):
            raise AssertionError("actual Cargo message shape is required")
        rows.append(value)
    return rows


def _report_shape(value: object) -> TypeGuard[GuardReport]:
    report = guard._object(value, "fixture capacity report")
    threshold = report["threshold_bytes"]
    if threshold is not None:
        guard._number(threshold, "fixture threshold bytes")
    for field in ("current_bytes", "actual_removed_bytes"):
        guard._integer(report[field], "fixture " + field)
    post_clean = report["post_clean_bytes"]
    if post_clean is not None:
        guard._integer(post_clean, "fixture post_clean_bytes")
    guard._text(report["cleanup_status"], "fixture cleanup status")
    if "finite_cleanup" in report:
        cleanup = guard._object(report["finite_cleanup"], "fixture finite cleanup")
        guard._integer(cleanup["deleted_files"], "fixture deleted files")
        if "error" in cleanup:
            guard._text(cleanup["error"], "fixture cleanup error")
    return True


def capacity_report(value: object) -> GuardReport:
    if not _report_shape(value):
        raise AssertionError("actual capacity report shape is required")
    return value


def _state_shape(value: object) -> TypeGuard[FixtureState]:
    state = guard._object(value, "fixture capacity state")
    guard._integer(state["baseline_bytes"], "fixture baseline bytes")
    return True


def fixture_state(value: object) -> FixtureState:
    if not _state_shape(value):
        raise AssertionError("actual capacity state shape is required")
    return value


def allocated_bytes(value: object) -> int:
    if value is None:
        raise AssertionError("actual allocated-byte metric is required")
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AssertionError("actual allocated-byte metric must be a non-negative integer")
    return value


def size_bytes(target: Path) -> int:
    return allocated_bytes(guard.size_bytes(target))


def cargo_env(target: Path, build: Path | None = None) -> dict[str, str]:
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("CARGO_") and key not in {"RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}}
    env.update(CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="2")
    env["CARGO_BUILD_BUILD_DIR"] = str(build if build is not None else target)
    env.update(RUSTFLAGS="", RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="")
    return env


def real_cargo(root: Path, args: Sequence[str], env: dict[str, str]) -> str:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("real Cargo is required")
    completed = subprocess.run([cargo, *args, "--manifest-path", str(root / "Cargo.toml")],
                               cwd=root, env=env, text=True, capture_output=True, timeout=60)
    log_cargo(root, args, env, completed)
    if completed.returncode:
        raise AssertionError(completed.stderr)
    return completed.stdout


def log_cargo(root: Path, args: Sequence[str], env: dict[str, str],
              completed: subprocess.CompletedProcess[str]) -> None:
    destination = os.environ.get("CAPACITY_FIXTURE_LOG_DIR")
    if destination is None:
        return
    base = Path(destination) / str(next(CALLS))
    base.with_suffix(".stdout").write_text(completed.stdout)
    base.with_suffix(".stderr").write_text(completed.stderr)
    inputs = [root / "Cargo.lock", *root.rglob("*.toml"), *root.rglob("*.rs")]
    record = {"root": str(root), "args": args, "exit": completed.returncode,
              "env": {key: env[key] for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR", "CARGO_BUILD_JOBS") if key in env},
              "sourcePins": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}}
    base.with_suffix(".json").write_text(json.dumps(record, sort_keys=True) + "\n")


def create_workspace(root: Path) -> None:
    (root / "Cargo.toml").write_text('[workspace]\nmembers=["helper","consumer"]\nresolver="2"\n')
    for name in ("helper", "consumer"):
        member = root / name
        (member / "src").mkdir(parents=True)
        content = f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2021"\n'
        if name == "consumer":
            content += '[dependencies]\nhelper={path="../helper"}\n'
        (member / "Cargo.toml").write_text(content)
        body = 'pub fn inc(a: u64) -> u64 { a + 1 }\n'
        if name == "consumer":
            body = 'pub fn sum(a: u64) -> u64 { helper::inc(a) }\n'
        (member / "src/lib.rs").write_text(body)


def fixture(root: Path) -> tuple[Path, Path, list[Path], int, list[str], WorkspaceMetadata]:
    create_workspace(root)
    target = root / "target"
    scope = target / "active"
    real_cargo(root, ["build", "--release", "--offline"], cargo_env(scope, scope / "build"))
    baseline = size_bytes(target)
    inactive = target / "inactive/debug"
    real_cargo(root, ["build", "--offline"], cargo_env(inactive.parent))
    meta = workspace_metadata(json.loads(real_cargo(
        root, ["metadata", "--no-deps", "--locked", "--format-version", "1"], cargo_env(target))))
    prefixes = guard.workspace_incremental_prefixes(root, meta)
    active = [scope / "release", scope / "build/release"]
    return target, inactive, active, baseline, prefixes, meta


def identities(paths: Sequence[Path]) -> list[FileIdentity]:
    return [(str(path), path.stat().st_dev, path.stat().st_ino, path.stat().st_size,
             hashlib.sha256(path.read_bytes()).hexdigest()) for path in sorted(paths)]


def active_files(profiles: Sequence[Path]) -> list[Path]:
    return [path for profile in profiles for path in profile.rglob("*")
            if path.is_file() and path.name not in {".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"}]


def protect_outputs(root: Path, profiles: Sequence[Path]) -> list[Path]:
    outputs = [path for profile in profiles for path in (profile / "deps").glob("*.rlib")]
    if not outputs:
        raise AssertionError("actual Cargo libraries are required")
    consumers = root / "frozen-consumers"
    consumers.mkdir()
    paths = []
    for index in range(15):
        path = consumers / str(index)
        os.link(outputs[index % len(outputs)], path)
        paths.append(path)
    return paths


def guard_call(root: Path, target: Path, retained: Sequence[Path], refs: Sequence[Path],
               baseline: int, sentinel: Path) -> tuple[int, GuardReport, FixtureState]:
    state = root / "state.json"
    state.write_text(json.dumps({"baseline_bytes": baseline, "target": str(target), "factor": 2,
                                 "metric": guard.CAPACITY_METRIC, "metric_version": guard.CAPACITY_METRIC_VERSION}))
    env = cargo_env(target)
    env.update(CARGO_CAPACITY_GUARD_ROOT=str(root), CARGO_CAPACITY_PROTECTED_REFS=json.dumps(list(map(str, refs))))
    options = [part for ref in refs for part in ("--protected-ref", str(ref))]
    options += [part for profile in retained for part in ("--retain-profile", str(profile))]
    child = [sys.executable, "-B", "-c", "from pathlib import Path; import sys; Path(sys.argv[1]).touch()", str(sentinel)]
    completed = subprocess.run([sys.executable, "-B", guard.__file__, "--state", str(state), "--factor", "2",
                                "--print-json", *options, "--", *child], cwd=root, env=env,
                               text=True, capture_output=True, timeout=60)
    return (completed.returncode, capacity_report(json.loads(completed.stdout)),
            fixture_state(json.loads(state.read_text())))
