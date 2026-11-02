"""Strict validation of authenticated remote performance transport."""
from __future__ import annotations

import hashlib
import json
import zipfile
from pathlib import Path, PurePosixPath

import html_performance_reference as reference_domain
from verify_html_performance_evidence import (
    Bundle, ExpectedBuild, FileReader, HARNESS_ROOT, HistoricalBuildReader, JsonObject, LOCK_SHA, _array,
    _canonical_build_path, _historical_reference_source, _integer, _object, _objects, _recorded_capacity_state,
    _require_formal_release_result, _text, cargo_build_artifacts, current_source,
    digest, read_json, records_map, require, sha256, unique_object,
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
    snapshot_ref = _object(proof.get("sourceSnapshotRef"), "remote source snapshot")
    snapshot = bundle.json(snapshot_ref)
    rows = _array(snapshot.get("records"), "remote source records")
    require(records_map(rows) == records
            and snapshot.get("digest") == hashlib.sha256(json.dumps(
                rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "remote candidate source snapshot differs")
    require(proof.get("lockSha256") == LOCK_SHA
            and records.get(HARNESS_ROOT + "Cargo.lock", (None,))[0] == LOCK_SHA,
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
            "buildArtifactSha256": sha256(proof_path), "lockSha256": LOCK_SHA,
            "cargoExecutableSha256": _text(_object(proof.get("cargoExecutable"), "Cargo executable").get("sha256"), "Cargo SHA"),
            "rustcExecutableSha256": _text(_object(proof.get("rustcExecutable"), "Rustc executable").get("sha256"), "Rustc SHA"),
            "buildEnvironmentSha256": proof["buildEnvironmentSha256"], "toolVersions": proof["toolVersions"],
            "cargoConfigPins": proof["cargoConfigPins"], "cargoLauncher": proof["cargoLauncher"],
            "rustcLauncher": proof["rustcLauncher"], "guardPython": proof["guardPython"]}


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
