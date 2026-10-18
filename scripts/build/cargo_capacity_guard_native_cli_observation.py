#!/usr/bin/env python3
"""Observe the guard's one admitted CLI build; outer group closure stays external."""
from __future__ import annotations

import hashlib
import json
import os
import re
import signal
import shutil
import stat
import sys
import subprocess
import time
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from types import FrameType
from typing import BinaryIO

from cargo_capacity_guard import (
    JsonObject, _array, _integer, _number, _object, _text,
    _cargo_configuration, cargo_configuration_paths, collect_protected_references, load_state,
    validate_build_executable, validate_build_profile, validate_build_selection,
)

REQUEST_ENV = "KRR_NATIVE_CLI_GUARD_REQUEST"
REQUEST_SHA_ENV = "KRR_NATIVE_CLI_GUARD_REQUEST_SHA256"
REQUEST_SCHEMA = "krr-native-cli-guard-request-v1"
OBSERVATION_SCHEMA = "krr-native-cli-guard-observation-v1"
REQUEST_FIELDS = frozenset((
    "schema", "attemptToken", "cwd", "target", "binaryPath", "issuedMonotonicNs",
    "expiresMonotonicNs", "cargo", "sourceManifest", "sourceDigest", "metadata",
    "metadataObservation", "expectedPackage", "expectedTarget", "environment",
    "environmentSha256", "guardCodePins", "capacityAdmission",
))
BUILD_FLAGS = ["build", "-p", "katana-render-runtime-cli"]
JSON_FLAG = "--message-format=json-render-diagnostics"
BASELINE = 2363047936
THRESHOLD = 4726095872
REQUEST_TTL_NS = 120_000_000_000
BUILD_LIMIT_SECONDS = 1800
TERMINATE_LIMIT_SECONDS = 2
REAP_LIMIT_SECONDS = 10
EMPTY_JUST_ENVIRONMENT_KEYS = frozenset((
    "KRR_BUILD_RECEIPT", "MERMAID_RASTER_SESSION_CONFIG", "MERMAID_RASTER_SESSION_CONFIG_SHA",
    "MERMAID_RASTER_BINDING_CONFIG", "MERMAID_RASTER_BINDING_CONFIG_SHA",
))
ENVIRONMENT_KEYS = frozenset((
    "PATH", "HOME", "USER", "LOGNAME", "LANG", "LC_ALL", "LC_CTYPE", "TERM",
    "TMPDIR", "SHELL", "RUSTFLAGS", "CARGO_CAPACITY_PROTECTED_REFS", "CARGO_TARGET_DIR",
    "PWD", "SHLVL", "_", "__CF_USER_TEXT_ENCODING",
)) | EMPTY_JUST_ENVIRONMENT_KEYS


def require(ok: bool, label: str) -> None:
    if not ok:
        raise RuntimeError(label)


def exact(value: object, fields: frozenset[str] | set[str], label: str) -> JsonObject:
    result = _object(value, label)
    require(set(result) == fields, label + " field set differs")
    return result


def unique_object(pairs: list[tuple[str, object]]) -> JsonObject:
    result: JsonObject = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key: " + key)
        result[key] = value
    return result


def parse_json(raw: bytes) -> JsonObject:
    try:
        return _object(json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object), "JSON")
    except (ValueError, UnicodeError) as error:
        raise RuntimeError("invalid UTF-8 JSON") from error


def digest(value: object) -> str:
    text = _text(value, "SHA256")
    require(re.fullmatch(r"[0-9a-f]{64}", text) is not None, "invalid SHA256")
    return text


def canonical(value: object, *, regular: bool = False) -> Path:
    text = _text(value, "canonical path")
    path = Path(text)
    require(path.is_absolute() and str(path) == text and ".." not in path.parts,
            "path must be absolute without traversal")
    require(path.resolve() == path, "path is not canonical")
    for ancestor in (path, *path.parents):
        require(not ancestor.is_symlink(), "symlink path component")
    if regular:
        require(stat.S_ISREG(path.lstat().st_mode), "path is not a regular file")
    return path


