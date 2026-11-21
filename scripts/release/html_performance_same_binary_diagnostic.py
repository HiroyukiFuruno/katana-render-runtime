"""同一 candidate binary の paired 測定。正式性能証跡としては受理しない。"""
from __future__ import annotations

import os
import statistics
import time
from pathlib import Path
from typing import cast

import html_performance_paired_stats as paired_stats
import html_performance_remote_evidence as remote
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate


def environment_comparison(previous: list[gate.JsonObject],
                           actual: gate.JsonObject) -> gate.JsonObject:
    gate.require(len(previous) == 50, "original 50 worker observations required")
    old = previous[0]
    for row in previous:
        gate.require(row.get("schema") == "krr-html-worker-runtime-execution-v1"
                     and row.get("nonMetadataEnvironmentSha256") == old.get("nonMetadataEnvironmentSha256")
                     and row.get("keyNamesSha256") == old.get("keyNamesSha256")
                     and row.get("keyNames") == old.get("keyNames"),
                     "original worker environments differ")
    old_keys = {gate._text(value, "worker key")
                for value in gate._array(old.get("keyNames"), "worker keys")}
    new_keys = {gate._text(value, "worker key")
                for value in gate._array(actual.get("keyNames"), "worker keys")}
    return {"originalEnvironmentValuesAvailable": False,
            "environmentKeyNamesAdded": sorted(new_keys - old_keys),
            "environmentKeyNamesRemoved": sorted(old_keys - new_keys),
            "originalNonMetadataEnvironmentSha256": old["nonMetadataEnvironmentSha256"],
            "diagnosticNonMetadataEnvironmentSha256": actual["nonMetadataEnvironmentSha256"],
            "originalKeyNamesSha256": old["keyNamesSha256"],
            "diagnosticKeyNamesSha256": actual["keyNamesSha256"]}


