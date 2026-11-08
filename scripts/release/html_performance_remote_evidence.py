"""Strict validation of authenticated remote performance transport."""
from __future__ import annotations

import hashlib
import json
import os
import zipfile
from pathlib import Path, PurePosixPath
from typing import NamedTuple

import html_performance_reference as reference_domain
from verify_html_performance_evidence import (
    Bundle, ExpectedBuild, FileReader, GitQuery, HARNESS_ROOT, HistoricalBuildReader, JsonObject, LOCK_SHA, _array,
    _canonical_build_path, _historical_reference_source, _integer, _object, _objects, _recorded_capacity_state,
    _require_formal_release_result, _text, cargo_build_artifacts, current_source,
    committed_records, digest, git, read_json, records_map, require, selected_historical_lock, sha256, unique_object,
    validate_guarded_build_argv,
)


def extract_remote_transport(directory: Path, archive: Path, expected_head: str) -> None:
    """Verify only the acquired transport envelope, without asserting performance acceptance."""
    with zipfile.ZipFile(archive) as zipped:
        seen: set[Path] = set()
        names: set[str] = set()
        for entry in zipped.infolist():
            relative = PurePosixPath(entry.filename)
            path = (directory / entry.filename).resolve()
            require(not relative.is_absolute() and all(part not in {".", ".."} for part in relative.parts)
                    and path.is_relative_to(directory.resolve()) and path not in seen
                    and path != archive.resolve() and path != (directory / "acquisition.json").resolve()
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000,
                    "unsafe/duplicate archive path")
            seen.add(path)
            if not entry.is_dir():
                names.add(relative.as_posix())
        zipped.extractall(directory)
    bundle = Bundle(directory)
    require(bundle.manifest.get("head") == expected_head, "remote bundle HEAD differs")
    allowed = {"bundle.json", *bundle.relative_files}
    if "producer-verification.json" in names:
        allowed.add("producer-verification.json")
    require(names == allowed, "remote bundle archive inventory differs")


def validate_producer_verification_receipt(directory: Path, expected_head: str) -> JsonObject:
    """Check the producer result binding; authentication and remote recomputation are separate."""
    bundle = Bundle(directory)
    path = directory / "producer-verification.json"
    require(path.is_file() and not path.is_symlink(), "producer verification receipt is missing")
    receipt = read_json(path)
    require(set(receipt) == {"schema", "role", "head", "bundleManifestSha256", "result"}
            and receipt.get("schema") == "krr-html-performance-producer-verification-v1"
            and receipt.get("role") == "producer-formal-verification"
            and receipt.get("head") == expected_head == bundle.manifest.get("head")
            and receipt.get("bundleManifestSha256") == sha256(directory / "bundle.json"),
            "producer verification receipt identity differs")
    result = _object(receipt.get("result"), "producer verification result")
    require(result.get("head") == expected_head and result.get("conditions") == 84
            and result.get("candidateConditions") == 84
            and result.get("checks") == 1680
            and len(_objects(bundle.manifest.get("attempts"), "viewport attempts")) == 84,
            "producer verification result scope differs")
    _require_formal_release_result(result)
    return result


def validate_remote_candidate_build(bundle: FileReader, root: Path, reference: object, head: str, *,
                                    historical: reference_domain.ActiveReference | None = None) -> JsonObject:
    """Inspect transported candidate build bytes; callers must independently authenticate the artifact."""
    proof_path = bundle.file(reference)
    proof = read_json(proof_path)
    require(proof.get("schema") == "krr-html-performance-build-artifact-v3"
            and proof.get("arm") == "candidate" and proof.get("buildExit") == 0,
            "remote candidate build role differs")
    before = bundle.json(proof.get("sourceBeforeRef"))
    require(before.get("head") == head, "remote candidate source HEAD differs")
    records = (current_source(root, before) if historical is None else
               _historical_reference_source(root, before, historical, git_query=None))
    logical_root = _canonical_build_path(before.get("root"), "producer source root")
    controller_root = logical_root
    if proof.get("referenceRole") == "fresh-history-source":
        from html_performance_source_materialization import verify_transported_source_proof
        if historical is None:
            raise ValueError("fresh remote build requires the selected reference")
        controller_ref = _object(proof.get("controllerSourceBeforeRef"), "remote controller source")
        controller_pin = {"path": _text(controller_ref.get("path"), "controller source path"),
                          "sha256": _text(controller_ref.get("sha256"), "controller source SHA")}
        controller_before = bundle.json(controller_ref)
        current_source(root, controller_before)
        controller_root = _canonical_build_path(controller_before.get("root"), "controller source root")
        selected_proof = bundle.json(proof.get("selectedSourceProofRef"))
        require(verify_transported_source_proof(root, historical, controller_pin, selected_proof,
                                               controller_root, file_resolver=bundle.file) == records
                and selected_proof.get("materializedRoot") == str(logical_root),
                "remote fresh source proof differs")
    else:
        require(proof.get("referenceRole") is None, "unknown remote build role")
    require(sha256(root / HARNESS_ROOT / "Cargo.lock") == LOCK_SHA, "current lock pin differs")
    lock_sha = LOCK_SHA if historical is None else selected_historical_lock(root, historical)[1]
    return _validate_transported_candidate_body(
        bundle, proof_path, proof, head, records, logical_root, controller_root, lock_sha,
    )


