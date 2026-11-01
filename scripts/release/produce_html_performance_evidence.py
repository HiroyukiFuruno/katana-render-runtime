#!/usr/bin/env python3
"""公開対象のclean sourceからbaseline/candidateを実行して3幅のpaired証跡を生成する。"""
from __future__ import annotations

import argparse
import errno
import hashlib
import io
import json
import math
import os
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import tempfile
from contextlib import ExitStack, contextmanager
from functools import lru_cache
from pathlib import Path
from types import FrameType
from typing import BinaryIO, Literal, NamedTuple, NoReturn, TypedDict, cast, overload
from collections.abc import Callable, Iterable, Iterator, Mapping, Sequence

import verify_html_performance_evidence as gate
import html_performance_inventory as inventory_ops
import html_performance_reference as reference

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
LOCAL_CAPACITY_BASELINE = 2363047936
LOCAL_CAPACITY_THRESHOLD = 4726095872
PROTECTED_REGISTRY_SCHEMA = "krr-protected-refs-successor-v1"


@contextmanager
def candidate_build_locks(root: Path) -> Iterator[None]:
    hooks = str(Path(__file__).resolve().parents[1] / "hooks")
    sys.path.insert(0, hooks)
    try:
        from local_review_lock import review_lock
    finally:
        sys.path.remove(hooks)
    with ExitStack() as locks:
        lock_paths = (
            root / ".moon/cache/krr-terminal-report.lock",
            root / "tmp/quality/standalone-performance-coverage/root-nextest-producer.lock",
        )
        for path in lock_paths:
            path.parent.mkdir(parents=True, exist_ok=True)
        for path in lock_paths:
            locks.enter_context(review_lock(path))
        yield


class FilePin(TypedDict):
    path: str
    sha256: str


class SizedFilePin(FilePin):
    size: int


class SourceFile(FilePin):
    size: int


class Viewport(TypedDict):
    width: int
    height: int
    scale: int


class OperationPlan(TypedDict):
    fixture_sha256: str
    operation: str
    x: float
    y: float
    expected_dom_state_path: str
    expected_dom_state_sha256: str


class CatalogOperationPlan(TypedDict):
    profile: str
    conditionId: str
    caseId: str
    state: str
    operationKind: str
    targetSelector: str | None
    fixtureSha256: str
    viewport: Viewport
    operationEvidencePath: str
    operationEvidenceSha256: str
    evidenceBinarySha256: str
    operationStream: list[gate.JsonObject]


class CatalogCondition(TypedDict):
    condition_id: str
    case_id: str
    state: str
    operation_kind: str
    target_selector: str | None
    fixture: Path
    fixture_sha256: str
    viewport: Viewport
    operation_plan: CatalogOperationPlan
    operation_evidence: FilePin
    capture: FilePin
    chrome_capture: SizedFilePin
    chrome_receipt: SizedFilePin
    font_identity_request: SizedFilePin
    font_identity_response: SizedFilePin
    font_identity_binary_path: str
    font_identity_binary_sha256: str
    link_captures: list[FilePin]


class Provenance(TypedDict):
    rustc: str
    cargo: str
    environment_sha256: str


class ArmPin(TypedDict):
    binary_sha256: str
    source_snapshot_sha256: str
    lock_sha256: str
    build_artifact_ref: FilePin


class ToolContext(TypedDict):
    rustc: str
    cargo: str
    platform: str
    cpuCount: int | None
    environmentSha256: str


class ContextSnapshot(TypedDict):
    source: gate.JsonObject
    inputs: list[gate.JsonObject]
    registry: gate.JsonObject
    cache: gate.JsonObject
    cacheUnion: dict[str, gate.JsonObject]
    scope: dict[str, str]
    config: list[gate.JsonObject]
    preference: gate.JsonObject
    toolContext: ToolContext


class BatchOptions(TypedDict, total=False):
    max_conditions: int
    resume_batch: Path
    reference_parent_wrapper: Path
    reference_parent_wrapper_sha256: str


def _environment_sha256(environment: Mapping[str, str]) -> str:
    canonical = json.dumps(dict(environment), sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(canonical.encode()).hexdigest()


def _performance_worker_environment(rustc: str, cargo: str, environment_sha: str,
                                    lock: Path, snapshot: FilePin) -> dict[str, str]:
    return {**os.environ, "KRR_PERF_RUSTC": rustc, "KRR_PERF_CARGO": cargo,
            "KRR_PERF_ENVIRONMENT_SHA256": environment_sha, "KRR_PERF_LOCK_PATH": str(lock),
            "KRR_PERF_SOURCE_SNAPSHOT_PATH": snapshot["path"]}


class _ExpectedFields(TypedDict):
    fixture_sha256: str
    operation_plan_sha256: str
    operation: str
    expected_dom_state_sha256: str
    viewport: Viewport
    provenance: Provenance
    arms: dict[str, ArmPin]
    reportPins: list[FilePin]
    operationPlanRef: FilePin
    domStateRef: FilePin


class ExpectedPerformance(_ExpectedFields, total=False):
    sourcePair: gate.JsonObject
    endpointRefs: dict[str, FilePin]
    catalogCondition: gate.JsonObject
    operationEvidenceRef: FilePin
    catalogManifestRef: FilePin
    catalogPlanRef: FilePin
    chromeCaptureRef: SizedFilePin
    chromeReceiptRef: SizedFilePin
    catalogCaptureValidationRef: FilePin
    fontIdentityRequestRef: SizedFilePin
    fontIdentityResponseRef: SizedFilePin
    fontIdentityProbeBinary: FilePin
    catalogBinaryProofRef: FilePin
    catalogBuildReportRef: FilePin
    catalogCaptureBinaryRef: FilePin
    catalogCurrentBuildReportRef: FilePin
    referenceParent: gate.JsonObject


class ConditionWorkerCollection(NamedTuple):
    order: list[gate.JsonObject]
    reports: list[Path]
    report_pins: list[FilePin]
    failures: list[gate.JsonObject]
    worker_attempt_pins: list[FilePin]
    actual_phase_records: int


class CapacityBinding(NamedTuple):
    document: Path
    document_sha256: str
    state: Path
    state_sha256: str
    registry: Path
    registry_sha256: str
    references: tuple[str, ...]


def _catalog_pair_manifest_fields(plan: Mapping[str, object], condition_id: object,
                                  evidence: object) -> gate.JsonObject:
    condition = gate._text(condition_id, "catalog condition id")
    gate.require(plan.get("conditionId") == condition, "catalog pair condition differs from plan")
    binary_sha = gate._text(plan.get("evidenceBinarySha256"), "catalog evidence binary SHA")
    gate.digest(binary_sha)
    return {"condition_id": condition, "operation_evidence_ref": evidence,
            "binarySha256": binary_sha}


def capacity_transport_refs(binding: CapacityBinding) -> list[FilePin]:
    return [
        {"path": str(binding.document), "sha256": binding.document_sha256},
        {"path": str(binding.state), "sha256": binding.state_sha256},
        {"path": str(binding.registry), "sha256": binding.registry_sha256},
    ]


def _pinned_file(path: Path, checksum: str) -> None:
    gate.require(path.is_absolute() and path.resolve() == path and path.is_file()
                 and not path.is_symlink(), "capacity input must be a canonical regular file")
    gate.require(len(checksum) == 64 and all(c in "0123456789abcdef" for c in checksum), "capacity input SHA256 is invalid")
    gate.require(gate.sha256(path) == checksum, "capacity input SHA256 differs")


def _catalog_probe_file(value: object, label: str) -> SizedFilePin:
    pin = gate._object(value, label)
    path = Path(gate._text(pin.get("path"), label + " path"))
    checksum = gate._text(pin.get("sha256"), label + " SHA")
    size = gate._integer(pin.get("sizeBytes"), label + " size")
    _pinned_file(path, checksum)
    gate.require(path.stat().st_size == size, label + " size differs")
    return {"path": str(path), "sha256": checksum, "size": size}


def _pinned_json(path: Path, checksum: str) -> gate.JsonObject:
    _pinned_file(path, checksum)
    raw = path.read_bytes()
    gate.require(hashlib.sha256(raw).hexdigest() == checksum, "capacity input SHA256 differs")
    return gate._object(json.loads(raw, object_pairs_hook=gate.unique_object), "capacity input")


def load_capacity_binding(root: Path, document: Path, checksum: str) -> CapacityBinding:
    import cargo_capacity_guard as capacity
    from cargo_capacity_guard_native_cli_observation import protected_identity

    value = _pinned_json(document, checksum)
    gate.require(value.get("schema") == "krr-html-performance-capacity-binding-v1", "capacity binding schema differs")
    state_ref = gate._object(value["state"], "capacity state pin")
    registry_ref = gate._object(value["protectedRegistry"], "protected registry pin")
    state = Path(gate._text(state_ref["path"], "capacity state path"))
    state_sha = gate._text(state_ref["sha256"], "capacity state SHA256")
    registry = Path(gate._text(registry_ref["path"], "protected registry path"))
    registry_sha = gate._text(registry_ref["sha256"], "protected registry SHA256")
    _pinned_file(state, state_sha)
    _pinned_file(registry, registry_sha)
    saved = capacity.load_state(state, state_sha)
    gate.require(saved.get("target") == str(root / "target") and saved.get("metric") == capacity.CAPACITY_METRIC,
                 "capacity state target/metric differs")
    gate.require(gate._integer(saved["metric_version"], "capacity metric version") == capacity.CAPACITY_METRIC_VERSION,
                 "capacity metric version differs")
    gate.require(gate._integer(saved["baseline_bytes"], "capacity baseline") == LOCAL_CAPACITY_BASELINE
                 and gate._numeric(saved["factor"], "capacity factor") == 2,
                 "fixed capacity baseline/factor differs")
    registry_value = _pinned_json(registry, registry_sha)
    gate.require(set(registry_value) == {"schema", "physicalRefCount", "refs"},
                 "protected registry fields differ")
    gate.require(registry_value["schema"] == PROTECTED_REGISTRY_SCHEMA,
                 "protected registry schema differs")
    physical_count = gate._integer(registry_value["physicalRefCount"], "protected reference count")
    records = gate._objects(registry_value["refs"], "protected references")
    gate.require(physical_count > 0 and physical_count == len(records),
                 "protected reference count differs from registry records")
    references: list[str] = []
    physical_identities: list[tuple[int, int]] = []
    for record in records:
        validated = protected_identity(record)
        path = Path(gate._text(validated["path"], "protected reference path"))
        references.append(str(path))
        physical_identities.append((gate._integer(validated["dev"], "protected dev"),
                                    gate._integer(validated["ino"], "protected ino")))
    gate.require(len(set(references)) == len(records),
                 "protected reference paths are not unique")
    gate.require(len(set(physical_identities)) == len(records),
                 "protected reference physical identities are not unique")
    return CapacityBinding(document, checksum, state, state_sha, registry, registry_sha, tuple(references))


ExpectedBuild = gate.ExpectedBuild


class ArmBuildResult(NamedTuple):
    configs: list[Path]
    binary: Path
    binary_sha256: str
    proof: gate.JsonObject
    pins: list[FilePin]


def _bytes(value: object, label: str) -> bytes:
    if not isinstance(value, bytes):
        raise ValueError(label + " must be bytes")
    return value


def _canonical_file(value: object, label: str) -> Path:
    path = Path(gate._text(value, label))
    gate.require(path.is_absolute() and path.resolve() == path and not path.is_symlink()
                 and path.is_file(), label + " must be a canonical regular file")
    return path


expected_build = gate.expected_build
cargo_build_artifacts = gate.cargo_build_artifacts


def _file_identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns


def _guard_python_pin() -> gate.JsonObject:
    launcher = Path(sys.executable)
    gate.require(launcher.is_absolute() and str(launcher) == sys.executable and launcher.is_file()
                 and os.access(launcher, os.X_OK), "guard Python launcher is invalid")
    executable = launcher.resolve(strict=True)
    gate.require(executable.is_file() and not executable.is_symlink() and os.access(executable, os.X_OK),
                 "guard Python executable is invalid")
    return {"launcher": str(launcher), "executable": {"path": str(executable), "sha256": gate.sha256(executable)}}


def freeze_binary(source: Path, destination: Path) -> str:
    _canonical_file(str(source), "selected binary")
    gate.require(destination.is_absolute() and destination.parent.resolve() == destination.parent, "frozen destination must be canonical")
    no_follow: object = getattr(os, "O_NOFOLLOW", None)
    if isinstance(no_follow, bool) or not isinstance(no_follow, int) or no_follow <= 0:
        raise ValueError("binary freeze requires a no-follow file API")
    flags = os.O_RDONLY | no_follow
    descriptor = os.open(source, flags)
    try:
        with os.fdopen(descriptor, "rb") as reader:
            descriptor = -1
            before = os.fstat(reader.fileno())
            gate.require(stat.S_ISREG(before.st_mode), "selected binary must be regular")
            destination.parent.mkdir(parents=True, exist_ok=False)
            checksum = hashlib.sha256()
            with destination.open("xb") as writer:
                while block := reader.read(1024 * 1024):
                    checksum.update(block)
                    writer.write(block)
            _canonical_file(str(destination), "frozen binary")
            frozen_identity = _file_identity(destination.lstat())
            after = os.fstat(reader.fileno())
            gate.require(_file_identity(before) == _file_identity(after) == _file_identity(source.lstat()), "selected binary identity changed during freeze")
            reader.seek(0)
            original_hash = hashlib.sha256()
            while block := reader.read(1024 * 1024):
                original_hash.update(block)
            original_checksum = original_hash.hexdigest()
            gate.require(_file_identity(before) == _file_identity(os.fstat(reader.fileno()))
                         == _file_identity(source.lstat()), "selected binary identity changed during freeze")
            gate.require(original_checksum == checksum.hexdigest() == gate.sha256(destination), "frozen binary SHA differs")
            gate.require(_file_identity(destination.lstat()) == frozen_identity, "frozen binary identity changed")
            destination.chmod(0o555)
            return checksum.hexdigest()
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def guarded_arm_build(root: Path, harness: Path, arm: str, binding: CapacityBinding,
                      source_records: gate.SourceRecords, output: Path) -> ArmBuildResult:
    import cargo_capacity_guard as capacity

    gate.require(arm in {"baseline", "candidate"}, "measurement arm differs")
    target = root / "target"
    gate.require(target.resolve() == target and not target.is_symlink(), "canonical capacity target required")
    environment = dict(os.environ)
    environment["CARGO_TARGET_DIR"] = str(target)
    references = capacity.collect_protected_references(list(binding.references), environment.get("CARGO_CAPACITY_PROTECTED_REFS"))
    for protected_reference in references:
        path = Path(protected_reference)
        gate.require(path.is_file() and not path.is_symlink() and path.resolve() == path,
                     "protected reference must be a canonical regular file")
    factor = environment.get("CARGO_CAPACITY_FACTOR")
    if factor is not None:
        try:
            gate.require(float(factor) == 2, "capacity environment factor differs")
        except ValueError as error:
            raise ValueError("capacity environment factor differs") from error
    capacity.validate_build_selection(harness, environment)
    configs = capacity.cargo_configuration_paths(harness, environment)
    lifecycle_logs = output.parent / f"{output.name}.lifecycle"

    def rustup_query(argv: Sequence[str], cwd: Path, environment: Mapping[str, str]) -> str:
        return command(argv, text=True, cwd=cwd, env=environment, role="tool-query",
                       log_directory=lifecycle_logs)

    def resolve_tool(name: str, cwd: Path) -> gate.RustTool:
        return gate.resolve_rust_tool(root, name, environment, cwd=cwd, query=rustup_query)

    cargo_tool = resolve_tool("cargo", root)
    rustc_tool = resolve_tool("rustc", root)
    gate.require(cargo_tool == resolve_tool("cargo", harness)
                 and rustc_tool == resolve_tool("rustc", harness),
                 "Rust toolchain selection differs between repository and harness")
    cargo, rustc = cargo_tool.executable, rustc_tool.executable
    cargo_launcher_pin = gate.rust_tool_launcher_pin(cargo_tool)
    rustc_launcher_pin = gate.rust_tool_launcher_pin(rustc_tool)
    cargo_info = {"path": str(cargo), "sha256": gate.sha256(cargo)}
    rustc_info = {"path": str(rustc), "sha256": gate.sha256(rustc)}
    capacity.validate_build_executable(root, [str(cargo_tool.launcher), "build"], environment)
    required = [harness / "Cargo.toml", harness / "Cargo.lock", root / "scripts/build/cargo_capacity_guard.py",
                root / "scripts/build/cargo_capacity_guard_lock.py", harness / "src/main.rs",
                root / "Cargo.toml", root / "crates/katana-render-runtime/Cargo.toml"]
    for path in required:
        relative = str(path.relative_to(root))
        gate.require(not path.is_symlink() and path.resolve() == path and relative in source_records
                     and source_records[relative] == (gate.sha256(path), path.stat().st_size),
                     "capacity/manifest source binding differs")
    gate.require(gate.sha256(harness / "Cargo.lock") == gate.LOCK_SHA, "common lock pin differs")
    config_pins = [(path, gate.sha256(path)) for path in configs]
    metadata = _measurement_command(
        [str(cargo_tool.launcher), "metadata", "--format-version", "1", "--locked", "--manifest-path",
         str(harness / "Cargo.toml"), "--no-default-features", "--features", arm],
        cwd=harness, env=environment, role="metadata",
        log_directory=lifecycle_logs,
    )
    gate.require(gate._integer(metadata.returncode, "metadata exit") == 0, "metadata failed")
    metadata_stdout = _bytes(metadata.stdout, "metadata stdout")
    metadata_stderr = _bytes(metadata.stderr, "metadata stderr")
    value = gate._object(json.loads(metadata_stdout.decode("utf8"), object_pairs_hook=gate.unique_object), "standalone Cargo metadata")
    gate.require(value.get("workspace_root") == str(harness) and value.get("target_directory") == str(target),
                 "standalone metadata root/target differs")
    retained = capacity.current_build_profiles(target, value, "release")
    capacity.validate_build_selection(harness, environment)
    gate.require(capacity.cargo_configuration_paths(harness, environment) == configs, "Cargo configuration selection changed during metadata")
    gate.require(all(gate.sha256(path) == pin for path, pin in config_pins), "Cargo configuration changed during metadata")
    for path in required:
        gate.require(not path.is_symlink() and path.resolve() == path
                     and source_records[str(path.relative_to(root))] == (gate.sha256(path), path.stat().st_size),
                     "capacity/manifest source changed during metadata")
    expected = expected_build(root, harness, arm, value, environment)
    cargo_version = command([str(cargo_tool.launcher), "-vV"], text=True,
        cwd=root, env=environment, role="tool-query", log_directory=lifecycle_logs)
    rustc_version = command([str(rustc_tool.launcher), "-vV"], text=True,
        cwd=root, env=environment, role="tool-query", log_directory=lifecycle_logs)
    load_capacity_binding(root, binding.document, binding.document_sha256)
    guard_python = _guard_python_pin()
    argv = gate.guarded_build_argv(root, harness, cargo_tool.launcher, arm, binding.state, binding.state_sha256,
                                   references, [str(profile) for profile in retained])
    output.mkdir(parents=True, exist_ok=False)
    build_lifecycle: list[Path] = []
    result = _measurement_command(argv, cwd=root, env=environment, check=False, role="build",
                                  log_directory=lifecycle_logs, lifecycle_sink=build_lifecycle.append)
    gate.require(len(build_lifecycle) == 1 and _guard_python_pin() == guard_python,
                 "guard Python or build lifecycle changed")
    lifecycle_path = build_lifecycle[0]
    lifecycle_pin: FilePin = {"path": str(lifecycle_path), "sha256": gate.sha256(lifecycle_path)}
    raw_stdout = _bytes(result.stdout, "guard stdout")
    raw_stderr = _bytes(result.stderr, "guard stderr")
    stdout_path, stderr_path, metadata_path = output / "cargo.jsonl", output / "guard-cargo.stderr", output / "metadata.json"
    stdout_path.write_bytes(raw_stdout)
    stderr_path.write_bytes(raw_stderr)
    metadata_path.write_bytes(metadata_stdout)
    metadata_stderr_path = output / "metadata.stderr"
    metadata_stderr_path.write_bytes(metadata_stderr)
    pins: list[FilePin] = [{"path": str(p), "sha256": gate.sha256(p)}
                           for p in (stdout_path, stderr_path, metadata_path, metadata_stderr_path)]
    build_exit = gate._integer(result.returncode, "build exit")
    if build_exit != 0:
        raise subprocess.CalledProcessError(build_exit, argv, output=raw_stdout, stderr=raw_stderr)
    binary, artifacts = cargo_build_artifacts(raw_stdout, expected, target)
    load_capacity_binding(root, binding.document, binding.document_sha256)
    capacity.validate_build_selection(harness, environment)
    gate.require(capacity.cargo_configuration_paths(harness, environment) == configs, "Cargo configuration selection changed during build")
    gate.require(all(gate.sha256(path) == pin for path, pin in config_pins), "Cargo configuration changed during build")
    for path in required:
        gate.require(not path.is_symlink() and path.resolve() == path
                     and source_records[str(path.relative_to(root))] == (gate.sha256(path), path.stat().st_size),
                     "capacity/manifest source changed during build")
    cargo_version_after = command([str(cargo_tool.launcher), "-vV"], text=True,
        cwd=root, env=environment, role="tool-query", log_directory=lifecycle_logs)
    rustc_version_after = command([str(rustc_tool.launcher), "-vV"], text=True,
        cwd=root, env=environment, role="tool-query", log_directory=lifecycle_logs)
    cargo_tool_after = resolve_tool("cargo", root)
    rustc_tool_after = resolve_tool("rustc", root)
    cargo_harness_after = resolve_tool("cargo", harness)
    rustc_harness_after = resolve_tool("rustc", harness)
    gate.require(cargo_tool_after == cargo_tool and rustc_tool_after == rustc_tool
                 and cargo_harness_after == cargo_tool_after and rustc_harness_after == rustc_tool_after
                 and cargo_version_after == cargo_version and rustc_version_after == rustc_version
                 and gate.sha256(cargo_tool.dispatcher) == cargo_launcher_pin["sha256"]
                 and gate.sha256(rustc_tool.dispatcher) == rustc_launcher_pin["sha256"]
                 and gate.sha256(cargo) == cargo_info["sha256"] and gate.sha256(rustc) == rustc_info["sha256"],
                 "Cargo/Rustc executable changed during build")
    frozen = output / "binary" / "krr-html-performance-harness"
    checksum = freeze_binary(binary, frozen)
    proof: gate.JsonObject = {"schema": "krr-html-performance-build-artifact-v3", "arm": arm,
        "argv": argv, "buildExit": build_exit, "expectedHarness": expected.harness,
        "guardPython": guard_python, "guardLifecycleRef": lifecycle_pin,
        "expectedRuntime": expected.runtime, "artifacts": artifacts, "rawPins": pins,
        "lockSha256": gate.LOCK_SHA, "selectedBinary": {"path": str(binary), "sha256": checksum},
        "frozenBinary": {"path": str(frozen), "sha256": checksum},
        "cargoLauncher": cargo_launcher_pin, "cargoExecutable": cargo_info,
        "cargoConfigPins": [{"path": str(path), "sha256": pin} for path, pin in config_pins],
        "rustcLauncher": rustc_launcher_pin, "rustcExecutable": rustc_info,
        "buildEnvironmentSha256": _environment_sha256(environment),
        "toolVersions": {"cargo": cargo_version, "rustc": rustc_version}}
    config_file_pins: list[FilePin] = [{"path": str(path), "sha256": pin} for path, pin in config_pins]
    python_ref = cast(FilePin, guard_python["executable"])
    return ArmBuildResult(configs, frozen, checksum, proof, [*pins, *config_file_pins, lifecycle_pin, python_ref])


def require_frozen_binary(build: ArmBuildResult) -> None:
    gate.require(not build.binary.is_symlink() and build.binary.resolve() == build.binary
                 and gate.sha256(build.binary) == build.binary_sha256,
                 "frozen build binary changed before measurement")


def expected_arm_pins(builds: Mapping[str, ArmBuildResult], snapshot_sha256: str,
                      build_artifact_refs: Mapping[str, FilePin], *,
                      reference_parent: reference.ReferenceParentProof | None = None,
                      reference_contract: gate.JsonObject | None = None) -> dict[str, ArmPin]:
    arms: dict[str, ArmPin] = {}
    baseline_id = (gate.baseline_arm_id({"referenceParent": reference_contract})
                   if reference_contract is not None else gate.DEFAULT_BASELINE_ARM_ID)
    for arm, build in builds.items():
        require_frozen_binary(build)
        source_snapshot = (reference_parent.source_snapshot_sha256
                           if arm == "baseline" and reference_parent is not None
                           else snapshot_sha256)
        arms[baseline_id if arm == "baseline" else arm] = {
            "binary_sha256": build.binary_sha256,
            "source_snapshot_sha256": source_snapshot,
            "lock_sha256": (reference_parent.lock_sha256
                            if arm == "baseline" and reference_parent is not None
                            else gate.LOCK_SHA),
            "build_artifact_ref": build_artifact_refs[arm],
        }
    return arms


def load_reference_parent(
    root: Path, wrapper: Path, wrapper_sha256: str, *,
    git_query: gate.GitQuery | None = None,
) -> tuple[reference.ReferenceParentProof, gate.JsonObject, FilePin, list[FilePin]]:
    wrapper = wrapper.resolve()
    _pinned_file(wrapper, wrapper_sha256)

    def query(path: Path, args: tuple[str, ...]) -> str:
        return gate.git(path, *args, git_query=git_query)
    proof = reference.load_reference_parent_proof(wrapper, root, git_query=query)
    contract = reference.reference_parent_contract(proof)
    wrapper_pin: FilePin = {"path": str(wrapper), "sha256": proof.wrapper_sha256}
    contract["wrapperRef"] = wrapper_pin
    refs: list[FilePin] = [wrapper_pin]
    for pin in proof.dependency_refs:
        refs.append({"path": pin.logical_path, "sha256": pin.sha256})
    return proof, contract, wrapper_pin, refs


def encoded(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def write(path: Path, value: object) -> FilePin:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))
    return {"path": str(path.resolve()), "sha256": gate.sha256(path)}


