#!/usr/bin/env python3
"""実測paired evidenceを再集計し、公開前の時間・RSS非劣化を検証する。"""
from __future__ import annotations

import argparse
import base64
import hashlib
import importlib
import json
import marshal
import math
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import types
import weakref
import zipfile
from functools import lru_cache
from pathlib import Path, PurePath, PurePosixPath, PureWindowsPath
from datetime import datetime
from threading import Lock
from typing import NamedTuple, Protocol, TypedDict, cast
from collections import OrderedDict
from collections.abc import Callable, Mapping, Sequence
import html_performance_reference as reference_domain
from html_performance_harness_dependency_closure import verify_harness_closure
from html_performance_paired_stats import evaluate_paired_regression

JsonObject = dict[str, object]
SourceRecords = dict[str, tuple[str, int]]
CATALOG_SOURCE_SUFFIX = ("tests", "fixtures", "html", "feature-catalog.json")

class GitQuery(Protocol):
    def __call__(self, root: Path, args: Sequence[str], input_bytes: bytes | None,
                 text: bool) -> str | bytes: ...


class FileReader(Protocol):
    def file(self, reference: object) -> Path: ...
    def json(self, reference: object) -> JsonObject: ...


class SharedCurrentSource(NamedTuple):
    root: Path
    head: str
    records: SourceRecords
    origin: Path


class SourceContract(TypedDict):
    head: str
    source: SourceRecords
    snapshot: str


class MemoryPolicyKwargs(TypedDict, total=False):
    memory_policy_version: str


def _memory_policy_kwargs(version: str | None) -> MemoryPolicyKwargs:
    return {"memory_policy_version": version} if version is not None else {}


class SourceEpochV3Validation(NamedTuple):
    """Context pins and projections authenticated by the v3 caller."""
    fixed_context_sha256: str
    before_context_sha256: str
    end_context_sha256: str
    fixed_projection: JsonObject
    before_projection: JsonObject
    end_projection: JsonObject


class ExpectedBuild(NamedTuple):
    harness: JsonObject
    runtime: JsonObject
    arm: str


class RustTool(NamedTuple):
    launcher: Path
    dispatcher: Path
    executable: Path
    resolution_argv: tuple[str, ...]


class RustupQuery(Protocol):
    def __call__(self, argv: Sequence[str], cwd: Path, environment: Mapping[str, str]) -> str: ...


def resolve_rust_tool(root: Path, name: str, environment: Mapping[str, str], *,
                      cwd: Path | None = None, query: RustupQuery | None = None) -> RustTool:
    require(name in {"cargo", "rustc"}, "unsupported Rust tool")
    value = environment.get("RUSTC") if name == "rustc" else None
    value = value or shutil.which(name, path=environment.get("PATH"))
    if value is None:
        raise ValueError(name + " executable is unavailable")
    launcher = Path(value)
    if not launcher.is_absolute():
        located = shutil.which(value, path=environment.get("PATH")) if len(launcher.parts) == 1 else None
        launcher = Path(located) if located is not None else (cwd or root) / launcher
    launcher = Path(os.path.abspath(launcher))
    require(launcher.is_file() and os.access(launcher, os.X_OK), name + " launcher is not executable")
    physical_launcher = launcher.resolve(strict=True)
    rustup_value = shutil.which("rustup", path=environment.get("PATH"))
    rustup = Path(rustup_value).resolve(strict=True) if rustup_value is not None else None
    resolution_argv: tuple[str, ...] = ()
    if physical_launcher.name == "rustup":
        if rustup is None or rustup != physical_launcher:
            raise ValueError(name + " Rustup proxy cannot be identified")
        command = [str(rustup), "which", name]
        resolution_argv = tuple(command)
        selected_output = (query(command, cwd or root, environment) if query is not None else
                           subprocess.check_output(command, cwd=cwd or root, env=dict(environment), text=True))
        selected = selected_output.strip()
        executable = Path(selected)
        require(selected == str(executable) and executable.is_absolute()
                and executable.resolve(strict=True) == executable and executable.is_file()
                and os.access(executable, os.X_OK), name + " rustup toolchain path is invalid")
    else:
        executable = physical_launcher
    return RustTool(launcher, physical_launcher, executable, resolution_argv)


def validate_rust_tool_pin(proof: JsonObject, name: str, tool: RustTool) -> None:
    launcher_info = _object(proof.get(name + "Launcher"), name + " launcher pin")
    launcher = _canonical_build_path(launcher_info.get("path"), name + " launcher path")
    dispatcher = _canonical_build_file(launcher_info.get("resolvedPath"), name + " dispatcher path")
    resolution_argv = [_text(item, name + " resolver argv") for item
                       in _array(launcher_info.get("resolutionArgv"), name + " resolver argv")]
    executable_info = _object(proof.get(name + "Executable"), name + " executable pin")
    executable = _canonical_build_file(executable_info.get("path"), name + " executable path")
    require(launcher == tool.launcher and dispatcher == tool.dispatcher
            and resolution_argv == list(tool.resolution_argv)
            and sha256(dispatcher) == _text(launcher_info.get("sha256"), name + " dispatcher SHA")
            and executable == tool.executable
            and sha256(executable) == _text(executable_info.get("sha256"), name + " executable SHA"),
            name + " executable differs from build receipt")


def rust_tool_launcher_pin(tool: RustTool) -> JsonObject:
    return {"path": str(tool.launcher), "resolvedPath": str(tool.dispatcher),
            "sha256": sha256(tool.dispatcher), "resolutionArgv": list(tool.resolution_argv)}


class CargoCapacityGuard(Protocol):
    def validate_build_selection(self, root: Path, environment: Mapping[str, str]) -> None: ...

    def validate_build_executable(self, root: Path, command: list[str],
                                  environment: Mapping[str, str]) -> None: ...

    def cargo_configuration_paths(self, root: Path, environment: Mapping[str, str]) -> list[Path]: ...


def _cargo_capacity_guard() -> CargoCapacityGuard:
    build_directory = str(Path(__file__).resolve().parents[1] / "build")
    if build_directory not in sys.path:
        sys.path.insert(0, build_directory)
    return cast(CargoCapacityGuard, importlib.import_module("cargo_capacity_guard"))


def guarded_build_argv(root: Path, harness: Path, cargo: Path, arm: str, state: Path,
                       state_sha256: str, references: Sequence[str], profiles: Sequence[str], *,
                       guard_python: Path | None = None) -> list[str]:
    argv = [str(guard_python) if guard_python is not None else sys.executable, str(root / "scripts/build/cargo_capacity_guard.py"), "--root", str(root),
            "--target-dir", str(root / "target"), "--state", str(state), "--factor", "2",
            "--expected-state-sha256", state_sha256]
    for reference in references:
        argv.extend(["--protected-ref", reference])
    for profile in profiles:
        argv.extend(["--retain-profile", profile])
    argv.extend(["--", str(cargo), "build", "--release", "--locked", "--manifest-path",
                 str(harness / "Cargo.toml"), "--target-dir", str(root / "target"),
                 "--message-format=json", "--no-default-features", "--features", arm])
    return argv


def validate_guarded_build_argv(argv: Sequence[str], root: Path, harness: Path, cargo: Path, arm: str,
                               state_path: Path, state_sha256: str,
                               expected_references: Sequence[str], *, guard_python: Path | None = None) -> None:
    selected_python = guard_python if guard_python is not None else Path(sys.executable)
    fixed_prefix = [str(selected_python), str(root / "scripts/build/cargo_capacity_guard.py"), "--root", str(root),
                    "--target-dir", str(root / "target"), "--state"]
    require(list(argv[:7]) == fixed_prefix and len(argv) > 12, "build argv guard prefix differs")
    argv_state_path = Path(argv[7])
    require(state_path.is_absolute() and str(state_path) == os.path.normpath(str(state_path))
            and argv_state_path == state_path and argv[7] == str(argv_state_path)
            and argv[11] == state_sha256
            and list(argv[8:11]) == ["--factor", "2", "--expected-state-sha256"],
            "build argv recorded capacity state/factor differs")
    index, references, profiles = 12, [], []
    while index < len(argv) and argv[index] == "--protected-ref":
        require(index + 1 < len(argv), "build argv protected reference is incomplete")
        references.append(argv[index + 1])
        index += 2
    while index < len(argv) and argv[index] == "--retain-profile":
        require(index + 1 < len(argv), "build argv retained profile is incomplete")
        profiles.append(argv[index + 1])
        index += 2
    require(index < len(argv) and argv[index] == "--" and references == list(expected_references)
            and bool(references) and len(set(references)) == len(references),
            "build argv has missing/duplicate/unbound capacity references")
    for value in references:
        path = Path(value)
        require(path.is_absolute() and str(path) == value,
                "build argv contains a noncanonical protected reference")
    for value in profiles:
        path = Path(value)
        require(path.is_absolute() and str(path) == value,
                "build argv contains a noncanonical retained profile")
    expected = guarded_build_argv(root, harness, cargo, arm, state_path, state_sha256, references, profiles,
                                  guard_python=selected_python)
    require(list(argv) == expected, "build argv differs from canonical guarded Cargo command")


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
LOCK_SHA = "fd56d97336b4156b56c1fb89ecc79a5155ada5523b76c3e5c0d91820febcd9f0"
SOURCE_KEYS = ("inventoryDigest", "head", "index", "files", "trackedPaths", "untrackedPaths", "missingTrackedPaths")
ENDPOINT_FLAGS = ("sourceBeforeEqualsEnd", "inputsBeforeEqualsEnd", "registryBeforeEqualsEnd", "cacheBeforeEqualsEnd", "cacheUnionBeforeEqualsEnd", "cacheUnionScopeRestored", "configBeforeEqualsEnd", "preferenceBeforeEqualsEnd", "toolContextBeforeEqualsEnd")
HARNESS_ROOT = "scripts/html/performance-harness/"
DEFAULT_BASELINE_ARM_ID = "baseline-0.4.23"
REFERENCE_PARENT_ARM_ID = "reference-parent-371274ed"
NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES = {
    "navigation/top/1280x900@1": "#a",
    "navigation/top/960x720@1": "#a",
    "navigation/top/620x900@1": "#a",
    "navigation/middle/1280x900@1": "#b",
    "navigation/middle/960x720@1": "#b",
    "navigation/middle/620x900@1": "#b",
    "navigation/bottom/1280x900@1": "#c",
    "navigation/bottom/960x720@1": "#c",
    "navigation/bottom/620x900@1": "#c",
    "navigation/all-toc-links/1280x900@1": "#a",
    "navigation/all-toc-links/960x720@1": "#a",
    "navigation/all-toc-links/620x900@1": "#a",
}


class PendingEvidence(ValueError):
    pass


def baseline_arm_id(expected: Mapping[str, object]) -> str:
    """期待値が選択する比較基準armを返す。旧証跡は既定で不変にする。"""
    reference = expected.get("referenceParent")
    if reference is None:
        return DEFAULT_BASELINE_ARM_ID
    value = _object(reference, "reference parent contract")
    role = value.get("role")
    legacy = role == REFERENCE_PARENT_ARM_ID and value.get("referenceKind") == "correct-parent-source"
    record_id = value.get("recordId")
    historical = (isinstance(record_id, str) and bool(record_id)
                  and role == "reference-history-" + record_id
                  and value.get("referenceKind") == "accepted-history-candidate"
                  and value.get("originalArm") == "candidate")
    fresh = (isinstance(record_id, str) and bool(record_id)
             and role == "fresh-history-source-" + record_id
             and value.get("referenceKind") == "fresh-history-source"
             and value.get("originalArm") == "candidate")
    require((legacy or historical or fresh) and value.get("publishedSdk") is False,
            "reference parent contract identity differs")
    arm_id = _text(value.get("armId"), "reference parent arm id")
    require(arm_id == role, "reference parent arm id differs")
    return arm_id


def non_comparable_baseline_fragment(expected: Mapping[str, object]) -> str | None:
    """既知の旧parent fragment拒否だけを比較不能候補として識別する。"""
    if baseline_arm_id(expected) != REFERENCE_PARENT_ARM_ID:
        return None
    catalog = expected.get("catalogCondition")
    if not isinstance(catalog, dict):
        return None
    condition_id = catalog.get("condition_id")
    if not isinstance(condition_id, str):
        return None
    expected_operation_kind = (
        "navigation-all-pointer-clicks"
        if condition_id.startswith("navigation/all-toc-links/")
        else "navigation-pointer-click"
    )
    if catalog.get("operation_kind") != expected_operation_kind:
        return None
    return NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES.get(condition_id)


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


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


def provenance_file_refs(report: JsonObject, *,
                         file_resolver: Callable[[JsonObject], Path] | None = None) -> list[JsonObject]:
    refs: dict[str, JsonObject] = {}
    resources = report.get("resourceProvenance")
    if isinstance(resources, dict) and "requests" in resources:
        requests = resources.get("requests")
        if not isinstance(requests, list):
            raise ValueError("resource provenance requests are unavailable")
        for request in requests:
            item = _object(request, "resource provenance request")
            _add_provenance_ref(refs, _text(item.get("bodyPath"), "resource body path"),
                                _provenance_sha(item.get("bytesSha256"), "resource body SHA"), file_resolver)
    fonts = report.get("fontProvenance")
    if not isinstance(fonts, list):
        raise ValueError("font provenance is unavailable")
    for font in fonts:
        item = _object(font, "font provenance")
        require(item.get("fileBytesVerified") is True, "font physical provenance is unavailable")
        _add_provenance_ref(refs, _text(item.get("filePath"), "font file path"),
                            _provenance_sha(item.get("fileSha256"), "font file SHA"), file_resolver)
    return list(refs.values())


def _provenance_sha(value: object, label: str) -> str:
    digest(value)
    return _text(value, label)


def _add_provenance_ref(refs: dict[str, JsonObject], path: str, checksum: str,
                        file_resolver: Callable[[JsonObject], Path] | None = None) -> None:
    pin: JsonObject = {"path": path, "sha256": checksum}
    file_path = Path(path) if file_resolver is None else file_resolver(pin)
    require(file_path.is_file() and not file_path.is_symlink() and sha256(file_path) == checksum,
            "provenance input file differs")
    previous = refs.get(path)
    require(previous is None or previous["sha256"] == checksum,
            "provenance path has conflicting hashes")
    refs[path] = {"path": path, "sha256": checksum}


def _required_context_input_refs(
    expected: JsonObject, snapshot: JsonObject, capture: JsonObject | None,
    links: list[JsonObject], provenance: list[JsonObject], execution_inputs: list[JsonObject],
) -> list[JsonObject]:
    refs = [_object(expected["operationPlanRef"], "operation reference"),
            _object(expected["domStateRef"], "DOM reference"), snapshot, *execution_inputs]
    if capture is not None:
        for key in ("operationEvidenceRef", "catalogManifestRef", "catalogPlanRef",
                    "catalogCaptureValidationRef", "catalogBinaryProofRef", "catalogBuildReportRef",
                    "catalogCaptureBinaryRef", "catalogCurrentBuildReportRef", "chromeCaptureRef",
                    "chromeReceiptRef", "fontIdentityRequestRef", "fontIdentityResponseRef",
                    "fontIdentityProbeBinary"):
            refs.append(_object(expected[key], key))
        refs.extend([capture, *links, *provenance])
    return refs


def validate_context_input_refs(
    before_context: JsonObject, expected: JsonObject, snapshot: JsonObject,
    capture: JsonObject | None, links: list[JsonObject], provenance: list[JsonObject],
    execution_inputs: list[JsonObject],
) -> dict[str, JsonObject]:
    context_rows = _objects(before_context["inputs"], "context inputs")
    context_inputs = {_text(row["path"], "context input path"): row for row in context_rows}
    require(len(context_inputs) == len(context_rows), "duplicate context input")
    refs = _required_context_input_refs(expected, snapshot, capture, links, provenance,
                                        execution_inputs)
    for ref in refs:
        path = _text(ref["path"], "input reference path")
        require(path in context_inputs, "measurement input context is missing")
        item = context_inputs[path]
        require(item["exists"] is True and item["sha256"] == ref["sha256"],
                "measurement input context differs")
    return context_inputs


def planned_execution_inputs(root: Path, expected: JsonObject,
                             binaries: Mapping[str, JsonObject]) -> list[JsonObject]:
    arms = _object(expected["arms"], "expected arms")
    baseline = baseline_arm_id(expected)
    require(set(binaries) == {"baseline", "candidate"}, "execution binary roles differ")
    refs: list[JsonObject] = []
    for role, raw_arm in (("baseline", baseline), ("candidate", "candidate")):
        binary = binaries[role]
        arm = _object(arms[raw_arm], "expected arm")
        require(binary.get("sha256") == arm.get("binary_sha256"), "execution binary role differs")
        refs.append(binary)
        refs.append(_object(arm.get("build_artifact_ref"), "execution build artifact"))
    contract = expected.get("referenceParent")
    if isinstance(contract, dict) and contract.get("referenceKind") == "fresh-history-source":
        source_refs = _objects(contract.get("sourceRefs"), "fresh reference source refs")
        require(len(source_refs) == 4, "fresh reference source ref inventory differs")
        refs.extend(source_refs)
    catalog = expected.get("catalogCondition")
    fixture = (_text(_object(catalog, "catalog condition")["fixture_path"], "fixture path")
               if catalog is not None else str(root / "tests/fixtures/html/interaction-fidelity.html"))
    refs.extend([{"path": fixture, "sha256": expected["fixture_sha256"]},
                 {"path": str(root / HARNESS_ROOT / "Cargo.lock"), "sha256": LOCK_SHA}])
    return refs


def authenticated_measurement_input_refs(
    root: Path, before_context: JsonObject, expected: JsonObject, snapshot: JsonObject,
    capture: JsonObject | None, links: list[JsonObject], provenance: list[JsonObject],
    binaries: Mapping[str, JsonObject],
) -> dict[str, JsonObject]:
    """Return only references authenticated by the existing context validator.

    The caller must obtain capture, link and provenance references through the
    existing catalog validators before calling this function.
    """
    require((capture is not None) == (expected.get("catalogCondition") is not None),
            "measurement catalog capture role differs")
    require(capture is not None or not links and not provenance,
            "unexpected measurement input role")
    execution_inputs = planned_execution_inputs(root, expected, binaries)
    validated = validate_context_input_refs(before_context, expected, snapshot, capture,
                                             links, provenance, execution_inputs)
    refs = _required_context_input_refs(expected, snapshot, capture, links, provenance,
                                        execution_inputs)
    paths = {_text(ref["path"], "input reference path") for ref in refs}
    return {path: {"path": path, "sha256": validated[path]["sha256"]} for path in sorted(paths)}


def require_execution_binary_role(row: JsonObject, binary: JsonObject) -> None:
    require(row.get("executable") == binary.get("path")
            and row.get("executable_sha256") == binary.get("sha256"),
            "execution binary differs from role proof")


def execution_binary_refs(bundle: Bundle, expected: JsonObject,
                          validated_builds: Mapping[str, JsonObject] | None) -> dict[str, JsonObject]:
    arms = _object(expected["arms"], "expected arms")
    baseline = baseline_arm_id(expected)
    binaries = {role: _object(bundle.json(_object(arms[arm], "expected arm")
                                         ["build_artifact_ref"]).get("frozenBinary"), "execution frozen binary")
                for role, arm in (("baseline", baseline), ("candidate", "candidate"))}
    contract = expected.get("referenceParent")
    if not isinstance(contract, dict) or contract.get("referenceKind") != "accepted-history-candidate":
        return binaries
    if validated_builds is None or "baseline" not in validated_builds:
        raise ValueError("verified reference parent build is missing")
    build = validated_builds["baseline"]
    arm = _object(arms[baseline], "expected baseline")
    wrapper_ref = _object(arm.get("build_artifact_ref"), "reference wrapper")
    require(wrapper_ref == contract.get("wrapperRef")
            and wrapper_ref.get("sha256") == build.get("buildArtifactSha256"),
            "reference wrapper validation differs")
    wrapper = bundle.json(wrapper_ref)
    manifest_ref = _object(wrapper.get("sourceBundleManifest"), "historical manifest")
    manifest = bundle.json(manifest_ref)
    require(manifest.get("schema") == "krr-html-performance-release-bundle-v1"
            and manifest.get("head") == build.get("head"), "historical manifest identity differs")
    frozen = binaries["baseline"]
    require(frozen.get("sha256") == build.get("binarySha256") == arm.get("binary_sha256"),
            "reference binary identity differs")
    rows = [row for row in _objects(manifest.get("files"), "historical manifest files")
            if row.get("originalPath") == frozen.get("path") and row.get("sha256") == frozen.get("sha256")]
    require(len(rows) == 1, "historical binary transport differs")
    relative = _text(rows[0].get("relativePath"), "historical binary transport path")
    relative_path = PurePosixPath(relative)
    require(bool(relative_path.parts) and not relative_path.is_absolute() and relative_path.as_posix() == relative
            and all(part not in {".", ".."} for part in relative_path.parts),
            "historical binary transport path differs")
    _, manifest_path = _logical_absolute_path(manifest_ref.get("path"), "historical manifest path")
    require(manifest_path.name == "bundle.json", "historical manifest path differs")
    recorded = {"path": str(manifest_path.parent.joinpath(*relative_path.parts)), "sha256": frozen["sha256"]}
    bundle.file(frozen)
    bundle.file(recorded)
    binaries["baseline"] = recorded
    return binaries


