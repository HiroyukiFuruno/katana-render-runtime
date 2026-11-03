"""既に認証されたepoch入力の宣言を比較する。"""
from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from pathlib import Path
from typing import TypedDict

import verify_html_performance_evidence as gate


class DeclaredEpochInputs(TypedDict):
    workers: Mapping[str, gate.JsonObject]
    programs: Mapping[str, gate.JsonObject]
    inputs: Mapping[str, gate.JsonObject]
    resources: Mapping[str, gate.JsonObject]
    fonts: Mapping[str, gate.JsonObject]
    tool_context: gate.JsonObject


def _verified_pins(reader: gate.FileReader, values: Mapping[str, gate.JsonObject],
                   label: str) -> dict[str, tuple[str, str]]:
    result: dict[str, tuple[str, str]] = {}
    for role, ref in values.items():
        gate.require(isinstance(role, str) and bool(role), label + " role is invalid")
        pin = gate._object(ref, label + " pin")
        gate.require(set(pin) == {"path", "sha256"}, label + " pin fields differ")
        path = gate._text(pin.get("path"), label + " path")
        checksum = gate._text(pin.get("sha256"), label + " SHA")
        gate.digest(checksum)
        gate.require(gate.sha256(reader.file(pin)) == checksum, label + " bytes differ")
        result[role] = (path, checksum)
    return result


def compare_declared_inputs(old_reader: gate.FileReader, old: DeclaredEpochInputs,
                            new_reader: gate.FileReader, new: DeclaredEpochInputs) -> None:
    """宣言済み入力だけを比較する。宣言の完全性や測定適格性は呼出側が検証する。"""
    for epoch in (old, new):
        gate.require(set(epoch["workers"]) == {"baseline", "candidate"},
                     "epoch worker roles differ")
        gate.require(bool(epoch["programs"]) and bool(epoch["inputs"]),
                     "epoch program or input inventory is empty")
        context = epoch["tool_context"]
        gate.require(all(name in context for name in ("environmentSha256", "platform", "cpuCount")),
                     "epoch tool context is incomplete")
        gate.digest(context["environmentSha256"])
        gate.require(bool(gate._text(context["platform"], "epoch platform"))
                     and gate._integer(context["cpuCount"], "epoch CPU count") > 0,
                     "epoch platform or CPU count is invalid")
    groups = (("workers", old["workers"], new["workers"]),
              ("programs", old["programs"], new["programs"]),
              ("inputs", old["inputs"], new["inputs"]),
              ("resources", old["resources"], new["resources"]),
              ("fonts", old["fonts"], new["fonts"]))
    for name, old_group, new_group in groups:
        old_pins = _verified_pins(old_reader, old_group, "old " + name)
        new_pins = _verified_pins(new_reader, new_group, "new " + name)
        gate.require(old_pins == new_pins, "epoch " + name + " differs")
    gate.require(old["tool_context"] == new["tool_context"], "epoch tool context differs")


class EpochProjection(TypedDict):
    declared: DeclaredEpochInputs
    operational_pins: dict[str, gate.JsonObject]
    operational_digest: str
    unclassified_context_digest: str


def _inventory_digest(value: object) -> str:
    data = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(data.encode("utf8")).hexdigest()


def _pin(value: object, label: str) -> gate.JsonObject:
    ref = gate._object(value, label)
    path = gate._text(ref.get("path"), label + " path")
    checksum = gate._text(ref.get("sha256"), label + " SHA")
    gate.digest(checksum)
    return {"path": path, "sha256": checksum}


def _program_pins(reader: gate.FileReader, root: Path, snapshot: gate.JsonObject,
                  programs: Mapping[str, gate.JsonObject]) -> dict[str, tuple[str, str]]:
    result = _verified_pins(reader, programs, "program")
    gate.require(bool(result), "program inventory is empty")
    records = gate.records_map(reader.json(snapshot)["records"])
    for path, checksum in result.values():
        relative = Path(path).relative_to(root).as_posix()
        gate.require(records.get(relative, (None,))[0] == checksum,
                     "program source snapshot differs")
    return result