def _clone_transport_file(source: Path, destination: Path, checksum: str) -> None:
    from reflink_copy import reflink_or_copy

    gate.require(source != destination and not destination.exists() and not destination.is_symlink(),
                 "transport destination already exists or is source")
    before = source.lstat()
    _pinned_file(source, checksum)
    reflink_or_copy(source, destination)
    after = source.lstat()
    target = destination.lstat()
    gate.require(_file_identity(before) == _file_identity(after) and before.st_mode == after.st_mode,
                 "transport source identity changed")
    gate.require(stat.S_ISREG(target.st_mode) and (target.st_dev, target.st_ino) != (before.st_dev, before.st_ino)
                 and target.st_size == before.st_size and gate.sha256(destination) == checksum,
                 "transport clone identity differs")
    final = source.lstat()
    gate.require(_file_identity(before) == _file_identity(final) and before.st_mode == final.st_mode,
                 "transport source changed during validation")


def write_transport_bundle(bundle: Path, head: str, refs: Sequence[FilePin],
                           attempts: list[dict[str, FilePin]]) -> dict[str, str]:
    transport_files: list[gate.JsonObject] = []
    relative_paths: dict[str, str] = {}
    transport: gate.JsonObject = {"schema": "krr-html-performance-release-bundle-v1", "head": head,
                                  "files": transport_files, "attempts": attempts}
    bundle.mkdir()
    unique = {ref["path"]: ref for ref in refs}
    for index, ref in enumerate(unique.values()):
        original = Path(ref["path"])
        relative = f"payload/{index}/{original.name}"
        destination = bundle / relative
        destination.parent.mkdir(parents=True)
        if platform.system() == "Darwin":
            _clone_transport_file(original, destination, ref["sha256"])
        else:
            shutil.copyfile(original, destination)
        os.chmod(destination, stat.S_IMODE(original.stat().st_mode))
        transport_files.append({"originalPath": ref["path"], "relativePath": relative,
                                "sha256": ref["sha256"]})
        relative_paths[ref["path"]] = relative
    write(bundle / "bundle.json", transport)
    return relative_paths


BATCH_RECEIPT_SCHEMA = "krr-html-performance-batch-receipt-v1"


def _receipt_file_pin(value: object, label: str) -> FilePin:
    pin = gate._object(value, label)
    path = Path(gate._text(pin.get("path"), label + " path"))
    checksum = gate._text(pin.get("sha256"), label + " SHA")
    _pinned_file(path, checksum)
    return {"path": str(path), "sha256": checksum}


def _attempt_condition_id(bundle: gate.Bundle, attempt: Mapping[str, object]) -> str:
    expected = bundle.json(attempt["expected"])
    catalog = gate._object(expected.get("catalogCondition"), "batch catalog condition")
    return gate._text(catalog.get("condition_id"), "batch condition id")


def write_batch_receipt(
    output: Path,
    *,
    head: str,
    source_before: FilePin,
    source_end: FilePin,
    context_before: FilePin,
    context_end: FilePin,
    build_artifacts: Mapping[str, FilePin],
    build_pins: Sequence[FilePin],
    condition_ids: Sequence[str],
    complete_condition_ids: Sequence[str],
    policy_accepted: bool,
    attempts: Sequence[Mapping[str, FilePin]],
    bundle: Path,
) -> FilePin:
    ordered_ids = list(condition_ids)
    complete_ids = list(complete_condition_ids)
    gate.require(len(ordered_ids) == len(set(ordered_ids)), "batch condition ids are duplicated")
    gate.require(set(complete_ids) <= set(ordered_ids),
                 "batch complete condition is outside the batch")
    batch_attempts = [dict(attempt) for attempt in attempts]
    receipt: gate.JsonObject = {
        "schema": BATCH_RECEIPT_SCHEMA,
        "status": "closed",
        "formalAccepted": policy_accepted and len(complete_ids) == 84 and len(ordered_ids) == 84,
        "policyAccepted": policy_accepted,
        "head": head,
        "conditionIds": ordered_ids,
        "completeConditionIds": complete_ids,
        "source": {"before": source_before, "end": source_end},
        "contexts": {"before": context_before, "end": context_end},
        "buildArtifacts": dict(build_artifacts),
        "buildPins": list(build_pins),
        "attempts": batch_attempts,
        "bundle": {"path": str((bundle / "bundle.json").resolve()),
                    "sha256": gate.sha256(bundle / "bundle.json")},
    }
    return write_exclusive(output / "batch-receipt.json", receipt)


def _complete_batch_attempts(
    bundle: gate.Bundle,
    attempts: Sequence[dict[str, FilePin]],
    complete_ids: Sequence[str],
) -> list[dict[str, FilePin]]:
    selected = [attempt for attempt in attempts
                if _attempt_condition_id(bundle, attempt) in set(complete_ids)]
    gate.require(len(selected) == len(complete_ids),
                 "batch complete condition ids are duplicated")
    actual_ids = [_attempt_condition_id(bundle, attempt) for attempt in selected]
    gate.require(actual_ids == list(complete_ids),
                 "batch complete condition order differs from attempts")
    return selected


def load_closed_batch(
    batch: Path,
    root: Path,
    *,
    git_query: gate.GitQuery | None = None,
) -> tuple[gate.JsonObject, gate.Bundle, list[dict[str, FilePin]]]:
    receipt_path = batch / "batch-receipt.json"
    receipt = gate.read_json(receipt_path)
    gate.require(receipt.get("schema") == BATCH_RECEIPT_SCHEMA,
                 "unsupported HTML performance batch receipt")
    gate.require(receipt.get("status") == "closed",
                 "HTML performance batch endpoint is not closed")
    gate.require(type(receipt.get("policyAccepted")) is bool,
                 "batch policy result is missing")
    condition_ids = [gate._text(value, "batch condition id")
                     for value in gate._array(receipt.get("conditionIds"), "batch condition ids")]
    gate.require(len(condition_ids) == len(set(condition_ids)),
                 "batch condition ids are duplicated")
    bundle_ref = _receipt_file_pin(receipt.get("bundle"), "batch bundle")
    gate.require(Path(bundle_ref["path"]) == (batch / "bundle" / "bundle.json").resolve(),
                 "batch bundle path differs")
    bundle = gate.Bundle(Path(bundle_ref["path"]).parent)
    gate.require(bundle.manifest.get("head") == receipt.get("head"),
                 "batch bundle HEAD differs")
    all_attempts = [cast(dict[str, FilePin], attempt)
                    for attempt in gate._objects(receipt.get("attempts"), "batch attempts")]
    manifest_attempts = gate._objects(bundle.manifest.get("attempts"), "bundle attempts")
    gate.require(all_attempts == manifest_attempts, "batch receipt attempts differ from bundle")
    actual_ids = [_attempt_condition_id(bundle, attempt) for attempt in all_attempts]
    gate.require(actual_ids == condition_ids, "batch condition order differs from attempts")
    complete_ids = [gate._text(value, "complete batch condition id")
                    for value in gate._array(receipt.get("completeConditionIds"),
                                             "complete batch condition ids")]
    gate.require(len(complete_ids) == len(set(complete_ids))
                 and set(complete_ids) <= set(actual_ids),
                 "batch complete condition ids are invalid")
    attempts = _complete_batch_attempts(bundle, all_attempts, complete_ids)
    for label in ("source", "contexts"):
        section = gate._object(receipt.get(label), "batch " + label)
        for endpoint in ("before", "end"):
            pin = _receipt_file_pin(section.get(endpoint),
                                    "batch " + label + " " + endpoint)
            bundle.file(pin)
    for arm, value in gate._object(receipt.get("buildArtifacts"), "batch build artifacts").items():
        gate.require(arm in {"baseline", "candidate"}, "unknown batch build arm")
        bundle.file(_receipt_file_pin(value, "batch " + arm + " artifact"))
    for value in gate._array(receipt.get("buildPins"), "batch build pins"):
        bundle.file(_receipt_file_pin(value, "batch build pin"))
    validated_builds: Mapping[str, gate.JsonObject] = {}
    if attempts:
        first_expected = bundle.json(attempts[0]["expected"])
        validated_builds = gate.validate_expected_builds(
            bundle, root, first_expected, git_query=git_query,
        )
    for attempt in attempts:
        attempt_object: gate.JsonObject = {
            key: value for key, value in attempt.items()
        }
        gate.validate_attempt(bundle, root, attempt_object, git_query=git_query,
                             validated_builds=validated_builds)
    return receipt, bundle, attempts


def _row_condition_id(row: Mapping[str, object]) -> str:
    return gate._text(row.get("condition_id"), "measurement condition id")


def _same_arm_identity(left: Mapping[str, object], right: Mapping[str, object]) -> bool:
    return all(left.get(key) == right.get(key)
               for key in ("binary_sha256", "source_snapshot_sha256", "lock_sha256"))


def _select_batch_rows(
    rows: Sequence[gate.JsonObject], completed_ids: set[str], limit: int | None,
) -> list[gate.JsonObject]:
    remaining = [row for row in rows if _row_condition_id(row) not in completed_ids]
    return remaining if limit is None else remaining[:limit]


def _batch_mode(
    catalog_manifest_path: Path | None, max_conditions: int | None, resume_batch: Path | None,
) -> bool:
    return catalog_manifest_path is not None or max_conditions is not None or resume_batch is not None


def _batch_deadline(started_at: float, enabled: bool) -> float | None:
    return started_at + BATCH_WORK_BUDGET_SECONDS if enabled else None


def _batch_formal_acceptance(
    condition_ids: Sequence[str], complete_ids: Sequence[str], policy_accepted: bool,
) -> bool:
    return policy_accepted and len(condition_ids) == 84 and len(complete_ids) == 84


