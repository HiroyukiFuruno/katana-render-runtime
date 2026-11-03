"""Strict display projection for a native CLI guard observation."""
from __future__ import annotations

import re
from typing import cast

SCHEMA = "krr-native-cli-guard-observation-v2"
FIELDS = {
    "schema", "status", "diagnosticOnly", "accepted", "attemptToken", "request",
    "sourceManifest", "sourceDigest", "metadata", "metadataObservation", "expectedPackage",
    "expectedTarget", "cargo", "rustc", "cargoLauncher", "rustcLauncher",
    "rustupResolutionQueries", "guardCodePins", "pinsBefore", "pinsAfter", "capacityBefore",
    "capacityAfter", "capacityProtection", "ownedCargo", "binaryBefore", "binaryAfter",
    "artifact", "buildFinished", "errors", "externalGroupClosureRequired", "scope",
}
CARGO_FIELDS = {
    "argv", "cwd", "attemptToken", "environment", "environmentSha256", "pid", "parentPid",
    "sessionId", "processGroupId", "spawnObservedMonotonicNs", "startedNs", "endedNs",
    "actualExit", "signal", "reaped", "stdout", "stderr", "rawFdsClosed", "errors",
}
SHA = re.compile(r"^[0-9a-f]{64}$")
DECIMAL = re.compile(r"^[0-9]+$")
ERROR_CLASSES = {"FileNotFoundError", "PermissionError", "TimeoutExpired", "RuntimeError", "OSError"}
ERROR_REASONS = {
    "RuntimeError: capacity metric unavailable or over capacity": "capacity_unavailable_or_over_limit",
    "RuntimeError: capacity current bytes invalid": "capacity_current_bytes_invalid",
    "RuntimeError: admission changed before spawn": "admission_changed_before_spawn",
    "RuntimeError: admission changed at spawn boundary": "admission_changed_at_spawn",
    "RuntimeError: admission changed after Cargo": "admission_changed_after_cargo",
    "RuntimeError: raw output FDs alias": "raw_output_alias",
    "RuntimeError: Cargo did not inherit guard session/group": "cargo_session_group_mismatch",
    "RuntimeError: Cargo exited unsuccessfully": "cargo_unsuccessful",
    "RuntimeError: raw FD path identity changed": "raw_output_identity_changed",
    "RuntimeError: observed tool/config pins changed": "tool_config_pins_changed",
    "RuntimeError: artifact differs from observed binary": "artifact_binary_mismatch",
    "RuntimeError: Cargo artifact stream empty": "cargo_artifact_stream_empty",
    "RuntimeError: expected CLI artifact must be unique": "cargo_artifact_not_unique",
    "RuntimeError: CLI binary missing from artifact filenames": "cargo_binary_missing",
    "RuntimeError: source manifest differs from current Git source set": "source_inventory_changed",
    "RuntimeError: source digest differs": "source_digest_changed",
    "RuntimeError: Cargo config appeared": "cargo_config_appeared",
}


def _invalid() -> ValueError:
    return ValueError("invalid observation")


def _object(value: object) -> dict[str, object]:
    if type(value) is not dict or any(type(key) is not str for key in value):
        raise _invalid()
    return cast(dict[str, object], value)


def _strings(value: object) -> list[str]:
    if type(value) is not list or any(type(item) is not str for item in value):
        raise _invalid()
    return cast(list[str], value)


def _artifact(value: object) -> dict[str, object] | None:
    if value is None:
        return None
    item = _object(value)
    if (set(item) != {"path", "sha256", "size"} or type(item["path"]) is not str
            or not item["path"] or type(item["sha256"]) is not str
            or SHA.fullmatch(item["sha256"]) is None or type(item["size"]) is not int
            or item["size"] < 0):
        raise _invalid()
    return {key: item[key] for key in ("path", "sha256", "size")}


def _validate_envelope(value: dict[str, object]) -> None:
    if (set(value) != FIELDS or value["schema"] != SCHEMA
            or value["status"] not in ("failed", "cargo-completed")
            or value["diagnosticOnly"] is not True or value["accepted"] is not False
            or value["externalGroupClosureRequired"] is not True):
        raise _invalid()
    for key in ("attemptToken", "sourceDigest", "scope"):
        if type(value[key]) is not str:
            raise _invalid()
    for key in ("request", "sourceManifest", "metadata", "metadataObservation", "expectedPackage",
                "expectedTarget", "cargo", "rustc", "cargoLauncher", "rustcLauncher", "capacityProtection"):
        _object(value[key])
    for key in ("pinsBefore", "pinsAfter", "capacityBefore", "capacityAfter", "binaryBefore",
                "binaryAfter", "artifact", "buildFinished"):
        if value[key] is not None:
            _object(value[key])
    for key in ("rustupResolutionQueries", "guardCodePins"):
        if type(value[key]) is not list or any(type(item) is not dict for item in cast(list[object], value[key])):
            raise _invalid()
    _strings(value["errors"])


def _validate_cargo(cargo: dict[str, object]) -> None:
    if set(cargo) != CARGO_FIELDS:
        raise _invalid()
    _strings(cargo["argv"])
    _strings(cargo["errors"])
    environment = _object(cargo["environment"])
    if any(type(value) is not str for value in environment.values()):
        raise _invalid()
    for key in ("cwd", "attemptToken", "environmentSha256"):
        if type(cargo[key]) is not str:
            raise _invalid()
    for key in ("pid", "parentPid", "sessionId", "processGroupId", "signal"):
        value = cargo[key]
        if value is not None and (type(value) is not int or value <= 0):
            raise _invalid()
    if type(cargo["parentPid"]) is not int:
        raise _invalid()
    if cargo["actualExit"] is not None and type(cargo["actualExit"]) is not int:
        raise _invalid()
    for key in ("startedNs", "endedNs", "spawnObservedMonotonicNs"):
        value = cargo[key]
        if value is not None and (type(value) is not str or DECIMAL.fullmatch(value) is None):
            raise _invalid()
    if any(type(cargo[key]) is not bool for key in ("reaped", "rawFdsClosed")):
        raise _invalid()


def safe_guard_summary(value: object) -> dict[str, object]:
    observation = _object(value)
    _validate_envelope(observation)
    cargo = _object(observation["ownedCargo"])
    _validate_cargo(cargo)
    errors = _strings(observation["errors"])
    classes = sorted({name if name in ERROR_CLASSES else "other" for name in
                      (error.split(":", 1)[0] for error in errors)})
    return {
        "schema": SCHEMA, "status": observation["status"],
        **{key: cargo[key] for key in ("pid", "actualExit", "signal", "reaped", "startedNs", "endedNs", "rawFdsClosed")},
        "stdout": _artifact(cargo["stdout"]), "stderr": _artifact(cargo["stderr"]),
        "errorClasses": classes, "errorCount": len(errors),
        "errorReasons": [ERROR_REASONS.get(error, "other") for error in errors],
    }
