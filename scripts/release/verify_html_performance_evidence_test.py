"""既存paired schemaの境界・欠落・改ざんを検証する単体テスト。"""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import subprocess
import tempfile
import unittest
import warnings
import zipfile
from collections.abc import Callable
from pathlib import Path
from typing import cast
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("performance_gate", Path(__file__).with_name("verify_html_performance_evidence.py"))
if spec is None or spec.loader is None:
    raise ImportError("performance evidence gate module is unavailable")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)

JsonObject = dict[str, object]
JsonMutation = Callable[[JsonObject], None]


def json_object(value: object) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise TypeError("fixture value must be a JSON object")
    return cast(JsonObject, value)


def json_array(value: object) -> list[object]:
    if not isinstance(value, list):
        raise TypeError("fixture value must be a JSON array")
    return cast(list[object], value)


def json_path_name(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("fixture path must be text")
    return Path(value).name


def json_object_at(value: object, *path: str | int) -> JsonObject:
    current = value
    for part in path:
        if isinstance(current, dict) and isinstance(part, str):
            current = current[part]
        elif isinstance(current, list) and isinstance(part, int):
            current = current[part]
        else:
            raise TypeError("fixture JSON path does not identify an object")
    return json_object(current)


def json_array_at(value: object, *path: str | int) -> list[object]:
    current = value
    for part in path:
        if isinstance(current, dict) and isinstance(part, str):
            current = current[part]
        elif isinstance(current, list) and isinstance(part, int):
            current = current[part]
        else:
            raise TypeError("fixture JSON path does not identify an array")
    return json_array(current)


def modify_measurement(report: JsonObject, phase: str, metric: str, value: int) -> None:
    for sample in json_array_at(report, "samples"):
        measured = json_object_at(sample, "phases", phase)
        if phase == "post_operation":
            measured = json_object_at(measured, "measurement")
        measured[metric] = value


def add_cold_open_tail(report: JsonObject) -> None:
    samples = json_array_at(report, "samples")
    for sample in samples[-2:]:
        json_object_at(sample, "phases", "cold_open")["elapsed_ms"] = 1000


def remove_full_y_phase(report: JsonObject) -> None:
    json_object_at(report, "samples", 0, "phases").pop("full_y")


def violate_quiet_load(manifest: JsonObject) -> None:
    json_object_at(manifest, "order", 0)["load_ratio_after"] = 0.3
    workers = json_array_at(manifest, "order", 0, "workerLoads")
    json_object(workers[-1])["after"] = 0.3


class PerformanceEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root: Path = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.bundle: Path = Path(self.temp.name) / "bundle"
        self.bundle.mkdir()
        (self.root / "source.rs").write_text("fn main() {}\n")
        fixture_lock = Path(__file__).resolve().parents[1] / "html/performance-harness/Cargo.lock"
        lock_path = self.root / gate.HARNESS_ROOT / "Cargo.lock"
        lock_path.parent.mkdir(parents=True)
        lock_path.write_bytes(fixture_lock.read_bytes())
        fixture_path = self.root / "tests/fixtures/html/interaction-fidelity.html"
        fixture_path.parent.mkdir(parents=True)
        fixture_path.write_text("<button>fixture</button>\n")
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid", "commit", "-qm", "fixture"], check=True)
        self.head: str = gate.git(self.root, "rev-parse", "HEAD").strip()
        self.transport: JsonObject = {
            "schema": "krr-html-performance-release-bundle-v1",
            "head": self.head,
            "files": [],
            "attempts": [],
        }
        self.fixture_sha: str = gate.sha256(fixture_path)
        self.dom_pin: JsonObject = self.add_file(
            "/captured/dom.json",
            "common/dom.json",
            {"inputSha256": self.fixture_sha, "states": {"js": {"expanded": True, "result": "opened"}}},
        )
        self.plan_pin: JsonObject = self.add_file(
            "/captured/plan.json",
            "common/plan.json",
            {
                "fixture_sha256": self.fixture_sha,
                "operation": "toggle",
                "expected_dom_state_path": self.dom_pin["path"],
                "expected_dom_state_sha256": self.dom_pin["sha256"],
            },
        )
        self.binary_refs: dict[str, JsonObject] = {
            arm: self.add_file(f"/captured/{arm}-binary", f"common/{arm}-binary", arm.encode())
            for arm in ("baseline", "candidate")
        }
        self.snapshot_pin: JsonObject | None = None
        self.context_pins: dict[str, JsonObject] | None = None
        self.report_paths: list[list[Path]] = []
        self.expected_paths: list[Path] = []
        self.result_paths: list[Path] = []
        self.manifest_paths: list[Path] = []
        for index, (width, height) in enumerate(gate.VIEWPORTS):
            self.make_attempt(index, width, height)
        self.save_transport()

    def add_file(self, original: str, relative: str, data: object) -> JsonObject:
        path = self.bundle / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data if isinstance(data, bytes) else json.dumps(data, sort_keys=True).encode())
        checksum = gate.sha256(path)
        json_array_at(self.transport, "files").append(
            {"originalPath": original, "relativePath": relative, "sha256": checksum}
        )
        return {"path": original, "sha256": checksum}

    def make_attempt(self, index: int, width: int, height: int) -> None:
        origin = "/captured/repo"
        attempt = f"/captured/attempt-{index}"
        files = [
            {"path": origin + "/" + p, "sha256": checksum, "size": size}
            for p, (checksum, size) in gate.committed_records(self.root).items()
        ]
        source: JsonObject = {
            "schema": "krr-native-build-source-inventory-v1",
            "sourceManifestSchema": "krr-native-build-source-manifest-v1",
            "root": origin,
            "head": self.head,
            "index": {"sha256": "a" * 64},
            "files": files,
            "trackedPaths": [p["path"] for p in files],
            "untrackedPaths": [],
            "missingTrackedPaths": [],
            "inventoryDigest": "b" * 64,
            "stableEndpoints": True,
        }
        before = self.add_file(attempt + "/before.json", f"{index}/before.json", source)
        end = self.add_file(attempt + "/end.json", f"{index}/end.json", source)
        approved = self.add_file(attempt + "/approved.json", f"{index}/approved.json", source)
        rows = [{**row, "path": row["path"].removeprefix(origin + "/")} for row in files]
        snapshot = {
            "records": rows,
            "digest": hashlib.sha256(
                json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
        }
        snapshot_pin = self.snapshot_pin
        if snapshot_pin is None:
            snapshot_pin = self.add_file("/captured/snapshot.json", "common/snapshot.json", snapshot)
            self.snapshot_pin = snapshot_pin
        binary_refs = self.binary_refs
        context_pins = self.context_pins
        if context_pins is None:
            inputs = [{"path": ref["path"], "exists": True, "sha256": ref["sha256"]} for ref in (self.plan_pin, self.dom_pin, snapshot_pin, *binary_refs.values())]
            context = {"source": source, "inputs": inputs, "registry": {}, "cache": {}, "cacheUnion": {"registry": {}, "target": {}}, "scope": {}, "config": [], "preference": {}, "toolContext": {"rustc": "rustc 1.97\nrelease", "cargo": "cargo 1.97\nrelease"}}
            context_pins = {
                name: self.add_file(
                    f"/captured/context-{name}.json", f"common/context-{name}.json", context
                )
                for name in ("before", "end")
            }
            self.context_pins = context_pins
        expected: JsonObject = {
            "fixture_sha256": self.fixture_sha,
            "operation_plan_sha256": self.plan_pin["sha256"],
            "operation": "toggle",
            "expected_dom_state_sha256": self.dom_pin["sha256"],
            "viewport": {"width": width, "height": height, "scale": 1},
            "provenance": {
                "rustc": "rustc 1.97\nrelease",
                "cargo": "cargo 1.97\nrelease",
                "environment_sha256": context_pins["before"]["sha256"],
            },
            "sourcePair": {
                "before": before,
                "end": end,
                "approvedSourceRef": approved,
                "inputPins": [approved],
                "approvedInventoryDigest": source["inventoryDigest"],
                "snapshot": snapshot_pin,
            },
            "arms": {},
            "reportPins": [],
            "operationPlanRef": self.plan_pin,
            "domStateRef": self.dom_pin,
            "endpointRefs": context_pins,
        }
        for arm, ref in binary_refs.items():
            raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
            json_object_at(expected, "arms")[raw_arm] = {
                "binary_sha256": ref["sha256"],
                "source_snapshot_sha256": snapshot_pin["sha256"],
                "lock_sha256": gate.LOCK_SHA,
            }
        manifest: JsonObject = {
            "measurement_executed": True,
            "input_sha256": expected["fixture_sha256"],
            "environment_sha256": json_object_at(expected, "provenance")["environment_sha256"],
            "order": [],
        }
        reports: list[Path] = []
        for pair in range(5):
            for arm in (("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")):
                raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
                provenance = {
                    **json_object_at(expected, "provenance"),
                    "binary_sha256": binary_refs[arm]["sha256"],
                    "source_snapshot_sha256": snapshot_pin["sha256"],
                    "deps_lock_sha256": gate.LOCK_SHA,
                }
                samples = []
                for sample in range(5):
                    phases: dict[str, object] = {
                        p: {
                            "elapsed_ms": 10,
                            "peak_rss_bytes": 100,
                            "frame": {
                                "viewport": expected["viewport"],
                                "pixel_bytes": width * height * 4,
                            },
                        }
                        for p in gate.PHASES
                    }
                    json_object_at(phases, "full_y").update(
                        actual_bottom=True, scroll_steps=3, max_scroll=1000, final_scroll_y=1000
                    )
                    phases["post_operation"] = {
                        "operation": expected["operation"],
                        "expected_dom_state_sha256": expected["expected_dom_state_sha256"],
                        "measurement": phases["post_operation"],
                        "generation_before": 1,
                        "generation_after": 2,
                        "pixel_sha_before": "2" * 64,
                        "pixel_sha_after": "3" * 64,
                        "visual_change_observed": True,
                    }
                    samples.append(
                        {"sample": sample, "viewport": expected["viewport"], "phases": phases}
                    )
                report: JsonObject = {
                    "arm": raw_arm,
                    "measurement_executed": True,
                    "pair_index": pair,
                    "source_sha256": expected["fixture_sha256"],
                    "viewport": expected["viewport"],
                    "operation_plan_sha256": expected["operation_plan_sha256"],
                    "operation": expected["operation"],
                    "expected_dom_state_sha256": expected["expected_dom_state_sha256"],
                    "provenance": provenance,
                    "sample_count": 5,
                    "samples": samples,
                }
                pin = self.add_file(attempt + f"/reports/{arm}-{pair}.json", f"{index}/reports/{arm}-{pair}.json", report)
                json_array_at(expected, "reportPins").append(pin)
                json_array_at(manifest, "order").append(
                    {
                        "arm": arm,
                        "pair_index": pair,
                        "executable": binary_refs[arm]["path"],
                        "executable_sha256": binary_refs[arm]["sha256"],
                        "output": pin["path"],
                        "load_ratio_before": 0.2,
                        "load_ratio_after": 0.2,
                        "workerLoads": [{"before": 0.2, "after": 0.2} for _ in range(5)],
                    }
                )
                reports.append(self.bundle / f"{index}/reports/{arm}-{pair}.json")
        aggregate = gate.aggregate_comparison(reports, expected)
        policy = {p: {k: v <= 0 for k, v in values.items()} for p, values in aggregate["comparison"].items()}
        result = {"measurementExecuted": True, "performanceAccepted": False, "formalAccepted": False, **{k: True for k in gate.ENDPOINT_FLAGS}, "policy": {"policyPass": True, "policy": policy, "deltaThreshold": 0, "quietLoadLimit": 0.25, "phaseRecords": 250, "aggregate": aggregate}}
        expected_pin = self.add_file(attempt + "/expected-enriched.json", f"{index}/expected-enriched.json", expected)
        result_pin = self.add_file(attempt + "/result.json", f"{index}/result.json", result)
        manifest_pin = self.add_file(attempt + "/pair-manifest.json", f"{index}/pair-manifest.json", manifest)
        json_array_at(self.transport, "attempts").append(
            {"expected": expected_pin, "result": result_pin, "pairManifest": manifest_pin}
        )
        self.report_paths.append(reports)
        self.expected_paths.append(self.bundle / f"{index}/expected-enriched.json")
        self.result_paths.append(self.bundle / f"{index}/result.json")
        self.manifest_paths.append(self.bundle / f"{index}/pair-manifest.json")

    def save_transport(self) -> None:
        (self.bundle / "bundle.json").write_text(json.dumps(self.transport))

    def repin(self, path: Path) -> None:
        relative = str(path.relative_to(self.bundle))
        row = next(
            json_object(item)
            for item in json_array_at(self.transport, "files")
            if json_object(item)["relativePath"] == relative
        )
        row["sha256"] = gate.sha256(path)
        for attempt in json_array_at(self.transport, "attempts"):
            for ref in json_object(attempt).values():
                reference = json_object(ref)
                if reference["path"] == row["originalPath"]:
                    reference["sha256"] = row["sha256"]
        self.save_transport()

    def mutate_json(self, path: Path, action: JsonMutation) -> None:
        value = json_object(gate.read_json(path))
        action(value)
        path.write_text(json.dumps(value))
        self.repin(path)

    def mutate_report(self, path: Path, action: JsonMutation) -> None:
        self.mutate_json(path, action)
        index = int(path.relative_to(self.bundle).parts[0])
        expected = json_object(gate.read_json(self.expected_paths[index]))
        pin = next(
            json_object(item)
            for item in json_array_at(expected, "reportPins")
            if json_path_name(json_object(item)["path"]) == path.name
        )
        pin["sha256"] = gate.sha256(path)
        self.expected_paths[index].write_text(json.dumps(expected))
        self.repin(self.expected_paths[index])

    def reaggregate(self, index: int) -> None:
        expected = gate.read_json(self.expected_paths[index])
        aggregate = gate.aggregate_comparison(self.report_paths[index], expected)
        policy = {p: {k: v <= 0 for k, v in values.items()} for p, values in aggregate["comparison"].items()}
        self.mutate_json(
            self.result_paths[index],
            lambda value: json_object_at(value, "policy").update(
                aggregate=aggregate, policy=policy, policyPass=True
            ),
        )

    def reject(self) -> None:
        with self.assertRaises((ValueError, KeyError)):
            gate.verify(self.bundle, self.root)

    def change_candidate_metric(self, path: Path, phase: str, metric: str, value: int) -> None:
        self.mutate_report(
            path,
            lambda report: modify_measurement(report, phase, metric, value),
        )

    def test_current_harness_lock_matches_explicit_pin(self) -> None:
        fixture_lock = Path(__file__).resolve().parents[1] / "html/performance-harness/Cargo.lock"
        self.assertEqual(gate.sha256(fixture_lock), gate.LOCK_SHA)

    def test_tampered_harness_lock_is_rejected(self) -> None:
        lock_path = self.root / gate.HARNESS_ROOT / "Cargo.lock"
        with patch.object(gate, "LOCK_SHA", "0" * 64):
            with self.assertRaisesRegex(ValueError, "harness lock differs"):
                gate.verify(self.bundle, self.root)

        lock_path.write_bytes(lock_path.read_bytes() + b"\n# tampered\n")
        with self.assertRaisesRegex(
            ValueError, "release source is dirty or untracked"
        ):
            gate.verify(self.bundle, self.root)

    def test_equal_all_three_viewports_pass_with_diagnostic_acceptance_false(self) -> None:
        self.assertEqual(gate.verify(self.bundle, self.root)["checks"], 60)

    def test_injected_git_query_transports_real_fixture_queries(self) -> None:
        calls: list[tuple[tuple[str, ...], bytes | None, bool]] = []

        def query(root: Path, args: tuple[str, ...] | list[str], input_bytes: bytes | None,
                  text: bool) -> str | bytes:
            calls.append((tuple(args), input_bytes, text))
            command = ["git", "-C", str(root), *args]
            if input_bytes is None:
                output: object = subprocess.check_output(command, text=text)
            else:
                output = subprocess.check_output(command, input=input_bytes, text=text)
            if text and isinstance(output, str):
                return output
            if not text and isinstance(output, bytes):
                return output
            raise AssertionError("real Git query returned unexpected output type")

        self.assertEqual(gate.verify(self.bundle, self.root, git_query=query)["checks"], 60)
        self.assertTrue(any(args == ("cat-file", "--batch") and input_bytes is not None and not text
                            for args, input_bytes, text in calls))
        self.assertTrue(any(args == ("ls-tree", "-r", "-z", "HEAD") and not text
                            for args, _, text in calls))

    def test_absent_evidence_fails(self) -> None:
        with self.assertRaises(OSError):
            gate.verify(self.bundle / "absent", self.root)

    def test_only_one_viewport_fails(self) -> None:
        self.transport["attempts"] = json_array_at(self.transport, "attempts")[:1]
        self.save_transport()
        self.reject()

    def test_stale_head_fails(self) -> None:
        self.transport["head"] = "0" * 40
        self.save_transport()
        self.reject()

    def test_current_source_change_and_untracked_extra_fail(self) -> None:
        (self.root / "source.rs").write_text("changed")
        self.reject()
        (self.root / "source.rs").write_text("fn main() {}\n")
        (self.root / "extra.rs").write_text("extra")
        self.reject()

    def test_self_consistent_dirty_inventory_cannot_bind_old_head(self) -> None:
        path = self.bundle / "0/before.json"
        before = gate.read_json(path)
        changed = b"fn main() { println!(\"dirty\"); }\n"
        (self.root / "source.rs").write_bytes(changed)
        row = next(r for r in before["files"] if r["path"].endswith("/source.rs"))
        row.update(sha256=hashlib.sha256(changed).hexdigest(), size=len(changed))
        with self.assertRaisesRegex(ValueError, "dirty or untracked"):
            gate.current_source(self.root, before)
        (self.root / "source.rs").write_text("fn main() {}\n")
        with self.assertRaisesRegex(ValueError, "committed HEAD blobs"):
            gate.current_source(self.root, before)

    def test_endpoint_false_fails(self) -> None:
        self.mutate_json(self.result_paths[0], lambda v: v.update(sourceBeforeEqualsEnd=False))
        self.reject()

    def test_each_phase_time_and_rss_regression_fails_after_correct_reaggregation(self) -> None:
        for phase in gate.PHASES:
            for metric, baseline in (("elapsed_ms", 10), ("peak_rss_bytes", 100)):
                with self.subTest(phase=phase, metric=metric):
                    candidates = [p for p in self.report_paths[0] if p.name.startswith("candidate")]
                    for path in candidates:
                        self.change_candidate_metric(path, phase, metric, baseline + 1)
                    self.reaggregate(0)
                    with self.assertRaisesRegex(ValueError, "time/RSS median/p95 regression"):
                        gate.verify(self.bundle, self.root)
                    for path in candidates:
                        self.change_candidate_metric(path, phase, metric, baseline)
                    self.reaggregate(0)

    def test_p95_only_regression_is_rejected(self) -> None:
        path = self.bundle / "0/reports/candidate-4.json"
        self.mutate_report(path, add_cold_open_tail)
        self.reaggregate(0)
        result = gate.read_json(self.result_paths[0])
        delta = result["policy"]["aggregate"]["comparison"]["cold_open"]
        self.assertEqual(delta["elapsed_ms_delta_median"], 0)
        self.assertGreater(delta["elapsed_ms_delta_p95"], 0)
        with self.assertRaisesRegex(ValueError, "time/RSS median/p95 regression"):
            gate.verify(self.bundle, self.root)

    def test_false_policy_flag_fails(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: json_object_at(value, "policy").update(policyPass=False),
        )
        self.reject()

    def test_missing_phase_fails(self) -> None:
        self.mutate_report(
            self.report_paths[0][0],
            remove_full_y_phase,
        )
        self.reject()

    def test_worker_viewport_mismatch_and_partial_full_y_fail(self) -> None:
        path = self.report_paths[0][0]
        self.mutate_report(
            path,
            lambda value: json_object_at(value, "samples", 0).update(
                viewport={"width": 620, "height": 900, "scale": 1}
            ),
        )
        with self.assertRaisesRegex(ValueError, "worker viewport differs"):
            gate.verify(self.bundle, self.root)
        self.mutate_report(
            path,
            lambda value: json_object_at(value, "samples", 0).update(
                viewport={"width": 1280, "height": 900, "scale": 1}
            ),
        )
        self.mutate_report(
            path,
            lambda value: json_object_at(value, "samples", 0, "phases", "full_y").update(
                actual_bottom=False
            ),
        )
        with self.assertRaisesRegex(ValueError, "full-Y span incomplete"):
            gate.verify(self.bundle, self.root)

    def test_quiet_load_violation_fails(self) -> None:
        self.mutate_json(self.manifest_paths[0], violate_quiet_load)
        with self.assertRaisesRegex(ValueError, "worker quiet load limit exceeded"):
            gate.verify(self.bundle, self.root)

    def test_duplicate_json_key_and_transport_traversal_fail(self) -> None:
        (self.bundle / "bundle.json").write_text('{"schema":"a","schema":"b"}')
        self.reject()
        json_object_at(self.transport, "files", 0)["relativePath"] = "../outside.json"
        self.save_transport()
        self.reject()


class PerformanceAcquisitionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture: PerformanceEvidenceTest = PerformanceEvidenceTest(
            "test_equal_all_three_viewports_pass_with_diagnostic_acceptance_false"
        )
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.repository: str = "owner/repo"
        self.destination: Path = self.fixture.root.parent / "acquired"
        self.calls: list[str] = []

    def acquire_case(self, case: str) -> None:
        run: JsonObject = {"run_started_at": "2026-10-10T01:00:00Z", "id": 101, "head_sha": self.fixture.head, "head_repository": {"full_name": self.repository}, "path": ".github/workflows/test-and-build.yml", "status": "completed", "conclusion": "success", "run_attempt": 1}
        if case == "pending":
            run["status"] = "in_progress"
        elif case == "failed":
            run["conclusion"] = "failure"
        elif case == "foreign_repository":
            json_object_at(run, "head_repository")["full_name"] = "foreign/repo"
        producer = case.startswith("producer_")
        environment: dict[str, str] = {}
        if producer:
            run.update(head_sha="a" * 40, status="in_progress", conclusion=None, event="pull_request", repository={"full_name": self.repository}, pull_requests=[{"number": 7, "head": {"sha": self.fixture.head}, "base": {"sha": "b" * 40}}])
            if case == "producer_wrong_head":
                json_object_at(run, "pull_requests", 0, "head")["sha"] = "c" * 40
            event_path = self.fixture.root.parent / "event.json"
            event_path.write_text(json.dumps({"pull_request": {"number": 7, "draft": False, "head": {"sha": self.fixture.head, "ref": "release/v0.4.24", "repo": {"full_name": self.repository}}, "base": {"sha": "b" * 40, "ref": "master"}}}))
            environment = {"GITHUB_JOB": "preflight", "GITHUB_EVENT_NAME": "pull_request", "GITHUB_EVENT_PATH": str(event_path), "GITHUB_RUN_ID": "101", "GITHUB_RUN_ATTEMPT": "1"}
            if case == "producer_wrong_job":
                environment["GITHUB_JOB"] = "release"
        if case.endswith("invalid_calendar"):
            run["run_started_at"] = "2026-02-30T01:00:00Z"
        downloaded = False
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as zipped:
            for file in sorted(self.fixture.bundle.rglob("*")):
                if file.is_file():
                    zipped.writestr(str(file.relative_to(self.fixture.bundle)), file.read_bytes())
            if case == "zip_traversal":
                zipped.writestr("../escape", b"bad")
            elif case == "duplicate_zip":
                # 故意の重複ZIPを検証するため、fixture作成時の警告だけを抑える。
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore", UserWarning)
                    zipped.writestr("bundle.json", b"duplicate")
        archive = buffer.getvalue()
        self.artifact_digest: str = "sha256:" + hashlib.sha256(archive).hexdigest()
        artifact: JsonObject = {"id": 202, "name": f"html-performance-paired-{self.fixture.head}-101-1", "expired": False, "digest": self.artifact_digest}
        if case == "digest_mismatch":
            artifact["digest"] = "sha256:" + "0" * 64
        actual_output = gate.subprocess.check_output

        def synthetic_api(args: list[str], **kwargs: object) -> bytes:
            nonlocal downloaded
            if args[0] != "gh":
                return cast(bytes, actual_output(args, **kwargs))
            path = args[2]
            self.calls.append(path)
            if "workflows/test-and-build.yml/runs?" in path:
                payload = {"total_count": 1, "workflow_runs": [run]}
                if case == "older_run_rerun":
                    payload = {"total_count": 2, "workflow_runs": [run, {**run, "id": 99, "run_attempt": 2, "run_started_at": "2026-10-10T02:00:00Z", "status": "in_progress"}]}
                if downloaded and case.endswith(("new_pending", "new_success", "same_id_rerun", "same_second")):
                    new: JsonObject = {**run, "id": 102, "run_started_at": "2026-10-10T02:00:00Z"}
                    if case.endswith("new_pending"):
                        new.update(status="in_progress", conclusion=None)
                    elif case.endswith("new_success"):
                        new.update(status="completed", conclusion="success")
                    elif case.endswith("same_id_rerun"):
                        new.update(id=101, run_attempt=2)
                    else:
                        new.update(run_started_at=run["run_started_at"], status="in_progress", conclusion=None)
                    payload = {"total_count": 1, "workflow_runs": [new]} if case.endswith("same_id_rerun") else {"total_count": 2, "workflow_runs": [run, new]}
            elif path.endswith("/attempts/1/jobs?per_page=100"):
                jobs: list[JsonObject] = [{"name": name, "run_id": 101, "status": "completed", "conclusion": "success"} for name in ("Test and Build (ubuntu-latest, linux64)", "Test and Build (macos-15, mac-arm64)", "Test and Build (windows-latest, win64)", "Native paired HTML time and RSS nonregression")]
                if case == "producer_skipped_dependency":
                    jobs[-1]["conclusion"] = "skipped"
                payload = {"total_count": len(jobs), "jobs": jobs}
            elif path.endswith("/artifacts?per_page=100"):
                payload = {"total_count": 1, "artifacts": [artifact]}
            elif path.endswith("/202/zip"):
                downloaded = True
                return archive
            elif path.endswith("/runs/101"):
                payload = dict(run)
                if case == "run_attempt_changed":
                    payload["run_attempt"] = 2
            else:
                raise AssertionError(f"unexpected synthetic API request: {args}")
            return json.dumps(payload).encode()

        with patch.object(gate.subprocess, "check_output", synthetic_api):
            if producer:
                with patch.dict(os.environ, environment):
                    gate.acquire(self.destination, self.fixture.root, self.repository, True)
            else:
                gate.acquire(self.destination, self.fixture.root, self.repository)

    def assert_download_race_rejected(self, case: str) -> None:
        with self.assertRaises(ValueError):
            self.acquire_case(case)
        self.assertEqual(sum("workflows/test-and-build.yml/runs?" in path for path in self.calls), 2)
        self.assertFalse((self.destination / "acquisition.json").exists())

    def assert_invalid_calendar_rejected(self, case: str) -> None:
        with self.assertRaisesRegex(ValueError, "generation start time"):
            self.acquire_case(case)
        self.assertFalse(self.destination.exists())

    def test_completed_acquisition_rejects_new_pending_during_download(self) -> None:
        self.assert_download_race_rejected("new_pending")

    def test_completed_acquisition_rejects_new_success_during_download(self) -> None:
        self.assert_download_race_rejected("new_success")

    def test_completed_acquisition_rejects_same_id_rerun_during_download(self) -> None:
        self.assert_download_race_rejected("same_id_rerun")

    def test_completed_acquisition_rejects_same_second_during_download(self) -> None:
        self.assert_download_race_rejected("same_second")

    def test_producer_acquisition_rejects_new_pending_during_download(self) -> None:
        self.assert_download_race_rejected("producer_new_pending")

    def test_producer_acquisition_rejects_new_success_during_download(self) -> None:
        self.assert_download_race_rejected("producer_new_success")

    def test_producer_acquisition_rejects_same_id_rerun_during_download(self) -> None:
        self.assert_download_race_rejected("producer_same_id_rerun")

    def test_producer_acquisition_rejects_same_second_during_download(self) -> None:
        self.assert_download_race_rejected("producer_same_second")

    def test_completed_acquisition_rejects_invalid_calendar(self) -> None:
        self.assert_invalid_calendar_rejected("invalid_calendar")

    def test_producer_acquisition_rejects_invalid_calendar(self) -> None:
        self.assert_invalid_calendar_rejected("producer_invalid_calendar")

    def test_later_attempt_of_older_run_blocks_old_success(self) -> None:
        with self.assertRaisesRegex(gate.PendingEvidence, "still running"):
            self.acquire_case("older_run_rerun")
        self.assertFalse(self.destination.exists())

    def test_current_producer_success_before_workflow_completion(self) -> None:
        self.acquire_case("producer_success")
        self.assertEqual(gate.verify(self.destination, self.fixture.root)["checks"], 60)

    def test_current_producer_requires_exact_pr_head(self) -> None:
        with self.assertRaisesRegex(ValueError, "PR head/base binding"):
            self.acquire_case("producer_wrong_head")
        self.assertFalse(self.destination.exists())

    def test_current_producer_cannot_be_used_by_publication_job(self) -> None:
        with self.assertRaisesRegex(ValueError, "only for PR preflight"):
            self.acquire_case("producer_wrong_job")
        self.assertFalse(self.destination.exists())

    def test_current_producer_requires_every_completed_dependency(self) -> None:
        with self.assertRaisesRegex(ValueError, "dependency did not succeed"):
            self.acquire_case("producer_skipped_dependency")
        self.assertFalse(self.destination.exists())

    def test_matching_run_artifact_and_raw_bundle_pass(self) -> None:
        self.acquire_case("success")
        self.assertEqual(gate.verify(self.destination, self.fixture.root)["checks"], 60)
        self.assertEqual(gate.read_json(self.destination / "acquisition.json"), {"repository": self.repository, "head": self.fixture.head, "runId": 101, "runAttempt": 1, "artifactId": 202, "artifactDigest": self.artifact_digest})

    def test_pending_run_does_not_download_artifact(self) -> None:
        with self.assertRaisesRegex(gate.PendingEvidence, "still running"):
            self.acquire_case("pending")
        self.assertEqual(len(self.calls), 1)
        self.assertFalse(self.destination.exists())

    def test_failed_run_does_not_download_artifact(self) -> None:
        with self.assertRaisesRegex(ValueError, "CI attempt failed"):
            self.acquire_case("failed")
        self.assertEqual(len(self.calls), 1)
        self.assertFalse(self.destination.exists())

    def test_foreign_repository_is_not_current_source_evidence(self) -> None:
        with self.assertRaisesRegex(gate.PendingEvidence, "has not started"):
            self.acquire_case("foreign_repository")
        self.assertEqual(len(self.calls), 1)
        self.assertFalse(self.destination.exists())

    def test_artifact_digest_mismatch_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "artifact digest absent/mismatch"):
            self.acquire_case("digest_mismatch")
        self.assertFalse((self.destination / "bundle.json").exists())

    def test_zip_traversal_is_rejected_before_extraction(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsafe/duplicate archive path"):
            self.acquire_case("zip_traversal")
        self.assertFalse((self.fixture.root.parent / "escape").exists())
        self.assertFalse((self.destination / "bundle.json").exists())

    def test_duplicate_zip_path_is_rejected_before_extraction(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsafe/duplicate archive path"):
            self.acquire_case("duplicate_zip")
        self.assertFalse((self.destination / "bundle.json").exists())

    def test_run_attempt_change_invalidates_acquisition_receipt(self) -> None:
        with self.assertRaisesRegex(ValueError, "run changed during acquisition"):
            self.acquire_case("run_attempt_changed")
        self.assertFalse((self.destination / "acquisition.json").exists())


class PerformanceJsonBoundaryTest(unittest.TestCase):
    def test_json_container_and_text_shapes_reject_invalid_values(self) -> None:
        document: JsonObject = {"field": "value"}
        array: list[object] = [document]
        self.assertIs(gate._object(document, "document"), document)
        self.assertIs(gate._array(array, "array"), array)
        self.assertEqual(gate._text("0", "text"), "0")
        scenarios: tuple[tuple[Callable[[object, str], object], tuple[object, ...]], ...] = (
            (gate._object, (None, [], "{}", {1: "value"})),
            (gate._array, (None, {}, "[]", (1,))),
            (gate._text, (None, 0, True, [], b"text")),
        )
        for validator, values in scenarios:
            for value in values:
                with self.subTest(validator=validator.__name__, value=value), self.assertRaises(ValueError):
                    validator(value, "boundary")

    def test_integer_boundaries_reject_bool_and_coercion(self) -> None:
        for value in (True, False, 1.0, "1", None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                gate._integer(value, "integer")
            with self.subTest(source_size=value), self.assertRaises(ValueError):
                gate.records_map([{"path": "source.rs", "sha256": "a" * 64, "size": value}])
        for value in (0, -1, 7):
            self.assertEqual(gate._integer(value, "integer"), value)
        self.assertEqual(gate.records_map([{"path": "source.rs", "sha256": "a" * 64, "size": 0}]),
                         {"source.rs": ("a" * 64, 0)})

    def test_numeric_boundaries_preserve_numbers_and_reject_weak_coercion(self) -> None:
        invalid_values: tuple[object, ...] = (True, False, "1", "1.5", None, {})
        for value in invalid_values:
            with self.subTest(value=value), self.assertRaises(ValueError):
                gate._numeric(value, "numeric")
        for value in (0, -1, 1, 1.5):
            self.assertIs(gate._numeric(value, "numeric"), value)
        for value in (True, "1", None, 0, -1, float("nan"), float("inf")):
            sample: JsonObject = {"phases": {"cold_open": {"elapsed_ms": value, "peak_rss_bytes": 1}}}
            with self.subTest(elapsed_ms=value), self.assertRaises(ValueError):
                gate._phase_sample(sample, "cold_open")
        self.assertEqual(gate._number(1, "positive measurement"), 1.0)


if __name__ == "__main__":
    unittest.main()