def _validate_transported_candidate_body(
    bundle: FileReader, proof_path: Path, proof: JsonObject, head: str,
    records: dict[str, tuple[str, int]], logical_root: Path, controller_root: Path, lock_sha: str,
) -> JsonObject:
    snapshot_ref = _object(proof.get("sourceSnapshotRef"), "remote source snapshot")
    snapshot = bundle.json(snapshot_ref)
    rows = _array(snapshot.get("records"), "remote source records")
    require(records_map(rows) == records
            and snapshot.get("digest") == hashlib.sha256(json.dumps(
                rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "remote candidate source snapshot differs")
    require(proof.get("lockSha256") == lock_sha
            and records.get(HARNESS_ROOT + "Cargo.lock", (None,))[0] == lock_sha,
            "remote candidate lock differs")
    logical_harness = logical_root / HARNESS_ROOT.rstrip("/")
    for name in ("cargo", "rustc"):
        launcher = _object(proof.get(name + "Launcher"), "remote tool launcher")
        executable = _object(proof.get(name + "Executable"), "remote tool executable")
        _canonical_build_path(launcher.get("path"), "remote tool launcher")
        dispatcher = {"path": launcher.get("resolvedPath"), "sha256": launcher.get("sha256")}
        bundle.file(dispatcher)
        bundle.file(executable)
        require(bool(_array(launcher.get("resolutionArgv"), "remote tool resolution argv"))
                or launcher.get("path") == launcher.get("resolvedPath"), "remote tool resolution differs")
    digest(proof.get("buildEnvironmentSha256"))
    versions = _object(proof.get("toolVersions"), "remote tool versions")
    require(all(_text(versions.get(name), "remote tool version").startswith(name + " ")
                for name in ("cargo", "rustc")), "remote tool versions differ")
    for config in _objects(proof.get("cargoConfigPins"), "remote Cargo configuration pins"):
        bundle.file(config)
    raw_pins = _objects(proof.get("rawPins"), "remote raw build pins")
    raw = {Path(_text(pin.get("path"), "remote raw path")).name: bundle.file(pin) for pin in raw_pins}
    require(len(raw) == len(raw_pins) and {"metadata.json", "cargo.jsonl", "guard-cargo.stderr"} <= set(raw),
            "remote build raw inputs differ")
    metadata = _object(json.loads(raw["metadata.json"].read_text(), object_pairs_hook=unique_object),
                       "remote Cargo metadata")
    packages = _objects(metadata.get("packages"), "remote Cargo packages")
    harness = _object(proof.get("expectedHarness"), "remote expected harness")
    runtime = dict(_object(proof.get("expectedRuntime"), "remote expected runtime"))
    features = runtime.pop("resolvedFeatures", None)
    require(metadata.get("version") == 1 and metadata.get("workspace_root") == str(logical_harness)
            and metadata.get("target_directory") == str(controller_root / "target")
            and sum(package == harness for package in packages) == 1
            and sum(package == runtime for package in packages) == 1
            and harness.get("name") == "krr-html-performance-harness"
            and harness.get("manifest_path") == str(logical_harness / "Cargo.toml")
            and harness.get("source") is None
            and runtime.get("name") == "katana-render-runtime"
            and runtime.get("manifest_path") == str(logical_root / "crates/katana-render-runtime/Cargo.toml")
            and runtime.get("source") is None, "remote Cargo package identity differs")
    harness_id = _text(harness.get("id"), "remote harness package ID")
    runtime_id = _text(runtime.get("id"), "remote runtime package ID")
    resolve = _object(metadata.get("resolve"), "remote Cargo resolution")
    nodes = _objects(resolve.get("nodes"), "remote Cargo nodes")
    roots = [node for node in nodes if node.get("id") == harness_id]
    runtime_nodes = [node for node in nodes if node.get("id") == runtime_id]
    require(metadata.get("workspace_members") == [harness_id] and resolve.get("root") == harness_id
            and len(roots) == len(runtime_nodes) == 1 and roots[0].get("features") == ["candidate"]
            and runtime_nodes[0].get("features") == features,
            "remote Cargo feature resolution differs")
    dependencies = _objects(roots[0].get("deps"), "remote harness dependencies")
    arms = [item for item in dependencies if item.get("name") in {"baseline", "candidate"}]
    require(len(arms) == 1 and arms[0].get("name") == "candidate" and arms[0].get("pkg") == runtime_id,
            "remote Cargo runtime selection differs")
    resolved_runtime = dict(runtime)
    resolved_runtime["resolvedFeatures"] = features
    binary, artifacts = cargo_build_artifacts(raw["cargo.jsonl"].read_bytes(),
                                               ExpectedBuild(harness, resolved_runtime, "candidate"),
                                               controller_root / "target")
    require(artifacts == _object(proof.get("artifacts"), "remote build artifacts"),
            "remote Cargo artifacts differ")
    selected = _object(proof.get("selectedBinary"), "remote selected binary")
    frozen = _object(proof.get("frozenBinary"), "remote frozen binary")
    require(_canonical_build_path(selected.get("path"), "remote selected binary") == binary
            and selected.get("sha256") == frozen.get("sha256"), "remote selected binary differs")
    bundle.file(frozen)
    python = _object(proof.get("guardPython"), "remote guard Python")
    bundle.file(python.get("executable"))
    argv = [_text(item, "remote build argv") for item in _array(proof.get("argv"), "remote build argv")]
    lifecycle = bundle.json(proof.get("guardLifecycleRef"))
    identity = _object(lifecycle.get("identity"), "remote guard identity")
    pid = _integer(identity.get("pid"), "remote guard pid")
    require(pid > 0 and identity.get("sessionId") == pid and identity.get("processGroupId") == pid
            and lifecycle.get("schema") == "krr-performance-command-lifecycle-v1"
            and lifecycle.get("role") == "build" and lifecycle.get("cwd") == str(controller_root)
            and lifecycle.get("argv") == argv and lifecycle.get("exitCode") == 0
            and lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
            and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None,
            "remote guard lifecycle differs")
    lifecycle_raw = _objects(lifecycle.get("raw"), "remote guard raw files")
    require(len(lifecycle_raw) == 2 and all(sha256(bundle.file(pin)) == sha256(raw[name])
            for pin, name in zip(lifecycle_raw, ("cargo.jsonl", "guard-cargo.stderr"), strict=True)),
            "remote guard raw files differ")
    capacity_input = bundle.json(proof.get("capacityInputRef"))
    state, state_sha = _recorded_capacity_state(bundle, capacity_input)
    registry = bundle.json(capacity_input.get("protectedRegistryRef"))
    protected = [_text(row.get("path"), "remote protected path")
                 for row in _objects(registry.get("refs"), "remote protected refs")]
    require(registry.get("schema") == "krr-protected-refs-successor-v1"
            and _integer(registry.get("physicalRefCount"), "remote protected count") == len(protected)
            and bool(protected) and len(set(protected)) == len(protected), "remote protected refs differ")
    cargo = _object(proof.get("cargoLauncher"), "remote Cargo launcher")
    validate_guarded_build_argv(argv, controller_root, logical_harness,
                                _canonical_build_path(cargo.get("path"), "remote Cargo launcher"),
                                "candidate", state, state_sha, protected,
                                guard_python=_canonical_build_path(python.get("launcher"), "remote Python launcher"))
    return {"head": head, "binarySha256": _text(frozen.get("sha256"), "remote binary SHA"),
            "sourceSnapshotSha256": _text(snapshot_ref.get("sha256"), "remote snapshot SHA"),
            "buildArtifactSha256": sha256(proof_path), "lockSha256": lock_sha,
            "cargoExecutableSha256": _text(_object(proof.get("cargoExecutable"), "Cargo executable").get("sha256"), "Cargo SHA"),
            "rustcExecutableSha256": _text(_object(proof.get("rustcExecutable"), "Rustc executable").get("sha256"), "Rustc SHA"),
            "buildEnvironmentSha256": proof["buildEnvironmentSha256"], "toolVersions": proof["toolVersions"],
            "cargoConfigPins": proof["cargoConfigPins"], "cargoLauncher": proof["cargoLauncher"],
            "rustcLauncher": proof["rustcLauncher"], "guardPython": proof["guardPython"]}


class FixedEpochCandidateOrigin(NamedTuple):
    """Only the original candidate build identity; never a current release acceptance."""
    head: str
    binary_sha256: str
    snapshot_sha256: str
    lock_sha256: str
    artifact_sha256: str
    cargo_sha256: str
    rustc_sha256: str


def _fixed_epoch_git_records(root: Path, before: JsonObject, head: str, *,
                             git_query: GitQuery | None = None) -> dict[str, tuple[str, int]]:
    require(len(head) == 40 and all(char in "0123456789abcdef" for char in head)
            and git(root, "rev-parse", "--verify", head + "^{commit}", git_query=git_query).strip() == head,
            "fixed epoch commit differs")
    require(before.get("schema") == "krr-native-build-source-inventory-v1"
            and before.get("sourceManifestSchema") == "krr-native-build-source-manifest-v1"
            and before.get("head") == head and before.get("stableEndpoints") is True
            and before.get("untrackedPaths") == [] and before.get("missingTrackedPaths") == [],
            "fixed epoch source declaration differs")
    origin = _canonical_build_path(before.get("root"), "fixed epoch source root")
    require(".." not in origin.parts, "fixed epoch source root differs")
    def relative(value: object) -> str:
        path = _canonical_build_path(value, "fixed epoch tracked path")
        require(".." not in path.parts, "fixed epoch tracked path differs")
        if os.name == "nt":
            return path.relative_to(origin).as_posix()
        require(Path(path.anchor) == Path(origin.anchor), "fixed epoch tracked path differs")
        relative_path = os.path.relpath(path, origin)
        require(relative_path != os.pardir and not relative_path.startswith(os.pardir + os.sep),
                "fixed epoch tracked path differs")
        return Path(relative_path).as_posix()
    tracked = _array(before.get("trackedPaths"), "fixed tracked paths")
    tracked_paths = ({relative(value) for value in tracked} if os.name == "nt" else
                     {_text(value, "fixed epoch tracked path") for value in tracked})
    absolute = records_map(before.get("files"))
    require(len(tracked_paths) == len(tracked)
            and (os.name == "nt" or tracked_paths == absolute.keys()),
            "fixed epoch Git source differs")
    saved = {relative(path): value for path, value in absolute.items()}
    records = committed_records(root, head, git_query=git_query)
    require((os.name != "nt" or tracked_paths == records.keys())
            and len(saved) == len(absolute) and saved == records, "fixed epoch Git source differs")
    index = _object(before.get("index"), "fixed epoch index")
    digest(index.get("sha256"))
    unsigned = {key: value for key, value in before.items() if key != "inventoryDigest"}
    actual_digest = hashlib.sha256(json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(before.get("inventoryDigest") == actual_digest, "fixed epoch source inventory digest differs")
    return records


class _BundleWithManifest:
    """Expose a bundle's own manifest only under its exact pinned identity."""
    def __init__(self, bundle: Bundle, manifest_ref: JsonObject) -> None:
        self.bundle = bundle
        self.manifest_ref = manifest_ref

    def file(self, reference: object) -> Path:
        if reference == self.manifest_ref:
            path = self.bundle.directory / "bundle.json"
            require(sha256(path) == self.manifest_ref["sha256"], "fixed epoch bundle SHA differs")
            return path
        return self.bundle.file(reference)

    def json(self, reference: object) -> JsonObject:
        return read_json(self.file(reference))


def validate_fixed_epoch_candidate_origin_reader(
    reader: FileReader, root: Path, manifest_ref: JsonObject, artifact_ref: JsonObject,
    expected_head: str, *, git_query: GitQuery | None = None,
) -> FixedEpochCandidateOrigin:
    """Authenticate the old build through its pinned transport, not current acceptance."""
    require(set(manifest_ref) == {"path", "sha256"} and set(artifact_ref) == {"path", "sha256"},
            "fixed epoch reference fields differ")
    manifest_sha = _text(manifest_ref.get("sha256"), "fixed epoch manifest SHA")
    artifact_sha = _text(artifact_ref.get("sha256"), "fixed epoch artifact SHA")
    digest(manifest_sha)
    digest(artifact_sha)
    # HistoricalBuildReader confines every resolution to this exact manifest's original pins.
    selection = reference_domain.ActiveReference("fixed-epoch", expected_head, manifest_sha, "", {})
    scoped = HistoricalBuildReader(reader, manifest_ref, selection)
    proof_path = scoped.file(artifact_ref)
    proof = read_json(proof_path)
    require(proof.get("schema") == "krr-html-performance-build-artifact-v3"
            and proof.get("arm") == "candidate" and proof.get("buildExit") == 0
            and proof.get("referenceRole") is None, "fixed epoch candidate build role differs")
    before = scoped.json(proof.get("sourceBeforeRef"))
    records = _fixed_epoch_git_records(root, before, expected_head, git_query=git_query)
    lock_sha = records.get(HARNESS_ROOT + "Cargo.lock", (None,))[0]
    digest(lock_sha)
    if not isinstance(lock_sha, str):
        raise ValueError("fixed epoch lock is missing")
    logical_root = _canonical_build_path(before.get("root"), "fixed epoch source root")
    result = _validate_transported_candidate_body(
        scoped, proof_path, proof, expected_head, records, logical_root, logical_root, lock_sha,
    )
    require(sha256(reader.file(manifest_ref)) == manifest_sha
            and sha256(scoped.file(artifact_ref)) == artifact_sha
            and _fixed_epoch_git_records(root, before, expected_head, git_query=git_query) == records,
            "fixed epoch inputs changed during validation")
    return FixedEpochCandidateOrigin(
        expected_head, _text(result.get("binarySha256"), "fixed binary SHA"),
        _text(result.get("sourceSnapshotSha256"), "fixed snapshot SHA"),
        _text(result.get("lockSha256"), "fixed lock SHA"),
        _text(result.get("buildArtifactSha256"), "fixed artifact SHA"),
        _text(result.get("cargoExecutableSha256"), "fixed cargo SHA"),
        _text(result.get("rustcExecutableSha256"), "fixed rustc SHA"),
    )


def validate_fixed_epoch_candidate_origin(
    directory: Path, root: Path, manifest_sha256: str, artifact_path: str,
    artifact_sha256: str, expected_head: str, *, git_query: GitQuery | None = None,
) -> FixedEpochCandidateOrigin:
    """Verify original transported build and Git source without evaluating current source."""
    digest(manifest_sha256)
    digest(artifact_sha256)
    require(sha256(directory / "bundle.json") == manifest_sha256, "fixed epoch bundle SHA differs")
    bundle = Bundle(directory)
    require(bundle.manifest.get("head") == expected_head, "fixed epoch bundle HEAD differs")
    artifact_ref = bundle.relative_reference(artifact_path, artifact_sha256)
    manifest_ref: JsonObject = {"path": str(bundle.directory / "bundle.json"), "sha256": manifest_sha256}
    return validate_fixed_epoch_candidate_origin_reader(
        _BundleWithManifest(bundle, manifest_ref), root, manifest_ref, artifact_ref,
        expected_head, git_query=git_query,
    )


def validate_remote_history_baseline(bundle: Bundle, root: Path, expected: JsonObject, head: str) -> JsonObject:
    """Bind a transported historical baseline to the active committed reference."""
    from verify_html_performance_evidence import baseline_arm_id, git

    def query(path: Path, args: tuple[str, ...]) -> str:
        return git(path, *args)
    selection = reference_domain.active_reference_at_commit(root, head, query, allow_legacy_missing=True)
    if selection is None:
        raise ValueError("remote history reference is unavailable")
    role = "reference-history-" + selection.record_id
    require(baseline_arm_id(expected) == role, "remote baseline is not the active history reference")
    contract = _object(expected.get("referenceParent"), "remote reference contract")
    wrapper = bundle.file(contract.get("wrapperRef"))
    wrapper_data = read_json(wrapper)
    history = HistoricalBuildReader(bundle, wrapper_data.get("sourceBundleManifest"), selection)

    def resolve(value: object, label: str) -> Path:
        return bundle.file(value) if label in {"source bundle manifest", "verifier stdout"} else history.file(value)

    def validate(value: object, active: reference_domain.ActiveReference) -> JsonObject:
        return validate_remote_candidate_build(history, root, value, active.source_head, historical=active)

    proof = reference_domain.load_history_reference_proof(wrapper, selection, file_resolver=resolve,
                                                          validate_build=validate)
    arm = _object(_object(expected.get("arms"), "remote arms").get(role), "remote baseline arm")
    require(contract.get("recordId") == selection.record_id and contract.get("parentHead") == selection.source_head
            and contract.get("binarySha256") == proof.binary_sha256
            and arm.get("binary_sha256") == proof.binary_sha256
            and arm.get("source_snapshot_sha256") == proof.source_snapshot_sha256
            and arm.get("lock_sha256") == proof.lock_sha256,
            "remote history baseline proof differs")
    return {"binarySha256": proof.binary_sha256, "sourceSnapshotSha256": proof.source_snapshot_sha256,
            "lockSha256": proof.lock_sha256, "head": proof.head,
            "buildArtifactSha256": proof.wrapper_sha256}
