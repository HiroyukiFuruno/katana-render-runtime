#!/usr/bin/env python3
"""独立source admissionから一回の管理済みCLI build receiptを生成する。"""
from __future__ import annotations

import argparse
import json
import secrets
import subprocess
import time
from pathlib import Path

from native_cli_build_bindings import Bindings, SCHEMA, construct_bindings
import cargo_capacity_guard_native_cli_observation as guard
from cargo_capacity_guard_lock import retained_profile_paths
from cargo_capacity_guard import JsonObject, _array, _object, _text
import produce_html_performance_evidence as owner


def _capacity_preflight(bindings: Bindings, root: Path, attempt: Path) -> JsonObject:
    """native観測内の整理を避けるため、request発行前に既存guardで容量を確保する。"""
    tools = _object(bindings.tools, "bound tools")
    python = _text(_object(tools["python3"], "python3 tool")["path"], "python3 path")
    argv = [python, str(root / "scripts/build/cargo_capacity_guard.py"),
            "--root", str(root), "--target", str(root / "target"),
            "--state", str(root / "tmp/.cargo-capacity-guard.json"),
            "--require-capacity", "--print-json"]
    debug_profile = root / "target" / "debug"
    if debug_profile.exists() or debug_profile.is_symlink():
        retained = retained_profile_paths(root / "target", [debug_profile])
        guard.require(len(retained) == 1, "debug profile retention is ambiguous")
        argv.extend(["--retain-profile", str(retained[0])])
    def observe(log_directory: Path) -> tuple[subprocess.CompletedProcess[bytes], Path, JsonObject,
                                                JsonObject, int, int]:
        lifecycle_paths: list[Path] = []
        started_ns = time.time_ns()
        result = owner._measurement_command(
            argv, role="preflight", log_directory=log_directory,
            cwd=root, env=dict(bindings.child_environment), check=False,
            lifecycle_sink=lifecycle_paths.append, native_attempt_token=attempt.name,
        )
        ended_ns = time.time_ns()
        guard.require(len(lifecycle_paths) == 1, "capacity preflight lifecycle must be unique")
        lifecycle_path = lifecycle_paths[0]
        lifecycle = guard.parse_json(lifecycle_path.read_bytes())
        report = guard.parse_json(result.stdout)
        return result, lifecycle_path, lifecycle, report, started_ns, ended_ns

    def evidence_record(result: subprocess.CompletedProcess[bytes], lifecycle_path: Path,
                        lifecycle: JsonObject, report: JsonObject,
                        started_ns: int, ended_ns: int) -> JsonObject:
        raw = _array(lifecycle["raw"], "capacity preflight raw")
        guard.require(len(raw) == 2, "capacity preflight raw streams are incomplete")
        stdout = guard.file_record(Path(_text(_object(raw[0], "capacity stdout")["path"], "stdout path")))
        stderr = guard.file_record(Path(_text(_object(raw[1], "capacity stderr")["path"], "stderr path")))
        return {"argv": argv, "cwd": str(root), "actualExit": result.returncode, "reaped": True,
                "stdout": stdout, "stderr": stderr, "lifecycle": guard.file_record(lifecycle_path),
                "startedNs": str(started_ns), "endedNs": str(ended_ns), "report": report}

    def require_closed_lifecycle(result: subprocess.CompletedProcess[bytes], lifecycle: JsonObject) -> None:
        guard.require(lifecycle.get("exitCode") == result.returncode
                      and lifecycle.get("groupClosed") is True
                      and lifecycle.get("rawFdsClosed") is True,
                      "capacity preflight lifecycle is incomplete")

    first, first_lifecycle_path, first_lifecycle, first_report, first_started_ns, first_ended_ns = observe(
        attempt / "capacity-preflight")
    first_record = evidence_record(first, first_lifecycle_path, first_lifecycle, first_report,
                                   first_started_ns, first_ended_ns)
    cleanup_status = first_report.get("cleanup_status")
    first_shows_cleanup = (bool(first_report.get("removed")) or bool(first_report.get("cleanup_attempts"))
                           or cleanup_status != "not-needed")
    cleanup_observation: JsonObject | None = None
    refresh_observation: JsonObject | None = None
    if first_shows_cleanup:
        cleanup_path = attempt / "capacity-preflight-cleanup-observation.json"
        guard.exclusive_json(cleanup_path,
                             {"schema": "krr-native-cli-capacity-preflight-cleanup-v1",
                              **first_record})
        cleanup_observation = guard.file_record(cleanup_path)
    guard.require(first.returncode == 0, "capacity preflight did not admit the fixed capacity")
    require_closed_lifecycle(first, first_lifecycle)
    cleanup_ran = (bool(first_report.get("removed")) or bool(first_report.get("cleanup_attempts"))
                   or cleanup_status == "completed" or cleanup_status == "incomplete")
    if cleanup_ran:
        guard.require(cleanup_status == "completed" or cleanup_status == "incomplete",
                      "capacity preflight cleanup status is not recoverable")
    else:
        guard.require(cleanup_status == "not-needed",
                      "capacity preflight cleanup status is not recoverable")
    if cleanup_ran:
        guard.require(first.returncode == 0, "capacity preflight did not admit the fixed capacity")
        result, lifecycle_path, lifecycle, report, started_ns, ended_ns = observe(
            attempt / "capacity-preflight-refresh")
        fresh_record = evidence_record(result, lifecycle_path, lifecycle, report, started_ns, ended_ns)
        refresh_path = attempt / "capacity-preflight-refresh-observation.json"
        guard.exclusive_json(refresh_path,
                             {"schema": "krr-native-cli-capacity-preflight-refresh-v1",
                              **fresh_record})
        refresh_observation = guard.file_record(refresh_path)
        guard.require(result.returncode == 0,
                      "capacity preflight did not admit the fixed capacity")
        require_closed_lifecycle(result, lifecycle)
        guard.require(not report.get("removed") and not report.get("cleanup_attempts"),
                      "fresh capacity preflight attempted cleanup")
        guard.require(report.get("cleanup_status") == "not-needed",
                      "fresh capacity preflight cleanup status is unknown")
    else:
        result, lifecycle_path, lifecycle, report = first, first_lifecycle_path, first_lifecycle, first_report
        started_ns, ended_ns = first_started_ns, first_ended_ns
        guard.require(result.returncode == 0 and lifecycle.get("exitCode") == 0,
                      "capacity preflight did not admit the fixed capacity")
    guard.validate_capacity_report(report, root / "target")
    evidence = {"schema": "krr-native-cli-capacity-preflight-v1",
                **evidence_record(result, lifecycle_path, lifecycle, report, started_ns, ended_ns),
                "cleanupObservation": cleanup_observation,
                "refreshObservation": refresh_observation}
    guard.exclusive_json(attempt / "capacity-preflight.json", evidence)
    return evidence