def _capacity_pins(reader: gate.FileReader, proof: gate.JsonObject,
                   context: gate.JsonObject) -> dict[str, gate.JsonObject]:
    capacity_pin = _pin(proof.get("capacityInputRef"), "capacity input")
    capacity = reader.json(capacity_pin)
    binding_pin = _pin(capacity.get("bindingRef"), "capacity binding")
    binding = reader.json(binding_pin)
    gate.require(binding.get("schema") == "krr-html-performance-capacity-binding-v1",
                 "capacity binding schema differs")
    source_pin = _pin(capacity.get("sourceBeforeRef"), "capacity source")
    reader.file(source_pin)
    pins = {"capacityInputRef": capacity_pin, "bindingRef": binding_pin,
            "sourceBeforeRef": source_pin}
    for role, binding_role in (("stateRef", "state"), ("protectedRegistryRef", "protectedRegistry")):
        pin = _pin(capacity.get(role), role)
        gate.require(pin == _pin(binding.get(binding_role), binding_role),
                     "capacity binding reference differs")
        pins[role] = pin
    rows = gate._objects(context["inputs"], "context inputs")
    indexed = {gate._text(row["path"], "context path"): row for row in rows}
    gate.require(len(indexed) == len(rows), "duplicate context input")
    for role in ("bindingRef", "stateRef", "protectedRegistryRef"):
        pin = pins[role]
        row = indexed.get(gate._text(pin["path"], role))
        gate.require(row is not None and row.get("exists") is True
                     and row.get("sha256") == pin["sha256"], "capacity context differs")
        reader.file(pin)
    return pins


def _operational_pins(reader: gate.FileReader, root: Path, context: gate.JsonObject,
                      expected: gate.JsonObject, snapshot: gate.JsonObject,
                      capture: gate.JsonObject | None, links: list[gate.JsonObject],
                      refs: Mapping[str, gate.JsonObject],
                      binaries: Mapping[str, gate.JsonObject]) -> dict[str, gate.JsonObject]:
    pins = {"sourceSnapshot": _pin(snapshot, "snapshot")}
    arms = gate._object(expected["arms"], "expected arms")
    for role, arm in (("baselineBuild", gate.baseline_arm_id(expected)),
                      ("candidateBuild", "candidate")):
        pins[role] = _pin(gate._object(arms[arm], "arm")["build_artifact_ref"], role)
    proof = reader.json(pins["candidateBuild"])
    gate.require(proof.get("schema") == "krr-html-performance-build-artifact-v3"
                 and proof.get("arm") == "candidate"
                 and _pin(proof.get("frozenBinary"), "candidate frozen binary") == _pin(binaries["candidate"], "candidate binary")
                 and _pin(proof.get("sourceSnapshotRef"), "candidate snapshot") == _pin(snapshot, "snapshot"),
                 "candidate build proof differs")
    capacity_pins = _capacity_pins(reader, proof, context)
    source_pair = gate._object(expected.get("sourcePair"), "source pair")
    gate.require(_pin(proof.get("sourceBeforeRef"), "candidate source")
                 == capacity_pins["sourceBeforeRef"]
                 == _pin(source_pair.get("before"), "source before"),
                 "capacity source reference differs")
    pins.update(capacity_pins)
    for key in ("catalogManifestRef", "catalogPlanRef", "catalogCaptureValidationRef",
                "catalogBinaryProofRef", "catalogBuildReportRef", "catalogCaptureBinaryRef",
                "catalogCurrentBuildReportRef", "chromeCaptureRef", "chromeReceiptRef",
                "fontIdentityRequestRef", "fontIdentityResponseRef", "fontIdentityProbeBinary"):
        if capture is not None:
            pins[key] = _pin(expected[key], key)
    if capture is not None:
        pins["capture"] = _pin(capture, "capture")
        for index, pin in enumerate(links):
            pins["link:" + str(index)] = _pin(pin, "link")
    pins["lock"] = {"path": str(root / gate.HARNESS_ROOT / "Cargo.lock"), "sha256": gate.LOCK_SHA}
    context_independent = {"capacityInputRef", "sourceBeforeRef", "bindingRef", "stateRef", "protectedRegistryRef"}
    for role, pin in pins.items():
        path = gate._text(pin["path"], role)
        if role not in context_independent:
            gate.require(refs.get(path) == pin, "operational reference is not authenticated")
        reader.file(pin)
    return pins


