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
from contextlib import ExitStack
from pathlib import Path
from types import FrameType
from typing import BinaryIO, Literal, NamedTuple, TypedDict, cast, overload
from collections.abc import Callable, Mapping, Sequence

import verify_html_performance_evidence as gate
from toml_loader import load_toml_parser

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
LOCAL_CAPACITY_BASELINE = 2363047936
LOCAL_CAPACITY_THRESHOLD = 4726095872
PROTECTED_REGISTRY_SCHEMA = "krr-protected-refs-successor-v1"


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


class CapacityBinding(NamedTuple):
    document: Path
    document_sha256: str
    state: Path
    state_sha256: str
    registry: Path
    registry_sha256: str
    references: tuple[str, ...]


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


class ExpectedBuild(NamedTuple):
    harness: gate.JsonObject
    runtime: gate.JsonObject
    arm: str


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


def _runtime_lock_packages(harness: Path) -> dict[str, gate.JsonObject]:
    raw = (harness / "Cargo.lock").read_bytes()
    gate.require(hashlib.sha256(raw).hexdigest() == gate.LOCK_SHA, "common lock pin differs")
    lock = gate._object(load_toml_parser().loads(raw.decode("utf8")), "common Cargo lock")
    packages = [p for p in gate._objects(lock["package"], "locked packages")
                if p.get("name") == "katana-render-runtime"]
    baseline = [p for p in packages if p.get("source") == "registry+https://github.com/rust-lang/crates.io-index"
                and p.get("version") == "0.4.23"]
    candidate = [p for p in packages if "source" not in p]
    gate.require(len(packages) == 2 and len(baseline) == 1 and len(candidate) == 1,
                 "fixed runtime lock packages differ")
    gate._text(candidate[0]["version"], "locked candidate version")
    return {"baseline": baseline[0], "candidate": candidate[0]}