def _prebuild_rust_facts_host(bindings: Bindings) -> owner.NativeCliCommandEvidence:
    environment = bindings.child_environment
    guard.require(guard.REQUEST_ENV not in environment and guard.REQUEST_SHA_ENV not in environment,
                  "Rust facts host prebuild must precede the CLI request")
    rtk = _text(bindings.tools["rtk"]["path"], "RTK path")
    just = _text(bindings.tools["just"]["path"], "Just path")
    return bindings.run([rtk, "proxy", just, "rust-facts-host-prebuild"],
                        "build", "rust-facts-host-prebuild")


def completed_observation(value: JsonObject, request: JsonObject, build: JsonObject,
                          lifecycle: JsonObject, admission: guard.Admission) -> None:
    guard.exact(value, {"schema", "status", "diagnosticOnly", "accepted", "attemptToken", "request",
        "sourceManifest", "sourceDigest", "metadata", "metadataObservation", "expectedPackage", "expectedTarget",
        "cargo", "rustc", "cargoLauncher", "rustcLauncher", "rustupResolutionQueries",
        "guardCodePins", "pinsBefore", "pinsAfter",
        "capacityBefore", "capacityAfter", "capacityProtection",
        "ownedCargo", "binaryBefore", "binaryAfter", "artifact", "buildFinished", "errors",
        "externalGroupClosureRequired", "scope"}, "guard observation")
    guard.require(value.get("schema") == guard.OBSERVATION_SCHEMA
                  and value.get("status") == "cargo-completed" and value.get("accepted") is False
                  and value.get("diagnosticOnly") is True and value.get("externalGroupClosureRequired") is True
                  and value.get("errors") == [], "guard observation is not completed diagnostic evidence")
    guard.require(value["attemptToken"] == request["attemptToken"]
                  and value["request"] == request["requestPin"], "guard request binding differs")
    for key in ("sourceManifest", "sourceDigest", "metadata", "metadataObservation", "expectedPackage",
                "expectedTarget", "cargo", "rustc", "cargoLauncher", "rustcLauncher", "guardCodePins"):
        guard.require(value[key] == request[key], "guard observation binding differs: " + key)
    cargo = _object(value["ownedCargo"], "owned Cargo")
    identity = _object(lifecycle["identity"], "outer identity")
    guard.require(cargo.get("actualExit") == 0 and cargo.get("reaped") is True
                  and cargo.get("rawFdsClosed") is True and cargo.get("errors") == []
                  and cargo["argv"] == [*guard.cargo_proxy_command(guard.environment_record(request["environment"])),
                                         *guard.BUILD_FLAGS, guard.JSON_FLAG]
                  and cargo["attemptToken"] == request["attemptToken"]
                  and cargo["environment"] == request["environment"]
                  and cargo["environmentSha256"] == request["environmentSha256"]
                  and cargo["cwd"] == request["cwd"], "owned Cargo record differs")
    guard.require(cargo["pid"] != build["pid"] and cargo["parentPid"] != cargo["pid"]
                  and cargo["sessionId"] == cargo["processGroupId"] == identity["pid"]
                  and int(str(build["startedNs"])) <= int(str(cargo["startedNs"]))
                  <= int(str(cargo["endedNs"])) <= int(str(build["endedNs"])), "Cargo outer ownership/time differs")
    guard.require(value["pinsBefore"] == value["pinsAfter"], "guard input endpoints differ")
    for key in ("stdout", "stderr"):
        guard.validate_record(cargo[key])
    raw_pin = _object(cargo["stdout"], "Cargo stdout")
    artifact, finished = guard.cargo_events(Path(_text(raw_pin["path"], "Cargo stdout path")).read_bytes(), admission)
    guard.require(value["artifact"] == artifact and value["buildFinished"] == finished,
                  "guard artifact differs from unique raw Cargo stream")
    for key in ("capacityBefore", "capacityAfter"):
        pin = guard.validate_record(value[key])
        guard.validate_capacity_report(guard.parse_json(Path(_text(pin["path"], "capacity path")).read_bytes()),
                                       Path(_text(request["target"], "target")))


