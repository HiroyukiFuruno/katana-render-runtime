"""Bind the fixed runtime-unused Moon leaf to independently verified local results."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import sys
from collections.abc import Callable, Mapping
from contextlib import ExitStack
from pathlib import Path

import native_coverage_local_anchor as anchor
import runtime_script_identity as identity_collector
import runtime_script_junit_outputs as junit_outputs
import runtime_script_producer as producer_runner
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result
import runtime_script_result_verify as result_verify
import runtime_script_tool_fingerprint_inputs as pilot

PRODUCER = "runtime-unused-script-regression"
PILOT_COMPONENTS = frozenset(("sources", "packages", "resolver", "bun", "configurationAbsence",
                              "adapter", "semanticEnvironment"))


def _pilot_diagnostic(snapshot: Mapping[str, object]) -> dict[str, object]:
    diagnostic = snapshot.get("diagnostic")
    if not isinstance(diagnostic, dict) or set(diagnostic) != {
        "inputComponents", "environmentNames", "runtimeTool", "pythonContext"
    }:
        raise result.RuntimeScriptResultError("pilot diagnostic shape is invalid")
    components = diagnostic.get("inputComponents")
    names = diagnostic.get("environmentNames")
    if (not isinstance(components, dict) or set(components) != PILOT_COMPONENTS
            or any(not isinstance(value, str) or not identity_collector.SHA256_RE.fullmatch(value)
                   for value in components.values())
            or not isinstance(names, list)
            or any(not isinstance(name, str) or not name or not name.isprintable() or "=" in name
                   for name in names)
            or any(not isinstance(diagnostic.get(key), str)
                   or not identity_collector.SHA256_RE.fullmatch(str(diagnostic[key]))
                   for key in ("runtimeTool", "pythonContext"))):
        raise result.RuntimeScriptResultError("pilot diagnostic fields are invalid")
    return diagnostic


def _pilot_identity_digests(snapshot: Mapping[str, object]) -> dict[str, str | None]:
    fields = (
        "sourceManifestSha256", "sourceInputsSha256", "recipeSha256",
        "toolEnvironmentSha256", "inputSha256", "identitySha256",
    )
    digests: dict[str, str | None] = {}
    for field in fields:
        value = snapshot.get(field)
        digests[field] = (
            value if isinstance(value, str) and identity_collector.SHA256_RE.fullmatch(value)
            else None
        )
    return digests


def _pilot_reuse_attempt(repo: Path, snapshot: Mapping[str, object]) -> dict[str, object]:
    directory = repo / result.RECEIPT_DIRECTORY
    digests = _pilot_identity_digests(snapshot)
    return {
        "receiptExists": (directory / f"{pilot.PROFILE}.json").exists(),
        "currentJunitExists": (directory / f"{pilot.PROFILE}-current-group-001.xml").exists(),
        "receiptIdentityMatches": _pilot_receipt_identity_matches(repo, digests["identitySha256"]),
        "cacheable": snapshot.get("cacheable") is True,
        "identity": digests,
    }


def _pilot_receipt_identity_matches(repo: Path, identity: str | None) -> bool | None:
    if identity is None:
        return None
    relative = f"{result.RECEIPT_DIRECTORY}/{pilot.PROFILE}.json"
    try:
        receipt = json.loads(junit_outputs.read_output(repo, relative, pilot.PROFILE, ".json"))
    except (OSError, ValueError):
        return None
    if (
        not isinstance(receipt, dict)
        or set(receipt) != {"schema", "producer", "identitySha256", "epoch", "junitReports"}
        or receipt.get("schema") != result.SCHEMA
        or receipt.get("producer") != pilot.PROFILE
        or not isinstance(receipt.get("junitReports"), list)
        or any(not isinstance(receipt.get(key), str)
               or not identity_collector.SHA256_RE.fullmatch(receipt[key])
               for key in ("identitySha256", "epoch"))
    ):
        return None
    return bool(receipt["identitySha256"] == identity)


def _bound_identity_report(
    repo: Path, producer: str
) -> tuple[str, str, identity_collector.IdentityReport]:
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
    return identity, epoch, report


def bound_identity(repo: Path, producer: str = PRODUCER) -> tuple[str, str]:
    identity, epoch, _report = _bound_identity_report(repo, producer)
    return identity, epoch


def current_key(repo: Path, producer: str = PRODUCER) -> dict[str, str]:
    diagnostic_flag = os.environ.get("KRR_RUNTIME_SCRIPT_KEY_DIAGNOSTICS")
    if producer == pilot.PROFILE and diagnostic_flag not in (None, "1"):
        raise result.RuntimeScriptResultError("pilot key diagnostic flag is invalid")
    identity, epoch, report = _bound_identity_report(repo, producer)
    if producer == pilot.PROFILE and diagnostic_flag == "1":
        projection = {
            "schema": "krr-runtime-script-pilot-key-diagnostic-v1",
            "identity": _pilot_identity_digests(report),
            "diagnostic": _pilot_diagnostic(report),
        }
        directory = repo / result.RECEIPT_DIRECTORY / "key-diagnostics"
        producer_runner._ensure_directory(directory)
        path = directory / f"key-{os.getpid()}-{secrets.token_hex(12)}.json"
        producer_runner._write_exclusive(
            path, json.dumps(projection, sort_keys=True, separators=(",", ":")).encode()
        )
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
        or report.get("sourceInputScope") != (
            pilot.SEMANTIC_SCOPE if producer == pilot.PROFILE
            else "krr-native-coverage-source-inputs-v1"
        )
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


def _stable_batch_validation(repo: Path) -> tuple[list[str], list[dict[str, str | bool]]]:
    expected = set(profiles.RUNTIME_SCRIPT_PROFILES)
    with ExitStack() as leases:
        for producer in sorted(expected):
            lease, device, inode = producer_runner._lease(repo, producer)
            leases.callback(producer_runner._release_lease, lease, device, inode)
        before = identity_collector.collect_fixed_leaf_identity_reports(repo)
        if set(before) != expected:
            raise result.RuntimeScriptResultError("fixed producer identity report set is invalid")
        invalid: list[str] = []
        validated: list[dict[str, str | bool]] = []
        for producer in sorted(expected):
            try:
                validated.append(_validate_report(repo, producer, before[producer]))
            except result.RuntimeScriptResultError:
                invalid.append(producer)
        if invalid:
            return invalid, validated
        after = identity_collector.collect_fixed_leaf_identity_reports(repo)
        if before != after:
            raise result.RuntimeScriptResultError("batch reuse inputs changed during validation")
        for producer in sorted(expected):
            try:
                _validate_report(repo, producer, after[producer])
            except result.RuntimeScriptResultError:
                invalid.append(producer)
        return invalid, validated


def ensure_all(repo: Path) -> dict[str, object]:
    invalid, validated = _stable_batch_validation(repo)
    for producer in invalid:
        run_current(repo, producer)
    if invalid:
        remaining, validated = _stable_batch_validation(repo)
        if remaining:
            raise result.RuntimeScriptResultError("recovered batch validation failed")
    report: dict[str, object] = {
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
        "sourceInputScope",
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
    report: dict[str, object] = {
        "schema": "krr-runtime-script-execution-v1",
        "producer": producer,
        "valid": True,
        "executedGroups": len(run.reports),
        "rawLogDirectory": run.raw_log_directory,
        "receiptPath": run.receipt_path,
    }
    if producer == pilot.PROFILE:
        report["diagnostic"] = _pilot_diagnostic(after)
    return report


def ensure_current(repo: Path, producer: str = PRODUCER) -> dict[str, object]:
    reuse_attempt: dict[str, object] | None = None
    try:
        lease, device, inode = producer_runner._lease(repo, producer)
        try:
            before = identity_collector.collect_identity_report(repo, producer)
            if producer == pilot.PROFILE:
                reuse_attempt = _pilot_reuse_attempt(repo, before)
            report: dict[str, object] = dict(_validate_report(repo, producer, before))
            after = identity_collector.collect_identity_report(repo, producer)
            if before != after:
                raise result.RuntimeScriptResultError("reuse inputs changed during validation")
            _validate_report(repo, producer, after)
            report["executedGroups"] = 0
            if producer == pilot.PROFILE:
                report["diagnostic"] = _pilot_diagnostic(after)
        finally:
            producer_runner._release_lease(lease, device, inode)
    except result.RuntimeScriptResultError:
        report = run_current(repo, producer)
        report["recovered"] = True
        if reuse_attempt is not None:
            report["reuseAttempt"] = reuse_attempt
        return report
    report["recovered"] = False
    if reuse_attempt is not None:
        report["reuseAttempt"] = reuse_attempt
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
