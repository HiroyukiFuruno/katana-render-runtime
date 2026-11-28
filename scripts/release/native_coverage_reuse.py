#!/usr/bin/env python3
"""Verify KRR coverage cache manifests using GitHub CLI attestations."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import cast

import native_coverage_result as coverage_result
import native_coverage_result_cli as coverage_cli


RUN_ID_RE = re.compile(r"[0-9]+\Z")
REUSE_STATUS_SCHEMA = "krr-native-test-coverage-seed-v1"
MAX_MANIFEST_BYTES = 4 * 1024 * 1024


class CoverageReuseError(ValueError):
    """Raised when a signed coverage result cannot be reused safely."""


def _read_json(path: Path, label: str) -> object:
    value = coverage_cli._read_json(path, label)
    return value


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _snapshot_manifest(path: Path) -> bytes:
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        initial = path.lstat()
        if not stat.S_ISREG(initial.st_mode) or initial.st_size > MAX_MANIFEST_BYTES:
            raise CoverageReuseError("coverage manifest must be a bounded regular file")
        descriptor = os.open(path, flags)
        with os.fdopen(descriptor, "rb") as source:
            before = os.fstat(source.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_MANIFEST_BYTES:
                raise CoverageReuseError("coverage manifest must be a bounded regular file")
            data = source.read(MAX_MANIFEST_BYTES + 1)
            after = os.fstat(source.fileno())
        if len(data) > MAX_MANIFEST_BYTES:
            raise CoverageReuseError("coverage manifest exceeds the size limit")
        def identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
            return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                    value.st_mtime_ns, value.st_ctime_ns)

        if identity(before) != identity(after) or len(data) != after.st_size:
            raise CoverageReuseError("coverage manifest changed while being read")
        return data
    except OSError as error:
        raise CoverageReuseError("coverage manifest bytes are unavailable") from error


def _write_immutable(path: Path, data: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o400)
    with os.fdopen(descriptor, "wb") as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())
        os.fchmod(output.fileno(), 0o400)


def _immutable_copy(directory: Path, data: bytes) -> Path:
    path = directory / "manifest.json"
    _write_immutable(path, data)
    return path


def _assert_copy_unchanged(path: Path, expected: bytes) -> None:
    mode = path.lstat().st_mode
    if not stat.S_ISREG(mode) or stat.S_IMODE(mode) != 0o400:
        raise CoverageReuseError("private immutable manifest permissions changed")
    if _snapshot_manifest(path) != expected:
        raise CoverageReuseError("private immutable manifest changed during verification")


def _install_seed_copy(repo: Path, requested: Path, data: bytes) -> Path:
    output = requested if requested.is_absolute() else repo / requested
    try:
        relative = output.relative_to(repo)
    except ValueError as error:
        raise CoverageReuseError("seed cache output must be under repository tmp") from error
    if not relative.parts or relative.parts[0] != "tmp" or ".." in relative.parts:
        raise CoverageReuseError("seed cache output must be under repository tmp")
    cursor = repo
    for part in relative.parts[:-1]:
        cursor /= part
        if not cursor.exists():
            cursor.mkdir()
        mode = cursor.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
            raise CoverageReuseError("seed cache output parent must be a real directory")
    try:
        _write_immutable(output, data)
    except OSError as error:
        raise CoverageReuseError("fresh immutable seed copy could not be written") from error
    return output


def _context(path: Path, tested_sha: str, workflow_sha: str) -> coverage_result.JsonObject:
    context = coverage_result.parse_coverage_context(_read_json(path, "coverage context"))
    if context["testedSha"] != tested_sha or context["workflowSha"] != workflow_sha:
        raise CoverageReuseError("live context differs from expected source identity")
    return context


def _run_result_cli(repo: Path, argv: list[str]) -> dict[str, object]:
    command = [sys.executable, str(Path(__file__).with_name("native_coverage_result_cli.py")),
               "--repo", str(repo), *argv]
    result = subprocess.run(command, cwd=repo, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        raise CoverageReuseError("coverage manifest validation failed")
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise CoverageReuseError("coverage manifest validator returned invalid output") from error
    if not isinstance(value, dict):
        raise CoverageReuseError("coverage manifest validator returned invalid output")
    return cast(dict[str, object], value)


def _verify_manifest(repo: Path, context_path: Path, manifest_path: Path,
                     tested_sha: str, workflow_sha: str) -> tuple[bytes, str]:
    _context(context_path, tested_sha, workflow_sha)
    data = _snapshot_manifest(manifest_path)
    digest = _sha256(data)
    with tempfile.TemporaryDirectory(prefix="krr-coverage-local-") as temporary:
        immutable_manifest = _immutable_copy(Path(temporary), data)
        result = _run_result_cli(repo, ["validate", "--context", str(context_path),
                                        "--manifest", str(immutable_manifest)])
        if result != {"authentication": "not-performed", "schema": coverage_result.SCHEMA,
                      "valid": True}:
            raise CoverageReuseError("coverage manifest validator did not confirm the manifest")
        _assert_copy_unchanged(immutable_manifest, data)
    if _snapshot_manifest(manifest_path) != data:
        raise CoverageReuseError("coverage manifest bytes changed during validation")
    return data, digest


def _attestation_command(manifest: Path, repository: str, tested_sha: str,
                         workflow_sha: str, run_ref: str) -> list[str]:
    return ["gh", "attestation", "verify", str(manifest), "--repo", repository,
            "--signer-workflow", f"{repository}/.github/workflows/test-and-build.yml",
            "--signer-digest", workflow_sha, "--source-digest", tested_sha,
            "--source-ref", run_ref, "--deny-self-hosted-runners", "--format", "json"]


def _parse_attestation_output(stdout: str) -> list[dict[str, object]]:
    try:
        verified = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise CoverageReuseError("GitHub CLI returned invalid verification output") from error
    if not isinstance(verified, list) or not verified or not all(
            _is_verified_attestation_row(row) for row in verified):
        raise CoverageReuseError("GitHub CLI returned no verified attestations")
    return cast(list[dict[str, object]], verified)


def _is_verified_attestation_row(value: object) -> bool:
    if not isinstance(value, dict):
        return False
    attestation = value.get("attestation")
    result = value.get("verificationResult")
    if not isinstance(attestation, dict) or not attestation or not isinstance(result, dict):
        return False
    signature = result.get("signature")
    timestamps = result.get("verifiedTimestamps")
    statement = result.get("statement")
    if not isinstance(signature, dict) or not isinstance(signature.get("certificate"), dict):
        return False
    if (not isinstance(timestamps, list) or not all(isinstance(item, dict) for item in timestamps)
            or not isinstance(statement, dict)):
        return False
    subjects = statement.get("subject")
    predicate_type = statement.get("predicateType")
    return (isinstance(subjects, list) and bool(subjects)
            and all(isinstance(subject, dict) for subject in subjects)
            and isinstance(predicate_type, str) and bool(predicate_type)
            and isinstance(statement.get("predicate"), dict))


def verify_cache_manifest(repo: Path, manifest: Path, context_path: Path, repository: str,
                          tested_sha: str, workflow_sha: str, run_ref: str) -> dict[str, object]:
    if run_ref != "refs/heads/master":
        raise CoverageReuseError("coverage reuse is limited to the protected master ref")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise CoverageReuseError("repository identity is invalid")
    data, manifest_sha = _verify_manifest(repo, context_path, manifest, tested_sha, workflow_sha)
    with tempfile.TemporaryDirectory(prefix="krr-coverage-verify-") as temporary:
        immutable_manifest = _immutable_copy(Path(temporary), data)
        _run_result_cli(repo, ["validate", "--context", str(context_path),
                               "--manifest", str(immutable_manifest)])
        _assert_copy_unchanged(immutable_manifest, data)
        command = _attestation_command(immutable_manifest, repository, tested_sha,
                                       workflow_sha, run_ref)
        try:
            result = subprocess.run(command, cwd=repo, text=True, capture_output=True,
                                    check=False, timeout=120)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise CoverageReuseError("GitHub attestation verification is unavailable") from error
        if result.returncode != 0:
            raise CoverageReuseError("GitHub attestation verification failed")
        _parse_attestation_output(result.stdout)
        _assert_copy_unchanged(immutable_manifest, data)
    return {"schema": coverage_result.SCHEMA, "verified": True, "manifestSha256": manifest_sha}


def verify_fresh_seed(repo: Path, manifest: Path, context_path: Path, cache_key_path: Path,
                      status_path: Path, tested_sha: str, workflow_sha: str,
                      run_id: str, run_attempt: str,
                      cache_output: Path | None = None) -> dict[str, object]:
    status = _read_json(status_path, "coverage seed status")
    if not isinstance(status, dict) or set(status) != {"schema", "fresh", "runId", "runAttempt"}:
        raise CoverageReuseError("coverage seed status fields are invalid")
    if (status.get("schema") != REUSE_STATUS_SCHEMA or status.get("fresh") is not True
            or status.get("runId") != run_id or status.get("runAttempt") != run_attempt
            or RUN_ID_RE.fullmatch(run_id) is None or RUN_ID_RE.fullmatch(run_attempt) is None):
        raise CoverageReuseError("coverage seed was not created by this fresh workflow run")
    data, manifest_sha = _verify_manifest(repo, context_path, manifest, tested_sha, workflow_sha)
    key_result = _run_result_cli(repo, ["cache-key", "--context", str(context_path)])
    cache_key = key_result.get("key")
    try:
        recorded_key = cache_key_path.read_text(encoding="ascii").strip()
    except OSError as error:
        raise CoverageReuseError("coverage cache key is unavailable") from error
    if not isinstance(cache_key, str) or recorded_key != cache_key:
        raise CoverageReuseError("coverage cache key differs from current source and context")
    if cache_output is not None:
        _install_seed_copy(repo, cache_output, data)
    return {"schema": coverage_result.SCHEMA, "eligible": True, "key": cache_key,
            "manifestSha256": manifest_sha}


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="native_coverage_reuse.py")
    subparsers = parser.add_subparsers(dest="command", required=True)
    cache = subparsers.add_parser("verify-cache")
    cache.add_argument("--repo", required=True, type=Path)
    cache.add_argument("--manifest", required=True, type=Path)
    cache.add_argument("--context", required=True, type=Path)
    cache.add_argument("--repository", required=True)
    cache.add_argument("--tested-sha", required=True)
    cache.add_argument("--workflow-sha", required=True)
    cache.add_argument("--source-ref", required=True)
    seed = subparsers.add_parser("verify-seed")
    seed.add_argument("--repo", required=True, type=Path)
    seed.add_argument("--manifest", required=True, type=Path)
    seed.add_argument("--context", required=True, type=Path)
    seed.add_argument("--cache-key", required=True, type=Path)
    seed.add_argument("--status", required=True, type=Path)
    seed.add_argument("--tested-sha", required=True)
    seed.add_argument("--workflow-sha", required=True)
    seed.add_argument("--run-id", required=True)
    seed.add_argument("--run-attempt", required=True)
    seed.add_argument("--cache-output", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        repo = args.repo.resolve(strict=True)
        if args.command == "verify-cache":
            result = verify_cache_manifest(repo, args.manifest, args.context, args.repository,
                                           args.tested_sha, args.workflow_sha, args.source_ref)
        else:
            result = verify_fresh_seed(repo, args.manifest, args.context, args.cache_key,
                                      args.status, args.tested_sha, args.workflow_sha,
                                      args.run_id, args.run_attempt, args.cache_output)
    except (OSError, CoverageReuseError, coverage_result.CoverageResultError) as error:
        print(f"coverage reuse: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