def file_record(path: Path) -> JsonObject:
    canonical(str(path), regular=True)
    before = path.stat()
    raw = path.read_bytes()
    after = path.stat()
    require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns),
            "file changed during observation")
    return {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def validate_record(value: object, *, expected: Path | None = None) -> JsonObject:
    record = exact(value, {"path", "sha256", "size"}, "file record")
    path_text = _text(record["path"], "file path")
    digest(record["sha256"])
    require(_integer(record["size"], "file size") >= 0, "negative file size")
    if expected is not None:
        require(path_text == str(expected), "file path differs from required role")
    path = canonical(path_text, regular=True)
    require(file_record(path) == record, "pinned file changed")
    return record


def identity(path: Path) -> JsonObject:
    record = file_record(path)
    value = path.stat()
    return {**record, "device": value.st_dev, "inode": value.st_ino,
            "mtimeNs": str(value.st_mtime_ns), "ctimeNs": str(value.st_ctime_ns)}


def environment_digest(environment: Mapping[str, str]) -> str:
    raw = json.dumps(dict(environment), sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def environment_record(value: object) -> dict[str, str]:
    return {key: _text(item, "environment value") for key, item in _object(value, "environment").items()}


def decimal(value: object, label: str) -> int:
    text = _text(value, label)
    require(re.fullmatch(r"0|[1-9][0-9]*", text) is not None, label + " is not decimal ns")
    return int(text)


def source_digest(records: list[JsonObject]) -> str:
    material = "".join(f"{row['path']}\0{row['sha256']}\0{row['size']}\n" for row in records)
    return hashlib.sha256(material.encode("utf-8")).hexdigest()


def protected_identity(value: object) -> JsonObject:
    record = exact(value, {"path", "sha256", "size", "dev", "ino", "mode"}, "protected identity")
    pin = validate_record({name: record[name] for name in ("path", "sha256", "size")})
    actual = Path(_text(pin["path"], "protected path")).stat()
    for key, expected in (("dev", actual.st_dev), ("ino", actual.st_ino), ("mode", actual.st_mode)):
        require(_integer(record[key], "protected " + key) == expected, "protected identity differs")
    return record


def policy(target: Path) -> JsonObject:
    return {"metric": "unique-inode-allocated-bytes", "metric_version": 1,
            "baseline_bytes": BASELINE, "factor": 2, "threshold_bytes": THRESHOLD,
            "target": str(target)}


def validate_capacity_report(report: JsonObject, target: Path) -> None:
    for key, expected in policy(target).items():
        actual = report.get(key)
        if isinstance(expected, int):
            require(_number(actual, key) == expected, "capacity " + key + " differs")
        else:
            require(actual == expected, "capacity " + key + " differs")
    require(report.get("metric_available", True) is True and report.get("over_capacity") is False,
            "capacity metric unavailable or over capacity")
    require(0 <= _integer(report.get("current_bytes"), "current bytes") <= THRESHOLD,
            "capacity current bytes invalid")
    require(not report.get("removed") and report.get("cleanup_status", "not-needed") == "not-needed",
            "native observation requires capacity without cleanup")


@dataclass(frozen=True)
class Admission:
    request: JsonObject
    request_pin: JsonObject
    attempt: Path
    environment: dict[str, str]
    source: list[JsonObject]
    protected: list[str]
    state: Path
    state_sha256: str
    expected_features: tuple[str, ...]


def validate_metadata(request: JsonObject, root: Path, attempt: Path) -> tuple[str, ...]:
    package = exact(request["expectedPackage"], {"id", "name", "manifestPath"}, "expected package")
    target = exact(request["expectedTarget"], {"name", "kind", "srcPath"}, "expected target")
    require(bool(_text(package["id"], "package id")), "empty package id")
    require(package["name"] == "katana-render-runtime-cli"
            and package["manifestPath"] == str(root / "crates/katana-render-runtime-cli/Cargo.toml"),
            "expected package differs")
    require(target == {"name": "krr", "kind": ["bin"],
                       "srcPath": str(root / "crates/katana-render-runtime-cli/src/main.rs")},
            "expected target differs")
    metadata_pin = validate_record(request["metadata"])
    observation_pin = validate_record(request["metadataObservation"])
    for pin in (metadata_pin, observation_pin):
        require(Path(_text(pin["path"], "metadata path")).parent == attempt, "metadata outside attempt")
    metadata = parse_json(Path(_text(metadata_pin["path"], "metadata path")).read_bytes())
    require(_integer(metadata.get("version"), "metadata version") == 1
            and metadata.get("workspace_root") == str(root)
            and metadata.get("target_directory") == str(root / "target"), "metadata workspace differs")
    packages = [_object(item, "metadata package") for item in _array(metadata.get("packages"), "packages")]
    ids = [_text(item.get("id"), "package id") for item in packages]
    require(len(ids) == len(set(ids)), "duplicate metadata package id")
    selected = [item for item in packages if item.get("name") == package["name"]
                or item.get("id") == package["id"] or item.get("manifest_path") == package["manifestPath"]]
    require(len(selected) == 1, "CLI metadata package is not unique")
    selected_package = selected[0]
    require(all(selected_package.get(key) == package[expected] for key, expected in
                (("id", "id"), ("name", "name"), ("manifest_path", "manifestPath"))), "CLI metadata differs")
    metadata_features = _object(selected_package.get("features"), "CLI metadata features")
    manifest_features = _object(_cargo_configuration(root / "crates/katana-render-runtime-cli/Cargo.toml").get("features", {}),
                                "CLI manifest features")
    require(metadata_features == manifest_features == {}, "current CLI must have no package features")
    targets = [_object(item, "metadata target") for item in _array(selected_package.get("targets"), "targets")]
    selected_targets = [item for item in targets if item.get("name") == "krr"]
    require(len(selected_targets) == 1 and selected_targets[0].get("kind") == ["bin"]
            and selected_targets[0].get("src_path") == target["srcPath"], "CLI metadata target differs")
    observation = parse_json(Path(_text(observation_pin["path"], "metadata observation path")).read_bytes())
    exact(observation, {"argv", "stdout", "stderr", "label", "cwd", "attemptToken", "environment",
                        "environmentSha256", "pid", "actualExit", "startedNs", "endedNs", "reaped", "errors"},
          "metadata observation")
    argv = [_text(item, "metadata argv") for item in _array(observation["argv"], "metadata argv")]
    cargo = _object(request["cargo"], "Cargo")
    require(len(argv) == 10 and Path(argv[0]).name == "rtk"
            and argv[1:] == ["proxy", cargo["path"], "metadata", "--frozen", "--format-version", "1",
                             "--no-deps", "--manifest-path", str(root / "Cargo.toml")], "metadata argv differs")
    require(canonical(argv[0], regular=True).stat().st_mode & 0o111 != 0, "metadata RTK is not executable")
    require(re.fullmatch(r"probe-[1-9][0-9]*-cargo-metadata", _text(observation["label"], "probe label"))
            is not None, "metadata label differs")
    require(observation["cwd"] == str(root) and observation["attemptToken"] == request["attemptToken"]
            and observation["environment"] == request["environment"]
            and observation["environmentSha256"] == request["environmentSha256"]
            and _integer(observation["pid"], "metadata pid") > 0
            and _integer(observation["actualExit"], "metadata exit") == 0
            and decimal(observation["endedNs"], "metadata ended ns") >= decimal(observation["startedNs"], "metadata started ns")
            and observation["reaped"] is True and observation["errors"] == [], "metadata process proof differs")
    require(validate_record(observation["stdout"]) == metadata_pin, "metadata stdout differs")
    stderr = validate_record(observation["stderr"])
    require(Path(_text(stderr["path"], "metadata stderr")).parent == attempt, "metadata stderr outside attempt")
    return ()


def request_shapes(request: JsonObject) -> None:
    for key in ("cwd", "target", "binaryPath", "issuedMonotonicNs", "expiresMonotonicNs"):
        _text(request[key], key)
    for key in ("attemptToken", "sourceDigest", "environmentSha256"):
        digest(request[key])
    exact(request["expectedPackage"], {"id", "name", "manifestPath"}, "expected package")
    exact(request["expectedTarget"], {"name", "kind", "srcPath"}, "expected target")
    environment_record(request["environment"])
    admission = exact(request["capacityAdmission"], {"state", "protectedRegistry", "refs", "capacityPolicy",
                      "environmentValue", "environmentSha256"}, "capacity admission")
    exact(admission["capacityPolicy"], set(policy(Path(_text(request["target"], "target")))), "capacity policy")
    pins = [request[key] for key in ("cargo", "sourceManifest", "metadata", "metadataObservation")]
    pins += _array(request["guardCodePins"], "guard code pins")
    pins += [admission[key] for key in ("state", "protectedRegistry")]
    for value in pins:
        row = exact(value, {"path", "sha256", "size"}, "file record")
        _text(row["path"], "file path")
        digest(row["sha256"])
        require(_integer(row["size"], "file size") >= 0, "negative file size")
    for value in _array(admission["refs"], "protected refs"):
        row = exact(value, {"path", "sha256", "size", "dev", "ino", "mode"}, "protected identity")
        _text(row["path"], "protected path")
        digest(row["sha256"])
        for key in ("size", "dev", "ino", "mode"):
            require(_integer(row[key], "protected " + key) >= 0, "negative protected identity")


def validate_request(root: Path, command: list[str], environment: Mapping[str, str],
                     *, consumed: bool = False, check_deadline: bool = True) -> Admission:
    require(os.name == "posix", "native managed observation requires POSIX")
    require(REQUEST_ENV in environment and REQUEST_SHA_ENV in environment, "request environment pair is incomplete")
    request_path = canonical(environment[REQUEST_ENV], regular=True)
    attempt = request_path.parent
    require(attempt != root and attempt.is_relative_to(root / "tmp"), "request attempt must be inside repository tmp")
    pin = file_record(request_path)
    require(pin["sha256"] == digest(environment[REQUEST_SHA_ENV]), "request SHA256 differs")
    request = exact(parse_json(request_path.read_bytes()), REQUEST_FIELDS, "native request")
    require(request["schema"] == REQUEST_SCHEMA, "native request schema differs")
    request_shapes(request)
    nonce = digest(request["attemptToken"])
    require(attempt.name == nonce, "attempt directory differs from nonce")
    require(not os.path.lexists(attempt / "observation.json")
            and (consumed or not os.path.lexists(attempt / "started.json")), "attempt already consumed")
    if not consumed:
        require(all(not os.path.lexists(attempt / name) for name in
                    ("cargo.stdout.raw", "cargo.stderr.raw", "capacity.before.json", "capacity.after.json")),
                "attempt contains stale observation outputs")
    issued = decimal(request["issuedMonotonicNs"], "request issue")
    expires = decimal(request["expiresMonotonicNs"], "request expiry")
    require(expires == issued + REQUEST_TTL_NS and issued <= time.monotonic_ns()
            and (not check_deadline or time.monotonic_ns() < expires), "request deadline invalid")
    for key, expected in (("cwd", root), ("target", root / "target"), ("binaryPath", root / "target/debug/krr")):
        require(request[key] == str(expected), key + " differs")
        canonical(request[key])
    cargo = validate_record(request["cargo"])
    cargo_path = Path(_text(cargo["path"], "Cargo path"))
    require(cargo_path.name == "cargo" and cargo_path.stat().st_mode & 0o111 != 0, "Cargo must be actual executable")
    require(command == [str(cargo_path), *BUILD_FLAGS], "only exact direct CLI Cargo build is admitted")
    validate_build_profile("debug", command)
    validate_build_selection(root, environment)
    validate_build_executable(root, command, environment)
    require(not cargo_configuration_paths(root, environment), "native observation does not admit Cargo configs")
    child_environment = {key: value for key, value in environment.items() if key not in (REQUEST_ENV, REQUEST_SHA_ENV)}
    require(all(child_environment.get(key, "") == "" for key in EMPTY_JUST_ENVIRONMENT_KEYS),
            "Just receipt/raster selectors must be empty for native build")
    require(set(child_environment).issubset(ENVIRONMENT_KEYS)
            and bool(child_environment.get("PATH")) and bool(child_environment.get("HOME"))
            and child_environment.get("RUSTFLAGS") == "-D warnings", "unsupported Cargo environment")
    if "PWD" in child_environment:
        require(child_environment["PWD"] == str(root), "Just PWD differs from repository")
    if "SHLVL" in child_environment:
        require(child_environment["SHLVL"] == "0", "Just SHLVL must preserve managed base zero")
    if "_" in child_environment:
        python = shutil.which("python3", path=child_environment["PATH"])
        require(python is not None and Path(python).is_absolute() and child_environment["_"] == python,
                "Just underscore differs from PATH python3 invocation")
    if "__CF_USER_TEXT_ENCODING" in child_environment:
        encoding = child_environment["__CF_USER_TEXT_ENCODING"]
        require(sys.platform == "darwin" and re.fullmatch(r"0x[0-9A-Fa-f]+:0x[0-9A-Fa-f]+:0x[0-9A-Fa-f]+", encoding) is not None,
                "CoreFoundation environment is not a Darwin uid record")
        require(int(encoding.split(":")[0], 16) == os.getuid(), "CoreFoundation environment uid differs")
    require(environment_record(request["environment"]) == child_environment
            and request["environmentSha256"] == environment_digest(child_environment), "Cargo environment differs")
    source_pin = validate_record(request["sourceManifest"])
    require(Path(_text(source_pin["path"], "source manifest path")).parent == attempt, "source manifest outside attempt")
    manifest = exact(parse_json(Path(_text(source_pin["path"], "source manifest path")).read_bytes()),
                     {"schema", "digest", "files"}, "source manifest")
    require(manifest["schema"] == "krr-native-build-source-manifest-v1", "source manifest schema differs")
    records = [validate_record(item) for item in _array(manifest["files"], "source files")]
    paths = [_text(row["path"], "source path") for row in records]
    require(paths == sorted(set(paths), key=lambda item: item.encode("utf-8")), "source paths not unique sorted")
    require(all(Path(path).is_relative_to(root) and not Path(path).is_relative_to(root / "target")
                and not Path(path).is_relative_to(root / "tmp") for path in paths), "source contains generated/outside path")
    require(manifest["digest"] == request["sourceDigest"] == source_digest(records), "source digest differs")
    visible: set[str] = set()
    for flags in (["ls-files", "-z"], ["ls-files", "--others", "--exclude-standard", "-z"]):
        query = subprocess.run(["git", *flags], cwd=root, capture_output=True, timeout=30, check=False)
        require(query.returncode == 0, "Git source inventory unavailable")
        visible.update(str(root / item.decode("utf-8")) for item in query.stdout.split(b"\0") if item)
    require(set(paths) == visible, "source manifest differs from current Git source set")
    required = {str(root / suffix) for suffix in ("Cargo.toml", "Cargo.lock", "Justfile",
                "crates/katana-render-runtime-cli/Cargo.toml", "crates/katana-render-runtime-cli/src/main.rs")}
    code_paths = [root / "scripts/build" / name for name in
                  ("cargo_capacity_guard.py", "cargo_capacity_guard_lock.py", Path(__file__).name)]
    code = [validate_record(item) for item in _array(request["guardCodePins"], "guard code pins")]
    require([row["path"] for row in code] == [str(path) for path in code_paths], "guard code pin roles differ")
    require(required.union(str(path) for path in code_paths).issubset(paths)
            and all(row in records for row in code), "required source/code missing")
    expected_features = validate_metadata(request, root, attempt)
    admission = exact(request["capacityAdmission"], {"state", "protectedRegistry", "refs", "capacityPolicy",
                      "environmentValue", "environmentSha256"}, "capacity admission")
    state = validate_record(admission["state"], expected=root / "tmp/.cargo-capacity-guard.json")
    registry = validate_record(admission["protectedRegistry"])
    registry_path = Path(_text(registry["path"], "registry path"))
    require(registry_path.is_relative_to(root / "tmp") and not registry_path.is_relative_to(attempt),
            "registry must be a current independent capacity input")
    refs = [protected_identity(item) for item in _array(admission["refs"], "protected refs")]
    registry_value = exact(parse_json(registry_path.read_bytes()), {"schema", "physicalRefCount", "refs"}, "protected registry")
    require(registry_value["schema"] == "krr-protected-refs-successor-v1"
            and _integer(registry_value["physicalRefCount"], "protected count") == len(refs) > 0
            and registry_value["refs"] == refs, "current protected registry differs")
    protected = [_text(row["path"], "protected path") for row in refs]
    require(len(set(protected)) == len(protected), "protected paths not unique")
    env_value = child_environment.get("CARGO_CAPACITY_PROTECTED_REFS")
    require(env_value is not None and admission["environmentValue"] == env_value
            and admission["environmentSha256"] == hashlib.sha256(env_value.encode()).hexdigest(), "protected environment differs")
    require(_array(json.loads(_text(env_value, "protected environment"), object_pairs_hook=unique_object), "protected environment") == protected
            and collect_protected_references([], env_value) == protected, "protected environment paths differ")
    fixed_policy = exact(admission["capacityPolicy"], set(policy(root / "target")), "capacity policy")
    for name, wanted in policy(root / "target").items():
        if isinstance(wanted, int):
            require(_number(fixed_policy[name], "capacity policy " + name) == wanted, "fixed capacity policy differs")
        else:
            require(fixed_policy[name] == wanted, "fixed capacity policy differs")
    state_sha = digest(state["sha256"])
    saved = load_state(root / "tmp/.cargo-capacity-guard.json", state_sha)
    for key, policy_value in policy(root / "target").items():
        if key != "threshold_bytes":
            require(saved.get(key) == policy_value and not isinstance(saved.get(key), bool), "fixed capacity state differs")
    metadata_process = parse_json(Path(_text(_object(request["metadataObservation"], "metadata observation")["path"],
                                                 "metadata observation path")).read_bytes())
    roles = [pin, source_pin, _object(request["metadata"], "metadata"),
             _object(request["metadataObservation"], "metadata observation"),
             _object(metadata_process["stderr"], "metadata stderr"), cargo, state, registry]
    reserved_outputs = {attempt / name for name in ("started.json", "observation.json", "cargo.stdout.raw",
                        "cargo.stderr.raw", "capacity.before.json", "capacity.after.json")}
    require(all(Path(_text(row["path"], "role path")) not in reserved_outputs for row in roles),
            "input aliases reserved output role")
    role_ids = [(Path(_text(row["path"], "role path")).stat().st_dev,
                 Path(_text(row["path"], "role path")).stat().st_ino) for row in roles]
    require(len(set(role_ids)) == len(role_ids), "input roles alias inodes")
    return Admission(request, pin, attempt, child_environment, records, protected,
                     root / "tmp/.cargo-capacity-guard.json", state_sha, expected_features)


def validate_cli_artifact_configuration(artifact: JsonObject, expected_features: tuple[str, ...]) -> None:
    features = [_text(item, "CLI artifact feature") for item in _array(artifact.get("features"), "CLI artifact features")]
    require(len(features) == len(set(features)) and set(features) == set(expected_features),
            "CLI artifact feature set differs from metadata/manifest")
    profile = exact(artifact.get("profile"),
                    {"opt_level", "debuginfo", "debug_assertions", "overflow_checks", "test"}, "CLI artifact profile")
    require(_text(profile["opt_level"], "artifact optimization level") in {"0", "1", "2", "3", "s", "z"},
            "artifact optimization level invalid")
    debug = profile["debuginfo"]
    require(debug is None or type(debug) is int and debug in (0, 1, 2)
            or isinstance(debug, str) and debug in {"line-directives-only", "line-tables-only"},
            "artifact debug information level invalid")
    for field in ("debug_assertions", "overflow_checks", "test"):
        require(isinstance(profile[field], bool), "artifact profile " + field + " must be boolean")
    require(profile["test"] is False, "CLI artifact must not use a test profile")


def cargo_events(raw: bytes, admission: Admission) -> tuple[JsonObject, JsonObject]:
    events = [parse_json(line) for line in raw.splitlines() if line.strip()]
    require(bool(events), "Cargo artifact stream empty")
    for event in events:
        _text(event.get("reason"), "Cargo event reason")
    finished = [event for event in events if event["reason"] == "build-finished"]
    require(len(finished) == 1 and events[-1] == finished[0] and finished[0].get("success") is True,
            "Cargo build-finished is missing, duplicate, failed or nonterminal")
    package = _object(admission.request["expectedPackage"], "expected package")
    target = _object(admission.request["expectedTarget"], "expected target")
    selected = [event for event in events if event["reason"] == "compiler-artifact"
                and event.get("package_id") == package["id"]
                and _object(event.get("target"), "artifact target").get("name") == target["name"]]
    require(len(selected) == 1, "expected CLI artifact must be unique")
    artifact = selected[0]
    actual_target = _object(artifact["target"], "artifact target")
    require(actual_target.get("kind") == ["bin"] and actual_target.get("src_path") == target["srcPath"]
            and artifact.get("executable") == admission.request["binaryPath"]
            and isinstance(artifact.get("fresh"), bool), "CLI artifact identity differs")
    validate_cli_artifact_configuration(artifact, admission.expected_features)
    filenames = [_text(item, "artifact filename") for item in _array(artifact.get("filenames"), "artifact filenames")]
    require(admission.request["binaryPath"] in filenames, "CLI binary missing from artifact filenames")
    canonical(artifact["executable"], regular=True)
    return artifact, finished[0]


def exclusive_json(path: Path, value: object) -> JsonObject:
    raw = (json.dumps(value, sort_keys=True, ensure_ascii=False, indent=2) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        require(stat.S_ISREG(os.fstat(handle.fileno()).st_mode), "output FD is not regular")
        handle.write(raw)
        handle.flush()
        os.fsync(handle.fileno())
    return file_record(path)


def raw_output(path: Path) -> BinaryIO:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        require(stat.S_ISREG(os.fstat(descriptor).st_mode), "raw FD is not regular")
        return os.fdopen(descriptor, "wb")
    except BaseException:
        os.close(descriptor)
        raise


def observation_pins(admission: Admission, root: Path) -> JsonObject:
    rustc = shutil.which("rustc", path=admission.environment["PATH"])
    require(rustc is not None, "PATH rustc resolution unavailable")
    rustc_path = Path(_text(rustc, "rustc path")).resolve(strict=True)
    require(rustc_path.stat().st_mode & 0o111 != 0, "PATH rustc is not executable")
    config_paths = [base / name for base in (root, *root.parents)
                    for name in (".cargo/config", ".cargo/config.toml")]
    config_paths += [Path(admission.environment["HOME"]) / ".cargo" / name for name in ("config", "config.toml")]
    require(all(not os.path.lexists(path) for path in config_paths), "Cargo config appeared")
    return {"source": admission.source, "cargo": admission.request["cargo"],
            "guardCodePins": admission.request["guardCodePins"],
            "capacityAdmission": admission.request["capacityAdmission"],
            "observedPathRustc": file_record(rustc_path),
            "unsupportedCargoConfigsAbsent": [str(path) for path in dict.fromkeys(config_paths)]}


def observe(admission: Admission, command: list[str], root: Path, report: JsonObject,
            inspect_post: Callable[[], JsonObject]) -> int:
    request = admission.request
    attempt = admission.attempt
    binary_path = root / "target/debug/krr"
    errors: list[str] = []
    owned: JsonObject = {"argv": [*command, JSON_FLAG], "cwd": str(root), "attemptToken": request["attemptToken"],
        "environment": admission.environment, "environmentSha256": request["environmentSha256"],
        "pid": None, "parentPid": os.getpid(), "sessionId": None, "processGroupId": None,
        "spawnObservedMonotonicNs": None, "startedNs": None, "endedNs": None,
        "actualExit": None, "signal": None, "reaped": False, "stdout": None, "stderr": None,
        "rawFdsClosed": False, "errors": errors}
    output: JsonObject = {"schema": OBSERVATION_SCHEMA, "status": "failed", "diagnosticOnly": True,
        "accepted": False, "attemptToken": request["attemptToken"], "request": admission.request_pin,
        "sourceManifest": request["sourceManifest"], "sourceDigest": request["sourceDigest"],
        "metadata": request["metadata"], "metadataObservation": request["metadataObservation"],
        "expectedPackage": request["expectedPackage"], "expectedTarget": request["expectedTarget"],
        "cargo": request["cargo"], "guardCodePins": request["guardCodePins"], "pinsBefore": None, "pinsAfter": None,
        "capacityBefore": None, "capacityAfter": None, "capacityProtection": request["capacityAdmission"],
        "ownedCargo": owned, "binaryBefore": None, "binaryAfter": None, "artifact": None, "buildFinished": None,
        "errors": errors, "externalGroupClosureRequired": True,
        "scope": "PATH rustc file is observed, not independently admitted; effective rustup compiler/version is unverified. POSIX leader observation; spawn/PID sampling is non-atomic; pinsAfter precedes external group closure; source endpoints do not prove atomic freeze or registry content attestation."}
    process: subprocess.Popen[bytes] | None = None
    handles: list[BinaryIO] = []
    raw_identities: list[tuple[int, int]] = []
    started_pin: JsonObject | None = None
    previous_handlers: dict[signal.Signals, int | Callable[[int, FrameType | None], object] | None] = {}

    def interrupted(signum: int, frame: FrameType | None) -> None:
        raise RuntimeError("Cargo observation interrupted by signal " + str(signum))

    try:
        started_pin = exclusive_json(attempt / "started.json", {"schema": "krr-native-cli-guard-started-v1",
                       "attemptToken": request["attemptToken"], "request": admission.request_pin})
        current = validate_request(root, command, os.environ, consumed=True)
        require(current == admission, "admission changed before spawn")
        validate_capacity_report(report, root / "target")
        output["capacityBefore"] = exclusive_json(attempt / "capacity.before.json", report)
        output["pinsBefore"] = observation_pins(admission, root)
        if binary_path.exists():
            output["binaryBefore"] = identity(binary_path)
        handles.append(raw_output(attempt / "cargo.stdout.raw"))
        handles.append(raw_output(attempt / "cargo.stderr.raw"))
        raw_identities = [(os.fstat(handle.fileno()).st_dev, os.fstat(handle.fileno()).st_ino) for handle in handles]
        require(len(set(raw_identities)) == 2, "raw output FDs alias")
        validate_record(started_pin, expected=attempt / "started.json")
        require(validate_request(root, command, os.environ, consumed=True) == admission,
                "admission changed at spawn boundary")
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous_handlers[signum] = signal.signal(signum, interrupted)
        started = time.monotonic_ns()
        owned["startedNs"] = str(time.time_ns())
        process = subprocess.Popen([*command, JSON_FLAG], cwd=root, env=admission.environment,
                                   stdout=handles[0], stderr=handles[1])
        owned["pid"] = process.pid
        owned["spawnObservedMonotonicNs"] = str(time.monotonic_ns())
        owned["sessionId"] = os.getsid(process.pid)
        owned["processGroupId"] = os.getpgid(process.pid)
        require(owned["sessionId"] == os.getsid(0) and owned["processGroupId"] == os.getpgrp(),
                "Cargo did not inherit guard session/group")
        process.wait(timeout=max(0.0, BUILD_LIMIT_SECONDS - (time.monotonic_ns() - started) / 1e9))
        require(process.returncode == 0, "Cargo exited unsuccessfully")
    except BaseException as error:
        errors.append(type(error).__name__ + ": " + str(error))
    finally:
        for signum, previous in previous_handlers.items():
            signal.signal(signum, previous)
        if process is not None:
            if process.poll() is None:
                try:
                    process.terminate()
                    process.wait(timeout=TERMINATE_LIMIT_SECONDS)
                except BaseException as error:
                    errors.append("leader terminate: " + type(error).__name__ + ": " + str(error))
                    try:
                        process.kill()
                        process.wait(timeout=REAP_LIMIT_SECONDS)
                    except BaseException as cleanup_error:
                        errors.append("leader kill/reap: " + type(cleanup_error).__name__ + ": " + str(cleanup_error))
            owned["actualExit"] = process.returncode
            owned["signal"] = -process.returncode if process.returncode is not None and process.returncode < 0 else None
            owned["reaped"] = process.returncode is not None
        owned["endedNs"] = str(time.time_ns())
        close_errors: list[str] = []
        for handle in handles:
            try:
                handle.flush()
                os.fsync(handle.fileno())
            except BaseException as error:
                close_errors.append("raw flush: " + str(error))
            try:
                handle.close()
            except BaseException as error:
                close_errors.append("raw close: " + str(error))
        errors.extend(close_errors)
        owned["rawFdsClosed"] = len(handles) == 2 and all(handle.closed for handle in handles) and not close_errors
    try:
        if owned["reaped"] is True and owned["rawFdsClosed"] is True:
            for index, (key, filename) in enumerate((("stdout", "cargo.stdout.raw"), ("stderr", "cargo.stderr.raw"))):
                raw_stat = (attempt / filename).lstat()
                require((raw_stat.st_dev, raw_stat.st_ino) == raw_identities[index], "raw FD path identity changed")
                owned[key] = file_record(attempt / filename)
        validate_record(started_pin, expected=attempt / "started.json")
        current = validate_request(root, command, os.environ, consumed=True, check_deadline=False)
        require(current == admission, "admission changed after Cargo")
        output["pinsAfter"] = observation_pins(admission, root)
        require(output["pinsAfter"] == output["pinsBefore"], "observed tool/config pins changed")
        post = inspect_post()
        output["capacityAfter"] = exclusive_json(attempt / "capacity.after.json", post)
        validate_capacity_report(post, root / "target")
        if not errors:
            artifact, finished = cargo_events((attempt / "cargo.stdout.raw").read_bytes(), admission)
            output["artifact"], output["buildFinished"] = artifact, finished
            output["binaryAfter"] = identity(binary_path)
            require(file_record(canonical(artifact["executable"], regular=True))
                    == {key: _object(output["binaryAfter"], "binary after")[key] for key in ("path", "sha256", "size")},
                    "artifact differs from observed binary")
            output["status"] = "cargo-completed"
    except BaseException as error:
        errors.append(type(error).__name__ + ": " + str(error))
    try:
        exclusive_json(attempt / "observation.json", output)
    except BaseException as error:
        raise RuntimeError("observation save failed; prior errors: " + repr(errors) + "; save: " + str(error)) from error
    return 0 if not errors else 1