def _seal_failed_batch(
    failure: BaseException, seal: Callable[[], FilePin], summary: gate.JsonObject,
) -> NoReturn:
    seal()
    print(json.dumps(summary))
    raise failure


def _completed_batch_ids(
    resumed_ids: Iterable[str], planned_rows: Sequence[gate.JsonObject],
    completed_rows: Sequence[gate.JsonObject],
    results: Sequence[tuple[Path, ExpectedPerformance, gate.JsonObject, FilePin]],
) -> list[str]:
    gate.require(list(completed_rows) == list(planned_rows[:len(completed_rows)]),
                 "batch completed rows are not a planned prefix")
    gate.require(len(completed_rows) == len(results),
                 "batch completed rows differ from measured results")
    ids = list(resumed_ids)
    ids.extend(_row_condition_id(row) for row, (_, _, result, _) in zip(
        completed_rows, results, strict=True,
    ) if gate._object(result.get("policy"), "condition policy").get("status") != "incomplete")
    gate.require(len(ids) == len(set(ids)), "batch complete conditions are duplicated")
    return ids


def _measured_phase_record_count(
    results: Sequence[tuple[Path, ExpectedPerformance, gate.JsonObject, FilePin]],
) -> int:
    count = 0
    for _, _, result, _ in results:
        policy = gate._object(result.get("policy"), "condition policy")
        measured = gate._integer(policy.get("phaseRecords"), "measured phase records")
        gate.require(measured >= 0, "measured phase records must be non-negative")
        count += measured
    return count


def _saved_phase_record_count(results: Sequence[gate.JsonObject]) -> int:
    count = 0
    for result in results:
        policy = gate._object(result.get("policy"), "condition policy")
        measured = gate._integer(policy.get("phaseRecords"), "measured phase records")
        gate.require(measured >= 0, "measured phase records must be non-negative")
        count += measured
    return count


def _resume_condition_plan(
    batch: Path, bundle: gate.Bundle, condition_id: str,
) -> tuple[Path, gate.JsonObject, FilePin]:
    label = re.sub(r"[^a-zA-Z0-9.-]+", "_", condition_id)
    path = batch / "conditions" / label / "operation-plan.json"
    pin: FilePin = {"path": str(path.resolve()), "sha256": gate.sha256(path)}
    _pinned_file(path, pin["sha256"])
    bundle.file(pin)
    return path, bundle.json(pin), pin


def _resume_context_pin(
    bundle: gate.Bundle, receipt: Mapping[str, object], current: FilePin, previous_pin: FilePin,
) -> FilePin:
    contexts = gate._object(receipt.get("contexts"), "resume context refs")
    previous = bundle.json(contexts["before"])
    rows = gate._objects(previous.get("inputs"), "resumed context inputs")
    matches = [row for row in rows if gate._text(row.get("path"),
               "resumed context input path") == previous_pin["path"]]
    gate.require(len(matches) == 1, "resume context input is ambiguous or missing")
    row = matches[0]
    pin: FilePin = {"path": gate._text(row.get("path"), "resume snapshot path"),
                    "sha256": gate._text(row.get("sha256"), "resume snapshot SHA")}
    gate.require(row.get("exists") is True and pin == previous_pin
                 and pin["sha256"] == current["sha256"],
                 "resume context input differs")
    _pinned_file(Path(current["path"]), current["sha256"])
    _pinned_file(Path(pin["path"]), pin["sha256"])
    bundle.file(pin)
    return pin


def _resume_candidate_snapshot_pin(bundle: gate.Bundle, receipt: Mapping[str, object]) -> FilePin:
    artifacts = gate._object(receipt.get("buildArtifacts"), "resume build artifacts")
    artifact = _receipt_file_pin(artifacts.get("candidate"), "resume candidate build artifact")
    proof = bundle.json(artifact)
    return _receipt_file_pin(proof.get("sourceSnapshotRef"), "resume candidate source snapshot")


def _resume_catalog_validation_pin(bundle: gate.Bundle, receipt: Mapping[str, object]) -> FilePin:
    attempts = gate._objects(receipt.get("attempts"), "resume attempts")
    gate.require(bool(attempts), "resume catalog validation declaration is missing")
    pins = [
        _receipt_file_pin(bundle.json(attempt["expected"]).get("catalogCaptureValidationRef"),
                          "resume catalog validation")
        for attempt in attempts
    ]
    gate.require(all(pin == pins[0] for pin in pins),
                 "resume catalog validation declarations differ")
    return pins[0]


def _validate_resume_batch(
    receipt: Mapping[str, object],
    bundle: gate.Bundle,
    attempts: Sequence[Mapping[str, FilePin]],
    *,
    head: str,
    source: gate.JsonObject,
    before_context: ContextSnapshot,
    after_context: ContextSnapshot,
    arms: Mapping[str, ArmPin],
) -> None:
    gate.require(receipt.get("head") == head, "resume batch HEAD differs")
    source_refs = gate._object(receipt.get("source"), "resume source refs")
    context_refs = gate._object(receipt.get("contexts"), "resume context refs")
    gate.require(bundle.json(source_refs["before"]) == source,
                 "resume source-before differs")
    gate.require(bundle.json(context_refs["before"]) == before_context,
                 "resume context-before differs")
    gate.require(bundle.json(context_refs["end"]) == after_context,
                 "resume context-end differs")
    for attempt in attempts:
        expected = bundle.json(attempt["expected"])
        old_arms = gate._object(expected.get("arms"), "resume expected arms")
        baseline_id = gate.baseline_arm_id(expected)
        gate.require(all(_same_arm_identity(gate._object(old_arms.get(raw_arm), "resume arm"),
                                            gate._object(arms.get(raw_arm), "current arm"))
                         for raw_arm in (baseline_id, "candidate")),
                     "resume build pins differ")

MeasurementRole = Literal["tool-query", "metadata", "preflight", "build", "worker"]
# 軽いquery、graph解決、同期検査、release build、単一workerに別の停止上限を置く。
MEASUREMENT_LIMITS: dict[MeasurementRole, float] = {
    "tool-query": 30, "metadata": 120, "preflight": 120, "build": 1800, "worker": 60,
}
MEASUREMENT_CLEANUP_SECONDS = 10.0
MEASUREMENT_TERM_SECONDS = 2.0
MAX_WORKER_ATTEMPTS = 3
MAX_WORKER_LOAD = 0.25
# 作業上限は新規worker起動だけを制限し、終了コンテキスト保存用の外側余裕は残す。
BATCH_WORK_BUDGET_SECONDS = 1_200.0


class BatchBudgetExhausted(RuntimeError):
    """A bounded batch stopped before starting another measurement process."""


def _batch_policy_accepted(
    batch_failure: BaseException | None,
    results: Sequence[gate.JsonObject],
    resumed_policy_accepted: bool,
) -> bool:
    if batch_failure is not None and not isinstance(batch_failure, BatchBudgetExhausted):
        return False
    if not resumed_policy_accepted:
        return False
    non_comparable_ids: set[str] = set()
    for result in results:
        policy = gate._object(result.get("policy"), "condition policy")
        if result.get("comparisonStatus") == "nonComparable":
            condition_id = gate._text(policy.get("conditionId"),
                                      "non-comparable condition id")
            fragment = gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES.get(condition_id)
            if (fragment is None or policy.get("fragment") != fragment
                    or policy.get("status") != "nonComparable"
                    or policy.get("policyPass") is not False
                    or policy.get("reason") != "known baseline fragment navigation refusal"
                    or policy.get("candidateOperationalPass") is not True
                    or type(policy.get("candidateSamples")) is not int
                    or policy.get("candidateSamples") != 25
                    or type(policy.get("phaseRecords")) is not int
                    or policy.get("phaseRecords") != len(gate.PHASES) * 25
                    or condition_id in non_comparable_ids):
                return False
            non_comparable_ids.add(condition_id)
            continue
        if (result.get("comparisonStatus") != "paired"
                or policy.get("policyPass") is not True):
            return False
    if len(results) == 84:
        return (not non_comparable_ids
                or non_comparable_ids == set(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES))
    return len(non_comparable_ids) <= len(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES)


def _require_batch_budget(deadline: float | None) -> None:
    if deadline is not None and time.monotonic() >= deadline:
        raise BatchBudgetExhausted("bounded HTML performance batch budget exhausted")


def _measurement_exit(process: subprocess.Popen[bytes]) -> bool:
    observed: object = process.poll()
    if observed is None:
        return False
    gate._integer(observed, "measurement exit code")
    return True


def _signal_interruption(signum: int) -> BaseException:
    if signum == signal.SIGINT:
        return KeyboardInterrupt()
    return InterruptedError(f"measurement interrupted by signal {signum}")


def _measurement_group_exists(pid: int) -> bool:
    try:
        os.killpg(pid, 0)
    except ProcessLookupError:
        return False
    return True


def _close_measurement_group(process: subprocess.Popen[bytes], deadline: float | None = None) -> list[str]:
    deadline = min(time.monotonic() + MEASUREMENT_CLEANUP_SECONDS,
                   deadline if deadline is not None else float("inf"))
    failures: list[BaseException] = []
    exited_signal_errors: list[str] = []

    def record_signal_error(error: PermissionError) -> None:
        # Darwinはzombieをgroup signal対象から除き、対象0件でもEPERMを返す。
        # reap後の消滅確認まで成功にはせず、この観測もraw記録へ残す。
        if platform.system() == "Darwin" and error.errno == errno.EPERM and _measurement_exit(process):
            exited_signal_errors.append(repr(error))
        else:
            failures.append(error)

    try:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except PermissionError as error:
            record_signal_error(error)
        term_deadline = min(deadline, time.monotonic() + MEASUREMENT_TERM_SECONDS)
        while time.monotonic() < term_deadline and not _measurement_exit(process):
            time.sleep(min(0.01, max(0.0, term_deadline - time.monotonic())))
    except BaseException as error:
        failures.append(error)
    # 親のexitだけで孫を残さず、起動時に固定した専用groupへ送る。
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        try:
            record_signal_error(error)
        except BaseException as observation_error:
            failures.append(observation_error)
    except BaseException as error:
        failures.append(error)
    # group観測・signalの失敗でも、直接所有するchildの回収を必ず試す。
    try:
        process.kill()
        process.wait(timeout=max(0.0, deadline - time.monotonic()))
    except BaseException as error:
        failures.append(error)
    # 最後のsignal後は残存を観測するだけとし、観測失敗は受入不可にする。
    try:
        last_probe_error: PermissionError | None = None
        while True:
            try:
                if not _measurement_group_exists(process.pid):
                    break
            except PermissionError as error:
                # 孫のzombieもinitによるreapまでEPERMになり得る。消滅へ読み替えない。
                last_probe_error = error
                note = "group observation: " + repr(error)
                if note not in exited_signal_errors:
                    exited_signal_errors.append(note)
            if time.monotonic() >= deadline:
                raise RuntimeError("measurement process group did not close before cleanup deadline") from last_probe_error
            time.sleep(min(0.01, max(0.0, deadline - time.monotonic())))
    except BaseException as error:
        failures.append(error)
    if failures:
        raise RuntimeError("measurement cleanup failed: " + "; ".join(repr(error) for error in failures)) from failures[0]
    return exited_signal_errors


def native_raw_fd_observer_path() -> Path:
    gate.require(platform.system() in ("Darwin", "Linux"), "native raw FD observer platform unsupported")
    path = Path("/usr/sbin/lsof" if platform.system() == "Darwin" else "/usr/bin/lsof").resolve(strict=True)
    gate.require(path.name == "lsof" and path.is_file() and path.stat().st_mode & 0o111 != 0,
                 "native raw FD observer executable unavailable")
    return path


def _writable_raw_holders(raw: bytes, identities: list[tuple[int, int]]) -> list[gate.JsonObject]:
    holders: list[gate.JsonObject] = []
    pid = group = 0
    record: dict[str, str] = {}

    def flush() -> None:
        if record:
            gate.require(all(key in record for key in ("D", "i", "f", "t")), "raw FD observer fields missing")
            pair = (int(record["D"], 16), int(record["i"]))
            gate.require(pair in identities and record["t"] == "REG", "raw FD observer identity differs")
            access = record.get("a", "unknown")
            # lsofのtxtは実行イメージのmappingであり、書込み可能な数値FDではない。
            text_mapping = record["f"] == "txt" and access.strip() == ""
            if access != "r" and not text_mapping:
                holders.append({"pid": pid, "processGroupId": group,
                                "descriptor": record["f"], "access": access})
    for line in raw.decode("ascii").splitlines():
        gate.require(bool(line), "empty raw FD observer field")
        key, value = line[0], line[1:]
        if key == "p":
            flush()
            record = {}
            pid = int(value)
            group = 0
        elif key == "g":
            group = int(value)
        elif key == "f":
            flush()
            record = {"f": value}
        else:
            gate.require(key in ("a", "t", "D", "i") and key not in record, "unknown/duplicate raw FD field")
            record[key] = value
    flush()
    return holders


def _native_raw_fd_closure(paths: tuple[Path, Path], identities: list[tuple[int, int]],
                           cwd: Path | None, env: Mapping[str, str] | None,
                           attempt: str, deadline: float) -> gate.JsonObject:
    if env is None or cwd is None:
        raise ValueError("native observer needs exact cwd/environment")
    gate.require([(path.stat().st_dev, path.stat().st_ino) for path in paths] == identities,
                 "native raw file identity changed")
    tool = native_raw_fd_observer_path()
    pin: gate.JsonObject = {"path": str(tool), "sha256": gate.sha256(tool), "size": tool.stat().st_size}
    argv = [str(tool), "-nP", "-FpgfatiD", "--", *(str(path) for path in paths)]
    stdout, stderr = paths[0].parent / "native-fd-stdout.raw", paths[0].parent / "native-fd-stderr.raw"
    process: subprocess.Popen[bytes] | None = None
    closed = False
    started = time.time_ns()
    remaining = deadline - time.monotonic() - 2 * MEASUREMENT_TERM_SECONDS
    gate.require(remaining > 0, "native FD observation cleanup deadline exceeded")
    with stdout.open("xb") as out, stderr.open("xb") as err:
        try:
            process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=out, stderr=err, start_new_session=True)
            identity: gate.JsonObject = {"pid": process.pid, "parentPid": os.getpid(),
                "sessionId": os.getsid(process.pid), "processGroupId": os.getpgid(process.pid)}
            gate.require(identity["sessionId"] == process.pid == identity["processGroupId"], "native FD observer group differs")
            process.wait(timeout=remaining)
        finally:
            if process is not None:
                _close_measurement_group(process, deadline)
                closed = True
    gate.require(process is not None and closed and process.returncode in (0, 1), "native FD observer failed")
    gate.require(not stderr.read_bytes(), "native FD observer coverage is unknown")
    gate.require({"path": str(tool), "sha256": gate.sha256(tool), "size": tool.stat().st_size} == pin,
                 "native FD observer tool changed")
    environment = dict(env)
    environment_sha = _environment_sha256(environment)
    return {"schema": "krr-native-raw-fd-closure-v1", "toolBefore": pin, "toolAfter": pin,
        "argv": argv, "cwd": str(cwd), "attemptToken": attempt,
        "environment": environment, "environmentSha256": environment_sha, "identity": identity,
        "actualExit": process.returncode, "startedNs": str(started), "endedNs": str(time.time_ns()),
        "reaped": True, "groupClosed": True, "rawFdsClosed": out.closed and err.closed,
        "runLimitSeconds": remaining, "cleanupLimitSeconds": MEASUREMENT_CLEANUP_SECONDS,
        "stdout": {"path": str(stdout), "sha256": gate.sha256(stdout), "size": stdout.stat().st_size},
        "stderr": {"path": str(stderr), "sha256": gate.sha256(stderr), "size": stderr.stat().st_size},
        "rawIdentities": [{"device": dev, "inode": ino} for dev, ino in identities],
        "holders": _writable_raw_holders(stdout.read_bytes(), identities), "descendantClosureProved": False}


