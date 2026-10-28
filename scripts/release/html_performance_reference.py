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


class FilePin(NamedTuple):
    logical_path: str
    sha256: str


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


def reference_parent_contract(proof: ReferenceParentProof) -> JsonObject:
    """Return the producer-facing contract while preserving original arm identity."""
    return {
        "role": proof.role,
        "armId": proof.role,
        "referenceKind": REFERENCE_KIND,
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
