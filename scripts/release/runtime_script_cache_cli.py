"""Bind the fixed runtime-unused Moon leaf to independently verified local results."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Callable
from pathlib import Path

import native_coverage_local_anchor as anchor
import runtime_script_identity as identity_collector
import runtime_script_producer as producer_runner
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result
import runtime_script_result_verify as result_verify

PRODUCER = "runtime-unused-script-regression"


def bound_identity(repo: Path, producer: str = PRODUCER) -> tuple[str, str]:
    report = identity_collector.collect_identity_report(repo, producer)
    identity = report["identitySha256"]
    if (
        report["schema"] != identity_collector.SCHEMA
        or report["cacheable"] is not True
        or not isinstance(identity, str)
    ):
        raise result.RuntimeScriptResultError(
            "runtime-script cache identity is unavailable: "
            + ",".join(report["missReasons"])
        )
    epoch = anchor.prepare(repo, producer, identity)
    if epoch is None:
        raise result.RuntimeScriptResultError("private runtime-script anchor is unavailable")
    return identity, epoch


def current_key(repo: Path, producer: str = PRODUCER) -> dict[str, str]:
    identity, epoch = bound_identity(repo, producer)
    # 成功出力の破損時だけ世代が変わり、Moon自身の次回実行・保存を必須にする。
    fingerprint = hashlib.sha256(
        f"krr-runtime-script-key-v1\0{identity}\0{epoch}".encode()
    ).hexdigest()
    return {
        "schema": "krr-runtime-script-key-v1",
        "producer": producer,
        "fingerprint": fingerprint,
    }


def validate_current(repo: Path, producer: str = PRODUCER) -> dict[str, str | bool]:
    report = identity_collector.collect_identity_report(repo, producer)
    return _validate_report(repo, producer, report)


def _validate_report(
    repo: Path, producer: str, report: object
) -> dict[str, str | bool]:
    if (
        not isinstance(report, dict)
        or report.get("schema") != identity_collector.SCHEMA
        or report.get("producer") != producer
        or report.get("cacheable") is not True
        or not isinstance(report.get("identitySha256"), str)
        or not isinstance(report.get("sourceManifestSha256"), str)
        or identity_collector.SHA256_RE.fullmatch(report["sourceManifestSha256"]) is None
        or not isinstance(report.get("sourceInputsSha256"), str)
        or identity_collector.SHA256_RE.fullmatch(report["sourceInputsSha256"]) is None
    ):
        raise result.RuntimeScriptResultError("fixed producer identity is unavailable")
    identity = report["identitySha256"]
    if not isinstance(identity, str):
        raise result.RuntimeScriptResultError("fixed producer identity is unavailable")
    epoch = anchor.prepare(repo, producer, identity)
    if epoch is None:
        raise result.RuntimeScriptResultError("private runtime-script anchor is unavailable")
    receipt_path = repo / result.RECEIPT_DIRECTORY / f"{producer}.json"
    try:
        result_verify.verify_receipt(repo, producer, identity, epoch, receipt_path)
    except (result.RuntimeScriptResultError, OSError, ValueError):
        anchor.invalidate(repo, producer, identity, epoch)
        raise
    return {
        "schema": "krr-runtime-script-validation-v1",
        "producer": producer,
        "valid": True,
    }


def validate_all(repo: Path) -> dict[str, object]:
    reports = identity_collector.collect_fixed_leaf_identity_reports(repo)
    expected = set(profiles.RUNTIME_SCRIPT_PROFILES)
    if set(reports) != expected:
        raise result.RuntimeScriptResultError("fixed producer identity report set is invalid")
    validated = [
        _validate_report(repo, producer, reports[producer])
        for producer in sorted(expected)
    ]
    return {
        "schema": "krr-runtime-script-validation-all-v1",
        "producers": validated,
        "valid": True,
    }


def ensure_all(repo: Path) -> dict[str, object]:
    reports = identity_collector.collect_fixed_leaf_identity_reports(repo)
    expected = set(profiles.RUNTIME_SCRIPT_PROFILES)
    if set(reports) != expected:
        raise result.RuntimeScriptResultError("fixed producer identity report set is invalid")
    invalid: list[str] = []
    validated: list[dict[str, str | bool]] = []
    for producer in sorted(expected):
        try:
            validated.append(_validate_report(repo, producer, reports[producer]))
        except result.RuntimeScriptResultError:
            invalid.append(producer)
    for producer in invalid:
        run_current(repo, producer)
    report = validate_all(repo) if invalid else {
        "schema": "krr-runtime-script-validation-all-v1",
        "producers": validated,
        "valid": True,
    }
    report["recovered"] = bool(invalid)
    return report


def run_current(repo: Path, producer: str = PRODUCER) -> dict[str, object]:
    before = identity_collector.collect_execution_snapshot(repo, producer)
    input_digest = before["inputSha256"]
    if (
        before["schema"] != identity_collector.SCHEMA
        or before["cacheable"] is not True
        or before.get("producer") != producer
        or not isinstance(input_digest, str)
    ):
        raise result.RuntimeScriptResultError("current producer inputs are unavailable")
    recipe = recipe_reader.read_runtime_script_recipe(repo, producer)
    if recipe.recipe_sha256 != before["recipeSha256"]:
        raise result.RuntimeScriptResultError("producer recipe changed before execution")
    run = producer_runner.run_runtime_script(repo, producer, input_digest, recipe)
    after = identity_collector.collect_execution_snapshot(repo, producer)
    identity_fields = (
        "producer",
        "sourceManifestSha256",
        "sourceInputsSha256",
        "recipeSha256",
        "toolEnvironmentSha256",
        "inputSha256",
        "identitySha256",
    )
    if (
        after["schema"] != identity_collector.SCHEMA
        or after["cacheable"] is not True
        or any(before.get(field) != after.get(field) for field in identity_fields)
        or after.get("identitySha256") != run.identity_sha256
    ):
        anchor.invalidate(repo, producer, run.identity_sha256, run.epoch)
        raise result.RuntimeScriptResultError("producer inputs changed during execution")
    try:
        result_verify.verify_receipt(
            repo, producer, run.identity_sha256, run.epoch, repo / run.receipt_path
        )
    except (result.RuntimeScriptResultError, OSError, ValueError):
        anchor.invalidate(repo, producer, run.identity_sha256, run.epoch)
        raise
    return {
        "schema": "krr-runtime-script-execution-v1",
        "producer": producer,
        "valid": True,
        "executedGroups": len(run.reports),
        "rawLogDirectory": run.raw_log_directory,
        "receiptPath": run.receipt_path,
    }


def ensure_current(repo: Path, producer: str = PRODUCER) -> dict[str, object]:
    try:
        report: dict[str, object] = dict(validate_current(repo, producer))
    except result.RuntimeScriptResultError:
        report = run_current(repo, producer)
        report["recovered"] = True
        return report
    report["recovered"] = False
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "operation", choices=("key", "validate", "validate-all", "ensure-all", "run", "ensure")
    )
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--producer", default=PRODUCER, choices=tuple(sorted(profiles.accepted_producers())))
    args = parser.parse_args(argv)
    try:
        operations: dict[str, Callable[[Path], object]] = {
            "key": lambda repo: current_key(repo, args.producer),
            "validate": lambda repo: validate_current(repo, args.producer),
            "run": lambda repo: run_current(repo, args.producer),
            "ensure": lambda repo: ensure_current(repo, args.producer),
        }
        report: object
        if args.operation == "validate-all":
            report = validate_all(args.repo)
        elif args.operation == "ensure-all":
            report = ensure_all(args.repo)
        else:
            report = operations[args.operation](args.repo)
    except (result.RuntimeScriptResultError, producer_runner.RuntimeScriptProducerError, OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
