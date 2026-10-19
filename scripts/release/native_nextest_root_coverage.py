"""Collect KRR root coverage with authenticated Nextest retry selection."""

from __future__ import annotations

import argparse
from collections.abc import Iterator
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time
from typing import cast
import uuid

import native_coverage_local_anchor as anchor
import native_coverage_local_result as local
from native_nextest_coverage_commands import REPORT_DIRECTORY, clean_arguments, nextest_arguments, report_arguments
import native_nextest_retry_artifacts as artifacts
import native_nextest_retry_checkpoint as checkpoint

@contextmanager
def _root_lock(path: Path) -> Iterator[None]:
    hooks = str(Path(__file__).resolve().parents[1] / "hooks")
    sys.path.insert(0, hooks)
    try:
        from local_review_lock import review_lock
        from local_review_state import ReviewError
    finally:
        sys.path.remove(hooks)

    try:
        with review_lock(path):
            yield
    except ReviewError as error:
        raise ValueError(str(error)) from error

def _child_environment(repo: Path) -> dict[str, str]:
    environment = dict(os.environ)
    environment.pop("LLVM_PROFILE_FILE", None)
    environment.pop("LLVM_PROFILE_FILE_NAME", None)
    environment.update({
        "CARGO_TARGET_DIR": str(repo / "target"),
        "CARGO_LLVM_COV_TARGET_DIR": str(repo / "target/llvm-cov-target"),
        "NEXTEST_STATE_DIR": str(repo / "target/nextest-root-state"),
    })
    return environment


def _guarded(repo: Path, mode: str, cargo: list[str], arguments: list[str]) -> list[str]:
    profile = repo / "target/llvm-cov-target/krr-coverage"
    retains_profile = mode == "--coverage-collection-domain" or (
        mode == "--coverage-domain" and arguments == clean_arguments()
    )
    retain = (["--retain-profile", str(profile)]
              if retains_profile and profile.is_dir() else [])
    return [
        sys.executable, str(repo / "scripts/build/cargo_capacity_guard.py"),
        *retain, mode, "root", "--", *cargo, *arguments,
    ]


def _attempt_directory(repo: Path) -> Path:
    parent = repo / REPORT_DIRECTORY / "root-nextest-attempts"
    parent.mkdir(exist_ok=True)
    if parent.is_symlink() or parent.resolve() != parent or not parent.is_dir():
        raise ValueError("Nextest attempt parent must be a physical directory")
    directory = parent / str(uuid.uuid4())
    directory.mkdir()
    return directory


def _execute(directory: Path, name: str, argv: list[str], repo: Path,
             environment: dict[str, str]) -> int:
    logs = directory / name
    logs.mkdir()
    with (logs / "argv.json").open("x") as output:
        json.dump(argv, output)
        output.write("\n")
    started = time.monotonic()
    with (logs / "stdout.raw").open("xb") as stdout, (logs / "stderr.raw").open("xb") as stderr:
        child = subprocess.Popen(argv, cwd=repo, env=environment, stdout=stdout, stderr=stderr)
        with (logs / "pid.json").open("x") as output:
            json.dump({"pid": child.pid}, output)
        print(json.dumps({"rootCoverageStage": name, "pid": child.pid, "logs": str(logs)}), flush=True)
        code = child.wait()
    with (logs / "result.json").open("x") as output:
        json.dump({"exit": code, "seconds": time.monotonic() - started}, output)
    print(json.dumps({"rootCoverageStage": name, "exit": code, "logs": str(logs)}), flush=True)
    return code


def _new_run_id(repo: Path, before: set[str]) -> str | None:
    try:
        added = artifacts.recorded_run_ids(repo) - before
    except (OSError, ValueError):
        return None
    return next(iter(added)) if len(added) == 1 else None