def _measurement_command(args: Sequence[str], *, role: MeasurementRole, log_directory: Path,
                         cwd: Path | None = None, env: Mapping[str, str] | None = None,
                         check: bool = True, input_bytes: bytes | None = None,
                         lifecycle_sink: Callable[[Path], None] | None = None,
                         native_attempt_token: str | None = None) -> subprocess.CompletedProcess[bytes]:
    gate.require(os.name == "posix", "POSIX measurement ownership required")
    gate.require(role in MEASUREMENT_LIMITS, "unknown measurement command role")
    log_directory.mkdir(parents=True, exist_ok=True)
    gate.require(not log_directory.is_symlink(), "measurement raw directory must not be a symlink")
    folder = Path(tempfile.mkdtemp(prefix=f"{role}-", dir=log_directory))
    stdout_path, stderr_path = folder / "stdout.raw", folder / "stderr.raw"
    process: subprocess.Popen[bytes] | None = None
    result: subprocess.CompletedProcess[bytes] | None = None
    primary: BaseException | None = None
    cleanup_failure: BaseException | None = None
    cleanup_errors: list[BaseException] = []
    lifecycle_failure: BaseException | None = None
    started = time.monotonic()
    closed = False
    raw_closed = False
    identity: gate.JsonObject = {}
    previous_mask: Iterable[int] | None = None
    previous_handlers: dict[int, Callable[[int, FrameType | None], object] | int | None] = {}
    pending: list[int] = []
    signal_observations: list[str] = []
    raw_identities: list[tuple[int, int]] = []
    native_closure: gate.JsonObject | None = None
    cleanup_deadline: float | None = None

    def interrupted(signum: int, frame: FrameType | None) -> None:
        del frame
        pending.append(signum)

    try:
        with ExitStack() as stack:
            stdout = stack.enter_context(stdout_path.open("xb"))
            stderr = stack.enter_context(stderr_path.open("xb"))
            if native_attempt_token is not None:
                raw_identities = [(os.fstat(handle.fileno()).st_dev, os.fstat(handle.fileno()).st_ino)
                                  for handle in (stdout, stderr)]
            stdin: int | BinaryIO = subprocess.DEVNULL
            if input_bytes is not None:
                input_file = stack.enter_context(tempfile.TemporaryFile(mode="w+b"))
                input_file.write(input_bytes)
                input_file.flush()
                input_file.seek(0)
                gate.require(stat.S_ISREG(os.fstat(input_file.fileno()).st_mode),
                             "measurement input FD must be regular")
                stdin = input_file
            gate.require(stat.S_ISREG(os.fstat(stdout.fileno()).st_mode)
                         and stat.S_ISREG(os.fstat(stderr.fileno()).st_mode), "measurement raw FDs must be regular")
            try:
                # spawn中は例外を保留し、子へblocked signal maskを継承させない。
                for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
                    previous_handlers[signum] = signal.signal(signum, interrupted)
                process = subprocess.Popen(list(args), cwd=cwd, env=env, stdin=stdin,
                                           stdout=stdout, stderr=stderr, start_new_session=True)
                identity = {"pid": process.pid, "parentPid": os.getpid(),
                            "sessionId": os.getsid(process.pid), "processGroupId": os.getpgid(process.pid),
                            "spawnObservedMonotonic": time.monotonic(),
                            "observationLimit": "trusted dedicated session/group; non-atomic PID observations"}
                gate.require(identity["sessionId"] == process.pid and identity["processGroupId"] == process.pid,
                             "measurement process ownership differs")
                deadline = started + MEASUREMENT_LIMITS[role]
                while True:
                    if pending:
                        pending_signum = pending.pop(0)
                        signal_observations.append(f"signal {pending_signum} received while command was active")
                        raise _signal_interruption(pending_signum)
                    if time.monotonic() >= deadline:
                        raise subprocess.TimeoutExpired(list(args), MEASUREMENT_LIMITS[role])
                    if _measurement_exit(process):
                        break
                    time.sleep(min(0.01, max(0.0, deadline - time.monotonic())))
            except BaseException as error:
                primary = error
            finally:
                cleanup_deadline = time.monotonic() + MEASUREMENT_CLEANUP_SECONDS
                if process is not None:
                    try:
                        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM, signal.SIGHUP})
                    except BaseException as error:
                        cleanup_errors.append(error)
                    try:
                        signal_observations.extend(_close_measurement_group(process, cleanup_deadline))
                        closed = True
                    except BaseException as error:
                        cleanup_errors.append(error)
        raw_closed = True
        if native_attempt_token is not None:
            raw_closed = False
            try:
                if cleanup_deadline is None:
                    raise ValueError("native cleanup deadline missing")
                native_closure = _native_raw_fd_closure((stdout_path, stderr_path), raw_identities,
                                                       cwd, env, native_attempt_token, cleanup_deadline)
                gate.require(native_closure["holders"] == [], "native raw output has unknown writable holders; no unknown PID is signalled")
                raw_closed = True
            except BaseException as error:
                cleanup_errors.append(error)
                if primary is None:
                    primary = error
        mask_to_restore = previous_mask
        if mask_to_restore is not None:
            try:
                signal.pthread_sigmask(signal.SIG_SETMASK, mask_to_restore)
                previous_mask = None
            except BaseException as error:
                cleanup_errors.append(error)
                try:
                    signal.pthread_sigmask(signal.SIG_SETMASK, mask_to_restore)
                    previous_mask = None
                except BaseException as retry_error:
                    cleanup_errors.append(retry_error)
        if pending:
            signal_observations.extend(f"signal {pending_signum} deferred until cleanup completed"
                                       for pending_signum in pending)
            if primary is None:
                primary = _signal_interruption(pending[0])
            pending.clear()
        if primary is None:
            if process is None or not closed:
                raise ValueError("measurement process closure missing")
            returncode = gate._integer(process.returncode, "measurement exit code")
            result = subprocess.CompletedProcess(list(args), returncode, stdout_path.read_bytes(), stderr_path.read_bytes())
            if check:
                result.check_returncode()
    except BaseException as error:
        if primary is None:
            primary = error
        else:
            cleanup_errors.append(error)
    finally:
        if previous_mask is not None:
            try:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
                previous_mask = None
            except BaseException as error:
                cleanup_errors.append(error)
        if cleanup_errors:
            cleanup_failure = RuntimeError(
                "measurement cleanup failed: " + "; ".join(repr(error) for error in cleanup_errors)
            )
        try:
            write(folder / "lifecycle.json", {
                "schema": "krr-performance-command-lifecycle-v1", "argv": list(args), "role": role,
                "cwd": str(cwd) if cwd is not None else None, "identity": identity,
                "runLimitSeconds": MEASUREMENT_LIMITS[role], "cleanupLimitSeconds": MEASUREMENT_CLEANUP_SECONDS,
                "elapsedSeconds": time.monotonic() - started, "groupClosed": closed,
                "rawFdsClosed": raw_closed,
                "exitCode": process.returncode if process is not None else None,
                "failure": repr(primary) if primary is not None else None,
                "cleanupFailure": repr(cleanup_failure) if cleanup_failure is not None else None,
                "exitedLeaderSignalObservations": signal_observations,
                "raw": [{"path": str(path), "sha256": gate.sha256(path) if closed else None}
                        for path in (stdout_path, stderr_path)],
                **({"nativeRawFdClosure": native_closure, "descendantClosureProved": False}
                   if native_attempt_token is not None else {}),
            })
            if lifecycle_sink is not None:
                lifecycle_sink(folder / "lifecycle.json")
        except BaseException as error:
            lifecycle_failure = error
        finally:
            for restored_signum, handler in previous_handlers.items():
                signal.signal(restored_signum, handler)
            if pending and primary is None:
                # lifecycle.jsonの記録境界後の割込みは記録を書き換えず、呼出元へ必ず伝える。
                pending_signum = pending.pop(0)
                signal_observations.append(f"signal {pending_signum} received while lifecycle was being recorded")
                primary = _signal_interruption(pending_signum)
    if cleanup_failure is not None or lifecycle_failure is not None:
        failures = [label + ": " + repr(error) for label, error in (
            ("primary", primary), ("cleanup", cleanup_failure), ("lifecycle record", lifecycle_failure)
        ) if error is not None]
        message = "measurement cleanup failed" if cleanup_failure is not None else "measurement lifecycle record write failed"
        raise RuntimeError(message + "; " + "; ".join(failures)) from primary or cleanup_failure or lifecycle_failure
    if primary is not None:
        if isinstance(primary, subprocess.TimeoutExpired) and closed:
            primary.output, primary.stderr = stdout_path.read_bytes(), stderr_path.read_bytes()
        raise primary
    if result is None:
        raise ValueError("measurement result missing")
    return result


class NativeCliCommandEvidence(NamedTuple):
    observation: gate.JsonObject
    lifecycle: gate.JsonObject


class WorkerAttemptObservation(NamedTuple):
    record: gate.JsonObject
    result: subprocess.CompletedProcess[bytes] | None
    error: BaseException | None


def observe_native_cli_command(args: Sequence[str], *, role: MeasurementRole,
                               log_directory: Path, cwd: Path, env: Mapping[str, str],
                               attempt_token: str, label: str,
                               check: bool = True) -> NativeCliCommandEvidence:
    """CLI専用producerへ、既存ownerが閉じたraw記録だけを返す。"""
    gate.require(role in ("tool-query", "metadata", "preflight", "build"),
                 "native CLI command role differs")
    gate.require(len(attempt_token) == 64 and all(c in "0123456789abcdef" for c in attempt_token),
                 "native CLI attempt token differs")
    paths: list[Path] = []
    started = time.time_ns()
    result = _measurement_command(args, role=role, log_directory=log_directory, cwd=cwd,
                                  env=env, check=check, lifecycle_sink=paths.append,
                                  native_attempt_token=attempt_token)
    ended = time.time_ns()
    gate.require(len(paths) == 1, "native CLI lifecycle must be unique")
    lifecycle_path = paths[0]
    lifecycle = gate._object(gate.read_json(lifecycle_path), "native CLI lifecycle")
    gate.require(lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
                 and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None
                 and lifecycle.get("exitCode") == result.returncode,
                 "native CLI command closure differs")
    gate.require(not check or result.returncode == 0, "native CLI command failed")
    identity = gate._object(lifecycle["identity"], "native CLI process identity")
    raw = [gate._object(row, "native CLI raw") for row in gate._array(lifecycle["raw"], "raw")]
    pins: list[gate.JsonObject] = []
    for row in raw:
        path = Path(gate._text(row["path"], "native CLI raw path"))
        gate.require(gate.sha256(path) == row["sha256"], "native CLI raw changed")
        pins.append({"path": str(path), "sha256": row["sha256"], "size": path.stat().st_size})
    gate.require(len(pins) == 2 and result.stdout == Path(str(pins[0]["path"])).read_bytes()
                 and result.stderr == Path(str(pins[1]["path"])).read_bytes(), "native CLI raw bytes differ")
    environment = dict(env)
    return NativeCliCommandEvidence({"argv": list(args), "stdout": pins[0], "stderr": pins[1],
        "label": label, "cwd": str(cwd), "attemptToken": attempt_token,
        "environment": environment, "environmentSha256": _environment_sha256(environment),
        "pid": identity["pid"], "actualExit": result.returncode,
        "startedNs": str(started), "endedNs": str(ended),
        "reaped": True, "errors": []},
        {"path": str(lifecycle_path), "sha256": gate.sha256(lifecycle_path),
         "size": lifecycle_path.stat().st_size})


@overload
def command(args: Sequence[str], *, text: Literal[True], cwd: Path | None = None,
            env: Mapping[str, str] | None = None, role: MeasurementRole,
            log_directory: Path) -> str: ...


@overload
def command(args: Sequence[str], *, text: Literal[False] = False, cwd: Path | None = None,
            env: Mapping[str, str] | None = None, role: MeasurementRole,
            log_directory: Path) -> bytes: ...


def command(args: Sequence[str], *, text: bool = False, cwd: Path | None = None,
            env: Mapping[str, str] | None = None, role: MeasurementRole,
            log_directory: Path) -> str | bytes:
    result: object = _measurement_command(args, role=role, log_directory=log_directory, cwd=cwd, env=env).stdout
    if text:
        raw = _bytes(result, "command stdout")
        with io.TextIOWrapper(io.BytesIO(raw)) as stream:
            return gate._text(stream.read(), "command stdout")
    if not isinstance(result, bytes):
        raise ValueError("worker stdout must be bytes")
    return result


def _make_git_query(log_directory: Path) -> gate.GitQuery:
    def execute(root: Path, args: Sequence[str], input_bytes: bytes | None,
                text: bool) -> str | bytes:
        completed = _measurement_command(["git", "-C", str(root), *args], role="tool-query",
                                         log_directory=log_directory, cwd=root, input_bytes=input_bytes)
        if text:
            with io.TextIOWrapper(io.BytesIO(completed.stdout)) as stream:
                return gate._text(stream.read(), "Git query stdout")
        return completed.stdout

    @lru_cache(maxsize=1)
    def cached_cat_file_batch(root_path: str, args: tuple[str, ...], input_bytes: bytes) -> bytes:
        result = execute(Path(root_path), args, input_bytes, False)
        if not isinstance(result, bytes):
            raise ValueError("Git batch query returned text output")
        return result

    def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str | bytes:
        argv = tuple(args)
        object_ids = input_bytes[:-1].split(b"\n") if input_bytes and input_bytes.endswith(b"\n") else []
        stable_object_batch = bool(object_ids) and all(
            re.fullmatch(rb"(?:[0-9a-f]{40}|[0-9a-f]{64})", object_id) is not None
            for object_id in object_ids
        )
        if argv == ("cat-file", "--batch") and stable_object_batch and input_bytes is not None and not text:
            return cached_cat_file_batch(str(root.resolve()), argv, input_bytes)
        return execute(root, args, input_bytes, text)

    return query


def source(root: Path, *, git_query: gate.GitQuery | None = None) -> gate.JsonObject:
    gate.require(not gate.git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query), "clean committed source required before measurement")
    rows: list[SourceFile] = [{"path": str(root / name), "sha256": checksum, "size": size} for name, (checksum, size) in sorted(gate.committed_records(root, git_query=git_query).items())]
    index_path = Path(gate.git(root, "rev-parse", "--git-path", "index", git_query=git_query).strip())
    if not index_path.is_absolute():
        index_path = root / index_path
    payload: gate.JsonObject = {"schema": "krr-native-build-source-inventory-v1", "sourceManifestSchema": "krr-native-build-source-manifest-v1", "root": str(root), "head": gate.git(root, "rev-parse", "HEAD", git_query=git_query).strip(), "index": {"sha256": gate.sha256(index_path)}, "files": rows, "trackedPaths": [row["path"] for row in rows], "untrackedPaths": [], "missingTrackedPaths": [], "stableEndpoints": True}
    payload["inventoryDigest"] = hashlib.sha256(encoded(payload)).hexdigest()
    gate.current_source(root, payload, git_query=git_query)
    return payload


class BuildSourceInputs(NamedTuple):
    source_before: FilePin
    capacity_input: FilePin
    snapshot: FilePin
    source_records: gate.SourceRecords


def write_build_source_inputs(root: Path, output: Path, initial: gate.JsonObject,
                              binding: CapacityBinding, *, git_query: gate.GitQuery | None = None,
                              extra_capacity_inputs: Mapping[str, FilePin] | None = None) -> BuildSourceInputs:
    source_before = write(output / "source-before.json", initial)
    capacity_input_value: gate.JsonObject = {
        "sourceBeforeRef": source_before,
        "bindingRef": {"path": str(binding.document), "sha256": binding.document_sha256},
        "stateRef": {"path": str(binding.state), "sha256": binding.state_sha256},
        "protectedRegistryRef": {"path": str(binding.registry), "sha256": binding.registry_sha256},
        "protectedReferenceCount": len(binding.references), "target": str(root / "target"),
        "baselineBytes": LOCAL_CAPACITY_BASELINE, "factor": 2, "thresholdBytes": LOCAL_CAPACITY_THRESHOLD,
    }
    if extra_capacity_inputs is not None:
        capacity_input_value.update(extra_capacity_inputs)
    capacity_input = write(output / "capacity-inputs.json", capacity_input_value)
    snapshot_rows = [{**row, "path": str(Path(gate._text(row["path"], "source path")).relative_to(root))}
                     for row in gate._objects(initial["files"], "source files")]
    snapshot = write(output / "shared-source-snapshot.json", {
        "records": snapshot_rows,
        "digest": hashlib.sha256(encoded(snapshot_rows)).hexdigest(),
    })
    source_records = gate.current_source(root, initial, git_query=git_query)
    return BuildSourceInputs(source_before, capacity_input, snapshot, source_records)


def produce_candidate_build_proof(root: Path, output: Path, capacity_document: Path,
                                 capacity_document_sha256: str) -> gate.JsonObject:
    gate.require(platform.system() == "Darwin" and platform.machine() == "arm64",
                 "measurement lane requires macOS Apple Silicon")
    tmp_root = root / "tmp"
    gate.require(not tmp_root.is_symlink() and output.is_relative_to(tmp_root)
                 and not output.exists() and not output.is_symlink(),
                 "fresh ignored owned output required")
    lifecycle = output.parent / f"{output.name}.lifecycle"
    gate.require(not lifecycle.exists() and not lifecycle.is_symlink(),
                 "fresh measurement lifecycle output required")
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    git_query = _make_git_query(lifecycle)
    initial = source(root, git_query=git_query)
    _measurement_command(["just", "runtime-bundle-check"], cwd=root,
                         role="preflight", log_directory=lifecycle)
    gate.require(source(root, git_query=git_query) == initial,
                 "source changed during runtime bundle check")
    binding = load_capacity_binding(root, capacity_document, capacity_document_sha256)
    output.mkdir(parents=True)
    build_inputs = write_build_source_inputs(root, output, initial, binding, git_query=git_query)
    harness = root / gate.HARNESS_ROOT
    gate.require(gate.sha256(harness / "Cargo.lock") == gate.LOCK_SHA, "common lock pin differs")
    build = guarded_arm_build(root, harness, "candidate", binding, build_inputs.source_records,
                              output / "builds" / "candidate")
    build.proof.update({"sourceBeforeRef": build_inputs.source_before,
                        "sourceSnapshotRef": build_inputs.snapshot,
                        "capacityInputRef": build_inputs.capacity_input})
    artifact_pin = write(output / "builds" / "candidate" / "build-artifact.json", build.proof)
    source_end = write(output / "source-end.json", source(root, git_query=git_query))
    gate.require(gate.read_json(Path(source_end["path"])) == initial,
                 "source changed during candidate build")
    refs: list[FilePin] = [build_inputs.source_before, source_end, build_inputs.snapshot,
                           build_inputs.capacity_input, artifact_pin, *build.pins,
                           {"path": str(build.binary), "sha256": build.binary_sha256},
                           *capacity_transport_refs(binding)]
    for log_directory in (lifecycle, *(output / "builds").glob("*.lifecycle")):
        for path in sorted(log_directory.rglob("*")):
            if path.is_file():
                refs.append({"path": str(path), "sha256": gate.sha256(path)})
    bundle = output / "bundle"
    relative_paths = write_transport_bundle(bundle, gate._text(initial["head"], "source HEAD"), refs, [])
    bundle_sha256 = gate.sha256(bundle / "bundle.json")
    relative_artifact = relative_paths[artifact_pin["path"]]
    facts = gate.validate_candidate_build_proof(
        bundle, root, bundle_sha256, relative_artifact, artifact_pin["sha256"],
        git_query=git_query,
    )
    gate.require(source(root, git_query=git_query) == initial,
                 "source changed during candidate build proof validation")
    relative_binary = relative_paths[str(build.binary)]
    return {
        "schema": "krr-html-candidate-build-proof-result-v1",
        "buildProofValidated": True,
        "htmlPerformanceAcceptance": "not-evaluated",
        "bundleManifest": {"path": str(bundle / "bundle.json"), "sha256": bundle_sha256},
        "candidateBuildArtifact": {"path": str(bundle / relative_artifact),
                                   "sha256": artifact_pin["sha256"]},
        "candidateProbeBinary": {"path": str(bundle / relative_binary),
                                  "sha256": build.binary_sha256},
        "candidateBuildFacts": facts,
    }


