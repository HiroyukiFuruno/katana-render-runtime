#!/usr/bin/env python3
"""公開対象のclean sourceからbaseline/candidateを実行して3幅のpaired証跡を生成する。"""
from __future__ import annotations

import argparse
import errno
import hashlib
import io
import json
import os
import platform
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
from typing import BinaryIO, Literal, NamedTuple, TypedDict, overload
from collections.abc import Callable, Mapping, Sequence

import verify_html_performance_evidence as gate
from toml_loader import load_toml_parser

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
LOCAL_CAPACITY_BASELINE = 2363047936
LOCAL_CAPACITY_THRESHOLD = 4726095872
PROTECTED_REGISTRY_SCHEMA = "krr-protected-refs-successor-v1"
PROTECTED_REFERENCE_COUNT = 40


class FilePin(TypedDict):
    path: str
    sha256: str


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
    environment: dict[str, str]


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
    gate.require(physical_count == len(records) == PROTECTED_REFERENCE_COUNT,
                 "fixed protected reference count differs")
    references = []
    for record in records:
        validated = protected_identity(record)
        path = Path(gate._text(validated["path"], "protected reference path"))
        references.append(str(path))
    gate.require(len(set(references)) == PROTECTED_REFERENCE_COUNT,
                 "protected reference paths are not unique")
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
    environment_sha = hashlib.sha256(json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
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
    encoded = json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return NativeCliCommandEvidence({"argv": list(args), "stdout": pins[0], "stderr": pins[1],
        "label": label, "cwd": str(cwd), "attemptToken": attempt_token,
        "environment": environment, "environmentSha256": hashlib.sha256(encoded.encode()).hexdigest(),
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
    return {"source": source(root, git_query=git_query), "inputs": [file_state(p) for p in input_paths], "registry": registry, "cache": cache, "cacheUnion": {"registry": registry, "target": cache}, "scope": scope, "config": [file_state(p) for p in (root / ".cargo/config.toml", root / ".cargo/config", cargo_home / "config.toml", cargo_home / "config")], "preference": file_state(Path.home() / ".CFUserTextEncoding"), "toolContext": {"rustc": command(["rustc", "-vV"], text=True, role="tool-query", log_directory=log_directory), "cargo": command(["cargo", "-vV"], text=True, role="tool-query", log_directory=log_directory), "platform": platform.platform(), "cpuCount": os.cpu_count(), "environment": dict(os.environ)}}


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


def produce(root: Path, output: Path, capacity_document: Path | None = None,
            capacity_document_sha256: str | None = None) -> None:
    gate.require(platform.system() == "Darwin" and platform.machine() == "arm64", "measurement lane requires macOS Apple Silicon")
    gate.require(not output.exists() and output.is_relative_to(root / "tmp"), "fresh ignored owned output required")
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"
    # 生成済みJSとsourceの同期が未確認のcandidateをビルド・計測しない。
    lifecycle = output.parent / f"{output.name}.lifecycle"
    gate.require(not lifecycle.exists(), "fresh measurement lifecycle output required")
    git_query = _make_git_query(lifecycle)
    initial = source(root, git_query=git_query)
    _measurement_command(["just", "runtime-bundle-check"], cwd=root, role="preflight", log_directory=lifecycle)
    gate.require(source(root, git_query=git_query) == initial, "source changed during runtime bundle check")
    if capacity_document is None or capacity_document_sha256 is None:
        raise ValueError("capacity binding input required before builds")
    binding = load_capacity_binding(root, capacity_document, capacity_document_sha256)
    output.mkdir(parents=True)
    harness = root / gate.HARNESS_ROOT
    lock = harness / "Cargo.lock"
    gate.require(gate.sha256(lock) == gate.LOCK_SHA, "common lock pin differs")
    fixture = root / "tests/fixtures/html/interaction-fidelity.html"
    dom = root / "tmp/html-interaction-suite/krr.json"
    state = gate.read_json(dom)
    validate_interaction(state, gate.sha256(fixture))
    plan: OperationPlan = {"fixture_sha256": gate.sha256(fixture), "operation": "action-button-toggle", "x": 100.0, "y": 100.0, "expected_dom_state_path": str(dom), "expected_dom_state_sha256": gate.sha256(dom)}
    plan_path = output / "operation-plan.json"
    plan_pin = write(plan_path, plan)
    source_before = write(output / "source-before.json", initial)
    capacity_input_pin = write(output / "capacity-inputs.json", {
        "sourceBeforeRef": source_before,
        "bindingRef": {"path": str(binding.document), "sha256": binding.document_sha256},
        "stateRef": {"path": str(binding.state), "sha256": binding.state_sha256},
        "protectedRegistryRef": {"path": str(binding.registry), "sha256": binding.registry_sha256},
        "protectedReferenceCount": len(binding.references), "target": str(root / "target"),
        "baselineBytes": LOCAL_CAPACITY_BASELINE, "factor": 2, "thresholdBytes": LOCAL_CAPACITY_THRESHOLD,
    })
    approved = write(output / "approved-source.json", initial)
    head = gate._text(initial["head"], "source HEAD")
    inventory_digest = gate._text(initial["inventoryDigest"], "source inventory digest")
    snapshot_rows = [{**row, "path": str(Path(gate._text(row["path"], "source path")).relative_to(root))}
                     for row in gate._objects(initial["files"], "source files")]
    snapshot_pin = write(output / "shared-source-snapshot.json", {"records": snapshot_rows, "digest": hashlib.sha256(encoded(snapshot_rows)).hexdigest()})
    target = root / "target"
    source_records = gate.current_source(root, initial, git_query=git_query)
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
    input_paths = [fixture, plan_path, dom, lock, output / "shared-source-snapshot.json", *binaries.values(),
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
    for width, height in gate.VIEWPORTS:
        folder = output / f"{width}x{height}"
        folder.mkdir()
        viewport: Viewport = {"width": width, "height": height, "scale": 1}
        expected: ExpectedPerformance = {"fixture_sha256": gate.sha256(fixture), "operation_plan_sha256": plan_pin["sha256"], "operation": plan["operation"], "expected_dom_state_sha256": gate.sha256(dom), "viewport": viewport, "provenance": {"rustc": before_context["toolContext"]["rustc"], "cargo": before_context["toolContext"]["cargo"], "environment_sha256": environment_sha}, "arms": arms, "reportPins": [], "operationPlanRef": plan_pin, "domStateRef": {"path": str(dom), "sha256": gate.sha256(dom)}}
        order: list[gate.JsonObject] = []
        reports: list[Path] = []
        for pair in range(5):
            for arm in (("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")):
                binary = binaries[arm]
                report_path = folder / "reports" / f"{arm}-{pair}.json"
                raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
                environment = {**os.environ, "KRR_PERF_RUSTC": expected["provenance"]["rustc"], "KRR_PERF_CARGO": expected["provenance"]["cargo"], "KRR_PERF_ENVIRONMENT_SHA256": environment_sha, "KRR_PERF_LOCK_PATH": str(lock), "KRR_PERF_SOURCE_SNAPSHOT_PATH": snapshot_pin["path"]}
                samples: list[gate.JsonObject] = []
                loads: list[dict[str, float]] = []
                for sample in range(5):
                    load_before = await_quiet()
                    require_frozen_binary(builds[arm])
                    stdout = command([str(binary), "--worker", str(sample), str(fixture), str(report_path), str(plan_path), str(width), str(height), "1"], cwd=root, env=environment, role="worker", log_directory=lifecycle)
                    load_after = quiet()
                    gate.require(load_after <= 0.25 and gate.sha256(binary) == arms[raw_arm]["binary_sha256"], "worker load/binary changed")
                    payload = worker_sample(stdout, sample, viewport)
                    samples.append(payload)
                    loads.append({"before": load_before, "after": load_after})
                report: gate.JsonObject = {"arm": raw_arm, "pair_index": pair, "measurement_executed": True, "sample_count": 5, "source_sha256": expected["fixture_sha256"], "operation_plan_sha256": expected["operation_plan_sha256"], "operation": plan["operation"], "expected_dom_state_sha256": expected["expected_dom_state_sha256"], "viewport": viewport, "provenance": {**expected["provenance"], "binary_sha256": arms[raw_arm]["binary_sha256"], "source_snapshot_sha256": snapshot_pin["sha256"], "deps_lock_sha256": gate.LOCK_SHA}, "samples": samples}
                report_pin = write(report_path, report)
                expected["reportPins"].append(report_pin)
                reports.append(report_path)
                order.append({"arm": arm, "pair_index": pair, "executable": str(binary), "executable_sha256": arms[raw_arm]["binary_sha256"], "output": str(report_path), "load_ratio_before": max(x["before"] for x in loads), "load_ratio_after": max(x["after"] for x in loads), "workerLoads": loads})
        manifest_pin = write(folder / "reports/pair-manifest.json", {"measurement_executed": True, "input_sha256": expected["fixture_sha256"], "environment_sha256": environment_sha, "platform": platform.platform(), "order": order})
        aggregate = gate.aggregate_comparison(reports, {**expected})
        policy = {p: {k: v <= 0 for k, v in values.items()} for p, values in aggregate["comparison"].items()}
        result: gate.JsonObject = {"formalAccepted": False, "performanceAccepted": False, "measurementExecuted": True, "policy": {"policyPass": all(ok for phase in policy.values() for ok in phase.values()), "policy": policy, "deltaThreshold": 0, "quietLoadLimit": 0.25, "phaseRecords": 250, "aggregate": aggregate}}
        expected_all.append((folder, expected, result, manifest_pin))
    after_context = context(root, target, input_paths, lifecycle, git_query=git_query)
    end_pin = write(output / "context-end.json", after_context)
    source_end = write(output / "source-end.json", after_context["source"])
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    before_endpoint: Mapping[str, object] = before_context
    after_endpoint: Mapping[str, object] = after_context
    flags = {name: before_endpoint[key] == after_endpoint[key] for name, key in mapping.items()}
    refs: list[FilePin] = [source_before, source_end, approved, snapshot_pin, before_pin, end_pin, plan_pin, capacity_input_pin, *build_pins, {"path": str(dom), "sha256": gate.sha256(dom)}, *[FilePin(path=str(p), sha256=gate.sha256(p)) for p in binaries.values()]]
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
    gate.verify(bundle, root, git_query=git_query)
    print(json.dumps({"head": head, "bundle": str(bundle), "checks": 60, "phaseRecords": 750}))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, default=Path("tmp/html-performance-producer"))
    parser.add_argument("--capacity-binding", type=Path)
    parser.add_argument("--capacity-binding-sha256")
    args = parser.parse_args()
    produce(args.repo.resolve(), args.output.resolve(), args.capacity_binding, args.capacity_binding_sha256)


if __name__ == "__main__":
    main()
