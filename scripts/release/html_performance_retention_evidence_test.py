from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path
from typing import cast

import html_performance_retention_capture as capture
import html_performance_retention_evidence as evidence
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate

SECRET_MARKER = "retention-unit-secret-7a80d2"


class RetentionEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.rows: list[gate.JsonObject] = [
            {"schema": "krr-html-retention-memory-v1", "method": "psutil.Process.memory_info",
             "psutilVersion": "7.2.2", "cycle": cycle, "warmup": cycle < 5, "pid": 123,
             "createTime": 100.0, "rss": 1000 + cycle, "vms": 2000 + cycle,
             "monotonicNs": 10000 + cycle}
            for cycle in range(30)
        ]

    def write_rows(self, rows: list[gate.JsonObject]) -> Path:
        path = self.root / "memory.jsonl"
        path.write_bytes(b"".join(json.dumps(row).encode() + b"\n" for row in rows))
        return path

    def test_metrics_keep_only_measured_cycles(self) -> None:
        self.assertEqual(evidence._metrics(self.write_rows(self.rows), 123), list(range(1005, 1030)))

    def test_metrics_reject_mutations_and_missing_or_extra_rows(self) -> None:
        mutations: tuple[tuple[str, object], ...] = (
            ("cycle", 6), ("warmup", True), ("pid", 999), ("createTime", 101.0),
            ("rss", 0), ("vms", -1), ("monotonicNs", 1), ("psutilVersion", "0"),
        )
        for key, value in mutations:
            with self.subTest(key=key):
                rows = copy.deepcopy(self.rows)
                rows[5][key] = value
                with self.assertRaises(ValueError):
                    evidence._metrics(self.write_rows(rows), 123)
        for rows in (self.rows[:-1], [*self.rows, self.rows[-1]]):
            with self.assertRaises(ValueError):
                evidence._metrics(self.write_rows(rows), 123)

    def test_writer_pins_originals_and_does_not_overwrite(self) -> None:
        directory = self.root / "capture"
        directory.mkdir()
        metrics = directory / "memory.jsonl"
        metrics.write_bytes(self.write_rows(self.rows).read_bytes())
        command = directory / "command.json"
        command.write_text('{}', encoding="utf8")
        files = [directory / name for name in ("stdout.raw", "stderr.raw", "fd-stdout.raw", "fd-stderr.raw", "lsof")]
        for path in files:
            path.write_bytes(b"fixture")
        pins = [{"path": str(path), "sha256": gate.sha256(path)} for path in files]
        lifecycle = directory / "lifecycle.json"
        runtime: gate.JsonObject = {"schema": "synthetic"}
        lifecycle.write_text(json.dumps({"identity": {"pid": 123}, "raw": pins[:2],
            "runtimeExecution": runtime, "argv": ["synthetic"],
            "nativeRawFdClosure": {"stdout": pins[2], "stderr": pins[3], "toolBefore": pins[4],
                "rawIdentities": [{"device": path.stat().st_dev, "inode": path.stat().st_ino}
                                  for path in files[:2]]}}),
            encoding="utf8")
        expected: gate.JsonObject = {"catalogCondition": {"condition_id": "example"},
            "arms": {"candidate": {"binary_sha256": "a" * 64}},
            "fixture_sha256": "b" * 64, "operation_plan_sha256": "c" * 64}
        item = capture.RetentionCapture(directory, lifecycle, runtime)
        output = self.root / "observation.json"
        pin, refs = evidence.write_retention_evidence(output, capture=item, expected=expected)
        self.assertEqual(pin["sha256"], gate.sha256(output))
        self.assertEqual(gate.read_json(output)["retainedRssBytes"], list(range(1005, 1030)))
        self.assertEqual({cast(str, ref["path"]) for ref in refs},
                         {str(output), str(command), str(metrics), str(lifecycle), *(str(path) for path in files)})
        with self.assertRaises(ValueError):
            evidence.write_retention_evidence(output, capture=item, expected=expected)

    def test_reader_rejects_missing_pin_before_any_acceptance(self) -> None:
        bundle_dir = self.root / "bundle"
        bundle_dir.mkdir()
        (bundle_dir / "bundle.json").write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1", "files": []}), encoding="utf8")
        with self.assertRaises((ValueError, KeyError)):
            evidence.read_retained_rss(gate.Bundle(bundle_dir), {})

    def test_reader_rejects_foreign_condition_input_binary_and_tampered_payload(self) -> None:
        expected: gate.JsonObject = {"catalogCondition": {"condition_id": "condition"},
            "fixture_sha256": "a" * 64, "operation_plan_sha256": "b" * 64,
            "arms": {"candidate": {"binary_sha256": "c" * 64}}}
        document: gate.JsonObject = {"schema": evidence.SCHEMA, "conditionId": "condition",
            "fixtureSha256": "a" * 64, "planSha256": "b" * 64, "candidateBinarySha256": "c" * 64,
            "argv": [], "commandRef": {}, "metricsRef": {}, "lifecycleRef": {},
            "stdoutRef": {}, "stderrRef": {}, "runtimeExecution": {}, "retainedRssBytes": []}
        for key in ("conditionId", "fixtureSha256", "planSha256", "candidateBinarySha256"):
            with self.subTest(key=key):
                changed = dict(document)
                changed[key] = "different"
                bundle, pin = self.bundle_document(changed, key)
                with self.assertRaisesRegex(ValueError, "retention source identity differs"):
                    evidence.read_retained_rss(bundle, {**expected, "memoryRetentionRef": pin})
        bundle, pin = self.bundle_document(document, "tamper")
        bundle.file(pin).write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            evidence.read_retained_rss(bundle, {**expected, "memoryRetentionRef": pin})

    def bundle_document(self, value: gate.JsonObject, name: str) -> tuple[gate.Bundle, gate.JsonObject]:
        directory = self.root / name
        directory.mkdir()
        payload = directory / "payload.json"
        payload.write_text(json.dumps(value), encoding="utf8")
        checksum = gate.sha256(payload)
        original = str(self.root / (name + "-original.json"))
        (directory / "bundle.json").write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1", "files": [{
                "originalPath": original, "relativePath": "payload.json", "sha256": checksum}]}), encoding="utf8")
        return gate.Bundle(directory), {"path": original, "sha256": checksum}

    def sample_fixture(self) -> tuple[gate.JsonObject, gate.JsonObject, gate.JsonObject]:
        viewport: gate.JsonObject = {"width": 100, "height": 100, "scale": 1.0}
        resized: gate.JsonObject = {"width": 75, "height": 100, "scale": 1.0}
        catalog: gate.JsonObject = {"condition_id": "condition", "case_id": "case",
                                    "state": "state", "operation_kind": "static"}
        operation_ref: gate.JsonObject = {"path": "/evidence", "sha256": "d" * 64}
        expected: gate.JsonObject = {"viewport": viewport, "catalogCondition": catalog,
            "operationEvidenceRef": operation_ref, "fixture_sha256": "a" * 64,
            "operation_plan_sha256": "b" * 64, "operation": "static",
            "expected_dom_state_sha256": "", "provenance": {"rustc": "r", "cargo": "c", "environment_sha256": "e"},
            "arms": {"candidate": {"binary_sha256": "f" * 64, "source_snapshot_sha256": "s" * 64,
                                    "lock_sha256": "l" * 64}}}
        def frame(view: gate.JsonObject) -> gate.JsonObject:
            return {"viewport": view, "pixel_bytes": cast(int, view["width"]) * 100 * 4, "scroll_y": 0}
        def measurement(view: gate.JsonObject) -> gate.JsonObject:
            return {"elapsed_ms": 1, "peak_rss_bytes": 1, "frame": frame(view)}
        phases: gate.JsonObject = {key: measurement(viewport) for key in gate.PHASES}
        phases["full_y"] = {**measurement(viewport), "max_frame_elapsed_ms": 1, "scroll_steps": 0,
                            "max_scroll": 0, "final_scroll_y": 0, "actual_bottom": True}
        phases["post_operation"] = {"measurement": measurement(viewport), "catalog_operation": {
            **catalog, "operation_evidence_sha256": "d" * 64, "delivery": "measured-close-and-new-session-open"}}
        phases["same_session_resize"] = {"measurement": measurement(resized), "resize_evidence": {
            "from_viewport": viewport, "to_viewport": resized, "generation_before": 1,
            "generation_after": 2, "real_raster_frame": True}}
        sample: gate.JsonObject = {"sample": 0, "viewport": viewport, "phases": phases,
            "catalog_condition": {"profile": "catalog-plan84", **catalog, "operation_evidence_sha256": "d" * 64}}
        plan: gate.JsonObject = {"operationStream": []}
        return expected, sample, plan

    def test_real_sample_validator_rejects_frame_time_and_operation_drift(self) -> None:
        expected, sample, plan = self.sample_fixture()
        evidence._validate_sample(expected, 0, json.dumps(sample).encode(), plan)
        mutations = (
            ("frame", ("cold_open", "frame", "pixel_bytes"), 1),
            ("time", ("cold_open", "elapsed_ms"), 3000),
            ("operation", ("post_operation", "catalog_operation", "condition_id"), "other"),
        )
        for label, keys, value in mutations:
            with self.subTest(label=label):
                changed = copy.deepcopy(sample)
                current = cast(gate.JsonObject, changed["phases"])
                for key in keys[:-1]:
                    current = cast(gate.JsonObject, current[key])
                current[keys[-1]] = value
                with self.assertRaises(ValueError):
                    evidence._validate_sample(expected, 0, json.dumps(changed).encode(), plan)

    def legacy_bundle(self, name: str, mutation: str = "") -> tuple[gate.Bundle, gate.JsonObject]:
        root = self.root / name
        root.mkdir()
        stored: dict[str, bytes] = {}

        def pin(label: str, data: bytes, *, logical: str | None = None) -> gate.JsonObject:
            path = logical or str(root / label)
            stored[path] = data
            import hashlib
            return {"path": path, "sha256": hashlib.sha256(data).hexdigest()}

        def object_pin(label: str, value: gate.JsonObject) -> gate.JsonObject:
            return pin(label, json.dumps(value, sort_keys=True, separators=(",", ":")).encode())

        expected, sample, plan = self.sample_fixture()
        fixture = pin("fixture.html", b"<p>synthetic</p>")
        plan_ref = object_pin("plan.json", plan)
        snapshot = object_pin("snapshot.json", {"schema": "synthetic-snapshot"})
        source_root = str(root / "foreign") if mutation == "source" else str(root)
        source = object_pin("source.json", {"root": source_root})
        end_source = object_pin("source-end.json", {"root": source_root})
        base_env = {"SYNTHETIC": SECRET_MARKER}
        before = object_pin("before.json", {"toolContext": {"environmentSha256": producer._environment_sha256(base_env)}})
        after = object_pin("after.json", {"endpoint": "after"})
        environment = {**base_env, "KRR_PERF_RUSTC": "r", "KRR_PERF_CARGO": "c",
            "KRR_PERF_ENVIRONMENT_SHA256": before["sha256"],
            "KRR_PERF_LOCK_PATH": str(root / gate.HARNESS_ROOT / "Cargo.lock"),
            "KRR_PERF_SOURCE_SNAPSHOT_PATH": snapshot["path"]}
        executable = root / "worker"
        executable.write_bytes(b"synthetic worker")
        executable.chmod(0o755)
        worker = pin("worker", executable.read_bytes())
        details = executable.stat()
        binary: gate.JsonObject = {"path": worker["path"], "sha256": worker["sha256"],
            "size": details.st_size, "device": details.st_dev, "inode": details.st_ino,
            "mode": details.st_mode, "mtimeNs": str(details.st_mtime_ns), "ctimeNs": str(details.st_ctime_ns)}
        if mutation == "worker":
            binary["sha256"] = "e" * 64
        execution: gate.JsonObject = {"schema": "krr-html-worker-runtime-execution-v1",
            "binaryIdentityBefore": binary, "binaryIdentityAfter": binary,
            **producer._worker_environment_projection(cast(dict[str, str], environment))}
        if mutation == "environment":
            execution["nonMetadataEnvironmentSha256"] = "f" * 64
        proof = object_pin("build.json", {"schema": "krr-html-performance-build-artifact-v3",
            "arm": "candidate", "buildExit": 0, "frozenBinary": worker,
            "sourceBeforeRef": source, "sourceSnapshotRef": snapshot})
        catalog = cast(gate.JsonObject, expected["catalogCondition"])
        catalog["fixture_path"] = fixture["path"]
        expected.update({"fixture_sha256": fixture["sha256"], "operation_plan_sha256": plan_ref["sha256"],
            "operationPlanRef": plan_ref, "sourcePair": {"before": source, "end": end_source, "snapshot": snapshot},
            "endpointRefs": {"before": before, "end": after}})
        cast(gate.JsonObject, expected["provenance"])["environment_sha256"] = before["sha256"]
        candidate = cast(gate.JsonObject, cast(gate.JsonObject, expected["arms"])["candidate"])
        candidate.update({"binary_sha256": worker["sha256"], "build_artifact_ref": proof})
        return self._legacy_bundle_tail(root, stored, expected, sample, plan, worker, execution, environment, mutation)

    def _legacy_bundle_tail(self, root: Path, stored: dict[str, bytes], expected: gate.JsonObject,
                            sample: gate.JsonObject, plan: gate.JsonObject, worker: gate.JsonObject,
                            execution: gate.JsonObject, environment: dict[str, object],
                            mutation: str) -> tuple[gate.Bundle, gate.JsonObject]:
        del plan
        import hashlib
        def pin(label: str, data: bytes, logical: str | None = None) -> gate.JsonObject:
            path = logical or str(root / label)
            stored[path] = data
            return {"path": path, "sha256": hashlib.sha256(data).hexdigest()}
        def object_pin(label: str, value: gate.JsonObject) -> gate.JsonObject:
            return pin(label, json.dumps(value, sort_keys=True, separators=(",", ":")).encode())
        lines: list[bytes] = []
        for cycle in range(30):
            value = {**sample, "sample": cycle % 5}
            lines.append(f"SAMPLE {cycle} ".encode() + json.dumps(value).encode() + b"\n")
            lines.append(f"READY {cycle}\n".encode())
        stdout = pin("stdout.raw", b"".join(lines))
        stderr = pin("stderr.raw", b"")
        fd_stdout = pin("fd-stdout.raw", b"")
        fd_stderr = pin("fd-stderr.raw", b"")
        tool = {**pin("tool", b"synthetic lsof", "/usr/sbin/lsof"), "size": len(b"synthetic lsof")}
        metrics = pin("memory.jsonl", b"".join(json.dumps(row).encode() + b"\n" for row in self.rows))
        argv = [worker["path"], "--retention", cast(gate.JsonObject, expected["catalogCondition"])["fixture_path"],
                str(root / "unused.json"), cast(gate.JsonObject, expected["operationPlanRef"])["path"],
                "100", "100", "1.0"]
        command = object_pin("command.json", {"argv": argv, "kind": "retention-observation"})
        native: gate.JsonObject = {"schema": "krr-native-raw-fd-closure-v2", "actualExit": 1,
            "reaped": True, "groupClosed": True, "rawFdsClosed": True, "holders": [],
            "toolBefore": tool, "toolAfter": tool, "stdout": fd_stdout, "stderr": fd_stderr,
            "rawIdentities": [{"device": 1, "inode": 2}, {"device": 1, "inode": 3}],
            "identity": {"pid": 456, "sessionId": 456, "processGroupId": 456}, "cwd": str(root),
            "argv": ["/usr/sbin/lsof", "-nP", "-FpgfatiD", "--", stdout["path"], stderr["path"]],
            "attemptToken": "a" * 64, "startedNs": "1", "endedNs": "2",
            "runLimitSeconds": 1, "cleanupLimitSeconds": 10, "descendantClosureProved": False,
            "environmentProjection": producer._worker_environment_projection(cast(dict[str, str], environment)),
            "environmentSha256": producer._environment_sha256(cast(dict[str, str], environment))}
        if mutation == "native_schema":
            native["schema"] = "krr-native-raw-fd-closure-v1"
        elif mutation == "native_projection":
            cast(gate.JsonObject, native["environmentProjection"])["nonMetadataEnvironmentSha256"] = "f" * 64
        elif mutation == "native_environment":
            native["environment"] = environment
        lifecycle = object_pin("lifecycle.json", {"schema": "krr-performance-command-lifecycle-v1",
            "role": "worker", "exitCode": 0, "groupClosed": True, "rawFdsClosed": True,
            "failure": None, "cleanupFailure": None, "cwd": str(root), "argv": argv,
            "identity": {"pid": 123, "sessionId": 123, "processGroupId": 123}, "raw": [stdout, stderr],
            "runtimeExecution": execution, "nativeRawFdClosure": native})
        document = object_pin("observation.json", {"schema": evidence.SCHEMA, "conditionId": "condition",
            "fixtureSha256": expected["fixture_sha256"], "planSha256": expected["operation_plan_sha256"],
            "candidateBinarySha256": worker["sha256"], "argv": argv, "commandRef": command,
            "metricsRef": metrics, "lifecycleRef": lifecycle, "stdoutRef": stdout, "stderrRef": stderr,
            "runtimeExecution": execution, "retainedRssBytes": list(range(1005, 1030))})
        expected["memoryRetentionRef"] = document
        directory = root / "bundle"
        directory.mkdir()
        rows: list[gate.JsonObject] = []
        for index, (original, data) in enumerate(stored.items()):
            relative = f"payload-{index}"
            (directory / relative).write_bytes(data)
            rows.append({"originalPath": original, "relativePath": relative,
                         "sha256": hashlib.sha256(data).hexdigest()})
        (directory / "bundle.json").write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1", "files": rows}), encoding="utf8")
        return gate.Bundle(directory), expected

    def test_legacy_bundle_validates_and_rejects_source_environment_and_worker_mismatch(self) -> None:
        bundle, expected = self.legacy_bundle("valid")
        self.assertEqual(evidence.read_retained_rss(bundle, expected), list(map(float, range(1005, 1030))))
        observation = bundle.json(expected["memoryRetentionRef"])
        lifecycle = bundle.json(observation["lifecycleRef"])
        closure = gate._object(lifecycle["nativeRawFdClosure"], "closure")
        self.assertNotIn("environment", closure)
        self.assertNotIn(SECRET_MARKER.encode(), json.dumps(lifecycle).encode())
        for path, _checksum in bundle.files.values():
            self.assertNotIn(SECRET_MARKER.encode(), path.read_bytes())
        for kind in ("source", "environment", "worker", "native_schema", "native_projection", "native_environment"):
            with self.subTest(kind=kind):
                bundle, expected = self.legacy_bundle("bad-" + kind, kind)
                with self.assertRaises(ValueError):
                    evidence.read_retained_rss(bundle, expected)


if __name__ == "__main__":
    unittest.main()