def inventory(path: Path) -> gate.JsonObject:
    return inventory_ops.inventory(path)


def file_state(path: Path) -> gate.JsonObject:
    return {"path": str(path), "exists": path.is_file(), "sha256": gate.sha256(path) if path.is_file() else None}


def context(root: Path, target: Path, input_paths: Sequence[Path], log_directory: Path,
            *, git_query: gate.GitQuery | None = None) -> ContextSnapshot:
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
    registry = inventory(cargo_home / "registry")
    cache = inventory(target)
    scope = {"registry": str(cargo_home / "registry"), "target": str(target)}
    return {"source": source(root, git_query=git_query), "inputs": [file_state(p) for p in input_paths], "registry": registry, "cache": cache, "cacheUnion": {"registry": registry, "target": cache}, "scope": scope, "config": [file_state(p) for p in (root / ".cargo/config.toml", root / ".cargo/config", cargo_home / "config.toml", cargo_home / "config")], "preference": file_state(Path.home() / ".CFUserTextEncoding"), "toolContext": {"rustc": command(["rustc", "-vV"], text=True, role="tool-query", log_directory=log_directory), "cargo": command(["cargo", "-vV"], text=True, role="tool-query", log_directory=log_directory), "platform": platform.platform(), "cpuCount": os.cpu_count(), "environmentSha256": _environment_sha256(os.environ)}}


def quiet() -> float:
    ratio = os.getloadavg()[0] / (os.cpu_count() or 1)
    return ratio


def await_quiet(batch_deadline: float | None = None) -> float:
    quiet_deadline = time.monotonic() + 900
    while quiet() > MAX_WORKER_LOAD:
        _require_batch_budget(batch_deadline)
        gate.require(time.monotonic() < quiet_deadline, "quiet load0.25 was not reached")
        sleep_seconds = 5.0
        if batch_deadline is not None:
            sleep_seconds = min(sleep_seconds, max(0.0, batch_deadline - time.monotonic()))
        time.sleep(sleep_seconds)
    _require_batch_budget(batch_deadline)
    return quiet()


def worker_attempts_directory(folder: Path, arm: str, pair: int) -> Path:
    gate.require(arm in ("baseline", "candidate"), "unknown performance arm")
    gate.require(pair >= 0, "negative performance pair index")
    return folder / "worker-attempts" / f"{arm}-{pair}"


def worker_sample(stdout: bytes, sample: int, viewport: Viewport) -> gate.JsonObject:
    payload = gate._object(json.loads(stdout, object_pairs_hook=gate.unique_object), "worker sample")
    actual_viewport = gate._object(payload["viewport"], "worker viewport")
    gate.require(gate._integer(payload["sample"], "worker sample id") == sample and actual_viewport == viewport,
                 "worker sample/viewport differs")
    for field in ("width", "height"):
        gate._integer(actual_viewport[field], "worker viewport " + field)
    gate._numeric(actual_viewport["scale"], "worker viewport scale")
    return payload


def write_exclusive(path: Path, value: object) -> FilePin:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp",
                                                  dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded(value))
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)
    return {"path": str(path.resolve()), "sha256": gate.sha256(path)}


def binary_sha_or_none(path: Path) -> str | None:
    try:
        if path.is_symlink() or path.resolve() != path:
            return None
        return gate.sha256(path)
    except (OSError, RuntimeError):
        return None


def worker_started_from_lifecycle(path: Path) -> bool | None:
    try:
        lifecycle = gate._object(gate.read_json(path), "worker lifecycle")
        identity = lifecycle.get("identity")
        if not isinstance(identity, dict):
            return None
        pid = identity.get("pid")
    except (OSError, ValueError):
        return None
    return True if type(pid) is int and pid > 0 else None


def _closed_nonzero_baseline_failure(
    attempts: Sequence[gate.JsonObject], error: BaseException,
    expected: ExpectedPerformance,
) -> bool:
    fragment = gate.non_comparable_baseline_fragment(expected)
    if (fragment is None or not isinstance(error, subprocess.CalledProcessError)
            or type(error.returncode) is not int or error.returncode != 1 or not attempts):
        return False
    record = attempts[-1]
    actual_exit = record.get("actualExit")
    lifecycle_value = record.get("lifecycle")
    expected_baseline = gate.baseline_arm_id(expected)
    expected_arms = gate._object(expected.get("arms"), "expected arms")
    expected_arm = gate._object(expected_arms.get(expected_baseline), "expected baseline arm")
    expected_binary = expected_arm.get("binary_sha256")
    if (type(actual_exit) is not int or actual_exit != 1
            or actual_exit != error.returncode or record.get("workerStarted") is not True
            or record.get("sample") != 0 or record.get("attempt") != 1
            or record.get("reason") != "worker command exited nonzero"
            or record.get("binarySha256Before") != expected_binary
            or record.get("binarySha256After") != expected_binary
            or not isinstance(lifecycle_value, dict)):
        return False
    for key in ("loadRatioBefore", "loadRatioAfter"):
        value = record.get(key)
        if (isinstance(value, bool) or not isinstance(value, (int, float))
                or not math.isfinite(value) or not 0 <= value <= MAX_WORKER_LOAD):
            return False
    lifecycle_pin = cast(FilePin, lifecycle_value)
    try:
        lifecycle_path = Path(lifecycle_pin["path"])
        if gate.sha256(lifecycle_path) != lifecycle_pin["sha256"]:
            return False
        lifecycle = gate._object(gate.read_json(lifecycle_path), "worker lifecycle")
        identity = gate._object(lifecycle.get("identity"), "worker identity")
        pid = identity.get("pid")
        streams = _lifecycle_worker_streams(lifecycle)
    except (KeyError, OSError, TypeError, ValueError):
        return False
    stdout, stderr = streams
    expected_error = (
        f'Error: RuntimeFailure {{ error: "navigation is not allowed: {fragment}" }}'
    ).encode()
    return (type(pid) is int and pid > 0 and lifecycle.get("role") == "worker"
            and lifecycle.get("groupClosed") is True
            and lifecycle.get("rawFdsClosed") is True
            and lifecycle.get("failure") is None
            and lifecycle.get("cleanupFailure") is None
            and lifecycle.get("exitCode") == actual_exit
            and error.output == stdout and error.stderr == stderr
            and stderr.splitlines() == [expected_error])


def _lifecycle_worker_streams(lifecycle: Mapping[str, object]) -> tuple[bytes, bytes]:
    raw = gate._objects(lifecycle.get("raw"), "worker lifecycle raw files")
    if len(raw) != 2:
        raise ValueError("worker lifecycle raw file count differs")
    streams: dict[str, bytes] = {}
    for item in raw:
        path = Path(gate._text(item.get("path"), "worker lifecycle raw path"))
        checksum = gate._text(item.get("sha256"), "worker lifecycle raw SHA")
        if (not path.is_absolute() or path.resolve() != path or path.is_symlink()
                or not path.is_file() or gate.sha256(path) != checksum):
            raise ValueError("worker lifecycle raw file differs")
        if path.name == "stdout.raw":
            streams["stdout"] = path.read_bytes()
        elif path.name == "stderr.raw":
            streams["stderr"] = path.read_bytes()
    if set(streams) != {"stdout", "stderr"}:
        raise ValueError("worker lifecycle stream names differ")
    return streams["stdout"], streams["stderr"]


def _worker_failure_can_continue(
    arm: str, attempts: Sequence[gate.JsonObject], error: BaseException,
    expected: ExpectedPerformance,
) -> bool:
    return arm == "baseline" and _closed_nonzero_baseline_failure(attempts, error, expected)


def _baseline_slot_is_missing(
    arm: str, baseline_failure: gate.JsonObject | None,
) -> bool:
    return arm == "baseline" and baseline_failure is not None


def _append_worker_attempt_pins(
    destination: list[FilePin], attempts: Sequence[gate.JsonObject],
) -> None:
    destination.extend(cast(FilePin, gate._object(attempt["attemptRecord"], "worker attempt record"))
                      for attempt in attempts)


def _worker_attempt_pins_in(folder: Path) -> list[FilePin]:
    return [{"path": str(path.resolve()), "sha256": gate.sha256(path)}
            for path in sorted(folder.rglob("attempt-*.json"))
            if path.is_file() and not path.is_symlink()]


def observe_worker_attempt(
    build: ArmBuildResult,
    sample: int,
    attempt: int,
    fixture: Path,
    report_path: Path,
    plan_path: Path,
    width: int,
    height: int,
    scale: int,
    cwd: Path,
    env: Mapping[str, str],
    log_directory: Path,
    *,
    batch_deadline: float | None = None,
) -> WorkerAttemptObservation:
    load_before = await_quiet(batch_deadline)
    gate.require(math.isfinite(load_before) and 0 <= load_before <= MAX_WORKER_LOAD,
                 "worker pre-load did not reach quiet limit")
    binary_before = binary_sha_or_none(build.binary)
    if binary_before != build.binary_sha256:
        return WorkerAttemptObservation({
            "sample": sample, "attempt": attempt, "loadRatioBefore": load_before,
            "loadRatioAfter": None, "binarySha256Before": binary_before,
            "binarySha256After": None, "actualExit": None,
            "workerStarted": False, "accepted": False,
            "reason": "worker binary changed before measurement",
        }, None, None)
    _require_batch_budget(batch_deadline)
    argv = [str(build.binary), "--worker", str(sample), str(fixture), str(report_path),
            str(plan_path), str(width), str(height), str(scale)]
    lifecycle_paths: list[Path] = []
    result: subprocess.CompletedProcess[bytes] | None = None
    error: BaseException | None = None
    try:
        result = _measurement_command(argv, cwd=cwd, env=env, role="worker",
            log_directory=log_directory, check=False, lifecycle_sink=lifecycle_paths.append)
    except BaseException as caught:
        error = caught
    load_after = quiet()
    binary_after = binary_sha_or_none(build.binary)
    lifecycle: FilePin | None = None
    if len(lifecycle_paths) == 1:
        lifecycle_path = lifecycle_paths[0]
        lifecycle = {"path": str(lifecycle_path.resolve()), "sha256": gate.sha256(lifecycle_path)}
    actual_exit = result.returncode if result is not None else None
    worker_started: bool | None = True if result is not None else None
    if result is None and len(lifecycle_paths) == 1:
        worker_started = worker_started_from_lifecycle(lifecycle_paths[0])
    record: gate.JsonObject = {
        "sample": sample, "attempt": attempt, "loadRatioBefore": load_before,
        "loadRatioAfter": load_after if math.isfinite(load_after) else None,
        "binarySha256Before": binary_before, "binarySha256After": binary_after,
        "actualExit": actual_exit, "lifecycle": lifecycle,
        "workerStarted": worker_started, "accepted": False,
    }
    if error is not None:
        record["reason"] = "worker command raised"
        record["errorType"] = type(error).__name__
    elif result is not None and result.returncode != 0:
        record["reason"] = "worker command exited nonzero"
    elif binary_after != build.binary_sha256:
        record["reason"] = "worker binary changed during measurement"
    elif not math.isfinite(load_after) or not 0 <= load_after <= MAX_WORKER_LOAD:
        record["reason"] = "host load exceeded limit"
    return WorkerAttemptObservation(record, result, error)


def collect_worker_sample(
    build: ArmBuildResult,
    sample: int,
    fixture: Path,
    report_path: Path,
    plan_path: Path,
    width: int,
    height: int,
    scale: int,
    cwd: Path,
    env: Mapping[str, str],
    log_directory: Path,
    attempts_directory: Path,
    failure_sink: Callable[[list[gate.JsonObject]], None] | None = None,
    *,
    batch_deadline: float | None = None,
) -> tuple[gate.JsonObject, dict[str, float], list[gate.JsonObject]]:
    attempts: list[gate.JsonObject] = []

    def preserve_failure() -> None:
        if failure_sink is not None:
            failure_sink(attempts)

    for attempt in range(1, MAX_WORKER_ATTEMPTS + 1):
        _require_batch_budget(batch_deadline)
        observation = observe_worker_attempt(build, sample, attempt, fixture,
            report_path, plan_path,
            width, height, scale, cwd, env, log_directory,
            batch_deadline=batch_deadline)
        record = observation.record
        result = observation.result
        if observation.error is None and record.get("reason") is None and result is not None:
            try:
                payload = worker_sample(result.stdout, sample, {"width": width, "height": height,
                                                                 "scale": scale})
            except BaseException as error:
                record["reason"] = "worker sample invalid"
                record["errorType"] = type(error).__name__
                pin = write_exclusive(attempts_directory / f"attempt-{sample}-{attempt}.json", record)
                record["attemptRecord"] = pin
                attempts.append(record)
                preserve_failure()
                raise
            record["accepted"] = True
            pin = write_exclusive(attempts_directory / f"attempt-{sample}-{attempt}.json", record)
            record["attemptRecord"] = pin
            attempts.append(record)
            return payload, {"before": cast(float, record["loadRatioBefore"]),
                             "after": cast(float, record["loadRatioAfter"])}, attempts
        pin = write_exclusive(attempts_directory / f"attempt-{sample}-{attempt}.json", record)
        record["attemptRecord"] = pin
        attempts.append(record)
        if observation.error is not None:
            preserve_failure()
            raise observation.error
        if result is not None and result.returncode != 0:
            preserve_failure()
            raise subprocess.CalledProcessError(result.returncode, result.args,
                                                output=result.stdout, stderr=result.stderr)
        if record.get("reason") == "worker binary changed during measurement":
            preserve_failure()
            raise ValueError("worker binary changed during measurement")
        if record.get("reason") == "worker binary changed before measurement":
            preserve_failure()
            raise ValueError("frozen build binary changed before measurement")
        if record.get("reason") != "host load exceeded limit":
            preserve_failure()
            raise ValueError("worker sample invalid")
        if attempt == MAX_WORKER_ATTEMPTS:
            preserve_failure()
            raise ValueError("host load remained above limit after three attempts")
    preserve_failure()
    raise ValueError("worker attempt loop ended without a sample")


def write_condition_checkpoint(
    folder: Path,
    expected: Mapping[str, object],
    result: Mapping[str, object],
    manifest: Mapping[str, object],
) -> FilePin:
    return write_exclusive(folder / "condition-checkpoint.json", {
        "schema": "krr-html-performance-condition-checkpoint-v1",
        "formalAccepted": False, "endpointValidated": False,
        "expected": dict(expected), "result": dict(result), "manifest": dict(manifest),
    })


def validate_interaction(value: object, fixture_sha256: str) -> None:
    state = gate._object(value, "interaction evidence")
    interaction_states = gate._object(state["states"], "interaction states")
    js_state = gate._object(interaction_states["js"], "interaction JavaScript state")
    gate.require(state["inputSha256"] == fixture_sha256 and js_state == {"expanded": True, "result": "opened"}
                 and js_state["expanded"] is True, "real same-source interaction evidence absent")


def _catalog_operation_stream(
    operation_kind: str,
    evidence: gate.JsonObject,
    expected_state: object | None = None,
    report_state: object | None = None,
) -> list[gate.JsonObject]:
    return gate.catalog_operation_stream(operation_kind, evidence, expected_state, report_state)


def _validate_catalog_capture_envelope(stdout: bytes, manifest_pin: FilePin,
                                       plan_pin: FilePin,
                                       conditions: Sequence[CatalogCondition]) -> gate.JsonObject:
    envelope = gate._object(json.loads(stdout, object_pairs_hook=gate.unique_object),
                            "strict Chrome capture validation output")
    gate.require(envelope.get("schema") == "krr-html84-independent-stage90-validation-v1"
                 and envelope.get("threshold") == 90
                 and envelope.get("measurement") == "not-performed"
                 and envelope.get("manifestPath") == manifest_pin["path"]
                 and envelope.get("manifestSha256") == manifest_pin["sha256"]
                 and envelope.get("planPath") == plan_pin["path"]
                 and envelope.get("planSha256") == plan_pin["sha256"],
                 "strict Chrome capture validation manifest/plan binding differs")
    rows = gate._objects(envelope.get("conditions"), "validated Chrome conditions")
    gate.require(len(rows) == len(conditions) == 84,
                 "strict Chrome capture validation must cover exactly 84 conditions")
    by_id = {gate._text(row.get("conditionId"), "validated condition id"): row for row in rows}
    gate.require(len(by_id) == len(rows), "strict Chrome validation condition ids are duplicated")
    probe = gate._object(envelope.get("fontIdentityProbe"), "validated font identity probe pins")
    gate._text(probe.get("inventoryMethod"), "font inventory method")
    gate._text(probe.get("inventoryLimit"), "font inventory limit")
    first = conditions[0]
    gate.require(probe.get("request") == first["font_identity_request"]
                 and probe.get("response") == first["font_identity_response"]
                 and probe.get("binary") == {"path": first["font_identity_binary_path"],
                                              "sha256": first["font_identity_binary_sha256"]},
                 "validated font identity probe pins differ from catalog manifest")
    for condition in conditions:
        row = by_id.get(condition["condition_id"])
        if row is None:
            raise ValueError(f"strict Chrome validation is missing {condition['condition_id']}")
        gate.require(gate._numeric(row.get("score"), "independent image score") >= 90,
                     f"independent raw stage90 score is below 90: {condition['condition_id']}")
        gate._numeric(row.get("rmseScore"), "independent RMSE score")
        gate._numeric(row.get("maeScore"), "independent MAE score")
        for output_key, pin in (("capture", condition["chrome_capture"]),
                                ("receipt", condition["chrome_receipt"])):
            gate.require(row.get(output_key + "Path") == pin["path"]
                         and row.get(output_key + "Sha256") == pin["sha256"]
                         and gate._integer(row.get(output_key + "Size"), output_key + " size") == pin["size"],
                         f"strict Chrome {output_key} transport pin differs: {condition['condition_id']}")
            path = Path(pin["path"])
            _pinned_file(path, pin["sha256"])
            gate.require(path.stat().st_size == pin["size"],
                         f"strict Chrome {output_key} size changed: {condition['condition_id']}")
    return envelope


