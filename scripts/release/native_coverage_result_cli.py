#!/usr/bin/env python3
"""Expose the bounded KRR coverage manifest operations to existing CI tooling."""

from __future__ import annotations

import argparse
import json
import os
import stat
import sys
from pathlib import Path
from typing import cast

import native_coverage_linux_context as linux_context
import native_coverage_local_result as local_result
import native_coverage_result as coverage_result


def _read_json(path: Path, label: str) -> object:
    try:
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
            raise coverage_result.CoverageResultError(f"{label} must be a regular non-symlink file")
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise coverage_result.CoverageResultError(f"{label} is unavailable or invalid JSON") from error


def _fresh_output(root: Path, requested: Path) -> Path:
    output = requested if requested.is_absolute() else root / requested
    try:
        relative = output.relative_to(root)
    except ValueError as error:
        raise coverage_result.CoverageResultError("new manifest must be written under repository tmp") from error
    if not relative.parts or relative.parts[0] != "tmp" or ".." in relative.parts:
        raise coverage_result.CoverageResultError("new manifest must be written under repository tmp")
    cursor = root
    for part in relative.parts[:-1]:
        cursor /= part
        try:
            mode = cursor.lstat().st_mode
        except OSError as error:
            raise coverage_result.CoverageResultError("manifest output parent must already exist") from error
        if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
            raise coverage_result.CoverageResultError("manifest output parent must be a real directory")
    if output.exists() or output.is_symlink():
        raise coverage_result.CoverageResultError("manifest output already exists")
    return output


def _write_fresh(path: Path, value: object) -> None:
    encoded = (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(encoded)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="native_coverage_result_cli.py")
    parser.add_argument("--repo", required=True, type=Path)
    commands = parser.add_subparsers(dest="command", required=True)
    key = commands.add_parser("cache-key")
    key.add_argument("--context", required=True, type=Path)
    create = commands.add_parser("create")
    create.add_argument("--context", required=True, type=Path)
    create.add_argument("--output", required=True, type=Path)
    validate = commands.add_parser("validate")
    validate.add_argument("--context", required=True, type=Path)
    validate.add_argument("--manifest", required=True, type=Path)
    commands.add_parser("collect-context")
    for name in ("local-key", "local-begin", "local-validate", "local-collection-validate"):
        local = commands.add_parser(name)
        local.add_argument("--producer", required=True, choices=("root", "performance"))
        if name == "local-key":
            local.add_argument("--moon-fingerprint", action="store_true", required=True)
    finish = commands.add_parser("local-finish")
    finish.add_argument("--producer", required=True, choices=("root", "performance"))
    finish.add_argument("--producer-exit", required=True, type=int)
    collection_finish = commands.add_parser("local-collect-finish")
    collection_finish.add_argument("--producer", required=True, choices=("root", "performance"))
    collection_finish.add_argument("--producer-exit", required=True, type=int)
    return parser


def _context(path: Path) -> coverage_result.JsonObject:
    return coverage_result.parse_coverage_context(_read_json(path, "coverage context"))


def _run(args: argparse.Namespace) -> dict[str, object]:
    repo = args.repo.resolve(strict=True)
    if args.command == "local-key":
        return local_result.local_key(repo, args.producer)
    if args.command == "local-begin":
        return local_result.local_begin(repo, args.producer)
    if args.command == "local-finish":
        return local_result.local_finish(repo, args.producer, args.producer_exit)
    if args.command == "local-collect-finish":
        return local_result.local_collect_finish(repo, args.producer, args.producer_exit)
    if args.command == "local-validate":
        return local_result.local_validate(repo, args.producer)
    if args.command == "local-collection-validate":
        return local_result.local_collection_validate(repo, args.producer)
    if args.command == "collect-context":
        try:
            return linux_context.collect_context(repo)
        except OSError as error:
            raise linux_context.LinuxCoverageContextError("Linux host identity could not be read") from error
    context = _context(args.context)
    if args.command == "cache-key":
        return coverage_result.coverage_cache_key(repo, context)
    tested_sha = cast(str, context["testedSha"])
    workflow_sha = cast(str, context["workflowSha"])
    profile = cast(dict[str, str], context["profile"])
    tools = cast(dict[str, str], context["tools"])
    if args.command == "create":
        manifest = coverage_result.create_coverage_result_manifest(
            repo, tested_sha, workflow_sha, profile, tools,
        )
        output = _fresh_output(repo, args.output)
        try:
            _write_fresh(output, manifest)
        except OSError as error:
            raise coverage_result.CoverageResultError("fresh manifest output could not be written") from error
        return {"schema": coverage_result.SCHEMA, "created": True}
    received_manifest = _read_json(args.manifest, "coverage result manifest")
    coverage_result.verify_coverage_result_manifest(
        received_manifest, repo, tested_sha, workflow_sha, profile, tools,
    )
    return {"schema": coverage_result.SCHEMA, "valid": True,
            "authentication": "not-performed"}


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = _run(args)
    except coverage_result.CoveragePolicyError as error:
        print(json.dumps({"valid": False, "failure": "coverage-policy"}, sort_keys=True))
        print(f"coverage result: {error}", file=sys.stderr)
        return 1
    except (OSError, coverage_result.CoverageResultError, linux_context.LinuxCoverageContextError,
            local_result.LocalCoverageError) as error:
        print(json.dumps({"valid": False, "failure": "invalid-collection"}, sort_keys=True))
        print(f"coverage result: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