def validate_build_admission(root: Path, environment: dict[str, str]) -> guard.Admission:
    command = [*guard.cargo_proxy_command(environment), *guard.BUILD_FLAGS]
    return guard.validate_request(root, command, environment)


def produce(root: Path, expected_source: Path, expected_sha: str, registry: Path,
            registry_sha: str, cargo: Path, rustc: Path, runtime: Path) -> Path:
    nonce = secrets.token_hex(32)
    attempt = root / "tmp/native-cli-managed-build" / nonce
    attempt.mkdir(parents=True, exist_ok=False)
    bindings = construct_bindings(root, attempt, expected_source, expected_sha, registry, registry_sha, cargo, rustc)
    initial = bindings.snapshot()
    _capacity_preflight(bindings, root, attempt)
    host_prebuild = _prebuild_rust_facts_host(bindings)
    host_receipt = guard.file_record(root / "tmp/quality/rust-facts-host/current/receipt.json")
    bindings = construct_bindings(root, attempt, expected_source, expected_sha, registry, registry_sha, cargo, rustc)
    after_preflight = bindings.snapshot()
    for key in ("source", "tools", "toolLaunchers", "control", "environment", "environmentSha256",
                "toolchainFiles", "toolchainAbsent"):
        guard.require(after_preflight[key] == initial[key], "inputs changed during capacity preflight: " + key)
    before = after_preflight
    source_files = [_object(row, "source pin") for row in _array(before["source"], "source pins")]
    source_manifest = guard.exclusive_json(attempt / "source-manifest.json", bindings.expected)
    runtime_pin = guard.file_record(runtime)
    guard.require(runtime_pin in source_files, "selected runtime absent from admitted source")
    rtk = _text(bindings.tools["rtk"]["path"], "RTK path")
    just = _text(bindings.tools["just"]["path"], "Just path")
    probes: list[JsonObject] = []
    for number, (name, flags) in enumerate((("cargo", ["--version"]), ("rustc", ["-Vv"])), 3):
        probe = bindings.run([rtk, "proxy", _text(bindings.tool_launchers[name]["path"], name), *flags],
                             "tool-query", f"probe-{number}-{name}")
        probes.append({"observation": probe.observation, "lifecycle": probe.lifecycle})
    cargo_launcher = _text(bindings.tool_launchers["cargo"]["path"], "Cargo launcher path")
    metadata_run = bindings.run([rtk, "proxy", cargo_launcher,
                                 "metadata", "--frozen", "--format-version", "1",
                                 "--no-deps", "--manifest-path", str(root / "Cargo.toml")],
                                "metadata", "probe-5-cargo-metadata")
    metadata_record = dict(metadata_run.observation)
    for key in ("stdout", "stderr"):
        pin = guard.validate_record(metadata_record[key])
        destination = attempt / ("metadata.json" if key == "stdout" else "metadata.stderr.raw")
        with destination.open("xb") as handle:
            handle.write(Path(_text(pin["path"], "metadata raw path")).read_bytes())
        metadata_record[key] = guard.file_record(destination)
    metadata_pin = _object(metadata_record["stdout"], "metadata stdout")
    metadata_observation = guard.exclusive_json(attempt / "metadata-observation.json", metadata_record)
    metadata = guard.parse_json((attempt / "metadata.json").read_bytes())
    packages = [_object(item, "metadata package") for item in _array(metadata["packages"], "packages")]
    selected = [item for item in packages if item.get("name") == "katana-render-runtime-cli"]
    guard.require(len(selected) == 1, "CLI metadata package must be unique")
    package = selected[0]
    issued = time.monotonic_ns()
    request: JsonObject = {"schema": guard.REQUEST_SCHEMA, "attemptToken": nonce, "cwd": str(root),
        "target": str(root / "target"), "binaryPath": str(root / "target/debug/krr"),
        "issuedMonotonicNs": str(issued), "expiresMonotonicNs": str(issued + guard.REQUEST_TTL_NS),
        "cargo": bindings.tools["cargo"], "rustc": bindings.tools["rustc"],
        "cargoLauncher": bindings.tool_launchers["cargo"],
        "rustcLauncher": bindings.tool_launchers["rustc"], "sourceManifest": source_manifest,
        "sourceDigest": bindings.expected["digest"], "metadata": metadata_pin,
        "metadataObservation": metadata_observation,
        "expectedPackage": {"id": package["id"], "name": package["name"], "manifestPath": package["manifest_path"]},
        "expectedTarget": {"name": "krr", "kind": ["bin"],
                           "srcPath": str(root / "crates/katana-render-runtime-cli/src/main.rs")},
        "environment": bindings.child_environment,
        "environmentSha256": guard.environment_digest(bindings.child_environment),
        "guardCodePins": [guard.file_record(root / "scripts/build" / name) for name in
                          ("cargo_capacity_guard.py", "cargo_capacity_guard_lock.py",
                           "cargo_capacity_guard_native_cli_observation.py")],
        "capacityAdmission": bindings.capacity}
    request_pin = guard.exclusive_json(attempt / "request.json", request)
    child = {**bindings.child_environment, guard.REQUEST_ENV: str(attempt / "request.json"),
             guard.REQUEST_SHA_ENV: _text(request_pin["sha256"], "request SHA")}
    admission = validate_build_admission(root, child)
    guard.require(bindings.snapshot() == before, "inputs changed before sole build")
    outer_environment = {**bindings.environment, guard.REQUEST_ENV: str(attempt / "request.json"),
                         guard.REQUEST_SHA_ENV: _text(request_pin["sha256"], "request SHA")}
    build = bindings.run([rtk, "proxy", just, "krr-build"],
                         "build", "probe-6-native-cli-build", environment=outer_environment)
    observation_pin = guard.file_record(attempt / "observation.json")
    observation = guard.parse_json((attempt / "observation.json").read_bytes())
    lifecycle = guard.parse_json(Path(_text(build.lifecycle["path"], "build lifecycle")).read_bytes())
    completed_observation(observation, {**request, "requestPin": request_pin}, build.observation, lifecycle, admission)
    guard.require(observation["ownedCargo"] != metadata_record
                  and _object(observation["ownedCargo"], "Cargo")["pid"] != metadata_record["pid"],
                  "metadata and build Cargo PID differ")
    after = bindings.snapshot()
    guard.require(before == after, "inputs changed after outer group closure")
    binary = guard.identity(root / "target/debug/krr")
    guard.require(binary == observation["binaryAfter"], "binary changed after guard observation")
    receipt = {"schema": SCHEMA, "status": "completed", "accepted": True, "attemptToken": nonce,
        "cwd": str(root), "expectedSource": guard.file_record(expected_source),
        "expectedProtectedRegistry": guard.file_record(registry), "request": request_pin,
        "capacityPreflight": guard.file_record(attempt / "capacity-preflight.json"),
        "observation": observation_pin, "metadataLifecycle": metadata_run.lifecycle,
        "hostPrebuild": {"observation": host_prebuild.observation, "lifecycle": host_prebuild.lifecycle,
                         "receipt": host_receipt},
        "buildLifecycle": build.lifecycle, "buildObservation": build.observation,
        "snapshotBefore": before, "snapshotAfter": after, "binary": binary,
        "runtimeAssetPins": [runtime_pin], "toolProbes": probes,
        "scope": {"sourceEndpointsOnly": True, "effectiveCompilerReadsProved": False,
                  "registryAtomicityProved": False, "installedDependencyClosureProved": False, "descendantClosureProved": False}}
    output = attempt / "receipt.json"
    guard.exclusive_json(output, receipt)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("root", "expected-source", "protected-registry", "cargo", "rustc", "runtime"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--expected-source-sha256", required=True)
    parser.add_argument("--protected-registry-sha256", required=True)
    args = parser.parse_args()
    output = produce(args.root, args.expected_source, args.expected_source_sha256,
                     args.protected_registry, args.protected_registry_sha256, args.cargo, args.rustc, args.runtime)
    print(json.dumps({"receipt": str(output)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
