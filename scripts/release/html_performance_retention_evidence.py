"""Transport and authenticate candidate retention observations."""
from __future__ import annotations

import json
import stat
from pathlib import Path

import html_performance_retention_capture as capture_domain
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate

SCHEMA = "krr-html-retention-observation-v1"


def validate_sample(expected: gate.JsonObject, cycle: int, raw: bytes) -> None:
    """Validate a sample using the existing report contract, without asserting wrapper provenance."""
    plan_ref = gate._object(expected.get("operationPlanRef"), "retention operation plan")
    plan_path = Path(gate._text(plan_ref.get("path"), "retention plan path"))
    gate.require(_pin(plan_path).get("sha256") == plan_ref.get("sha256"), "retention plan bytes differ")
    _validate_sample(expected, cycle, raw, gate.read_json(plan_path))


def _validate_sample(expected: gate.JsonObject, cycle: int, raw: bytes, plan: gate.JsonObject) -> None:
    sample = gate._object(json.loads(raw, object_pairs_hook=gate.unique_object), "retention sample")
    gate.require(type(sample.get("sample")) is int and sample.get("sample") == cycle % 5
                 and sample.get("viewport") == expected.get("viewport"),
                 "retention sample identity differs")
    arms = gate._object(expected.get("arms"), "retention arms")
    candidate = gate._object(arms.get("candidate"), "retention candidate")
    provenance = gate._object(expected.get("provenance"), "retention provenance")
    catalog = gate._object(expected.get("catalogCondition"), "retention catalog")
    report: gate.JsonObject = {
        "arm": "candidate", "source_sha256": expected.get("fixture_sha256"),
        "viewport": expected.get("viewport"), "operation_plan_sha256": expected.get("operation_plan_sha256"),
        "operation": expected.get("operation"), "expected_dom_state_sha256": expected.get("expected_dom_state_sha256"),
        "provenance": {"binary_sha256": candidate.get("binary_sha256"),
                       "source_snapshot_sha256": candidate.get("source_snapshot_sha256"),
                       "deps_lock_sha256": candidate.get("lock_sha256"), **provenance},
        "catalog_condition": catalog, "condition_id": catalog.get("condition_id"),
        "case_id": catalog.get("case_id"), "state": catalog.get("state"),
        "operation_kind": catalog.get("operation_kind"),
        "operation_evidence_ref": expected.get("operationEvidenceRef"),
        "sample_count": 1, "samples": [sample],
    }
    gate._validate_report(report, expected)
    gate.validate_measurement_sample(sample, expected, plan, candidate=True)


def _metrics(path: Path, pid: int) -> list[int]:
    rows: list[int] = []
    identity: float | None = None
    previous = -1
    with path.open("rb") as stream:
        for cycle in range(capture_domain.TOTAL_CYCLES):
            line = stream.readline(capture_domain.MAX_LINE_BYTES + 1)
            gate.require(bool(line) and len(line) <= capture_domain.MAX_LINE_BYTES and line.endswith(b"\n"),
                         "retention memory row missing or too large")
            row = gate._object(json.loads(line, object_pairs_hook=gate.unique_object), "retention memory row")
            gate.require(set(row) == {"schema", "method", "psutilVersion", "cycle", "warmup", "pid", "createTime",
                                      "rss", "vms", "monotonicNs"}
                         and row.get("schema") == "krr-html-retention-memory-v1"
                         and row.get("method") == "psutil.Process.memory_info" and row.get("psutilVersion") == "7.2.2"
                         and type(row.get("cycle")) is int and row.get("cycle") == cycle
                         and type(row.get("warmup")) is bool and row.get("warmup") == (cycle < 5)
                         and type(row.get("pid")) is int and row.get("pid") == pid,
                         "retention memory identity differs")
            created = gate._nonnegative_number(row.get("createTime"), "retention process creation time")
            gate.require(created > 0 and (identity is None or created == identity), "retention process identity differs")
            identity = created
            monotonic = gate._integer(row.get("monotonicNs"), "retention monotonic clock")
            rss = gate._integer(row.get("rss"), "retention RSS")
            vms = gate._integer(row.get("vms"), "retention VMS")
            gate.require(monotonic > previous and rss > 0 and vms > 0, "retention memory values differ")
            previous = monotonic
            if cycle >= capture_domain.WARMUP_CYCLES:
                rows.append(rss)
        gate.require(not stream.read(1), "extra retention memory row")
    return rows


