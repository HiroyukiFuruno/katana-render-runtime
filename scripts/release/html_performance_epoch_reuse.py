"""既に認証されたepoch入力の宣言を比較する。"""
from __future__ import annotations

import hashlib
import json
import os
import platform
import stat
from collections.abc import Mapping
from pathlib import Path
from typing import TypedDict

import html_performance_inventory as inventory
import local_macos_tool_provenance as provenance
import native_coverage_local_context as native_context
import verify_html_performance_evidence as gate


class SystemOnlyObservation(TypedDict):
    schema: str
    worker: gate.JsonObject
    stamp: list[int]
    platform: str
    environment_sha256: str


def observe_system_only_worker(
    worker: gate.JsonObject, tool_context: gate.JsonObject, environment: Mapping[str, str],
) -> SystemOnlyObservation:
    """Observe a frozen worker at the current epoch boundary; this is not historical proof."""
    pin = _pin(worker, "system-only worker")
    path = Path(gate._text(pin["path"], "system-only worker path"))
    gate.require(path.is_absolute() and str(path) == pin["path"] and path.resolve(strict=True) == path,
                 "system-only worker path differs")
    before = inventory._stamp(path)
    gate.require(len(before) == 6 and all(type(value) is int and value >= 0 for value in before)
                 and stat.S_ISREG(before[2]) and before[3] > 0,
                 "system-only worker is not a regular file")
    host = platform.platform()
    environment_digest = _inventory_digest(dict(environment))
    gate.require(platform.system() == "Darwin"
                 and tool_context.get("platform") == host
                 and tool_context.get("environmentSha256") == environment_digest,
                 "system-only host context differs")
    gate.require(not any(name.startswith("DYLD_") and value
                         for env in (environment, os.environ) for name, value in env.items()),
                 "system-only dynamic loader override")
    data = path.read_bytes()
    gate.require(len(data) == before[3] and hashlib.sha256(data).hexdigest() == pin["sha256"],
                 "system-only worker bytes differ")
    parsed = provenance._native_dependencies(data)
    if parsed is None:
        raise ValueError("system-only worker is not supported Mach-O")
    dependencies, rpaths = parsed
    gate.require(not rpaths, "system-only worker has rpaths")
    for dependency in dependencies:
        gate.require(dependency.startswith(("/usr/lib/", "/System/Library/"))
                     and os.path.normpath(dependency) == dependency,
                     "system-only worker has non-system dependency")
    records: dict[str, dict[str, object]] = {}
    native_context._record_macho_closure(path, records, "worker")
    gate.require(not records and not native_context._macho_rpaths(path),
                 "system-only worker closure differs")
    gate.require(inventory._stamp(path) == before and gate.sha256(path) == pin["sha256"]
                 and platform.platform() == host
                 and _inventory_digest(dict(environment)) == environment_digest,
                 "system-only worker or host changed during observation")
    return {"schema": "krr-html-system-only-worker-observation-v1", "worker": pin,
            "stamp": list(before), "platform": host, "environment_sha256": environment_digest}


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
                            new_reader: gate.FileReader, new: DeclaredEpochInputs, *,
                            program_changes: frozenset[str] = frozenset()) -> None:
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
        if name == "programs":
            from html_performance_source_reuse import VALIDATION_COHORT_ADDITIONS
            additions = set(new_pins) - set(old_pins)
            gate.require(not (set(old_pins) - set(new_pins))
                         and (not additions or additions == set(VALIDATION_COHORT_ADDITIONS))
                         and all((new_pins[key][1] == VALIDATION_COHORT_ADDITIONS[key][0]
                                  or key == "scripts/drawio/candidate-svg-rasterize.ts"
                                  and new_pins[key][1] == "96244e3710fd8b69307ad6da002cc67487bc6f55f43ed87d87eee7211e3ffbad")
                                 and new_pins[key][0].endswith(
                                     "/tmp/html-performance-programs/"
                                     + new_pins[key][1] + "/" + key)
                                 for key in additions)
                         and all(old_pins[key] == new_pins[key] or key in program_changes
                                 for key in old_pins), "epoch programs differs")
        else:
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
                  programs: Mapping[str, gate.JsonObject], *,
                  program_records: gate.SourceRecords | None = None) -> dict[str, tuple[str, str]]:
    result = _verified_pins(reader, programs, "program")
    gate.require(bool(result), "program inventory is empty")
    records = (gate.records_map(reader.json(snapshot)["records"])
               if program_records is None else program_records)
    for role, (path, checksum) in result.items():
        actual = Path(path)
        relative = actual.relative_to(root).as_posix()
        if role.startswith("scripts/") or role == "Justfile":
            alias = root / "tmp/html-performance-programs" / checksum / role
            gate.require(actual == root / role or actual == alias,
                         "program source storage path differs")
            relative = role
        gate.require(records.get(relative, (None,))[0] == checksum,
                     "program source snapshot differs")
    return result


