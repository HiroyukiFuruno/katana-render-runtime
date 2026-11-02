"""HTML性能比較基準の版・証跡identityを検証する型付きdomain loader。"""
from __future__ import annotations

import hashlib
import json
from collections.abc import Callable
from pathlib import Path
from typing import NamedTuple

from html_performance_harness_dependency_closure import verify_harness_closure

JsonObject = dict[str, object]
GitQuery = Callable[[Path, tuple[str, ...]], str]
FileResolver = Callable[[object, str], Path]

REFERENCE_ROLE = "reference-parent-371274ed"
REFERENCE_KIND = "correct-parent-source"
PARENT_HEAD = "371274ed712e088fcac665ce45a1a71ceddcaa01"
PARENT_SOURCE_SNAPSHOT_SHA256 = "416c20fe067c5a50c0673071bc4f20bd92b4efc4aea8d5c815d8a3cae3d40bed"
PARENT_BINARY_SHA256 = "2f268de6f1d1fdfa2e280d23b05dc9238f4dc7bded66b349523c758ecfccfdb9"
PARENT_LOCK_SHA256 = "94c1a2cc5959e5687ace14ef02ad516dfe1797e581704c7b9b37df9105e83ed2"
PARENT_BUILD_ARTIFACT_SHA256 = "85af32ad7964531a77a0623f0757f6de067b110c8afcee0a36029d8c42eac056"
PARENT_CARGO_SHA256 = "7672ead309d505577c018fff2cafb3433601f073e38cbe87359ac1f7b944bbf5"
PARENT_RUSTC_SHA256 = "210df6794001b73ec3d453878707fa1e0bdcb63c427024a6e6574bbe5615a4da"
PARENT_CONTEXT_BEFORE_SHA256 = "e7fbb4ff960eb75b1b7b6d4a61c08b61d18f6a07e69102b457d103d1d24bc268"
HISTORY_PATH = "docs/performance/html-performance-history.json"
REFERENCE_BUILD_FIELDS = frozenset({
    "buildArtifactSha256", "sourceSnapshotSha256", "binarySha256", "lockSha256",
    "cargoExecutableSha256", "rustcExecutableSha256", "contextBeforeSha256",
})


class FilePin(NamedTuple):
    logical_path: str
    sha256: str


class ActiveReference(NamedTuple):
    record_id: str
    source_head: str
    bundle_manifest_sha256: str
    verifier_stdout_sha256: str
    build_pins: dict[str, str]


def active_reference_at_commit(root: Path, revision: str, git_query: GitQuery, *,
                               allow_legacy_missing: bool = False) -> ActiveReference | None:
    """Read the selection from the measured commit, never from the working tree."""
    _require(len(revision) == 40 and all(c in "0123456789abcdef" for c in revision),
             "reference selection revision is invalid")
    found = git_query(root, ("ls-tree", "--name-only", revision, "--", HISTORY_PATH)).splitlines()
    if not found:
        _require(allow_legacy_missing, "reference history is absent at selected commit")
        return None
    _require(found == [HISTORY_PATH], "reference history Git tree differs")
    history = json.loads(git_query(root, ("show", f"{revision}:{HISTORY_PATH}")),
                         object_pairs_hook=_unique_object)
    if not isinstance(history, dict) or history.get("schema") != "krr-html-performance-history-v1":
        raise ValueError("unsupported reference history")
    active = _text(history.get("activeReferenceRecordId"), "active reference id")
    records = history.get("records")
    if not isinstance(records, list) or not records or any(not isinstance(row, dict) for row in records):
        raise ValueError("reference history records are invalid")
    ids = [_text(row.get("id"), "reference record id") for row in records]
    _require(len(ids) == len(set(ids)) and ids.count(active) == 1, "reference record identity differs")
    record = records[ids.index(active)]
    return _active_record(record, active)