def _stable_checkpoint(repo: Path, identity_sha: str, run_id: str | None, code: int) -> None:
    if run_id is None or code not in {0, 100}:
        return
    try:
        _, identity, _ = local._current_run(repo, "root")
        if identity.get("cacheable") is not True or identity.get("baseIdentitySha256") != identity_sha:
            return
        created = checkpoint.write_checkpoint(repo, identity_sha, run_id, code)
        print(json.dumps({"nextestRetryCheckpoint": created, "testExit": code}), flush=True)
    except (OSError, ValueError) as error:
        print(f"Nextest checkpoint unavailable: {error}", file=sys.stderr)


def _test(repo: Path, cargo: list[str], threads: str, identity_sha: str,
          cached: dict[str, object] | None, directory: Path,
          cacheable: bool = True) -> tuple[int, str | None]:
    environment = _child_environment(repo)
    if not cacheable:
        cached = None
    if cached is not None and cached["testExit"] == 0:
        print(json.dumps({"nextestRetry": "prior-all-passed", "runId": cached["nextestRunId"]}), flush=True)
        return 0, cast(str, cached["nextestRunId"])
    parent = cast(str, cached["nextestRunId"]) if cached is not None else None
    checkpoint.invalidate_checkpoint(repo, identity_sha)
    if parent is None:
        clean = _guarded(repo, "--coverage-domain", cargo, clean_arguments())
        code = _execute(directory, "clean", clean, repo, environment)
        if code:
            return code, None
    try:
        before = artifacts.recorded_run_ids(repo) if cacheable else set()
    except (OSError, ValueError):
        before = set()
    environment["LLVM_PROFILE_FILE_NAME"] = f"krr-nextest-retry-{directory.name}-%4m.profraw"
    command = _guarded(repo, "--coverage-collection-domain", cargo,
                       nextest_arguments(repo, threads, parent))
    code = _execute(directory, "tests", command, repo, environment)
    if parent is not None and code not in {0, 100}:
        print(json.dumps({"nextestRetryFallback": "record-unavailable", "retryExit": code}), flush=True)
        return _test(repo, cargo, threads, identity_sha, None, _attempt_directory(repo), cacheable)
    run_id = _new_run_id(repo, before) if cacheable else None
    if code:
        _stable_checkpoint(repo, identity_sha, run_id, code)
    return code, run_id


def _begin(repo: Path) -> tuple[str, bool]:
    print(json.dumps(local.local_begin(repo, "root")), flush=True)
    _, identity, _ = local._current_run(repo, "root")
    epoch = identity.get("anchorEpoch")
    identity_sha = cast(str, identity["baseIdentitySha256"])
    if epoch is not None and not anchor.begin_root_collection(repo, identity_sha, cast(str, epoch)):
        raise ValueError("root collection anchor could not be marked pending")
    return identity_sha, identity.get("cacheable") is True


def run_root(repo: Path) -> int:
    local._ensure_report_directory(repo)
    with _root_lock(repo / REPORT_DIRECTORY / "root-nextest-producer.lock"):
        identity_sha, cacheable = _begin(repo)
        cached = checkpoint.read_checkpoint(repo, identity_sha) if cacheable else None
        directory = _attempt_directory(repo)
        cargo = shlex.split(os.environ.get("CARGO", "cargo"))
        if not cargo:
            raise ValueError("CARGO must name the existing Cargo command")
        code, run_id = _test(repo, cargo, os.environ.get("TEST_THREADS", "1"),
                             identity_sha, cached, directory, cacheable)
        if code:
            return code
        report = _guarded(repo, "--coverage-collection-domain", cargo, report_arguments(repo))
        code = _execute(directory, "report", report, repo, _child_environment(repo))
        if cacheable:
            _stable_checkpoint(repo, identity_sha, run_id, 0)
        if code == 0:
            print(json.dumps(local.local_collect_finish(repo, "root", 0)), flush=True)
        return code


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo
    if not repo.is_absolute() or repo.resolve() != repo or repo != Path(__file__).resolve().parents[2]:
        parser.error("--repo must be this physical KRR repository")
    try:
        return run_root(repo)
    except (OSError, ValueError) as error:
        print(f"root Nextest coverage failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