def expected_build(root: Path, harness: Path, arm: str, metadata: gate.JsonObject,
                   environment: Mapping[str, str]) -> ExpectedBuild:
    gate.require(gate._integer(metadata["version"], "metadata schema version") == 1, "metadata schema differs")
    packages = gate._objects(metadata["packages"], "standalone Cargo packages")
    harness_packages = [p for p in packages if p.get("manifest_path") == str(harness / "Cargo.toml")
                        and p.get("name") == "krr-html-performance-harness"]
    gate.require(len(harness_packages) == 1, "standalone manifest identity differs")
    selected_harness = harness_packages[0]
    harness_manifest = gate._object(load_toml_parser().loads((harness / "Cargo.toml").read_text()), "harness manifest")
    harness_package = gate._object(harness_manifest["package"], "harness package")
    gate.require(selected_harness["source"] is None
                 and selected_harness.get("version") == gate._text(harness_package["version"], "harness version"),
                 "harness package version/origin differs")
    harness_id = gate._text(selected_harness["id"], "harness package id")
    gate.require(metadata["workspace_members"] == [harness_id], "standalone workspace members differ")
    ids = [gate._text(p["id"], "metadata package id") for p in packages]
    gate.require(len(set(ids)) == len(ids), "duplicate metadata package id")
    resolve = gate._object(metadata["resolve"], "Cargo resolve")
    gate.require(resolve["root"] == harness_id, "Cargo resolve root differs")
    nodes = gate._objects(resolve["nodes"], "Cargo resolve nodes")
    roots = [node for node in nodes if node.get("id") == harness_id]
    gate.require(len(roots) == 1 and roots[0].get("features") == [arm], "Cargo arm features differ")
    dependencies = gate._objects(roots[0]["deps"], "harness resolved dependencies")
    arms = [dep for dep in dependencies if dep.get("name") in {"baseline", "candidate"}]
    gate.require(len(arms) == 1 and arms[0].get("name") == arm, "Cargo resolved arm differs")
    runtime_id = gate._text(arms[0]["pkg"], "runtime package id")
    runtime_packages = [p for p in packages if p.get("id") == runtime_id]
    gate.require(len(runtime_packages) == 1, "resolved runtime package missing")
    runtime = dict(runtime_packages[0])
    runtime_nodes = [node for node in nodes if node.get("id") == runtime_id]
    gate.require(len(runtime_nodes) == 1, "resolved runtime node missing")
    runtime["resolvedFeatures"] = [gate._text(f, "resolved runtime feature")
                                   for f in gate._array(runtime_nodes[0]["features"], "resolved runtime features")]
    locked = _runtime_lock_packages(harness)[arm]
    gate.require(runtime.get("name") == "katana-render-runtime" and runtime.get("version") == locked["version"],
                 "runtime name/version differs from fixed lock")
    manifest = _canonical_file(runtime["manifest_path"], "runtime manifest")
    if arm == "baseline":
        cargo_home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
        gate.require(runtime.get("source") == locked["source"]
                     and manifest.is_relative_to(cargo_home / "registry/src")
                     and manifest.parent.name == "katana-render-runtime-0.4.23", "baseline runtime origin differs")
    else:
        gate.require(runtime["source"] is None
                     and manifest == root / "crates/katana-render-runtime/Cargo.toml", "candidate runtime origin differs")
        manifest_value = gate._object(load_toml_parser().loads(manifest.read_text()), "candidate manifest")
        package = gate._object(manifest_value["package"], "candidate package")
        inherited = gate._object(package["version"], "candidate inherited version")
        workspace = gate._object(load_toml_parser().loads((root / "Cargo.toml").read_text()), "root manifest")
        workspace_package = gate._object(gate._object(workspace["workspace"], "workspace")["package"], "workspace package")
        gate.require(inherited.get("workspace") is True and workspace_package.get("version") == locked["version"],
                     "candidate version differs from fixed lock")
    for package, name, kind, source in ((selected_harness, "krr-html-performance-harness", ["bin"], harness / "src/main.rs"),
                                       (runtime, "katana_render_runtime", ["lib"], manifest.parent / "src/lib.rs")):
        targets = [t for t in gate._objects(package["targets"], "Cargo package targets") if t.get("name") == name]
        gate.require(len(targets) == 1 and targets[0].get("kind") == kind
                     and targets[0].get("crate_types") == kind and targets[0].get("src_path") == str(source),
                     "Cargo package target differs")
        _canonical_file(str(source), "Cargo target source")
    return ExpectedBuild(selected_harness, runtime, arm)