def _execution_inputs(root: Path, expected: gate.JsonObject, capture: gate.JsonObject | None,
                      refs: Mapping[str, gate.JsonObject], operational: Mapping[str, gate.JsonObject],
                      retained: set[str]) -> dict[str, gate.JsonObject]:
    for key in ("operationPlanRef", "domStateRef"):
        retained.add(gate._text(gate._object(expected[key], key)["path"], key))
    if capture is not None:
        retained.add(gate._text(gate._object(expected["operationEvidenceRef"],
                                            "operation evidence")["path"], "operation evidence path"))
    catalog = expected.get("catalogCondition")
    fixture = (gate._text(gate._object(catalog, "catalog condition")["fixture_path"], "fixture path")
               if catalog is not None else str(root / "tests/fixtures/html/interaction-fidelity.html"))
    retained.add(fixture)
    retained.add(str(root / gate.HARNESS_ROOT / "Cargo.lock"))
    excluded = {gate._text(pin["path"], "operational path") for pin in operational.values()} - retained
    return {path: pin for path, pin in refs.items() if path not in excluded}


def declared_from_authenticated_refs(
    reader: gate.FileReader, root: Path, context: gate.JsonObject, expected: gate.JsonObject,
    snapshot: gate.JsonObject, capture: gate.JsonObject | None, links: list[gate.JsonObject],
    resources: Mapping[str, gate.JsonObject], fonts: Mapping[str, gate.JsonObject],
    binaries: Mapping[str, gate.JsonObject], programs: Mapping[str, gate.JsonObject],
) -> EpochProjection:
    """Project a previously validated attempt; this alone does not establish eligibility.

    The caller must validate the attempt, catalog and complete program closure.
    """
    refs = gate.authenticated_measurement_input_refs(
        root, context, expected, snapshot, capture, links, [*resources.values(), *fonts.values()], binaries,
    )
    for pin in refs.values():
        reader.file(pin)
    for category, values in (("workers", binaries), ("resources", resources), ("fonts", fonts)):
        for role, value in values.items():
            pin = _pin(value, category)
            path = gate._text(pin["path"], category + " path")
            gate.require(bool(role) and refs.get(path) == pin, category + " reference differs")
    program_pins = _program_pins(reader, root, snapshot, programs)
    operational = _operational_pins(reader, root, context, expected, snapshot, capture, links, refs, binaries)
    worker_paths = {gate._text(value["path"], "worker path") for value in binaries.values()}
    gate.require(not worker_paths.intersection(gate._text(pin["path"], "operational path")
                                                for pin in operational.values()),
                 "worker and operational roles overlap")
    retained = {path for path, _ in program_pins.values()} | worker_paths
    for group in (resources, fonts):
        retained.update(gate._text(pin["path"], "retained path") for pin in group.values())
    inputs = _execution_inputs(root, expected, capture, refs, operational, retained)
    known = set(refs) | {gate._text(pin["path"], "operational path") for pin in operational.values()}
    unknown = [row for row in gate._objects(context["inputs"], "context inputs")
               if gate._text(row["path"], "context input path") not in known]
    unknown.sort(key=lambda row: gate._text(row["path"], "context input path"))
    tool = gate._object(context["toolContext"], "tool context")
    gate.require({"rustc", "cargo", "platform", "cpuCount", "environmentSha256"} <= set(tool),
                 "epoch tool context is incomplete")
    declared: DeclaredEpochInputs = {"workers": binaries, "programs": programs, "inputs": inputs,
                                    "resources": resources, "fonts": fonts, "tool_context": tool}
    return {"declared": declared, "operational_pins": operational,
            "operational_digest": _inventory_digest(operational),
            "unclassified_context_digest": _inventory_digest(unknown)}


def compare_epoch_projections(old_reader: gate.FileReader, old: EpochProjection,
                              new_reader: gate.FileReader, new: EpochProjection) -> None:
    """Compare classified inputs while independently validating operational pins."""
    for reader, value in ((old_reader, old), (new_reader, new)):
        pins = value["operational_pins"]
        gate.require(_inventory_digest(pins) == value["operational_digest"],
                     "operational role inventory differs")
        _verified_pins(reader, pins, "operational")
        gate.digest(value["unclassified_context_digest"])
    gate.require(set(old["operational_pins"]) == set(new["operational_pins"])
                 and old["unclassified_context_digest"] == new["unclassified_context_digest"],
                 "epoch role or unclassified context differs")
    compare_declared_inputs(old_reader, old["declared"], new_reader, new["declared"])
