"""Validate a same-host build of the selected historical reference source."""
from __future__ import annotations

from pathlib import Path

import html_performance_reference as reference
import verify_html_performance_evidence as gate


def require_fresh_source_inputs(bundle: gate.Bundle, expected: gate.JsonObject,
                                proof: gate.JsonObject) -> None:
    endpoints = gate._object(expected.get("endpointRefs"), "reference endpoint refs")
    context = bundle.json(endpoints.get("before"))
    rows = gate._objects(context.get("inputs"), "reference context inputs")
    inputs = {gate._text(row.get("path"), "reference input path"): row for row in rows}
    gate.require(len(inputs) == len(rows), "reference context inputs are duplicated")
    fields = ("controllerSourceBeforeRef", "selectedSourceProofRef", "sourceBeforeRef", "sourceSnapshotRef")
    contract = gate._object(expected.get("referenceParent"), "fresh reference contract")
    gate.require(contract.get("sourceRefs") == [proof.get(field) for field in fields],
                 "fresh reference source role declarations differ")
    source_pair = gate._object(expected.get("sourcePair"), "measurement source pair")
    measured_source = gate._object(source_pair.get("before"), "measurement source before")
    controller_source = gate._object(proof.get("controllerSourceBeforeRef"), "controller source before")
    gate.require(measured_source.get("sha256") == controller_source.get("sha256"),
                 "fresh reference controller source differs from measurement")
    for field in fields:
        pin = gate._object(proof.get(field), field)
        path = gate._text(pin.get("path"), field + " path")
        bundle.file(pin)
        row = inputs.get(path)
        gate.require(row is not None and row.get("exists") is True and row.get("sha256") == pin.get("sha256"),
                     "fresh reference source is absent from measurement context")


def validate_fresh_reference(bundle: gate.Bundle, root: Path, expected: gate.JsonObject, *,
                             git_query: gate.GitQuery | None = None) -> gate.JsonObject:
    contract = gate._object(expected.get("referenceParent"), "fresh reference contract")
    source_pair = gate._object(expected.get("sourcePair"), "source pair")
    before = bundle.json(source_pair.get("before"))
    head = gate._text(before.get("head"), "controller source HEAD")
    def query(path: Path, args: tuple[str, ...]) -> str:
        return gate.git(path, *args, git_query=git_query)
    selected = reference.active_reference_at_commit(root, head, query)
    if selected is None:
        raise ValueError("active reference is unavailable")
    role = "fresh-history-source-" + selected.record_id
    gate.require(gate.baseline_arm_id(expected) == role and contract.get("parentHead") == selected.source_head,
                 "fresh reference selection differs")
    arm = gate._object(gate._object(expected.get("arms"), "expected arms").get(role), "fresh reference arm")
    artifact = gate._object(arm.get("build_artifact_ref"), "fresh reference build")
    proof = bundle.json(artifact)
    require_fresh_source_inputs(bundle, expected, proof)
    source_proof = bundle.json(proof.get("selectedSourceProofRef"))
    source_root = Path(gate._text(source_proof.get("materializedRoot"), "historical source root"))
    result = gate._validate_build_proof(bundle, root, artifact, "candidate", git_query=git_query,
                                        historical_reference=selected, fresh_source_root=source_root)
    gate.require(result.get("head") == selected.source_head
                 and result.get("binarySha256") == arm.get("binary_sha256")
                 and result.get("sourceSnapshotSha256") == arm.get("source_snapshot_sha256")
                 and result.get("lockSha256") == arm.get("lock_sha256"),
                 "fresh reference build differs from its declared role")
    result["arm"] = "baseline"
    return result


def validate_remote_fresh_reference(bundle: gate.Bundle, root: Path, expected: gate.JsonObject,
                                    head: str) -> gate.JsonObject:
    from html_performance_remote_evidence import validate_remote_candidate_build

    def query(path: Path, args: tuple[str, ...]) -> str:
        return gate.git(path, *args)
    selected = reference.active_reference_at_commit(root, head, query)
    if selected is None:
        raise ValueError("active reference is unavailable")
    role = "fresh-history-source-" + selected.record_id
    contract = gate._object(expected.get("referenceParent"), "fresh reference contract")
    gate.require(gate.baseline_arm_id(expected) == role and contract.get("parentHead") == selected.source_head,
                 "remote fresh reference selection differs")
    arm = gate._object(gate._object(expected.get("arms"), "remote arms").get(role), "fresh reference arm")
    require_fresh_source_inputs(bundle, expected, bundle.json(arm.get("build_artifact_ref")))
    result = validate_remote_candidate_build(bundle, root, arm.get("build_artifact_ref"),
                                             selected.source_head, historical=selected)
    gate.require(result.get("binarySha256") == arm.get("binary_sha256")
                 and result.get("sourceSnapshotSha256") == arm.get("source_snapshot_sha256")
                 and result.get("lockSha256") == arm.get("lock_sha256"),
                 "remote fresh reference build differs from its declared role")
    return result