def cargo_build_artifacts(raw: bytes, expected: ExpectedBuild, target: Path) -> tuple[Path, gate.JsonObject]:
    events: list[gate.JsonObject] = []
    terminal_seen = False
    for line in raw.decode("utf8").splitlines():
        gate.require(not terminal_seen or not line.strip(), "nonterminal build-finished event")
        candidate = line.lstrip()
        if not candidate:
            continue
        starts_object = candidate.startswith("{")
        starts_array = candidate.startswith("[")
        starts_scalar = candidate[0] in '"tfn-0123456789'
        if not (starts_object or starts_array or starts_scalar):
            continue
        try:
            decoded = json.loads(candidate, object_pairs_hook=gate.unique_object)
        except json.JSONDecodeError:
            gate.require(not starts_object, "malformed Cargo JSON object")
            # proc-macro出力はCargo JSONではなく「[」から始まることがある。
            continue
        event = gate._object(decoded, "Cargo JSON event")
        reason = gate._text(event["reason"], "Cargo event reason")
        events.append(event)
        terminal_seen = reason == "build-finished"
    finished = [e for e in events if e.get("reason") == "build-finished"]
    gate.require(len(finished) == 1 and finished[0].get("success") is True
                 and events[-1] is finished[0], "unique terminal successful build-finished required")
    selected: list[gate.JsonObject] = []
    binary: Path | None = None
    for package, name in ((expected.runtime, "katana_render_runtime"), (expected.harness, "krr-html-performance-harness")):
        rows = [e for e in events if e.get("reason") == "compiler-artifact"
                and gate._object(e.get("target"), "artifact target").get("name") == name]
        gate.require(len(rows) == 1, "unique runtime/harness compiler-artifact required")
        artifact = rows[0]
        profile = gate._object(artifact["profile"], "artifact profile")
        actual_target = gate._object(artifact["target"], "artifact target")
        expected_targets = [t for t in gate._objects(package["targets"], "expected targets") if t.get("name") == name]
        gate.require(len(expected_targets) == 1, "expected artifact target differs")
        gate.require(artifact.get("package_id") == package["id"] and artifact.get("manifest_path") == package["manifest_path"]
                     and all(actual_target.get(k) == expected_targets[0][k] for k in ("name", "kind", "crate_types", "src_path")),
                     "artifact package/origin/target differs")
        gate.require(profile.get("opt_level") == "3" and profile.get("test") is False
                     and type(artifact.get("fresh")) is bool, "artifact is not release non-test")
        features = [gate._text(f, "artifact feature") for f in gate._array(artifact["features"], "artifact features")]
        if package is expected.runtime:
            gate.require(sorted(features) == sorted(gate._text(f, "expected runtime feature")
                         for f in gate._array(package["resolvedFeatures"], "expected runtime features")),
                         "runtime artifact features differ")
        if package is expected.harness:
            gate.require(features == [expected.arm], "harness artifact arm features differ")
            binary = _canonical_file(artifact["executable"], "artifact executable")
            gate.require(binary == target / "release/krr-html-performance-harness"
                         and str(binary) in [gate._text(f, "artifact filename") for f in gate._array(artifact["filenames"], "artifact filenames")],
                         "artifact executable namespace differs")
        else:
            gate.require(artifact["executable"] is None, "runtime library executable differs")
        selected.append(artifact)
    gate.require(binary is not None, "artifact executable missing")
    return _canonical_file(str(binary), "artifact executable"), {"runtimeArtifact": selected[0], "binaryArtifact": selected[1], "buildFinished": finished[0]}


def _file_identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns


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
    for reference in references:
        path = Path(reference)
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
    cargo_value = shutil.which("cargo")
    gate.require(cargo_value is not None, "Cargo executable is unavailable")
    cargo = Path(gate._text(cargo_value, "Cargo executable")).resolve(strict=True)
    capacity.validate_build_executable(root, [str(cargo), "build"])
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
        [str(cargo), "metadata", "--format-version", "1", "--locked", "--manifest-path",
         str(harness / "Cargo.toml"), "--no-default-features", "--features", arm],
        cwd=harness, env=environment, role="metadata",
        log_directory=output.parent / f"{output.name}.lifecycle",
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
    load_capacity_binding(root, binding.document, binding.document_sha256)
    argv = [sys.executable, str(root / "scripts/build/cargo_capacity_guard.py"), "--root", str(root),
            "--target-dir", str(target), "--state", str(binding.state), "--factor", "2",
            "--expected-state-sha256", binding.state_sha256]
    for reference in references:
        argv.extend(["--protected-ref", reference])
    for profile in retained:
        argv.extend(["--retain-profile", str(profile)])
    argv.extend(["--", str(cargo), "build", "--release", "--locked", "--manifest-path", str(harness / "Cargo.toml"),
                 "--target-dir", str(target), "--message-format=json", "--no-default-features", "--features", arm])
    output.mkdir(parents=True, exist_ok=False)
    result = _measurement_command(argv, cwd=root, env=environment, check=False, role="build",
                                  log_directory=output.parent / f"{output.name}.lifecycle")
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
    frozen = output / "binary" / "krr-html-performance-harness"
    checksum = freeze_binary(binary, frozen)
    proof: gate.JsonObject = {"schema": "krr-html-performance-build-artifact-v1", "arm": arm,
        "argv": argv, "buildExit": build_exit, "expectedHarness": expected.harness,
        "expectedRuntime": expected.runtime, "artifacts": artifacts, "rawPins": pins,
        "lockSha256": gate.LOCK_SHA, "selectedBinary": {"path": str(binary), "sha256": checksum}, "frozenBinary": {"path": str(frozen), "sha256": checksum}}
    return ArmBuildResult(configs, frozen, checksum, proof, pins)