def _pin(path: Path) -> gate.JsonObject:
    resolved = path.resolve(strict=True)
    gate.require(path.is_absolute() and path == resolved and path.is_file() and not path.is_symlink(),
                 "retention evidence file identity differs")
    return {"path": str(path), "sha256": gate.sha256(path)}


def _native_refs(lifecycle: gate.JsonObject) -> list[gate.JsonObject]:
    native = gate._object(lifecycle.get("nativeRawFdClosure"), "retention FD closure")
    return [gate._object(native.get(name), "retention native reference")
            for name in ("stdout", "stderr", "toolBefore")]


def write_retention_evidence(output: Path, *, capture: capture_domain.RetentionCapture,
                             expected: gate.JsonObject) -> tuple[gate.JsonObject, list[gate.JsonObject]]:
    """Write a pinned observation and return every referenced transport file."""
    lifecycle = gate.read_json(capture.lifecycle)
    raw = gate._objects(lifecycle.get("raw"), "retention raw streams")
    gate.require(len(raw) == 2 and lifecycle.get("runtimeExecution") == capture.runtime_execution,
                 "retention lifecycle differs")
    condition = gate._object(expected.get("catalogCondition"), "retention condition")
    candidate = gate._object(gate._object(expected.get("arms"), "retention arms").get("candidate"),
                             "retention candidate")
    refs = [_pin(capture.directory / "command.json"), _pin(capture.directory / "memory.jsonl"),
            _pin(capture.lifecycle), *raw, *_native_refs(lifecycle)]
    for ref in refs:
        path = Path(gate._text(ref.get("path"), "retention transport path"))
        gate.require(_pin(path).get("sha256") == ref.get("sha256"), "retention transport bytes differ")
    native = gate._object(lifecycle.get("nativeRawFdClosure"), "retention FD closure")
    raw_ids = gate._objects(native.get("rawIdentities"), "retention raw identities")
    gate.require(len(raw_ids) == 2, "retention raw identity count differs")
    for item, ref in zip(raw_ids, raw, strict=True):
        current = Path(gate._text(ref.get("path"), "retention raw path")).stat()
        gate.require(item.get("device") == current.st_dev and item.get("inode") == current.st_ino,
                     "retention raw identity changed after closure")
    document: gate.JsonObject = {
        "schema": SCHEMA, "conditionId": condition.get("condition_id"),
        "fixtureSha256": expected.get("fixture_sha256"),
        "planSha256": expected.get("operation_plan_sha256"),
        "candidateBinarySha256": candidate.get("binary_sha256"),
        "argv": lifecycle.get("argv"), "commandRef": refs[0], "metricsRef": refs[1],
        "lifecycleRef": refs[2], "stdoutRef": refs[3], "stderrRef": refs[4],
        "runtimeExecution": capture.runtime_execution,
        "retainedRssBytes": _metrics(capture.directory / "memory.jsonl",
                                     gate._integer(gate._object(lifecycle.get("identity"),
                                                               "retention lifecycle identity").get("pid"),
                                                   "retention PID")),
    }
    gate.require(output.is_absolute() and not output.exists(), "retention output already exists or is relative")
    for ref in refs:
        gate.require(_pin(Path(gate._text(ref.get("path"), "retention transport path"))).get("sha256")
                     == ref.get("sha256"), "retention transport changed during projection")
    with output.open("xb") as stream:
        stream.write(json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode() + b"\n")
    document_pin = _pin(output)
    return document_pin, [document_pin, *refs]