def _active_record(record: dict[str, object], active: str) -> ActiveReference:
    source_head = _text(record.get("sourceCommit"), "reference source commit")
    _require(len(source_head) == 40 and all(c in "0123456789abcdef" for c in source_head),
             "reference source commit is invalid")
    acceptance = record.get("measuredAcceptance")
    scope = record.get("scope")
    evidence = record.get("evidence")
    pins = record.get("referenceBuild")
    if not all(isinstance(item, dict) for item in (acceptance, scope, evidence, pins)):
        raise ValueError("reference record admission fields are missing")
    assert isinstance(acceptance, dict) and isinstance(scope, dict)
    assert isinstance(evidence, dict) and isinstance(pins, dict)
    score = acceptance.get("htmlMinimumScore")
    threshold = acceptance.get("htmlScoreThreshold")
    phases = record.get("candidatePhaseStatistics")
    phase_names = {"cold_open", "warm_open", "re_display", "full_y", "post_operation", "same_session_resize"}
    if isinstance(score, bool) or not isinstance(score, (int, float)):
        raise ValueError("reference score is invalid")
    if isinstance(threshold, bool) or not isinstance(threshold, (int, float)):
        raise ValueError("reference threshold is invalid")
    _require(type(acceptance.get("verifierExitCode")) is int
             and acceptance.get("verifierExitCode") == 0
             and acceptance.get("performanceNonregression") is True
             and acceptance.get("outputFontIdentityEquivalence") == "established"
             and type(scope.get("candidateConditions")) is int
             and scope.get("candidateConditions") == 84
             and 100.0 >= float(score) >= float(threshold) >= 90.0,
             "reference record is not an accepted complete candidate")
    _require(isinstance(phases, dict) and set(phases) == phase_names,
             "reference phase inventory differs")
    assert isinstance(phases, dict)
    for phase in phases.values():
        if not isinstance(phase, dict):
            raise ValueError("reference phase is invalid")
        duration = phase.get("maxSampleSeconds")
        _require(not isinstance(duration, bool) and isinstance(duration, (int, float))
                 and 0 <= duration < 3.0, "reference phase exceeds the performance limit")
    _require(pins.get("schema") == "krr-html-performance-reference-build-pins-v1"
             and pins.get("buildArtifactSchema") == "krr-html-performance-build-artifact-v3"
             and set(pins) == REFERENCE_BUILD_FIELDS | {"schema", "buildArtifactSchema"},
             "reference build pin schema differs")
    return ActiveReference(active, source_head, _sha(record.get("bundleManifestSha256"), "bundle manifest"),
                           _sha(evidence.get("verifierStdoutSha256"), "verifier stdout"),
                           {field: _sha(pins[field], field) for field in REFERENCE_BUILD_FIELDS})


class ReferenceParentProof(NamedTuple):
    role: str
    original_arm: str
    head: str
    source_snapshot_sha256: str
    binary_sha256: str
    lock_sha256: str
    cargo_sha256: str
    original_build_artifact_sha256: str
    wrapper_sha256: str
    dependency_refs: tuple[FilePin, ...]
    build_artifact_path: Path
    frozen_binary_path: Path
    reference_kind: str = REFERENCE_KIND
    record_id: str | None = None
    transport_sources: tuple[tuple[str, Path], ...] = ()


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(label + " must be non-empty text")
    return value


def _sha(value: object, label: str) -> str:
    result = _text(value, label)
    _require(len(result) == 64 and all(c in "0123456789abcdef" for c in result),
             label + " must be a lowercase SHA256")
    return result


def _file_ref(value: object, label: str) -> tuple[Path, str]:
    if not isinstance(value, dict):
        raise ValueError(label + " must be an object")
    path = Path(_text(value.get("path"), label + " path"))
    checksum = _sha(value.get("sha256"), label + " SHA256")
    _require(path.is_absolute() and path.resolve() == path and path.is_file() and not path.is_symlink(),
             label + " must be a canonical regular file")
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    _require(actual == checksum, label + " bytes differ")
    return path, checksum