def run(root: Path, output: Path, batch: Path, batch_sha256: str,
        origin: producer.FixedEpochContextInput, condition_id: str) -> None:
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    gate.require(output.is_relative_to(root / "tmp") and not output.exists(),
                 "fresh diagnostic output required")
    gate.digest(batch_sha256)
    gate.require(gate.sha256(batch / "batch-receipt.json") == batch_sha256,
                 "batch receipt SHA differs")
    receipt = gate.read_json(batch / "batch-receipt.json")
    gate.require(receipt.get("schema") in (producer.BATCH_RECEIPT_SCHEMA,
                                          producer.BATCH_RECEIPT_SCHEMA_V2)
                 and receipt.get("status") == "closed", "closed batch required")
    bundle_pin = gate._object(receipt.get("bundle"), "batch bundle")
    bundle_path = batch / "bundle" / "bundle.json"
    gate.require(bundle_pin.get("path") == str(bundle_path)
                 and bundle_pin.get("sha256") == gate.sha256(bundle_path), "batch bundle differs")
    bundle = gate.Bundle(bundle_path.parent)
    attempts = gate._objects(receipt.get("attempts"), "batch attempts")
    gate.require(attempts == bundle.manifest.get("attempts"), "batch attempts differ")
    matches = [a for a in attempts if producer._attempt_condition_id(bundle, a) == condition_id]
    complete_ids = gate._array(receipt.get("completeConditionIds"), "complete conditions")
    gate.require(len(matches) == 1 and condition_id not in complete_ids,
                 "one incomplete diagnostic condition required")
    attempt = matches[0]
    producer._require_cpu_busy_resume(bundle, [cast(dict[str, producer.FilePin], attempt)])
    expected = cast(producer.ExpectedPerformance, bundle.json(attempt["expected"]))
    result = bundle.json(attempt["result"])
    policy = gate._object(result.get("policy"), "original policy")
    gate.require(result.get("comparisonStatus") == "paired" and policy.get("policyPass") is False,
                 "failed paired condition required")
    fixed = remote.validate_fixed_epoch_candidate_origin(
        origin["directory"], root, origin["manifest_sha256"], origin["artifact_path"],
        origin["artifact_sha256"], origin["head"])
    original = gate.Bundle(origin["directory"])
    artifact = original.relative_reference(origin["artifact_path"], origin["artifact_sha256"])
    proof = original.json(artifact)
    binary_pin = gate._object(proof.get("frozenBinary"), "fixed worker")
    gate.require(binary_pin == origin["workers"].get("candidate"), "fixed worker pin differs")
    binary = Path(gate._text(binary_pin.get("path"), "worker path"))
    producer._pinned_file(binary, fixed.binary_sha256)
    candidate = expected["arms"]["candidate"]
    gate.require(candidate["binary_sha256"] == fixed.binary_sha256
                 and candidate["source_snapshot_sha256"] == fixed.snapshot_sha256
                 and candidate["lock_sha256"] == fixed.lock_sha256
                 and candidate["build_artifact_ref"] == artifact,
                 "original candidate identity differs")
    snapshot = cast(producer.FilePin, gate._object(
        gate.fixed_source_pair(cast(gate.JsonObject, expected))["snapshot"], "snapshot"))
    bundle.file(snapshot)
    producer._pinned_file(Path(snapshot["path"]), snapshot["sha256"])
    plan = cast(producer.FilePin, gate._object(expected["operationPlanRef"], "operation plan"))
    bundle.file(plan)
    gate.require(plan["sha256"] == expected["operation_plan_sha256"], "plan identity differs")
    producer._pinned_file(Path(plan["path"]), plan["sha256"])
    catalog = gate._object(expected.get("catalogCondition"), "catalog condition")
    fixture = Path(gate._text(catalog.get("fixture_path"), "fixture path"))
    producer._pinned_file(fixture, expected["fixture_sha256"])
    lock = root / gate.HARNESS_ROOT / "Cargo.lock"
    producer._pinned_file(lock, fixed.lock_sha256)
    manifest = bundle.json(attempt["pairManifest"])
    previous: list[gate.JsonObject] = []
    for pair in gate._objects(manifest.get("order"), "pair order"):
        for worker_record in gate._objects(pair.get("workerAttempts"), "worker attempts"):
            record = bundle.json(worker_record["attemptRecord"])
            gate.require(record == {key: value for key, value in worker_record.items()
                                    if key != "attemptRecord"}, "worker attempt differs")
            if record.get("accepted") is True:
                projection = gate._object(record.get("runtimeExecution"), "worker runtime")
                gate.require(projection.get("schema") == "krr-html-worker-runtime-execution-v1",
                             "original worker runtime schema differs")
                previous.append(projection)
    gate.require(len(previous) == 50, "original 50 worker observations required")
    environment_comparison(previous, previous[0])
    output.mkdir(parents=True)
    lifecycle = output / "lifecycle"
    worker = producer.ArmBuildResult([], binary, fixed.binary_sha256, proof, [])
    viewport = expected["viewport"]
    provenance = expected["provenance"]
    environment_sha = producer._environment_sha256(os.environ)
    expected["provenance"] = {**provenance, "environment_sha256": environment_sha}
    row: gate.JsonObject = {"catalog": True, "condition_id": catalog["condition_id"],
        "case_id": catalog["case_id"], "state": catalog["state"],
        "operation_kind": catalog["operation_kind"], "operation_evidence": expected["operationEvidenceRef"]}
    collected = producer._collect_condition_worker_pairs(
        folder=output, row=row, expected=expected, fixture=fixture, plan_path=Path(plan["path"]),
        width=viewport["width"], height=viewport["height"], viewport=viewport,
        builds={"baseline": worker, "candidate": worker},
        binaries={"baseline": binary, "candidate": binary}, arms=expected["arms"],
        root=root, lifecycle=lifecycle, lock=lock, snapshot=snapshot,
        environment_sha=environment_sha,
        batch_deadline=time.monotonic() + producer.BATCH_WORK_BUDGET_SECONDS,
        capture_runtime_execution=True, same_binary_control=True)
    gate.require(not collected.failures and collected.actual_phase_records == 300,
                 "diagnostic worker collection incomplete")
    groups: dict[str, list[gate.JsonObject]] = {"baseline": [], "candidate": []}
    for path in collected.reports:
        label = path.name.split("-", 1)[0]
        report = gate.read_json(path)
        role, samples = gate._validate_report(report, cast(gate.JsonObject, expected))
        gate.require(role == "candidate" and len(samples) == 5, "diagnostic report differs")
        groups[label].extend(samples)
    tests: gate.JsonObject = {}
    for phase in gate.PHASES:
        values = {role: [gate._phase_sample(sample, phase) for sample in samples]
                  for role, samples in groups.items()}
        phase_result: gate.JsonObject = {}
        for index, metric in enumerate(("elapsed_ms", "peak_rss_bytes")):
            baseline = [sample[index] for sample in values["baseline"]]
            candidate_values = [sample[index] for sample in values["candidate"]]
            phase_result[metric] = {
                "baselineMedian": statistics.median(baseline),
                "candidateMedian": statistics.median(candidate_values),
                "paired": paired_stats.evaluate_paired_regression(baseline, candidate_values).to_json(),
            }
        tests[phase] = phase_result
    actual_env = producer._performance_worker_environment(provenance["rustc"], provenance["cargo"],
                                                            environment_sha, lock, snapshot)
    projection = producer._worker_environment_projection(actual_env)
    for pair in collected.order:
        for record in gate._objects(pair.get("workerAttempts"), "diagnostic worker attempts"):
            if record.get("accepted") is True:
                observed = gate._object(record.get("runtimeExecution"), "diagnostic worker runtime")
                gate.require(all(observed.get(key) == value for key, value in projection.items()),
                             "diagnostic worker environments differ")
    producer.write_exclusive(output / "diagnostic-result.json", {
        "schema": "krr-html-same-binary-diagnostic-v1", "formalAccepted": False,
        "conditionId": condition_id, "worker": binary_pin, "sourceHead": origin["head"],
        "sourceSnapshotSha256": fixed.snapshot_sha256, "lockSha256": fixed.lock_sha256,
        "phaseRecords": collected.actual_phase_records, "tests": tests,
        **environment_comparison(previous, projection),
    })