def _check_lifecycle(bundle: gate.Bundle, expected: gate.JsonObject,
                     document: gate.JsonObject) -> int:
    lifecycle = bundle.json(document.get("lifecycleRef"))
    identity = gate._object(lifecycle.get("identity"), "retention lifecycle identity")
    pid = gate._integer(identity.get("pid"), "retention PID")
    argv = gate._array(lifecycle.get("argv"), "retention argv")
    gate.require(len(argv) == 8, "retention argv count differs")
    unused_output = gate._text(argv[3], "retention unused output")
    catalog = gate._object(expected.get("catalogCondition"), "retention catalog")
    plan = gate._object(expected.get("operationPlanRef"), "retention plan")
    viewport = gate._object(expected.get("viewport"), "retention viewport")
    endpoints = gate._object(expected.get("endpointRefs"), "retention endpoints")
    context = bundle.json(endpoints.get("before"))
    candidate = gate._object(gate._object(expected.get("arms"), "retention arms").get("candidate"),
                             "retention candidate")
    pair = gate._object(expected.get("sourcePair"), "retention source pair")
    is_v3 = pair.get("schema") == "krr-html-performance-source-epoch-v3"
    if is_v3:
        workers = gate._object(context.get("systemWorkers"), "retention workers")
        worker = gate._object(gate._object(workers.get("candidate"), "retention worker").get("worker"),
                              "retention worker pin")
        gate.require(pair.get("beforeContextRef") == endpoints.get("before")
                     and pair.get("endContextRef") == endpoints.get("end"),
                     "retention epoch endpoints differ")
        bundle.json(pair.get("epochContextRef"))
    else:
        proof = bundle.json(candidate.get("build_artifact_ref"))
        gate.require(proof.get("schema") == "krr-html-performance-build-artifact-v3"
                     and proof.get("arm") == "candidate" and proof.get("buildExit") == 0
                     and proof.get("sourceBeforeRef") == pair.get("before")
                     and proof.get("sourceSnapshotRef") == pair.get("snapshot"),
                     "legacy retention build role differs")
        worker = gate._object(proof.get("frozenBinary"), "legacy retention worker")
    gate.require(plan.get("sha256") == expected.get("operation_plan_sha256")
                 and worker.get("sha256") == candidate.get("binary_sha256"),
                 "retention source or worker differs")
    bundle.json(endpoints.get("end"))
    fixed_pair = gate.fixed_source_pair(expected)
    bundle.json(fixed_pair.get("before"))
    bundle.json(fixed_pair.get("snapshot"))
    worker_path = bundle.file(worker)
    bundle.file(plan)
    bundle.file({"path": catalog.get("fixture_path"), "sha256": expected.get("fixture_sha256")})
    gate.require(lifecycle.get("schema") == "krr-performance-command-lifecycle-v1"
                 and lifecycle.get("role") == "worker" and lifecycle.get("exitCode") == 0
                 and lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
                 and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None
                 and pid > 0 and identity.get("sessionId") == pid and identity.get("processGroupId") == pid
                 and document.get("argv") == argv
                 and argv[0] == worker.get("path") and argv[1] == "--retention"
                 and argv[2] == catalog.get("fixture_path") and Path(unused_output).is_absolute()
                 and argv[4] == plan.get("path")
                 and argv[5:] == [str(viewport["width"]), str(viewport["height"]), str(viewport["scale"])],
                 "retention lifecycle/argv differs")
    command = bundle.json(document.get("commandRef"))
    gate.require(command == {"argv": argv, "kind": "retention-observation"},
                 "retention command differs")
    execution = gate._object(lifecycle.get("runtimeExecution"), "retention execution")
    gate.require(document.get("runtimeExecution") == execution, "retention execution differs")
    binary_before = gate._object(execution.get("binaryIdentityBefore"), "retention binary before")
    binary_after = gate._object(execution.get("binaryIdentityAfter"), "retention binary after")
    attempt: gate.JsonObject = {"workerStarted": True, "binarySha256Before": binary_before.get("sha256"),
        "binarySha256After": binary_after.get("sha256"), "runtimeExecution": execution}
    if is_v3:
        gate.validate_v3_worker_runtime_execution(expected, context, "candidate", attempt, lifecycle)
    else:
        _check_legacy_runtime(bundle, expected, context, execution, worker, worker_path)
    _check_native_closure(bundle, lifecycle, document)
    return pid