def load_reference_parent_proof(wrapper_path: Path, root: Path, *,
                                git_query: GitQuery | None = None,
                                file_resolver: FileResolver | None = None) -> ReferenceParentProof:
    """Load the immutable parent reference wrapper without treating it as a public SDK."""
    wrapper = json.loads(wrapper_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    if not isinstance(wrapper, dict):
        raise ValueError("reference parent wrapper must be an object")
    wrapper_sha256 = hashlib.sha256(wrapper_path.read_bytes()).hexdigest()
    dependency_refs: list[FilePin] = []
    def file_ref(value: object, label: str) -> tuple[Path, str]:
        logical_path = _text(value.get("path") if isinstance(value, dict) else None, label + " path")
        checksum = _sha(value.get("sha256") if isinstance(value, dict) else None, label + " SHA256")
        path = file_resolver(value, label) if file_resolver is not None else _file_ref(value, label)[0]
        _require(path.is_file() and not path.is_symlink()
                 and hashlib.sha256(path.read_bytes()).hexdigest() == checksum,
                 label + " bytes differ")
        dependency_refs.append(FilePin(logical_path, checksum))
        return path, checksum
    _require(wrapper.get("schema") == "krr-html-performance-reference-parent-v1",
             "unsupported reference parent wrapper schema")
    _require(wrapper.get("role") == REFERENCE_ROLE
             and wrapper.get("referenceKind") == REFERENCE_KIND
             and wrapper.get("publishedSdk") is False,
             "reference parent wrapper identity differs")
    _require(wrapper.get("originalArm") == "candidate", "original reference arm must remain candidate")
    _require(_text(wrapper.get("parentHead"), "parent HEAD") == PARENT_HEAD,
             "reference parent HEAD differs")
    source_snapshot = file_ref(wrapper.get("sourceSnapshot"), "source snapshot")
    _require(source_snapshot[1] == PARENT_SOURCE_SNAPSHOT_SHA256,
             "reference parent source snapshot differs")
    binary = file_ref(wrapper.get("frozenBinary"), "frozen binary")
    _require(binary[1] == PARENT_BINARY_SHA256, "reference parent binary differs")
    _require(_sha(wrapper.get("lockSha256"), "lock SHA256") == PARENT_LOCK_SHA256,
             "reference parent lock differs")
    build_path, build_sha = file_ref(wrapper.get("originalBuildArtifact"), "original build artifact")
    build = json.loads(build_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    if not (isinstance(build, dict) and build.get("schema") == "krr-html-performance-build-artifact-v2"
            and build.get("arm") == "candidate" and build.get("buildExit") == 0):
        raise ValueError("original build proof is not the retained candidate proof")
    selected_binary = build.get("selectedBinary")
    frozen_binary = build.get("frozenBinary")
    snapshot_ref = build.get("sourceSnapshotRef")
    _require(isinstance(selected_binary, dict) and isinstance(frozen_binary, dict)
             and isinstance(snapshot_ref, dict)
             and selected_binary.get("sha256") == PARENT_BINARY_SHA256
             and frozen_binary.get("sha256") == PARENT_BINARY_SHA256
             and snapshot_ref.get("sha256") == PARENT_SOURCE_SNAPSHOT_SHA256
             and build.get("lockSha256") == PARENT_LOCK_SHA256,
             "original build proof pins differ")
    _require(snapshot_ref == wrapper.get("sourceSnapshot"),
             "original build source snapshot reference differs")
    source_before_ref = build.get("sourceBeforeRef")
    _require(isinstance(source_before_ref, dict), "original build source-before reference is missing")
    source_before_path, _ = file_ref(source_before_ref, "source-before")
    source_before = json.loads(source_before_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    _require(isinstance(source_before, dict) and source_before.get("head") == PARENT_HEAD,
             "original build source HEAD differs")
    snapshot = json.loads(source_snapshot[0].read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    records = snapshot.get("records") if isinstance(snapshot, dict) else None
    if not isinstance(records, list):
        raise ValueError("original source snapshot records are missing")
    harness_records: dict[str, dict[str, object]] = {}
    for record in records:
        if not isinstance(record, dict):
            raise ValueError("original source snapshot record is invalid")
        record_path = _text(record.get("path"), "source snapshot path")
        if record_path.startswith("scripts/html/performance-harness/"):
            if record_path in harness_records:
                raise ValueError("original harness source snapshot has duplicate paths")
            harness_records[record_path] = record
    harness_lock_path = "scripts/html/performance-harness/Cargo.lock"
    _require(len(harness_records) == 34 and harness_lock_path in harness_records,
             "original harness source snapshot inventory differs")
    if git_query is None:
        raise ValueError("parent commit query is required")
    for revision in (PARENT_HEAD, "HEAD"):
        tracked = git_query(root, ("ls-tree", "-r", "--name-only", revision,
                                   "--", "scripts/html/performance-harness/"))
        paths = [path for path in tracked.splitlines() if path]
        _require(len(paths) == len(set(paths)) and set(paths) == set(harness_records),
                 "HTML harness source inventory differs from pinned parent")
    for record_path, record in harness_records.items():
        record_sha = _sha(record.get("sha256"), "source snapshot SHA256")
        record_size = record.get("size")
        if isinstance(record_size, bool) or not isinstance(record_size, int) or record_size < 0:
            raise ValueError("source snapshot size is invalid")
        if record_path == harness_lock_path:
            _require(record_sha == PARENT_LOCK_SHA256 and record_size > 0,
                     "original harness lock snapshot pin differs")
            continue
        path = root / record_path
        _require(path.is_file() and not path.is_symlink()
                 and hashlib.sha256(path.read_bytes()).hexdigest() == record_sha
                 and path.stat().st_size == record_size,
                 "current harness source differs from parent snapshot")
    _require(build_sha == PARENT_BUILD_ARTIFACT_SHA256
             and _sha(wrapper.get("originalBuildArtifactSha256"), "original build artifact SHA")
             == PARENT_BUILD_ARTIFACT_SHA256,
             "original build artifact wrapper pin differs")
    _cargo_path, cargo_sha = file_ref(build.get("cargoExecutable"), "original Cargo executable")
    _require(cargo_sha == PARENT_CARGO_SHA256, "original Cargo executable differs")
    _rustc_path, rustc_sha = file_ref(build.get("rustcExecutable"), "original Rustc executable")
    _require(rustc_sha == PARENT_RUSTC_SHA256, "original Rustc executable differs")
    config_pins = build.get("cargoConfigPins")
    if not isinstance(config_pins, list):
        raise ValueError("original Cargo config pins are missing")
    for pin in config_pins:
        file_ref(pin, "original Cargo config pin")
    raw_pins = build.get("rawPins")
    if not isinstance(raw_pins, list) or not raw_pins:
        raise ValueError("original raw build pins are missing")
    for pin in raw_pins:
        file_ref(pin, "original raw build pin")
    argv = build.get("argv")
    if not isinstance(argv, list) or "--locked" not in argv:
        raise ValueError("original build argv is incomplete")
    runtime = build.get("expectedRuntime")
    harness = build.get("expectedHarness")
    if not isinstance(runtime, dict) or not isinstance(harness, dict):
        raise ValueError("original Cargo package closure is missing")
    _require(runtime.get("name") == "katana-render-runtime"
             and runtime.get("version") == "0.4.24"
             and runtime.get("source") is None
             and str(runtime.get("manifest_path", "")).endswith("crates/katana-render-runtime/Cargo.toml"),
             "original runtime package identity differs")
    _require(harness.get("name") == "krr-html-performance-harness"
             and str(harness.get("manifest_path", "")).endswith("scripts/html/performance-harness/Cargo.toml"),
             "original harness package identity differs")
    _context_path, context_sha = file_ref(wrapper.get("parentContextBefore"), "parent context-before")
    _require(context_sha == PARENT_CONTEXT_BEFORE_SHA256, "parent context-before differs")
    context = json.loads(_context_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    inputs = context.get("inputs") if isinstance(context, dict) else None
    if not isinstance(inputs, list):
        raise ValueError("parent context inputs are missing")
    for input_ref in inputs:
        if not isinstance(input_ref, dict):
            raise ValueError("parent context input is invalid")
        path_text = _text(input_ref.get("path"), "parent context input path")
        runtime_input = ("/tests/fixtures/html/" in path_text
                         or path_text.startswith("/System/Library/Fonts/")
                         or "/resource-bodies/" in path_text
                         or "/runtime-bundles/" in path_text
                         or "/bundled-assets/" in path_text)
        historical = ("/target/" in path_text or "/tmp/" in path_text) and not runtime_input
        _require(historical or runtime_input or path_text.endswith("scripts/html/performance-harness/Cargo.lock"),
                 "parent context input classification is unknown")
        if path_text.endswith("scripts/html/performance-harness/Cargo.lock"):
            input_sha = _sha(input_ref.get("sha256"), "parent context Cargo.lock SHA256")
            _require(input_sha == PARENT_LOCK_SHA256, "parent context Cargo.lock pin differs")
        elif runtime_input:
            file_ref(input_ref, "parent runtime input")
    resolved = git_query(root, ("rev-parse", "--verify", PARENT_HEAD + "^{commit}")).strip()
    _require(resolved == PARENT_HEAD, "reference parent commit is unavailable")
    parent_lock_text = git_query(root, ("show", PARENT_HEAD + ":" + harness_lock_path))
    parent_lock = parent_lock_text.encode("utf8")
    _require(harness_records[harness_lock_path]["size"] == len(parent_lock),
             "original harness lock snapshot size differs")
    current_lock_path = root / harness_lock_path
    _require(current_lock_path.is_file() and not current_lock_path.is_symlink(),
             "current harness Cargo.lock is not a regular file")
    current_lock = current_lock_path.read_bytes()
    verify_harness_closure(parent_lock, current_lock, PARENT_LOCK_SHA256)
    return ReferenceParentProof(REFERENCE_ROLE, "candidate", PARENT_HEAD, source_snapshot[1],
                                binary[1], PARENT_LOCK_SHA256, cargo_sha, build_sha, wrapper_sha256,
                                tuple(dependency_refs), build_path, binary[0])


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result = dict(pairs)
    if len(result) != len(pairs):
        raise ValueError("duplicate JSON keys")
    return result


def load_history_reference_proof(
    wrapper_path: Path, selection: ActiveReference, *, file_resolver: FileResolver,
    validate_build: Callable[[object, ActiveReference], dict[str, object]],
) -> ReferenceParentProof:
    """Bind a selected historical candidate to its original validated v3 build."""
    wrapper = json.loads(wrapper_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    if not isinstance(wrapper, dict):
        raise ValueError("history reference wrapper must be an object")
    role = "reference-history-" + selection.record_id
    _require(wrapper.get("schema") == "krr-html-performance-reference-history-v1"
             and wrapper.get("recordId") == selection.record_id
             and wrapper.get("role") == role
             and wrapper.get("referenceKind") == "accepted-history-candidate"
             and wrapper.get("originalArm") == "candidate"
             and wrapper.get("publishedSdk") is False
             and wrapper.get("parentHead") == selection.source_head,
             "history reference wrapper identity differs")
    refs: list[FilePin] = []
    def checked_ref(value: object, field: str, expected_sha: str) -> tuple[Path, str]:
        if not isinstance(value, dict):
            raise ValueError(field + " reference is missing")
        path = _text(value.get("path"), field + " path")
        checksum = _sha(value.get("sha256"), field + " SHA")
        _require(checksum == expected_sha, field + " pin differs")
        physical = file_resolver(value, field)
        _require(physical.is_file() and not physical.is_symlink()
                 and hashlib.sha256(physical.read_bytes()).hexdigest() == checksum,
                 field + " bytes differ")
        refs.append(FilePin(path, checksum))
        return physical, checksum
    manifest_path, _ = checked_ref(wrapper.get("sourceBundleManifest"), "source bundle manifest",
                                   selection.bundle_manifest_sha256)
    manifest = json.loads(manifest_path.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    _require(isinstance(manifest, dict) and manifest.get("schema") == "krr-html-performance-release-bundle-v1"
             and manifest.get("head") == selection.source_head, "reference source bundle identity differs")
    checked_ref(wrapper.get("verifierStdout"), "verifier stdout", selection.verifier_stdout_sha256)
    pins = selection.build_pins
    build_ref = wrapper.get("originalBuildArtifact")
    build, build_sha = checked_ref(build_ref, "original build", pins["buildArtifactSha256"])
    snapshot, snapshot_sha = checked_ref(wrapper.get("sourceSnapshot"), "source snapshot",
                                          pins["sourceSnapshotSha256"])
    binary_ref = wrapper.get("frozenBinary")
    if not isinstance(binary_ref, dict):
        raise ValueError("frozen binary reference is missing")
    binary, binary_sha = checked_ref(binary_ref, "frozen binary", pins["binarySha256"])
    context, _ = checked_ref(wrapper.get("parentContextBefore"), "context before", pins["contextBeforeSha256"])
    context_data = json.loads(context.read_text(encoding="utf8"), object_pairs_hook=_unique_object)
    if not isinstance(context_data, dict) or not isinstance(context_data.get("inputs"), list):
        raise ValueError("reference context input inventory differs")
    binary_rows = [row for row in context_data["inputs"]
                   if isinstance(row, dict) and row.get("path") == binary_ref.get("path")]
    _require(len(binary_rows) == 1 and binary_rows[0].get("sha256") == binary_sha,
             "reference context does not bind the frozen binary")
    validated = validate_build(build_ref, selection)
    for name, value in (("head", selection.source_head), ("binarySha256", binary_sha),
                        ("sourceSnapshotSha256", snapshot_sha), ("buildArtifactSha256", build_sha),
                        ("lockSha256", pins["lockSha256"]),
                        ("cargoExecutableSha256", pins["cargoExecutableSha256"]),
                        ("rustcExecutableSha256", pins["rustcExecutableSha256"])):
        _require(validated.get(name) == value, "reference validated build differs: " + name)
    del snapshot
    return ReferenceParentProof(role, "candidate", selection.source_head, snapshot_sha, binary_sha,
                                pins["lockSha256"], pins["cargoExecutableSha256"], build_sha,
                                hashlib.sha256(wrapper_path.read_bytes()).hexdigest(), tuple(refs),
                                build, binary, "accepted-history-candidate", selection.record_id)


def reference_parent_contract(proof: ReferenceParentProof) -> JsonObject:
    """Return the producer-facing contract while preserving original arm identity."""
    contract: JsonObject = {
        "role": proof.role,
        "armId": proof.role,
        "referenceKind": proof.reference_kind,
        "publishedSdk": False,
        "originalArm": proof.original_arm,
        "parentHead": proof.head,
        "sourceSnapshotSha256": proof.source_snapshot_sha256,
        "binarySha256": proof.binary_sha256,
        "lockSha256": proof.lock_sha256,
        "cargoExecutableSha256": proof.cargo_sha256,
        "originalBuildArtifactSha256": proof.original_build_artifact_sha256,
        "dependencyRefs": [{"path": pin.logical_path, "sha256": pin.sha256}
                           for pin in proof.dependency_refs],
    }
    if proof.record_id is not None:
        contract["recordId"] = proof.record_id
    return contract
