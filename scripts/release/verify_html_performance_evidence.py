#!/usr/bin/env python3
"""実測paired evidenceを再集計し、公開前の時間・RSS非劣化を検証する。"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import zipfile
from pathlib import Path
from datetime import datetime
from typing import Protocol, TypedDict, cast
from collections.abc import Sequence

JsonObject = dict[str, object]
SourceRecords = dict[str, tuple[str, int]]

class GitQuery(Protocol):
    def __call__(self, root: Path, args: Sequence[str], input_bytes: bytes | None,
                 text: bool) -> str | bytes: ...


class SourceContract(TypedDict):
    head: str
    source: SourceRecords
    snapshot: str


class Percentiles(TypedDict):
    median: float
    p95: float


class RssPercentiles(Percentiles):
    max: float


class PhaseAggregate(TypedDict):
    sample_count: int
    elapsed_ms: Percentiles
    peak_rss_bytes: RssPercentiles


class ArmAggregate(TypedDict):
    arm: str | None
    viewport: object
    sample_count: int
    phases: dict[str, PhaseAggregate]


class ComparisonAggregate(TypedDict):
    measurement_executed: bool
    acceptance: bool
    reason: str
    viewport: object
    arms: dict[str, ArmAggregate]
    comparison: dict[str, dict[str, float]]

VIEWPORTS = ((1280, 900), (960, 720), (620, 900))
LOCK_SHA = "923db924dc0585e622d6c537f25386b05e7f0e17cb49e5e823c806a9bfca5e76"
SOURCE_KEYS = ("inventoryDigest", "head", "index", "files", "trackedPaths", "untrackedPaths", "missingTrackedPaths")
ENDPOINT_FLAGS = ("sourceBeforeEqualsEnd", "inputsBeforeEqualsEnd", "registryBeforeEqualsEnd", "cacheBeforeEqualsEnd", "cacheUnionBeforeEqualsEnd", "cacheUnionScopeRestored", "configBeforeEqualsEnd", "preferenceBeforeEqualsEnd", "toolContextBeforeEqualsEnd")
HARNESS_ROOT = "scripts/html/performance-harness/"


class PendingEvidence(ValueError):
    pass


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def digest(value: object) -> None:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None, "invalid SHA256")


def _object(value: object, label: str) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise ValueError(f"{label} must be an object")
    return cast(JsonObject, value)


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise ValueError(f"{label} must be an array")
    return cast("list[object]", value)


def _objects(value: object, label: str) -> list[JsonObject]:
    return [_object(item, label) for item in _array(value, label)]


def _text(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be text")
    return value


def _integer(value: object, label: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{label} must be an integer")
    return value


def _numeric(value: object, label: str) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    return value


def unique_object(pairs: list[tuple[str, object]]) -> JsonObject:
    result = dict(pairs)
    require(len(result) == len(pairs), "duplicate JSON keys")
    return result


def read_json(path: Path) -> JsonObject:
    return _object(json.loads(path.read_bytes(), object_pairs_hook=unique_object), "JSON document")


def _git_output(root: Path, args: Sequence[str], *, git_query: GitQuery | None,
                input_bytes: bytes | None = None, text: bool = False) -> str | bytes:
    if git_query is not None:
        output = git_query(root, args, input_bytes, text)
    else:
        command = ["git", "-C", str(root), *args]
        if input_bytes is None:
            output = subprocess.check_output(command, text=text)
        else:
            output = subprocess.check_output(command, input=input_bytes, text=text)
    if text:
        require(isinstance(output, str), "Git text query returned binary output")
    else:
        require(isinstance(output, bytes), "Git binary query returned text output")
    return output


def git(root: Path, *args: str, git_query: GitQuery | None = None) -> str:
    output = _git_output(root, args, git_query=git_query, text=True)
    if not isinstance(output, str):
        raise ValueError("Git text query returned binary output")
    return output


def records_map(rows: object) -> SourceRecords:
    result: SourceRecords = {}
    for row in _objects(rows, "source records"):
        key = _text(row["path"], "source path")
        require(isinstance(key, str) and key not in result, "duplicate source path")
        checksum = _text(row["sha256"], "source SHA256")
        digest(checksum)
        size = _integer(row["size"], "source size")
        require(size >= 0, "invalid source size")
        result[key] = (checksum, size)
    return result


class Bundle:
    # 絶対パスを含む元証跡は書き換えず、輸送索引だけでファイルを再配置する。
    def __init__(self, directory: Path) -> None:
        self.directory = directory.resolve()
        self.manifest = read_json(self.directory / "bundle.json")
        require(self.manifest["schema"] == "krr-html-performance-release-bundle-v1", "unsupported transport schema")
        self.files: dict[str, tuple[Path, str]] = {}
        used = set()
        for row in _objects(self.manifest["files"], "transport files"):
            original = _text(row["originalPath"], "original path")
            relative = Path(_text(row["relativePath"], "transport path"))
            path = (self.directory / relative).resolve()
            require(Path(original).is_absolute() and original not in self.files, "duplicate/nonabsolute original path")
            require(not relative.is_absolute() and path.is_relative_to(self.directory) and path not in used, "invalid transport path")
            checksum = _text(row["sha256"], "transport SHA256")
            digest(checksum)
            require(path.is_file() and sha256(path) == checksum, "transport file hash mismatch")
            self.files[original] = (path, checksum)
            used.add(path)

    def file(self, reference: object) -> Path:
        ref = _object(reference, "file reference")
        digest(ref["sha256"])
        path, recorded = self.files[_text(ref["path"], "file reference path")]
        require(recorded == ref["sha256"] and sha256(path) == recorded, "original pin mismatch")
        return path

    def json(self, ref: object) -> JsonObject:
        return read_json(self.file(ref))


def committed_records(root: Path, ref: str = "HEAD", *, git_query: GitQuery | None = None) -> SourceRecords:
    tree = _git_output(root, ("ls-tree", "-r", "-z", ref), git_query=git_query)
    if not isinstance(tree, bytes):
        raise ValueError("Git tree query returned text output")
    rows = []
    for entry in tree.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, kind, oid = metadata.decode().split()
        require(kind == "blob" and mode in ("100644", "100755"), "unsupported committed source type")
        rows.append((os.fsdecode(raw_path), oid))
    payload = _git_output(root, ("cat-file", "--batch"), git_query=git_query,
                          input_bytes="".join(oid + "\n" for _, oid in rows).encode())
    if not isinstance(payload, bytes):
        raise ValueError("Git blob query returned text output")
    result: SourceRecords = {}
    offset = 0
    for name, oid in rows:
        header_end = payload.index(b"\n", offset)
        actual_oid, kind, size = payload[offset:header_end].decode().split()
        require(actual_oid == oid and kind == "blob", "committed blob query differs")
        start, length = header_end + 1, int(size)
        data = payload[start:start + length]
        require(len(data) == length and payload[start + length:start + length + 1] == b"\n", "truncated committed blob")
        result[name] = (hashlib.sha256(data).hexdigest(), length)
        offset = start + length + 1
    require(offset == len(payload), "extra committed blob data")
    return result


def current_source(root: Path, before: JsonObject, *, git_query: GitQuery | None = None) -> SourceRecords:
    require(before["schema"] == "krr-native-build-source-inventory-v1", "unsupported source schema")
    require(before["sourceManifestSchema"] == "krr-native-build-source-manifest-v1", "unsupported source manifest")
    require(before["head"] == git(root, "rev-parse", "HEAD", git_query=git_query).strip(), "stale source HEAD")
    require(not git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query), "release source is dirty or untracked")
    origin = Path(_text(before["root"], "source root"))
    require(origin.is_absolute(), "absolute source root required")
    def relative(name: str) -> str:
        return str(Path(name).relative_to(origin))
    committed = committed_records(root, git_query=git_query)
    saved_tracked = [relative(_text(p, "tracked path")) for p in _array(before["trackedPaths"], "tracked paths")]
    require(len(set(saved_tracked)) == len(saved_tracked) and set(saved_tracked) == set(committed), "committed source membership differs")
    require(before["untrackedPaths"] == [] and before["missingTrackedPaths"] == [], "measured source is untracked or missing")
    saved = {relative(p): value for p, value in records_map(before["files"]).items()}
    require(saved == committed, "measured source differs from complete committed HEAD blobs")
    for name, (checksum, size) in committed.items():
        path = root / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size == size and sha256(path) == checksum, "working source differs from committed HEAD: " + name)
    return saved


def source_contract(bundle: Bundle, root: Path, expected: JsonObject, *,
                    git_query: GitQuery | None = None) -> SourceContract:
    contract = _object(expected["sourcePair"], "source pair")
    before, end, approved = (bundle.json(contract[k]) for k in ("before", "end", "approvedSourceRef"))
    require(all(document.get("stableEndpoints") is True for document in (before, end, approved)), "unstable source endpoint")
    require(all(before[k] == end[k] == approved[k] for k in SOURCE_KEYS), "source epoch differs")
    require(before["inventoryDigest"] == contract["approvedInventoryDigest"], "approved inventory differs")
    approved_ref = _object(contract["approvedSourceRef"], "approved source reference")
    matches = [p for p in _objects(contract["inputPins"], "source input pins") if p.get("path") == approved_ref["path"]]
    require(len(matches) == 1 and matches[0].get("sha256") == approved_ref["sha256"], "approved source pin missing")
    saved = current_source(root, before, git_query=git_query)
    snapshot = bundle.json(contract["snapshot"])
    rows = snapshot["records"]
    actual = records_map(rows)
    require(set(actual) == set(saved) and all(actual[p] == value for p, value in saved.items()), "snapshot source subset/extra")
    require(actual[HARNESS_ROOT + "Cargo.lock"][0] == LOCK_SHA, "harness lock differs")
    require(snapshot["digest"] == hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), "snapshot digest differs")
    snapshot_ref = _object(contract["snapshot"], "source snapshot reference")
    for arm_value in _object(expected["arms"], "expected arms").values():
        arm = _object(arm_value, "expected arm")
        require(arm["source_snapshot_sha256"] == snapshot_ref["sha256"] and arm["lock_sha256"] == LOCK_SHA, "shared build provenance differs")
    return {"head": _text(before["head"], "source HEAD"), "source": saved, "snapshot": _text(snapshot_ref["sha256"], "snapshot SHA256")}


PHASES = ("cold_open", "warm_open", "re_display", "full_y", "post_operation")


def _number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise ValueError(f"{label} must be finite and positive")
    return number


def _percentile(values: list[float], percentile: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * percentile
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def _phase_sample(sample: JsonObject, phase: str) -> tuple[float, float]:
    phases = sample.get("phases")
    if not isinstance(phases, dict) or phase not in phases:
        raise ValueError(f"sample is missing phase {phase}")
    value = phases[phase]
    if phase == "post_operation":
        if not isinstance(value, dict):
            raise ValueError("post_operation must be an object")
        value = value.get("measurement")
    if not isinstance(value, dict):
        raise ValueError(f"phase {phase} must be an object")
    elapsed = _number(value.get("elapsed_ms"), f"{phase}.elapsed_ms")
    rss = _number(value.get("peak_rss_bytes"), f"{phase}.peak_rss_bytes")
    return elapsed, rss


def _validate_report(report: JsonObject, expected: JsonObject) -> tuple[str, list[JsonObject]]:
    arm = report.get("arm")
    expected_arms = _object(expected["arms"], "expected arms")
    if not isinstance(arm, str) or arm not in expected_arms:
        raise ValueError(f"unknown arm provenance: {arm!r}")
    source_sha = report.get("source_sha256")
    if source_sha != expected["fixture_sha256"]:
        raise ValueError("report fixture SHA does not match expected provenance")
    viewport = report.get("viewport")
    if not isinstance(viewport, dict) or set(("width", "height", "scale")) - viewport.keys():
        raise ValueError("report has incomplete viewport provenance")
    if viewport != expected["viewport"]:
        raise ValueError("report viewport does not match expected provenance")
    for key in ("operation_plan_sha256", "operation", "expected_dom_state_sha256"):
        if report.get(key) != expected[key]:
            raise ValueError(f"report metadata mismatch for {key}")
    provenance = report.get("provenance")
    expected_arm = _object(expected_arms[arm], "expected arm")
    if not isinstance(provenance, dict):
        raise ValueError("report has no provenance")
    for key in ("binary_sha256", "source_snapshot_sha256", "deps_lock_sha256"):
        expected_key = {"binary_sha256": "binary_sha256", "source_snapshot_sha256": "source_snapshot_sha256", "deps_lock_sha256": "lock_sha256"}[key]
        if provenance.get(key) != expected_arm.get(expected_key, expected.get(expected_key)):
            raise ValueError(f"{arm} provenance mismatch for {key}")
    for key in ("rustc", "cargo", "environment_sha256"):
        if provenance.get(key) != _object(expected["provenance"], "expected provenance")[key]:
            raise ValueError(f"{arm} provenance mismatch for {key}")
    samples = report.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError("report has no samples")
    if report.get("sample_count") != len(samples):
        raise ValueError("sample_count does not match samples")
    sample_objects = _objects(samples, "sample")
    for sample in sample_objects:
        for phase in PHASES:
            _phase_sample(sample, phase)
        operation = _object(_object(sample["phases"], "sample phases")["post_operation"], "sample operation")
        if operation["operation"] != expected["operation"]:
            raise ValueError("sample operation does not match operation plan")
        if operation["expected_dom_state_sha256"] != expected["expected_dom_state_sha256"]:
            raise ValueError("sample expected-state SHA does not match operation plan")
    return arm, sample_objects


def aggregate_reports(reports: list[JsonObject], expected: JsonObject) -> ArmAggregate:
    if not reports:
        raise ValueError("at least one report is required")
    samples: list[JsonObject] = []
    arm: str | None = None
    viewport: object = None
    for report in reports:
        report_arm, report_samples = _validate_report(report, expected)
        if arm is None:
            arm = report_arm
            viewport = report["viewport"]
        elif report_arm != arm or report["viewport"] != viewport:
            raise ValueError("reports do not share arm and viewport provenance")
        samples.extend(report_samples)
    aggregates: dict[str, PhaseAggregate] = {}
    for phase in PHASES:
        elapsed = [_phase_sample(sample, phase)[0] for sample in samples]
        rss = [_phase_sample(sample, phase)[1] for sample in samples]
        aggregates[phase] = {
            "sample_count": len(elapsed),
            "elapsed_ms": {"median": _percentile(elapsed, 0.5), "p95": _percentile(elapsed, 0.95)},
            "peak_rss_bytes": {"median": _percentile(rss, 0.5), "p95": _percentile(rss, 0.95), "max": max(rss)},
        }
    return {"arm": arm, "viewport": viewport, "sample_count": len(samples), "phases": aggregates}


def aggregate_comparison(report_paths: list[Path], expected: JsonObject) -> ComparisonAggregate:
    """交互順序の5組が揃ったときだけ集計する。"""
    if len(report_paths) != 10 or len({path.resolve() for path in report_paths}) != 10:
        raise ValueError("exactly ten distinct pair reports are required")
    grouped: dict[str, dict[int, JsonObject]] = {"baseline": {}, "candidate": {}}
    report_arm_for_filename = {"baseline": "baseline-0.4.23", "candidate": "candidate"}
    pattern = re.compile(r"^(baseline|candidate)-(\d+)\.json$")
    for path in report_paths:
        match = pattern.match(path.name)
        if not match:
            raise ValueError(f"report filename must identify arm and pair: {path.name}")
        arm, pair_text = match.groups()
        pair = int(pair_text)
        if pair not in range(5) or pair in grouped[arm]:
            raise ValueError("pair reports must contain each arm exactly once for pairs 0..4")
        report = read_json(path)
        report_arm, samples = _validate_report(report, expected)
        if report_arm != report_arm_for_filename[arm] or report.get("pair_index") != pair:
            raise ValueError("report arm or pair index does not match its filename")
        sample_ids = [_integer(sample.get("sample"), "sample id") for sample in samples]
        if sample_ids != list(range(5)) or len(set(sample_ids)) != 5:
            raise ValueError("each pair must contain unique samples 0..4")
        grouped[arm][pair] = report
    if any(set(pairs) != set(range(5)) for pairs in grouped.values()):
        raise ValueError("both arms must contain all five pair reports")
    arms = {
        arm: aggregate_reports([grouped[arm][pair] for pair in range(5)], expected)
        for arm in grouped
    }
    comparisons: dict[str, dict[str, float]] = {}
    for phase in PHASES:
        base = arms["baseline"]["phases"][phase]
        cand = arms["candidate"]["phases"][phase]
        comparisons[phase] = {
            "elapsed_ms_delta_median": cand["elapsed_ms"]["median"] - base["elapsed_ms"]["median"],
            "elapsed_ms_delta_p95": cand["elapsed_ms"]["p95"] - base["elapsed_ms"]["p95"],
            "peak_rss_bytes_delta_median": cand["peak_rss_bytes"]["median"] - base["peak_rss_bytes"]["median"],
            "peak_rss_bytes_delta_p95": cand["peak_rss_bytes"]["p95"] - base["peak_rss_bytes"]["p95"],
        }
    return {
        "measurement_executed": all(report.get("measurement_executed") is True for reports in grouped.values() for report in reports.values()),
        "acceptance": False,
        "reason": "non-regression acceptance remains false until paired measurement policy is applied",
        "viewport": expected["viewport"],
        "arms": arms,
        "comparison": comparisons,
    }



def validate_attempt(bundle: Bundle, root: Path, attempt: JsonObject, *,
                     git_query: GitQuery | None = None) -> JsonObject:
    expected = bundle.json(attempt["expected"])
    result = bundle.json(attempt["result"])
    manifest = bundle.json(attempt["pairManifest"])
    require(result.get("measurementExecuted") is True and all(result.get(k) is True for k in ENDPOINT_FLAGS), "measurement/endpoint proof absent or false")
    require(not result.get("failure") and not result.get("endpointFailure"), "failed measurement attempt")
    expected_arms = _object(expected["arms"], "expected arms")
    provenance = _object(expected["provenance"], "expected provenance")
    source_pair = _object(expected["sourcePair"], "source pair")
    require(set(expected_arms) == {"baseline-0.4.23", "candidate"}, "versioned baseline/candidate required")
    for key in ("fixture_sha256", "operation_plan_sha256", "expected_dom_state_sha256"):
        digest(expected[key])
    for arm_value in expected_arms.values():
        expected_arm = _object(arm_value, "expected arm")
        for key in ("binary_sha256", "source_snapshot_sha256", "lock_sha256"):
            digest(expected_arm[key])
    digest(provenance["environment_sha256"])
    for tool in ("rustc", "cargo"):
        tool_version = _text(provenance[tool], "tool provenance")
        require(tool_version.startswith(tool + " ") and "\n" in tool_version, "actual tool provenance absent")
    viewport = _object(expected["viewport"], "viewport")
    width, height = (_integer(viewport[key], "viewport dimension") for key in ("width", "height"))
    require(set(viewport) == {"width", "height", "scale"} and type(viewport["scale"]) in (int, float) and viewport["scale"] == 1, "invalid viewport types/scale")
    require(isinstance(expected["operation"], str) and bool(expected["operation"]), "missing operation")
    source = source_contract(bundle, root, expected, git_query=git_query)
    endpoints = _object(expected["endpointRefs"], "endpoint references")
    before_context, end_context = (bundle.json(endpoints[k]) for k in ("before", "end"))
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    require(all(before_context[key] == end_context[key] for key in mapping.values()), "raw endpoint context changed")
    require(before_context["source"] == bundle.json(source_pair["before"]) and end_context["source"] == bundle.json(source_pair["end"]), "source/context endpoint differs")
    require(provenance["environment_sha256"] == _object(endpoints["before"], "before endpoint reference")["sha256"], "measurement context pin differs")
    require(before_context["cacheUnion"] == {"registry": before_context["registry"], "target": before_context["cache"]}, "cache union proof differs")
    for tool in ("rustc", "cargo"):
        require(_object(before_context["toolContext"], "tool context")[tool] == provenance[tool], "measured tool context differs")
    plan = bundle.json(expected["operationPlanRef"])
    dom = bundle.json(expected["domStateRef"])
    operation_ref = _object(expected["operationPlanRef"], "operation reference")
    dom_ref = _object(expected["domStateRef"], "DOM reference")
    require(operation_ref["sha256"] == expected["operation_plan_sha256"] and dom_ref["sha256"] == expected["expected_dom_state_sha256"], "operation/DOM pin differs")
    require(plan["fixture_sha256"] == expected["fixture_sha256"] and plan["operation"] == expected["operation"] and plan["expected_dom_state_sha256"] == expected["expected_dom_state_sha256"] and plan["expected_dom_state_path"] == dom_ref["path"], "operation plan differs")
    require(source["source"]["tests/fixtures/html/interaction-fidelity.html"][0] == expected["fixture_sha256"], "performance fixture is not committed source")
    require(dom["inputSha256"] == expected["fixture_sha256"] and _object(dom["states"], "DOM states")["js"] == {"expanded": True, "result": "opened"}, "actual interaction DOM evidence absent")
    context_rows = _objects(before_context["inputs"], "context inputs")
    context_inputs = {_text(row["path"], "context input path"): row for row in context_rows}
    require(len(context_inputs) == len(context_rows), "duplicate context input")
    input_refs = [operation_ref, dom_ref, _object(source_pair["snapshot"], "snapshot reference")]
    for ref in input_refs:
        context_input = context_inputs[_text(ref["path"], "input reference path")]
        require(context_input["exists"] is True and context_input["sha256"] == ref["sha256"], "measurement input context differs")
    require(manifest["measurement_executed"] is True and manifest["input_sha256"] == expected["fixture_sha256"] and manifest["environment_sha256"] == provenance["environment_sha256"], "runner context differs")
    order, pins = _objects(manifest["order"], "pair order"), _objects(expected["reportPins"], "report pins")
    require(len(order) == len(pins) == 10, "five paired runs required")
    paths = []
    for index in range(len(order)):
        row, pin = order[index], pins[index]
        pair = index // 2
        arm = ("baseline", "candidate")[index % 2] if pair % 2 == 0 else ("candidate", "baseline")[index % 2]
        raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
        binary_sha = _object(expected_arms[raw_arm], "expected arm")["binary_sha256"]
        require(row["arm"] == arm and type(row["pair_index"]) is int and row["pair_index"] == pair and row["executable_sha256"] == binary_sha, "pair order/binary differs")
        bundle.file({"path": row["executable"], "sha256": binary_sha})
        binary_context = context_inputs[_text(row["executable"], "executable path")]
        require(binary_context["exists"] is True and binary_context["sha256"] == binary_sha, "binary context proof differs")
        require(row["output"] == pin["path"] and Path(_text(pin["path"], "report path")).name == f"{arm}-{pair}.json", "report path/order differs")
        path = bundle.file(pin)
        require(path.name == f"{arm}-{pair}.json", "transport must retain report basename")
        report = read_json(path)
        require(report["measurement_executed"] is True and type(report["pair_index"]) is int and type(report["sample_count"]) is int and report["sample_count"] == 5, "actual five samples required")
        samples = _objects(report["samples"], "report samples")
        require([s["sample"] for s in samples] == list(range(5)) and all(type(s["sample"]) is int and set(_object(s["phases"], "sample phases")) == set(PHASES) for s in samples), "incomplete sample/phase coverage")
        for sample in samples:
            require(sample["viewport"] == viewport, "worker viewport differs")
            phases = _object(sample["phases"], "sample phases")
            for phase in PHASES:
                measured = _object(phases[phase], "measured phase")
                if phase == "post_operation":
                    measured = _object(measured["measurement"], "post-operation measurement")
                frame = _object(measured["frame"], "measured frame")
                require(frame["viewport"] == viewport and frame["pixel_bytes"] == width * height * 4, "actual rendered viewport differs")
            full_y = _object(phases["full_y"], "full-Y phase")
            scroll_steps = _integer(full_y["scroll_steps"], "full-Y scroll steps")
            require(full_y["actual_bottom"] is True and scroll_steps > 0 and _number(full_y["max_scroll"], "full_y max_scroll") == full_y["final_scroll_y"], "full-Y span incomplete")
            operation = _object(phases["post_operation"], "post operation")
            digest(operation["pixel_sha_before"])
            digest(operation["pixel_sha_after"])
            require(_numeric(operation["generation_after"], "generation after") > _numeric(operation["generation_before"], "generation before") and operation["pixel_sha_before"] != operation["pixel_sha_after"] and operation["visual_change_observed"] is True, "operation did not produce a changed frame")
        loads = _objects(row["workerLoads"], "worker loads")
        require(len(loads) == 5, "all five worker load observations required")
        for load in loads:
            require(set(load) == {"before", "after"} and all(type(load[k]) in (int, float) and math.isfinite(_numeric(load[k], "worker load")) and 0 <= _numeric(load[k], "worker load") <= 0.25 for k in ("before", "after")), "worker quiet load limit exceeded")
        require(row["load_ratio_before"] == max(_numeric(x["before"], "worker before load") for x in loads) and row["load_ratio_after"] == max(_numeric(x["after"], "worker after load") for x in loads), "worker load aggregate differs")
        for key in ("load_ratio_before", "load_ratio_after"):
            value = _numeric(row[key], "quiet load")
            require(type(value) in (int, float) and math.isfinite(value) and 0 <= value <= 0.25, "quiet load limit exceeded")
        paths.append(path)
    aggregate = aggregate_comparison(paths, expected)
    require(aggregate["measurement_executed"] is True, "actual measurements absent")
    saved = _object(result["policy"], "measurement policy")
    require(saved["aggregate"] == aggregate and saved["deltaThreshold"] == 0 and saved["quietLoadLimit"] == 0.25 and saved["phaseRecords"] == 250, "saved policy/aggregate differs from raw reports")
    policy = {phase: {key: value <= 0 for key, value in deltas.items()} for phase, deltas in aggregate["comparison"].items()}
    require(saved["policy"] == policy and saved["policyPass"] is True and all(ok for phase in policy.values() for ok in phase.values()), "time/RSS median/p95 regression")
    require(all(a["sample_count"] == 25 and all(p["sample_count"] == 25 for p in a["phases"].values()) for a in aggregate["arms"].values()), "250 phase records required")
    # 診断用acceptance:falseは許可し、生データの20条件だけで受理する。
    return {"viewport": expected["viewport"], "source": source, "fixture": expected["fixture_sha256"], "operation": expected["operation"], "plan": expected["operation_plan_sha256"], "dom": expected["expected_dom_state_sha256"], "provenance": expected["provenance"], "arms": expected["arms"]}


def verify(directory: Path, root: Path, *, git_query: GitQuery | None = None) -> JsonObject:
    bundle = Bundle(directory)
    head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(bundle.manifest["head"] == head, "bundle is not current HEAD")
    attempts = _objects(bundle.manifest["attempts"], "viewport attempts")
    require(len(attempts) == 3, "all three viewports required")
    values = [validate_attempt(bundle, root, attempt, git_query=git_query) for attempt in attempts]
    require([v["viewport"] for v in values] == [{"width": w, "height": h, "scale": 1} for w, h in VIEWPORTS], "viewport coverage/order differs")
    shared = [{k: v[k] for k in v if k != "viewport"} for v in values]
    require(shared[0] == shared[1] == shared[2], "three viewports do not share exact source/fixture/state/build/context")
    return {"head": head, "checks": 60, "phaseRecords": 750, "performanceNonregression": True}


def latest_acquisition_generation(runs: list[JsonObject]) -> JsonObject:
    if not runs:
        raise PendingEvidence("current-source CI run has not started")
    for run in runs:
        require(_integer(run.get("id"), "CI run id") > 0 and _integer(run.get("run_attempt"), "CI attempt") > 0, "invalid CI generation")
        started = _text(run.get("run_started_at"), "CI generation start time")
        require(re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", started) is not None, "invalid CI generation start time")
        try:
            datetime.strptime(started, "%Y-%m-%dT%H:%M:%SZ")
        except ValueError as error:
            raise ValueError("invalid CI generation start time") from error
    latest_start = max(_text(run["run_started_at"], "CI generation start time") for run in runs)
    latest = [run for run in runs if run["run_started_at"] == latest_start]
    require(len(latest) == 1, "ambiguous latest CI generation")
    return latest[0]


def acquire(directory: Path, root: Path, repository: str, current_producer: bool = False, *,
            git_query: GitQuery | None = None) -> None:
    # 同一SHAの成功CI由来だけを取得し、証跡の欠落を環境変数で免除しない。
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None, "invalid repository")
    head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(not directory.exists(), "acquisition destination already exists")
    def api(path: str) -> JsonObject:
        return _object(json.loads(subprocess.check_output(["gh", "api", path]), object_pairs_hook=unique_object), "GitHub response")
    pull: JsonObject
    pull_head: JsonObject
    pull_base: JsonObject
    if current_producer:
        # 自分のCIのpreflightだけが、既に成功した依存jobを全workflow完了前に読む。
        # 公開経路の取得は従来どおりcompleted/successのworkflowだけを受理する。
        require(os.environ.get("GITHUB_JOB") == "preflight" and os.environ.get("GITHUB_EVENT_NAME") == "pull_request", "current producer acquisition is only for PR preflight")
        event = read_json(Path(os.environ["GITHUB_EVENT_PATH"]))
        pull = _object(event["pull_request"], "pull request")
        pull_head = _object(pull["head"], "PR head")
        pull_base = _object(pull["base"], "PR base")
        require(pull["draft"] is False and pull_head["sha"] == head and _object(pull_head["repo"], "PR repository")["full_name"] == repository and pull_base["ref"] == "master" and re.fullmatch(r"release/v[0-9]+\.[0-9]+\.[0-9]+", _text(pull_head["ref"], "PR branch")) is not None, "current producer PR identity mismatch")
        run_id, attempt = int(os.environ["GITHUB_RUN_ID"]), int(os.environ["GITHUB_RUN_ATTEMPT"])
        require(run_id > 0 and attempt > 0, "invalid current CI generation")
        run = api(f"repos/{repository}/actions/runs/{run_id}")
        require(run["id"] == run_id and run["run_attempt"] == attempt and run["event"] == "pull_request" and run["path"] == ".github/workflows/test-and-build.yml" and _object(run["repository"], "CI repository")["full_name"] == repository and _object(run["head_repository"], "CI head repository")["full_name"] == repository, "current CI generation identity mismatch")
        require(any(binding["number"] == pull["number"] and _object(binding["head"], "CI PR head")["sha"] == head and _object(binding["base"], "CI PR base")["sha"] == pull_base["sha"] for binding in _objects(run["pull_requests"], "CI PR bindings")), "current CI PR head/base binding mismatch")
        jobs = api(f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100")
        job_rows = _objects(jobs["jobs"], "CI jobs")
        require(jobs["total_count"] == len(job_rows), "incomplete current CI jobs")
        for name in ("Test and Build (ubuntu-latest, linux64)", "Test and Build (macos-15, mac-arm64)", "Test and Build (windows-latest, win64)", "Native paired HTML time and RSS nonregression"):
            matches = [job for job in job_rows if job["name"] == name]
            require(len(matches) == 1 and matches[0]["run_id"] == run_id and matches[0]["status"] == "completed" and matches[0]["conclusion"] == "success", f"current producer dependency did not succeed: {name}")
    else:
        snapshot = api(f"repos/{repository}/actions/workflows/test-and-build.yml/runs?head_sha={head}&per_page=100")
        runs = _objects(snapshot["workflow_runs"], "workflow runs")
        require(type(snapshot["total_count"]) is int and snapshot["total_count"] == len(runs), "incomplete current-source workflow run page")
        runs = [r for r in runs if r["head_sha"] == head and _object(r["head_repository"], "CI head repository")["full_name"] == repository and r["path"] == ".github/workflows/test-and-build.yml"]
        run = latest_acquisition_generation(runs)
        if run["status"] != "completed":
            raise PendingEvidence("current-source CI/measurement is still running")
        require(run["conclusion"] == "success" and _integer(run["run_attempt"], "CI attempt") > 0, "latest exact-source CI attempt failed")
    if current_producer:
        runs_path = f"repos/{repository}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch={pull_head['ref']}&per_page=100"
    else:
        runs_path = f"repos/{repository}/actions/workflows/test-and-build.yml/runs?head_sha={head}&per_page=100"
    def current_latest() -> JsonObject:
        snapshot = api(runs_path)
        rows = _objects(snapshot["workflow_runs"], "workflow runs")
        require(type(snapshot["total_count"]) is int and snapshot["total_count"] == len(rows), "incomplete current-source workflow run page")
        rows = [row for row in rows if _object(row["head_repository"], "CI head repository")["full_name"] == repository and row["path"] == ".github/workflows/test-and-build.yml"]
        if current_producer:
            rows = [row for row in rows if row.get("event") == "pull_request" and _object(row.get("repository", {}), "CI repository").get("full_name") == repository and any(binding.get("number") == pull["number"] and _object(binding.get("head", {}), "CI PR head").get("sha") == head and _object(binding.get("base", {}), "CI PR base").get("sha") == pull_base["sha"] for binding in _objects(row.get("pull_requests", []), "CI PR bindings"))]
        else:
            rows = [row for row in rows if row["head_sha"] == head]
        return latest_acquisition_generation(rows)
    if current_producer:
        require(current_latest() == run, "current producer is not latest exact-PR CI generation")
    artifact_snapshot = api(f"repos/{repository}/actions/runs/{run['id']}/artifacts?per_page=100")
    artifacts = _objects(artifact_snapshot["artifacts"], "CI artifacts")
    require(artifact_snapshot["total_count"] == len(artifacts), "incomplete artifact page")
    matches = [a for a in artifacts if a["name"] == f"html-performance-paired-{head}-{run['id']}-{run['run_attempt']}" and a["expired"] is False]
    require(len(matches) == 1, "one immutable current-source performance artifact required")
    directory.mkdir(parents=True)
    archive = directory / "artifact.zip"
    archive.write_bytes(subprocess.check_output(["gh", "api", f"repos/{repository}/actions/artifacts/{matches[0]['id']}/zip"]))
    artifact_digest = matches[0].get("digest", "")
    require(artifact_digest == "sha256:" + sha256(archive), "GitHub artifact digest absent/mismatch")
    with zipfile.ZipFile(archive) as zipped:
        seen = set()
        for entry in zipped.infolist():
            path = (directory / entry.filename).resolve()
            require(path.is_relative_to(directory.resolve()) and path not in seen and (entry.external_attr >> 16) & 0o170000 != 0o120000, "unsafe/duplicate archive path")
            seen.add(path)
        zipped.extractall(directory)
    latest = api(f"repos/{repository}/actions/runs/{run['id']}")
    require(all(latest[k] == run[k] for k in ("id", "head_sha", "run_attempt", "status", "conclusion")), "CI run changed during acquisition")
    # ZIP取得中の新規runも照合し、古い成功世代だけのGETで取得を完了しない。
    require(current_latest() == run, "latest CI generation changed during acquisition")
    (directory / "acquisition.json").write_text(json.dumps({"repository": repository, "head": head, "runId": run["id"], "runAttempt": run["run_attempt"], "artifactId": matches[0]["id"], "artifactDigest": artifact_digest}, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--evidence-dir", type=Path, default=Path("tmp/html-performance-release-evidence"))
    parser.add_argument("--acquire", action="store_true")
    parser.add_argument("--acquire-current-producer", action="store_true")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    args = parser.parse_args()
    try:
        require(not (args.acquire and args.acquire_current_producer), "ambiguous acquisition mode")
        if args.acquire or args.acquire_current_producer:
            acquire(args.evidence_dir, args.repo, args.repository, args.acquire_current_producer)
        print(json.dumps(verify(args.evidence_dir, args.repo), sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"HTML performance release gate failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