def _check_legacy_runtime(bundle: gate.Bundle, expected: gate.JsonObject, context: gate.JsonObject,
                          execution: gate.JsonObject, worker: gate.JsonObject, worker_path: Path) -> None:
    gate.require(set(execution) == {"schema", "binaryIdentityBefore", "binaryIdentityAfter",
                    "actualEnvironmentSha256", "keyNames", "keyNamesSha256", "nonMetadataEnvironmentSha256",
                    "metadataKeys", "metadataValuesSha256"}
                 and execution.get("schema") == "krr-html-worker-runtime-execution-v1",
                 "legacy retention runtime execution differs")
    pair = gate.fixed_source_pair(expected)
    source = bundle.json(pair.get("before"))
    bundle.json(pair.get("end"))
    snapshot = gate._object(pair.get("snapshot"), "legacy retention snapshot")
    bundle.json(snapshot)
    root = Path(gate._text(source.get("root"), "legacy source root"))
    gate.require(root.is_absolute(), "legacy source root differs")
    before = gate._object(execution.get("binaryIdentityBefore"), "legacy binary before")
    after = gate._object(execution.get("binaryIdentityAfter"), "legacy binary after")
    gate.require(before == after and set(before) == {"path", "sha256", "size", "device", "inode", "mode", "mtimeNs", "ctimeNs"}
                 and before.get("path") == worker.get("path") and before.get("sha256") == worker.get("sha256")
                 and type(before.get("size")) is int and before.get("size") == worker_path.stat().st_size,
                 "legacy retention binary differs")
    mode = gate._integer(before.get("mode"), "legacy binary mode")
    gate.require(stat.S_ISREG(mode) and mode & 0o111 != 0, "legacy binary mode differs")
    for key in ("device", "inode"):
        gate.require(gate._integer(before.get(key), "legacy binary identity") >= 0, "legacy binary identity differs")
    for key in ("mtimeNs", "ctimeNs"):
        value = gate._text(before.get(key), "legacy binary timestamp")
        gate.require(value.isascii() and value.isdigit() and str(int(value)) == value,
                     "legacy binary timestamp differs")
    _check_legacy_environment(expected, context, execution, root, snapshot)


def _check_legacy_environment(expected: gate.JsonObject, context: gate.JsonObject,
                              execution: gate.JsonObject, root: Path, snapshot: gate.JsonObject) -> None:
    provenance = gate._object(expected.get("provenance"), "legacy retention provenance")
    endpoint = gate._object(gate._object(expected.get("endpointRefs"), "legacy endpoints").get("before"),
                            "legacy before endpoint")
    tool = gate._object(context.get("toolContext"), "legacy tool context")
    values: gate.JsonObject = {"KRR_PERF_RUSTC": provenance.get("rustc"),
        "KRR_PERF_CARGO": provenance.get("cargo"),
        "KRR_PERF_ENVIRONMENT_SHA256": endpoint.get("sha256"),
        "KRR_PERF_LOCK_PATH": str(root / gate.HARNESS_ROOT / "Cargo.lock"),
        "KRR_PERF_SOURCE_SNAPSHOT_PATH": snapshot.get("path")}
    gate.require(all(isinstance(value, str) and bool(value) for value in values.values()),
                 "legacy metadata values differ")
    gate.require(provenance.get("environment_sha256") == endpoint.get("sha256"),
                 "legacy measurement endpoint differs")
    gate.validate_worker_runtime_environment(
        execution, non_metadata_environment_sha=gate._text(tool.get("environmentSha256"), "legacy environment SHA"),
        metadata_values=values,
    )