def _integer(value: object, label: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{label} must be an integer")
    return value


def _numeric(value: object, label: str) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    return value


def _fixture_expected_state(fixture: Path) -> object:
    matches = re.findall(
        r'<script type="application/json" id="expected-state">([^<]*)</script>',
        fixture.read_text(encoding="utf-8"),
    )
    require(len(matches) == 1, "catalog fixture must contain one expected-state JSON script")
    return json.loads(matches[0])


def _same_json(left: object, right: object) -> bool:
    return json.dumps(left, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
                      allow_nan=False) == json.dumps(
        right, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    )


def catalog_operation_stream(operation_kind: str, evidence_value: object,
                             expected_state: object | None = None,
                             report_state: object | None = None) -> list[JsonObject]:
    evidence = _object(evidence_value, "typed operation evidence")
    steps = _objects(evidence.get("steps", []), "catalog operation step")
    if operation_kind in {"load-and-observe-target", "script-load-observe",
                          "input-state-empty", "input-state-disabled-observe"}:
        require(not steps, "catalog observation/reopen operation unexpectedly dispatched inputs")
        expected_delivery = {
            "load-and-observe-target": "session-load",
            "script-load-observe": "session-load",
        }.get(operation_kind)
        if expected_delivery is not None:
            require(evidence.get("delivery") == expected_delivery,
                    "catalog observation operation delivery evidence differs")
        return []
    if operation_kind == "script-await-mutation-delivery":
        state = _object(evidence.get("state"), "mutation operation state")
        require("before" in state and "after" in state,
                "mutation operation state transition is missing")
        if evidence.get("delivery") == "mutation-observer":
            require(not _same_json(state["before"], state["after"]),
                    "mutation observer did not prove a state transition")
            return []
        require(evidence.get("delivery") == "session-load-settled"
                and set(state) == {"before", "after"}
                and set(evidence) == {
                    "schema", "kind", "status", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery",
                }
                and evidence.get("status") == "observed"
                and len(steps) == 1
                and set(steps[0]) == {"kind", "stateBefore", "stateAfter"}
                and steps[0].get("kind") == "session-load-settled-observer-state"
                and expected_state is not None and report_state is not None
                and all(_same_json(value, expected_state) for value in (
                    state["before"], state["after"], steps[0]["stateBefore"],
                    steps[0]["stateAfter"], report_state,
                )),
                "settled mutation evidence differs from its fixture callback log")
        return []
    if operation_kind in {"input-state-selected", "input-state-expanded", "navigation-pointer-click"}:
        stream = _catalog_pointer_stream(steps)
        if operation_kind == "navigation-pointer-click":
            require(isinstance(evidence.get("hashBefore"), str)
                    and isinstance(evidence.get("hashAfter"), str)
                    and evidence.get("hashAfter") != evidence.get("hashBefore")
                    and evidence.get("destination") == evidence.get("hashAfter"),
                    "navigation operation lacks its actual requested hash transition")
        return stream
    if operation_kind == "input-state-filled-save":
        require(len(steps) == 7 and [step.get("kind") for step in steps] ==
                ["move", "down", "up", "text", "move", "down", "up"],
                "filled/save operation evidence is missing its exact pointer/text sequence")
        stream = _catalog_pointer_stream(steps[:3])
        text_step = steps[3]
        require(set(text_step) == {"kind", "targetNodeId", "text", "before", "after"}
                and text_step.get("text") == "架空の名称"
                and _integer(text_step.get("targetNodeId"), "text target node") > 0
                and text_step.get("targetNodeId") == steps[0].get("targetNodeId")
                and text_step.get("after") == text_step.get("text")
                and isinstance(text_step.get("before"), str),
                "filled/save text evidence differs")
        stream.append({"kind": "text", "text": text_step["text"]})
        stream.extend(_catalog_pointer_stream(steps[4:]))
        return stream
    if operation_kind == "input-state-focus-visible-tab":
        require(len(steps) == 1 and steps[0].get("kind") == "key" and steps[0].get("key") == "Tab"
                and _integer(steps[0].get("targetNodeId"), "Tab target node") > 0,
                "focus-visible operation is missing its actual Tab key step")
        return [{"kind": "key_down", "key": "Tab"}]
    if operation_kind == "navigation-all-pointer-clicks":
        require(evidence.get("delivery") == "session-load"
                and set(evidence) == {
                    "schema", "kind", "status", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery", "links",
                }
                and evidence.get("status") == "observed"
                and evidence.get("sourceSha256Before") == evidence.get("sourceSha256After"),
                "all-TOC operation evidence is not bound to one fresh fixture source")
        links = _objects(evidence.get("links"), "catalog TOC link")
        require(len(links) == 3 and [link.get("destination") for link in links] == ["#a", "#b", "#c"],
                "all-TOC evidence does not contain the exact ordered three links")
        traces = [_objects(link.get("pointerTrace"), "TOC pointer trace") for link in links]
        require(all(len(trace) == 3 for trace in traces),
                "TOC link is missing its nested actual pointer trace")
        flattened = [step for trace in traces for step in trace]
        require(steps == flattened, "all-TOC top-level steps differ from nested link traces")
        previous_hash: str | None = None
        capture_paths: set[str] = set()
        target_node_ids: set[int] = set()
        for link_index, (destination, link) in enumerate(zip(("#a", "#b", "#c"), links, strict=True)):
            require(set(link) == {
                "destination", "hash", "scrollY", "capturePath", "captureSha256",
                "targetVisible", "svgPath", "svgSha256", "hashBefore", "hashAfter",
                "frame", "pointerTrace", "sourceSha256",
            }, "all-TOC link evidence fields differ")
            hash_before = _text(link.get("hashBefore"), "TOC hash before")
            require(link.get("hash") == destination and link.get("hashAfter") == destination
                    and hash_before != destination
                    and (previous_hash is None or hash_before == previous_hash),
                    "all-TOC actual hash transition differs")
            previous_hash = destination
            require(link.get("targetVisible") is True
                    and link.get("sourceSha256") == evidence.get("sourceSha256Before"),
                    "all-TOC target visibility or fixture source differs")
            capture_path = _text(link.get("capturePath"), "TOC capture path")
            require(Path(capture_path).is_absolute(), "all-TOC capture path must be absolute")
            capture_paths.add(capture_path)
            digest(link.get("captureSha256"))
            scroll_y = _numeric(link.get("scrollY"), "TOC captured scroll")
            frame = _object(link.get("frame"), "TOC captured frame")
            require(set(frame) == {"generation", "contentHeight", "scrollY"}
                    and _integer(frame.get("generation"), "TOC frame generation") > 0
                    and math.isfinite(_numeric(frame.get("contentHeight"), "TOC content height"))
                    and _numeric(frame.get("contentHeight"), "TOC content height") > 0
                    and math.isfinite(scroll_y) and scroll_y >= 0
                    and _numeric(frame.get("scrollY"), "TOC frame scroll") == scroll_y,
                    "all-TOC actual frame/scroll evidence differs")
            for metadata_path, metadata_sha in ((link.get("svgPath"), link.get("svgSha256")),):
                require(Path(_text(metadata_path, "TOC SVG path")).is_absolute(),
                        "all-TOC SVG path must be absolute")
                digest(metadata_sha)
            first_pointer = traces[link_index][0]
            target_node = _integer(first_pointer.get("targetNodeId"), "TOC target node")
            require(target_node not in target_node_ids, "TOC links reused an actual target node")
            target_node_ids.add(target_node)
            final_pointer = traces[link_index][2]
            final_frame = _object(final_pointer.get("postFrame"), "final TOC pointer frame")
            require(final_frame.get("generation") == frame.get("generation")
                    and final_frame.get("scrollY") == frame.get("scrollY"),
                    "TOC screenshot frame differs from the actual final pointer frame")
        require(len(capture_paths) == 3, "all-TOC links must have separate capture files")
        return _catalog_pointer_stream(steps)
    if operation_kind == "script-await-timer-microtask":
        if evidence.get("delivery") == "session-load-settled":
            state = _object(evidence.get("state"), "settled timer operation state")
            settled_steps = _objects(evidence.get("steps", []), "settled timer operation step")
            require(
                set(evidence) == {
                    "schema", "kind", "status", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery",
                }
                and evidence.get("status") == "observed"
                and set(state) == {"before", "after"}
                and len(settled_steps) == 1
                and set(settled_steps[0]) == {
                    "kind", "elapsedMs", "timerRan", "microtaskCheckpoint",
                    "stateBefore", "stateAfter", "fixtureStateVerified",
                }
                and settled_steps[0].get("kind") == "host-time-advance"
                and settled_steps[0].get("elapsedMs") == 0
                and settled_steps[0].get("timerRan") is False
                and settled_steps[0].get("microtaskCheckpoint") == "runtime-advance-timers"
                and settled_steps[0].get("fixtureStateVerified") is True
                and expected_state is not None and report_state is not None
                and _same_json(state["before"], state["after"])
                and _same_json(state["before"], settled_steps[0]["stateBefore"])
                and _same_json(state["after"], settled_steps[0]["stateAfter"]),
                "settled timer operation lacks a verified load-settled state",
            )
            require(
                all(_same_json(value, expected_state) for value in (
                    state["before"], state["after"],
                    settled_steps[0]["stateBefore"], settled_steps[0]["stateAfter"],
                    report_state,
                )),
                "settled timer operation differs from its fixture state",
            )
            return []
        require(len(steps) == 1 and set(steps[0]) == {
                    "kind", "elapsedMs", "timerRan", "microtaskCheckpoint"
                } and steps[0].get("kind") == "host-time-advance"
                and steps[0].get("timerRan") is True
                and steps[0].get("microtaskCheckpoint") == "runtime-advance-timers",
                "timer operation has no observed timer dispatch")
        elapsed = _numeric(steps[0].get("elapsedMs"), "timer elapsed milliseconds")
        require(math.isfinite(elapsed) and 0 <= elapsed <= 60_000,
                "timer operation elapsed time is outside the supported range")
        return [{"kind": "advance_time", "elapsed_ms": elapsed}]
    if operation_kind == "script-real-events":
        require(evidence.get("delivery") == "session-load"
                and set(evidence) == {
                    "schema", "kind", "status", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery", "events",
                }
                and evidence.get("status") == "observed"
                and evidence.get("sourceSha256Before") == evidence.get("sourceSha256After"),
                "script event operation does not prove one fresh fixture load")
        require(len(steps) == 9
                and [step.get("kind") for step in steps] == [
                    "move", "down", "up", "text", "key_down", "key_up", "move", "down", "up",
                ], "script event operation steps differ from the actual dispatch order")
        events = _objects(evidence.get("events"), "script event")
        event_types = ["focus", "input", "keydown", "keyup", "click", "submit"]
        require(len(events) == len(event_types)
                and [event.get("type") for event in events] == event_types,
                "script event evidence omits or reorders a dispatched event")
        script_first_pointer = _catalog_pointer_stream(steps[:3])
        script_last_pointer = _catalog_pointer_stream(steps[6:])
        field_id = _integer(steps[0].get("targetNodeId"), "script event field node")
        text_step, down_step, up_step = steps[3:6]
        require(set(text_step) == {"kind", "targetNodeId", "text", "stateBefore", "stateAfter"}
                and text_step.get("text") == "入力"
                and text_step.get("targetNodeId") == field_id
                and set(down_step) == {"kind", "targetNodeId", "key", "stateBefore", "stateAfter"}
                and down_step.get("targetNodeId") == field_id and down_step.get("key") == "Shift"
                and set(up_step) == {"kind", "targetNodeId", "key", "stateBefore", "stateAfter"}
                and up_step.get("targetNodeId") == field_id and up_step.get("key") == "Shift",
                "script event typed input steps differ")
        expected_fields = {
            "focus": {"type", "targetNodeId", "focused", "stateBefore", "stateAfter"},
            "input": {"type", "targetNodeId", "stateBefore", "stateAfter"},
            "keydown": {"type", "targetNodeId", "key", "stateBefore", "stateAfter"},
            "keyup": {"type", "targetNodeId", "key", "stateBefore", "stateAfter"},
            "click": {"type", "targetNodeId", "stateBefore", "stateAfter"},
            "submit": {"type", "targetNodeId", "stateBefore", "stateAfter"},
        }
        require(all(set(event) == expected_fields.get(_text(event.get("type"), "script event type"))
                    and _integer(event.get("targetNodeId"), "script event target") > 0
                    and "stateBefore" in event and "stateAfter" in event
                    for event in events), "script event fields or observed states differ")
        require(events[0].get("focused") is True
                and events[0].get("targetNodeId") == field_id
                and events[1].get("targetNodeId") == field_id
                and events[2].get("targetNodeId") == field_id
                and events[3].get("targetNodeId") == field_id
                and events[2].get("key") == down_step.get("key")
                and events[3].get("key") == up_step.get("key")
                and events[4].get("targetNodeId") == _integer(steps[6].get("targetNodeId"), "submit button node")
                and _same_json(events[4].get("stateBefore"), events[5].get("stateBefore"))
                and _same_json(events[4].get("stateAfter"), events[5].get("stateAfter")),
                "script event records do not bind to the observed dispatch steps")
        for event, step in ((events[1], text_step), (events[2], down_step), (events[3], up_step)):
            require(_same_json(event.get("stateBefore"), step.get("stateBefore"))
                    and _same_json(event.get("stateAfter"), step.get("stateAfter")),
                    "script event state differs from its actual typed step")
        state = _object(evidence.get("state"), "script event operation state")
        require(set(state) == {"before", "after"}
                and _same_json(state["before"], events[0].get("stateBefore"))
                and _same_json(state["after"], events[5].get("stateAfter"))
                and expected_state is not None and report_state is not None
                and all(_same_json(left, right) for left, right in (
                    (events[0].get("stateAfter"), events[1].get("stateBefore")),
                    (events[1].get("stateBefore"), text_step.get("stateBefore")),
                    (events[1].get("stateAfter"), text_step.get("stateAfter")),
                    (events[1].get("stateAfter"), events[2].get("stateBefore")),
                    (events[2].get("stateBefore"), down_step.get("stateBefore")),
                    (events[2].get("stateAfter"), down_step.get("stateAfter")),
                    (events[2].get("stateAfter"), events[3].get("stateBefore")),
                    (events[3].get("stateBefore"), up_step.get("stateBefore")),
                    (events[3].get("stateAfter"), up_step.get("stateAfter")),
                    (events[3].get("stateAfter"), events[4].get("stateBefore")),
                    (events[4].get("stateBefore"), events[5].get("stateBefore")),
                    (events[4].get("stateAfter"), events[5].get("stateAfter")),
                    (state["after"], report_state),
                    (report_state, expected_state),
                    (events[5].get("stateAfter"), expected_state),
                )),
                "script event operation state differs from its observed event trace")
        text_input: JsonObject = {"kind": "text", "text": text_step["text"]}
        key_down = {"kind": "key_down", "key": down_step["key"]}
        key_up = {"kind": "key_up", "key": up_step["key"]}
        return [*script_first_pointer, text_input, key_down, key_up, *script_last_pointer]
    if operation_kind == "script-await-resource-font":
        state = _object(evidence.get("state"), "resource/font operation state")
        settled_steps = _objects(evidence.get("steps", []), "resource/font operation step")
        require(
            evidence.get("delivery") == "session-load-settled"
            and set(evidence) == {
                "schema", "kind", "status", "sourceSha256Before", "sourceSha256After",
                "preFrame", "postFrame", "target", "state", "steps", "delivery",
            }
            and evidence.get("status") == "observed"
            and set(state) == {"before", "after"}
            and len(settled_steps) == 1
            and set(settled_steps[0]) == {"kind", "stateBefore", "stateAfter"}
            and settled_steps[0].get("kind") == "session-load-settled-resource-state"
            and _same_json(state["before"], state["after"])
            and _same_json(state["before"], settled_steps[0]["stateBefore"])
            and _same_json(state["after"], settled_steps[0]["stateAfter"])
            and expected_state is not None and report_state is not None
            and _same_json(state["before"], expected_state)
            and _same_json(report_state, expected_state),
            "resource/font operation lacks a verified settled resource state",
        )
        return []
    raise ValueError(f"unsupported finite catalog operation: {operation_kind}")


def _catalog_pointer_stream(steps: list[JsonObject]) -> list[JsonObject]:
    require(len(steps) >= 3 and len(steps) % 3 == 0,
            "catalog pointer trace must contain complete move/down/up groups")
    stream: list[JsonObject] = []
    for index in range(0, len(steps), 3):
        group = steps[index:index + 3]
        move, down, up = group
        require([step.get("kind") for step in group] == ["move", "down", "up"],
                "catalog pointer trace order differs")
        for step in group:
            expected_fields = {"kind", "targetNodeId", "x", "y"}
            if "preFrame" in step or "postFrame" in step:
                require({"preFrame", "postFrame"} <= set(step)
                        and set(step) <= expected_fields | {"preFrame", "postFrame"},
                        "catalog pointer frame fields differ")
                for frame_key in ("preFrame", "postFrame"):
                    frame = _object(step.get(frame_key), "pointer frame")
                    require(set(frame) in ({"generation", "scrollY"},
                                           {"generation", "contentHeight", "scrollY"})
                            and _integer(frame.get("generation"), "pointer generation") > 0
                            and math.isfinite(_numeric(frame.get("scrollY"), "pointer scroll"))
                            and _numeric(frame.get("scrollY"), "pointer scroll") >= 0
                            and ("contentHeight" not in frame or (
                                math.isfinite(_numeric(frame.get("contentHeight"), "pointer content height"))
                                and _numeric(frame.get("contentHeight"), "pointer content height") >= 0)),
                            "catalog pointer frame identity is incomplete")
            else:
                require(set(step) == expected_fields, "catalog pointer dispatch fields differ")
            require(_integer(step.get("targetNodeId"), "pointer target node") > 0,
                    "catalog pointer trace has no target node")
            for coordinate in ("x", "y"):
                number = _numeric(step.get(coordinate), "pointer " + coordinate)
                require(math.isfinite(number), "catalog pointer coordinate is not finite")
            require(move["targetNodeId"] == down["targetNodeId"] == up["targetNodeId"]
                and move["x"] == down["x"] == up["x"]
                and move["y"] == down["y"] == up["y"],
                "catalog pointer trace target or coordinates differ")
        stream.extend([
            {"kind": "pointer_move", "x": move["x"], "y": move["y"]},
            {"kind": "pointer_down", "x": down["x"], "y": down["y"], "button": 0},
            {"kind": "pointer_up", "x": up["x"], "y": up["y"], "button": 0},
        ])
    return stream


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


def require_no_git_replacements(root: Path, *, git_query: GitQuery | None = None) -> None:
    """置換参照でGitの解釈が変わるため、証跡の検証では使用を認めない。"""
    require(os.environ.get("GIT_REPLACE_REF_BASE") in (None, "refs/replace/"),
            "Git replacement ref base is not supported")
    refs = git(root, "for-each-ref", "--format=%(refname)", "refs/replace/", git_query=git_query)
    require(not refs.strip(), "Git replacement refs invalidate performance evidence")


class Bundle:
    # 絶対パスを含む元証跡は書き換えず、輸送索引だけでファイルを再配置する。
    def __init__(self, directory: Path) -> None:
        self.directory = directory.resolve()
        self.manifest = read_json(self.directory / "bundle.json")
        require(self.manifest["schema"] == "krr-html-performance-release-bundle-v1", "unsupported transport schema")
        self.files: dict[str, tuple[Path, str]] = {}
        self.relative_files: dict[str, tuple[Path, str, str]] = {}
        used = set()
        for row in _objects(self.manifest["files"], "transport files"):
            original = _text(row["originalPath"], "original path")
            relative = Path(_text(row["relativePath"], "transport path"))
            path = (self.directory / relative).resolve()
            try:
                _logical_absolute_path(original, "original path")
            except ValueError as error:
                raise ValueError("duplicate/nonabsolute original path") from error
            require(original not in self.files, "duplicate/nonabsolute original path")
            require(not relative.is_absolute() and path.is_relative_to(self.directory) and path not in used, "invalid transport path")
            checksum = _text(row["sha256"], "transport SHA256")
            digest(checksum)
            require(path.is_file() and sha256(path) == checksum, "transport file hash mismatch")
            self.files[original] = (path, checksum)
            self.relative_files[relative.as_posix()] = (path, checksum, original)
            used.add(path)

    def file(self, reference: object) -> Path:
        ref = _object(reference, "file reference")
        digest(ref["sha256"])
        path, recorded = self.files[_text(ref["path"], "file reference path")]
        require(recorded == ref["sha256"] and sha256(path) == recorded, "original pin mismatch")
        return path

    def json(self, ref: object) -> JsonObject:
        return read_json(self.file(ref))

    def relative_reference(self, relative_path: str, checksum: str) -> JsonObject:
        digest(checksum)
        row = self.relative_files.get(relative_path)
        if row is None:
            raise ValueError("bundle-relative file pin mismatch")
        require(row[1] == checksum and sha256(row[0]) == checksum,
                "bundle-relative file pin mismatch")
        return {"path": row[2], "sha256": checksum}


class HistoricalBuildReader:
    """Resolve only references declared by the pinned historical bundle."""
    def __init__(self, reader: FileReader, manifest_ref: object, selection: reference_domain.ActiveReference) -> None:
        self.reader = reader
        manifest_path = reader.file(manifest_ref)
        require(sha256(manifest_path) == selection.bundle_manifest_sha256,
                "historical manifest pin differs")
        manifest = read_json(manifest_path)
        require(manifest.get("schema") == "krr-html-performance-release-bundle-v1"
                and manifest.get("head") == selection.source_head,
                "historical manifest identity differs")
        self.records: dict[str, str] = {}
        self.used: dict[str, str] = {}
        relative_paths: set[str] = set()
        for row in _objects(manifest.get("files"), "historical manifest files"):
            path = _text(row.get("originalPath"), "historical original path")
            _logical_absolute_path(path, "historical original path")
            relative = Path(_text(row.get("relativePath"), "historical transport path"))
            require(not relative.is_absolute() and ".." not in relative.parts
                    and relative.as_posix() not in relative_paths,
                    "historical transport path differs")
            relative_paths.add(relative.as_posix())
            checksum = _text(row.get("sha256"), "historical original SHA")
            digest(checksum)
            require(path not in self.records, "historical original path is duplicated")
            self.records[path] = checksum

    def file(self, reference: object) -> Path:
        pin = _object(reference, "historical file reference")
        path = _text(pin.get("path"), "historical file path")
        checksum = _text(pin.get("sha256"), "historical file SHA")
        require(self.records.get(path) == checksum, "historical file is not in the original bundle")
        physical = self.reader.file(pin)
        self.used[path] = checksum
        return physical

    def json(self, reference: object) -> JsonObject:
        return read_json(self.file(reference))


class FilePin(TypedDict):
    path: str
    sha256: str


class CatalogBinaryRefs(NamedTuple):
    proof_ref: FilePin
    build_report_ref: FilePin
    binary_ref: FilePin
    current_build_report_ref: FilePin


def _logical_absolute_path(value: object, label: str) -> tuple[str, PurePath]:
    text = _text(value, label)
    windows = PureWindowsPath(text)
    if windows.is_absolute():
        require(str(windows) == text and ".." not in windows.parts,
                label + " must be a canonical absolute path")
        return text, windows
    posix = PurePosixPath(text)
    require(posix.is_absolute() and not text.startswith("//")
            and posix.as_posix() == text and ".." not in posix.parts,
            label + " must be a canonical absolute path")
    return text, posix


def _catalog_source_target(catalog_path: str, windows: bool) -> PurePath:
    catalog: PurePath = PureWindowsPath(catalog_path) if windows else PurePosixPath(catalog_path)
    parts = catalog.parts
    expected = (tuple(part.casefold() for part in CATALOG_SOURCE_SUFFIX)
                if windows else CATALOG_SOURCE_SUFFIX)
    actual = (tuple(part.casefold() for part in parts[-len(CATALOG_SOURCE_SUFFIX):])
              if windows else parts[-len(CATALOG_SOURCE_SUFFIX):])
    require(actual == expected, "catalog path cannot identify its source repository root")
    prefix = parts[:-len(CATALOG_SOURCE_SUFFIX)]
    target = PureWindowsPath(*prefix, "target") if windows else PurePosixPath(*prefix, "target")
    return target


def validate_catalog_binary_proof(
    manifest: Mapping[str, object], read_file: Callable[[FilePin], Path],
) -> CatalogBinaryRefs:
    require(manifest.get("schema") == "krr-html84-catalog-captures-v2",
            "catalog capture manifest schema differs")
    binary_path, binary = _logical_absolute_path(
        manifest.get("binaryPath"), "catalog frozen binary path",
    )
    binary_sha = _text(manifest.get("binarySha256"), "catalog frozen binary SHA")
    digest(binary_sha)
    catalog_path, catalog = _logical_absolute_path(
        manifest.get("catalogPath"), "catalog feature catalog path",
    )
    require(isinstance(binary, PureWindowsPath) == isinstance(catalog, PureWindowsPath),
            "catalog binary and feature catalog path platforms differ")
    source_target = _catalog_source_target(catalog_path, isinstance(catalog, PureWindowsPath))
    require(not binary.is_relative_to(source_target),
            "catalog frozen binary must be canonical and outside target")
    proof_ref = _object(manifest.get("binaryProof"), "catalog binary proof reference")
    require(set(proof_ref) == {"path", "sha256"}, "catalog binary proof reference fields differ")
    proof_path, _ = _logical_absolute_path(proof_ref["path"], "catalog binary proof path")
    proof_sha = _text(proof_ref["sha256"], "catalog binary proof SHA")
    digest(proof_sha)
    normalized_proof_ref: FilePin = {"path": proof_path, "sha256": proof_sha}
    proof = read_json(read_file(normalized_proof_ref))
    require(set(proof) == {"schema", "buildReport", "buildReportSha256", "source", "path", "sha256"}
            and proof.get("schema") == "krr-html84-capture-binary-v1",
            "catalog binary proof schema/fields differ")
    report_path, _ = _logical_absolute_path(proof["buildReport"], "catalog Cargo build report path")
    report_sha = _text(proof["buildReportSha256"], "catalog Cargo build report SHA")
    _logical_absolute_path(proof["source"], "catalog Cargo selected executable")
    proven_path, _ = _logical_absolute_path(proof["path"], "catalog frozen binary proof path")
    proven_sha = _text(proof["sha256"], "catalog frozen binary proof SHA")
    digest(report_sha)
    digest(proven_sha)
    require(proven_path == binary_path and proven_sha == binary_sha,
            "catalog binary proof does not bind the manifest binary")
    report_ref: FilePin = {"path": report_path, "sha256": report_sha}
    binary_ref: FilePin = {"path": binary_path, "sha256": binary_sha}
    read_file(report_ref)
    read_file(binary_ref)
    current = _object(manifest.get("currentBuild"), "catalog current build proof")
    require(set(current) == {"path", "sha256", "source", "binarySha256"},
            "catalog current build proof fields differ")
    current_path, _ = _logical_absolute_path(current["path"], "catalog current build report path")
    current_sha = _text(current["sha256"], "catalog current build report SHA")
    _logical_absolute_path(current["source"], "catalog current selected executable")
    current_binary_sha = _text(current["binarySha256"], "catalog current binary SHA")
    digest(current_sha)
    digest(current_binary_sha)
    require(current_binary_sha == binary_sha,
            "catalog current build proof does not bind the captured binary")
    current_build_report_ref: FilePin = {"path": current_path, "sha256": current_sha}
    read_file(current_build_report_ref)
    return CatalogBinaryRefs(normalized_proof_ref, report_ref, binary_ref,
                             current_build_report_ref)


def validate_catalog_binary_closure(
    root: Path, bundle: Bundle, manifest_ref: JsonObject, refs: CatalogBinaryRefs,
) -> JsonObject:
    closure = [manifest_ref, refs.proof_ref, refs.build_report_ref,
               refs.binary_ref, refs.current_build_report_ref]
    transported: dict[str, JsonObject] = {}
    for value in closure:
        pin = _object(value, "catalog binary closure file reference")
        logical_path = _text(pin.get("path"), "catalog binary logical path")
        checksum = _text(pin.get("sha256"), "catalog binary logical SHA")
        digest(checksum)
        payload = bundle.file(pin)
        require(payload.is_file() and payload.resolve() == payload and not payload.is_symlink(),
                "catalog binary closure payload is not a canonical regular file")
        row: JsonObject = {"path": logical_path, "sha256": checksum, "payloadPath": str(payload)}
        prior = transported.get(logical_path)
        require(prior is None or prior == row,
                "catalog binary closure logical path has conflicting payloads")
        transported[logical_path] = row
    require(4 <= len(transported) <= 5,
            "catalog binary closure must contain four or five unique files")
    request: JsonObject = {
        "schema": "krr-html84-binary-closure-request-v1",
        "manifest": {"path": manifest_ref["path"], "sha256": manifest_ref["sha256"]},
        "files": list(transported.values()),
    }
    encoded_request = json.dumps(request, sort_keys=True, separators=(",", ":")).encode()
    request_sha = hashlib.sha256(encoded_request).hexdigest()
    with tempfile.TemporaryDirectory(prefix="krr-html-binary-closure-") as directory:
        request_path = Path(directory).resolve(strict=True) / "request.json"
        with request_path.open("xb") as stream:
            stream.write(encoded_request)
        argv = ["bun", "run", "scripts/html/fidelity-acceptance-cli.ts",
                "catalog-stage90-validate-binary-closure", str(request_path)]
        completed = subprocess.run(argv, cwd=root, check=False, capture_output=True)
    require(completed.returncode == 0,
            "strict TypeScript catalog binary closure validation failed")
    result = _object(json.loads(completed.stdout, object_pairs_hook=unique_object),
                     "strict TypeScript catalog binary closure result")
    require(set(result) == {"schema", "requestSha256", "manifest"}
            and result.get("schema") == "krr-html84-binary-closure-result-v1"
            and result.get("requestSha256") == request_sha
            and result.get("manifest") == request["manifest"],
            "strict TypeScript catalog binary closure binding differs")
    return result


def _canonical_build_file(value: object, label: str) -> Path:
    path = Path(_text(value, label))
    require(path.is_absolute() and path.resolve() == path and not path.is_symlink()
            and path.is_file(), label + " must be a canonical regular file")
    return path


def _canonical_build_path(value: object, label: str) -> Path:
    path = Path(_text(value, label))
    require(path.is_absolute() and str(path) == value, label + " must be a canonical absolute path")
    return path


def _validate_build_environment(proof: JsonObject, root: Path, harness: Path, cargo: Path,
                                environment: Mapping[str, str]) -> list[Path]:
    recorded_sha = _text(proof.get("buildEnvironmentSha256"), "build environment SHA")
    digest(recorded_sha)
    capacity = _cargo_capacity_guard()
    capacity.validate_build_selection(harness, environment)
    capacity.validate_build_executable(root, [str(cargo), "build"], environment)
    return capacity.cargo_configuration_paths(harness, environment)


def selected_historical_lock(root: Path, selection: reference_domain.ActiveReference, *,
                             git_query: GitQuery | None = None) -> tuple[bytes, str]:
    if not isinstance(selection, reference_domain.ActiveReference):
        raise ValueError("unapproved historical reference")
    checksum = selection.build_pins["lockSha256"]
    raw = git(root, "show", selection.source_head + ":" + HARNESS_ROOT + "Cargo.lock",
              git_query=git_query).encode("utf8")
    require(hashlib.sha256(raw).hexdigest() == checksum, "historical lock pin differs")
    return raw, checksum


def _runtime_lock_packages(harness: Path, *, historical_lock: tuple[bytes, str] | None = None) -> dict[str, JsonObject]:
    from toml_loader import load_toml_parser

    raw, checksum = historical_lock if historical_lock is not None else ((harness / "Cargo.lock").read_bytes(), LOCK_SHA)
    require(hashlib.sha256(raw).hexdigest() == checksum, "common lock pin differs")
    lock = _object(load_toml_parser().loads(raw.decode("utf8")), "common Cargo lock")
    packages = [p for p in _objects(lock["package"], "locked packages")
                if p.get("name") == "katana-render-runtime"]
    baseline = [p for p in packages if p.get("source") == "registry+https://github.com/rust-lang/crates.io-index"
                and p.get("version") == "0.4.23"]
    candidate = [p for p in packages if "source" not in p]
    require(len(packages) == 2 and len(baseline) == 1 and len(candidate) == 1,
            "fixed runtime lock packages differ")
    _text(candidate[0]["version"], "locked candidate version")
    return {"baseline": baseline[0], "candidate": candidate[0]}


def expected_build(root: Path, harness: Path, arm: str, metadata: JsonObject,
                   environment: Mapping[str, str], *,
                   historical_lock: tuple[bytes, str] | None = None) -> ExpectedBuild:
    from toml_loader import load_toml_parser

    require(_integer(metadata["version"], "metadata schema version") == 1, "metadata schema differs")
    packages = _objects(metadata["packages"], "standalone Cargo packages")
    harness_packages = [p for p in packages if p.get("manifest_path") == str(harness / "Cargo.toml")
                        and p.get("name") == "krr-html-performance-harness"]
    require(len(harness_packages) == 1, "standalone manifest identity differs")
    selected_harness = harness_packages[0]
    harness_manifest = _object(load_toml_parser().loads((harness / "Cargo.toml").read_text()), "harness manifest")
    harness_package = _object(harness_manifest["package"], "harness package")
    require(selected_harness["source"] is None
            and selected_harness.get("version") == _text(harness_package["version"], "harness version"),
            "harness package version/origin differs")
    harness_id = _text(selected_harness["id"], "harness package id")
    require(metadata["workspace_members"] == [harness_id], "standalone workspace members differ")
    ids = [_text(p["id"], "metadata package id") for p in packages]
    require(len(set(ids)) == len(ids), "duplicate metadata package id")
    resolve = _object(metadata["resolve"], "Cargo resolve")
    require(resolve["root"] == harness_id, "Cargo resolve root differs")
    nodes = _objects(resolve["nodes"], "Cargo resolve nodes")
    roots = [node for node in nodes if node.get("id") == harness_id]
    require(len(roots) == 1 and roots[0].get("features") == [arm], "Cargo arm features differ")
    dependencies = _objects(roots[0]["deps"], "harness resolved dependencies")
    arms = [dep for dep in dependencies if dep.get("name") in {"baseline", "candidate"}]
    require(len(arms) == 1 and arms[0].get("name") == arm, "Cargo resolved arm differs")
    runtime_id = _text(arms[0]["pkg"], "runtime package id")
    runtime_packages = [p for p in packages if p.get("id") == runtime_id]
    require(len(runtime_packages) == 1, "resolved runtime package missing")
    runtime = dict(runtime_packages[0])
    runtime_nodes = [node for node in nodes if node.get("id") == runtime_id]
    require(len(runtime_nodes) == 1, "resolved runtime node missing")
    runtime["resolvedFeatures"] = [_text(f, "resolved runtime feature")
                                   for f in _array(runtime_nodes[0]["features"], "resolved runtime features")]
    locked = _runtime_lock_packages(harness, historical_lock=historical_lock)[arm]
    require(runtime.get("name") == "katana-render-runtime" and runtime.get("version") == locked["version"],
            "runtime name/version differs from fixed lock")
    manifest = _canonical_build_file(runtime["manifest_path"], "runtime manifest")
    if arm == "baseline":
        cargo_home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
        require(runtime.get("source") == locked["source"]
                and manifest.is_relative_to(cargo_home / "registry/src")
                and manifest.parent.name == "katana-render-runtime-0.4.23", "baseline runtime origin differs")
    else:
        require(runtime["source"] is None
                and manifest == root / "crates/katana-render-runtime/Cargo.toml", "candidate runtime origin differs")
        manifest_value = _object(load_toml_parser().loads(manifest.read_text()), "candidate manifest")
        package = _object(manifest_value["package"], "candidate package")
        inherited = _object(package["version"], "candidate inherited version")
        workspace = _object(load_toml_parser().loads((root / "Cargo.toml").read_text()), "root manifest")
        workspace_package = _object(_object(workspace["workspace"], "workspace")["package"], "workspace package")
        require(inherited.get("workspace") is True and workspace_package.get("version") == locked["version"],
                "candidate version differs from fixed lock")
    for package, name, kind, source in ((selected_harness, "krr-html-performance-harness", ["bin"], harness / "src/main.rs"),
                                        (runtime, "katana_render_runtime", ["lib"], manifest.parent / "src/lib.rs")):
        targets = [t for t in _objects(package["targets"], "Cargo package targets") if t.get("name") == name]
        require(len(targets) == 1 and targets[0].get("kind") == kind
                and targets[0].get("crate_types") == kind and targets[0].get("src_path") == str(source),
                "Cargo package target differs")
        _canonical_build_file(str(source), "Cargo target source")
    return ExpectedBuild(selected_harness, runtime, arm)


def cargo_build_artifacts(raw: bytes, expected: ExpectedBuild, target: Path) -> tuple[Path, JsonObject]:
    events: list[JsonObject] = []
    terminal_seen = False
    for line in raw.decode("utf8").splitlines():
        require(not terminal_seen or not line.strip(), "nonterminal build-finished event")
        candidate = line.lstrip()
        if not candidate:
            continue
        starts_object = candidate.startswith("{")
        starts_array = candidate.startswith("[")
        starts_scalar = candidate[0] in '\"tfn-0123456789'
        if not (starts_object or starts_array or starts_scalar):
            continue
        try:
            decoded = json.loads(candidate, object_pairs_hook=unique_object)
        except json.JSONDecodeError:
            require(not starts_object, "malformed Cargo JSON object")
            continue
        event = _object(decoded, "Cargo JSON event")
        reason = _text(event["reason"], "Cargo event reason")
        events.append(event)
        terminal_seen = reason == "build-finished"
    finished = [e for e in events if e.get("reason") == "build-finished"]
    require(len(finished) == 1 and finished[0].get("success") is True
            and events[-1] is finished[0], "unique terminal successful build-finished required")
    selected: list[JsonObject] = []
    binary: Path | None = None
    for package, name in ((expected.runtime, "katana_render_runtime"),
                          (expected.harness, "krr-html-performance-harness")):
        rows = [e for e in events if e.get("reason") == "compiler-artifact"
                and _object(e.get("target"), "artifact target").get("name") == name]
        require(len(rows) == 1, "unique runtime/harness compiler-artifact required")
        artifact = rows[0]
        profile = _object(artifact["profile"], "artifact profile")
        actual_target = _object(artifact["target"], "artifact target")
        expected_targets = [t for t in _objects(package["targets"], "expected targets") if t.get("name") == name]
        require(len(expected_targets) == 1, "expected artifact target differs")
        require(artifact.get("package_id") == package["id"] and artifact.get("manifest_path") == package["manifest_path"]
                and all(actual_target.get(k) == expected_targets[0][k] for k in ("name", "kind", "crate_types", "src_path")),
                "artifact package/origin/target differs")
        require(profile.get("opt_level") == "3" and profile.get("test") is False
                and type(artifact.get("fresh")) is bool, "artifact is not release non-test")
        features = [_text(f, "artifact feature") for f in _array(artifact["features"], "artifact features")]
        if package is expected.runtime:
            require(sorted(features) == sorted(_text(f, "expected runtime feature")
                    for f in _array(package["resolvedFeatures"], "expected runtime features")),
                    "runtime artifact features differ")
        if package is expected.harness:
            require(features == [expected.arm], "harness artifact arm features differ")
            binary = _canonical_build_path(artifact["executable"], "artifact executable")
            require(binary == target / "release/krr-html-performance-harness"
                    and str(binary) in [_text(f, "artifact filename") for f in _array(artifact["filenames"], "artifact filenames")],
                    "artifact executable namespace differs")
        else:
            require(artifact["executable"] is None, "runtime library executable differs")
        selected.append(artifact)
    require(binary is not None, "artifact executable missing")
    return _canonical_build_path(str(binary), "artifact executable"), {
        "runtimeArtifact": selected[0], "binaryArtifact": selected[1], "buildFinished": finished[0]}


def _recorded_capacity_state(bundle: FileReader, capacity_input: JsonObject) -> tuple[Path, str]:
    state_ref = _object(capacity_input.get("stateRef"), "recorded prebuild capacity state")
    state_path = _canonical_build_path(state_ref.get("path"), "recorded capacity state path")
    state_sha = _text(state_ref.get("sha256"), "recorded capacity state SHA")
    bundle.file(state_ref)
    return state_path, state_sha


def _guard_python_from_proof(bundle: FileReader, proof: JsonObject, root: Path, argv: list[str],
                             raw_by_name: Mapping[str, Path]) -> Path:
    python = _object(proof.get("guardPython"), "guard Python pin")
    require(set(python) == {"launcher", "executable"}, "guard Python pin fields differ")
    launcher = _canonical_build_path(python.get("launcher"), "guard Python launcher")
    executable = _object(python.get("executable"), "guard Python executable pin")
    _canonical_build_path(executable.get("path"), "guard Python executable")
    payload = bundle.file(executable)
    require(os.access(payload, os.X_OK), "guard Python executable is not executable")
    lifecycle = bundle.json(proof.get("guardLifecycleRef"))
    identity = _object(lifecycle.get("identity"), "guard process identity")
    pid = _integer(identity.get("pid"), "guard process pid")
    require(pid > 0 and identity.get("sessionId") == pid and identity.get("processGroupId") == pid
            and lifecycle.get("schema") == "krr-performance-command-lifecycle-v1"
            and lifecycle.get("role") == "build" and lifecycle.get("cwd") == str(root)
            and lifecycle.get("argv") == argv and lifecycle.get("exitCode") == 0
            and lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
            and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None,
            "guard process lifecycle differs")
    require(bool(argv) and argv[0] == str(launcher), "guard Python launcher differs from process argv")
    raw = _objects(lifecycle.get("raw"), "guard lifecycle raw files")
    require(len(raw) == 2 and all(sha256(bundle.file(pin)) == sha256(raw_by_name[name])
            for pin, name in zip(raw, ("cargo.jsonl", "guard-cargo.stderr"), strict=True)),
            "guard process raw output differs")
    return launcher


def _validate_build_proof(bundle: FileReader, root: Path, reference: object, arm: str, *,
                           git_query: GitQuery | None = None,
                           reuse_context: object = None,
                           controller_reuse_context: object = None,
                           historical_reference: object = None,
                           fresh_source_root: Path | None = None) -> JsonObject:
    proof_path = bundle.file(reference)
    proof = read_json(proof_path)
    require(proof.get("schema") == "krr-html-performance-build-artifact-v3", "unsupported build artifact schema")
    require(proof.get("arm") == arm and proof.get("buildExit") == 0, "build artifact arm/exit differs")
    require(fresh_source_root is None or historical_reference is not None,
            "fresh historical build requires a selected reference")
    source_root = root if fresh_source_root is None else fresh_source_root
    harness = source_root / HARNESS_ROOT.rstrip("/")
    require(sha256(root / HARNESS_ROOT / "Cargo.lock") == LOCK_SHA,
            "build artifact lock differs")
    historical_lock: tuple[bytes, str] | None = None
    if historical_reference is not None:
        if not isinstance(historical_reference, reference_domain.ActiveReference):
            raise ValueError("unapproved historical reference")
        historical_lock = selected_historical_lock(root, historical_reference, git_query=git_query)
        lock_sha = historical_lock[1]
        require(proof.get("lockSha256") == lock_sha
                and (fresh_source_root is None or sha256(harness / "Cargo.lock") == lock_sha),
                "build artifact lock differs")
    else:
        require(proof.get("lockSha256") == LOCK_SHA and sha256(harness / "Cargo.lock") == LOCK_SHA,
                "build artifact lock differs")
    source_before_ref = _object(proof["sourceBeforeRef"], "build source-before reference")
    source_before = bundle.json(source_before_ref)
    if fresh_source_root is not None:
        from html_performance_source_materialization import verify_selected_source_proof
        if not isinstance(historical_reference, reference_domain.ActiveReference):
            raise ValueError("fresh historical reference is invalid")
        require(proof.get("referenceRole") == "fresh-history-source"
                and source_before.get("root") == str(fresh_source_root),
                "fresh historical build identity differs")
        controller_ref = _object(proof.get("controllerSourceBeforeRef"), "controller source reference")
        controller_pin = {"path": _text(controller_ref.get("path"), "controller source path"),
                          "sha256": _text(controller_ref.get("sha256"), "controller source SHA")}
        bundle.file(controller_ref)
        _source_for_validation(root, bundle.json(controller_ref), git_query=git_query,
                               reuse_context=controller_reuse_context)
        selected_proof = bundle.json(proof.get("selectedSourceProofRef"))
        verify_selected_source_proof(root, fresh_source_root, historical_reference, controller_pin,
                                     selected_proof, git_query=git_query)
    if historical_reference is None:
        current = _source_for_validation(root, source_before, git_query=git_query,
                                         reuse_context=reuse_context)
    else:
        require(reuse_context is None, "historical reference cannot replace source reuse")
        current = _historical_reference_source(root, source_before, historical_reference,
                                               git_query=git_query)
    snapshot_ref = _object(proof["sourceSnapshotRef"], "build source snapshot reference")
    snapshot = bundle.json(snapshot_ref)
    snapshot_records = records_map(snapshot["records"])
    snapshot_digest = hashlib.sha256(json.dumps(snapshot["records"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(snapshot_records == current and snapshot.get("digest") == snapshot_digest
            and sha256(bundle.file(snapshot_ref)) == _text(snapshot_ref["sha256"], "source snapshot pin"),
            "build source snapshot differs from current committed source")
    digest(_text(proof.get("buildEnvironmentSha256"), "build environment SHA"))
    tool_versions = _object(proof.get("toolVersions"), "build tool versions")
    actual_environment = dict(os.environ)
    actual_environment["CARGO_TARGET_DIR"] = str(root / "target")
    cargo_tool = resolve_rust_tool(root, "cargo", actual_environment)
    rustc_tool = resolve_rust_tool(root, "rustc", actual_environment)
    require(cargo_tool == resolve_rust_tool(root, "cargo", actual_environment, cwd=harness)
            and rustc_tool == resolve_rust_tool(root, "rustc", actual_environment, cwd=harness),
            "Rust toolchain selection differs between repository and harness")
    validate_rust_tool_pin(proof, "cargo", cargo_tool)
    validate_rust_tool_pin(proof, "rustc", rustc_tool)
    cargo_info = _object(proof.get("cargoExecutable"), "Cargo executable pin")
    cargo_launcher = cargo_tool.launcher
    rustc_launcher = rustc_tool.launcher
    cargo_version = subprocess.check_output([str(cargo_launcher), "-vV"], cwd=root, env=actual_environment, text=True)
    rustc_version = subprocess.check_output([str(rustc_launcher), "-vV"], cwd=root, env=actual_environment, text=True)
    require(tool_versions.get("cargo") == cargo_version and tool_versions.get("rustc") == rustc_version,
            "Cargo/Rustc selected toolchain differs from build receipt")
    config_pins = _objects(proof.get("cargoConfigPins"), "Cargo configuration pins")
    config_paths = [Path(_text(item.get("path"), "Cargo configuration path")) for item in config_pins]
    current_config_paths = _validate_build_environment(proof, root, harness, cargo_launcher, actual_environment)
    require(config_paths == current_config_paths, "Cargo configuration selection differs")
    for item, path in zip(config_pins, config_paths, strict=True):
        bundle.file(item)
        require(path.is_file() and not path.is_symlink() and sha256(path) == _text(item.get("sha256"), "Cargo configuration SHA"),
                "Cargo configuration differs from build receipt")
    raw_pins = _objects(proof.get("rawPins"), "build raw pins")
    raw_by_name = {Path(_text(pin["path"], "raw build path")).name: bundle.file(pin) for pin in raw_pins}
    require(len(raw_by_name) == len(raw_pins) and {"metadata.json", "cargo.jsonl", "guard-cargo.stderr"} <= set(raw_by_name),
            "build raw metadata/Cargo JSON pins absent")
    metadata = _object(json.loads(raw_by_name["metadata.json"].read_text(encoding="utf8"), object_pairs_hook=unique_object),
                       "Cargo metadata")
    build_expected = expected_build(source_root, harness, arm, metadata, actual_environment,
                                    historical_lock=historical_lock)
    require(build_expected.harness == _object(proof["expectedHarness"], "expected harness")
            and build_expected.runtime == _object(proof["expectedRuntime"], "expected runtime"),
            "expected build package records differ")
    selected_binary, artifacts = cargo_build_artifacts(raw_by_name["cargo.jsonl"].read_bytes(), build_expected, root / "target")
    require(artifacts == _object(proof["artifacts"], "build artifacts"), "Cargo JSON artifacts differ from receipt")
    selected = _object(proof.get("selectedBinary"), "selected binary pin")
    frozen = _object(proof.get("frozenBinary"), "frozen binary pin")
    selected_path = _canonical_build_path(selected.get("path"), "selected binary")
    frozen_path = bundle.file(frozen)
    binary_sha = _text(selected.get("sha256"), "selected binary SHA")
    capacity_input = bundle.json(proof["capacityInputRef"])
    require("stateRef" in capacity_input and "protectedRegistryRef" in capacity_input,
            "build capacity input refs are missing")
    registry_ref = _object(capacity_input.get("protectedRegistryRef"), "recorded protected registry")
    state_path, state_sha = _recorded_capacity_state(bundle, capacity_input)
    registry = read_json(bundle.file(registry_ref))
    registry_records = _objects(registry.get("refs"), "recorded protected references")
    protected_paths = [str(Path(_text(record.get("path"), "protected reference path")))
                       for record in registry_records]
    require(registry.get("schema") == "krr-protected-refs-successor-v1"
            and _integer(registry.get("physicalRefCount"), "protected reference count") == len(protected_paths)
            and bool(protected_paths) and len(set(protected_paths)) == len(protected_paths),
            "recorded protected registry differs")
    argv = [_text(value, "build argv item") for value in _array(proof.get("argv"), "build argv")]
    guard_python = _guard_python_from_proof(bundle, proof, root, argv, raw_by_name)
    validate_guarded_build_argv(argv, root, harness, cargo_launcher, arm, state_path, state_sha, protected_paths,
                                guard_python=guard_python)
    require(selected_path == selected_binary
            and sha256(frozen_path) == binary_sha and frozen.get("sha256") == binary_sha,
            "selected/frozen binary differs from Cargo artifact")
    return {"arm": arm, "binarySha256": binary_sha, "head": _text(source_before["head"], "source HEAD"),
            "sourceSnapshotSha256": _text(snapshot_ref["sha256"], "source snapshot SHA"),
            "lockSha256": historical_lock[1] if historical_lock is not None else LOCK_SHA,
            "buildArtifactSha256": sha256(proof_path),
            "cargoExecutableSha256": _text(cargo_info["sha256"], "Cargo executable SHA"),
            "rustcExecutableSha256": sha256(rustc_tool.executable),
            "buildEnvironmentSha256": proof["buildEnvironmentSha256"],
            "toolVersions": proof["toolVersions"], "cargoConfigPins": proof["cargoConfigPins"],
            "cargoLauncher": proof["cargoLauncher"], "rustcLauncher": proof["rustcLauncher"],
            "guardPython": proof["guardPython"]}


def validate_candidate_build_proof(directory: Path, root: Path, bundle_manifest_sha256: str,
                                   artifact_path: str, artifact_sha256: str, *,
                                   git_query: GitQuery | None = None) -> JsonObject:
    digest(bundle_manifest_sha256)
    require(sha256(directory / "bundle.json") == bundle_manifest_sha256,
            "bundle manifest SHA differs from managed pin")
    bundle = Bundle(directory)
    reference = bundle.relative_reference(artifact_path, artifact_sha256)
    result = _validate_build_proof(bundle, root, reference, "candidate", git_query=git_query)
    return {"binarySha256": result["binarySha256"], "sourceHead": result["head"],
            "sourceSnapshotSha256": result["sourceSnapshotSha256"], "lockSha256": result["lockSha256"],
            "buildArtifactSha256": result["buildArtifactSha256"],
            "cargoExecutableSha256": result["cargoExecutableSha256"]}


def _validate_reference_parent_wrapper(bundle: Bundle, root: Path, expected: JsonObject, *,
                                       git_query: GitQuery | None = None) -> JsonObject:
    """Validate the retained candidate proof through the explicit parent-reference wrapper."""
    contract = _object(expected.get("referenceParent"), "reference parent contract")
    wrapper_ref = _object(contract.get("wrapperRef"), "reference parent wrapper reference")
    wrapper = bundle.file(wrapper_ref)
    helper = importlib.import_module("html_performance_reference")
    def query(path: Path, args: Sequence[str]) -> str:
        return git(path, *args, git_query=git_query)
    before = bundle.json(fixed_source_pair(expected)["before"])
    measured_head = _text(before.get("head"), "measured source HEAD")
    selection = helper.active_reference_at_commit(root, measured_head, query, allow_legacy_missing=True)
    if selection is None:
        require(baseline_arm_id(expected) == REFERENCE_PARENT_ARM_ID, "legacy reference role differs")
        proof = helper.load_reference_parent_proof(
            wrapper, root, git_query=query,
            file_resolver=lambda value, _label: bundle.file(value),
        )
    else:
        require(baseline_arm_id(expected) == "reference-history-" + selection.record_id,
                "active reference role differs")
        wrapper_data = read_json(wrapper)
        historical = HistoricalBuildReader(bundle, wrapper_data.get("sourceBundleManifest"), selection)
        def resolve(value: object, label: str) -> Path:
            if label in {"source bundle manifest", "verifier stdout"}:
                return bundle.file(value)
            return historical.file(value)
        def validate(reference: object, selected: reference_domain.ActiveReference) -> JsonObject:
            return _validate_build_proof(historical, root, reference, "candidate",
                                         git_query=git_query, historical_reference=selected)
        proof = helper.load_history_reference_proof(wrapper, selection, file_resolver=resolve,
                                                    validate_build=validate)
        extra = tuple(helper.FilePin(path, checksum) for path, checksum in historical.used.items())
        proof = proof._replace(dependency_refs=proof.dependency_refs + extra)
        require({key: value for key, value in contract.items() if key != "wrapperRef"}
                == helper.reference_parent_contract(proof), "history reference contract differs")
    arm = _object(_object(expected["arms"], "expected arms")[baseline_arm_id(expected)], "reference parent arm")
    require(arm["binary_sha256"] == proof.binary_sha256
            and arm["source_snapshot_sha256"] == proof.source_snapshot_sha256
            and arm["lock_sha256"] == proof.lock_sha256,
            "reference parent expected arm differs")
    return {"arm": "baseline", "binarySha256": proof.binary_sha256,
            "head": proof.head, "sourceSnapshotSha256": proof.source_snapshot_sha256,
            "lockSha256": proof.lock_sha256,
            "buildArtifactSha256": proof.wrapper_sha256,
            "cargoExecutableSha256": proof.cargo_sha256,
            "originalBuildArtifactSha256": proof.original_build_artifact_sha256,
            "dependencyRefs": [{"path": pin.logical_path, "sha256": pin.sha256}
                               for pin in proof.dependency_refs]}


def _committed_blob_records(payload: bytes, oids: tuple[str, ...]) -> tuple[tuple[str, int], ...]:
    result: list[tuple[str, int]] = []
    offset = 0
    for oid in oids:
        header_end = payload.index(b"\n", offset)
        actual_oid, kind, size = payload[offset:header_end].decode().split()
        require(actual_oid == oid and kind == "blob", "committed blob query differs")
        start, length = header_end + 1, int(size)
        data = payload[start:start + length]
        require(len(data) == length and payload[start + length:start + length + 1] == b"\n", "truncated committed blob")
        result.append((hashlib.sha256(data).hexdigest(), length))
        offset = start + length + 1
    require(offset == len(payload), "extra committed blob data")
    return tuple(result)


_ParsedBatch = Callable[[str, tuple[str, ...]], tuple[tuple[str, int], ...]]
_PARSED_BATCHES: weakref.WeakKeyDictionary[types.FunctionType, _ParsedBatch] = weakref.WeakKeyDictionary()
_PARSED_BATCH_FACT_LIMIT = 32768


def _parsed_batch_cache(query: types.FunctionType) -> _ParsedBatch:
    cached = _PARSED_BATCHES.get(query)
    if cached is not None:
        return cached
    query_ref = weakref.ref(query)
    facts: OrderedDict[tuple[str, str], tuple[str, int]] = OrderedDict()

    def parsed(root_path: str, oids: tuple[str, ...]) -> tuple[tuple[str, int], ...]:
        callback = query_ref()
        if callback is None:
            raise ValueError("Git query transaction is unavailable")
        resolved: dict[str, tuple[str, int]] = {}
        missing: list[str] = []
        for oid in oids:
            key = (root_path, oid)
            if key in facts:
                resolved[oid] = facts[key]
            elif oid not in resolved:
                missing.append(oid)
                resolved[oid] = ("", 0)
        if missing:
            selected = tuple(missing)
            payload = _git_output(Path(root_path), ("cat-file", "--batch"), git_query=callback,
                                  input_bytes="".join(oid + "\n" for oid in selected).encode())
            if not isinstance(payload, bytes):
                raise ValueError("Git blob query returned text output")
            checked = _committed_blob_records(payload, selected)
            resolved.update(zip(selected, checked, strict=True))
        for oid in oids:
            key = (root_path, oid)
            facts[key] = resolved[oid]
            facts.move_to_end(key)
            if len(facts) > _PARSED_BATCH_FACT_LIMIT:
                facts.popitem(last=False)
        return tuple(resolved[oid] for oid in oids)

    _PARSED_BATCHES[query] = parsed
    return parsed


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
    oids = tuple(oid for _, oid in rows)
    if (isinstance(git_query, types.FunctionType) and root.is_absolute() and root.resolve() == root and oids
            and all(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", oid) for oid in oids)):
        records = _parsed_batch_cache(git_query)(str(root), oids)
    else:
        payload = _git_output(root, ("cat-file", "--batch"), git_query=git_query,
                              input_bytes="".join(oid + "\n" for oid in oids).encode())
        if not isinstance(payload, bytes):
            raise ValueError("Git blob query returned text output")
        records = _committed_blob_records(payload, oids)
    return {name: record for (name, _), record in zip(rows, records, strict=True)}


def _historical_reference_source(root: Path, before: JsonObject, selection: object, *,
                                 git_query: GitQuery | None) -> SourceRecords:
    if not isinstance(selection, reference_domain.ActiveReference):
        raise ValueError("unapproved historical reference")
    head = selection.source_head
    require(before.get("schema") == "krr-native-build-source-inventory-v1"
            and before.get("sourceManifestSchema") == "krr-native-build-source-manifest-v1"
            and before.get("head") == head, "reference source identity differs")
    require(git(root, "rev-parse", "--verify", head + "^{commit}", git_query=git_query).strip() == head,
            "reference source commit is unavailable")
    origin = Path(_text(before.get("root"), "reference source root"))
    require(origin.is_absolute(), "absolute reference source root required")
    tracked = [str(Path(_text(name, "reference tracked path")).relative_to(origin))
               for name in _array(before.get("trackedPaths"), "reference tracked paths")]
    saved = {str(Path(name).relative_to(origin)): value
             for name, value in records_map(before.get("files")).items()}
    committed = committed_records(root, head, git_query=git_query)
    require(len(tracked) == len(set(tracked)) and set(tracked) == set(committed)
            and before.get("untrackedPaths") == [] and before.get("missingTrackedPaths") == []
            and saved == committed, "reference committed source differs")
    current = committed_records(root, git_query=git_query)
    lock_path = HARNESS_ROOT + "Cargo.lock"
    from html_performance_harness_equivalence import verify_harness_sources
    def read_harness_blob(revision: str, path: str) -> bytes:
        value = _git_output(root, ("show", revision + ":" + path), git_query=git_query)
        if not isinstance(value, bytes):
            raise ValueError("harness Git blob is not binary")
        return value
    verify_harness_sources(committed, current, head, read_harness_blob)
    old_lock = git(root, "show", head + ":" + lock_path, git_query=git_query).encode("utf8")
    verify_harness_closure(old_lock, (root / lock_path).read_bytes(),
                           selection.build_pins["lockSha256"])
    return saved


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


def _shared_current_source(root: Path, before: JsonObject, shared: SharedCurrentSource) -> SourceRecords:
    require(root == shared.root, "shared source root differs")
    require(before.get("schema") == "krr-native-build-source-inventory-v1"
            and before.get("sourceManifestSchema") == "krr-native-build-source-manifest-v1"
            and before.get("head") == shared.head, "shared source identity differs")
    origin = Path(_text(before.get("root"), "source root"))
    require(origin.is_absolute() and origin == shared.origin, "shared source origin differs")
    tracked = [str(Path(_text(path, "tracked path")).relative_to(origin))
               for path in _array(before.get("trackedPaths"), "tracked paths")]
    saved = {str(Path(path).relative_to(origin)): value
             for path, value in records_map(before.get("files")).items()}
    require(len(tracked) == len(set(tracked)) and set(tracked) == set(shared.records)
            and before.get("untrackedPaths") == [] and before.get("missingTrackedPaths") == []
            and saved == shared.records, "shared source inventory differs")
    return saved


def _source_for_validation(root: Path, before: JsonObject, *, git_query: GitQuery | None,
                           reuse_context: object,
                           shared_source: SharedCurrentSource | None = None) -> SourceRecords:
    if reuse_context is None:
        return (_shared_current_source(root, before, shared_source) if shared_source is not None
                else current_source(root, before, git_query=git_query))
    require(shared_source is None, "source reuse cannot share a current source")
    from html_performance_source_reuse import SourceReuseContext
    if not isinstance(reuse_context, SourceReuseContext):
        raise ValueError("invalid source reuse context")
    require(before.get("schema") == "krr-native-build-source-inventory-v1"
            and before.get("sourceManifestSchema") == "krr-native-build-source-manifest-v1"
            and before.get("head") == reuse_context.measured_head, "measured source identity differs")
    origin = Path(_text(before["root"], "source root"))
    require(origin.is_absolute(), "absolute source root required")
    saved_paths = [str(Path(_text(p, "tracked path")).relative_to(origin))
                   for p in _array(before["trackedPaths"], "tracked paths")]
    saved = {str(Path(p).relative_to(origin)): value for p, value in records_map(before["files"]).items()}
    require(len(saved_paths) == len(set(saved_paths)) and set(saved_paths) == set(reuse_context.measured_records)
            and before["untrackedPaths"] == [] and before["missingTrackedPaths"] == []
            and saved == reuse_context.measured_records, "measured committed source differs")
    return saved


def _verified_reuse_context(root: Path, measured_head: str, *, git_query: GitQuery | None,
                            allow_test_changes: bool = False,
                            program_records: SourceRecords | None = None,
                            program_modes: Mapping[str, str] | None = None,
                            same_binary_rust: bool = False) -> object:
    from html_performance_source_reuse import validate_reuse_source
    require(re.fullmatch(r"[a-f0-9]{40}", measured_head) is not None, "invalid measured HEAD")
    eligible = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    def text_query(args: Sequence[str]) -> str:
        return git(root, *args, git_query=git_query)
    def bytes_query(args: Sequence[str]) -> bytes:
        value = _git_output(root, args, git_query=git_query)
        if not isinstance(value, bytes):
            raise ValueError("Git binary query returned text output")
        return value
    current = committed_records(root, eligible, git_query=git_query)
    measured = committed_records(root, measured_head, git_query=git_query)
    if program_records is None and program_modes is None:
        return validate_reuse_source(root, measured_head, current, measured,
                                     text_query, bytes_query, allow_test_changes=allow_test_changes)
    return validate_reuse_source(root, measured_head, current, measured, text_query, bytes_query,
                                 allow_test_changes=allow_test_changes, program_records=program_records,
                                 program_modes=program_modes, same_binary_rust=same_binary_rust)


class PartialEpochCurrentBuild(NamedTuple):
    directory: Path
    manifest_sha256: str
    artifact_path: str
    artifact_sha256: str


class CurrentBuildKwargs(TypedDict, total=False):
    current_build: PartialEpochCurrentBuild


def _current_build_kwargs(current: PartialEpochCurrentBuild | None) -> CurrentBuildKwargs:
    return {"current_build": current} if current is not None else {}


def _verified_partial_epoch_source(
    old_bundle: Bundle, root: Path, old_expected: JsonObject, current: PartialEpochCurrentBuild,
    *, git_query: GitQuery | None = None,
) -> object:
    """Authenticate source/build identity only; this does not establish runtime eligibility."""
    from html_performance_source_reuse import SourceReuseContext, _modes, equivalent_program_records

    digest(current.manifest_sha256)
    digest(current.artifact_sha256)
    current_bundle = Bundle(current.directory)
    require(sha256(current_bundle.directory / "bundle.json") == current.manifest_sha256,
            "partial epoch current build bundle differs")
    current_head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(current_bundle.manifest.get("head") == current_head,
            "partial epoch current build HEAD differs")
    old_pair = _object(old_expected.get("sourcePair"), "old source pair")
    old_before = old_bundle.json(old_pair["before"])
    old_head = _text(old_before.get("head"), "partial epoch source HEAD")
    require(old_bundle.manifest.get("head") == old_head, "partial epoch bundle HEAD differs")
    require(git(root, "merge-base", old_head, current_head, git_query=git_query).strip() == old_head,
            "partial epoch source is not an ancestor")
    old_records = committed_records(root, old_head, git_query=git_query)
    current_records = committed_records(root, current_head, git_query=git_query)
    require(not git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query),
            "partial epoch current source is dirty")
    def program_closure(records: SourceRecords) -> SourceRecords:
        return {path: value for path, value in records.items()
                if path.startswith("scripts/") or path == "Justfile"}
    require(equivalent_program_records(program_closure(old_records), program_closure(current_records)),
            "partial epoch measurement program source differs")
    old_tree = _git_output(root, ("ls-tree", "-r", "-z", old_head), git_query=git_query)
    current_tree = _git_output(root, ("ls-tree", "-r", "-z", current_head), git_query=git_query)
    if not isinstance(old_tree, bytes) or not isinstance(current_tree, bytes):
        raise ValueError("partial epoch Git tree output is not binary")
    old_modes, current_modes = _modes(old_tree), _modes(current_tree)
    require(all(old_modes.get(path) == current_modes.get(path)
                for path in program_closure(old_records)), "partial epoch program mode differs")
    current_ref = current_bundle.relative_reference(current.artifact_path, current.artifact_sha256)
    current_proof = _validate_build_proof(current_bundle, root, current_ref, "candidate",
                                          git_query=git_query)
    require(current_proof.get("head") == current_head, "partial epoch current build source differs")
    # The provisional object is local: it is returned only after both strict build proofs match.
    provisional = SourceReuseContext(old_head, current_head, old_records, (), True)
    arms = _object(old_expected.get("arms"), "old arms")
    candidate = _object(arms.get("candidate"), "old candidate arm")
    old_proof = _validate_build_proof(old_bundle, root, candidate.get("build_artifact_ref"),
                                      "candidate", git_query=git_query, reuse_context=provisional)
    require(old_proof.get("head") == old_head, "partial epoch old build source differs")
    endpoints = _object(old_expected.get("endpointRefs"), "partial epoch endpoints")
    old_context = old_bundle.json(endpoints.get("before"))
    tool = _object(old_context.get("toolContext"), "partial epoch tool context")
    versions = _object(old_proof.get("toolVersions"), "partial epoch build tool versions")
    require(all(tool.get(name) == versions.get(name) for name in ("rustc", "cargo")),
            "partial epoch measured/build tool versions differ")
    for key in ("binarySha256", "lockSha256", "cargoExecutableSha256", "rustcExecutableSha256",
                "buildEnvironmentSha256", "toolVersions", "cargoConfigPins",
                "cargoLauncher", "rustcLauncher", "guardPython"):
        require(key in old_proof and key in current_proof and old_proof[key] == current_proof[key],
                "partial epoch build differs: " + key)
    require(candidate.get("binary_sha256") == old_proof.get("binarySha256")
            and candidate.get("source_snapshot_sha256") == old_proof.get("sourceSnapshotSha256")
            and candidate.get("lock_sha256") == old_proof.get("lockSha256"),
            "partial epoch candidate role differs")
    require(sha256(current_bundle.directory / "bundle.json") == current.manifest_sha256
            and git(root, "rev-parse", "HEAD", git_query=git_query).strip() == current_head
            and not git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query),
            "partial epoch current source changed during validation")
    return provisional


def _validate_epoch_projection_context(context: JsonObject, projection: JsonObject) -> None:
    """Bind an authenticated projection to the actual transported context."""
    from html_performance_epoch_reuse import _inventory_digest

    declared = _object(projection.get("declared"), "epoch declared inputs")
    require(declared.get("tool_context") == context.get("toolContext"),
            "epoch projection tool context differs")
    rows = _objects(context.get("inputs"), "epoch context inputs")
    indexed = {_text(row.get("path"), "epoch context path"): row for row in rows}
    require(len(indexed) == len(rows), "duplicate epoch context input")
    known: set[str] = set()
    for group in ("workers", "inputs", "resources", "fonts"):
        for pin_value in _object(declared.get(group), "epoch " + group).values():
            pin = _object(pin_value, "epoch input pin")
            path = _text(pin.get("path"), "epoch input path")
            row = indexed.get(path)
            require(row is not None and row.get("exists") is True
                    and row.get("sha256") == pin.get("sha256"),
                    "epoch projection input context differs")
            known.add(path)
    for value in _object(projection.get("operational_pins"), "epoch operational pins").values():
        pin = _object(value, "epoch operational pin")
        path = _text(pin.get("path"), "epoch operational path")
        row = indexed.get(path)
        require(row is None or row.get("exists") is True and row.get("sha256") == pin.get("sha256"),
                "epoch operational context differs")
        known.add(path)
    unknown = [row for row in rows if _text(row.get("path"), "epoch context path") not in known]
    unknown.sort(key=lambda row: _text(row.get("path"), "epoch context path"))
    require(projection.get("unclassified_context_digest") == _inventory_digest(unknown),
            "epoch projection unclassified context differs")


def fixed_source_pair(expected: JsonObject) -> JsonObject:
    pair = _object(expected.get("sourcePair"), "source pair")
    return (_object(pair.get("epochSourcePair"), "fixed epoch source pair")
            if pair.get("schema") == "krr-html-performance-source-epoch-v3" else pair)


_V3_LIVE_CACHE: OrderedDict[tuple[str, bytes], tuple[tuple[str, tuple[str, int]], ...]] = OrderedDict()
_V3_LIVE_CACHE_LOCK = Lock()
_V3_LIVE_CACHE_BYTES = 32 * 1024 * 1024


def _v3_live_key(root: Path, checkout: JsonObject) -> tuple[str, bytes] | None:
    if type(root) is not type(Path()) or type(checkout) is not dict:
        return None
    pending: list[object] = [checkout]
    seen: set[int] = set()
    while pending:
        value = pending.pop()
        if type(value) is dict:
            if id(value) in seen or any(type(key) is not str for key in value):
                return None
            seen.add(id(value))
            pending.extend(value.values())
        elif type(value) is list:
            if id(value) in seen:
                return None
            seen.add(id(value))
            pending.extend(value)
        elif type(value) not in (str, int, float, bool, type(None)):
            return None
    try:
        raw = marshal.dumps(checkout, 2)
    except (TypeError, ValueError, OverflowError, RecursionError):
        return None
    return (str(root), raw) if len(raw) <= _V3_LIVE_CACHE_BYTES else None


def _v3_live_programs(root: Path, checkout: JsonObject) -> SourceRecords:
    key = _v3_live_key(root, checkout)
    if key is None:
        return _v3_live_programs_uncached(root, checkout)
    with _V3_LIVE_CACHE_LOCK:
        cached = _V3_LIVE_CACHE.get(key)
        if cached is not None:
            _V3_LIVE_CACHE.move_to_end(key)
    if cached is not None:
        if _v3_live_key(root, checkout) == key:
            return dict(cached)
        return _v3_live_programs_uncached(root, checkout)
    result = _v3_live_programs_uncached(root, checkout)
    if _v3_live_key(root, checkout) != key:
        return result
    with _V3_LIVE_CACHE_LOCK:
        _V3_LIVE_CACHE[key] = tuple(result.items())
        _V3_LIVE_CACHE.move_to_end(key)
        while (len(_V3_LIVE_CACHE) > 8
               or sum(len(item[1]) for item in _V3_LIVE_CACHE) > _V3_LIVE_CACHE_BYTES):
            _V3_LIVE_CACHE.popitem(last=False)
    return dict(result)


def _v3_live_programs_uncached(root: Path, checkout: JsonObject) -> SourceRecords:
    require(set(checkout) == {"schema", "root", "head", "indexSha256", "statusBase64", "manifest"}
            and checkout.get("schema") == "krr-html-performance-live-source-observation-v1"
            and checkout.get("root") == str(root), "live checkout identity differs")
    require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", _text(checkout.get("head"), "live HEAD"))
            is not None, "live checkout HEAD differs")
    digest(checkout.get("indexSha256"))
    status = checkout.get("statusBase64")
    require(isinstance(status, str), "live status must be a string")
    if not isinstance(status, str):
        raise ValueError("live status must be a string")
    try:
        status_bytes = base64.b64decode(status, validate=True)
        require(base64.b64encode(status_bytes).decode("ascii") == status,
                "live status encoding differs")
    except ValueError as error:
        raise ValueError("live status encoding differs") from error
    entries = status_bytes.split(b"\0")
    require(entries[-1] == b"", "live status terminator differs")
    index = 0
    while index < len(entries) - 1:
        entry = entries[index]
        require(len(entry) >= 4 and entry[2:3] == b" ", "live status entry differs")
        names = [entry[3:].decode("utf8")]
        if b"R" in entry[:2] or b"C" in entry[:2]:
            index += 1
            require(index < len(entries) - 1, "live status rename differs")
            names.append(entries[index].decode("utf8"))
        require(all(name != "Justfile" and not name.startswith("scripts/") for name in names),
                "live measurement program has uncommitted changes")
        index += 1
    manifest = _object(checkout.get("manifest"), "live source manifest")
    require(set(manifest) == {"schema", "digest", "files"}
            and manifest.get("schema") == "krr-native-build-source-manifest-v1",
            "live source manifest schema differs")
    rows = _objects(manifest.get("files"), "live source records")
    require(all(set(row) == {"path", "sha256", "size"} for row in rows),
            "live source record fields differ")
    records = records_map(rows)
    paths = list(records)
    require(paths == sorted(paths, key=lambda value: value.encode("utf8")),
            "live source record order differs")
    material = "".join(f"{path}\0{records[path][0]}\0{records[path][1]}\n" for path in paths)
    require(hashlib.sha256(material.encode("utf8")).hexdigest() == manifest.get("digest"),
            "live source manifest digest differs")
    live: SourceRecords = {}
    for name, item in records.items():
        path = Path(name)
        require(path.is_absolute() and os.path.normpath(name) == name and path.is_relative_to(root)
                and path != root, "live source record path differs")
        relative = path.relative_to(root).as_posix()
        if relative.startswith("scripts/") or relative == "Justfile":
            live[relative] = item
    require(bool(live), "live measurement program is empty")
    return live


_ProgramItems = tuple[tuple[str, str, int], ...]
_ProgramEndpoints = tuple[tuple[str, _ProgramItems], ...]
_ProgramCache = Callable[[str, str, _ProgramItems, _ProgramEndpoints, str | None], _ProgramItems]
_PROGRAM_CACHES: weakref.WeakKeyDictionary[types.FunctionType, _ProgramCache] = weakref.WeakKeyDictionary()


def _v3_authenticate_programs(root: Path, head: str, items: _ProgramItems,
                              endpoints: _ProgramEndpoints, *, git_query: GitQuery | None,
                              memory_policy_version: str | None = None) -> _ProgramItems:
    from html_performance_source_reuse import (
        RELEASE_ONLY_PROGRAMS, VALIDATION_COHORT_ADDITIONS, VALIDATION_COHORT_CHANGES,
        _modes, cache_only_collector_change,
        equivalent_program_records, known_auth_program_change, validation_cohort_change,
        v3_reassessment_program_change, v4_reassessment_program_change,
    )

    programs = {name: (checksum, size) for name, checksum, size in items}
    committed = committed_records(root, head, git_query=git_query)
    selected = {path: value for path, value in committed.items()
                if path.startswith("scripts/") or path == "Justfile"}
    require(programs == selected, "fixed epoch program source differs from Git")
    tree = _git_output(root, ("ls-tree", "-r", "-z", head), git_query=git_query)
    if not isinstance(tree, bytes):
        raise ValueError("fixed epoch program tree differs")
    modes = _modes(tree)
    # WHY: 固定 epoch から既存認証を維持し、今回の移行だけ別に固定する。
    from html_performance_source_reuse import (
        SAME_BINARY_REVIEWED_ADDITIONS, SAME_BINARY_REVIEWED_BASE,
        reviewed_same_binary_migration, reviewed_same_binary_self,
    )
    pending = []
    self_path = "scripts/release/html_performance_source_reuse.py"
    for live_head, live_items in endpoints:
        if live_head == head or self_path not in {name for name, _, _ in live_items}:
            pending.append((live_head, live_items))
            continue
        live_data = _git_output(root, ("show", live_head + ":" + self_path), git_query=git_query)
        try:
            base_data = _git_output(root, ("show", SAME_BINARY_REVIEWED_BASE + ":" + self_path),
                                    git_query=git_query)
            base_ancestor = (git(root, "merge-base", SAME_BINARY_REVIEWED_BASE, live_head,
                                 git_query=git_query).strip() == SAME_BINARY_REVIEWED_BASE)
        except subprocess.CalledProcessError:
            base_ancestor = False
            base_data = None
        if not base_ancestor or base_data == live_data:
            pending.append((live_head, live_items))
            continue
        require(isinstance(base_data, bytes) and isinstance(live_data, bytes)
                and reviewed_same_binary_self(live_data, allow_historical=True)
                and reviewed_same_binary_migration(self_path, base_data, live_data,
                                                    old_mode="100644", new_mode="100644"),
                "reviewed source migration differs")
        base_records = committed_records(root, SAME_BINARY_REVIEWED_BASE, git_query=git_query)
        base_programs = {path: value for path, value in base_records.items()
                         if path.startswith("scripts/") or path == "Justfile"}
        live_programs = {name: (checksum, size) for name, checksum, size in live_items}
        actual_live = committed_records(root, live_head, git_query=git_query)
        additions = set(live_programs) - set(base_programs)
        require(live_programs == {path: value for path, value in actual_live.items()
                                  if path.startswith("scripts/") or path == "Justfile"}
                and not (set(base_programs) - set(live_programs))
                and (not additions or additions == set(SAME_BINARY_REVIEWED_ADDITIONS)),
                "reviewed program membership differs")
        base_tree = _git_output(root, ("ls-tree", "-r", "-z", SAME_BINARY_REVIEWED_BASE),
                                git_query=git_query)
        live_tree = _git_output(root, ("ls-tree", "-r", "-z", live_head), git_query=git_query)
        require(isinstance(base_tree, bytes) and isinstance(live_tree, bytes),
                "reviewed program tree differs")
        if not isinstance(base_tree, bytes) or not isinstance(live_tree, bytes):
            raise ValueError("reviewed program tree differs")
        base_modes, live_modes = _modes(base_tree), _modes(live_tree)
        require(all(live_programs[path] == SAME_BINARY_REVIEWED_ADDITIONS[path]
                    and live_modes.get(path) == "100644" for path in additions),
                "reviewed program addition differs")
        for path in base_programs:
            require(base_modes.get(path) == live_modes.get(path), "reviewed program mode differs")
            if base_programs[path] == live_programs[path]:
                continue
            old_blob = _git_output(root, ("show", SAME_BINARY_REVIEWED_BASE + ":" + path),
                                   git_query=git_query)
            new_blob = _git_output(root, ("show", live_head + ":" + path), git_query=git_query)
            require(isinstance(old_blob, bytes) and isinstance(new_blob, bytes)
                    and reviewed_same_binary_migration(path, old_blob, new_blob,
                                                       old_mode=base_modes.get(path),
                                                       new_mode=live_modes.get(path)),
                    "unreviewed program migration")
        base_items = tuple((name, checksum, size) for name, (checksum, size) in sorted(base_programs.items()))
        _v3_authenticate_programs(root, head, items, ((SAME_BINARY_REVIEWED_BASE, base_items),),
                                  git_query=git_query, memory_policy_version=memory_policy_version)
    for live_head, live_items in pending:
        live_programs = {name: (checksum, size) for name, checksum, size in live_items}
        live_committed = committed_records(root, live_head, git_query=git_query)
        live_selected = {path: value for path, value in live_committed.items()
                         if path.startswith("scripts/") or path == "Justfile"}
        require(live_programs == live_selected, "live program source differs from Git")
        live_tree = _git_output(root, ("ls-tree", "-r", "-z", live_head), git_query=git_query)
        if not isinstance(live_tree, bytes):
            raise ValueError("live program tree differs")
        live_modes = _modes(live_tree)
        additions = set(live_programs) - set(programs)
        require(not (set(programs) - set(live_programs))
                and (not additions or additions == set(VALIDATION_COHORT_ADDITIONS))
                and all(validation_cohort_change(path, None, live_programs[path], None,
                                                 live_modes.get(path)) for path in additions),
                "live measurement program membership differs")
        if additions:
            cohort = set(VALIDATION_COHORT_CHANGES) | {
                "scripts/release/html_performance_source_reuse.py",
            }
            require(all(path in programs and path in live_programs
                        and validation_cohort_change(path, programs[path], live_programs[path],
                                                     modes.get(path), live_modes.get(path))
                        for path in cohort), "live validation cohort differs")
        comparable = {path: live_programs[path] for path in programs}
        for path in programs:
            if path in RELEASE_ONLY_PROGRAMS or programs[path] == live_programs[path]:
                continue
            if validation_cohort_change(path, programs[path], live_programs[path],
                                        modes.get(path), live_modes.get(path)):
                comparable[path] = programs[path]
                continue
            old_blob = _git_output(root, ("show", head + ":" + path), git_query=git_query)
            new_blob = _git_output(root, ("show", live_head + ":" + path), git_query=git_query)
            require(isinstance(old_blob, bytes) and isinstance(new_blob, bytes)
                    and (path == "scripts/release/produce_html_performance_evidence.py"
                         and cache_only_collector_change(old_blob, new_blob)
                         or (memory_policy_version not in ("html-memory-v3", "html-memory-v4")
                             and known_auth_program_change(path, old_blob, new_blob))
                         or memory_policy_version in ("html-memory-v3", "html-memory-v4")
                         and (v3_reassessment_program_change(
                             path, old_blob, new_blob, old_mode=modes.get(path),
                             new_mode=live_modes.get(path))
                              or memory_policy_version == "html-memory-v4"
                              and v4_reassessment_program_change(
                                  path, old_blob, new_blob, old_mode=modes.get(path),
                                  new_mode=live_modes.get(path)))),
                    "live measurement program differs from fixed epoch")
            comparable[path] = programs[path]
        require(equivalent_program_records(comparable, programs),
                "live measurement program differs from fixed epoch")
        require(all(modes.get(path) == live_modes.get(path) for path in programs),
                "live measurement program mode differs")
    return tuple((name, checksum, size) for name, (checksum, size) in sorted(programs.items()))


def _v3_program_cache(query: types.FunctionType) -> _ProgramCache:
    cached = _PROGRAM_CACHES.get(query)
    if cached is not None:
        return cached
    query_ref = weakref.ref(query)

    @lru_cache(maxsize=4)
    def authenticate(root_path: str, head: str, items: _ProgramItems,
                     endpoints: _ProgramEndpoints, memory_policy_version: str | None) -> _ProgramItems:
        callback = query_ref()
        if callback is None:
            raise ValueError("Git query transaction is unavailable")
        return _v3_authenticate_programs(Path(root_path), head, items, endpoints, git_query=callback,
            **({"memory_policy_version": memory_policy_version}
               if memory_policy_version is not None else {}))

    _PROGRAM_CACHES[query] = authenticate
    return authenticate


def _v3_program_source(root: Path, fixed: JsonObject, endpoints: Sequence[JsonObject], *,
                       git_query: GitQuery | None,
                       memory_policy_version: str | None = None) -> SourceRecords:
    program_source = _object(fixed.get("programSource"), "fixed epoch program source")
    programs = _v3_live_programs(root, program_source)
    head = _text(program_source.get("head"), "program source HEAD")
    items = tuple((name, checksum, size) for name, (checksum, size) in sorted(programs.items()))
    live_items: list[tuple[str, _ProgramItems]] = []
    for endpoint in endpoints:
        checkout = _object(endpoint.get("liveCheckout"), "live checkout")
        live = _v3_live_programs(root, checkout)
        live_head = _text(checkout.get("head"), "live program HEAD")
        live_items.append((live_head, tuple((name, checksum, size)
                                           for name, (checksum, size) in sorted(live.items()))))
    endpoint_items = tuple(live_items)
    if (isinstance(git_query, types.FunctionType) and root.is_absolute() and root.resolve() == root
            and re.fullmatch(r"[0-9a-f]{40}", head) is not None
            and all(re.fullmatch(r"[0-9a-f]{40}", live_head) is not None
                    for live_head, _ in endpoint_items)):
        checked = _v3_program_cache(git_query)(str(root), head, items, endpoint_items, memory_policy_version)
    else:
        checked = _v3_authenticate_programs(root, head, items, endpoint_items, git_query=git_query,
                                            **({"memory_policy_version": memory_policy_version}
                                               if memory_policy_version is not None else {}))
    return {name: (checksum, size) for name, checksum, size in checked}


def _host_load_source_records(bundle: Bundle, root: Path, expected: JsonObject,
                              measured_head: str, *, git_query: GitQuery | None,
                              memory_policy_version: str | None = None) -> SourceRecords:
    pair = _object(expected.get("sourcePair"), "host-load source pair")
    if pair.get("schema") != "krr-html-performance-source-epoch-v3":
        return committed_records(root, measured_head, git_query=git_query)
    fixed = bundle.json(pair["epochContextRef"])
    endpoints = [bundle.json(pair[key]) for key in ("beforeContextRef", "endContextRef")]
    return _v3_program_source(root, fixed, endpoints, git_query=git_query,
                              memory_policy_version=memory_policy_version)


def _require_reviewed_same_binary_program(root: Path, live_head: str, *,
                                          git_query: GitQuery | None) -> None:
    from html_performance_source_reuse import SAME_BINARY_REVIEWED_BASE, reviewed_same_binary_self

    require(git(root, "merge-base", SAME_BINARY_REVIEWED_BASE, live_head,
                git_query=git_query).strip() == SAME_BINARY_REVIEWED_BASE,
            "reviewed same-binary base is not an ancestor")
    source = _git_output(root, ("show", live_head + ":scripts/release/html_performance_source_reuse.py"),
                         git_query=git_query)
    require(isinstance(source, bytes) and reviewed_same_binary_self(source),
            "reviewed same-binary source differs")


def _verified_v3_reuse_context(bundle: Bundle, root: Path, measured_head: str,
                               expected: JsonObject, *, git_query: GitQuery | None,
                               memory_policy_version: str | None = None,
                               same_binary_rust: bool = False) -> object:
    from html_performance_source_reuse import _modes

    pair = _object(expected.get("sourcePair"), "v3 source pair")
    require(pair.get("schema") == "krr-html-performance-source-epoch-v3",
            "v3 source reuse requires v3 evidence")
    source = bundle.json(fixed_source_pair(expected)["before"])
    require(source.get("head") == measured_head, "v3 fixed source HEAD differs")
    origin_root = Path(_text(source.get("root"), "v3 source root"))
    require(origin_root == root, "v3 program source root differs")
    fixed = bundle.json(pair["epochContextRef"])
    endpoints = [bundle.json(pair[key]) for key in ("beforeContextRef", "endContextRef")]
    programs = _v3_program_source(root, fixed, endpoints, git_query=git_query,
                                  memory_policy_version=memory_policy_version)
    program_source = _object(fixed.get("programSource"), "v3 program source")
    program_head = _text(program_source.get("head"), "v3 program HEAD")
    raw_tree = _git_output(root, ("ls-tree", "-r", "-z", program_head), git_query=git_query)
    if not isinstance(raw_tree, bytes):
        raise ValueError("v3 program tree differs")
    modes = _modes(raw_tree)
    require(set(programs) <= set(modes), "v3 program modes missing")
    live_head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    if same_binary_rust:
        _require_reviewed_same_binary_program(root, live_head, git_query=git_query)
    live_records = committed_records(root, live_head, git_query=git_query)
    live_programs = {path: value for path, value in live_records.items()
                     if path.startswith("scripts/") or path == "Justfile"}
    fixed_items = tuple((name, checksum, size) for name, (checksum, size) in sorted(programs.items()))
    live_items = tuple((name, checksum, size) for name, (checksum, size) in sorted(live_programs.items()))
    _v3_authenticate_programs(root, program_head, fixed_items, ((live_head, live_items),),
                              git_query=git_query, memory_policy_version=memory_policy_version)
    live_tree = _git_output(root, ("ls-tree", "-r", "-z", live_head), git_query=git_query)
    require(isinstance(live_tree, bytes), "current program tree differs")
    if not isinstance(live_tree, bytes):
        raise ValueError("current program tree differs")
    live_modes = _modes(live_tree)
    require(all(modes[path] == live_modes.get(path) for path in programs), "current program mode differs")
    return _verified_reuse_context(
        root, measured_head, git_query=git_query, allow_test_changes=True,
        program_records=live_programs, program_modes={path: live_modes[path] for path in live_programs},
        same_binary_rust=same_binary_rust,
    )


def _v3_origin_and_endpoints(
    bundle: Bundle, root: Path, expected: JsonObject, result: JsonObject,
    before: JsonObject, end: JsonObject, *, git_query: GitQuery | None,
) -> tuple[object, JsonObject]:
    from html_performance_remote_evidence import validate_fixed_epoch_candidate_origin_reader
    from html_performance_source_reuse import SourceReuseContext

    require(before.get("schema") == end.get("schema") == "krr-html-performance-context-epoch-v3",
            "v3 endpoint schema differs")
    origin = _object(before.get("epochOrigin"), "v3 epoch origin")
    require(origin == end.get("epochOrigin"), "v3 epoch origin changed")
    manifest = _object(origin.get("bundleManifest"), "v3 origin bundle manifest")
    artifact = _object(origin.get("artifact"), "v3 origin artifact")
    manifest_path = Path(_text(manifest.get("path"), "v3 origin manifest path"))
    require(manifest_path.is_absolute() and manifest_path.name == "bundle.json",
            "v3 origin manifest path differs")
    bundle.file(manifest)
    bundle.file(artifact)
    fixed = validate_fixed_epoch_candidate_origin_reader(
        bundle, root, manifest, artifact, _text(origin.get("head"), "v3 origin HEAD"),
        git_query=git_query,
    )
    source_pair = fixed_source_pair(expected)
    candidate = _object(_object(expected.get("arms"), "v3 arms").get("candidate"), "v3 candidate")
    require(origin.get("source") == source_pair.get("before")
            and origin.get("snapshot") == source_pair.get("snapshot")
            and origin.get("binarySha256") == fixed.binary_sha256
            and origin.get("lockSha256") == fixed.lock_sha256
            and artifact == candidate.get("build_artifact_ref")
            and candidate.get("binary_sha256") == fixed.binary_sha256
            and candidate.get("source_snapshot_sha256") == fixed.snapshot_sha256
            and candidate.get("lock_sha256") == fixed.lock_sha256,
            "v3 origin source or worker differs")
    source_before = bundle.json(source_pair["before"])
    require(source_before.get("head") == fixed.head, "v3 origin source HEAD differs")
    records = committed_records(root, fixed.head, git_query=git_query)
    current = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    reuse = SourceReuseContext(fixed.head, current, records, (), True)
    proof = bundle.json(artifact)
    candidate_build: JsonObject = {"head": fixed.head, "binarySha256": fixed.binary_sha256,
        "sourceSnapshotSha256": fixed.snapshot_sha256, "lockSha256": fixed.lock_sha256,
        "buildArtifactSha256": fixed.artifact_sha256,
        "cargoExecutableSha256": fixed.cargo_sha256, "rustcExecutableSha256": fixed.rustc_sha256}
    for key in ("buildEnvironmentSha256", "toolVersions", "cargoConfigPins", "cargoLauncher",
                "rustcLauncher", "guardPython"):
        require(key in proof, "v3 candidate build environment is incomplete")
        candidate_build[key] = proof[key]
    mapping = {"sourceBeforeEqualsEnd": "liveCheckout", "inputsBeforeEqualsEnd": "inputs",
               "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache",
               "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope",
               "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference",
               "toolContextBeforeEqualsEnd": "toolContext"}
    for flag, field in mapping.items():
        require(field in before and field in end and type(result.get(flag)) is bool
                and result[flag] == (before[field] == end[field]), "v3 endpoint flag differs")
        if field not in {"liveCheckout", "registry", "cache", "cacheUnion"}:
            require(before[field] == end[field], "v3 runtime endpoint changed")
    require(before.get("workerEnvironment") == end.get("workerEnvironment")
            and before.get("systemWorkers") == end.get("systemWorkers"),
            "v3 worker boundary changed")
    for endpoint in (before, end):
        require(endpoint.get("cacheUnion") == {"registry": endpoint.get("registry"),
                                               "target": endpoint.get("cache")},
                "v3 cache inventory differs")
    return reuse, candidate_build


def _v3_worker_observations(bundle: Bundle, expected: JsonObject, context: JsonObject,
                            builds: Mapping[str, JsonObject]) -> None:
    import local_macos_tool_provenance as provenance

    workers = _object(context.get("systemWorkers"), "v3 system workers")
    binaries = execution_binary_refs(bundle, expected, builds)
    require(set(workers) == set(binaries) == {"baseline", "candidate"},
            "v3 system worker roles differ")
    tool = _object(context.get("toolContext"), "v3 tool context")
    for role, pin in binaries.items():
        observed = _object(workers[role], "v3 worker observation")
        require(set(observed) == {"schema", "worker", "stamp", "platform", "environment_sha256"}
                and observed.get("schema") == "krr-html-system-only-worker-observation-v1"
                and observed.get("worker") == pin and observed.get("platform") == tool.get("platform")
                and observed.get("environment_sha256") == tool.get("environmentSha256"),
                "v3 worker observation differs")
        stamp = _array(observed.get("stamp"), "v3 worker stamp")
        require(len(stamp) == 6 and all(type(item) is int and item >= 0 for item in stamp)
                and stat.S_ISREG(cast(int, stamp[2])) and cast(int, stamp[3]) > 0,
                "v3 worker stamp differs")
        data = bundle.file(pin).read_bytes()
        parsed = provenance._native_dependencies(data)
        require(parsed is not None, "v3 worker is not supported Mach-O")
        if parsed is None:
            raise ValueError("v3 worker is not supported Mach-O")
        dependencies, rpaths = parsed
        require(not rpaths and all(dependency.startswith(("/usr/lib/", "/System/Library/"))
                                   and os.path.normpath(dependency) == dependency
                                   for dependency in dependencies),
                "v3 worker has non-system dependencies or rpaths")
    worker_env = _object(context.get("workerEnvironment"), "v3 worker environment")
    metadata = _object(worker_env.get("metadata"), "v3 worker metadata")
    snapshot = _object(fixed_source_pair(expected).get("snapshot"), "v3 source snapshot")
    require(set(worker_env) == {"schema", "baseEnvironmentSha256", "metadata",
                                "nonMetadataEnvironmentSha256", "nonMetadataKeyNames",
                                "nonMetadataKeyNamesSha256"}
            and worker_env.get("schema") == "krr-html-performance-worker-environment-contract-v1"
            and worker_env.get("baseEnvironmentSha256") == tool.get("environmentSha256")
            and metadata == {"KRR_PERF_RUSTC": "toolContext.rustc",
                             "KRR_PERF_CARGO": "toolContext.cargo",
                             "KRR_PERF_ENVIRONMENT_SHA256": "before-context-sha256",
                             "KRR_PERF_LOCK_PATH": str(Path(_text(
                                 _object(context["liveCheckout"], "live checkout")["root"],
                                 "live root")) / HARNESS_ROOT / "Cargo.lock"),
                             "KRR_PERF_SOURCE_SNAPSHOT_PATH": snapshot.get("path")},
            "v3 worker environment contract differs")


def _v3_json_digest(value: object) -> str:
    raw = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(raw.encode("utf8")).hexdigest()


def _v3_process_stamp(value: object, worker: JsonObject, stamp: list[object]) -> None:
    identity = _object(value, "v3 worker binary identity")
    require(set(identity) == {"path", "sha256", "size", "device", "inode", "mode", "mtimeNs", "ctimeNs"}
            and identity.get("path") == worker.get("path")
            and identity.get("sha256") == worker.get("sha256"), "v3 worker binary identity differs")
    fields: list[int] = []
    for key in ("device", "inode", "mode", "size", "mtimeNs", "ctimeNs"):
        value = identity.get(key)
        if key in {"mtimeNs", "ctimeNs"} and isinstance(value, str):
            require(re.fullmatch(r"0|[1-9][0-9]*", value) is not None,
                    "v3 worker identity timestamp differs")
            value = int(value)
        require(type(value) is int and value >= 0, "v3 worker identity number differs")
        fields.append(cast(int, value))
    require(fields == stamp, "v3 worker execution stamp differs from observed boundary")


def validate_worker_runtime_environment(
    execution: JsonObject, *, non_metadata_environment_sha: str,
    metadata_values: Mapping[str, object], non_metadata_key_names: Sequence[str] | None = None,
    expected_environment_sha: str | None = None,
) -> str:
    """Check one execution against independently authenticated environment inputs."""
    metadata_keys = sorted(("KRR_PERF_RUSTC", "KRR_PERF_CARGO", "KRR_PERF_ENVIRONMENT_SHA256",
                            "KRR_PERF_LOCK_PATH", "KRR_PERF_SOURCE_SNAPSHOT_PATH"))
    require(set(metadata_values) == set(metadata_keys)
            and all(isinstance(value, str) and bool(value) for value in metadata_values.values()),
            "v3 metadata values differ")
    digest(non_metadata_environment_sha)
    names = _array(execution.get("keyNames"), "v3 worker environment keys")
    require(all(isinstance(name, str) and bool(name) for name in names)
            and names == sorted(set(cast(list[str], names)))
            and set(metadata_keys) <= set(cast(list[str], names)),
            "v3 worker process environment differs")
    base_names = [name for name in cast(list[str], names) if name not in metadata_keys]
    require(non_metadata_key_names is None or base_names == list(non_metadata_key_names),
            "v3 worker process environment differs")
    actual_environment_sha = _text(execution.get("actualEnvironmentSha256"), "v3 worker environment SHA")
    digest(actual_environment_sha)
    require(expected_environment_sha is None or actual_environment_sha == expected_environment_sha,
            "v3 actual worker environment changed")
    require(execution.get("keyNamesSha256") == _v3_json_digest(names)
            and execution.get("nonMetadataEnvironmentSha256") == non_metadata_environment_sha
            and execution.get("metadataKeys") == metadata_keys
            and execution.get("metadataValuesSha256") == _v3_json_digest(dict(metadata_values)),
            "v3 worker process environment differs")
    return actual_environment_sha


def validate_v3_worker_runtime_execution(
    expected: JsonObject, context: JsonObject, role: str, attempt: JsonObject,
    lifecycle: JsonObject, *, expected_environment_sha: str | None = None,
) -> str:
    """Authenticate one worker execution; caller validates lifecycle role and argv."""
    require(role in {"baseline", "candidate"}, "v3 worker role differs")
    workers = _object(context.get("systemWorkers"), "v3 system workers")
    environment = _object(context.get("workerEnvironment"), "v3 worker environment")
    base_names = _array(environment.get("nonMetadataKeyNames"), "v3 base environment keys")
    require(all(isinstance(name, str) and name for name in base_names)
            and base_names == sorted(set(cast(list[str], base_names)))
            and environment.get("nonMetadataKeyNamesSha256") == _v3_json_digest(base_names),
            "v3 base environment keys differ")
    base_sha = environment.get("nonMetadataEnvironmentSha256")
    digest(base_sha)
    metadata_keys = sorted(("KRR_PERF_RUSTC", "KRR_PERF_CARGO", "KRR_PERF_ENVIRONMENT_SHA256",
                            "KRR_PERF_LOCK_PATH", "KRR_PERF_SOURCE_SNAPSHOT_PATH"))
    require(not set(cast(list[str], base_names)).intersection(metadata_keys),
            "v3 base metadata keys overlap")
    metadata = _object(environment.get("metadata"), "v3 metadata roles")
    provenance = _object(expected.get("provenance"), "v3 provenance")
    before_ref = _object(_object(expected.get("endpointRefs"), "v3 endpoints").get("before"),
                         "v3 before endpoint")
    values = {"KRR_PERF_RUSTC": provenance.get("rustc"),
              "KRR_PERF_CARGO": provenance.get("cargo"),
              "KRR_PERF_ENVIRONMENT_SHA256": before_ref.get("sha256"),
              "KRR_PERF_LOCK_PATH": metadata.get("KRR_PERF_LOCK_PATH"),
              "KRR_PERF_SOURCE_SNAPSHOT_PATH": metadata.get("KRR_PERF_SOURCE_SNAPSHOT_PATH")}
    require(all(isinstance(value, str) and bool(value) for value in values.values()),
            "v3 metadata values differ")
    observed = _object(workers.get(role), "v3 worker boundary observation")
    pin = _object(observed.get("worker"), "v3 worker pin")
    stamp = _array(observed.get("stamp"), "v3 worker stamp")
    require(attempt.get("workerStarted") is True
            and attempt.get("binarySha256Before") == pin.get("sha256")
            and attempt.get("binarySha256After") == pin.get("sha256"),
            "v3 worker attempt binary differs")
    execution = _object(attempt.get("runtimeExecution"), "v3 worker runtime execution")
    require(execution == lifecycle.get("runtimeExecution")
            and set(execution) == {"schema", "binaryIdentityBefore", "binaryIdentityAfter",
                                   "actualEnvironmentSha256", "keyNames", "keyNamesSha256",
                                   "nonMetadataEnvironmentSha256", "metadataKeys", "metadataValuesSha256"}
            and execution.get("schema") == "krr-html-worker-runtime-execution-v1",
            "v3 worker runtime execution differs")
    _v3_process_stamp(execution.get("binaryIdentityBefore"), pin, stamp)
    _v3_process_stamp(execution.get("binaryIdentityAfter"), pin, stamp)
    return validate_worker_runtime_environment(
        execution, non_metadata_environment_sha=_text(base_sha, "v3 base environment SHA"),
        metadata_values=values, non_metadata_key_names=cast(list[str], base_names),
        expected_environment_sha=expected_environment_sha,
    )


def _v3_worker_runtime_execution(bundle: Bundle, expected: JsonObject, manifest: JsonObject,
                                 context: JsonObject) -> None:
    seen: set[str] = set()
    actual_environment_sha: str | None = None
    for order in _objects(manifest.get("order"), "v3 pair order"):
        role = _text(order.get("arm"), "v3 worker role")
        require(role in {"baseline", "candidate"}, "v3 worker role differs")
        attempts = _objects(order.get("workerAttempts"), "v3 worker attempts")
        if not attempts:
            require(manifest.get("measurement_status") == "nonComparable" and order.get("status") == "missing",
                    "v3 worker execution evidence is missing")
            continue
        seen.add(role)
        for attempt in attempts:
            lifecycle = _validated_noncomparable_attempt_lifecycle(bundle, attempt)
            exit_code = _integer(attempt.get("actualExit"), "v3 worker exit")
            sample = _integer(attempt.get("sample"), "v3 worker sample")
            _validate_worker_lifecycle_identity(expected, order, lifecycle, sample=sample, exit_code=exit_code)
            actual_environment_sha = validate_v3_worker_runtime_execution(
                expected, context, role, attempt, lifecycle,
                expected_environment_sha=actual_environment_sha,
            )
    require(seen == {"baseline", "candidate"}, "v3 worker role execution evidence missing")


def _v3_epoch_validation(bundle: Bundle, root: Path, expected: JsonObject,
                         builds: Mapping[str, JsonObject], *,
                         git_query: GitQuery | None,
                         memory_policy_version: str | None = None) -> SourceEpochV3Validation:
    from html_performance_epoch_reuse import declared_from_authenticated_refs

    pair = _object(expected["sourcePair"], "v3 source pair")
    fixed_pair = fixed_source_pair(expected)
    snapshot_ref = _object(fixed_pair["snapshot"], "v3 source snapshot")
    origin_root = Path(_text(bundle.json(fixed_pair["before"]).get("root"), "fixed epoch root"))
    fixed_context = bundle.json(pair["epochContextRef"])
    fixed_fields = {"schema", "source", "inputs", "toolContext", "programSource"}
    require(set(fixed_context) in (fixed_fields, fixed_fields | {"measurementCapacityBindingRef"}),
            "fixed epoch context fields differ")
    measurement_binding = (_object(fixed_context["measurementCapacityBindingRef"],
                                   "measurement capacity binding")
                           if "measurementCapacityBindingRef" in fixed_context else None)
    endpoints = [bundle.json(pair[key]) for key in ("beforeContextRef", "endContextRef")]
    program_records = _v3_program_source(origin_root, fixed_context, endpoints, git_query=git_query,
                                         memory_policy_version=memory_policy_version)
    endpoint_records = [_v3_live_programs(origin_root, _object(endpoint.get("liveCheckout"), "live checkout"))
                        for endpoint in endpoints]
    all_records = [program_records, *endpoint_records]
    def program_refs(records: SourceRecords) -> dict[str, JsonObject]:
        return {
            path: {"path": str(origin_root / path if path in program_records
                               and item == program_records[path]
                               else origin_root / "tmp/html-performance-programs" / item[0] / path),
                   "sha256": item[0]}
            for path, item in records.items()
        }
    old_expected = {**expected, "sourcePair": fixed_pair}
    dom = bundle.json(expected["domStateRef"])
    capture: JsonObject | None = None
    links: list[JsonObject] = []
    resources: dict[str, JsonObject] = {}
    if expected.get("catalogCondition") is not None:
        capture = {"path": _text(dom.get("capturePath"), "capture path"),
                   "sha256": _text(dom.get("captureSha256"), "capture SHA")}
        evidence = _object(dom.get("operationEvidence"), "operation evidence")
        catalog = _object(expected["catalogCondition"], "catalog condition")
        if catalog.get("operation_kind") == "navigation-all-pointer-clicks":
            links = [{"path": _text(row.get("capturePath"), "link path"),
                      "sha256": _text(row.get("captureSha256"), "link SHA")}
                     for row in _objects(evidence.get("links"), "link captures")]
        for ref in provenance_file_refs(dom, file_resolver=bundle.file):
            resources[_text(ref.get("path"), "resource path")] = ref
    binaries = execution_binary_refs(bundle, expected, builds)
    pins = [_object(pair.get(key), "v3 context pin")
            for key in ("epochContextRef", "beforeContextRef", "endContextRef")]
    projections = [declared_from_authenticated_refs(bundle, origin_root, bundle.json(pin), old_expected,
                   snapshot_ref, capture, links, resources, {}, binaries, program_refs(records),
                   program_records=records, measurement_binding=measurement_binding)
                   for pin, records in zip(pins, all_records, strict=True)]
    return SourceEpochV3Validation(
        _text(pins[0].get("sha256"), "v3 context SHA"),
        _text(pins[1].get("sha256"), "v3 context SHA"),
        _text(pins[2].get("sha256"), "v3 context SHA"),
        cast(JsonObject, projections[0]), cast(JsonObject, projections[1]),
        cast(JsonObject, projections[2]),
    )


def _source_contract_v3(
    bundle: Bundle, root: Path, expected: JsonObject, validation: SourceEpochV3Validation | None,
    *, git_query: GitQuery | None, validated_builds: Mapping[str, JsonObject] | None,
    reuse_context: object, shared_source: SharedCurrentSource | None,
    memory_policy_version: str | None = None,
) -> SourceContract:
    from html_performance_epoch_reuse import EpochProjection, compare_epoch_projections

    if not isinstance(validation, SourceEpochV3Validation):
        raise ValueError("authenticated epoch projection is required")
    contract = _object(expected["sourcePair"], "v3 source pair")
    require(set(contract) == {"schema", "epochSourcePair", "epochContextRef",
                              "beforeContextRef", "endContextRef"}, "v3 source pair fields differ")
    endpoints = _object(expected.get("endpointRefs"), "endpoint references")
    require(contract["beforeContextRef"] == endpoints.get("before")
            and contract["endContextRef"] == endpoints.get("end"), "v3 live context references differ")
    references = [contract["epochContextRef"], contract["beforeContextRef"], contract["endContextRef"]]
    pins = [_object(ref, "v3 context reference") for ref in references]
    checksums = (validation.fixed_context_sha256, validation.before_context_sha256,
                 validation.end_context_sha256)
    require(all(pin.get("sha256") == checksum for pin, checksum in zip(pins, checksums, strict=True)),
            "v3 authenticated context pin differs")
    contexts = [bundle.json(pin) for pin in pins]
    projections = [validation.fixed_projection, validation.before_projection, validation.end_projection]
    for context, projection in zip(contexts, projections, strict=True):
        _validate_epoch_projection_context(context, projection)
    nested = _object(contract.get("epochSourcePair"), "fixed epoch source pair")
    require(contexts[0]["source"] == bundle.json(nested["before"]),
            "fixed epoch source context differs")
    origin_root = Path(_text(bundle.json(nested["before"]).get("root"), "fixed epoch root"))
    for context in contexts[1:]:
        require(context.get("schema") == "krr-html-performance-context-epoch-v3",
                "v3 live context schema differs")
    fixed_programs = _v3_program_source(origin_root, contexts[0], contexts[1:], git_query=git_query,
                                         memory_policy_version=memory_policy_version)
    live_programs = [_v3_live_programs(origin_root, _object(context.get("liveCheckout"), "live checkout"))
                     for context in contexts[1:]]
    changes = frozenset(path for records in live_programs for path in fixed_programs
                        if records[path] != fixed_programs[path])
    compare_epoch_projections(bundle, cast(EpochProjection, projections[0]),
                              bundle, cast(EpochProjection, projections[1]), program_changes=changes)
    compare_epoch_projections(bundle, cast(EpochProjection, projections[1]),
                              bundle, cast(EpochProjection, projections[2]), program_changes=changes)
    original = {**expected, "sourcePair": nested}
    return source_contract(bundle, root, original, git_query=git_query,
                           validated_builds=validated_builds, reuse_context=reuse_context,
                           shared_source=shared_source)


def source_contract(bundle: Bundle, root: Path, expected: JsonObject, *,
                    git_query: GitQuery | None = None,
                    validated_builds: Mapping[str, JsonObject] | None = None,
                    reuse_context: object = None,
                    shared_source: SharedCurrentSource | None = None,
                    epoch_validation: SourceEpochV3Validation | None = None,
                    memory_policy_version: str | None = None) -> SourceContract:
    contract = _object(expected["sourcePair"], "source pair")
    if contract.get("schema") == "krr-html-performance-source-epoch-v3":
        return _source_contract_v3(bundle, root, expected, epoch_validation, git_query=git_query,
                                   validated_builds=validated_builds, reuse_context=reuse_context,
                                   shared_source=shared_source, memory_policy_version=memory_policy_version)
    require(epoch_validation is None, "epoch validation requires v3 source schema")
    before, end, approved = (bundle.json(contract[k]) for k in ("before", "end", "approvedSourceRef"))
    require(all(document.get("stableEndpoints") is True for document in (before, end, approved)), "unstable source endpoint")
    require(all(before[k] == end[k] == approved[k] for k in SOURCE_KEYS), "source epoch differs")
    require(before["inventoryDigest"] == contract["approvedInventoryDigest"], "approved inventory differs")
    approved_ref = _object(contract["approvedSourceRef"], "approved source reference")
    matches = [p for p in _objects(contract["inputPins"], "source input pins") if p.get("path") == approved_ref["path"]]
    require(len(matches) == 1 and matches[0].get("sha256") == approved_ref["sha256"], "approved source pin missing")
    saved = _source_for_validation(root, before, git_query=git_query, reuse_context=reuse_context,
                                   shared_source=shared_source)
    snapshot = bundle.json(contract["snapshot"])
    rows = snapshot["records"]
    actual = records_map(rows)
    require(set(actual) == set(saved) and all(actual[p] == value for p, value in saved.items()), "snapshot source subset/extra")
    require(actual[HARNESS_ROOT + "Cargo.lock"][0] == LOCK_SHA, "harness lock differs")
    require(snapshot["digest"] == hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), "snapshot digest differs")
    snapshot_ref = _object(contract["snapshot"], "source snapshot reference")
    expected_baseline = baseline_arm_id(expected)
    for raw_arm, arm_value in _object(expected["arms"], "expected arms").items():
        arm = _object(arm_value, "expected arm")
        if raw_arm == expected_baseline and expected_baseline != DEFAULT_BASELINE_ARM_ID:
            if validated_builds is None or "baseline" not in validated_builds:
                raise ValueError("verified reference parent build is missing")
            build = validated_builds["baseline"]
            require(arm["source_snapshot_sha256"] == build["sourceSnapshotSha256"]
                    and arm["lock_sha256"] == build["lockSha256"],
                    "reference parent build provenance differs")
        else:
            require(arm["source_snapshot_sha256"] == snapshot_ref["sha256"]
                    and arm["lock_sha256"] == LOCK_SHA, "shared build provenance differs")
    return {"head": _text(before["head"], "source HEAD"), "source": saved, "snapshot": _text(snapshot_ref["sha256"], "snapshot SHA256")}


PHASES = ("cold_open", "warm_open", "re_display", "full_y", "post_operation", "same_session_resize")
DISPLAY_COMPLETION_PHASES = (
    "cold_open",
    "warm_open",
    "re_display",
    "post_operation",
    "same_session_resize",
)
DISPLAY_COMPLETION_LIMIT_MS = 3_000.0


def _number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise ValueError(f"{label} must be finite and positive")
    return number


def _nonnegative_number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise ValueError(f"{label} must be finite and nonnegative")
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


def _is_proven_zero_scroll_full_y(sample: JsonObject, value: JsonObject) -> bool:
    frame = value.get("frame")
    viewport = sample.get("viewport")
    frame_viewport = frame.get("viewport") if isinstance(frame, dict) else None
    content_height = frame.get("content_height") if isinstance(frame, dict) else None
    if (
        not isinstance(viewport, dict)
        or not isinstance(frame_viewport, dict)
        or frame_viewport != viewport
    ):
        return False
    height = viewport.get("height")
    scale = _finite_number(viewport.get("scale"))
    content_height_number = _finite_number(content_height)
    if (
        type(height) is not int
        or height <= 0
        or scale is None
        or scale <= 0
        or content_height_number is None
        or content_height_number <= 0
    ):
        return False
    logical_height = height / scale
    return (
        value.get("actual_bottom") is True
        and type(value.get("scroll_steps")) is int
        and value["scroll_steps"] == 0
        and _is_finite_zero(value.get("max_scroll"))
        and _is_finite_zero(value.get("final_scroll_y"))
        and _is_finite_zero(value.get("max_frame_elapsed_ms"))
        and isinstance(frame, dict)
        and _is_finite_zero(frame.get("scroll_y"))
        and math.isfinite(logical_height)
        and math.floor(content_height_number + 0.5) <= logical_height
    )


def _finite_number(value: object) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    number = float(value)
    return number if math.isfinite(number) else None


def _is_finite_zero(value: object) -> bool:
    number = _finite_number(value)
    return number is not None and number == 0


def _phase_sample(sample: JsonObject, phase: str) -> tuple[float, float]:
    phases = sample.get("phases")
    if not isinstance(phases, dict) or phase not in phases:
        raise ValueError(f"sample is missing phase {phase}")
    value = phases[phase]
    if phase in ("post_operation", "same_session_resize"):
        if not isinstance(value, dict):
            raise ValueError("post_operation must be an object")
        value = value.get("measurement")
    if not isinstance(value, dict):
        raise ValueError(f"phase {phase} must be an object")
    raw_elapsed = value.get("elapsed_ms")
    zero_scroll_full_y = phase == "full_y" and _is_proven_zero_scroll_full_y(sample, value)
    elapsed = (
        0.0
        if zero_scroll_full_y and _is_finite_zero(raw_elapsed)
        else _number(raw_elapsed, f"{phase}.elapsed_ms")
    )
    rss = _number(value.get("peak_rss_bytes"), f"{phase}.peak_rss_bytes")
    return elapsed, rss


def _validate_report(
    report: JsonObject, expected: JsonObject, *, allow_empty_samples: bool = False,
) -> tuple[str, list[JsonObject]]:
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
    catalog = expected.get("catalogCondition")
    if catalog is not None:
        identity = _object(catalog, "expected catalog condition")
        evidence_ref = _object(expected["operationEvidenceRef"], "operation evidence reference")
        if (
            report.get("condition_id") != identity["condition_id"]
            or report.get("case_id") != identity["case_id"]
            or report.get("state") != identity["state"]
            or report.get("operation_kind") != identity["operation_kind"]
            or report.get("operation_evidence_ref") != evidence_ref
            or report.get("catalog_condition") != identity
        ):
            raise ValueError("report catalog condition identity differs")
    samples = report.get("samples")
    if not isinstance(samples, list) or (not samples and not allow_empty_samples):
        raise ValueError("report has no samples")
    if report.get("sample_count") != len(samples):
        raise ValueError("sample_count does not match samples")
    sample_objects = _objects(samples, "sample")
    for sample in sample_objects:
        for phase in PHASES:
            elapsed, _ = _phase_sample(sample, phase)
            require(arm != "candidate" or phase not in DISPLAY_COMPLETION_PHASES
                    or elapsed < DISPLAY_COMPLETION_LIMIT_MS,
                    f"candidate {phase} frame completion must be under 3000 ms")
        full_y = _object(_object(sample["phases"], "sample phases")["full_y"], "full-Y phase")
        if arm == "candidate":
            maximum_frame_elapsed = _nonnegative_number(
                full_y.get("max_frame_elapsed_ms"), "full-Y max frame elapsed"
            )
            require(maximum_frame_elapsed < DISPLAY_COMPLETION_LIMIT_MS,
                    "candidate full-Y frame completion must be under 3000 ms")
            scroll_steps = _integer(full_y["scroll_steps"], "full-Y scroll steps")
            max_scroll = _nonnegative_number(full_y.get("max_scroll"), "full-Y max scroll")
            final_scroll = _nonnegative_number(full_y.get("final_scroll_y"), "full-Y final scroll")
            bottom_frame = _object(full_y.get("frame"), "full-Y bottom frame")
            frame_scroll = _nonnegative_number(
                bottom_frame.get("scroll_y"), "full-Y bottom frame scroll"
            )
            full_y_ok = (
                full_y["actual_bottom"] is True
                and max_scroll == final_scroll == frame_scroll
                and scroll_steps >= 0
                and (
                    (scroll_steps > 0 and max_scroll > 0)
                    or (catalog is not None and scroll_steps == 0 and max_scroll == 0)
                )
            )
            require(full_y_ok, "full-Y span incomplete")
        resize = _object(_object(sample["phases"], "sample phases")["same_session_resize"],
                         "same-session resize")
        evidence = _object(resize.get("resize_evidence"), "resize evidence")
        from_viewport = _object(evidence.get("from_viewport"), "resize source viewport")
        to_viewport = _object(evidence.get("to_viewport"), "resize target viewport")
        from_width = _integer(from_viewport.get("width"), "resize source width")
        from_height = _integer(from_viewport.get("height"), "resize source height")
        resize_width = _integer(to_viewport.get("width"), "resize target width")
        resize_height = _integer(to_viewport.get("height"), "resize target height")
        generation_before = _integer(evidence.get("generation_before"), "resize generation before")
        generation_after = _integer(evidence.get("generation_after"), "resize generation after")
        require(from_viewport == expected["viewport"] and to_viewport != from_viewport,
                "same-session resize viewport transition differs")
        expected_resize_width = (from_width - max(from_width // 4, 1)) if from_width > 1 else 2
        require(to_viewport.get("scale") == from_viewport.get("scale")
                and resize_width == expected_resize_width and resize_height == from_height,
                "same-session resize target viewport is invalid")
        require(generation_after > generation_before
                and evidence.get("real_raster_frame") is True,
                "same-session resize did not produce a new raster frame")
        operation = _object(_object(sample["phases"], "sample phases")["post_operation"], "sample operation")
        catalog = expected.get("catalogCondition")
        if catalog is None:
            if operation["operation"] != expected["operation"]:
                raise ValueError("sample operation does not match operation plan")
            if operation["expected_dom_state_sha256"] != expected["expected_dom_state_sha256"]:
                raise ValueError("sample expected-state SHA does not match operation plan")
        else:
            identity = _object(catalog, "expected catalog condition")
            if sample.get("catalog_condition") != {
                "profile": "catalog-plan84",
                "condition_id": identity["condition_id"],
                "case_id": identity["case_id"],
                "state": identity["state"],
                "operation_kind": identity["operation_kind"],
                "operation_evidence_sha256": _object(expected["operationEvidenceRef"], "operation evidence reference")["sha256"],
            }:
                raise ValueError("sample catalog condition identity differs")
    return arm, sample_objects


def _validate_noncomparable_baseline_order(
    bundle: Bundle, expected: JsonObject, row: JsonObject, report: JsonObject,
    pair: int, fragment: str, failure: JsonObject,
) -> None:
    condition = _object(expected.get("catalogCondition"), "non-comparable catalog condition")
    condition_id = _text(condition.get("condition_id"), "non-comparable condition id")
    baseline_id = baseline_arm_id(expected)
    arm, samples = _validate_report(report, expected, allow_empty_samples=True)
    require(arm == baseline_id and samples == [] and report.get("sample_count") == 0
            and report.get("measurement_executed") is False,
            "non-comparable baseline report must contain no measured samples")
    require(row.get("arm") == "baseline" and row.get("pair_index") == pair
            and row.get("workerLoads") == [],
            "non-comparable baseline order contains measured worker loads")
    if pair > 0:
        missing: JsonObject = {
            "status": "missing",
            "reason": "baseline skipped after known fragment refusal",
            "afterFailure": failure,
        }
        require(row.get("load_ratio_before") is None and row.get("load_ratio_after") is None
                and row.get("status") == "missing" and row.get("failure") == missing
                and report.get("failure") == missing and row.get("workerAttempts") == [],
                "non-comparable skipped baseline row differs")
        return
    expected_failure: JsonObject = {
        "status": "nonComparable",
        "reason": "known baseline fragment navigation refusal",
        "conditionId": condition_id,
        "fragment": fragment,
        "sample": 0,
        "attempt": 1,
        "actualExit": 1,
        "errorType": "CalledProcessError",
    }
    require(failure == expected_failure and row.get("failure") == failure
            and report.get("failure") == failure and row.get("status") == "nonComparable",
            "non-comparable baseline failure classification differs")
    attempts = _objects(row.get("workerAttempts"), "non-comparable worker attempts")
    require(len(attempts) == 1, "known fragment refusal requires one failed worker attempt")
    _validate_noncomparable_worker_attempt(bundle, expected, row, attempts[0], fragment)


def _validate_noncomparable_worker_attempt(
    bundle: Bundle, expected: JsonObject, row: JsonObject,
    attempt: JsonObject, fragment: str,
) -> None:
    require(attempt.get("sample") == 0 and attempt.get("attempt") == 1
            and attempt.get("actualExit") == 1 and attempt.get("workerStarted") is True
            and attempt.get("accepted") is False
            and attempt.get("reason") == "worker command exited nonzero",
            "known baseline worker attempt identity differs")
    expected_arms = _object(expected.get("arms"), "expected arms")
    expected_arm = _object(expected_arms.get(baseline_arm_id(expected)),
                           "expected baseline arm")
    binary_sha = expected_arm.get("binary_sha256")
    require(attempt.get("binarySha256Before") == binary_sha
            and attempt.get("binarySha256After") == binary_sha,
            "known baseline worker binary differs")
    before = _number(attempt.get("loadRatioBefore"), "baseline worker load before")
    after = _number(attempt.get("loadRatioAfter"), "baseline worker load after")
    require(0 <= before <= 0.25 and 0 <= after <= 0.25,
            "known baseline worker load exceeded the quiet limit")
    lifecycle = _validated_noncomparable_attempt_lifecycle(bundle, attempt)
    _validate_noncomparable_lifecycle(bundle, expected, row, lifecycle, fragment)
    failed_loads = _objects(row.get("failedWorkerLoads"), "failed baseline worker loads")
    require(len(failed_loads) == 1
            and failed_loads[0] == {"before": before, "after": after}
            and row.get("load_ratio_before") == before
            and row.get("load_ratio_after") == after,
            "known baseline worker load observation differs")


def _validated_noncomparable_attempt_lifecycle(
    bundle: Bundle, attempt: JsonObject,
) -> JsonObject:
    record_ref = _object(attempt.get("attemptRecord"), "worker attempt record reference")
    record = read_json(bundle.file(record_ref))
    require(record == {key: value for key, value in attempt.items() if key != "attemptRecord"},
            "known baseline worker attempt record differs")
    lifecycle_ref = _object(attempt.get("lifecycle"), "worker lifecycle reference")
    require(record.get("lifecycle") == lifecycle_ref,
            "worker attempt lifecycle reference differs")
    return read_json(bundle.file(lifecycle_ref))


def _validate_noncomparable_lifecycle(
    bundle: Bundle, expected: JsonObject, row: JsonObject,
    lifecycle: JsonObject, fragment: str,
) -> None:
    _validate_worker_lifecycle_identity(expected, row, lifecycle, sample=0, exit_code=1)
    _validate_noncomparable_raw_streams(bundle, lifecycle, fragment)


def _validate_worker_lifecycle_identity(
    expected: JsonObject, row: JsonObject, lifecycle: JsonObject, *, sample: int, exit_code: int,
) -> None:
    identity = _object(lifecycle.get("identity"), "worker lifecycle identity")
    pid = identity.get("pid")
    argv = _array(lifecycle.get("argv"), "worker lifecycle argv")
    catalog = _object(expected.get("catalogCondition"), "catalog condition")
    viewport = _object(expected.get("viewport"), "worker viewport")
    operation_plan = _object(expected.get("operationPlanRef"), "operation plan reference")
    require(
        lifecycle.get("role") == "worker" and lifecycle.get("exitCode") == exit_code
        and type(pid) is int and pid > 0
        and lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
        and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None
        and len(argv) == 9 and argv[0] == row.get("executable")
        and argv[1:3] == ["--worker", str(sample)]
        and argv[3] == catalog.get("fixture_path") and argv[4] == row.get("output")
        and argv[5] == operation_plan.get("path")
        and argv[6:] == [str(viewport["width"]), str(viewport["height"]),
                         str(viewport["scale"])],
        "worker lifecycle/argv differs",
    )


def _validate_noncomparable_raw_streams(
    bundle: Bundle, lifecycle: JsonObject, fragment: str,
) -> None:
    raw = _objects(lifecycle.get("raw"), "worker lifecycle raw streams")
    require(len(raw) == 2, "known baseline worker raw stream count differs")
    stream_data: dict[str, bytes] = {}
    for item in raw:
        pin = {"path": item.get("path"), "sha256": item.get("sha256")}
        path = bundle.file(pin)
        if path.name == "stdout.raw":
            stream_data["stdout"] = path.read_bytes()
        elif path.name == "stderr.raw":
            stream_data["stderr"] = path.read_bytes()
    expected_error = (
        f'Error: RuntimeFailure {{ error: "navigation is not allowed: {fragment}" }}'
    ).encode()
    require(set(stream_data) == {"stdout", "stderr"}
            and stream_data["stderr"].splitlines() == [expected_error],
            "known baseline fragment refusal diagnostic differs")


def _validated_attempt_summary(
    expected: JsonObject, source: SourceContract, comparison_status: str,
    candidate_samples: int | None = None,
    sample_records: int | None = None,
    phase_records: int | None = None,
) -> JsonObject:
    summary: JsonObject = {
        "viewport": expected["viewport"], "source": source,
        "fixture": expected["fixture_sha256"], "operation": expected["operation"],
        "plan": expected["operation_plan_sha256"],
        "dom": expected["expected_dom_state_sha256"],
        "provenance": expected["provenance"], "arms": expected["arms"],
        "comparisonStatus": comparison_status,
    }
    if candidate_samples is not None:
        summary["candidateSamples"] = candidate_samples
    if sample_records is not None:
        summary["sampleRecords"] = sample_records
    if phase_records is not None:
        summary["phaseRecords"] = phase_records
    catalog_keys = (
        "catalogCondition", "catalogManifestRef", "catalogPlanRef",
        "operationEvidenceRef", "catalogCaptureValidationRef",
        "catalogBinaryProofRef", "catalogBuildReportRef", "catalogCaptureBinaryRef",
        "catalogCurrentBuildReportRef", "chromeCaptureRef", "chromeReceiptRef",
        "fontIdentityRequestRef", "fontIdentityResponseRef", "fontIdentityProbeBinary",
    )
    for key in catalog_keys:
        if key in expected:
            summary[key] = expected[key]
    return summary


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
    grouped = _group_reports(report_paths, expected)
    return _aggregate_grouped_reports(grouped, expected)


def _group_reports(report_paths: list[Path], expected: JsonObject) -> dict[str, dict[int, JsonObject]]:
    if len(report_paths) != 10 or len({path.resolve() for path in report_paths}) != 10:
        raise ValueError("exactly ten distinct pair reports are required")
    grouped: dict[str, dict[int, JsonObject]] = {"baseline": {}, "candidate": {}}
    report_arm_for_filename = {"baseline": baseline_arm_id(expected), "candidate": "candidate"}
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
    return grouped


def _aggregate_grouped_reports(
    grouped: dict[str, dict[int, JsonObject]], expected: JsonObject
) -> ComparisonAggregate:
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


def evaluate_measurement_policy(
    report_paths: list[Path], expected: Mapping[str, object]
) -> JsonObject:
    """raw reportからHTML性能のcanonical paired policyを再計算する。"""
    expected_object = cast(JsonObject, expected)
    grouped = _group_reports(report_paths, expected_object)
    aggregate = _aggregate_grouped_reports(grouped, expected_object)
    tests: dict[str, dict[str, JsonObject]] = {}
    for phase in PHASES:
        tests[phase] = {}
        for metric in ("elapsed_ms", "peak_rss_bytes"):
            baseline = [
                _phase_sample(sample, phase)[0 if metric == "elapsed_ms" else 1]
                for pair in range(5)
                for sample in _objects(grouped["baseline"][pair]["samples"], "baseline samples")
            ]
            candidate = [
                _phase_sample(sample, phase)[0 if metric == "elapsed_ms" else 1]
                for pair in range(5)
                for sample in _objects(grouped["candidate"][pair]["samples"], "candidate samples")
            ]
            tests[phase][metric] = evaluate_paired_regression(baseline, candidate).to_json()
    return {
        "version": "paired-wilcoxon-family-v1",
        "familyComparisons": 84 * 6 * 2,
        "familyAlpha": 0.05,
        "comparisonAlpha": 0.05 / (84 * 6 * 2),
        "tests": tests,
        "policyPass": all(
            test["detectedBad"] is False
            for phase in tests.values()
            for test in phase.values()
        ),
        "aggregate": aggregate,
        "nullDifference": 0,
        "quietLoadLimit": 0.25,
        "phaseRecords": len(PHASES) * 50,
    }



def _require_v2_memory_policy(
    bundle: Bundle, expected: JsonObject, result: JsonObject,
    paths: list[Path], *, non_comparable: bool,
    policy_version: str | None = None,
    assessment_sink: list[JsonObject] | None = None,
    policy_sink: list[JsonObject] | None = None,
    allow_rejected: bool = False,
) -> bool:
    if "memoryPolicy" not in result:
        return False
    retention_ref = _object(expected.get("memoryRetentionRef"), "memory retention reference")
    saved = _object(result["memoryPolicy"], "memory policy")
    from html_performance_memory_evidence import evaluate_candidate_only, evaluate_paired
    from html_performance_memory_policy import SUPPORTED_POLICY_VERSIONS
    from html_performance_retention_evidence import read_retained_rss

    retained = read_retained_rss(bundle, expected)
    original_version = _text(saved.get("version"), "saved memory policy version")
    require(original_version in SUPPORTED_POLICY_VERSIONS,
            "unsupported saved memory policy version")
    evaluator = evaluate_candidate_only if non_comparable else evaluate_paired
    canonical = evaluator(paths, expected, retained, policy_version=original_version)
    require(saved == canonical, "saved memory policy differs from authenticated evidence")
    requested = original_version if policy_version is None else policy_version
    require(requested in SUPPORTED_POLICY_VERSIONS, "unsupported memory policy version")
    if requested != original_version:
        canonical = evaluator(paths, expected, retained, policy_version=requested)
    if policy_sink is not None:
        policy_sink.append(canonical)
    accepted = (canonical.get("candidateRetentionPass") is True if non_comparable
                else canonical.get("policyPass") is True)
    if assessment_sink is not None:
        catalog = _object(expected.get("catalogCondition"), "memory catalog condition")
        retention_sha = _text(retention_ref.get("sha256"), "retention SHA")
        digest(retention_sha)
        assessment_sink.append({
            "conditionId": _text(catalog.get("condition_id"), "memory condition id"),
            "version": requested, "policySha256": _v3_json_digest(canonical),
            "retentionSha256": retention_sha,
            "originalPolicyVersion": original_version,
            "originalPolicySha256": _v3_json_digest(saved),
        })
    if allow_rejected:
        return accepted
    if non_comparable:
        require(accepted,
                "candidate retained memory regression")
    else:
        require(accepted, "paired time/memory regression")
    return True


def validate_measurement_sample(
    sample: JsonObject, expected: JsonObject, plan: JsonObject, *, candidate: bool,
) -> None:
    """Validate the frame and operation evidence of one real worker sample."""
    viewport = _object(expected["viewport"], "viewport")
    require(sample.get("viewport") == viewport, "worker viewport differs")
    phases = _object(sample.get("phases"), "sample phases")
    require(set(phases) == set(PHASES), "incomplete sample/phase coverage")
    for phase in PHASES:
        measured = _object(phases[phase], "measured phase")
        if phase in ("post_operation", "same_session_resize"):
            measured = _object(measured["measurement"], "post-operation measurement")
        frame = _object(measured["frame"], "measured frame")
        expected_frame_viewport = viewport
        if phase == "same_session_resize":
            resize = _object(phases[phase], "same-session resize")
            resize_evidence = _object(resize["resize_evidence"], "resize evidence")
            expected_frame_viewport = _object(resize_evidence["to_viewport"], "resize target viewport")
        expected_pixels = (_integer(expected_frame_viewport["width"], "frame viewport width")
                           * _integer(expected_frame_viewport["height"], "frame viewport height") * 4)
        require(frame["viewport"] == expected_frame_viewport and frame["pixel_bytes"] == expected_pixels,
                "actual rendered viewport differs")
    if candidate:
        _validate_candidate_operation(phases, expected, plan)


def _validate_candidate_operation(phases: JsonObject, expected: JsonObject, plan: JsonObject) -> None:
    operation = _object(phases["post_operation"], "post operation")
    catalog_expected = expected.get("catalogCondition")
    if catalog_expected is None:
        digest(operation["pixel_sha_before"])
        digest(operation["pixel_sha_after"])
        require(_numeric(operation["generation_after"], "generation after") > _numeric(operation["generation_before"], "generation before") and operation["pixel_sha_before"] != operation["pixel_sha_after"] and operation["visual_change_observed"] is True, "operation did not produce a changed frame")
        return
    operation_evidence = _object(operation.get("catalog_operation"), "catalog operation result")
    catalog = _object(catalog_expected, "catalog condition")
    require(operation_evidence.get("condition_id") == catalog["condition_id"]
            and operation_evidence.get("case_id") == catalog["case_id"]
            and operation_evidence.get("state") == catalog["state"]
            and operation_evidence.get("operation_kind") == catalog["operation_kind"]
            and operation_evidence.get("operation_evidence_sha256") == _object(expected["operationEvidenceRef"], "operation evidence ref")["sha256"],
            "catalog post-operation identity differs")
    if not plan["operationStream"]:
        require(operation_evidence.get("delivery") == "measured-close-and-new-session-open",
                "catalog static state did not use a measured close/reopen")
    else:
        dispatched = _array(operation_evidence.get("dispatched_inputs"), "dispatched catalog inputs")
        expected_kinds = [{"pointer_move": "pointer_move", "pointer_down": "pointer_down",
                           "pointer_up": "pointer_up", "text": "text", "key_down": "key_down",
                           "key_up": "key_up", "scroll": "scroll", "advance_time": "advance_time"}[_text(_object(item, "catalog operation stream item")["kind"], "catalog operation kind")]
                          for item in _array(plan["operationStream"], "operation stream")]
        require(dispatched == expected_kinds, "catalog dispatched inputs differ from observed operation stream")


def validate_attempt(bundle: Bundle, root: Path, attempt: JsonObject, *,
                     git_query: GitQuery | None = None,
                     validated_builds: Mapping[str, JsonObject] | None = None,
                     reuse_context: object = None,
                     remote_context: object = None,
                     shared_source: SharedCurrentSource | None = None,
                     memory_policy_version: str | None = None,
                     memory_assessment_sink: list[JsonObject] | None = None,
                     memory_acceptance_sink: list[bool] | None = None,
                     allow_memory_rejected: bool = False) -> JsonObject:
    expected = bundle.json(attempt["expected"])
    result = bundle.json(attempt["result"])
    manifest = bundle.json(attempt["pairManifest"])
    source_pair_contract = _object(expected["sourcePair"], "source pair")
    is_v3 = source_pair_contract.get("schema") == "krr-html-performance-source-epoch-v3"
    require(result.get("measurementExecuted") is True
            and (is_v3 or all(result.get(k) is True for k in ENDPOINT_FLAGS)),
            "measurement/endpoint proof absent or false")
    require(not result.get("failure") and not result.get("endpointFailure"), "failed measurement attempt")
    expected_arms = _object(expected["arms"], "expected arms")
    provenance = _object(expected["provenance"], "expected provenance")
    source_pair = fixed_source_pair(expected)
    recorded_root = root
    if is_v3:
        recorded_root = Path(_text(bundle.json(source_pair["before"])["root"], "fixed source root"))
    if remote_context is not None:
        from html_performance_remote_admission import RemoteEvidenceContext
        require(isinstance(remote_context, RemoteEvidenceContext)
                and remote_context.manifest_sha256 == sha256(bundle.directory / "bundle.json"),
                "remote evidence context differs")
        recorded_root = Path(_text(bundle.json(source_pair["before"])["root"], "recorded source root"))
    expected_baseline = baseline_arm_id(expected)
    require(set(expected_arms) == {expected_baseline, "candidate"}, "versioned baseline/candidate required")
    catalog_expected = expected.get("catalogCondition")
    for key in ("fixture_sha256", "operation_plan_sha256"):
        digest(expected[key])
    if catalog_expected is None:
        digest(expected["expected_dom_state_sha256"])
    for arm_value in expected_arms.values():
        expected_arm = _object(arm_value, "expected arm")
        for key in ("binary_sha256", "source_snapshot_sha256", "lock_sha256"):
            digest(expected_arm[key])
    if validated_builds is not None:
        require(set(validated_builds) == {"baseline", "candidate"}, "validated build arms differ")
        for raw_arm, arm in ((expected_baseline, "baseline"), ("candidate", "candidate")):
            expected_arm = _object(expected_arms[raw_arm], "expected arm")
            build = validated_builds[arm]
            build_ref = _object(expected_arm.get("build_artifact_ref"), "build artifact reference")
            require(bundle.file(build_ref).is_file()
                    and build["binarySha256"] == expected_arm["binary_sha256"]
                    and build["sourceSnapshotSha256"] == expected_arm["source_snapshot_sha256"]
                    and build["lockSha256"] == expected_arm["lock_sha256"],
                    "expected arm differs from validated build artifact")
    digest(provenance["environment_sha256"])
    for tool in ("rustc", "cargo"):
        tool_version = _text(provenance[tool], "tool provenance")
        require(tool_version.startswith(tool + " ") and "\n" in tool_version, "actual tool provenance absent")
    viewport = _object(expected["viewport"], "viewport")
    width, height = (_integer(viewport[key], "viewport dimension") for key in ("width", "height"))
    require(set(viewport) == {"width", "height", "scale"} and type(viewport["scale"]) in (int, float) and viewport["scale"] == 1, "invalid viewport types/scale")
    require(isinstance(expected["operation"], str) and bool(expected["operation"]), "missing operation")
    endpoints = _object(expected["endpointRefs"], "endpoint references")
    before_context, end_context = (bundle.json(endpoints[k]) for k in ("before", "end"))
    epoch_validation: SourceEpochV3Validation | None = None
    if is_v3:
        require(remote_context is None and shared_source is None, "unsupported v3 source context")
        reuse_context, candidate_build = _v3_origin_and_endpoints(
            bundle, root, expected, result, before_context, end_context, git_query=git_query,
        )
        fixed_expected = {**expected, "sourcePair": source_pair,
                          "endpointRefs": {"before": source_pair_contract["epochContextRef"],
                                           "end": source_pair_contract["epochContextRef"]}}
        contract = fixed_expected.get("referenceParent")
        if isinstance(contract, dict) and contract.get("referenceKind") == "fresh-history-source":
            from html_performance_fresh_reference import validate_remote_fresh_reference
            head = _text(bundle.json(source_pair["before"]).get("head"), "fixed source HEAD")
            validated_builds = {
                "baseline": validate_remote_fresh_reference(bundle, root, fixed_expected, head),
                "candidate": candidate_build,
            }
            require(all(validated_builds["baseline"].get(key) == validated_builds["candidate"].get(key)
                        for key in ("cargoExecutableSha256", "rustcExecutableSha256",
                                    "buildEnvironmentSha256", "toolVersions", "cargoConfigPins",
                                    "cargoLauncher", "rustcLauncher", "guardPython")),
                    "v3 fresh reference and candidate build environments differ")
        else:
            require(baseline_arm_id(fixed_expected) != DEFAULT_BASELINE_ARM_ID,
                    "v3 baseline requires an authenticated historical reference")
            validated_builds = {
                "baseline": _validate_reference_parent_wrapper(bundle, root, fixed_expected,
                                                                 git_query=git_query),
                "candidate": candidate_build,
            }
        _v3_worker_observations(bundle, expected, before_context, validated_builds)
        _v3_worker_observations(bundle, expected, end_context, validated_builds)
        _v3_worker_runtime_execution(bundle, expected, manifest, before_context)
        epoch_validation = _v3_epoch_validation(bundle, root, expected, validated_builds,
                                                git_query=git_query,
                                                memory_policy_version=memory_policy_version)
    source = source_contract(bundle, root, expected, git_query=git_query,
                             validated_builds=validated_builds, reuse_context=reuse_context,
                             shared_source=shared_source, epoch_validation=epoch_validation,
                             memory_policy_version=memory_policy_version)
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    if not is_v3:
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
    link_capture_refs: list[JsonObject] = []
    require(operation_ref["sha256"] == expected["operation_plan_sha256"], "operation plan pin differs")
    if catalog_expected is None:
        require(dom_ref["sha256"] == expected["expected_dom_state_sha256"], "DOM pin differs")
        require(plan["fixture_sha256"] == expected["fixture_sha256"] and plan["operation"] == expected["operation"] and plan["expected_dom_state_sha256"] == expected["expected_dom_state_sha256"] and plan["expected_dom_state_path"] == dom_ref["path"], "operation plan differs")
        require(source["source"]["tests/fixtures/html/interaction-fidelity.html"][0] == expected["fixture_sha256"], "performance fixture is not committed source")
        require(dom["inputSha256"] == expected["fixture_sha256"] and _object(dom["states"], "DOM states")["js"] == {"expanded": True, "result": "opened"}, "actual interaction DOM evidence absent")
    else:
        catalog = _object(catalog_expected, "catalog condition")
        evidence_ref = _object(expected.get("operationEvidenceRef"), "operation evidence reference")
        require(dom_ref == evidence_ref and plan.get("profile") == "catalog-plan84"
                and plan.get("conditionId") == catalog["condition_id"]
                and plan.get("caseId") == catalog["case_id"]
                and plan.get("state") == catalog["state"]
                and plan.get("operationKind") == catalog["operation_kind"]
                and plan.get("fixtureSha256") == expected["fixture_sha256"]
                and plan.get("operationEvidenceSha256") == evidence_ref["sha256"],
                "catalog operation plan/evidence binding differs")
        fixture_relative = Path(_text(catalog["fixture_path"], "catalog fixture path")).relative_to(recorded_root).as_posix()
        require(source["source"].get(fixture_relative, (None,))[0] == expected["fixture_sha256"],
                "catalog fixture is not committed source")
        require(dom.get("status") == "observed" and dom.get("conditionId") == catalog["condition_id"]
                and dom.get("caseId") == catalog["case_id"] and dom.get("boundaryState") == catalog["state"]
                and dom.get("operationKind") == catalog["operation_kind"]
                and dom.get("htmlSha256") == expected["fixture_sha256"],
                "catalog operation evidence is not observed or is mismatched")
        raw_viewport = _object(dom.get("viewport"), "raw catalog viewport")
        evidence_path = _text(evidence_ref["path"], "operation evidence path")
        evidence_sha = _text(evidence_ref["sha256"], "operation evidence SHA")
        operation_evidence = _object(dom.get("operationEvidence"), "raw operation evidence")
        require(raw_viewport == viewport
                and dom.get("binarySha256") == plan.get("evidenceBinarySha256")
                and manifest.get("binarySha256") == plan.get("evidenceBinarySha256")
                and plan.get("operationEvidencePath") == evidence_path
                and plan.get("operationEvidenceSha256") == evidence_sha
                and operation_evidence.get("schema") == "krr-html84-operation-evidence-v1"
                and operation_evidence.get("kind") == catalog.get("operation_kind")
                and operation_evidence.get("sourceSha256Before") == expected["fixture_sha256"]
                and operation_evidence.get("sourceSha256After") == expected["fixture_sha256"]
                and plan.get("operationStream") == catalog_operation_stream(
                    _text(catalog["operation_kind"], "catalog operation kind"), operation_evidence,
                    ((_fixture_expected_state(bundle.file({"path": catalog["fixture_path"],
                                                           "sha256": expected["fixture_sha256"]}))
                      if is_v3 else _fixture_expected_state(root / fixture_relative))
                     if catalog.get("operation_kind") in {
                         "script-await-timer-microtask", "script-await-mutation-delivery",
                         "script-real-events", "script-await-resource-font",
                     } else None),
                    dom.get("state"),
                ),
                "catalog raw report, binary, or replayable operation stream differs")
        capture_path = _text(dom.get("capturePath"), "raw report capture path")
        capture_sha = _text(dom.get("captureSha256"), "raw report capture SHA")
        digest(capture_sha)
        capture_ref: JsonObject = {"path": capture_path, "sha256": capture_sha}
        bundle.file(capture_ref)
        if catalog.get("operation_kind") == "navigation-all-pointer-clicks":
            links = _objects(operation_evidence.get("links"), "raw TOC links")
            for link in links:
                link_path = _text(link.get("capturePath"), "TOC capture path")
                link_sha = _text(link.get("captureSha256"), "TOC capture SHA")
                digest(link_sha)
                reference: JsonObject = {"path": link_path, "sha256": link_sha}
                bundle.file(reference)
                link_capture_refs.append(reference)
            require(len(link_capture_refs) == 3, "catalog TOC capture count differs")
        target = _object(operation_evidence.get("target"), "raw operation target")
        require(target.get("visible") is True
                and _integer(target.get("nodeId"), "raw target node") > 0
                and (catalog.get("target_selector") is None
                     or target.get("selector") == catalog.get("target_selector")),
                "catalog raw operation target differs")
    provenance_refs = (provenance_file_refs(dom, file_resolver=bundle.file if remote_context is not None or is_v3 else None)
                       if catalog_expected is not None else [])
    binary_refs = execution_binary_refs(bundle, expected, validated_builds)
    context_inputs = validate_context_input_refs(
        before_context, expected, _object(source_pair["snapshot"], "snapshot reference"),
        capture_ref if catalog_expected is not None else None, link_capture_refs, provenance_refs,
        planned_execution_inputs(recorded_root if catalog_expected is not None else root, expected, binary_refs),
    )
    require(manifest["measurement_executed"] is True and manifest["input_sha256"] == expected["fixture_sha256"] and manifest["environment_sha256"] == provenance["environment_sha256"], "runner context differs")
    if catalog_expected is not None:
        catalog = _object(catalog_expected, "catalog condition")
        require(manifest.get("condition_id") == catalog["condition_id"]
                and manifest.get("operation_evidence_ref") == expected["operationEvidenceRef"],
                "catalog pair manifest condition/evidence differs")
    order, pins = _objects(manifest["order"], "pair order"), _objects(expected["reportPins"], "report pins")
    require(len(order) == len(pins) == 10, "five paired runs required")
    non_comparable_fragment = non_comparable_baseline_fragment(expected)
    is_non_comparable = manifest.get("measurement_status") == "nonComparable"
    non_comparable_failure: JsonObject | None = None
    if is_non_comparable:
        non_comparable_condition = _object(catalog_expected, "non-comparable catalog condition")
        require(non_comparable_fragment is not None
                and result.get("comparisonStatus") == "nonComparable",
                "non-comparable measurement is not an approved baseline condition")
        failures = _objects(manifest.get("failures"), "non-comparable failures")
        require(len(failures) == 5, "non-comparable condition must retain five baseline slots")
        non_comparable_failure = failures[0]
        require(non_comparable_failure.get("status") == "nonComparable"
                and non_comparable_failure.get("conditionId") == non_comparable_condition["condition_id"]
                and non_comparable_failure.get("fragment") == non_comparable_fragment,
                "non-comparable failure does not match its catalog condition")
    else:
        require(result.get("comparisonStatus") == "paired",
                "paired comparison status is missing")
    paths = []
    candidate_reports: list[JsonObject] = []
    for index in range(len(order)):
        row, pin = order[index], pins[index]
        pair = index // 2
        arm = ("baseline", "candidate")[index % 2] if pair % 2 == 0 else ("candidate", "baseline")[index % 2]
        raw_arm = expected_baseline if arm == "baseline" else arm
        binary_sha = _object(expected_arms[raw_arm], "expected arm")["binary_sha256"]
        require(row["arm"] == arm and type(row["pair_index"]) is int and row["pair_index"] == pair and row["executable_sha256"] == binary_sha, "pair order/binary differs")
        require_execution_binary_role(row, binary_refs[arm])
        bundle.file({"path": row["executable"], "sha256": binary_sha})
        binary_context = context_inputs[_text(row["executable"], "executable path")]
        require(binary_context["exists"] is True and binary_context["sha256"] == binary_sha, "binary context proof differs")
        require(row["output"] == pin["path"] and Path(_text(pin["path"], "report path")).name == f"{arm}-{pair}.json", "report path/order differs")
        path = bundle.file(pin)
        require(path.name == f"{arm}-{pair}.json", "transport must retain report basename")
        report = read_json(path)
        if is_non_comparable and arm == "baseline":
            require(non_comparable_failure is not None,
                    "non-comparable baseline failure is missing")
            _validate_noncomparable_baseline_order(
                bundle, expected, row, report, pair,
                cast(str, non_comparable_fragment), cast(JsonObject, non_comparable_failure),
            )
            continue
        require(report["measurement_executed"] is True and type(report["pair_index"]) is int and type(report["sample_count"]) is int and report["sample_count"] == 5, "actual five samples required")
        if is_non_comparable:
            arm_from_report, samples_from_report = _validate_report(report, expected)
            require(arm_from_report == "candidate" and len(samples_from_report) == 5
                    and not report.get("failure"),
                    "non-comparable candidate report must contain five samples")
            candidate_reports.append(report)
        if catalog_expected is not None:
            catalog = _object(catalog_expected, "catalog condition")
            require(report.get("condition_id") == catalog["condition_id"]
                    and report.get("case_id") == catalog["case_id"]
                    and report.get("state") == catalog["state"]
                    and report.get("operation_kind") == catalog["operation_kind"]
                    and report.get("operation_evidence_ref") == expected["operationEvidenceRef"]
                    and report.get("catalog_condition") == catalog,
                    "catalog raw performance report identity differs")
        samples = _objects(report["samples"], "report samples")
        require([s["sample"] for s in samples] == list(range(5)) and all(type(s["sample"]) is int and set(_object(s["phases"], "sample phases")) == set(PHASES) for s in samples), "incomplete sample/phase coverage")
        for sample in samples:
            validate_measurement_sample(sample, expected, plan, candidate=arm == "candidate")
        loads = _objects(row["workerLoads"], "worker loads")
        require(len(loads) == 5, "all five worker load observations required")
        for load in loads:
            require(set(load) == {"before", "after"} and all(type(load[k]) in (int, float) and math.isfinite(_numeric(load[k], "worker load")) and 0 <= _numeric(load[k], "worker load") <= 0.25 for k in ("before", "after")), "worker quiet load limit exceeded")
        require(row["load_ratio_before"] == max(_numeric(x["before"], "worker before load") for x in loads) and row["load_ratio_after"] == max(_numeric(x["after"], "worker after load") for x in loads), "worker load aggregate differs")
        for key in ("load_ratio_before", "load_ratio_after"):
            value = _numeric(row[key], "quiet load")
            require(type(value) in (int, float) and math.isfinite(value) and 0 <= value <= 0.25, "quiet load limit exceeded")
        if not is_non_comparable or arm == "candidate":
            paths.append(path)
    if is_non_comparable:
        condition = _object(catalog_expected, "non-comparable catalog condition")
        condition_id = _text(condition.get("condition_id"), "non-comparable condition id")
        first_failure = cast(JsonObject, non_comparable_failure)
        expected_failures: list[JsonObject] = [first_failure]
        expected_failures.extend({
            "status": "missing",
            "reason": "baseline skipped after known fragment refusal",
            "afterFailure": first_failure,
        } for _ in range(4))
        require(manifest.get("failures") == expected_failures,
                "non-comparable skipped baseline slots differ")
        require(len(candidate_reports) == 5
                and sum(len(_objects(row["samples"], "candidate samples"))
                        for row in candidate_reports) == 25,
                "non-comparable candidate requires all 25 samples")
        expected_policy: JsonObject = {
            "status": "nonComparable",
            "policyPass": False,
            "reason": "known baseline fragment navigation refusal",
            "conditionId": condition_id,
            "fragment": non_comparable_fragment,
            "candidateOperationalPass": True,
            "candidateSamples": 25,
            "phaseRecords": len(PHASES) * 25,
        }
        require(result.get("policy") == expected_policy,
                "saved non-comparable candidate policy differs")
        memory_accepted = _require_v2_memory_policy(
            bundle, expected, result, paths, non_comparable=True,
            policy_version=memory_policy_version, assessment_sink=memory_assessment_sink,
            allow_rejected=allow_memory_rejected,
        )
        if memory_acceptance_sink is not None:
            memory_acceptance_sink.append(memory_accepted)
        return _validated_attempt_summary(
            expected, source, "nonComparable", candidate_samples=25,
            sample_records=25,
            phase_records=len(PHASES) * 25,
        )
    canonical_policy = evaluate_measurement_policy(paths, expected)
    aggregate = _object(canonical_policy["aggregate"], "canonical aggregate")
    require(aggregate["measurement_executed"] is True, "actual measurements absent")
    saved = _object(result["policy"], "measurement policy")
    require(saved == canonical_policy, "saved paired policy differs from raw reports")
    memory_accepted = _require_v2_memory_policy(
        bundle, expected, result, paths, non_comparable=False,
        policy_version=memory_policy_version, assessment_sink=memory_assessment_sink,
        allow_rejected=allow_memory_rejected,
    )
    if memory_acceptance_sink is not None:
        memory_acceptance_sink.append(memory_accepted)
    if not memory_accepted and "memoryPolicy" not in result:
        require(canonical_policy["policyPass"] is True, "paired time/RSS regression")
    require(result.get("comparisonStatus") == "paired",
            "paired condition comparison status differs")
    arms = _object(aggregate["arms"], "aggregate arms")
    arm_values = [_object(value, "aggregate arm") for value in arms.values()]
    require(all(a["sample_count"] == 25 and all(
        _object(
            _object(a["phases"], "aggregate phases")[phase],
            f"{phase} aggregate",
        )["sample_count"] == 25
        for phase in PHASES
    ) for a in arm_values), f"{len(PHASES) * 50} phase records required")
    # 診断用acceptance:falseは許可し、生データの20条件だけで受理する。
    return _validated_attempt_summary(
        expected, source, "paired", candidate_samples=25, sample_records=50,
        phase_records=_integer(canonical_policy.get("phaseRecords"),
                               "paired phase record count"),
    )


def _require_catalog_attempt_set(attempts: list[JsonObject], first_expected: JsonObject) -> None:
    require(len(attempts) == 84, "formal HTML performance requires all 84 catalog conditions")
    require("catalogCondition" in first_expected,
            "formal HTML performance requires catalog-bound current-source evidence")


def _require_html84_acceptance(validation: JsonObject, *, diagnostic: bool = False) -> None:
    if diagnostic:
        require(validation.get("acceptance") is False
                and validation.get("fontIdentityEquivalence") == "not-established",
                "HTML84 diagnostic evidence must remain unaccepted with font identity unresolved")
        return
    require(validation.get("acceptance") is True,
            "HTML84 visual acceptance is not established")
    require(validation.get("fontIdentityEquivalence") == "established",
            "HTML84 same-font equivalence is not established")


def _require_formal_release_result(result: JsonObject) -> None:
    stage90 = _object(result.get("htmlStage90"), "HTML84 stage90 result")
    _require_html84_acceptance(stage90)
    require(result.get("performanceNonregression") is True,
            "HTML performance non-regression is not established")


def _catalog_memory_summary(bundle: Bundle, attempts: list[JsonObject], *,
                            assessments: list[JsonObject] | None = None) -> JsonObject:
    """Summarize only attempts already authenticated by validate_attempt."""
    results = [bundle.json(attempt["result"]) for attempt in attempts]
    present = ["memoryPolicy" in result for result in results]
    if not any(present):
        return {}
    require(len(results) == 84 and all(present),
            "catalog memory policy requires all 84 conditions")
    from html_performance_memory_policy import SUPPORTED_POLICY_VERSIONS

    if assessments is not None:
        versions = {row.get("version") for row in assessments}
        require(len(assessments) == 84 and len(versions) == 1
                and versions <= set(SUPPORTED_POLICY_VERSIONS),
                "catalog memory policy assessment differs")
        return {"memoryPolicyVersion": next(iter(versions)), "retainedMemoryConditions": 84,
                "memoryPolicyAssessments": assessments}
    versions = {_object(result["memoryPolicy"], "memory policy").get("version") for result in results}
    require(len(versions) == 1 and versions <= set(SUPPORTED_POLICY_VERSIONS),
            "catalog memory policy version differs")
    return {"memoryPolicyVersion": next(iter(versions)), "retainedMemoryConditions": 84}


def validate_expected_builds(bundle: Bundle, root: Path, first_expected: JsonObject, *,
                             git_query: GitQuery | None = None,
                             reuse_context: object = None) -> dict[str, JsonObject]:
    """Validate each immutable arm proof once for the whole catalog batch."""
    first_arms = _object(first_expected["arms"], "expected arms")
    expected_baseline = baseline_arm_id(first_expected)
    if expected_baseline == DEFAULT_BASELINE_ARM_ID:
        before = bundle.json(fixed_source_pair(first_expected)["before"])
        def query(path: Path, args: Sequence[str]) -> str:
            return git(path, *args, git_query=git_query)
        require(reference_domain.active_reference_at_commit(
            root, _text(before.get("head"), "measured source HEAD"), query,
            allow_legacy_missing=True) is None, "active reference wrapper is required")
    validated: dict[str, JsonObject] = {}
    for raw_arm, arm in ((expected_baseline, "baseline"), ("candidate", "candidate")):
        arm_value = _object(first_arms[raw_arm], "expected arm")
        if arm == "baseline" and expected_baseline != DEFAULT_BASELINE_ARM_ID:
            contract = _object(first_expected.get("referenceParent"), "reference contract")
            if contract.get("referenceKind") == "fresh-history-source":
                from html_performance_fresh_reference import validate_fresh_reference
                if reuse_context is None:
                    validated[arm] = validate_fresh_reference(bundle, root, first_expected,
                                                               git_query=git_query)
                else:
                    validated[arm] = validate_fresh_reference(bundle, root, first_expected,
                                                               git_query=git_query,
                                                               reuse_context=reuse_context)
            else:
                validated[arm] = _validate_reference_parent_wrapper(
                    bundle, root, first_expected, git_query=git_query)
        else:
            validated[arm] = _validate_build_proof(
                bundle, root, arm_value.get("build_artifact_ref"), arm, git_query=git_query,
                reuse_context=reuse_context)
    contract_value = first_expected.get("referenceParent")
    if isinstance(contract_value, dict) and contract_value.get("referenceKind") == "fresh-history-source":
        require(all(validated["baseline"].get(key) == validated["candidate"].get(key) for key in (
            "cargoExecutableSha256", "rustcExecutableSha256", "buildEnvironmentSha256",
            "toolVersions", "cargoConfigPins", "cargoLauncher", "rustcLauncher", "guardPython")),
            "fresh reference and candidate verified build environments differ")
    return validated


def _require_measurement_execution_environment(before: JsonObject) -> None:
    tool = _object(before.get("toolContext"), "measurement tool context")
    environment = dict(os.environ)
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    encoded = json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    require(tool.get("environmentSha256") == hashlib.sha256(encoded.encode()).hexdigest()
            and tool.get("platform") == platform.platform() and tool.get("cpuCount") == os.cpu_count(),
            "measurement execution environment differs")


def _same_binary_v3_reuse_context(
    bundle: Bundle, root: Path, expected: JsonObject, result: JsonObject,
    current: PartialEpochCurrentBuild, *, git_query: GitQuery | None,
    memory_policy_version: str | None = None,
) -> object:
    """実行済み worker と現 HEAD の build を再認証してから Rust 差分を許す。"""
    digest(current.manifest_sha256)
    digest(current.artifact_sha256)
    current_bundle = Bundle(current.directory)
    require(sha256(current.directory / "bundle.json") == current.manifest_sha256,
            "current build bundle manifest SHA differs")
    live_head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(current_bundle.manifest.get("head") == live_head, "current build bundle HEAD differs")
    endpoints = _object(expected.get("endpointRefs"), "measurement endpoint refs")
    before = bundle.json(endpoints.get("before"))
    _, old = _v3_origin_and_endpoints(bundle, root, expected, result, before,
                                      bundle.json(endpoints.get("end")), git_query=git_query)
    ref = current_bundle.relative_reference(current.artifact_path, current.artifact_sha256)
    new = _validate_build_proof(current_bundle, root, ref, "candidate", git_query=git_query)
    require(new.get("head") == live_head, "current build source differs")
    for key in ("binarySha256", "lockSha256", "cargoExecutableSha256", "rustcExecutableSha256",
                "buildEnvironmentSha256", "toolVersions", "cargoConfigPins",
                "cargoLauncher", "rustcLauncher", "guardPython"):
        require(key in old and key in new and old[key] == new[key],
                "measured and current build differ: " + key)
    _require_measurement_execution_environment(before)
    context = _verified_v3_reuse_context(
        bundle, root, _text(bundle.manifest.get("head"), "measured HEAD"), expected,
        git_query=git_query, memory_policy_version=memory_policy_version, same_binary_rust=True)
    require(sha256(current.directory / "bundle.json") == current.manifest_sha256,
            "current build bundle changed during verification")
    return context


def _verify_impl(directory: Path, root: Path, *, git_query: GitQuery | None = None,
            require_catalog: bool, diagnostic_catalog: bool = False,
                         reuse_context: object = None, remote_context: object = None,
                         memory_policy_version: str | None = None,
                         current_build: PartialEpochCurrentBuild | None = None) -> JsonObject:
    bundle = Bundle(directory)
    head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    attempts = _objects(bundle.manifest["attempts"], "viewport attempts")
    first_expected = bundle.json(attempts[0]["expected"]) if attempts else {}
    first_pair_value = first_expected.get("sourcePair")
    first_pair = _object(first_pair_value, "source pair") if first_pair_value is not None else {}
    first_is_v3 = first_pair.get("schema") == "krr-html-performance-source-epoch-v3"
    if remote_context is not None:
        from html_performance_remote_admission import RemoteEvidenceContext
        require(isinstance(remote_context, RemoteEvidenceContext)
                and reuse_context is None and not diagnostic_catalog
                and remote_context.head == head
                and remote_context.manifest_sha256 == sha256(directory / "bundle.json"),
                "remote evidence context differs")
    if current_build is not None:
        require(reuse_context is not None and first_is_v3 and bool(attempts),
                "current build reuse requires v3 source context")
    if reuse_context is None:
        require(bundle.manifest["head"] == head, "bundle is not current HEAD")
    else:
        from html_performance_source_reuse import SourceReuseContext
        if not isinstance(reuse_context, SourceReuseContext):
            raise ValueError("invalid source reuse context")
        checked_context = (_same_binary_v3_reuse_context(
            bundle, root, first_expected, bundle.json(attempts[0]["result"]), current_build,
            git_query=git_query, memory_policy_version=memory_policy_version)
            if current_build is not None else _verified_v3_reuse_context(
            bundle, root, reuse_context.measured_head, first_expected, git_query=git_query,
            memory_policy_version=memory_policy_version)
            if first_is_v3 else _verified_reuse_context(
                root, reuse_context.measured_head, git_query=git_query,
                allow_test_changes=reuse_context.artifact_equivalence))
        require(bundle.manifest["head"] == reuse_context.measured_head
                and head == reuse_context.eligible_head
                and checked_context == reuse_context, "reuse HEAD or source differs")
        head = reuse_context.measured_head
    if require_catalog:
        _require_catalog_attempt_set(attempts, first_expected)
    if "catalogCondition" in first_expected:
        require(len(attempts) == 84, "catalog performance requires all 84 conditions")
        expected_rows = [bundle.json(attempt["expected"]) for attempt in attempts]
        v3_rows = [_object(value.get("sourcePair"), "source pair").get("schema")
                   == "krr-html-performance-source-epoch-v3" for value in expected_rows]
        if any(v3_rows):
            require(remote_context is None, "unsupported v3 remote source context")
            require(diagnostic_catalog or reuse_context is not None,
                    "v3 formal acceptance requires current build equivalence")
            require(all(value.get("arms") == first_expected.get("arms")
                        and fixed_source_pair(value) == fixed_source_pair(first_expected)
                        for value in expected_rows),
                    "v3 catalog attempts do not share the fixed build and source")
            require(first_is_v3 and all(
                _object(value.get("sourcePair"), "source pair").get("epochContextRef")
                == first_pair.get("epochContextRef") for value in expected_rows),
                "v3 catalog program source differs")
        legacy_expected = next((value for value, is_v3 in zip(expected_rows, v3_rows, strict=True)
                                if not is_v3), None)
        validated_builds: dict[str, JsonObject] | None = None
        if not diagnostic_catalog:
            if remote_context is None:
                if legacy_expected is not None:
                    validated_builds = validate_expected_builds(
                        bundle, root, legacy_expected, git_query=git_query, reuse_context=reuse_context)
            else:
                assert isinstance(remote_context, RemoteEvidenceContext)
                validated_builds = remote_context.builds
        shared_source: SharedCurrentSource | None = None
        shared_before: JsonObject | None = None
        if remote_context is None and reuse_context is None and legacy_expected is not None:
            source_pair = fixed_source_pair(legacy_expected)
            shared_before = bundle.json(source_pair["before"])
            shared_source = SharedCurrentSource(root, _text(shared_before.get("head"), "source HEAD"),
                                                current_source(root, shared_before, git_query=git_query),
                                                Path(_text(shared_before.get("root"), "source root")))
        assessments: list[JsonObject] | None = [] if memory_policy_version is not None else None
        if remote_context is None:
            values = [validate_attempt(bundle, root, attempt, git_query=git_query,
                                       validated_builds=None if is_v3 else validated_builds,
                                       reuse_context=reuse_context,
                                       shared_source=None if is_v3 else shared_source,
                                       memory_policy_version=memory_policy_version,
                                       memory_assessment_sink=assessments)
                      for attempt, is_v3 in zip(attempts, v3_rows, strict=True)]
        else:
            values = [validate_attempt(bundle, root, attempt, git_query=git_query,
                                       validated_builds=validated_builds, reuse_context=reuse_context,
                                       remote_context=remote_context,
                                       memory_policy_version=memory_policy_version,
                                       memory_assessment_sink=assessments) for attempt in attempts]
        memory_summary = _catalog_memory_summary(bundle, attempts, assessments=assessments)
        from html_performance_host_load_validation import verify_catalog_host_load
        verify_catalog_host_load(bundle, attempts, _host_load_source_records(
            bundle, root, first_expected, head, git_query=git_query,
            memory_policy_version=memory_policy_version))
        non_comparable_ids = {
            _text(_object(value["catalogCondition"], "catalog condition")["condition_id"],
                  "non-comparable condition id")
            for value in values if value.get("comparisonStatus") == "nonComparable"
        }
        paired_conditions = sum(value.get("comparisonStatus") == "paired" for value in values)
        non_comparable_conditions = len(non_comparable_ids)
        require(
            (paired_conditions == 84 and non_comparable_conditions == 0)
            or (paired_conditions == 72
                and non_comparable_ids == set(NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES)),
            "catalog performance requires 84 paired conditions or the exact 72/12 comparison set",
        )
        candidate_samples = sum(_integer(value.get("candidateSamples"),
                                          "candidate sample count") for value in values)
        sample_records = sum(_integer(value.get("sampleRecords"),
                                       "sample record count") for value in values)
        phase_records = sum(_integer(value.get("phaseRecords"),
                                     "condition phase record count") for value in values)
        expected_samples = 84 * 25 + paired_conditions * 25
        require(candidate_samples == 84 * 25 and sample_records == expected_samples
                and phase_records == sample_records * len(PHASES),
                "catalog candidate/sample/phase totals differ")
        manifest_refs = {json.dumps(value["catalogManifestRef"], sort_keys=True) for value in values}
        plan_refs = {json.dumps(value["catalogPlanRef"], sort_keys=True) for value in values}
        validation_refs = {json.dumps(value["catalogCaptureValidationRef"], sort_keys=True) for value in values}
        font_request_refs = {json.dumps(value["fontIdentityRequestRef"], sort_keys=True) for value in values}
        font_response_refs = {json.dumps(value["fontIdentityResponseRef"], sort_keys=True) for value in values}
        font_probe_binaries = {json.dumps(value["fontIdentityProbeBinary"], sort_keys=True) for value in values}
        binary_proof_refs = {json.dumps(value["catalogBinaryProofRef"], sort_keys=True) for value in values}
        build_report_refs = {json.dumps(value["catalogBuildReportRef"], sort_keys=True) for value in values}
        capture_binary_refs = {json.dumps(value["catalogCaptureBinaryRef"], sort_keys=True) for value in values}
        current_build_refs = {json.dumps(value["catalogCurrentBuildReportRef"], sort_keys=True) for value in values}
        require(len(manifest_refs) == len(plan_refs) == len(validation_refs) ==
                len(font_request_refs) == len(font_response_refs) == len(font_probe_binaries) ==
                len(binary_proof_refs) == len(build_report_refs) == len(capture_binary_refs) ==
                len(current_build_refs) == 1,
                "catalog attempts do not share manifest, build proof and Chrome validation")
        catalog_manifest_ref = _object(values[0]["catalogManifestRef"], "catalog manifest reference")
        catalog_plan_ref = _object(values[0]["catalogPlanRef"], "catalog plan reference")
        validation_ref = _object(values[0]["catalogCaptureValidationRef"], "Chrome validation reference")
        manifest = bundle.json(catalog_manifest_ref)
        binary_refs = validate_catalog_binary_proof(manifest, bundle.file)
        require(binary_refs.proof_ref == values[0]["catalogBinaryProofRef"]
                and binary_refs.build_report_ref == values[0]["catalogBuildReportRef"]
                and binary_refs.binary_ref == values[0]["catalogCaptureBinaryRef"]
                and binary_refs.current_build_report_ref == values[0]["catalogCurrentBuildReportRef"],
                "transported catalog binary proof refs differ from manifest")
        validate_catalog_binary_closure(root, bundle, catalog_manifest_ref, binary_refs)
        catalog_plan = bundle.json(catalog_plan_ref)
        validation = bundle.json(validation_ref)
        request_ref = _object(values[0]["fontIdentityRequestRef"], "font identity request reference")
        response_ref = _object(values[0]["fontIdentityResponseRef"], "font identity response reference")
        probe_binary = _object(values[0]["fontIdentityProbeBinary"], "font identity probe binary")
        manifest_probe = _object(manifest.get("fontIdentityProbe"), "manifest font identity probe")
        validation_probe = _object(validation.get("fontIdentityProbe"), "validated font identity probe")
        for key, reference in (("request", request_ref), ("response", response_ref)):
            declared = _object(manifest_probe.get(key), "manifest font probe file pin")
            validated = _object(validation_probe.get(key), "validated font probe file pin")
            transported = bundle.file(reference)
            require(declared.get("path") == reference.get("path")
                    and declared.get("sha256") == reference.get("sha256")
                    and declared.get("sizeBytes") == reference.get("size")
                    and validated == reference
                    and transported.stat().st_size == _integer(reference.get("size"), "font probe file size"),
                    f"font identity {key} transport pin differs")
        require(_object(manifest_probe.get("binary"), "manifest font probe binary") == probe_binary
                and _object(validation_probe.get("binary"), "validated font probe binary") == probe_binary,
                "font identity probe binary pin differs")
        require(validation_probe.get("inventoryMethod") == manifest_probe.get("inventoryMethod")
                and validation_probe.get("inventoryLimit") == manifest_probe.get("inventoryLimit")
                and bool(_text(validation_probe.get("inventoryMethod"), "font inventory method"))
                and bool(_text(validation_probe.get("inventoryLimit"), "font inventory limit")),
                "font identity inventory method/limit differs")
        require(validation.get("schema") == "krr-html84-independent-stage90-validation-v1"
                and validation.get("threshold") == 90
                and validation.get("measurement") == "not-performed"
                and validation.get("manifestPath") == catalog_manifest_ref["path"]
                and validation.get("manifestSha256") == catalog_manifest_ref["sha256"]
                and validation.get("planPath") == catalog_plan_ref["path"]
                and validation.get("planSha256") == catalog_plan_ref["sha256"],
                "transported strict Chrome validation does not bind this catalog manifest/plan")
        _require_html84_acceptance(validation, diagnostic=diagnostic_catalog)
        catalog_path = Path(_text(manifest.get("catalogPath"), "feature catalog path"))
        catalog_sha = _text(manifest.get("catalogSha256"), "feature catalog SHA")
        digest(catalog_sha)
        source_root = root if remote_context is None else Path(_text(
            bundle.json(fixed_source_pair(first_expected)["before"])["root"],
            "recorded source root"))
        catalog_relative = catalog_path.relative_to(source_root).as_posix()
        require(manifest.get("profile") == "catalog-stage90-captures"
                and manifest.get("acceptance") is False
                and catalog_plan.get("catalogPath") == str(catalog_path)
                and catalog_plan.get("catalogSha256") == catalog_sha
                and catalog_plan.get("profile") == "krr-html84-condition-plan-v1"
                and catalog_plan.get("acceptance") is False
                and manifest.get("planPath") == catalog_plan_ref["path"]
                and manifest.get("planSha256") == catalog_plan_ref["sha256"],
                "catalog manifest/plan binding differs")
        source = cast(SourceContract, values[0]["source"])
        require(source["source"].get(catalog_relative, (None,))[0] == catalog_sha,
                "catalog feature source is not current committed source")
        expected_catalog = _objects(catalog_plan.get("conditions"), "planned catalog condition")
        captured_catalog = _objects(manifest.get("conditions"), "captured catalog condition")
        validated_chrome = _objects(validation.get("conditions"), "validated Chrome conditions")
        require(len(expected_catalog) == len(captured_catalog) == len(validated_chrome) == 84,
                "catalog manifest/plan/Chrome validation coverage differs")
        actual_conditions = [_object(value["catalogCondition"], "measured catalog condition") for value in values]
        evidence_by_id = {
            _text(_object(value["catalogCondition"], "catalog condition")["condition_id"], "condition id"):
                _object(value["operationEvidenceRef"], "evidence reference")
            for value in values
        }
        require([_text(item.get("condition_id"), "measured condition id") for item in actual_conditions]
                == [_text(item.get("conditionId"), "planned condition id") for item in expected_catalog],
                "measured condition order or coverage differs")
        captured_by_id = {_text(item.get("conditionId"), "captured condition id"): item for item in captured_catalog}
        chrome_by_id = {_text(item.get("conditionId"), "validated Chrome condition id"): item
                        for item in validated_chrome}
        require(len(captured_by_id) == len(captured_catalog) == len(chrome_by_id) == len(validated_chrome),
                "catalog capture or Chrome validation ids are duplicated")
        for expected in expected_catalog:
            condition_id = _text(expected.get("conditionId"), "planned condition id")
            captured_value = captured_by_id.get(condition_id)
            if captured_value is None:
                raise ValueError(f"captured catalog condition is missing: {condition_id}")
            captured = captured_value
            match = next(item for item in actual_conditions if item["condition_id"] == condition_id)
            for plan_key, output_key in (("caseId", "case_id"), ("state", "state"),
                                         ("operationKind", "operation_kind"),
                                         ("targetSelector", "target_selector"),
                                         ("sourceSha256", "fixture_sha256")):
                require(expected.get(plan_key) == match.get(output_key),
                        f"measured catalog {output_key} differs: {condition_id}")
            require(expected.get("viewport") == match.get("viewport")
                    and captured.get("caseId") == expected.get("caseId")
                    and captured.get("state") == expected.get("state")
                    and captured.get("operationKind") == expected.get("operationKind")
                    and captured.get("sourceSha256") == expected.get("sourceSha256")
                    and captured.get("viewport") == expected.get("viewport"),
                    f"measured/captured catalog tuple differs: {condition_id}")
            evidence_ref = evidence_by_id[condition_id]
            require(captured.get("krrReportPath") == evidence_ref["path"]
                    and captured.get("krrReportSha256") == evidence_ref["sha256"],
                    f"measured catalog evidence ref differs: {condition_id}")
            chrome_row = chrome_by_id.get(condition_id)
            if chrome_row is None:
                raise ValueError(f"strict Chrome validation is missing: {condition_id}")
            require(captured.get("status") == "captured"
                    and _same_json(captured.get("chromeState"), captured.get("krrState")),
                    f"catalog Chrome/KRR state is not captured and identical: {condition_id}")
            match_value = next(value for value in values
                               if _object(value["catalogCondition"], "catalog condition")["condition_id"] == condition_id)
            chrome_capture_ref = _object(match_value["chromeCaptureRef"], "Chrome capture reference")
            chrome_receipt_ref = _object(match_value["chromeReceiptRef"], "Chrome receipt reference")
            for row, kind, ref in ((chrome_row, "capture", chrome_capture_ref),
                                   (chrome_row, "receipt", chrome_receipt_ref)):
                path_key, sha_key, size_key = kind + "Path", kind + "Sha256", kind + "Size"
                transported_path = bundle.file(ref)
                require(row.get(path_key) == ref["path"] and row.get(sha_key) == ref["sha256"]
                        and row.get(size_key) == ref.get("size")
                        and _integer(row.get(size_key), kind + " size") == transported_path.stat().st_size,
                        f"strict Chrome {kind} transport binding differs: {condition_id}")
            require(_same_json(chrome_row.get("state"), captured.get("chromeState")),
                    f"strict Chrome state differs from catalog condition: {condition_id}")
            krr_capture_ref = {"path": captured["krrCapturePath"], "sha256": captured["krrCaptureSha256"]}
            krr_capture_file = bundle.file(krr_capture_ref)
            require(chrome_row.get("krrCapturePath") == krr_capture_ref["path"]
                    and chrome_row.get("krrCaptureSha256") == krr_capture_ref["sha256"]
                    and _integer(chrome_row.get("krrCaptureSize"), "KRR capture size") == krr_capture_file.stat().st_size,
                    f"independent stage90 KRR capture binding differs: {condition_id}")
            score = _numeric(chrome_row.get("score"), "independent stage90 score")
            require(math.isfinite(score) and score >= 90,
                    f"independent stage90 score is below 90: {condition_id}")
        minimum_score = min(_numeric(row.get("score"), "independent stage90 score")
                            for row in validated_chrome)
        if shared_source is not None and shared_before is not None:
            require(current_source(root, shared_before, git_query=git_query) == shared_source.records,
                    "source changed during catalog verification")
        return {"head": head, "checks": 1680,
                "phaseRecords": phase_records, "sampleRecords": sample_records,
                "candidateSamples": candidate_samples,
                "conditions": 84, "candidateConditions": 84,
                "pairedConditions": paired_conditions,
                "nonComparableConditions": non_comparable_conditions,
                "htmlStage90": {"threshold": 90, "minimumScore": minimum_score,
                                "acceptance": validation["acceptance"],
                                "fontIdentityEquivalence": validation["fontIdentityEquivalence"]},
                "performanceNonregression": True, **memory_summary}
    require(len(attempts) == 3, "all three viewports required")
    values = [validate_attempt(bundle, root, attempt, git_query=git_query) for attempt in attempts]
    require([v["viewport"] for v in values] == [{"width": w, "height": h, "scale": 1} for w, h in VIEWPORTS], "viewport coverage/order differs")
    shared = [{k: v[k] for k in v if k != "viewport"} for v in values]
    require(shared[0] == shared[1] == shared[2], "three viewports do not share exact source/fixture/state/build/context")
    return {"head": head, "checks": 60, "phaseRecords": len(VIEWPORTS) * len(PHASES) * 50, "performanceNonregression": True}


def _verify(directory: Path, root: Path, *, git_query: GitQuery | None = None,
            require_catalog: bool, diagnostic_catalog: bool = False,
            reuse_context: object = None, remote_context: object = None,
            memory_policy_version: str | None = None,
            current_build: PartialEpochCurrentBuild | None = None) -> JsonObject:
    require_no_git_replacements(root, git_query=git_query)
    result = _verify_impl(directory, root, git_query=git_query, require_catalog=require_catalog,
                          diagnostic_catalog=diagnostic_catalog, reuse_context=reuse_context,
                          remote_context=remote_context,
                          **_current_build_kwargs(current_build),
                          **_memory_policy_kwargs(memory_policy_version))
    require_no_git_replacements(root, git_query=git_query)
    return result


def verify(directory: Path, root: Path, *, git_query: GitQuery | None = None,
           memory_policy_version: str | None = None) -> JsonObject:
    result = _verify(directory, root, git_query=git_query, require_catalog=True,
                     **_memory_policy_kwargs(memory_policy_version))
    _require_formal_release_result(result)
    return result


def _verify_source_reuse_impl(directory: Path, root: Path, measured_manifest_sha256: str,
                        build_directory: Path, build_manifest_sha256: str,
                        build_artifact_path: str, build_artifact_sha256: str, *,
                        git_query: GitQuery | None = None,
                        memory_policy_version: str | None = None) -> JsonObject:
    """旧測定の原本と現HEAD buildを照合し、文書のみの差分を別結果として認定する。"""
    root = root.resolve()
    digest(measured_manifest_sha256)
    digest(build_manifest_sha256)
    digest(build_artifact_sha256)
    require(sha256(directory / "bundle.json") == measured_manifest_sha256,
            "measured bundle manifest SHA differs")
    require(sha256(build_directory / "bundle.json") == build_manifest_sha256,
            "current build bundle manifest SHA differs")
    bundle = Bundle(directory)
    measured_head = _text(bundle.manifest.get("head"), "measured HEAD")
    eligible_head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    attempts = _objects(bundle.manifest["attempts"], "viewport attempts")
    first = bundle.json(attempts[0]["expected"]) if attempts else {}
    pair_value = first.get("sourcePair")
    pair = _object(pair_value, "source pair") if pair_value is not None else {}
    is_v3 = pair.get("schema") == "krr-html-performance-source-epoch-v3"
    current_build = PartialEpochCurrentBuild(build_directory, build_manifest_sha256,
                                             build_artifact_path, build_artifact_sha256)
    def source_context() -> object:
        if is_v3:
            require(bool(attempts), "reuse evidence has no attempts")
            return _same_binary_v3_reuse_context(bundle, root, first, bundle.json(attempts[0]["result"]),
                                                 current_build, git_query=git_query,
                                                 memory_policy_version=memory_policy_version)
        return _verified_reuse_context(root, measured_head, git_query=git_query, allow_test_changes=True)
    context = source_context()
    measured = _verify(directory, root, git_query=git_query, require_catalog=True,
                       reuse_context=context,
                       **_current_build_kwargs(current_build if is_v3 else None),
                       **_memory_policy_kwargs(memory_policy_version))
    _require_formal_release_result(measured)
    if not attempts:
        raise ValueError("reuse evidence has no attempts")
    has_v3 = is_v3 or any(
        _object(bundle.json(attempt["expected"]).get("sourcePair"), "source pair").get("schema")
        == "krr-html-performance-source-epoch-v3" for attempt in attempts[1:]
    )
    if is_v3:
        endpoints = _object(first.get("endpointRefs"), "endpoint references")
        first_result = bundle.json(attempts[0]["result"])
        _, old_build = _v3_origin_and_endpoints(
            bundle, root, first, first_result, bundle.json(endpoints["before"]),
            bundle.json(endpoints["end"]), git_query=git_query,
        )
    else:
        old_build = validate_expected_builds(bundle, root, first, git_query=git_query,
                                             reuse_context=context)["candidate"]
    current_bundle = Bundle(build_directory)
    require(current_bundle.manifest["head"] == eligible_head, "current build bundle HEAD differs")
    build_ref = current_bundle.relative_reference(build_artifact_path, build_artifact_sha256)
    new_build = _validate_build_proof(current_bundle, root, build_ref, "candidate", git_query=git_query)
    for key in ("binarySha256", "lockSha256", "cargoExecutableSha256", "rustcExecutableSha256"):
        require(old_build[key] == new_build[key], "measured and current build differ: " + key)
    from html_performance_source_reuse import SourceReuseContext
    if not isinstance(context, SourceReuseContext):
        raise ValueError("invalid source reuse context")
    if context.artifact_equivalence or has_v3:
        for key in ("buildEnvironmentSha256", "toolVersions", "cargoConfigPins",
                    "cargoLauncher", "rustcLauncher", "guardPython"):
            require(old_build[key] == new_build[key], "measured and current build differ: " + key)
        endpoint = _object(first.get("endpointRefs"), "measurement endpoint refs")
        before = bundle.json(endpoint.get("before"))
        _require_measurement_execution_environment(before)
    require(new_build["head"] == eligible_head and source_context() == context
            and sha256(directory / "bundle.json") == measured_manifest_sha256
            and sha256(build_directory / "bundle.json") == build_manifest_sha256,
            "source changed during reuse validation")
    result: JsonObject = {"measuredHead": measured_head, "eligibleHead": eligible_head,
                          "reuseEligibility": True, "measuredResult": measured}
    if is_v3 or context.artifact_equivalence or context.program_equivalence:
        result.update({"reuseKind": ("binary-bound-product-source-equivalence-v1" if is_v3 else
                                     "program-source-artifact-equivalence-v1"
                                     if context.program_equivalence else
                                     "test-source-artifact-equivalence-v1"),
                       "measuredManifestSha256": measured_manifest_sha256,
                       "currentBuildManifestSha256": build_manifest_sha256,
                       "currentBuildArtifactPath": build_artifact_path,
                       "currentBuildArtifactSha256": build_artifact_sha256,
                       "binarySha256": new_build["binarySha256"]})
    return result


def verify_source_reuse(directory: Path, root: Path, measured_manifest_sha256: str,
                        build_directory: Path, build_manifest_sha256: str,
                        build_artifact_path: str, build_artifact_sha256: str, *,
                        git_query: GitQuery | None = None,
                        memory_policy_version: str | None = None) -> JsonObject:
    root = root.resolve()
    require_no_git_replacements(root, git_query=git_query)
    result = _verify_source_reuse_impl(directory, root, measured_manifest_sha256,
                                       build_directory, build_manifest_sha256,
                                       build_artifact_path, build_artifact_sha256, git_query=git_query,
                                       **({"memory_policy_version": memory_policy_version}
                                          if memory_policy_version is not None else {}))
    require_no_git_replacements(root, git_query=git_query)
    return result


def _verify_catalog_diagnostic(directory: Path, root: Path, *,
                               git_query: GitQuery | None = None) -> JsonObject:
    result = _verify(directory, root, git_query=git_query, require_catalog=True,
                     diagnostic_catalog=True)
    _require_html84_acceptance(
        _object(result.get("htmlStage90"), "HTML84 diagnostic result"), diagnostic=True,
    )
    return result


def _verify_legacy_diagnostic(directory: Path, root: Path, *,
                              git_query: GitQuery | None = None) -> JsonObject:
    return _verify(directory, root, git_query=git_query, require_catalog=False)


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


def _require_acquisition_jobs(jobs: JsonObject, run_id: int) -> list[JsonObject]:
    rows = _objects(jobs["jobs"], "CI jobs")
    require(jobs["total_count"] == len(rows), "incomplete current CI jobs")
    for name in ("Test and Build (ubuntu-latest, linux64)", "Test and Build (macos-15, mac-arm64)",
                 "Test and Build (windows-latest, win64)", "Native paired HTML time and RSS nonregression"):
        matches = [job for job in rows if job["name"] == name]
        require(len(matches) == 1 and matches[0]["run_id"] == run_id
                and matches[0]["status"] == "completed" and matches[0]["conclusion"] == "success",
                f"current producer dependency did not succeed: {name}")
    return rows


def _completed_pr_binding(run: JsonObject, repository: str, head: str) -> tuple[int, str] | None:
    if run.get("event") != "pull_request":
        return None
    repo, bindings = run.get("repository"), run.get("pull_requests")
    if (not isinstance(repo, dict) or repo.get("full_name") != repository
            or not isinstance(bindings, list) or len(bindings) != 1
            or not isinstance(bindings[0], dict)):
        return None
    binding = bindings[0]
    number, base, pr_head = binding.get("number"), binding.get("base"), binding.get("head")
    if (type(number) is not int or number <= 0 or not isinstance(base, dict)
            or not isinstance(pr_head, dict) or pr_head.get("sha") != head):
        return None
    base_sha = base.get("sha")
    if not isinstance(base_sha, str) or re.fullmatch(r"[a-f0-9]{40}", base_sha) is None:
        return None
    return number, base_sha




def acquire(directory: Path, root: Path, repository: str, current_producer: bool = False, *,
            git_query: GitQuery | None = None) -> JsonObject:
    from html_performance_remote_admission import verify_authenticated_remote
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
        job_rows = _require_acquisition_jobs(jobs, run_id)
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
        from local_html_performance_evidence import verify_from_api
        if verify_from_api(repository, _integer(pull["number"], "PR number"),
                           _text(pull_base["sha"], "PR base"), head):
            require(current_latest() == run, "current producer generation changed")
            directory.mkdir(parents=True)
            (directory / "acquisition.json").write_text(json.dumps({
                "kind": "owner-local-html-performance", "head": head, "runId": run["id"],
                "runAttempt": run["run_attempt"],
            }))
            return {"head": head, "kind": "owner-local-html-performance", "conditions": 84}
    elif (pr_binding := _completed_pr_binding(run, repository, head)) is not None:
        number, base = pr_binding
        from local_html_performance_evidence import verify_from_api
        if verify_from_api(repository, number, base, head):
            run_id = _integer(run.get("id"), "CI run id")
            attempt = _integer(run.get("run_attempt"), "CI attempt")
            _require_acquisition_jobs(api(f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100"), run_id)
            latest = api(f"repos/{repository}/actions/runs/{run_id}")
            require(current_latest() == run and all(latest.get(key) == run.get(key) for key in
                    ("id", "head_sha", "run_attempt", "status", "conclusion", "event", "pull_requests")),
                    "completed CI generation changed")
            require(verify_from_api(repository, number, base, head),
                    "completed CI local evidence changed")
            directory.mkdir(parents=True)
            (directory / "acquisition.json").write_text(json.dumps({
                "kind": "owner-local-html-performance", "head": head, "runId": run_id,
                "runAttempt": attempt,
            }))
            return {"head": head, "kind": "owner-local-html-performance", "conditions": 84}
    artifact_snapshot = api(f"repos/{repository}/actions/runs/{run['id']}/artifacts?per_page=100")
    artifacts = _objects(artifact_snapshot["artifacts"], "CI artifacts")
    require(artifact_snapshot["total_count"] == len(artifacts), "incomplete artifact page")
    matches = [a for a in artifacts if a["name"] == f"html-performance-paired-{head}-{run['id']}-{run['run_attempt']}" and a["expired"] is False]
    if current_producer and not matches:
        from local_html_performance_evidence import require_fresh_measurement_if_local_expired
        require_fresh_measurement_if_local_expired(job_rows)
    require(len(matches) == 1, "one immutable current-source performance artifact required")
    directory.mkdir(parents=True)
    archive = directory / "artifact.zip"
    archive.write_bytes(subprocess.check_output(["gh", "api", f"repos/{repository}/actions/artifacts/{matches[0]['id']}/zip"]))
    artifact_digest = matches[0].get("digest", "")
    require(artifact_digest == "sha256:" + sha256(archive), "GitHub artifact digest absent/mismatch")
    result = verify_authenticated_remote(directory, archive, root, repository, run,
                                         _text(matches[0]["name"], "artifact name"), head)
    latest = api(f"repos/{repository}/actions/runs/{run['id']}")
    require(all(latest[k] == run[k] for k in ("id", "head_sha", "run_attempt", "status", "conclusion")), "CI run changed during acquisition")
    # ZIP取得中の新規runも照合し、古い成功世代だけのGETで取得を完了しない。
    require(current_latest() == run, "latest CI generation changed during acquisition")
    (directory / "acquisition.json").write_text(json.dumps({"repository": repository, "head": head, "runId": run["id"], "runAttempt": run["run_attempt"], "artifactId": matches[0]["id"], "artifactDigest": artifact_digest}, indent=2) + "\n")
    return result


def _require_cli_memory_v2(result: JsonObject, *, reused: bool) -> None:
    from html_performance_memory_policy import LEGACY_POLICY_VERSION

    if reused:
        require(result.get("reuseEligibility") is True, "performance reuse is not eligible")
        result = _object(result.get("measuredResult"), "reused measured result")
    require(result.get("memoryPolicyVersion") == LEGACY_POLICY_VERSION
            and type(result.get("retainedMemoryConditions")) is int
            and result.get("retainedMemoryConditions") == 84,
            "HTML performance requires v2 retained memory for all 84 conditions")


def _require_cli_memory_v3(result: JsonObject, *, reused: bool) -> None:
    from html_performance_memory_policy import PREVIOUS_POLICY_VERSION

    if reused:
        require(result.get("reuseEligibility") is True, "performance reuse is not eligible")
        result = _object(result.get("measuredResult"), "reused measured result")
    require(result.get("memoryPolicyVersion") == PREVIOUS_POLICY_VERSION
            and type(result.get("retainedMemoryConditions")) is int
            and result.get("retainedMemoryConditions") == 84
            and len(_array(result.get("memoryPolicyAssessments"), "memory assessments")) == 84,
            "HTML performance requires v3 retained memory for all 84 conditions")


def _require_cli_memory_v4(result: JsonObject, *, reused: bool) -> None:
    from html_performance_memory_policy import POLICY_VERSION

    if reused:
        require(result.get("reuseEligibility") is True, "performance reuse is not eligible")
        result = _object(result.get("measuredResult"), "reused measured result")
    require(result.get("memoryPolicyVersion") == POLICY_VERSION
            and type(result.get("retainedMemoryConditions")) is int
            and result.get("retainedMemoryConditions") == 84
            and len(_array(result.get("memoryPolicyAssessments"), "memory assessments")) == 84,
            "HTML performance requires v4 retained memory for all 84 conditions")


def _verify_reuse_request_with_policy(
    request_path: Path,
    root: Path,
    *,
    require_memory_policy_v2: bool,
    require_memory_policy_v3: bool,
    require_memory_policy_v4: bool,
) -> JsonObject:
    from html_performance_source_reuse_cli import verify_reuse_request

    version = ("html-memory-v4" if require_memory_policy_v4 else
               "html-memory-v3" if require_memory_policy_v3 else None)
    result = verify_reuse_request(
        request_path, root, **_memory_policy_kwargs(version),
    )
    if require_memory_policy_v2:
        _require_cli_memory_v2(result, reused=True)
    if require_memory_policy_v3:
        _require_cli_memory_v3(result, reused=True)
    if require_memory_policy_v4:
        _require_cli_memory_v4(result, reused=True)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--evidence-dir", type=Path, default=Path("tmp/html-performance-release-evidence"))
    parser.add_argument("--acquire", action="store_true")
    parser.add_argument("--acquire-current-producer", action="store_true")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--candidate-build-proof")
    parser.add_argument("--build-artifact-sha256")
    parser.add_argument("--bundle-manifest-sha256")
    parser.add_argument("--reuse-request", type=Path)
    parser.add_argument("--require-memory-policy-v2", action="store_true")
    parser.add_argument("--require-memory-policy-v3", action="store_true")
    parser.add_argument("--require-memory-policy-v4", action="store_true")
    parser.add_argument(
        "--diagnostic-only",
        action="store_true",
        help="verify a catalog-bound diagnostic bundle while retaining acceptance=false",
    )
    args = parser.parse_args()
    try:
        require(not (args.acquire and args.acquire_current_producer), "ambiguous acquisition mode")
        require(sum((args.require_memory_policy_v2, args.require_memory_policy_v3, args.require_memory_policy_v4)) <= 1,
                "memory policy versions cannot be combined")
        require(not ((args.require_memory_policy_v2 or args.require_memory_policy_v3 or args.require_memory_policy_v4) and (
            args.acquire or args.acquire_current_producer or args.diagnostic_only
            or args.candidate_build_proof is not None or args.build_artifact_sha256 is not None
            or args.bundle_manifest_sha256 is not None)),
            "v2 memory policy requires formal verification, not acquisition or diagnostics")
        require(not (args.reuse_request is not None and (
            args.acquire or args.acquire_current_producer or args.diagnostic_only
            or args.candidate_build_proof is not None or args.build_artifact_sha256 is not None
            or args.bundle_manifest_sha256 is not None)),
            "reuse request cannot be combined with acquisition, diagnostic or candidate build proof")
        require(not (args.diagnostic_only and (args.acquire or args.acquire_current_producer
                                               or args.candidate_build_proof is not None
                                               or args.build_artifact_sha256 is not None
                                               or args.bundle_manifest_sha256 is not None)),
                "diagnostic-only mode cannot be combined with acquisition or candidate build proof")
        require((args.candidate_build_proof is None) == (args.build_artifact_sha256 is None),
                "candidate build proof path/SHA must be provided together")
        if args.reuse_request is not None:
            result = _verify_reuse_request_with_policy(
                args.reuse_request, args.repo.resolve(),
                require_memory_policy_v2=args.require_memory_policy_v2,
                require_memory_policy_v3=args.require_memory_policy_v3,
                require_memory_policy_v4=args.require_memory_policy_v4,
            )
            print(json.dumps(result, sort_keys=True))
            return 0
        if args.candidate_build_proof is not None:
            require(args.bundle_manifest_sha256 is not None,
                    "bundle manifest SHA is required for candidate build proof")
            require(not args.acquire and not args.acquire_current_producer,
                    "candidate build proof cannot be combined with acquisition")
            result = validate_candidate_build_proof(args.evidence_dir, args.repo.resolve(),
                args.bundle_manifest_sha256, args.candidate_build_proof, args.build_artifact_sha256)
            print(json.dumps(result, sort_keys=True))
            return 0
        if args.acquire or args.acquire_current_producer:
            result = acquire(args.evidence_dir, args.repo, args.repository, args.acquire_current_producer)
            print(json.dumps(result, sort_keys=True))
            return 0
        elif not args.diagnostic_only:
            manifest_path = args.evidence_dir / "bundle.json"
            request_path = args.evidence_dir / "reuse-request.json"
            if request_path.exists() or request_path.is_symlink():
                manifest_present = manifest_path.exists() or manifest_path.is_symlink()
                use_reuse = not manifest_present
                if manifest_present:
                    manifest = Bundle(args.evidence_dir).manifest
                    _objects(manifest.get("attempts"), "viewport attempts")
                    head = git(args.repo.resolve(), "rev-parse", "HEAD").strip()
                    use_reuse = _text(manifest.get("head"), "bundle HEAD") != head
                if use_reuse:
                    result = _verify_reuse_request_with_policy(
                        request_path, args.repo.resolve(),
                        require_memory_policy_v2=args.require_memory_policy_v2,
                        require_memory_policy_v3=args.require_memory_policy_v3,
                        require_memory_policy_v4=args.require_memory_policy_v4,
                    )
                    print(json.dumps(result, sort_keys=True))
                    return 0
            acquisition = args.evidence_dir / "acquisition.json"
            if acquisition.exists() or acquisition.is_symlink():
                require(not (args.require_memory_policy_v2 or args.require_memory_policy_v3 or args.require_memory_policy_v4),
                        "v2 memory policy cannot be checked by acquisition")
                current_producer = (os.environ.get("GITHUB_JOB") == "preflight"
                                    and os.environ.get("GITHUB_EVENT_NAME") == "pull_request")
                with tempfile.TemporaryDirectory(prefix="krr-html-performance-revalidate-") as temporary:
                    result = acquire(Path(temporary).resolve() / "evidence", args.repo,
                                     args.repository, current_producer)
                print(json.dumps(result, sort_keys=True))
                return 0
        result = (_verify_catalog_diagnostic(args.evidence_dir, args.repo)
                  if args.diagnostic_only else verify(args.evidence_dir, args.repo,
                      **_memory_policy_kwargs("html-memory-v4" if args.require_memory_policy_v4 else "html-memory-v3" if args.require_memory_policy_v3 else None)))
        if args.require_memory_policy_v2:
            _require_cli_memory_v2(result, reused=False)
        if args.require_memory_policy_v3:
            _require_cli_memory_v3(result, reused=False)
        if args.require_memory_policy_v4:
            _require_cli_memory_v4(result, reused=False)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"HTML performance release gate failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