def require_frozen_binary(build: ArmBuildResult) -> None:
    gate.require(not build.binary.is_symlink() and build.binary.resolve() == build.binary
                 and gate.sha256(build.binary) == build.binary_sha256,
                 "frozen build binary changed before measurement")


def expected_arm_pins(builds: Mapping[str, ArmBuildResult], snapshot_sha256: str) -> dict[str, ArmPin]:
    arms: dict[str, ArmPin] = {}
    for arm, build in builds.items():
        require_frozen_binary(build)
        arms["baseline-0.4.23" if arm == "baseline" else arm] = {
            "binary_sha256": build.binary_sha256,
            "source_snapshot_sha256": snapshot_sha256,
            "lock_sha256": gate.LOCK_SHA,
        }
    return arms


def encoded(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def write(path: Path, value: object) -> FilePin:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))
    return {"path": str(path.resolve()), "sha256": gate.sha256(path)}


MeasurementRole = Literal["tool-query", "metadata", "preflight", "build", "worker"]
# 軽いquery、graph解決、同期検査、release build、単一workerに別の停止上限を置く。
MEASUREMENT_LIMITS: dict[MeasurementRole, float] = {
    "tool-query": 30, "metadata": 120, "preflight": 120, "build": 1800, "worker": 60,
}
MEASUREMENT_CLEANUP_SECONDS = 10.0
MEASUREMENT_TERM_SECONDS = 2.0


def _measurement_exit(process: subprocess.Popen[bytes]) -> bool:
    observed: object = process.poll()
    if observed is None:
        return False
    gate._integer(observed, "measurement exit code")
    return True


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
            if record.get("a") != "r":
                holders.append({"pid": pid, "processGroupId": group,
                                "descriptor": record["f"], "access": record.get("a", "unknown")})
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
    lifecycle_failure: BaseException | None = None
    started = time.monotonic()
    closed = False
    raw_closed = False
    identity: gate.JsonObject = {}
    previous_mask: set[int | signal.Signals] | None = None
    previous_handlers: dict[int, Callable[[int, FrameType | None], object] | int | None] = {}
    pending: list[int] = []
    signal_observations: list[str] = []
    armed = False
    raw_identities: list[tuple[int, int]] = []
    native_closure: gate.JsonObject | None = None
    cleanup_deadline: float | None = None

    def interrupted(signum: int, frame: FrameType | None) -> None:
        del frame
        if not armed:
            pending.append(signum)
        elif signum == signal.SIGINT:
            raise KeyboardInterrupt
        else:
            raise InterruptedError(f"measurement interrupted by signal {signum}")

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
                armed = True
                if pending:
                    interrupted(pending[0], None)
                deadline = started + MEASUREMENT_LIMITS[role]
                while True:
                    if time.monotonic() >= deadline:
                        raise subprocess.TimeoutExpired(list(args), MEASUREMENT_LIMITS[role])
                    if _measurement_exit(process):
                        break
                    time.sleep(min(0.01, max(0.0, deadline - time.monotonic())))
            except BaseException as error:
                primary = error
            finally:
                if process is not None:
                    try:
                        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM, signal.SIGHUP})
                        if native_attempt_token is not None:
                            cleanup_deadline = time.monotonic() + MEASUREMENT_CLEANUP_SECONDS
                            signal_observations = _close_measurement_group(process, cleanup_deadline)
                        else:
                            signal_observations = _close_measurement_group(process)
                        closed = True
                    except BaseException as error:
                        cleanup_failure = error
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
                cleanup_failure = error
                if primary is None:
                    primary = error
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
            cleanup_failure = error
    finally:
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
            if previous_mask is not None:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
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