def load_catalog_conditions(
    root: Path, manifest_path: Path,
) -> tuple[list[CatalogCondition], FilePin, FilePin, gate.CatalogBinaryRefs]:
    _pinned_file(manifest_path, gate.sha256(manifest_path))
    manifest = gate.read_json(manifest_path)
    gate.require(manifest.get("profile") == "catalog-stage90-captures"
                 and manifest.get("acceptance") is False,
                 "catalog performance input must be the non-accepted stage90 capture manifest")

    def read_catalog_file(pin: gate.FilePin) -> Path:
        path = Path(gate._text(pin.get("path"), "catalog binary proof file path"))
        checksum = gate._text(pin.get("sha256"), "catalog binary proof file SHA")
        _pinned_file(path, checksum)
        return path

    binary_refs = gate.validate_catalog_binary_proof(manifest, read_catalog_file)
    plan_path = Path(gate._text(manifest.get("planPath"), "catalog plan path"))
    plan_sha = gate._text(manifest.get("planSha256"), "catalog plan SHA")
    _pinned_file(plan_path, plan_sha)
    plan = gate.read_json(plan_path)
    gate.require(plan.get("profile") == "krr-html84-condition-plan-v1"
                 and plan.get("acceptance") is False,
                 "catalog stage90 condition plan profile differs")
    catalog_path = Path(gate._text(manifest.get("catalogPath"), "feature catalog path"))
    catalog_sha = gate._text(manifest.get("catalogSha256"), "feature catalog SHA")
    _pinned_file(catalog_path, catalog_sha)
    gate.require(plan.get("catalogPath") == str(catalog_path)
                 and plan.get("catalogSha256") == catalog_sha,
                 "catalog plan is bound to a different feature catalog")
    expected_conditions = gate._objects(plan.get("conditions"), "catalog plan condition")
    captured_conditions = gate._objects(manifest.get("conditions"), "catalog capture condition")
    gate.require(len(expected_conditions) == 84 and len(captured_conditions) == 84,
                 "catalog plan and capture manifest must contain exactly 84 conditions")
    by_id = {gate._text(item.get("conditionId"), "capture condition id"): item
             for item in captured_conditions}
    gate.require(len(by_id) == len(captured_conditions), "catalog capture condition ids are duplicated")
    binary_sha = gate._text(manifest.get("binarySha256"), "catalog capture binary SHA")
    gate.digest(binary_sha)
    font_probe = gate._object(manifest.get("fontIdentityProbe"), "font identity probe pins")
    font_request = _catalog_probe_file(font_probe.get("request"), "font identity request")
    font_response = _catalog_probe_file(font_probe.get("response"), "font identity response")
    font_binary = gate._object(font_probe.get("binary"), "font identity probe binary pin")
    font_binary_path = Path(gate._text(font_binary.get("path"), "font probe binary path"))
    font_binary_sha = gate._text(font_binary.get("sha256"), "font probe binary SHA")
    _pinned_file(font_binary_path, font_binary_sha)
    conditions: list[CatalogCondition] = []
    evidence_paths: set[Path] = set()
    evidence_shas: set[str] = set()
    for expected in expected_conditions:
        condition_id = gate._text(expected.get("conditionId"), "condition id")
        captured = by_id.get(condition_id)
        if captured is None:
            raise ValueError(f"catalog capture is missing {condition_id}")
        fixture = Path(gate._text(expected.get("fixturePath"), "condition fixture path"))
        source_sha = gate._text(expected.get("sourceSha256"), "condition source SHA")
        _pinned_file(fixture, source_sha)
        viewport = gate._object(expected.get("viewport"), "catalog viewport")
        viewport_value: Viewport = {
            "width": gate._integer(viewport.get("width"), "viewport width"),
            "height": gate._integer(viewport.get("height"), "viewport height"),
            "scale": gate._integer(viewport.get("scale"), "viewport scale"),
        }
        expected_id = (f"{expected.get('caseId')}/{expected.get('state')}/"
                       f"{viewport_value['width']}x{viewport_value['height']}@{viewport_value['scale']}")
        gate.require(condition_id == expected_id, f"catalog condition id does not bind its tuple: {condition_id}")
        for field in ("caseId", "state", "operationKind", "targetSelector", "sourceSha256"):
            gate.require(captured.get(field) == expected.get(field),
                         f"catalog capture {field} differs for {condition_id}")
        gate.require(captured.get("binarySha256") == binary_sha,
                     f"catalog capture binary differs: {condition_id}")
        gate.require(captured.get("viewport") == viewport_value,
                     f"catalog capture viewport differs for {condition_id}")
        raw_path = Path(gate._text(captured.get("krrReportPath"), "raw KRR report path"))
        raw_sha = gate._text(captured.get("krrReportSha256"), "raw KRR report SHA")
        _pinned_file(raw_path, raw_sha)
        gate.require(raw_path not in evidence_paths and raw_sha not in evidence_shas,
                     "catalog conditions reuse a raw KRR evidence file")
        evidence_paths.add(raw_path)
        evidence_shas.add(raw_sha)
        raw_report = gate.read_json(raw_path)
        capture_path = Path(gate._text(raw_report.get("capturePath"), "raw KRR capture path"))
        capture_sha = gate._text(raw_report.get("captureSha256"), "raw KRR capture SHA")
        _pinned_file(capture_path, capture_sha)
        gate.require(captured.get("krrCapturePath") == str(capture_path)
                     and captured.get("krrCaptureSha256") == capture_sha
                     and captured.get("krrState") == raw_report.get("state"),
                     f"catalog manifest capture binding differs: {condition_id}")
        gate.require(gate._same_json(captured.get("chromeState"), raw_report.get("state")),
                     f"independent Chrome/KRR semantic state differs: {condition_id}")
        gate.require(captured.get("status") == "captured",
                     f"catalog raw capture is not finalized: {condition_id}")
        chrome_path = Path(gate._text(captured.get("chromeCapturePath"), "Chrome capture path"))
        chrome_sha = gate._text(captured.get("chromeCaptureSha256"), "Chrome capture SHA")
        chrome_receipt_path = Path(gate._text(captured.get("chromeReceiptPath"), "Chrome receipt path"))
        chrome_receipt_sha = gate._text(captured.get("chromeReceiptSha256"), "Chrome receipt SHA")
        _pinned_file(chrome_path, chrome_sha)
        _pinned_file(chrome_receipt_path, chrome_receipt_sha)
        chrome_capture_pin: SizedFilePin = {
            "path": str(chrome_path), "sha256": chrome_sha, "size": chrome_path.stat().st_size,
        }
        chrome_receipt_pin: SizedFilePin = {
            "path": str(chrome_receipt_path), "sha256": chrome_receipt_sha,
            "size": chrome_receipt_path.stat().st_size,
        }
        operation_kind = gate._text(expected.get("operationKind"), "operation kind")
        gate.require(raw_report.get("status") == "observed",
                     f"catalog raw report is not observed: {condition_id}")
        gate.require("unknownReason" not in raw_report,
                     f"observed catalog report carries an unknown reason: {condition_id}")
        raw_viewport = gate._object(raw_report.get("viewport"), "raw report viewport")
        gate.require(raw_report.get("conditionId") == condition_id
                     and raw_report.get("caseId") == expected.get("caseId")
                     and raw_report.get("boundaryState") == expected.get("state")
                     and raw_report.get("operationKind") == operation_kind
                     and raw_report.get("htmlSha256") == source_sha
                     and raw_report.get("binarySha256") == binary_sha
                     and raw_viewport == viewport_value,
                     f"catalog raw report identity differs: {condition_id}")
        evidence = gate._object(raw_report.get("operationEvidence"), "typed operation evidence")
        gate.require(captured.get("operationEvidence") == evidence,
                     f"catalog manifest operation evidence differs: {condition_id}")
        gate.require(evidence.get("schema") == "krr-html84-operation-evidence-v1"
                     and evidence.get("kind") == operation_kind
                     and evidence.get("sourceSha256Before") == source_sha
                     and evidence.get("sourceSha256After") == source_sha,
                     f"catalog operation evidence identity differs: {condition_id}")
        for frame_name in ("preFrame", "postFrame"):
            frame = gate._object(evidence.get(frame_name), "catalog operation frame")
            gate.require(gate._integer(frame.get("generation"), "catalog frame generation") > 0,
                         f"catalog frame generation is missing: {condition_id}")
            scroll = gate._numeric(frame.get("scrollY"), "catalog frame scroll")
            gate.require(math.isfinite(scroll) and scroll >= 0,
                         f"catalog frame scroll is invalid: {condition_id}")
        state = gate._object(evidence.get("state"), "catalog semantic state evidence")
        gate.require("before" in state and "after" in state,
                     f"catalog state transition evidence is missing: {condition_id}")
        frame = gate._object(raw_report.get("frame"), "catalog captured frame")
        gate.require(gate._integer(frame.get("generation"), "captured frame generation") > 0,
                     f"catalog captured frame generation is missing: {condition_id}")
        content_height = gate._numeric(frame.get("contentHeight"), "captured content height")
        scroll_y = gate._numeric(frame.get("scrollY"), "captured scroll")
        gate.require(math.isfinite(content_height) and content_height > 0
                     and math.isfinite(scroll_y) and scroll_y >= 0
                     and raw_report.get("targetVisible") is True,
                     f"catalog frame or target visibility is unverified: {condition_id}")
        target = gate._object(evidence.get("target"), "catalog operation target")
        selector_value = expected.get("targetSelector")
        selector = None if selector_value is None else gate._text(selector_value, "target selector")
        gate.require(target.get("visible") is True and gate._integer(target.get("nodeId"), "target node id") > 0
                     and (selector is None or target.get("selector") == selector),
                     f"catalog target evidence differs: {condition_id}")
        link_captures: list[FilePin] = []
        if operation_kind == "navigation-all-pointer-clicks":
            for link in gate._objects(evidence.get("links"), "catalog TOC link"):
                link_capture_path = Path(gate._text(link.get("capturePath"), "TOC link capture path"))
                link_capture_sha = gate._text(link.get("captureSha256"), "TOC link capture SHA")
                _pinned_file(link_capture_path, link_capture_sha)
                link_captures.append({"path": str(link_capture_path), "sha256": link_capture_sha})
        expected_operation_state = (
            gate._fixture_expected_state(fixture)
            if operation_kind in {
                "script-await-timer-microtask",
                "script-await-mutation-delivery",
                "script-real-events",
                "script-await-resource-font",
            }
            else None
        )
        stream = _catalog_operation_stream(
            operation_kind, evidence, expected_operation_state, raw_report.get("state")
        )
        operation_plan: CatalogOperationPlan = {
            "profile": "catalog-plan84", "conditionId": condition_id,
            "caseId": gate._text(expected.get("caseId"), "case id"),
            "state": gate._text(expected.get("state"), "boundary state"),
            "operationKind": operation_kind, "targetSelector": selector,
            "fixtureSha256": source_sha, "viewport": viewport_value,
            "operationEvidencePath": str(raw_path), "operationEvidenceSha256": raw_sha,
            "evidenceBinarySha256": binary_sha, "operationStream": stream,
        }
        conditions.append({
            "condition_id": condition_id,
            "case_id": operation_plan["caseId"],
            "state": operation_plan["state"],
            "operation_kind": operation_kind,
            "target_selector": selector,
            "fixture": fixture,
            "fixture_sha256": source_sha,
            "viewport": viewport_value,
            "operation_plan": operation_plan,
            "operation_evidence": {"path": str(raw_path), "sha256": raw_sha},
            "capture": {"path": str(capture_path), "sha256": capture_sha},
            "chrome_capture": chrome_capture_pin,
            "chrome_receipt": chrome_receipt_pin,
            "font_identity_request": font_request,
            "font_identity_response": font_response,
            "font_identity_binary_path": str(font_binary_path),
            "font_identity_binary_sha256": font_binary_sha,
            "link_captures": link_captures,
        })
    return (conditions,
            {"path": str(manifest_path), "sha256": gate.sha256(manifest_path)},
            {"path": str(plan_path), "sha256": plan_sha},
            binary_refs)


def _build_worker_report(
    expected: Mapping[str, object], row: Mapping[str, object], arms: Mapping[str, ArmPin],
    raw_arm: str, pair: int, viewport: Viewport,
    samples: Sequence[gate.JsonObject], failure: gate.JsonObject | None,
) -> gate.JsonObject:
    provenance = cast(Provenance, expected["provenance"])
    report: gate.JsonObject = {
        "arm": raw_arm, "pair_index": pair, "measurement_executed": bool(samples),
        "sample_count": len(samples), "source_sha256": expected["fixture_sha256"],
        "operation_plan_sha256": expected["operation_plan_sha256"],
        "operation": expected["operation"],
        "expected_dom_state_sha256": expected["expected_dom_state_sha256"],
        "viewport": viewport,
        "provenance": {**provenance,
            "binary_sha256": arms[raw_arm]["binary_sha256"],
            "source_snapshot_sha256": arms[raw_arm]["source_snapshot_sha256"],
            "deps_lock_sha256": arms[raw_arm]["lock_sha256"]},
        "samples": list(samples),
    }
    if failure is not None:
        report["failure"] = failure
    if row["catalog"]:
        report.update({"condition_id": row["condition_id"], "case_id": row["case_id"],
            "state": row["state"], "operation_kind": row["operation_kind"],
            "operation_evidence_ref": row["operation_evidence"],
            "catalog_condition": expected["catalogCondition"]})
    return report


def _collect_condition_worker_pairs(
    *, folder: Path, row: Mapping[str, object], expected: ExpectedPerformance,
    fixture: Path, plan_path: Path, width: int, height: int, viewport: Viewport,
    builds: Mapping[str, ArmBuildResult], binaries: Mapping[str, Path],
    arms: Mapping[str, ArmPin], root: Path, lifecycle: Path, lock: Path,
    snapshot: FilePin, environment_sha: str, batch_deadline: float | None = None,
) -> ConditionWorkerCollection:
    order: list[gate.JsonObject] = []
    reports: list[Path] = []
    report_pins: list[FilePin] = []
    failures: list[gate.JsonObject] = []
    worker_attempt_pins: list[FilePin] = []
    baseline_failure: gate.JsonObject | None = None
    actual_phase_records = 0
    for pair in range(5):
        _require_batch_budget(batch_deadline)
        for arm in (("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")):
            _require_batch_budget(batch_deadline)
            binary = binaries[arm]
            report_path = folder / "reports" / f"{arm}-{pair}.json"
            raw_arm = gate.baseline_arm_id(expected) if arm == "baseline" else arm
            if _baseline_slot_is_missing(arm, baseline_failure):
                missing_failure: gate.JsonObject = {"status": "missing",
                    "reason": "baseline skipped after known fragment refusal",
                    "afterFailure": baseline_failure}
                failures.append(missing_failure)
                report = _build_worker_report(expected, row, arms, raw_arm, pair, viewport,
                                              [], missing_failure)
                report_pin = write(report_path, report)
                report_pins.append(report_pin)
                reports.append(report_path)
                order.append({"arm": arm, "pair_index": pair, "executable": str(binary),
                    "executable_sha256": arms[raw_arm]["binary_sha256"],
                    "output": str(report_path), "load_ratio_before": None,
                    "load_ratio_after": None, "workerLoads": [], "workerAttempts": [],
                    "status": "missing", "failure": missing_failure})
                continue
            provenance = expected["provenance"]
            environment = _performance_worker_environment(
                provenance["rustc"], provenance["cargo"],
                environment_sha, lock, snapshot)
            samples: list[gate.JsonObject] = []
            loads: list[dict[str, float]] = []
            worker_attempts: list[gate.JsonObject] = []
            failed_attempts: list[gate.JsonObject] = []
            try:
                for sample in range(5):
                    payload, sample_load, sample_attempts = collect_worker_sample(
                        builds[arm], sample, fixture, report_path, plan_path, width, height,
                        viewport["scale"], root, environment, lifecycle,
                        worker_attempts_directory(folder, arm, pair), failed_attempts.extend,
                        batch_deadline=batch_deadline)
                    samples.append(payload)
                    loads.append(sample_load)
                    worker_attempts.extend(sample_attempts)
                    worker_attempt_pins.extend(
                        cast(FilePin, attempt["attemptRecord"]) for attempt in sample_attempts)
            except BaseException as error:
                if not _worker_failure_can_continue(arm, failed_attempts, error, expected):
                    raise
                _append_worker_attempt_pins(worker_attempt_pins, failed_attempts)
                failed_record = failed_attempts[-1]
                condition = gate._object(expected.get("catalogCondition"),
                                         "non-comparable catalog condition")
                condition_id = gate._text(condition.get("condition_id"),
                                          "non-comparable condition id")
                fragment = gate.non_comparable_baseline_fragment(expected)
                gate.require(fragment is not None,
                             "known baseline fragment refusal lost its condition binding")
                worker_attempts.extend(failed_attempts)
                failure_load = {"before": failed_record["loadRatioBefore"],
                                "after": failed_record["loadRatioAfter"]}
                failure: gate.JsonObject = {"status": "nonComparable",
                    "reason": "known baseline fragment navigation refusal",
                    "conditionId": condition_id, "fragment": fragment,
                    "sample": failed_record["sample"], "attempt": failed_record["attempt"],
                    "actualExit": failed_record["actualExit"],
                    "errorType": type(error).__name__}
                failures.append(failure)
                baseline_failure = failure
                actual_phase_records += len(gate.PHASES) * len(samples)
                report = _build_worker_report(expected, row, arms, raw_arm, pair, viewport,
                                              samples, failure)
                report_pin = write(report_path, report)
                report_pins.append(report_pin)
                reports.append(report_path)
                order.append({"arm": arm, "pair_index": pair, "executable": str(binary),
                    "executable_sha256": arms[raw_arm]["binary_sha256"],
                    "output": str(report_path),
                    "load_ratio_before": failed_record["loadRatioBefore"],
                    "load_ratio_after": failed_record["loadRatioAfter"],
                    "workerLoads": loads, "failedWorkerLoads": [failure_load],
                    "workerAttempts": worker_attempts,
                    "status": "nonComparable", "failure": failure})
                continue
            report = _build_worker_report(expected, row, arms, raw_arm, pair, viewport,
                                          samples, None)
            report_pin = write(report_path, report)
            report_pins.append(report_pin)
            reports.append(report_path)
            actual_phase_records += len(gate.PHASES) * len(samples)
            order.append({"arm": arm, "pair_index": pair, "executable": str(binary),
                "executable_sha256": arms[raw_arm]["binary_sha256"],
                "output": str(report_path), "load_ratio_before": max(x["before"] for x in loads),
                "load_ratio_after": max(x["after"] for x in loads),
                "workerLoads": loads, "workerAttempts": worker_attempts})
    return ConditionWorkerCollection(order, reports, report_pins, failures,
                                     worker_attempt_pins, actual_phase_records)