def _capacity_pins(reader: gate.FileReader, root: Path, proof: gate.JsonObject,
                   context: gate.JsonObject,
                   measurement_binding: gate.JsonObject | None = None) -> dict[str, gate.JsonObject]:
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
    for pin in pins.values():
        reader.file(pin)
    if measurement_binding is None:
        measured = {role: pins[role] for role in ("bindingRef", "stateRef", "protectedRegistryRef")}
    else:
        from produce_html_performance_evidence import load_capacity_binding

        measured_binding = _pin(measurement_binding, "measurement capacity binding")
        measured_data = reader.json(measured_binding)
        gate.require(set(measured_data) == {"schema", "state", "protectedRegistry"}
                     and measured_data.get("schema") == "krr-html-performance-capacity-binding-v1",
                     "measurement capacity binding differs")
        validated = load_capacity_binding(root, Path(str(measured_binding["path"])),
                                          str(measured_binding["sha256"]), reader=reader)
        measured = {"bindingRef": measured_binding,
                    "stateRef": {"path": str(validated.state), "sha256": validated.state_sha256},
                    "protectedRegistryRef": {"path": str(validated.registry), "sha256": validated.registry_sha256}}
        pins.update({"measurement" + role[0].upper() + role[1:]: pin for role, pin in measured.items()})
    rows = gate._objects(context["inputs"], "context inputs")
    indexed = {gate._text(row["path"], "context path"): row for row in rows}
    gate.require(len(indexed) == len(rows), "duplicate context input")
    for role in ("bindingRef", "stateRef", "protectedRegistryRef"):
        pin = measured[role]
        row = indexed.get(gate._text(pin["path"], role))
        gate.require(row is not None and row.get("exists") is True
                     and row.get("sha256") == pin["sha256"], "capacity context differs")
        reader.file(pin)
    return pins


def _operational_pins(reader: gate.FileReader, root: Path, context: gate.JsonObject,
                      expected: gate.JsonObject, snapshot: gate.JsonObject,
                      capture: gate.JsonObject | None, links: list[gate.JsonObject],
                      refs: Mapping[str, gate.JsonObject],
                      binaries: Mapping[str, gate.JsonObject],
                      measurement_binding: gate.JsonObject | None = None) -> dict[str, gate.JsonObject]:
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
    capacity_pins = _capacity_pins(reader, root, proof, context, measurement_binding)
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
    context_independent = {"capacityInputRef", "sourceBeforeRef", "bindingRef", "stateRef", "protectedRegistryRef",
                           "measurementBindingRef", "measurementStateRef", "measurementProtectedRegistryRef"}
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
    *, program_records: gate.SourceRecords | None = None,
    measurement_binding: gate.JsonObject | None = None,
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
    program_pins = _program_pins(reader, root, snapshot, programs, program_records=program_records)
    operational = _operational_pins(reader, root, context, expected, snapshot, capture, links, refs, binaries,
                                    measurement_binding)
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
                              new_reader: gate.FileReader, new: EpochProjection, *,
                              program_changes: frozenset[str] = frozenset()) -> None:
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
    compare_declared_inputs(old_reader, old["declared"], new_reader, new["declared"],
                            program_changes=program_changes)