def observe_native_cli_command(args: Sequence[str], *, role: MeasurementRole,
                               log_directory: Path, cwd: Path, env: Mapping[str, str],
                               attempt_token: str, label: str) -> NativeCliCommandEvidence:
    """CLI専用producerへ、既存ownerが閉じたraw記録だけを返す。"""
    gate.require(role in ("tool-query", "metadata", "preflight", "build"),
                 "native CLI command role differs")
    gate.require(len(attempt_token) == 64 and all(c in "0123456789abcdef" for c in attempt_token),
                 "native CLI attempt token differs")
    paths: list[Path] = []
    started = time.time_ns()
    result = _measurement_command(args, role=role, log_directory=log_directory, cwd=cwd,
                                  env=env, lifecycle_sink=paths.append, native_attempt_token=attempt_token)
    ended = time.time_ns()
    gate.require(len(paths) == 1, "native CLI lifecycle must be unique")
    lifecycle_path = paths[0]
    lifecycle = gate._object(gate.read_json(lifecycle_path), "native CLI lifecycle")
    gate.require(lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
                 and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None
                 and lifecycle.get("exitCode") == 0, "native CLI command closure differs")
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
        "pid": identity["pid"], "actualExit": 0, "startedNs": str(started), "endedNs": str(ended),
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
    def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str | bytes:
        completed = _measurement_command(["git", "-C", str(root), *args], role="tool-query",
                                         log_directory=log_directory, cwd=root, input_bytes=input_bytes)
        if text:
            with io.TextIOWrapper(io.BytesIO(completed.stdout)) as stream:
                return gate._text(stream.read(), "Git query stdout")
        return completed.stdout

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


def inventory(path: Path) -> gate.JsonObject:
    # 計測区間ではregistryと容量ガード対象target全体の内容変化を検出する。
    if not path.exists():
        return {"root": str(path), "exists": False, "files": []}
    rows: list[gate.JsonObject] = []
    for member in sorted(path.rglob("*")):
        if member.is_symlink():
            rows.append({"path": str(member.relative_to(path)), "link": os.readlink(member)})
        elif member.is_file():
            rows.append({"path": str(member.relative_to(path)), "sha256": gate.sha256(member), "size": member.stat().st_size})
    return {"root": str(path), "exists": True, "files": rows}


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


def await_quiet() -> float:
    deadline = time.monotonic() + 900
    while quiet() > 0.25:
        gate.require(time.monotonic() < deadline, "quiet load0.25 was not reached")
        time.sleep(5)
    return quiet()


def worker_sample(stdout: bytes, sample: int, viewport: Viewport) -> gate.JsonObject:
    payload = gate._object(json.loads(stdout, object_pairs_hook=gate.unique_object), "worker sample")
    actual_viewport = gate._object(payload["viewport"], "worker viewport")
    gate.require(gate._integer(payload["sample"], "worker sample id") == sample and actual_viewport == viewport,
                 "worker sample/viewport differs")
    for field in ("width", "height"):
        gate._integer(actual_viewport[field], "worker viewport " + field)
    gate._numeric(actual_viewport["scale"], "worker viewport scale")
    return payload


def validate_interaction(value: object, fixture_sha256: str) -> None:
    state = gate._object(value, "interaction evidence")
    interaction_states = gate._object(state["states"], "interaction states")
    js_state = gate._object(interaction_states["js"], "interaction JavaScript state")
    gate.require(state["inputSha256"] == fixture_sha256 and js_state == {"expanded": True, "result": "opened"}
                 and js_state["expanded"] is True, "real same-source interaction evidence absent")


def _catalog_operation_stream(operation_kind: str, evidence: gate.JsonObject) -> list[gate.JsonObject]:
    return gate.catalog_operation_stream(operation_kind, evidence)


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


def load_catalog_conditions(root: Path, manifest_path: Path) -> tuple[list[CatalogCondition], FilePin, FilePin]:
    _pinned_file(manifest_path, gate.sha256(manifest_path))
    manifest = gate.read_json(manifest_path)
    gate.require(manifest.get("profile") == "catalog-stage90-captures"
                 and manifest.get("acceptance") is False,
                 "catalog performance input must be the non-accepted stage90 capture manifest")
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
        stream = _catalog_operation_stream(operation_kind, evidence)
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
            {"path": str(plan_path), "sha256": plan_sha})