def _condition_measurement_policy(
    reports: list[Path], expected: ExpectedPerformance,
    failures: Sequence[gate.JsonObject], actual_phase_records: int,
) -> gate.JsonObject:
    if failures:
        fragment = gate.non_comparable_baseline_fragment(expected)
        condition = expected.get("catalogCondition")
        condition_id = condition.get("condition_id") if isinstance(condition, dict) else None
        if (fragment is not None and isinstance(condition_id, str)
                and _known_fragment_failure_sequence(failures, condition_id, fragment)):
            candidate_paths = [path for path in reports if path.name.startswith("candidate-")]
            gate.require(len(candidate_paths) == 5,
                         "known non-comparable condition requires five candidate reports")
            candidate_samples = 0
            for path in candidate_paths:
                arm, samples = gate._validate_report(
                    gate.read_json(path), cast(gate.JsonObject, expected),
                )
                gate.require(arm == "candidate" and len(samples) == 5,
                             "known non-comparable candidate samples differ")
                candidate_samples += len(samples)
            gate.require(candidate_samples == 25
                         and actual_phase_records == len(gate.PHASES) * candidate_samples,
                         "known non-comparable candidate requires 25 complete samples")
            return {"status": "nonComparable", "policyPass": False,
                    "reason": "known baseline fragment navigation refusal",
                    "conditionId": condition_id, "fragment": fragment,
                    "candidateOperationalPass": True,
                    "candidateSamples": candidate_samples,
                    "phaseRecords": actual_phase_records}
        return {"policyPass": False, "status": "incomplete",
                "reason": "baseline worker failure", "policy": {},
                "nullDifference": 0, "quietLoadLimit": 0.25,
                "expectedPhaseRecords": len(gate.PHASES) * 50,
                "phaseRecords": actual_phase_records, "aggregate": None}
    return gate.evaluate_measurement_policy(reports, {**expected})


def _known_fragment_failure_sequence(
    failures: Sequence[gate.JsonObject], condition_id: str, fragment: str,
) -> bool:
    if len(failures) != 5:
        return False
    first = failures[0]
    if (first.get("status") != "nonComparable"
            or first.get("reason") != "known baseline fragment navigation refusal"
            or first.get("conditionId") != condition_id
            or first.get("fragment") != fragment
            or first.get("actualExit") != 1 or first.get("sample") != 0
            or first.get("attempt") != 1
            or first.get("errorType") != "CalledProcessError"):
        return False
    return all(
        item.get("status") == "missing"
        and item.get("reason") == "baseline skipped after known fragment refusal"
        and item.get("afterFailure") == first
        for item in failures[1:]
    )