def _check_native_closure(bundle: gate.Bundle, lifecycle: gate.JsonObject,
                          document: gate.JsonObject) -> None:
    raw = gate._objects(lifecycle.get("raw"), "retention lifecycle raw")
    gate.require(len(raw) == 2 and raw == [document.get("stdoutRef"), document.get("stderrRef")],
                 "retention raw references differ")
    for ref in raw:
        bundle.file(ref)
    native = gate._object(lifecycle.get("nativeRawFdClosure"), "retention FD closure")
    gate.require(set(native) == {"schema", "toolBefore", "toolAfter", "argv", "cwd", "attemptToken",
                 "environmentSha256", "environmentProjection", "identity", "actualExit", "startedNs",
                 "endedNs", "reaped", "groupClosed", "rawFdsClosed", "runLimitSeconds",
                 "cleanupLimitSeconds", "stdout", "stderr", "rawIdentities", "holders", "descendantClosureProved"}
                 and native.get("schema") == "krr-native-raw-fd-closure-v2"
                 and type(native.get("actualExit")) is int and native.get("actualExit") in (0, 1)
                 and native.get("reaped") is True
                 and native.get("groupClosed") is True and native.get("rawFdsClosed") is True
                 and native.get("holders") == [] and native.get("toolBefore") == native.get("toolAfter"),
                 "retention FD closure differs")
    native_refs = _native_refs(lifecycle)
    native_stdout, native_stderr, tool = [bundle.file(ref) for ref in native_refs]
    tool_pin = native_refs[2]
    tool_path = gate._text(tool_pin.get("path"), "retention FD observer path")
    native_identity = gate._object(native.get("identity"), "retention FD observer identity")
    native_pid = gate._integer(native_identity.get("pid"), "retention FD observer PID")
    execution = gate._object(lifecycle.get("runtimeExecution"), "retention execution")
    projection = gate._object(native.get("environmentProjection"), "retention FD observer environment projection")
    environment_keys = ("actualEnvironmentSha256", "keyNames", "keyNamesSha256",
                        "nonMetadataEnvironmentSha256", "metadataKeys", "metadataValuesSha256")
    gate.require(set(projection) == set(environment_keys)
                 and all(execution.get(key) == projection.get(key) for key in environment_keys),
                 "retention observed environment projection differs")
    source_paths = [gate._text(item.get("path"), "retention raw path") for item in raw]
    gate.require(tool_path in ("/usr/sbin/lsof", "/usr/bin/lsof")
                 and type(tool_pin.get("size")) is int and tool.stat().st_size == tool_pin.get("size")
                 and native.get("argv") == [tool_path, "-nP", "-FpgfatiD", "--", *source_paths]
                 and native.get("cwd") == lifecycle.get("cwd")
                 and native_pid > 0 and native_identity.get("sessionId") == native_pid
                 and native_identity.get("processGroupId") == native_pid
                 and native.get("environmentSha256") == execution.get("actualEnvironmentSha256")
                 and not native_stderr.read_bytes(), "retention FD observer differs")
    raw_ids = gate._objects(native.get("rawIdentities"), "retention raw identities")
    gate.require(len(raw_ids) == 2, "retention raw identity count differs")
    identities = [(gate._integer(item.get("device"), "retention raw device"),
                   gate._integer(item.get("inode"), "retention raw inode")) for item in raw_ids]
    gate.require(producer._writable_raw_holders(native_stdout.read_bytes(), identities) == [],
                 "retention writable raw holder remains")


def _check_samples(path: Path, expected: gate.JsonObject, plan: gate.JsonObject) -> None:
    with path.open("rb") as stream:
        for cycle in range(capture_domain.TOTAL_CYCLES):
            line = stream.readline(capture_domain.MAX_LINE_BYTES + 1)
            prefix = f"SAMPLE {cycle} ".encode("ascii")
            gate.require(len(line) <= capture_domain.MAX_LINE_BYTES and line.endswith(b"\n")
                         and line.startswith(prefix), "retention sample order differs")
            _validate_sample(expected, cycle, line[len(prefix):-1], plan)
            gate.require(stream.readline(128) == f"READY {cycle}\n".encode("ascii"),
                         "retention barrier order differs")
        gate.require(not stream.read(1), "extra retention output")


def read_retained_rss(bundle: gate.Bundle, expected: gate.JsonObject) -> list[float]:
    """Authenticate the raw 30-cycle observation and return only the 25 measured RSS values."""
    document = bundle.json(expected.get("memoryRetentionRef"))
    catalog = gate._object(expected.get("catalogCondition"), "retention catalog")
    candidate = gate._object(gate._object(expected.get("arms"), "retention arms").get("candidate"),
                             "retention candidate")
    condition_id = gate._text(catalog.get("condition_id"), "retention condition id")
    gate.require(bool(condition_id), "retention condition id missing")
    for value in (expected.get("fixture_sha256"), expected.get("operation_plan_sha256"),
                  candidate.get("binary_sha256")):
        gate.digest(value)
    gate.require(set(document) == {"schema", "conditionId", "fixtureSha256", "planSha256",
                 "candidateBinarySha256", "argv", "commandRef", "metricsRef", "lifecycleRef",
                 "stdoutRef", "stderrRef", "runtimeExecution", "retainedRssBytes"}
                 and document.get("schema") == SCHEMA and document.get("conditionId") == condition_id
                 and document.get("fixtureSha256") == expected.get("fixture_sha256")
                 and document.get("planSha256") == expected.get("operation_plan_sha256")
                 and document.get("candidateBinarySha256") == candidate.get("binary_sha256"),
                 "retention source identity differs")
    pid = _check_lifecycle(bundle, expected, document)
    _check_samples(bundle.file(document.get("stdoutRef")), expected,
                   bundle.json(expected.get("operationPlanRef")))
    values = _metrics(bundle.file(document.get("metricsRef")), pid)
    projection = gate._array(document.get("retainedRssBytes"), "retention RSS projection")
    gate.require(all(type(value) is int for value in projection) and projection == values,
                 "retention RSS projection differs")
    return [float(value) for value in values]