def produce(root: Path, output: Path, capacity_document: Path | None = None,
            capacity_document_sha256: str | None = None,
            catalog_manifest_path: Path | None = None, *,
            _allow_legacy_diagnostic: bool = False,
            _allow_catalog_diagnostic: bool = False) -> None:
    if catalog_manifest_path is None and not _allow_legacy_diagnostic:
        raise ValueError("catalog capture manifest required for formal performance production")
    gate.require(platform.system() == "Darwin" and platform.machine() == "arm64", "measurement lane requires macOS Apple Silicon")
    gate.require(not output.exists() and output.is_relative_to(root / "tmp"), "fresh ignored owned output required")
    catalog_conditions: list[CatalogCondition] | None = None
    catalog_manifest_pin: FilePin | None = None
    catalog_plan_pin: FilePin | None = None
    if catalog_manifest_path is not None:
        catalog_conditions, catalog_manifest_pin, catalog_plan_pin = load_catalog_conditions(root, catalog_manifest_path)
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    # 生成済みJSとsourceの同期が未確認のcandidateをビルド・計測しない。
    lifecycle = output.parent / f"{output.name}.lifecycle"
    gate.require(not lifecycle.exists(), "fresh measurement lifecycle output required")
    git_query = _make_git_query(lifecycle)
    initial = source(root, git_query=git_query)
    catalog_capture_validation: gate.JsonObject | None = None
    if catalog_conditions is not None:
        if catalog_manifest_pin is None or catalog_plan_pin is None:
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
    source_before = write(output / "source-before.json", initial)
    capacity_input_value: gate.JsonObject = {
        "sourceBeforeRef": source_before,
        "bindingRef": {"path": str(binding.document), "sha256": binding.document_sha256},
        "stateRef": {"path": str(binding.state), "sha256": binding.state_sha256},
        "protectedRegistryRef": {"path": str(binding.registry), "sha256": binding.registry_sha256},
        "protectedReferenceCount": len(binding.references), "target": str(root / "target"),
        "baselineBytes": LOCAL_CAPACITY_BASELINE, "factor": 2, "thresholdBytes": LOCAL_CAPACITY_THRESHOLD,
    }
    if catalog_manifest_pin is not None:
        capacity_input_value["catalogManifestRef"] = catalog_manifest_pin
    if catalog_plan_pin is not None:
        capacity_input_value["catalogPlanRef"] = catalog_plan_pin
    capacity_input_pin = write(output / "capacity-inputs.json", capacity_input_value)
    approved = write(output / "approved-source.json", initial)
    head = gate._text(initial["head"], "source HEAD")
    inventory_digest = gate._text(initial["inventoryDigest"], "source inventory digest")
    snapshot_rows = [{**row, "path": str(Path(gate._text(row["path"], "source path")).relative_to(root))}
                     for row in gate._objects(initial["files"], "source files")]
    snapshot_pin = write(output / "shared-source-snapshot.json", {"records": snapshot_rows, "digest": hashlib.sha256(encoded(snapshot_rows)).hexdigest()})
    target = root / "target"
    source_records = gate.current_source(root, initial, git_query=git_query)
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
    build_pins: list[FilePin] = []
    for arm in ("baseline", "candidate"):
        build = guarded_arm_build(root, harness, arm, binding, source_records, output / "builds" / arm)
        capacity_configs = build.configs
        build.proof.update({"sourceBeforeRef": source_before, "sourceSnapshotRef": snapshot_pin})
        build_pins.extend([*build.pins, write(output / "builds" / arm / "build-artifact.json", build.proof)])
        binaries[arm] = build.binary
        builds[arm] = build
        gate.require(source(root, git_query=git_query) == initial, "source changed during build")
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
    font_probe_input_paths: list[Path] = []
    if catalog_conditions:
        first_catalog = catalog_conditions[0]
        font_probe_input_paths = [
            Path(first_catalog["font_identity_request"]["path"]),
            Path(first_catalog["font_identity_response"]["path"]),
            Path(first_catalog["font_identity_binary_path"]),
        ]
    input_paths = [*[cast(Path, row["fixture"]) for row in measurement_rows],
                   *[cast(Path, row["plan_path"]) for row in measurement_rows],
                   *[cast(Path, row["dom"]) for row in measurement_rows if row["dom"] is not None],
                   *[Path(cast(FilePin, row["operation_evidence"])["path"]) for row in measurement_rows if row["catalog"]],
                   *[Path(cast(FilePin, row["capture"])["path"]) for row in measurement_rows if row["catalog"]],
                   *[Path(pin["path"]) for row in measurement_rows if row["catalog"]
                     for pin in cast(list[FilePin], row["link_captures"])],
                   *[Path(cast(SizedFilePin, row["chrome_capture"])['path']) for row in measurement_rows if row["catalog"]],
                   *[Path(cast(SizedFilePin, row["chrome_receipt"])['path']) for row in measurement_rows if row["catalog"]],
                   *font_probe_input_paths,
                   *([catalog_manifest_path] if catalog_manifest_path is not None else []),
                   *([Path(catalog_plan_pin["path"])] if catalog_plan_pin is not None else []),
                   *([Path(catalog_capture_validation_pin["path"])] if catalog_capture_validation_pin is not None else []),
                   lock, output / "shared-source-snapshot.json", *binaries.values(),
                   binding.document, binding.state, binding.registry, *capacity_configs,
                   *[Path(pin["path"]) for pin in build_pins]]
    arms = expected_arm_pins(builds, snapshot_pin["sha256"])
    before_context = context(root, target, input_paths, lifecycle, git_query=git_query)
    before_pin = write(output / "context-before.json", before_context)
    environment_sha = before_pin["sha256"]
    transport_files: list[gate.JsonObject] = []
    transport_attempts: list[dict[str, FilePin]] = []
    transport: gate.JsonObject = {"schema": "krr-html-performance-release-bundle-v1", "head": head, "files": transport_files, "attempts": transport_attempts}
    expected_all: list[tuple[Path, ExpectedPerformance, gate.JsonObject, FilePin]] = []
    for row in measurement_rows:
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
        order: list[gate.JsonObject] = []
        reports: list[Path] = []
        for pair in range(5):
            for arm in (("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")):
                binary = binaries[arm]
                report_path = folder / "reports" / f"{arm}-{pair}.json"
                raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
                environment = _performance_worker_environment(expected["provenance"]["rustc"], expected["provenance"]["cargo"], environment_sha, lock, snapshot_pin)
                samples: list[gate.JsonObject] = []
                loads: list[dict[str, float]] = []
                for sample in range(5):
                    load_before = await_quiet()
                    require_frozen_binary(builds[arm])
                    stdout = command([str(binary), "--worker", str(sample), str(fixture), str(report_path), str(plan_path), str(width), str(height), str(viewport_value["scale"])], cwd=root, env=environment, role="worker", log_directory=lifecycle)
                    load_after = quiet()
                    gate.require(load_after <= 0.25 and gate.sha256(binary) == arms[raw_arm]["binary_sha256"], "worker load/binary changed")
                    payload = worker_sample(stdout, sample, viewport_value)
                    samples.append(payload)
                    loads.append({"before": load_before, "after": load_after})
                report: gate.JsonObject = {"arm": raw_arm, "pair_index": pair, "measurement_executed": True, "sample_count": 5, "source_sha256": expected["fixture_sha256"], "operation_plan_sha256": expected["operation_plan_sha256"], "operation": expected["operation"], "expected_dom_state_sha256": expected["expected_dom_state_sha256"], "viewport": viewport_value, "provenance": {**expected["provenance"], "binary_sha256": arms[raw_arm]["binary_sha256"], "source_snapshot_sha256": snapshot_pin["sha256"], "deps_lock_sha256": gate.LOCK_SHA}, "samples": samples}
                if row["catalog"]:
                    report.update({"condition_id": row["condition_id"], "case_id": row["case_id"],
                        "state": row["state"], "operation_kind": row["operation_kind"],
                        "operation_evidence_ref": row["operation_evidence"],
                        "catalog_condition": expected["catalogCondition"]})
                report_pin = write(report_path, report)
                expected["reportPins"].append(report_pin)
                reports.append(report_path)
                order.append({"arm": arm, "pair_index": pair, "executable": str(binary), "executable_sha256": arms[raw_arm]["binary_sha256"], "output": str(report_path), "load_ratio_before": max(x["before"] for x in loads), "load_ratio_after": max(x["after"] for x in loads), "workerLoads": loads})
        manifest_data: gate.JsonObject = {"measurement_executed": True, "input_sha256": expected["fixture_sha256"], "environment_sha256": environment_sha, "platform": platform.platform(), "order": order}
        if row["catalog"]:
            manifest_data.update({"condition_id": row["condition_id"], "operation_evidence_ref": row["operation_evidence"]})
        manifest_pin = write(folder / "reports/pair-manifest.json", manifest_data)
        aggregate = gate.aggregate_comparison(reports, {**expected})
        policy = {p: {k: v <= 0 for k, v in values.items()} for p, values in aggregate["comparison"].items()}
        result: gate.JsonObject = {"formalAccepted": False, "performanceAccepted": False, "measurementExecuted": True, "policy": {"policyPass": all(ok for phase in policy.values() for ok in phase.values()), "policy": policy, "deltaThreshold": 0, "quietLoadLimit": 0.25, "phaseRecords": len(gate.PHASES) * 50, "aggregate": aggregate}}
        if row["catalog"]:
            result["catalogCondition"] = expected["catalogCondition"]
        expected_all.append((folder, expected, result, manifest_pin))
    after_context = context(root, target, input_paths, lifecycle, git_query=git_query)
    end_pin = write(output / "context-end.json", after_context)
    source_end = write(output / "source-end.json", after_context["source"])
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    before_endpoint: Mapping[str, object] = before_context
    after_endpoint: Mapping[str, object] = after_context
    flags = {name: before_endpoint[key] == after_endpoint[key] for name, key in mapping.items()}
    refs: list[FilePin] = [source_before, source_end, approved, snapshot_pin, before_pin, end_pin, capacity_input_pin, *build_pins, *[FilePin(path=str(p), sha256=gate.sha256(p)) for p in binaries.values()]]
    for row in measurement_rows:
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
    for folder, expected, result, manifest_pin in expected_all:
        expected["sourcePair"] = {"before": source_before, "end": source_end, "approvedSourceRef": approved, "inputPins": [approved, capacity_input_pin], "approvedInventoryDigest": inventory_digest, "snapshot": snapshot_pin}
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
    unique = {ref["path"]: ref for ref in refs}
    # 生成物だけを輸送し、巨大なCargo targetは公開artifactへ混入させない。
    bundle = output / "bundle"
    bundle.mkdir()
    for index, ref in enumerate(unique.values()):
        original = Path(ref["path"])
        relative = f"payload/{index}/{original.name}"
        destination = bundle / relative
        destination.parent.mkdir(parents=True)
        shutil.copyfile(original, destination)
        transport_files.append({"originalPath": ref["path"], "relativePath": relative, "sha256": ref["sha256"]})
    write(bundle / "bundle.json", transport)
    if catalog_manifest_path is None:
        gate._verify_legacy_diagnostic(bundle, root, git_query=git_query)
    elif _allow_catalog_diagnostic:
        gate._verify_catalog_diagnostic(bundle, root, git_query=git_query)
    else:
        gate.verify(bundle, root, git_query=git_query)
    print(json.dumps({"head": head, "bundle": str(bundle), "checks": len(expected_all) * 20, "phaseRecords": len(expected_all) * len(gate.PHASES) * 50}))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, default=Path("tmp/html-performance-producer"))
    parser.add_argument("--capacity-binding", type=Path)
    parser.add_argument("--capacity-binding-sha256")
    parser.add_argument("--catalog-plan", type=Path, required=True)
    args = parser.parse_args()
    produce(args.repo.resolve(), args.output.resolve(), args.capacity_binding, args.capacity_binding_sha256,
            args.catalog_plan.resolve())


if __name__ == "__main__":
    main()