def produce(root: Path, output: Path, capacity_document: Path | None = None,
            capacity_document_sha256: str | None = None,
            catalog_manifest_path: Path | None = None, *,
            _allow_legacy_diagnostic: bool = False,
            _allow_catalog_diagnostic: bool = False,
            max_conditions: int | None = None,
            resume_batch: Path | None = None,
            reference_parent_wrapper: Path | None = None,
            reference_parent_wrapper_sha256: str | None = None) -> None:
    if catalog_manifest_path is None and not _allow_legacy_diagnostic:
        raise ValueError("catalog capture manifest required for formal performance production")
    gate.require(platform.system() == "Darwin" and platform.machine() == "arm64", "measurement lane requires macOS Apple Silicon")
    gate.require(not output.exists() and output.is_relative_to(root / "tmp"), "fresh ignored owned output required")
    if max_conditions is not None:
        gate.require(catalog_manifest_path is not None and 0 < max_conditions <= 84,
                     "batch max conditions requires a catalog and is between 1 and 84")
    gate.require(resume_batch is None or catalog_manifest_path is not None,
                 "batch resume requires a catalog")
    gate.require((reference_parent_wrapper is None) == (reference_parent_wrapper_sha256 is None),
                 "reference parent wrapper requires its SHA256")
    gate.require(reference_parent_wrapper is None or catalog_manifest_path is not None,
                 "reference parent wrapper requires a catalog")
    batch_mode = _batch_mode(catalog_manifest_path, max_conditions, resume_batch)
    batch_deadline = _batch_deadline(time.monotonic(), batch_mode)
    batch_failure: BaseException | None = None
    resumed_receipt: gate.JsonObject | None = None
    resumed_bundle: gate.Bundle | None = None
    resumed_attempts: list[dict[str, FilePin]] = []
    if resume_batch is not None:
        resumed_receipt, resumed_bundle, resumed_attempts = load_closed_batch(
            resume_batch.resolve(), root,
        )
    catalog_conditions: list[CatalogCondition] | None = None
    catalog_manifest_pin: FilePin | None = None
    catalog_plan_pin: FilePin | None = None
    catalog_binary_refs: gate.CatalogBinaryRefs | None = None
    if catalog_manifest_path is not None:
        (catalog_conditions, catalog_manifest_pin, catalog_plan_pin,
         catalog_binary_refs) = load_catalog_conditions(root, catalog_manifest_path)
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    # 生成済みJSとsourceの同期が未確認のcandidateをビルド・計測しない。
    lifecycle = output.parent / f"{output.name}.lifecycle"
    gate.require(not lifecycle.exists(), "fresh measurement lifecycle output required")
    git_query = _make_git_query(lifecycle)
    initial = source(root, git_query=git_query)
    reference_proof: reference.ReferenceParentProof | None = None
    reference_contract: gate.JsonObject | None = None
    reference_wrapper_pin: FilePin | None = None
    reference_transport_refs: list[FilePin] = []
    if reference_parent_wrapper is not None and reference_parent_wrapper_sha256 is not None:
        (reference_proof, reference_contract, reference_wrapper_pin,
         reference_transport_refs) = load_reference_parent(
            root, reference_parent_wrapper, reference_parent_wrapper_sha256,
            git_query=git_query,
        )
    catalog_capture_validation: gate.JsonObject | None = None
    if catalog_conditions is not None:
        if (catalog_manifest_pin is None or catalog_plan_pin is None
                or catalog_binary_refs is None):
            raise ValueError("catalog capture validation pins are missing")
        validation = _measurement_command(
            ["bun", "run", "scripts/html/fidelity-acceptance-cli.ts",
             "catalog-stage90-validate-captures", catalog_manifest_pin["path"],
             catalog_plan_pin["path"]],
            cwd=root, role="preflight", log_directory=lifecycle,
        )
        catalog_capture_validation = _validate_catalog_capture_envelope(
            validation.stdout, catalog_manifest_pin, catalog_plan_pin, catalog_conditions,
        )
        gate._require_html84_acceptance(
            catalog_capture_validation, diagnostic=_allow_catalog_diagnostic,
        )
        gate.require(source(root, git_query=git_query) == initial,
                     "source changed during strict Chrome capture validation")
    _measurement_command(["just", "runtime-bundle-check"], cwd=root, role="preflight", log_directory=lifecycle)
    gate.require(source(root, git_query=git_query) == initial, "source changed during runtime bundle check")
    if capacity_document is None or capacity_document_sha256 is None:
        raise ValueError("capacity binding input required before builds")
    binding = load_capacity_binding(root, capacity_document, capacity_document_sha256)
    output.mkdir(parents=True)
    catalog_capture_validation_pin: FilePin | None = None
    if catalog_capture_validation is not None:
        catalog_capture_validation_pin = write(
            output / "catalog-chrome-capture-validation.json", catalog_capture_validation,
        )
        if resumed_receipt is not None and resumed_bundle is not None:
            catalog_capture_validation_pin = _resume_context_pin(
                resumed_bundle, resumed_receipt, catalog_capture_validation_pin,
                _resume_catalog_validation_pin(resumed_bundle, resumed_receipt),
            )
    harness = root / gate.HARNESS_ROOT
    lock = harness / "Cargo.lock"
    gate.require(gate.sha256(lock) == gate.LOCK_SHA, "common lock pin differs")
    fixture: Path | None = None
    dom: Path | None = None
    plan: OperationPlan | None = None
    plan_path: Path | None = None
    plan_pin: FilePin | None = None
    if catalog_conditions is None:
        fixture = root / "tests/fixtures/html/interaction-fidelity.html"
        dom = root / "tmp/html-interaction-suite/krr.json"
        state = gate.read_json(dom)
        validate_interaction(state, gate.sha256(fixture))
        plan = {"fixture_sha256": gate.sha256(fixture), "operation": "action-button-toggle", "x": 100.0, "y": 100.0, "expected_dom_state_path": str(dom), "expected_dom_state_sha256": gate.sha256(dom)}
        plan_path = output / "operation-plan.json"
        plan_pin = write(plan_path, plan)
    extra_capacity_inputs: dict[str, FilePin] = {}
    if catalog_manifest_pin is not None:
        extra_capacity_inputs["catalogManifestRef"] = catalog_manifest_pin
    if catalog_plan_pin is not None:
        extra_capacity_inputs["catalogPlanRef"] = catalog_plan_pin
    build_inputs = write_build_source_inputs(root, output, initial, binding,
        git_query=git_query, extra_capacity_inputs=extra_capacity_inputs)
    if resumed_receipt is not None and resumed_bundle is not None:
        build_inputs = build_inputs._replace(snapshot=_resume_context_pin(
            resumed_bundle, resumed_receipt, build_inputs.snapshot,
            _resume_candidate_snapshot_pin(resumed_bundle, resumed_receipt),
        ))
    approved = write(output / "approved-source.json", initial)
    head = gate._text(initial["head"], "source HEAD")
    inventory_digest = gate._text(initial["inventoryDigest"], "source inventory digest")
    target = root / "target"
    source_records = build_inputs.source_records
    if catalog_manifest_path is not None:
        manifest_value = gate.read_json(catalog_manifest_path)
        feature_catalog_path = Path(gate._text(manifest_value.get("catalogPath"), "feature catalog path"))
        feature_catalog_sha = gate._text(manifest_value.get("catalogSha256"), "feature catalog SHA")
        feature_relative = feature_catalog_path.relative_to(root).as_posix()
        gate.require(source_records.get(feature_relative, (None,))[0] == feature_catalog_sha,
                     "catalog feature source differs from current committed HEAD")
        for condition in catalog_conditions or []:
            fixture_relative = condition["fixture"].relative_to(root).as_posix()
            gate.require(source_records.get(fixture_relative, (None,))[0] == condition["fixture_sha256"],
                         f"catalog fixture differs from current committed HEAD: {fixture_relative}")
    capacity_configs: list[Path] = []
    binaries: dict[str, Path] = {}
    builds: dict[str, ArmBuildResult] = {}
    build_artifact_refs: dict[str, FilePin] = {}
    build_pins: list[FilePin] = []
    for arm in ("baseline", "candidate"):
        if arm == "baseline" and reference_proof is not None and reference_wrapper_pin is not None:
            build = ArmBuildResult([], reference_proof.frozen_binary_path,
                                   reference_proof.binary_sha256, {}, [])
        else:
            build = guarded_arm_build(root, harness, arm, binding, source_records,
                                      output / "builds" / arm)
        capacity_configs = build.configs
        if arm == "baseline" and reference_wrapper_pin is not None:
            build_artifact_pin = reference_wrapper_pin
            build_pins.append(reference_wrapper_pin)
        else:
            build.proof.update({"sourceBeforeRef": build_inputs.source_before,
                                "sourceSnapshotRef": build_inputs.snapshot,
                                "capacityInputRef": build_inputs.capacity_input})
            build_artifact_pin = write(output / "builds" / arm / "build-artifact.json", build.proof)
        build_artifact_refs[arm] = build_artifact_pin
        if arm != "baseline" or reference_wrapper_pin is None:
            build_pins.extend([*build.pins, build_artifact_pin])
        binaries[arm] = build.binary
        builds[arm] = build
        gate.require(source(root, git_query=git_query) == initial, "source changed during build")
    resumed_ids: list[str] = []
    if resumed_receipt is not None and resumed_bundle is not None:
        resumed_ids = [
            gate._text(value, "resumed condition id")
            for value in gate._array(resumed_receipt.get("completeConditionIds"),
                                     "resumed complete condition ids")
        ]
    resumed_id_set = set(resumed_ids)
    measurement_rows: list[gate.JsonObject] = []
    if catalog_conditions is None:
        assert fixture is not None and plan_path is not None and dom is not None and plan is not None and plan_pin is not None
        for width, height in gate.VIEWPORTS:
            measurement_rows.append({"fixture": fixture, "fixture_sha256": gate.sha256(fixture),
                "plan_path": plan_path, "plan": plan, "plan_pin": plan_pin,
                "dom": dom, "dom_sha256": gate.sha256(dom),
                "viewport": {"width": width, "height": height, "scale": 1},
                "folder_name": f"{width}x{height}", "catalog": False})
    else:
        for condition in catalog_conditions:
            label = re.sub(r"[^a-zA-Z0-9.-]+", "_", condition["condition_id"])
            condition_dir = output / "conditions" / label
            condition_dir.mkdir(parents=True)
            condition_plan_path = condition_dir / "operation-plan.json"
            condition_plan_pin = write(condition_plan_path, condition["operation_plan"])
            measurement_rows.append({"fixture": condition["fixture"],
                "fixture_sha256": condition["fixture_sha256"],
                "plan_path": condition_plan_path, "plan": condition["operation_plan"],
                "plan_pin": condition_plan_pin, "dom": None, "dom_sha256": "",
                "viewport": condition["viewport"], "folder_name": label, "catalog": True,
                "condition_id": condition["condition_id"], "case_id": condition["case_id"],
                "state": condition["state"], "operation_kind": condition["operation_kind"],
                "target_selector": condition["target_selector"],
                "operation_evidence": condition["operation_evidence"],
                "capture": condition["capture"], "link_captures": condition["link_captures"],
                "chrome_capture": condition["chrome_capture"],
                "chrome_receipt": condition["chrome_receipt"],
                "font_identity_request": condition["font_identity_request"],
                "font_identity_response": condition["font_identity_response"],
                "font_identity_binary_path": condition["font_identity_binary_path"],
                "font_identity_binary_sha256": condition["font_identity_binary_sha256"]})
    if resume_batch is not None and resumed_bundle is not None:
        for row in measurement_rows:
            condition_id = _row_condition_id(row)
            old_path, old_plan, old_pin = _resume_condition_plan(
                resume_batch.resolve(), resumed_bundle, condition_id,
            )
            row["plan_path"], row["plan"], row["plan_pin"] = old_path, old_plan, old_pin
    all_measurement_rows = measurement_rows
    all_condition_ids = ([_row_condition_id(row) for row in all_measurement_rows]
                         if catalog_conditions is not None else [])
    gate.require(len(all_condition_ids) == len(set(all_condition_ids)),
                 "measurement condition ids are duplicated")
    if resumed_receipt is not None:
        gate.require(resumed_id_set <= set(all_condition_ids),
                     "resume batch contains an unknown condition")
        measurement_rows = [row for row in all_measurement_rows
                            if _row_condition_id(row) not in resumed_id_set]
    if max_conditions is not None:
        measurement_rows = measurement_rows[:max_conditions]
    font_probe_input_paths: list[Path] = []
    if catalog_conditions:
        first_catalog = catalog_conditions[0]
        font_probe_input_paths = [
            Path(first_catalog["font_identity_request"]["path"]),
            Path(first_catalog["font_identity_response"]["path"]),
            Path(first_catalog["font_identity_binary_path"]),
        ]
    provenance_input_paths = [
        Path(cast(str, ref["path"]))
        for row in all_measurement_rows if row["catalog"]
        for ref in gate.provenance_file_refs(
            gate.read_json(Path(cast(FilePin, row["operation_evidence"])["path"]))
        )
    ]
    input_paths = [*[cast(Path, row["fixture"]) for row in all_measurement_rows],
                   *[cast(Path, row["plan_path"]) for row in all_measurement_rows],
                   *[cast(Path, row["dom"]) for row in all_measurement_rows if row["dom"] is not None],
                   *[Path(cast(FilePin, row["operation_evidence"])["path"]) for row in all_measurement_rows if row["catalog"]],
                   *[Path(cast(FilePin, row["capture"])["path"]) for row in all_measurement_rows if row["catalog"]],
                   *[Path(pin["path"]) for row in all_measurement_rows if row["catalog"]
                     for pin in cast(list[FilePin], row["link_captures"])],
                   *[Path(cast(SizedFilePin, row["chrome_capture"])['path']) for row in all_measurement_rows if row["catalog"]],
                   *[Path(cast(SizedFilePin, row["chrome_receipt"])['path']) for row in all_measurement_rows if row["catalog"]],
                   *font_probe_input_paths,
                   *provenance_input_paths,
                   *([catalog_manifest_path] if catalog_manifest_path is not None else []),
                   *([Path(catalog_plan_pin["path"])] if catalog_plan_pin is not None else []),
                   *([Path(catalog_capture_validation_pin["path"])] if catalog_capture_validation_pin is not None else []),
                   *([Path(pin["path"]) for pin in (
                       catalog_binary_refs.proof_ref,
                       catalog_binary_refs.build_report_ref,
                       catalog_binary_refs.binary_ref,
                       catalog_binary_refs.current_build_report_ref,
                   )] if catalog_binary_refs is not None else []),
                   lock, output / "shared-source-snapshot.json", *binaries.values(),
                   *[Path(pin["path"]) for pin in reference_transport_refs],
                   binding.document, binding.state, binding.registry, *capacity_configs,
                   *[Path(pin["path"]) for pin in build_pins]]
    input_paths = list(dict.fromkeys(input_paths))
    if resumed_bundle is not None and resumed_receipt is not None:
        context_refs = gate._object(resumed_receipt.get("contexts"), "resume context refs")
        old_context = resumed_bundle.json(context_refs["before"])
        input_paths = [
            Path(gate._text(item.get("path"), "resumed context input path"))
            for item in gate._objects(old_context.get("inputs"), "resumed context inputs")
        ]
    arms = expected_arm_pins(
        builds, build_inputs.snapshot["sha256"], build_artifact_refs,
        reference_parent=reference_proof, reference_contract=reference_contract,
    )
    before_context = context(root, target, input_paths, lifecycle, git_query=git_query)
    before_pin = write(output / "context-before.json", before_context)
    environment_sha = before_pin["sha256"]
    if resumed_receipt is not None and resumed_bundle is not None:
        # 前回の終端と今回の開始が違う場合は、重い測定より先に拒否する。
        _validate_resume_batch(
            resumed_receipt, resumed_bundle, resumed_attempts, head=head, source=initial,
            before_context=before_context, after_context=before_context, arms=arms,
        )
    transport_attempts: list[dict[str, FilePin]] = list(resumed_attempts)
    expected_all: list[tuple[Path, ExpectedPerformance, gate.JsonObject, FilePin]] = []
    completed_rows: list[gate.JsonObject] = []
    checkpoint_pins: list[FilePin] = []
    worker_attempt_pins: list[FilePin] = []
    for row in measurement_rows:
        try:
            _require_batch_budget(batch_deadline)
        except BatchBudgetExhausted as error:
            batch_failure = error
            break
        fixture = cast(Path, row["fixture"])
        plan_path = cast(Path, row["plan_path"])
        plan_pin = cast(FilePin, gate._object(row["plan_pin"], "operation plan pin"))
        plan_data = gate._object(row["plan"], "operation plan")
        row_dom = row["dom"]
        dom_sha256 = gate._text(row["dom_sha256"], "DOM evidence SHA")
        row_viewport = gate._object(row["viewport"], "measurement viewport")
        width = gate._integer(row_viewport.get("width"), "viewport width")
        height = gate._integer(row_viewport.get("height"), "viewport height")
        folder = output / gate._text(row["folder_name"], "measurement folder")
        folder.mkdir()
        operation_name = gate._text(plan_data.get("operation") if not row["catalog"]
                                    else plan_data.get("operationKind"), "operation name")
        fixture_sha256 = gate._text(row["fixture_sha256"], "fixture SHA")
        viewport_value: Viewport = {"width": width, "height": height,
                                    "scale": gate._integer(row_viewport.get("scale"), "viewport scale")}
        dom_ref: FilePin = ({"path": str(row_dom), "sha256": dom_sha256} if row_dom is not None
                           else cast(FilePin, row["operation_evidence"]))
        expected: ExpectedPerformance = {"fixture_sha256": fixture_sha256, "operation_plan_sha256": plan_pin["sha256"], "operation": operation_name, "expected_dom_state_sha256": dom_sha256, "viewport": viewport_value, "provenance": {"rustc": before_context["toolContext"]["rustc"], "cargo": before_context["toolContext"]["cargo"], "environment_sha256": environment_sha}, "arms": arms, "reportPins": [], "operationPlanRef": plan_pin, "domStateRef": dom_ref}
        if reference_contract is not None:
            expected["referenceParent"] = dict(reference_contract)
        if row["catalog"]:
            catalog_identity = {key: row[key] for key in ("condition_id", "case_id", "state", "operation_kind", "target_selector")}
            catalog_identity["fixture_path"] = str(fixture)
            expected["catalogCondition"] = {**catalog_identity, "viewport": viewport_value, "fixture_sha256": fixture_sha256}
            expected["operationEvidenceRef"] = cast(FilePin, row["operation_evidence"])
            expected["catalogManifestRef"] = cast(FilePin, catalog_manifest_pin)
            expected["catalogPlanRef"] = cast(FilePin, catalog_plan_pin)
            expected["chromeCaptureRef"] = cast(SizedFilePin, row["chrome_capture"])
            expected["chromeReceiptRef"] = cast(SizedFilePin, row["chrome_receipt"])
            expected["catalogCaptureValidationRef"] = cast(FilePin, catalog_capture_validation_pin)
            expected["fontIdentityRequestRef"] = cast(SizedFilePin, row["font_identity_request"])
            expected["fontIdentityResponseRef"] = cast(SizedFilePin, row["font_identity_response"])
            expected["fontIdentityProbeBinary"] = {
                "path": cast(str, row["font_identity_binary_path"]),
                "sha256": cast(str, row["font_identity_binary_sha256"]),
            }
            if catalog_binary_refs is None:
                raise ValueError("catalog binary proof refs are missing")
            expected["catalogBinaryProofRef"] = catalog_binary_refs.proof_ref
            expected["catalogBuildReportRef"] = catalog_binary_refs.build_report_ref
            expected["catalogCaptureBinaryRef"] = catalog_binary_refs.binary_ref
            expected["catalogCurrentBuildReportRef"] = catalog_binary_refs.current_build_report_ref
            preflight_evidence = gate.read_json(Path(dom_ref["path"]))
            gate.validate_context_input_refs(
                cast(gate.JsonObject, before_context), cast(gate.JsonObject, expected),
                cast(gate.JsonObject, build_inputs.snapshot),
                cast(gate.JsonObject, row["capture"]),
                cast(list[gate.JsonObject], row["link_captures"]),
                gate.provenance_file_refs(preflight_evidence),
            )
        try:
            collection = _collect_condition_worker_pairs(
                folder=folder, row=row, expected=expected, fixture=fixture,
                plan_path=plan_path, width=width, height=height, viewport=viewport_value,
                builds=builds, binaries=binaries, arms=arms, root=root, lifecycle=lifecycle,
                lock=lock, snapshot=build_inputs.snapshot, environment_sha=environment_sha,
                batch_deadline=batch_deadline,
            )
        except BatchBudgetExhausted as error:
            worker_attempt_pins.extend(_worker_attempt_pins_in(folder))
            batch_failure = error
            break
        except BaseException as error:
            if not batch_mode:
                raise
            worker_attempt_pins.extend(_worker_attempt_pins_in(folder))
            batch_failure = error
            break
        order = collection.order
        reports = collection.reports
        condition_failures = collection.failures
        worker_attempt_pins.extend(collection.worker_attempt_pins)
        expected["reportPins"].extend(collection.report_pins)
        actual_phase_records = collection.actual_phase_records
        measurement_policy = _condition_measurement_policy(
            reports, expected, condition_failures, actual_phase_records,
        )
        manifest_data: gate.JsonObject = {"measurement_executed": True, "input_sha256": expected["fixture_sha256"], "environment_sha256": environment_sha, "platform": platform.platform(), "order": order}
        if row["catalog"]:
            manifest_data.update(_catalog_pair_manifest_fields(
                plan_data, row["condition_id"], row["operation_evidence"]))
        if condition_failures:
            manifest_data.update({"measurement_status": measurement_policy["status"],
                                  "failures": condition_failures})
        manifest_pin = write(folder / "reports/pair-manifest.json", manifest_data)
        result: gate.JsonObject = {"formalAccepted": False, "performanceAccepted": False,
            "measurementExecuted": True,
            "comparisonStatus": (measurement_policy.get("status")
                                 or ("paired" if measurement_policy.get("policyPass") is True
                                     else "incomplete")),
            "policy": measurement_policy}
        if row["catalog"]:
            result["catalogCondition"] = expected["catalogCondition"]
        checkpoint_pins.append(write_condition_checkpoint(folder, expected, result, manifest_data))
        expected_all.append((folder, expected, result, manifest_pin))
        completed_rows.append(row)
    after_context = context(root, target, input_paths, lifecycle, git_query=git_query)
    if resumed_receipt is not None and resumed_bundle is not None:
        _validate_resume_batch(
            resumed_receipt, resumed_bundle, resumed_attempts, head=head, source=initial,
            before_context=before_context, after_context=after_context, arms=arms,
        )
    end_pin = write(output / "context-end.json", after_context)
    source_end = write(output / "source-end.json", after_context["source"])
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    before_endpoint: Mapping[str, object] = before_context
    after_endpoint: Mapping[str, object] = after_context
    flags = {name: before_endpoint[key] == after_endpoint[key] for name, key in mapping.items()}
    refs: list[FilePin] = [build_inputs.source_before, source_end, approved, build_inputs.snapshot,
                           before_pin, end_pin, build_inputs.capacity_input, *build_pins,
                           *[FilePin(path=str(p), sha256=gate.sha256(p)) for p in binaries.values()],
                           *checkpoint_pins, *worker_attempt_pins, *capacity_transport_refs(binding)]
    refs.extend(reference_transport_refs)
    if resumed_bundle is not None:
        refs.extend(
            FilePin(path=gate._text(item.get("originalPath"), "resumed bundle path"),
                    sha256=gate._text(item.get("sha256"), "resumed bundle SHA"))
            for item in gate._objects(resumed_bundle.manifest.get("files"), "resumed bundle files")
        )
    for row in all_measurement_rows:
        refs.append(FilePin(path=str(row["plan_path"]), sha256=gate.sha256(cast(Path, row["plan_path"]))))
        if row["dom"] is not None:
            refs.append(FilePin(path=str(row["dom"]), sha256=gate.sha256(cast(Path, row["dom"]))))
        if row["catalog"]:
            evidence = cast(FilePin, row["operation_evidence"])
            capture = cast(FilePin, row["capture"])
            refs.extend([evidence, capture,
                         cast(SizedFilePin, row["chrome_capture"]),
                         cast(SizedFilePin, row["chrome_receipt"]),
                         cast(SizedFilePin, row["font_identity_request"]),
                         cast(SizedFilePin, row["font_identity_response"]),
                         *cast(list[FilePin], row["link_captures"])])
    if catalog_manifest_path is not None and catalog_manifest_pin is not None:
        refs.append(catalog_manifest_pin)
    if catalog_plan_pin is not None:
        refs.append(catalog_plan_pin)
    if catalog_capture_validation_pin is not None:
        refs.append(catalog_capture_validation_pin)
    if catalog_binary_refs is not None:
        refs.extend((catalog_binary_refs.proof_ref,
                     catalog_binary_refs.build_report_ref,
                     catalog_binary_refs.binary_ref,
                     catalog_binary_refs.current_build_report_ref))
    for folder, expected, result, manifest_pin in expected_all:
        expected["sourcePair"] = {"before": build_inputs.source_before, "end": source_end,
            "approvedSourceRef": approved, "inputPins": [approved, build_inputs.capacity_input],
            "approvedInventoryDigest": inventory_digest, "snapshot": build_inputs.snapshot}
        expected["endpointRefs"] = {"before": before_pin, "end": end_pin}
        result.update(flags)
        expected_pin = write(folder / "expected-enriched.json", expected)
        result_pin = write(folder / "result.json", result)
        transport_attempts.append({"expected": expected_pin, "result": result_pin, "pairManifest": manifest_pin})
        refs.extend([expected_pin, result_pin, manifest_pin, *expected["reportPins"]])
    # 終了後に確定したrawとclosure記録も同じ輸送SHAへ束縛する。
    for log_directory in [lifecycle, *(output / "builds").glob("*.lifecycle")]:
        for path in sorted(log_directory.rglob("*")):
            if path.is_file():
                refs.append(FilePin(path=str(path), sha256=gate.sha256(path)))
    # 生成物だけを輸送し、巨大なCargo targetは公開artifactへ混入させない。
    bundle = output / "bundle"
    write_transport_bundle(bundle, head, refs, transport_attempts)
    current_bundle = gate.Bundle(bundle)
    attempt_ids = [_attempt_condition_id(current_bundle, attempt)
                   for attempt in transport_attempts]
    gate.require(len(attempt_ids) == len(set(attempt_ids)),
                 "batch attempts contain duplicate conditions")
    complete_ids = _completed_batch_ids(
        resumed_ids, measurement_rows, completed_rows, expected_all,
    )
    policy_accepted = _batch_policy_accepted(
        batch_failure, [result for _, _, result, _ in expected_all],
        resumed_receipt is None or resumed_receipt.get("policyAccepted") is True,
    )
    comparison_results = [
        current_bundle.json(gate._object(attempt["result"], "attempt result reference"))
        for attempt in transport_attempts
    ]
    paired_conditions = sum(
        result.get("comparisonStatus") == "paired" for result in comparison_results
    )
    non_comparable_conditions = sum(
        result.get("comparisonStatus") == "nonComparable" for result in comparison_results
    )
    summary: gate.JsonObject = {
        "head": head, "bundle": str(bundle), "checks": len(transport_attempts) * 20,
        "completeConditions": len(complete_ids), "conditions": len(attempt_ids),
        "candidateConditions": paired_conditions + non_comparable_conditions,
        "pairedConditions": paired_conditions,
        "nonComparableConditions": non_comparable_conditions,
        "formalAccepted": _batch_formal_acceptance(
            attempt_ids, complete_ids, policy_accepted,
        ),
        "phaseRecords": _saved_phase_record_count(comparison_results),
        "batchFailure": type(batch_failure).__name__ if batch_failure else None,
    }
    if catalog_manifest_path is not None:
        validated_builds: Mapping[str, gate.JsonObject] | None = None
        if transport_attempts and not _allow_catalog_diagnostic:
            first_expected = current_bundle.json(transport_attempts[0]["expected"])
            validated_builds = gate.validate_expected_builds(
                current_bundle, root, first_expected, git_query=git_query,
            )
        for attempt in transport_attempts:
            if _attempt_condition_id(current_bundle, attempt) in set(complete_ids):
                gate.validate_attempt(
                    current_bundle, root,
                    {key: value for key, value in attempt.items()}, git_query=git_query,
                    validated_builds=validated_builds,
                )
        def seal_batch_receipt() -> FilePin:
            return write_batch_receipt(
                output, head=head, source_before=build_inputs.source_before,
                source_end=source_end, context_before=before_pin, context_end=end_pin,
                build_artifacts=build_artifact_refs, build_pins=build_pins,
                condition_ids=attempt_ids, complete_condition_ids=complete_ids,
                policy_accepted=policy_accepted,
                attempts=transport_attempts, bundle=bundle,
            )
        if batch_failure is not None:
            _seal_failed_batch(batch_failure, seal_batch_receipt, summary)
        seal_batch_receipt()
    if catalog_manifest_path is None:
        gate._verify_legacy_diagnostic(bundle, root, git_query=git_query)
    elif _batch_formal_acceptance(attempt_ids, complete_ids, policy_accepted) and _allow_catalog_diagnostic:
        gate._verify_catalog_diagnostic(bundle, root, git_query=git_query)
    elif _batch_formal_acceptance(attempt_ids, complete_ids, policy_accepted):
        gate.verify(bundle, root, git_query=git_query)
    print(json.dumps(summary))
    if batch_failure is not None:
        raise batch_failure


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, default=Path("tmp/html-performance-producer"))
    parser.add_argument("--capacity-binding", type=Path)
    parser.add_argument("--capacity-binding-sha256")
    parser.add_argument("--catalog-plan", type=Path)
    parser.add_argument(
        "--max-conditions", type=int,
        help="run at most this many not-yet-complete catalog conditions",
    )
    parser.add_argument(
        "--resume-batch", type=Path,
        help="reuse only the closed, strictly validated conditions from this batch",
    )
    parser.add_argument("--reference-parent-wrapper", type=Path)
    parser.add_argument("--reference-parent-wrapper-sha256")
    parser.add_argument("--candidate-build-only", action="store_true")
    parser.add_argument(
        "--diagnostic-only",
        action="store_true",
        help="produce a catalog-bound diagnostic bundle while retaining acceptance=false",
    )
    args = parser.parse_args()
    if args.candidate_build_only:
        if (args.catalog_plan is not None or args.capacity_binding is None
                or args.capacity_binding_sha256 is None or args.diagnostic_only
                or args.max_conditions is not None or args.resume_batch is not None
                or args.reference_parent_wrapper is not None
                or args.reference_parent_wrapper_sha256 is not None):
            parser.error("candidate build-only mode requires capacity binding pins and no catalog plan")
        repo = args.repo.resolve()
        with candidate_build_locks(repo):
            result = produce_candidate_build_proof(
                repo, args.output.resolve(), args.capacity_binding, args.capacity_binding_sha256,
            )
        print(json.dumps(result, sort_keys=True))
        return
    if args.catalog_plan is None:
        parser.error("--catalog-plan is required for HTML performance production")
    if args.max_conditions is not None and not 0 < args.max_conditions <= 84:
        parser.error("--max-conditions must be between 1 and 84")
    if (args.reference_parent_wrapper is None) != (args.reference_parent_wrapper_sha256 is None):
        parser.error("reference parent wrapper requires --reference-parent-wrapper-sha256")
    repo = args.repo.resolve()
    arguments = (repo, args.output.resolve(), args.capacity_binding,
                 args.capacity_binding_sha256, args.catalog_plan.resolve())
    batch_options: BatchOptions = {}
    if args.max_conditions is not None:
        batch_options["max_conditions"] = args.max_conditions
    if args.resume_batch is not None:
        batch_options["resume_batch"] = args.resume_batch.resolve()
    if args.reference_parent_wrapper is not None:
        batch_options["reference_parent_wrapper"] = args.reference_parent_wrapper.resolve()
        batch_options["reference_parent_wrapper_sha256"] = cast(
            str, args.reference_parent_wrapper_sha256,
        )
    with candidate_build_locks(repo):
        if args.diagnostic_only:
            produce(*arguments, _allow_catalog_diagnostic=True, **batch_options)
        else:
            produce(*arguments, **batch_options)


if __name__ == "__main__":
    main()
