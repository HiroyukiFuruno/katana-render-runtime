"""既存paired schemaの境界・欠落・改ざんを検証する単体テスト。"""
from __future__ import annotations

import local_html_performance_evidence as local_html

import hashlib
import importlib.util
import io
import json
import math
import os
import shutil
import subprocess
import tempfile
import unittest
import warnings
import zipfile
from contextlib import nullcontext
from collections.abc import Callable, Sequence
from pathlib import Path, PureWindowsPath
from typing import TYPE_CHECKING, cast
from unittest.mock import patch

import html_performance_remote_evidence as remote_evidence
import html_performance_remote_admission as remote_admission
import html_performance_attestation as attestation
import html_performance_fresh_reference as fresh_reference
import html_performance_reference as reference_domain
import verify_html_performance_evidence as canonical_gate

if TYPE_CHECKING:
    from verify_html_performance_evidence import Bundle

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


def json_text(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("fixture value must be text")
    return value


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


def actual_catalog_operation_kinds() -> dict[str, str]:
    repository = Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory() as temporary:
        plan_path = Path(temporary) / "catalog-plan.json"
        subprocess.run(
            ["bun", "scripts/html/fidelity-acceptance-cli.ts",
             "catalog-stage90-plan", str(plan_path)],
            cwd=repository, check=True, capture_output=True, text=True,
        )
        plan = json_object(json.loads(plan_path.read_text()))
    conditions = json_array(plan.get("conditions"))
    return {
        json_text(condition.get("conditionId")): json_text(condition.get("operationKind"))
        for condition in map(json_object, conditions)
    }


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


class CatalogOperationStreamTest(unittest.TestCase):
    @staticmethod
    def pointer_steps() -> list[JsonObject]:
        return [
            {"kind": kind, "targetNodeId": 7, "x": 12.5, "y": 20.0}
            for kind in ("move", "down", "up")
        ]

    def test_pointer_stream_uses_only_observed_target_coordinates(self) -> None:
        stream = gate.catalog_operation_stream("input-state-selected", {
            "steps": self.pointer_steps(),
        })
        self.assertEqual([item["kind"] for item in stream],
                         ["pointer_move", "pointer_down", "pointer_up"])
        self.assertEqual(stream[0]["x"], 12.5)
        self.assertEqual(stream[1]["button"], 0)

    def test_pointer_stream_accepts_only_physically_present_optional_frame_trace(self) -> None:
        steps = self.pointer_steps()
        frame = {"generation": 2, "contentHeight": 100.0, "scrollY": 0}
        for step in steps:
            step["preFrame"] = frame
            step["postFrame"] = frame
        stream = gate.catalog_operation_stream("input-state-selected", {"steps": steps})
        self.assertEqual(len(stream), 3)

    def test_pointer_frame_accepts_both_observed_producer_shapes(self) -> None:
        for frame in (
            {"generation": 2, "contentHeight": 100.0, "scrollY": 0.0},
            {"generation": 2, "scrollY": 0.0},
        ):
            steps = self.pointer_steps()
            for step in steps:
                step["preFrame"] = frame
                step["postFrame"] = frame
            with self.subTest(frame=frame):
                self.assertEqual(len(gate.catalog_operation_stream(
                    "input-state-selected", {"steps": steps})), 3)

    def test_pointer_frame_rejects_negative_nonfinite_and_unknown_fields(self) -> None:
        invalid_frames = (
            {"generation": 2, "contentHeight": -1.0, "scrollY": 0.0},
            {"generation": 2, "contentHeight": float("nan"), "scrollY": 0.0},
            {"generation": 2, "contentHeight": 100.0, "scrollY": -1.0},
            {"generation": 2, "contentHeight": 100.0, "scrollY": 0.0, "extra": True},
        )
        for frame in invalid_frames:
            steps = self.pointer_steps()
            for step in steps:
                step["preFrame"] = frame
                step["postFrame"] = frame
            with self.subTest(frame=frame), self.assertRaisesRegex(ValueError, "frame identity"):
                gate.catalog_operation_stream("input-state-selected", {"steps": steps})

    def test_static_load_does_not_create_an_input_stream(self) -> None:
        self.assertEqual(gate.catalog_operation_stream(
            "load-and-observe-target", {"delivery": "session-load"}), [])

    @staticmethod
    def settled_timer_evidence() -> JsonObject:
        state_before = "microtask>ready>timer"
        state_after = "microtask>ready>timer"
        return {
            "schema": "krr-html84-operation-evidence-v1",
            "kind": "script-await-timer-microtask",
            "status": "observed",
            "sourceSha256Before": "a" * 64,
            "sourceSha256After": "a" * 64,
            "preFrame": {"generation": 1, "scrollY": 0},
            "postFrame": {"generation": 1, "scrollY": 0},
            "target": {"visible": True, "nodeId": 1, "selector": "#state"},
            "state": {"before": state_before, "after": state_after},
            "steps": [{
                "kind": "host-time-advance", "elapsedMs": 0, "timerRan": False,
                "microtaskCheckpoint": "runtime-advance-timers",
                "stateBefore": state_before, "stateAfter": state_after, "fixtureStateVerified": True,
            }],
            "delivery": "session-load-settled",
        }

    def test_settled_timer_is_a_close_reopen_operation(self) -> None:
        evidence = self.settled_timer_evidence()
        expected = "microtask>ready>timer"
        self.assertEqual(gate.catalog_operation_stream(
            "script-await-timer-microtask", evidence, expected, expected), [])

    def test_settled_timer_rejects_dispatch_claims_and_state_drift(self) -> None:
        evidence = self.settled_timer_evidence()
        json_object_at(evidence, "steps", 0)["timerRan"] = True
        expected = "microtask>ready>timer"
        with self.assertRaisesRegex(ValueError, "settled timer operation"):
            gate.catalog_operation_stream("script-await-timer-microtask", evidence, expected, expected)

        evidence = self.settled_timer_evidence()
        json_object_at(evidence, "state")["after"] = "wrong"
        expected = "microtask>ready>timer"
        with self.assertRaisesRegex(ValueError, "settled timer operation"):
            gate.catalog_operation_stream("script-await-timer-microtask", evidence, expected, expected)

    def test_timer_dispatch_still_emits_advance_time(self) -> None:
        evidence = {
            "steps": [{
                "kind": "host-time-advance", "elapsedMs": 25,
                "timerRan": True, "microtaskCheckpoint": "runtime-advance-timers",
            }],
        }
        self.assertEqual(gate.catalog_operation_stream(
            "script-await-timer-microtask", evidence),
            [{"kind": "advance_time", "elapsed_ms": 25}])

    def test_resource_settlement_replays_only_verified_loaded_state(self) -> None:
        state = {"state": "loaded", "fonts": "ready"}
        evidence = {
            "schema": "krr-html84-operation-evidence-v1",
            "kind": "script-await-resource-font", "status": "observed",
            "sourceSha256Before": "a" * 64, "sourceSha256After": "a" * 64,
            "preFrame": {"generation": 1, "scrollY": 0},
            "postFrame": {"generation": 1, "scrollY": 0},
            "target": {"visible": True, "nodeId": 1, "selector": "#state"},
            "state": {"before": state, "after": state},
            "steps": [{"kind": "session-load-settled-resource-state",
                       "stateBefore": state, "stateAfter": state}],
            "delivery": "session-load-settled",
        }
        self.assertEqual(gate.catalog_operation_stream(
            "script-await-resource-font", evidence, state, state), [])
        step: JsonObject = json_object_at(evidence, "steps", 0)
        step["stateAfter"] = {"state": "wrong"}
        with self.assertRaisesRegex(ValueError, "settled resource state"):
            gate.catalog_operation_stream("script-await-resource-font", evidence, state, state)

    def test_provenance_files_are_bound_and_tampering_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "resource.bin"
            path.write_bytes(b"verified")
            checksum = gate.sha256(path)
            report: JsonObject = {
                "resourceProvenance": {
                    "inventoryComplete": True,
                    "requests": [{"bodyPath": str(path), "bytesSha256": checksum}],
                },
                "fontProvenance": [{
                    "fileBytesVerified": True,
                    "filePath": str(path),
                    "fileSha256": checksum,
                }],
            }
            self.assertEqual(gate.provenance_file_refs(report), [
                {"path": str(path), "sha256": checksum},
            ])
            path.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "provenance input file differs"):
                gate.provenance_file_refs(report)

    def test_remote_provenance_requires_matching_transported_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            payload = Path(temporary) / "font"
            payload.write_bytes(b"verified")
            checksum = gate.sha256(payload)
            report: JsonObject = {"fontProvenance": [{"fileBytesVerified": True,
                                   "filePath": "/original-host/font", "fileSha256": checksum}]}
            with self.assertRaisesRegex(ValueError, "provenance input file differs"):
                gate.provenance_file_refs(report)
            def resolve(pin: JsonObject) -> Path:
                self.assertEqual(pin, {"path": "/original-host/font", "sha256": checksum})
                return payload
            self.assertEqual(gate.provenance_file_refs(report, file_resolver=resolve),
                             [{"path": "/original-host/font", "sha256": checksum}])
            payload.write_bytes(b"modified")
            with self.assertRaisesRegex(ValueError, "provenance input file differs"):
                gate.provenance_file_refs(report, file_resolver=resolve)

    def test_mutation_delivery_evidence_is_observation_not_input(self) -> None:
        self.assertEqual(gate.catalog_operation_stream("script-await-mutation-delivery", {
            "delivery": "mutation-observer",
            "state": {"before": ["initial"], "after": ["mutation"]},
        }), [])

    @staticmethod
    def settled_mutation_evidence() -> tuple[JsonObject, JsonObject]:
        expected: JsonObject = {"log": ["mutation", "resize"]}
        evidence: JsonObject = {
            "schema": "krr-html84-operation-evidence-v1",
            "kind": "script-await-mutation-delivery",
            "status": "observed",
            "sourceSha256Before": "a" * 64,
            "sourceSha256After": "a" * 64,
            "preFrame": {"generation": 1, "scrollY": 0},
            "postFrame": {"generation": 1, "scrollY": 0},
            "target": {"visible": True, "nodeId": 1, "selector": "#observed"},
            "state": {"before": expected, "after": expected},
            "steps": [{"kind": "session-load-settled-observer-state",
                       "stateBefore": expected, "stateAfter": expected}],
            "delivery": "session-load-settled",
        }
        return evidence, expected

    def test_settled_mutation_evidence_matches_fixture_state_and_raw_report(self) -> None:
        evidence, expected = self.settled_mutation_evidence()
        self.assertEqual(gate.catalog_operation_stream(
            "script-await-mutation-delivery", evidence, expected, expected), [])

    def test_settled_mutation_rejects_malformed_or_unbound_evidence(self) -> None:
        evidence, expected = self.settled_mutation_evidence()
        json_array(evidence["steps"])[0] = {
            "kind": "generic-observation", "stateBefore": expected, "stateAfter": expected,
        }
        with self.assertRaisesRegex(ValueError, "fixture callback log"):
            gate.catalog_operation_stream("script-await-mutation-delivery", evidence,
                                          expected, expected)

        evidence, expected = self.settled_mutation_evidence()
        json_array(evidence["steps"]).append({
            "kind": "session-load-settled-observer-state",
            "stateBefore": expected, "stateAfter": expected,
        })
        with self.assertRaisesRegex(ValueError, "fixture callback log"):
            gate.catalog_operation_stream("script-await-mutation-delivery", evidence,
                                          expected, expected)

        evidence, expected = self.settled_mutation_evidence()
        json_object(evidence["state"])["after"] = {"log": ["wrong"]}
        with self.assertRaisesRegex(ValueError, "fixture callback log"):
            gate.catalog_operation_stream("script-await-mutation-delivery", evidence,
                                          expected, expected)

    def test_settled_mutation_rejects_raw_state_and_other_operation_kind(self) -> None:
        evidence, expected = self.settled_mutation_evidence()
        with self.assertRaisesRegex(ValueError, "fixture callback log"):
            gate.catalog_operation_stream("script-await-mutation-delivery", evidence,
                                          expected, {"log": ["wrong"]})
        with self.assertRaisesRegex(ValueError, "observation/reopen"):
            gate.catalog_operation_stream("script-load-observe", evidence, expected, expected)

    def test_fixture_expected_observer_log_comes_from_pinned_fixture_json(self) -> None:
        fixture = Path(__file__).parents[2] / "tests/fixtures/html/aspects/script-observers.html"
        self.assertEqual(gate._fixture_expected_state(fixture), [
            "takeRecords", "mutation", "resize", "intersection",
        ])

    def test_filled_save_replays_only_observed_pointer_and_text_steps(self) -> None:
        input_steps = self.pointer_steps()
        save_steps = [dict(step, targetNodeId=8, x=30.0, y=40.0) for step in self.pointer_steps()]
        steps = [*input_steps, {"kind": "text", "targetNodeId": 7,
                                "text": "架空の名称", "before": "", "after": "架空の名称"},
                 *save_steps]
        stream = gate.catalog_operation_stream("input-state-filled-save", {"steps": steps})
        self.assertEqual([item["kind"] for item in stream], [
            "pointer_move", "pointer_down", "pointer_up", "text",
            "pointer_move", "pointer_down", "pointer_up",
        ])

    def test_missing_observed_key_up_fails_closed(self) -> None:
        evidence = self.script_event_evidence()
        json_array(evidence["events"]).pop(3)
        with self.assertRaisesRegex(ValueError, "omits or reorders"):
            gate.catalog_operation_stream("script-real-events", evidence)

    @staticmethod
    def script_event_evidence() -> JsonObject:
        start = {"result": "", "key": ""}
        typed = {"result": "", "key": ""}
        keyed = {"result": "", "key": "Shift"}
        submitted = {"result": "capture>bubble>input>capture>bubble>submit", "key": "Shift"}
        events: list[JsonObject] = [
            {"type": "focus", "targetNodeId": 7, "focused": True,
             "stateBefore": start, "stateAfter": start},
            {"type": "input", "targetNodeId": 7,
             "stateBefore": start, "stateAfter": typed},
            {"type": "keydown", "targetNodeId": 7, "key": "Shift",
             "stateBefore": typed, "stateAfter": keyed},
            {"type": "keyup", "targetNodeId": 7, "key": "Shift",
             "stateBefore": keyed, "stateAfter": keyed},
            {"type": "click", "targetNodeId": 8,
             "stateBefore": keyed, "stateAfter": submitted},
            {"type": "submit", "targetNodeId": 9,
             "stateBefore": keyed, "stateAfter": submitted},
        ]
        steps = [
            *CatalogOperationStreamTest.pointer_steps(),
            {"kind": "text", "targetNodeId": 7, "text": "入力",
             "stateBefore": start, "stateAfter": typed},
            {"kind": "key_down", "targetNodeId": 7, "key": "Shift",
             "stateBefore": typed, "stateAfter": keyed},
            {"kind": "key_up", "targetNodeId": 7, "key": "Shift",
             "stateBefore": keyed, "stateAfter": keyed},
            *[
                {"kind": kind, "targetNodeId": 8, "x": 30.0, "y": 40.0}
                for kind in ("move", "down", "up")
            ],
        ]
        return {
            "schema": "krr-html84-operation-evidence-v1",
            "kind": "script-real-events",
            "status": "observed",
            "sourceSha256Before": "a" * 64,
            "sourceSha256After": "a" * 64,
            "preFrame": {"generation": 1, "scrollY": 0},
            "postFrame": {"generation": 1, "scrollY": 0},
            "target": {"visible": True, "nodeId": 7, "selector": "#field"},
            "steps": steps,
            "events": events,
            "state": {"before": start, "after": submitted},
            "delivery": "session-load",
        }

    def test_script_real_events_replays_exact_ordered_dispatch_trace(self) -> None:
        evidence = self.script_event_evidence()
        expected = {"result": "capture>bubble>input>capture>bubble>submit", "key": "Shift"}
        stream = gate.catalog_operation_stream("script-real-events", evidence, expected, expected)
        self.assertEqual([item["kind"] for item in stream], [
            "pointer_move", "pointer_down", "pointer_up", "text", "key_down", "key_up",
            "pointer_move", "pointer_down", "pointer_up",
        ])
        self.assertEqual(stream[3]["text"], "入力")
        self.assertEqual(stream[4]["key"], "Shift")
        self.assertEqual(stream[5]["key"], "Shift")

    def test_script_real_events_rejects_missing_reordered_extra_and_mismatched_evidence(self) -> None:
        evidence = self.script_event_evidence()
        evidence["events"] = json_array(evidence["events"])[:3] + json_array(evidence["events"])[4:]
        with self.assertRaisesRegex(ValueError, "omits or reorders"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        json_array(evidence["steps"])[5], json_array(evidence["steps"])[4] = (
            json_array(evidence["steps"])[4], json_array(evidence["steps"])[5],
        )
        with self.assertRaisesRegex(ValueError, "dispatch order"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        json_array(evidence["steps"]).append({"kind": "key_up", "targetNodeId": 7, "key": "Shift"})
        with self.assertRaisesRegex(ValueError, "dispatch order"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        json_object(json_array(evidence["events"])[3])["targetNodeId"] = 8
        with self.assertRaisesRegex(ValueError, "bind to the observed dispatch"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        json_object(json_array(evidence["steps"])[5])["stateAfter"] = {"value": "wrong"}
        with self.assertRaisesRegex(ValueError, "state differs"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        json_array(evidence["events"])[2], json_array(evidence["events"])[3] = (
            json_array(evidence["events"])[3], json_array(evidence["events"])[2],
        )
        with self.assertRaisesRegex(ValueError, "omits or reorders"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        evidence["delivery"] = "unknown"
        with self.assertRaisesRegex(ValueError, "fresh fixture load"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        evidence["unexpected"] = True
        with self.assertRaisesRegex(ValueError, "fresh fixture load"):
            gate.catalog_operation_stream("script-real-events", evidence)

        evidence = self.script_event_evidence()
        with self.assertRaisesRegex(ValueError, "operation state differs"):
            gate.catalog_operation_stream("script-real-events", evidence,
                                          {"result": "different"},
                                          {"result": "different"})

    @classmethod
    def toc_evidence(cls) -> JsonObject:
        source_sha = "a" * 64
        links: list[JsonObject] = []
        steps: list[JsonObject] = []
        previous_hash = ""
        for index, destination in enumerate(("#a", "#b", "#c")):
            pointer_trace: list[JsonObject] = [
                {"kind": kind, "targetNodeId": 20 + index, "x": 12.0 + index,
                 "y": 20.0, "preFrame": {"generation": index + 1, "scrollY": float(index)},
                 "postFrame": {"generation": index + 1, "scrollY": float(index)}}
                for kind in ("move", "down", "up")
            ]
            steps.extend(pointer_trace)
            links.append({
                "destination": destination,
                "hash": destination,
                "hashBefore": previous_hash,
                "hashAfter": destination,
                "scrollY": float(index),
                "frame": {"generation": index + 1, "contentHeight": 1500.0,
                          "scrollY": float(index)},
                "targetVisible": True,
                "sourceSha256": source_sha,
                "capturePath": f"/captures/toc-{index}.png",
                "captureSha256": str(index + 1) * 64,
                "svgPath": f"/captures/toc-{index}.svg",
                "svgSha256": str(index + 4) * 64,
                "pointerTrace": pointer_trace,
            })
            previous_hash = destination
        return {
            "schema": "krr-html84-operation-evidence-v1",
            "kind": "navigation-all-pointer-clicks",
            "status": "observed",
            "sourceSha256Before": source_sha,
            "sourceSha256After": source_sha,
            "preFrame": {"generation": 1, "scrollY": 0},
            "postFrame": {"generation": 4, "scrollY": 2.0},
            "target": {"visible": True, "nodeId": 20, "selector": "#a"},
            "state": {"before": {}, "after": {}},
            "steps": steps,
            "delivery": "session-load",
            "links": links,
        }

    def test_all_toc_replays_actual_anchor_pointer_trace_without_invented_scroll(self) -> None:
        stream = gate.catalog_operation_stream("navigation-all-pointer-clicks", self.toc_evidence())
        self.assertEqual(len(stream), 9)
        self.assertEqual([item["kind"] for item in stream], [
            "pointer_move", "pointer_down", "pointer_up",
            "pointer_move", "pointer_down", "pointer_up",
            "pointer_move", "pointer_down", "pointer_up",
        ])

    def test_all_toc_rejects_hash_frame_source_capture_and_pointer_mismatch(self) -> None:
        evidence = self.toc_evidence()
        json_object(json_array(evidence["links"])[1])["hashBefore"] = "#c"
        with self.assertRaisesRegex(ValueError, "hash transition"):
            gate.catalog_operation_stream("navigation-all-pointer-clicks", evidence)

        evidence = self.toc_evidence()
        link = json_object(json_array(evidence["links"])[0])
        json_object(link["frame"])["scrollY"] = 10.0
        with self.assertRaisesRegex(ValueError, "frame/scroll"):
            gate.catalog_operation_stream("navigation-all-pointer-clicks", evidence)

        evidence = self.toc_evidence()
        json_object(json_array(evidence["links"])[0])["sourceSha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "source differs"):
            gate.catalog_operation_stream("navigation-all-pointer-clicks", evidence)

        evidence = self.toc_evidence()
        json_object(json_array(evidence["links"])[2])["capturePath"] = "/captures/toc-1.png"
        with self.assertRaisesRegex(ValueError, "separate capture"):
            gate.catalog_operation_stream("navigation-all-pointer-clicks", evidence)

        evidence = self.toc_evidence()
        json_array(evidence["steps"])[1] = dict(json_object(json_array(evidence["steps"])[1]), x=-1.0)
        with self.assertRaisesRegex(ValueError, "top-level steps"):
            gate.catalog_operation_stream("navigation-all-pointer-clicks", evidence)

    def test_pointer_trace_cannot_be_replaced_by_generic_steps(self) -> None:
        malformed = self.pointer_steps()
        malformed[1]["targetNodeId"] = 8
        with self.assertRaisesRegex(ValueError, "target or coordinates differ"):
            gate.catalog_operation_stream("navigation-pointer-click", {"steps": malformed})


def modify_measurement(report: JsonObject, phase: str, metric: str, value: int) -> None:
    for sample in json_array_at(report, "samples"):
        measured = json_object_at(sample, "phases", phase)
        if phase in ("post_operation", "same_session_resize"):
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


class BuildArtifactArgvTest(unittest.TestCase):
    def guard_fixture(self, root: Path) -> tuple[Bundle, JsonObject, list[str], dict[str, Path]]:
        launcher = str(root / "deleted-uv" / "python")
        argv = [launcher, "guard.py"]
        binary = root / "python-executable"
        stdout = root / "stdout.raw"
        stderr = root / "stderr.raw"
        for path, content in ((binary, b"python"), (stdout, b"stdout"), (stderr, b"stderr")):
            path.write_bytes(content)
        binary.chmod(0o755)
        def pin(path: Path) -> JsonObject:
            return {"path": str(path), "sha256": gate.sha256(path)}
        lifecycle = root / "lifecycle.json"
        lifecycle.write_text(json.dumps({"schema": "krr-performance-command-lifecycle-v1",
            "argv": argv, "role": "build", "cwd": str(root), "exitCode": 0,
            "identity": {"pid": 11, "sessionId": 11, "processGroupId": 11},
            "groupClosed": True, "rawFdsClosed": True, "failure": None, "cleanupFailure": None,
            "raw": [pin(stdout), pin(stderr)]}))
        files = [{"originalPath": str(path), "relativePath": path.name, "sha256": gate.sha256(path)}
                 for path in (binary, stdout, stderr, lifecycle)]
        (root / "bundle.json").write_text(json.dumps({"schema": "krr-html-performance-release-bundle-v1", "files": files}))
        proof: JsonObject = {"guardPython": {"launcher": launcher, "executable": pin(binary)},
                             "guardLifecycleRef": pin(lifecycle)}
        return gate.Bundle(root), proof, argv, {"cargo.jsonl": stdout, "guard-cargo.stderr": stderr}

    def test_guard_python_binds_producer_argv_and_transported_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            bundle, proof, argv, raw = self.guard_fixture(root)
            with patch.object(gate.sys, "executable", str(root / "different-reader-python")):
                self.assertEqual(gate._guard_python_from_proof(bundle, proof, root, argv, raw), Path(argv[0]))
            with self.assertRaisesRegex(ValueError, "lifecycle differs"):
                gate._guard_python_from_proof(bundle, proof, root, [str(root / "different"), *argv[1:]], raw)
            python = json_object(proof["guardPython"])
            python["launcher"] = str(root / "other-launcher")
            with self.assertRaisesRegex(ValueError, "launcher differs"):
                gate._guard_python_from_proof(bundle, proof, root, argv, raw)
            python["launcher"] = argv[0]
            (root / "python-executable").write_bytes(b"modified")
            with self.assertRaises(ValueError):
                gate._guard_python_from_proof(bundle, proof, root, argv, raw)

    def test_rustup_launcher_and_selected_toolchain_are_pinned_separately(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary_directory = root / "bin"
            toolchain = root / "toolchain"
            toolchain.mkdir()
            binary_directory.mkdir()
            for name in ("cargo", "rustc"):
                executable = toolchain / name
                executable.write_text(name + " fixture")
                executable.chmod(0o755)
            cargo_selector = root / "selected-cargo"
            rustc_selector = root / "selected-rustc"
            cargo_selector.write_text(str(toolchain / "cargo") + "\n")
            rustc_selector.write_text(str(toolchain / "rustc") + "\n")
            rustup = binary_directory / "rustup"
            rustup.write_text(
                "#!/bin/sh\ncase \"$2\" in\n"
                + "cargo) selector='" + str(cargo_selector) + "' ;;\n"
                + "rustc) selector='" + str(rustc_selector) + "' ;;\n"
                + "*) exit 9 ;;\nesac\n"
                + "IFS= read -r selected < \"$selector\"\n"
                + "printf '%s\\n' \"$selected\"\n"
            )
            rustup_source = rustup.read_text()
            rustup.chmod(0o755)
            for name in ("cargo", "rustc"):
                (binary_directory / name).symlink_to(rustup)
            environment = {"PATH": str(binary_directory)}

            first = gate.resolve_rust_tool(root, "cargo", environment)
            proof: JsonObject = {
                "cargoLauncher": gate.rust_tool_launcher_pin(first),
                "cargoExecutable": {"path": str(first.executable), "sha256": gate.sha256(first.executable)},
            }
            gate.validate_rust_tool_pin(proof, "cargo", first)
            self.assertEqual(first.launcher, binary_directory / "cargo")
            self.assertEqual(first.dispatcher, rustup)
            self.assertEqual(first.executable, toolchain / "cargo")
            self.assertEqual(first.resolution_argv, (str(rustup), "which", "cargo"))
            self.assertEqual(proof["cargoLauncher"], gate.rust_tool_launcher_pin(first))
            rustc_tool = gate.resolve_rust_tool(root, "rustc", environment)
            self.assertEqual(rustc_tool.launcher, binary_directory / "rustc")
            self.assertEqual(rustc_tool.dispatcher, rustup)
            self.assertEqual(rustc_tool.executable, toolchain / "rustc")

            rustup.write_text(rustup.read_text() + "# changed dispatcher\n")
            changed_dispatcher = gate.resolve_rust_tool(root, "cargo", environment)
            with self.assertRaisesRegex(ValueError, "cargo executable differs"):
                gate.validate_rust_tool_pin(proof, "cargo", changed_dispatcher)

            alternate = root / "alternate-cargo"
            alternate.write_text("alternate cargo fixture")
            alternate.chmod(0o755)
            rustup.write_text(rustup_source)
            cargo_selector.write_text(str(alternate) + "\n")
            changed = gate.resolve_rust_tool(root, "cargo", environment)
            with self.assertRaisesRegex(ValueError, "cargo executable differs"):
                gate.validate_rust_tool_pin(proof, "cargo", changed)

    def test_direct_rust_tool_and_guarded_argv_keep_the_launcher_token(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary_directory = root / "bin"
            binary_directory.mkdir()
            cargo = binary_directory / "cargo"
            rustc = binary_directory / "rustc"
            alternate_cargo = binary_directory / "alternate-cargo"
            cargo.write_text("direct cargo fixture")
            cargo.chmod(0o755)
            rustc.write_text("direct rustc fixture")
            rustc.chmod(0o755)
            alternate_cargo.write_text("alternate cargo fixture")
            alternate_cargo.chmod(0o755)
            harness = root / "scripts/html/performance-harness"
            harness.mkdir(parents=True)
            state = root / "capacity.json"
            state.write_bytes(b"state")
            launcher = gate.resolve_rust_tool(root, "cargo", {"PATH": str(binary_directory)})
            rustc_tool = gate.resolve_rust_tool(root, "rustc", {"PATH": str(binary_directory)})
            self.assertEqual(launcher.executable, cargo)
            self.assertEqual(rustc_tool.executable, rustc)
            self.assertEqual(launcher.resolution_argv, ())
            argv = gate.guarded_build_argv(root, harness, launcher.launcher, "candidate", state,
                                           gate.sha256(state), [str(state)], [])
            gate.validate_guarded_build_argv(argv, root, harness, launcher.launcher, "candidate",
                                             state, gate.sha256(state), [str(state)])
            command_start = argv.index("--") + 1
            self.assertEqual(argv[command_start], str(launcher.launcher))
            argv[command_start] = str(alternate_cargo)
            with self.assertRaisesRegex(ValueError, "canonical guarded Cargo command"):
                gate.validate_guarded_build_argv(argv, root, harness, launcher.launcher, "candidate",
                                                 state, gate.sha256(state), [str(state)])

    def test_relative_rustc_is_resolved_from_each_invocation_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "harness"
            for directory, contents in ((root, "root rustc"), (harness, "harness rustc")):
                tool_directory = directory / "bin"
                tool_directory.mkdir(parents=True)
                rustc = tool_directory / "rustc"
                rustc.write_text(contents)
                rustc.chmod(0o755)

            environment = {"RUSTC": "bin/rustc"}
            root_tool = gate.resolve_rust_tool(root, "rustc", environment, cwd=root)
            harness_tool = gate.resolve_rust_tool(root, "rustc", environment, cwd=harness)

            self.assertEqual(root_tool.launcher, root / "bin/rustc")
            self.assertEqual(harness_tool.launcher, harness / "bin/rustc")
            self.assertNotEqual(root_tool.executable, harness_tool.executable)

    def test_build_reader_reuses_policy_without_matching_consumer_volatile_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "scripts/html/performance-harness"
            harness.mkdir(parents=True)
            binary_directory = root / "bin"
            binary_directory.mkdir()
            cargo = binary_directory / "cargo"
            cargo.write_text("cargo fixture")
            cargo.chmod(0o755)
            home = root / "home"
            home.mkdir()
            environment = {"PATH": str(binary_directory), "HOME": str(home),
                           "CARGO_HOME": str(home / ".cargo"), "CARGO_TARGET_DIR": str(root / "target"),
                           "TERM": "build-terminal", "SHLVL": "2"}
            proof = {"buildEnvironmentSha256": "a" * 64}
            gate._validate_build_environment(proof, root, harness, cargo, environment)
            environment.update({"TERM": "consumer-terminal", "SHLVL": "7"})
            gate._validate_build_environment(proof, root, harness, cargo, environment)
            environment["CARGO_BUILD_TARGET"] = "aarch64-unknown-linux-gnu"
            with self.assertRaisesRegex(RuntimeError, "environment target selection"):
                gate._validate_build_environment(proof, root, harness, cargo, environment)

    def test_guarded_build_argv_accepts_only_exact_capacity_and_cargo_tokens(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "scripts/html/performance-harness"
            harness.mkdir(parents=True)
            cargo = root / "cargo"
            cargo.write_bytes(b"cargo")
            state = root / "capacity-state.json"
            state.write_bytes(b"state")
            profile = root / "target/release"
            profile.mkdir(parents=True)
            refs = []
            for index in range(40):
                reference = root / f"protected-{index}"
                reference.write_bytes(str(index).encode())
                refs.append(str(reference))
            argv = gate.guarded_build_argv(root, harness, cargo, "candidate", state,
                gate.sha256(state), refs, [str(profile)])
            recorded_state_sha = gate.sha256(state)
            old_python = root / "old-uv" / "python"
            recorded = gate.guarded_build_argv(root, harness, cargo, "candidate", state,
                recorded_state_sha, refs, [str(profile)], guard_python=old_python)
            gate.validate_guarded_build_argv(recorded, root, harness, cargo, "candidate", state,
                recorded_state_sha, refs, guard_python=old_python)
            changed_python = [str(root / "unbound-python"), *recorded[1:]]
            with self.assertRaisesRegex(ValueError, "guard prefix differs"):
                gate.validate_guarded_build_argv(changed_python, root, harness, cargo, "candidate", state,
                    recorded_state_sha, refs, guard_python=old_python)
            gate.validate_guarded_build_argv(argv, root, harness, cargo, "candidate", state,
                recorded_state_sha, refs)
            changed_state_path = list(argv)
            changed_state_path[changed_state_path.index("--state") + 1] = str(root / "different-state.json")
            with self.assertRaisesRegex(ValueError, "recorded capacity state/factor differs"):
                gate.validate_guarded_build_argv(changed_state_path, root, harness, cargo, "candidate",
                    state, recorded_state_sha, refs)
            state.write_bytes(b"post-build capacity state")
            Path(refs[0]).unlink()
            profile.rmdir()
            gate.validate_guarded_build_argv(argv, root, harness, cargo, "candidate", state,
                recorded_state_sha, refs)
            mutations: tuple[Callable[[list[str]], None], ...] = (
                lambda changed: changed.insert(changed.index("--features"), "--config"),
                lambda changed: changed.insert(changed.index("--features"), "--target"),
                lambda changed: changed.insert(changed.index("--features") + 1, "--all-features"),
                lambda changed: changed.insert(changed.index("--release"), "--profile"),
            )
            for mutation in mutations:
                changed = list(argv)
                mutation(changed)
                with self.assertRaisesRegex(ValueError, "canonical guarded Cargo command"):
                    gate.validate_guarded_build_argv(changed, root, harness, cargo, "candidate", state,
                        recorded_state_sha, refs)
            changed = list(argv)
            changed[changed.index(refs[0])] = str(root / "unrecorded-protected-ref")
            with self.assertRaisesRegex(ValueError, "unbound capacity references"):
                gate.validate_guarded_build_argv(changed, root, harness, cargo, "candidate", state,
                    recorded_state_sha, refs)


    def test_guarded_build_argv_accepts_one_bound_reference_and_rejects_bad_sets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "scripts/html/performance-harness"
            harness.mkdir(parents=True)
            cargo = root / "cargo"
            cargo.write_bytes(b"cargo fixture")
            state = root / "capacity-state.json"
            state.write_bytes(b"state fixture")
            paths = [root / name for name in ("protected", "unbound")]
            for path in paths:
                path.write_bytes(path.name.encode())
            protected, unbound = (str(path) for path in paths)
            expected = [protected]
            profiles: list[str] = []
            accepted = gate.guarded_build_argv(root, harness, cargo, "candidate", state,
                                               gate.sha256(state), expected, profiles)
            gate.validate_guarded_build_argv(accepted, root, harness, cargo, "candidate", state,
                                             gate.sha256(state), expected)
            cases: tuple[tuple[str, list[str], list[str]], ...] = (("zero", [], []), ("missing", expected, []),
                     ("duplicate", expected, [protected, protected]),
                     ("unbound", expected, [unbound]))
            for label, bound, argv_references in cases:
                with self.subTest(case=label):
                    argv = gate.guarded_build_argv(root, harness, cargo, "candidate", state,
                                                   gate.sha256(state), argv_references, profiles)
                    with self.assertRaisesRegex(ValueError, "missing/duplicate/unbound capacity references"):
                        gate.validate_guarded_build_argv(argv, root, harness, cargo, "candidate", state,
                                                         gate.sha256(state), bound)


def _set_policy_test_field(value: JsonObject, field: str, replacement: object) -> None:
    policy = gate._object(value["policy"], "measurement policy")
    tests = gate._object(policy["tests"], "paired tests")
    cold_open = gate._object(tests["cold_open"], "cold-open tests")
    elapsed = gate._object(cold_open["elapsed_ms"], "cold-open elapsed test")
    elapsed[field] = replacement


class PerformanceEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root: Path = Path(self.temp.name).resolve() / "repo"
        self.root.mkdir()
        self.bundle: Path = Path(self.temp.name).resolve() / "bundle"
        self.bundle.mkdir()
        (self.root / "source.rs").write_text("fn main() {}\n")
        history = self.root / "docs/performance/html-performance-history.json"
        history.parent.mkdir(parents=True)
        history.write_text("historical fixture\n")
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
        self.artifact_refs: dict[str, JsonObject] = {
            arm: self.add_file(f"/captured/{arm}-build.json", f"common/{arm}-build.json",
                               {"frozenBinary": binary})
            for arm, binary in self.binary_refs.items()
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

    def test_v3_source_contract_keeps_fixed_and_live_contexts_separate(self) -> None:
        import html_performance_epoch_reuse as epoch

        expected = gate.read_json(self.expected_paths[0])
        old_pair = json_object(expected["sourcePair"])
        original = gate.Bundle(self.bundle)
        fixed_source = original.json(old_pair["before"])
        tool: JsonObject = {"rustc": "rustc fixture", "cargo": "cargo fixture",
                            "platform": "fixture", "cpuCount": 4,
                            "environmentSha256": "a" * 64}
        declared: epoch.DeclaredEpochInputs = {
            "workers": self.binary_refs, "programs": {"program": self.plan_pin},
            "inputs": {"plan": self.plan_pin}, "resources": {}, "fonts": {},
            "tool_context": tool,
        }
        projection: epoch.EpochProjection = {
            "declared": declared, "operational_pins": {},
            "operational_digest": epoch._inventory_digest({}),
            "unclassified_context_digest": epoch._inventory_digest([]),
        }
        refs = [self.plan_pin, *self.binary_refs.values()]
        rows = [{**pin, "exists": True} for pin in refs]
        moved_source = {**fixed_source, "index": {"sha256": "f" * 64}}
        contexts = [{"source": source, "inputs": rows, "toolContext": tool}
                    for source in (fixed_source, fixed_source, moved_source)]
        pins = [self.add_file(f"/v3/context-{index}", f"v3/context-{index}", context)
                for index, context in enumerate(contexts)]
        self.save_transport()
        bundle = gate.Bundle(self.bundle)
        expected["sourcePair"] = {
            "schema": "krr-html-performance-source-epoch-v3", "epochSourcePair": old_pair,
            "epochContextRef": pins[0], "beforeContextRef": pins[1], "endContextRef": pins[2],
        }
        expected["endpointRefs"] = {"before": pins[1], "end": pins[2]}
        validation = gate.SourceEpochV3Validation(
            str(pins[0]["sha256"]), str(pins[1]["sha256"]), str(pins[2]["sha256"]),
            cast(JsonObject, projection), cast(JsonObject, projection), cast(JsonObject, projection),
        )
        result = gate.source_contract(bundle, self.root, expected, epoch_validation=validation)
        self.assertEqual(result["head"], self.head)
        self.assertNotEqual(contexts[1]["source"], contexts[2]["source"])
        with self.assertRaisesRegex(ValueError, "authenticated epoch projection is required"):
            gate.source_contract(bundle, self.root, expected)
        wrong = validation._replace(end_projection={**projection, "declared": {
            **declared, "inputs": {"other": self.plan_pin},
        }})
        with self.assertRaisesRegex(ValueError, "epoch inputs differs"):
            gate.source_contract(bundle, self.root, expected, epoch_validation=wrong)
        missing = {"path": "/v3/missing", "sha256": "0" * 64}
        pair = json_object(expected["sourcePair"])
        pair["endContextRef"] = missing
        expected["endpointRefs"] = {"before": pins[1], "end": missing}
        with self.assertRaises((KeyError, ValueError)):
            gate.source_contract(bundle, self.root, expected,
                                 epoch_validation=validation._replace(end_context_sha256="0" * 64))
        pair["endContextRef"] = pins[2]
        expected["endpointRefs"] = {"before": pins[1], "end": pins[2]}
        original_source = bundle.file(old_pair["before"])
        original_source.write_bytes(b"modified")
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            gate.source_contract(bundle, self.root, expected, epoch_validation=validation)

    def test_bundle_relative_build_artifact_pin_requires_matching_transport_sha(self) -> None:
        bundle = gate.Bundle(self.bundle)
        row = json_object(json_array_at(self.transport, "files")[0])
        reference = bundle.relative_reference(row["relativePath"], row["sha256"])
        self.assertEqual(bundle.file(reference).read_bytes(),
                         (self.bundle / str(row["relativePath"])).read_bytes())
        with self.assertRaisesRegex(ValueError, "bundle-relative file pin mismatch"):
            bundle.relative_reference(row["relativePath"], "0" * 64)

    def test_bundle_original_path_rejects_relative_and_duplicate_logical_paths(self) -> None:
        def invalid_bundle(original_paths: list[str]) -> Path:
            directory = self.root / "invalid-original-paths"
            rows = []
            for index, original in enumerate(original_paths):
                relative = f"payload/{index}.bin"
                payload = directory / relative
                payload.parent.mkdir(parents=True, exist_ok=True)
                payload.write_bytes(f"payload-{index}".encode())
                rows.append({"originalPath": original, "relativePath": relative,
                             "sha256": gate.sha256(payload)})
            (directory / "bundle.json").write_text(json.dumps({
                "schema": "krr-html-performance-release-bundle-v1", "files": rows,
            }))
            return directory

        with self.assertRaisesRegex(ValueError, "duplicate/nonabsolute original path"):
            gate.Bundle(invalid_bundle(["relative/source.json"]))
        with self.assertRaisesRegex(ValueError, "duplicate/nonabsolute original path"):
            gate.Bundle(invalid_bundle(["/captured/duplicate.json", "/captured/duplicate.json"]))

    def _catalog_capture_proof_fixture(
        self, *, windows: bool = False,
    ) -> tuple[JsonObject, JsonObject, JsonObject, JsonObject, JsonObject, JsonObject]:
        def logical_path(suffix: str) -> str:
            if windows:
                return str(PureWindowsPath("Z:/source", *suffix.split("/")))
            return str(self.root / suffix)

        binary_path = logical_path("tmp/frozen/capture-test")
        binary_ref = self.add_file(binary_path, "catalog/capture-test", b"frozen capture binary")
        original_report = self.add_file(
            logical_path("tmp/original-cargo.jsonl"), "catalog/original-cargo.jsonl", b"original cargo report\n"
        )
        current_report = self.add_file(
            logical_path("tmp/current-cargo.jsonl"), "catalog/current-cargo.jsonl", b"current cargo report\n"
        )
        source = logical_path("target/debug/deps/capture-test-unique")
        proof_value = {
            "schema": "krr-html84-capture-binary-v1",
            "buildReport": original_report["path"],
            "buildReportSha256": original_report["sha256"],
            "source": source,
            "path": binary_path,
            "sha256": binary_ref["sha256"],
        }
        proof_ref = self.add_file(
            logical_path("tmp/capture-binary-proof.json"), "catalog/capture-binary-proof.json", proof_value
        )
        manifest = {
            "schema": "krr-html84-catalog-captures-v2",
            "catalogPath": logical_path("tests/fixtures/html/feature-catalog.json"),
            "binaryPath": binary_path,
            "binarySha256": binary_ref["sha256"],
            "binaryProof": proof_ref,
            "currentBuild": {
                "path": current_report["path"],
                "sha256": current_report["sha256"],
                "source": source,
                "binarySha256": binary_ref["sha256"],
            },
        }
        manifest_ref = self.add_file(
            logical_path("tmp/catalog-stage90-captures.json"),
            "catalog/catalog-stage90-captures.json", manifest,
        )
        self.save_transport()
        return manifest, manifest_ref, proof_ref, original_report, binary_ref, current_report

    def test_catalog_capture_binary_proof_transports_one_bound_closure(self) -> None:
        manifest, _, proof_ref, original_report, binary_ref, current_report = (
            self._catalog_capture_proof_fixture()
        )
        refs = gate.validate_catalog_binary_proof(
            manifest, gate.Bundle(self.bundle).file,
        )
        self.assertEqual(refs.proof_ref, proof_ref)
        self.assertEqual(refs.build_report_ref, original_report)
        self.assertEqual(refs.binary_ref, binary_ref)
        self.assertEqual(refs.current_build_report_ref, current_report)

    def test_catalog_capture_binary_proof_rejects_manifest_drift_and_missing_refs(self) -> None:
        manifest, _, _, _, _, _ = self._catalog_capture_proof_fixture()
        bundle = gate.Bundle(self.bundle)
        current_build = json_object(manifest["currentBuild"])
        with self.assertRaisesRegex(ValueError, "does not bind the manifest binary"):
            gate.validate_catalog_binary_proof(
                {**manifest, "binaryPath": str(self.root / "tmp/frozen/other")}, bundle.file,
            )
        with self.assertRaisesRegex(ValueError, "canonical absolute path"):
            gate.validate_catalog_binary_proof(
                {**manifest, "binaryPath": "/captured/source/tmp/../target/capture-test"}, bundle.file,
            )
        with self.assertRaisesRegex(ValueError, "outside target"):
            gate.validate_catalog_binary_proof(
                {**manifest, "binaryPath": str(self.root / "target/debug/capture-test")}, bundle.file,
            )
        bad_current = {**manifest, "currentBuild": {**current_build, "sha256": "0" * 64}}
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            gate.validate_catalog_binary_proof(bad_current, bundle.file)
        missing_current = {
            **manifest,
            "currentBuild": {
                **current_build,
                "path": "/captured/missing-current.jsonl",
            },
        }
        with self.assertRaises(KeyError):
            gate.validate_catalog_binary_proof(missing_current, bundle.file)

    def test_catalog_capture_binary_closure_uses_strict_typescript_reader_once(self) -> None:
        manifest, manifest_ref, proof_ref, original_report, binary_ref, current_report = (
            self._catalog_capture_proof_fixture(windows=True)
        )
        bundle = gate.Bundle(self.bundle)
        refs = gate.validate_catalog_binary_proof(manifest, bundle.file)
        self.assertEqual(refs.proof_ref, proof_ref)
        self.assertEqual(refs.build_report_ref, original_report)
        self.assertEqual(refs.binary_ref, binary_ref)
        self.assertEqual(refs.current_build_report_ref, current_report)
        self.assertFalse(Path(json_text(original_report["path"])).exists())
        observed: dict[str, object] = {}

        def validate_request(
            argv: list[str], *, cwd: Path, check: bool, capture_output: bool,
        ) -> subprocess.CompletedProcess[bytes]:
            request_path = Path(argv[-1])
            request_bytes = request_path.read_bytes()
            request = json.loads(request_bytes)
            observed["argv"] = argv
            observed["cwd"] = cwd
            observed["check"] = check
            observed["capture_output"] = capture_output
            observed["request"] = request
            result = {
                "schema": "krr-html84-binary-closure-result-v1",
                "requestSha256": hashlib.sha256(request_bytes).hexdigest(),
                "manifest": request["manifest"],
            }
            return subprocess.CompletedProcess(argv, 0, stdout=json.dumps(result).encode(), stderr=b"")

        with patch.object(gate.subprocess, "run", side_effect=validate_request) as run:
            result = gate.validate_catalog_binary_closure(self.root, bundle, manifest_ref, refs)

        request = json_object(observed["request"])
        self.assertEqual(run.call_count, 1)
        self.assertEqual(result["schema"], "krr-html84-binary-closure-result-v1")
        self.assertEqual(request["schema"], "krr-html84-binary-closure-request-v1")
        self.assertEqual(request["manifest"], manifest_ref)
        files = [json_object(file) for file in json_array(request["files"])]
        self.assertEqual(len(files), 5)
        self.assertEqual(len({json_text(file["path"]) for file in files}), 5)
        self.assertTrue(all(Path(json_text(file["payloadPath"])).is_file() for file in files))

    def test_catalog_binary_closure_canonicalizes_aliased_temporary_directory(self) -> None:
        manifest, manifest_ref, _, _, _, _ = self._catalog_capture_proof_fixture(windows=True)
        bundle = gate.Bundle(self.bundle)
        refs = gate.validate_catalog_binary_proof(manifest, bundle.file)
        directory = self.root / "physical-temp"
        directory.mkdir()
        alias = self.root / "temporary-alias"
        try:
            alias.symlink_to(directory, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"directory symlinks unavailable: {error}")

        def validate_request(
            argv: list[str], *, cwd: Path, check: bool, capture_output: bool,
        ) -> subprocess.CompletedProcess[bytes]:
            request_path = Path(argv[-1])
            self.assertEqual(request_path, directory.resolve() / "request.json")
            self.assertEqual(request_path.resolve(strict=True), request_path)
            request_bytes = request_path.read_bytes()
            request = json.loads(request_bytes)
            result = {"schema": "krr-html84-binary-closure-result-v1",
                      "requestSha256": hashlib.sha256(request_bytes).hexdigest(),
                      "manifest": request["manifest"]}
            return subprocess.CompletedProcess(argv, 0, stdout=json.dumps(result).encode(), stderr=b"")

        with patch.object(gate.tempfile, "TemporaryDirectory", return_value=nullcontext(str(alias))), \
             patch.object(gate.subprocess, "run", side_effect=validate_request) as run:
            result = gate.validate_catalog_binary_closure(self.root, bundle, manifest_ref, refs)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(result["manifest"], manifest_ref)

    def test_frozen_binary_transport_survives_removed_target_and_rejects_tampering(self) -> None:
        selected_path = self.root / "target/release/krr-html-performance-harness"
        frozen_source = self.root / "tmp/frozen/krr-html-performance-harness"
        frozen_source.parent.mkdir(parents=True)
        frozen_source.write_bytes(b"frozen candidate binary")
        checksum = gate.sha256(frozen_source)
        frozen_ref = self.add_file(str(frozen_source), "build/candidate-binary", frozen_source.read_bytes())
        selected_pin = {"path": str(selected_path), "sha256": checksum}
        self.save_transport()
        selected_path.unlink(missing_ok=True)
        bundle = gate.Bundle(self.bundle)
        self.assertEqual(gate._canonical_build_path(selected_pin["path"], "selected binary"), selected_path)
        transported = bundle.file(frozen_ref)
        self.assertEqual(gate.sha256(transported), selected_pin["sha256"])
        self.assertFalse(selected_path.exists())
        transported.write_bytes(b"tampered frozen candidate binary")
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            bundle.file(frozen_ref)

    def test_recorded_capacity_state_requires_its_transported_bytes(self) -> None:
        state_ref = self.add_file("/captured/capacity-state.json", "capacity/state.json", b"prebuild state")
        valid_input_ref = self.add_file(
            "/captured/valid-capacity-input.json",
            "capacity/valid-input.json",
            {"stateRef": state_ref},
        )
        invalid_input_ref = self.add_file(
            "/captured/invalid-capacity-input.json",
            "capacity/invalid-input.json",
            {"stateRef": {**state_ref, "sha256": "0" * 64}},
        )
        self.save_transport()
        bundle = gate.Bundle(self.bundle)
        self.assertEqual(
            gate._recorded_capacity_state(bundle, bundle.json(valid_input_ref)),
            (Path(cast(str, state_ref["path"])), cast(str, state_ref["sha256"])),
        )
        with self.assertRaisesRegex(ValueError, "original pin mismatch"):
            gate._recorded_capacity_state(bundle, bundle.json(invalid_input_ref))

    def test_previous_build_receipt_schema_is_rejected(self) -> None:
        expected = json.loads(self.expected_paths[0].read_text())
        source_pair = json_object(expected["sourcePair"])
        proof = {
            "schema": "krr-html-performance-build-artifact-v1",
            "arm": "candidate",
            "buildExit": 0,
            "lockSha256": gate.LOCK_SHA,
            "sourceBeforeRef": source_pair["before"],
            "sourceSnapshotRef": source_pair["snapshot"],
        }
        ref = self.add_file("/captured/old-build.json", "old/build.json", proof)
        self.save_transport()
        bundle = gate.Bundle(self.bundle)
        with self.assertRaisesRegex(ValueError, "unsupported build artifact schema"):
            gate._validate_build_proof(bundle, self.root, ref, "candidate")

    def test_historical_build_uses_selected_lock_and_current_candidate_rejects_it(self) -> None:
        current: bytes = (self.root / gate.HARNESS_ROOT / "Cargo.lock").read_bytes()
        old = current + b"\n# historical fixture\n"
        old_sha = hashlib.sha256(old).hexdigest()
        selection = reference_domain.ActiveReference(
            "active", self.head, "b" * 64, "c" * 64, {"lockSha256": old_sha},
        )
        before_ref = self.add_file("/captured/source-before.json", "old/source.json", {})
        proof = {"schema": "krr-html-performance-build-artifact-v3", "arm": "candidate",
                 "buildExit": 0, "lockSha256": old_sha, "sourceBeforeRef": before_ref}
        ref = self.add_file("/captured/historical-build.json", "old/historical-build.json", proof)
        self.save_transport()
        bundle = gate.Bundle(self.bundle)
        def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str | bytes:
            self.assertEqual(root, self.root)
            self.assertIsNone(input_bytes)
            self.assertTrue(text)
            self.assertEqual(tuple(args), ("show", self.head + ":" + gate.HARNESS_ROOT + "Cargo.lock"))
            return old.decode("utf8")
        with patch.object(gate, "_historical_reference_source", side_effect=RuntimeError("history reached")):
            with self.assertRaisesRegex(RuntimeError, "history reached"):
                gate._validate_build_proof(bundle, self.root, ref, "candidate",
                                           historical_reference=selection, git_query=query)
        with self.assertRaisesRegex(ValueError, "build artifact lock differs"):
            gate._validate_build_proof(bundle, self.root, ref, "candidate")
        wrong = selection._replace(build_pins={"lockSha256": "0" * 64})
        with self.assertRaisesRegex(ValueError, "historical lock pin differs"):
            gate._validate_build_proof(bundle, self.root, ref, "candidate",
                                       historical_reference=wrong, git_query=query)
        harness = self.root / gate.HARNESS_ROOT
        self.assertEqual(gate._runtime_lock_packages(harness, historical_lock=(old, old_sha)),
                         gate._runtime_lock_packages(harness))
        with self.assertRaisesRegex(ValueError, "common lock pin differs"):
            gate._runtime_lock_packages(harness, historical_lock=(old, "0" * 64))

    def test_remote_candidate_rejects_unreviewed_schema_role_and_source_head(self) -> None:
        cases = (("schema", "krr-html-performance-build-artifact-v1", "remote candidate build role differs"),
                 ("arm", "baseline", "remote candidate build role differs"),
                 ("source", "other", "remote candidate source HEAD differs"))
        for index, (field, changed, message) in enumerate(cases):
            with self.subTest(field=field):
                source = self.add_file(f"/remote/source-{index}", f"remote/source-{index}",
                                       {"head": "f" * 40 if field == "source" else self.head})
                proof: JsonObject = {"schema": "krr-html-performance-build-artifact-v3",
                                     "arm": "candidate", "buildExit": 0, "sourceBeforeRef": source}
                if field != "source":
                    proof[field] = changed
                ref = self.add_file(f"/remote/build-{index}", f"remote/build-{index}", proof)
                self.save_transport()
                with self.assertRaisesRegex(ValueError, message):
                    remote_evidence.validate_remote_candidate_build(gate.Bundle(self.bundle), self.root, ref, self.head)

    def test_historical_execution_binary_uses_authenticated_original_transport(self) -> None:
        logical = "/captured/original-binary"
        transported = "/captured/old-bundle/payload/binary"
        original = self.add_file(logical, "history/original", b"binary")
        self.add_file(transported, "history/transported", b"binary")
        candidate = self.add_file("/captured/candidate", "history/candidate", b"candidate")
        manifest = self.add_file("/captured/old-bundle/bundle.json", "history/bundle.json", {
            "schema": "krr-html-performance-release-bundle-v1", "head": self.head,
            "files": [{"originalPath": logical, "relativePath": "payload/binary", "sha256": original["sha256"]}],
        })
        wrapper = self.add_file("/captured/wrapper", "history/wrapper", {
            "frozenBinary": original, "sourceBundleManifest": manifest})
        candidate_build = self.add_file("/captured/candidate-build", "history/candidate-build",
                                        {"frozenBinary": candidate})
        self.save_transport()
        role = "reference-history-record"
        expected: JsonObject = {"referenceParent": {"role": role, "armId": role, "recordId": "record",
            "referenceKind": "accepted-history-candidate", "originalArm": "candidate", "publishedSdk": False,
            "wrapperRef": wrapper}, "arms": {role: {"build_artifact_ref": wrapper, "binary_sha256": original["sha256"]},
                "candidate": {"build_artifact_ref": candidate_build}}}
        validated: dict[str, JsonObject] = {"baseline": {"head": self.head, "binarySha256": original["sha256"],
                                                        "buildArtifactSha256": wrapper["sha256"]}}
        bundle = gate.Bundle(self.bundle)
        binaries = gate.execution_binary_refs(bundle, expected, validated)
        self.assertEqual(binaries, {"baseline": {"path": transported, "sha256": original["sha256"]},
                                    "candidate": candidate})
        gate.require_execution_binary_role({"executable": transported, "executable_sha256": original["sha256"]},
                                           binaries["baseline"])
        for role_name, path, sha in (("baseline", logical, original["sha256"]),
                                      ("candidate", transported, candidate["sha256"])):
            with self.subTest(role=role_name), self.assertRaisesRegex(ValueError, "execution binary differs"):
                gate.require_execution_binary_role({"executable": path, "executable_sha256": sha}, binaries[role_name])
        for changed in ({}, {"baseline": {**validated["baseline"], "buildArtifactSha256": "0" * 64}},
                        {"baseline": {**validated["baseline"], "binarySha256": "0" * 64}}):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                gate.execution_binary_refs(bundle, expected, changed)
        original_json = bundle.json
        def invalid_mapping(ref: object) -> JsonObject:
            value = json_object(original_json(ref))
            if ref == manifest:
                return {**value, "files": [{"originalPath": logical, "relativePath": "../binary",
                                            "sha256": original["sha256"]}]}
            return value
        with patch.object(bundle, "json", side_effect=invalid_mapping):
            with self.assertRaisesRegex(ValueError, "historical binary transport path differs"):
                gate.execution_binary_refs(bundle, expected, validated)

    def test_remote_candidate_binds_historical_lock_to_selected_git_and_snapshot(self) -> None:
        current_lock = self.root / gate.HARNESS_ROOT / "Cargo.lock"
        old = current_lock.read_bytes() + b"\n# historical fixture\n"
        old_sha = hashlib.sha256(old).hexdigest()
        selection = reference_domain.ActiveReference("active", self.head, "b" * 64, "c" * 64,
                                                     {"lockSha256": old_sha})
        rows = [{"path": gate.HARNESS_ROOT + "Cargo.lock", "sha256": old_sha, "size": len(old)}]
        records = {gate.HARNESS_ROOT + "Cargo.lock": (old_sha, len(old))}
        before = self.add_file("/remote/old-source", "remote/old-source", {"head": self.head, "root": str(self.root)})
        snapshot = self.add_file("/remote/old-snapshot", "remote/old-snapshot", {"records": rows,
            "digest": hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest()})
        proof = {"schema": "krr-html-performance-build-artifact-v3", "arm": "candidate", "buildExit": 0,
                 "lockSha256": old_sha, "sourceBeforeRef": before, "sourceSnapshotRef": snapshot}
        ref = self.add_file("/remote/old-build", "remote/old-build", proof)
        self.save_transport()
        bundle = gate.Bundle(self.bundle)
        with patch.object(remote_evidence, "LOCK_SHA", gate.sha256(current_lock)), \
             patch.object(remote_evidence, "_historical_reference_source", return_value=records), \
             patch.object(remote_evidence, "current_source", return_value=records), \
             patch.object(canonical_gate, "git", return_value=old.decode()):
            with self.assertRaisesRegex(ValueError, "remote tool launcher"):
                remote_evidence.validate_remote_candidate_build(bundle, self.root, ref, self.head, historical=selection)
            with self.assertRaisesRegex(ValueError, "remote candidate lock differs"):
                remote_evidence.validate_remote_candidate_build(bundle, self.root, ref, self.head)
            wrong = selection._replace(build_pins={"lockSha256": "0" * 64})
            with self.assertRaisesRegex(ValueError, "historical lock pin differs"):
                remote_evidence.validate_remote_candidate_build(bundle, self.root, ref, self.head, historical=wrong)

    def test_fresh_reference_source_proof_inputs_are_bound_to_measurement_context(self) -> None:
        fields = ("controllerSourceBeforeRef", "selectedSourceProofRef", "sourceBeforeRef", "sourceSnapshotRef")
        pins = [self.add_file(f"/fresh/{field}.json", f"fresh/{field}.json", {"field": field}) for field in fields]
        proof: JsonObject = dict(zip(fields, pins, strict=True))
        context = self.add_file("/fresh/context.json", "fresh/context.json", {
            "inputs": [{**pin, "exists": True} for pin in pins]})
        self.save_transport()
        expected: JsonObject = {"referenceParent": {"sourceRefs": pins},
                                "sourcePair": {"before": pins[0]}, "endpointRefs": {"before": context}}
        bundle = gate.Bundle(self.bundle)
        fresh_reference.require_fresh_source_inputs(bundle, expected, proof)
        wrong = dict(proof)
        wrong["sourceSnapshotRef"] = pins[2]
        with self.assertRaisesRegex(ValueError, "role declarations differ"):
            fresh_reference.require_fresh_source_inputs(bundle, expected, wrong)
        expected["sourcePair"] = {"before": pins[1]}
        with self.assertRaisesRegex(ValueError, "controller source differs"):
            fresh_reference.require_fresh_source_inputs(bundle, expected, proof)

    def test_remote_baseline_cannot_switch_to_sdk_or_unselected_history(self) -> None:
        selection = reference_domain.ActiveReference("active", "a" * 40, "b" * 64, "c" * 64, {})
        cases: list[JsonObject] = [{}, {"referenceParent": {"role": "reference-history-other",
                                   "armId": "reference-history-other", "recordId": "other",
                                   "referenceKind": "accepted-history-candidate", "publishedSdk": False,
                                   "originalArm": "candidate"}}]
        with patch.object(reference_domain, "active_reference_at_commit", return_value=selection):
            for expected in cases:
                with self.subTest(expected=expected), \
                     self.assertRaisesRegex(ValueError, "remote baseline is not the active history reference"):
                    remote_evidence.validate_remote_history_baseline(
                        gate.Bundle(self.bundle), self.root, expected, self.head)

    def test_formal_verifier_rejects_untrusted_remote_context(self) -> None:
        with self.assertRaisesRegex(ValueError, "remote evidence context differs"):
            gate._verify(self.bundle, self.root, require_catalog=True, remote_context={})
        with self.assertRaisesRegex(ValueError, "remote evidence context is unauthenticated"):
            remote_admission.RemoteEvidenceContext(self.head, "0" * 64, {}, authority=object())

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
            fixture_ref: JsonObject = {"path": str(self.root / "tests/fixtures/html/interaction-fidelity.html"),
                                       "sha256": self.fixture_sha}
            lock_ref: JsonObject = {"path": str(self.root / gate.HARNESS_ROOT / "Cargo.lock"),
                                    "sha256": gate.LOCK_SHA}
            context_refs: list[JsonObject] = [self.plan_pin, self.dom_pin, snapshot_pin,
                                              *binary_refs.values(), *self.artifact_refs.values(),
                                              fixture_ref, lock_ref]
            inputs = [{"path": ref["path"], "exists": True, "sha256": ref["sha256"]}
                      for ref in context_refs]
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
                "build_artifact_ref": self.artifact_refs[arm],
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
                        actual_bottom=True,
                        scroll_steps=3,
                        max_scroll=1000,
                        final_scroll_y=1000,
                        max_frame_elapsed_ms=10,
                    )
                    json_object_at(phases, "full_y", "frame")["scroll_y"] = 1000
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
                    resized_viewport = {"width": width - max(width // 4, 1), "height": height, "scale": 1}
                    phases["same_session_resize"] = {
                        "measurement": {
                            "elapsed_ms": 10,
                            "peak_rss_bytes": 100,
                            "frame": {"viewport": resized_viewport,
                                      "pixel_bytes": resized_viewport["width"] * height * 4},
                        },
                        "resize_evidence": {
                            "from_viewport": expected["viewport"],
                            "to_viewport": resized_viewport,
                            "generation_before": 2,
                            "generation_after": 3,
                            "real_raster_frame": True,
                        },
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
        policy = gate.evaluate_measurement_policy(reports, expected)
        result = {"measurementExecuted": True, "performanceAccepted": False,
                  "formalAccepted": False, **{k: True for k in gate.ENDPOINT_FLAGS},
                  "comparisonStatus": "paired", "policy": policy}
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
        policy = gate.evaluate_measurement_policy(self.report_paths[index], expected)
        self.mutate_json(
            self.result_paths[index],
            lambda value: json_object_at(value, "policy").update(policy),
        )

    def reject(self) -> None:
        with self.assertRaises((ValueError, KeyError)):
            gate._verify_legacy_diagnostic(self.bundle, self.root)

    def test_noncomparable_fragment_allowlist_is_exact_and_parent_bound(self) -> None:
        parent = "reference-parent-371274ed"
        reference_parent = {"role": parent, "referenceKind": "correct-parent-source",
                            "publishedSdk": False, "armId": parent}
        operation_kinds = actual_catalog_operation_kinds()
        self.assertEqual(len(operation_kinds), 84)
        for condition_id, fragment in gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES.items():
            operation_kind = operation_kinds[condition_id]
            expected_operation_kind = (
                "navigation-all-pointer-clicks"
                if condition_id.startswith("navigation/all-toc-links/")
                else "navigation-pointer-click"
            )
            expected = {"referenceParent": reference_parent,
                        "catalogCondition": {"condition_id": condition_id,
                                             "operation_kind": operation_kind}}
            with self.subTest(condition_id=condition_id):
                self.assertEqual(operation_kind, expected_operation_kind)
                self.assertEqual(gate.non_comparable_baseline_fragment(expected), fragment)
        condition_id = next(iter(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES))
        expected = {"referenceParent": reference_parent,
                    "catalogCondition": {"condition_id": condition_id,
                                         "operation_kind": "navigation"}}
        with self.assertRaisesRegex(ValueError, "reference parent contract identity differs"):
            gate.non_comparable_baseline_fragment({
                **expected, "referenceParent": {**reference_parent, "publishedSdk": True},
            })
        self.assertIsNone(gate.non_comparable_baseline_fragment({
            **expected, "catalogCondition": {**expected["catalogCondition"],
                                             "operation_kind": "static"},
        }))

    def test_noncomparable_fragment_rejects_generic_navigation_kind(self) -> None:
        parent = "reference-parent-371274ed"
        condition_id = next(iter(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES))
        expected = {
            "referenceParent": {"role": parent, "referenceKind": "correct-parent-source",
                                "publishedSdk": False, "armId": parent},
            "catalogCondition": {"condition_id": condition_id,
                                 "operation_kind": "navigation"},
        }
        self.assertIsNone(gate.non_comparable_baseline_fragment(expected))

    def _noncomparable_baseline_fixture(
        self, label: str, *, stderr: bytes | None = None,
        group_closed: bool = True, actual_exit: int = 1,
    ) -> tuple[Bundle, JsonObject, JsonObject, JsonObject, JsonObject]:
        parent = "reference-parent-371274ed"
        expected = json.loads(self.expected_paths[0].read_text())
        arms = json_object(expected["arms"])
        arms[parent] = arms.pop("baseline-0.4.23")
        expected["referenceParent"] = {
            "role": parent, "referenceKind": "correct-parent-source",
            "publishedSdk": False, "armId": parent,
        }
        expected["catalogCondition"] = {
            "condition_id": "navigation/top/1280x900@1",
            "case_id": "navigation-top", "state": "top",
            "operation_kind": "navigation",
            "fixture_path": str(self.root / "tests/fixtures/html/interaction-fidelity.html"),
        }
        expected["operationPlanRef"] = self.plan_pin
        expected["operationEvidenceRef"] = self.dom_pin
        report = json.loads(self.report_paths[0][0].read_text())
        failure: JsonObject = {
            "status": "nonComparable",
            "reason": "known baseline fragment navigation refusal",
            "conditionId": "navigation/top/1280x900@1", "fragment": "#a",
            "sample": 0, "attempt": 1, "actualExit": 1,
            "errorType": "CalledProcessError",
        }
        report.update(
            arm=parent, measurement_executed=False, sample_count=0, samples=[],
            operation_evidence_ref=self.dom_pin,
            condition_id="navigation/top/1280x900@1", case_id="navigation-top",
            state="top", operation_kind="navigation",
            catalog_condition=expected["catalogCondition"], failure=failure,
        )
        report_ref = self.add_file(
            f"/captured/{label}/baseline-0.json", f"noncomparable/{label}/baseline-0.json",
            report,
        )
        executable = self.binary_refs["baseline"]["path"]
        output = report_ref["path"]
        row: JsonObject = {
            "arm": "baseline", "pair_index": 0, "executable": executable,
            "output": output, "workerLoads": [], "load_ratio_before": 0.1,
            "load_ratio_after": 0.1, "failedWorkerLoads": [{"before": 0.1, "after": 0.1}],
            "status": "nonComparable", "failure": failure,
        }
        attempt = self._noncomparable_worker_attempt(
            label, row, expected, actual_exit=actual_exit,
            stderr=stderr, group_closed=group_closed,
        )
        row["workerAttempts"] = [attempt]
        self.save_transport()
        return gate.Bundle(self.bundle), expected, row, report, failure

    def _noncomparable_worker_attempt(
        self, label: str, row: JsonObject, expected: JsonObject, *,
        actual_exit: int, stderr: bytes | None, group_closed: bool,
    ) -> JsonObject:
        stdout = self.add_file(f"/captured/{label}/stdout.raw",
                               f"noncomparable/{label}/stdout.raw", b"")
        diagnostic = (stderr if stderr is not None else
                      b'Error: RuntimeFailure { error: "navigation is not allowed: #a" }\n')
        stderr_ref = self.add_file(f"/captured/{label}/stderr.raw",
                                   f"noncomparable/{label}/stderr.raw", diagnostic)
        viewport = json_object(expected["viewport"])
        lifecycle = {
            "role": "worker", "identity": {"pid": 4242}, "exitCode": actual_exit,
            "groupClosed": group_closed, "rawFdsClosed": True,
            "failure": None, "cleanupFailure": None,
            "argv": [row["executable"], "--worker", "0",
                     json_object(expected["catalogCondition"])["fixture_path"],
                     row["output"], json_object(expected["operationPlanRef"])["path"],
                     str(viewport["width"]), str(viewport["height"]), str(viewport["scale"])],
            "raw": [stdout, stderr_ref],
        }
        lifecycle_ref = self.add_file(f"/captured/{label}/lifecycle.json",
                                      f"noncomparable/{label}/lifecycle.json", lifecycle)
        expected_arms = json_object(expected["arms"])
        binary_sha = json_object(expected_arms["reference-parent-371274ed"])["binary_sha256"]
        attempt: JsonObject = {
            "sample": 0, "attempt": 1, "actualExit": actual_exit,
            "workerStarted": True, "accepted": False,
            "reason": "worker command exited nonzero",
            "binarySha256Before": binary_sha, "binarySha256After": binary_sha,
            "loadRatioBefore": 0.1, "loadRatioAfter": 0.1,
            "lifecycle": lifecycle_ref,
        }
        record = self.add_file(f"/captured/{label}/attempt.json",
                               f"noncomparable/{label}/attempt.json", attempt)
        return {**attempt, "attemptRecord": record}

    def test_noncomparable_baseline_attempt_requires_exact_closed_fragment_refusal(self) -> None:
        bundle, expected, row, report, failure = self._noncomparable_baseline_fixture("valid")
        gate._validate_noncomparable_baseline_order(
            bundle, expected, row, report, 0, "#a", failure,
        )
        invalid_cases = (
            ("stderr", {"stderr": b'navigation is not allowed: #other\n'}),
            ("lifecycle", {"group_closed": False}),
            ("exit", {"actual_exit": 7}),
        )
        for label, options in invalid_cases:
            with self.subTest(label=label), self.assertRaises(ValueError):
                bundle, expected, row, report, failure = self._noncomparable_baseline_fixture(
                    label, **options,
                )
                gate._validate_noncomparable_baseline_order(
                    bundle, expected, row, report, 0, "#a", failure,
                )

    def test_noncomparable_candidate_report_keeps_all_samples_under_three_seconds(self) -> None:
        expected = json.loads(self.expected_paths[0].read_text())
        identity = {
            "condition_id": "navigation/top/1280x900@1", "case_id": "navigation-top",
            "state": "top", "operation_kind": "navigation",
        }
        evidence_ref = {"path": "/captured/operation.json", "sha256": "d" * 64}
        expected["catalogCondition"] = identity
        expected["operationEvidenceRef"] = evidence_ref
        report = json.loads(next(
            path for path in self.report_paths[0] if path.name.startswith("candidate")
        ).read_text())
        report.update(
            condition_id=identity["condition_id"], case_id=identity["case_id"],
            state=identity["state"], operation_kind=identity["operation_kind"],
            operation_evidence_ref=evidence_ref, catalog_condition=identity,
        )
        for sample in json_array(report["samples"]):
            json_object(sample)["catalog_condition"] = {
                "profile": "catalog-plan84", **identity,
                "operation_evidence_sha256": evidence_ref["sha256"],
            }
        arm, samples = gate._validate_report(report, expected)
        self.assertEqual((arm, len(samples)), ("candidate", 5))
        invalid = json.loads(json.dumps(report))
        json_object_at(invalid, "samples", 0, "phases", "warm_open")["elapsed_ms"] = 3000
        with self.assertRaisesRegex(ValueError, "under 3000 ms"):
            gate._validate_report(invalid, expected)
        invalid = json.loads(json.dumps(report))
        invalid["sample_count"] = 4
        with self.assertRaisesRegex(ValueError, "sample_count does not match samples"):
            gate._validate_report(invalid, expected)

    def test_attempt_validation_rejects_noncomparable_endpoint_and_candidate_count_mutations(self) -> None:
        self.mutate_json(
            self.result_paths[0], lambda result: result.update(
                sourceBeforeEqualsEnd=False, comparisonStatus="nonComparable",
            ),
        )
        endpoint_attempt = json_object(json_array(self.transport["attempts"])[0])
        with self.assertRaisesRegex(ValueError, "measurement/endpoint proof absent or false"):
            gate.validate_attempt(gate.Bundle(self.bundle), self.root, endpoint_attempt)
        self.mutate_json(
            self.result_paths[0], lambda result: result.update(
                sourceBeforeEqualsEnd=True, comparisonStatus="paired",
            ),
        )

        candidate = next(
            path for path in self.report_paths[0] if path.name.startswith("candidate")
        )

        def remove_candidate_sample(report: JsonObject) -> None:
            json_array(report["samples"]).pop()
            report["sample_count"] = 4

        self.mutate_report(candidate, remove_candidate_sample)
        attempt = json_object(json_array(self.transport["attempts"])[0])
        with self.assertRaisesRegex(ValueError, "actual five samples required"):
            gate.validate_attempt(gate.Bundle(self.bundle), self.root, attempt)

    def _summary_validation_fixture(
        self, *, field: str, value: int,
    ) -> tuple[Bundle, list[JsonObject]]:
        source_pair = gate._object(gate.read_json(self.expected_paths[0])["sourcePair"], "source pair")
        self.transport["attempts"] = []
        for index in range(84):
            condition_id = f"paired-condition-{index}"
            expected = {"catalogCondition": {"condition_id": condition_id},
                        "sourcePair": {"before": source_pair["before"]}}
            refs = [
                self.add_file(
                    f"/captured/summary/{field}/{index}/{name}.json",
                    f"summary/{field}/{index}/{name}.json", payload,
                )
                for name, payload in (("expected", expected), ("result", {}), ("manifest", {}))
            ]
            json_array(self.transport["attempts"]).append(
                {"expected": refs[0], "result": refs[1], "pairManifest": refs[2]}
            )
        summaries: list[JsonObject] = []
        for index in range(72):
            summaries.append({
                "catalogCondition": {"condition_id": f"paired-condition-{index}"},
                "comparisonStatus": "paired", "candidateSamples": 25,
                "sampleRecords": 50, "phaseRecords": len(gate.PHASES) * 50,
            })
        for condition_id in gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES:
            summaries.append({
                "catalogCondition": {"condition_id": condition_id},
                "comparisonStatus": "nonComparable", "candidateSamples": 25,
                "sampleRecords": 25, "phaseRecords": len(gate.PHASES) * 25,
            })
        summaries[0][field] = value
        self.save_transport()
        return gate.Bundle(self.bundle), summaries

    def test_catalog_summary_rejects_mutated_72_12_sample_and_phase_totals(self) -> None:
        mutations = (
            ("candidateSamples", 2_099),
            ("sampleRecords", 3_899),
            ("phaseRecords", 23_399),
        )
        for field, value in mutations:
            with self.subTest(field=field):
                bundle, summaries = self._summary_validation_fixture(field=field, value=value)
                with (patch.object(gate, "validate_attempt", side_effect=summaries),
                      patch("html_performance_host_load_validation.verify_catalog_host_load")):
                    with self.assertRaisesRegex(
                        ValueError, "catalog candidate/sample/phase totals differ",
                    ):
                        gate._verify(
                            self.bundle, self.root, require_catalog=False,
                            diagnostic_catalog=True,
                        )

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
                gate._verify_legacy_diagnostic(self.bundle, self.root)

        lock_path.write_bytes(lock_path.read_bytes() + b"\n# tampered\n")
        with self.assertRaisesRegex(
            ValueError, "release source is dirty or untracked"
        ):
            gate._verify_legacy_diagnostic(self.bundle, self.root)

    def test_equal_all_three_viewports_pass_with_diagnostic_acceptance_false(self) -> None:
        self.assertEqual(gate._verify_legacy_diagnostic(self.bundle, self.root)["checks"], 60)

    def test_formal_verifier_rejects_legacy_three_viewport_bundle(self) -> None:
        with patch.object(gate.sys, "argv", [
            "verify_html_performance_evidence.py",
            "--repo", str(self.root),
            "--evidence-dir", str(self.bundle),
        ]), patch.object(gate.sys, "stderr", io.StringIO()) as stderr:
            self.assertEqual(gate.main(), 1)
        self.assertIn("formal HTML performance requires all 84 catalog conditions",
                      stderr.getvalue())

    def test_partial_epoch_source_requires_matching_independent_build_proofs(self) -> None:
        from html_performance_source_reuse import SourceReuseContext

        old_expected = gate.read_json(self.expected_paths[0])
        bundle = gate.Bundle(self.bundle)
        history = self.root / "docs/performance/html-performance-history.json"
        history.write_text("changed documentation\n")
        subprocess.run(["git", "-C", str(self.root), "add", str(history)], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Unit", "-c",
                        "user.email=unit@example.invalid", "commit", "-qm", "docs"], check=True)
        current_head = gate.git(self.root, "rev-parse", "HEAD").strip()
        current_bundle = Path(self.temp.name) / "v3-current-build"
        shutil.copytree(self.bundle, current_bundle)
        manifest_path = current_bundle / "bundle.json"
        manifest = gate.read_json(manifest_path)
        manifest["head"] = current_head
        manifest_path.write_text(json.dumps(manifest, sort_keys=True))
        proof = gate.PartialEpochCurrentBuild(
            current_bundle, gate.sha256(manifest_path), "common/candidate-build.json",
            gate.sha256(current_bundle / "common/candidate-build.json"),
        )
        old_arm = json_object_at(old_expected, "arms", "candidate")
        common: JsonObject = {key: "fixture" for key in (
            "cargoExecutableSha256", "rustcExecutableSha256", "buildEnvironmentSha256",
            "toolVersions", "cargoConfigPins", "cargoLauncher", "rustcLauncher", "guardPython")}
        common["toolVersions"] = {"rustc": "rustc 1.97\nrelease", "cargo": "cargo 1.97\nrelease"}
        old_proof: JsonObject = {**common, "head": self.head,
            "binarySha256": old_arm["binary_sha256"], "lockSha256": old_arm["lock_sha256"],
            "sourceSnapshotSha256": old_arm["source_snapshot_sha256"]}
        current_proof: JsonObject = {**common, "head": current_head,
            "binarySha256": old_arm["binary_sha256"], "lockSha256": old_arm["lock_sha256"]}
        with patch.object(gate, "_validate_build_proof", side_effect=[current_proof, old_proof]) as checked:
            context = gate._verified_partial_epoch_source(bundle, self.root, old_expected, proof)
        self.assertIsInstance(context, SourceReuseContext)
        if not isinstance(context, SourceReuseContext):
            raise AssertionError("partial epoch context differs")
        self.assertEqual(checked.call_count, 2)
        self.assertEqual(context.measured_head, self.head)
        with patch.object(gate, "_validate_build_proof", side_effect=[{**current_proof, "binarySha256": "0" * 64}, old_proof]):
            with self.assertRaisesRegex(ValueError, "build differs: binarySha256"):
                gate._verified_partial_epoch_source(bundle, self.root, old_expected, proof)
        with self.assertRaisesRegex(ValueError, "current build bundle differs"):
            gate._verified_partial_epoch_source(
                bundle, self.root, old_expected, proof._replace(manifest_sha256="0" * 64),
            )
        program = self.root / "scripts/release/new-worker.py"
        program.parent.mkdir(parents=True)
        program.write_text("changed program")
        subprocess.run(["git", "-C", str(self.root), "add", str(program)], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Unit", "-c",
                        "user.email=unit@example.invalid", "commit", "-qm", "program"], check=True)
        manifest["head"] = gate.git(self.root, "rev-parse", "HEAD").strip()
        manifest_path.write_text(json.dumps(manifest, sort_keys=True))
        with self.assertRaisesRegex(ValueError, "measurement program source differs"):
            gate._verified_partial_epoch_source(
                bundle, self.root, old_expected, proof._replace(manifest_sha256=gate.sha256(manifest_path)),
            )

    def test_reuse_keeps_original_head_and_rejects_unaccepted_bundle(self) -> None:
        history = self.root / "docs/performance/html-performance-history.json"
        history.write_text("updated fixture\n")
        subprocess.run(["git", "-C", str(self.root), "add", str(history)], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Unit", "-c",
                        "user.email=unit@example.invalid", "commit", "-qm", "history"], check=True)
        eligible = gate.git(self.root, "rev-parse", "HEAD").strip()
        context = gate._verified_reuse_context(self.root, self.head, git_query=None)
        from html_performance_source_reuse import SourceReuseContext
        self.assertIsInstance(context, SourceReuseContext)
        if not isinstance(context, SourceReuseContext):
            raise AssertionError("invalid reuse context")
        self.assertEqual((context.measured_head, context.eligible_head), (self.head, eligible))
        before = gate.read_json(self.bundle / "0/before.json")
        self.assertEqual(gate._source_for_validation(self.root, before, git_query=None,
                                                     reuse_context=context),
                         gate.committed_records(self.root, self.head))
        with self.assertRaisesRegex(ValueError, "bundle is not current HEAD"):
            gate.verify(self.bundle, self.root)
        manifest_sha = gate.sha256(self.bundle / "bundle.json")
        with self.assertRaisesRegex(ValueError, "all 84 catalog conditions"):
            gate.verify_source_reuse(self.bundle, self.root, manifest_sha, self.bundle,
                                     manifest_sha, "common/candidate-build.json", "0" * 64)

    def test_reuse_rejects_tampered_original_bundle_file(self) -> None:
        manifest_sha = gate.sha256(self.bundle / "bundle.json")
        (self.bundle / "common/dom.json").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "transport file hash mismatch"):
            gate.verify_source_reuse(self.bundle, self.root, manifest_sha, self.bundle,
                                     manifest_sha, "common/candidate-build.json", "0" * 64)

    def test_reuse_checks_current_build_bundle_and_proof(self) -> None:
        current_bundle = Path(self.temp.name) / "current-build"
        shutil.copytree(self.bundle, current_bundle)
        measured_sha = gate.sha256(self.bundle / "bundle.json")
        current_sha = gate.sha256(current_bundle / "bundle.json")
        artifact = current_bundle / "common/candidate-build.json"
        artifact_sha = gate.sha256(artifact)
        original = artifact.read_bytes()
        with (patch.object(gate, "_verify", return_value={}),
              patch.object(gate, "_require_formal_release_result"),
              patch.object(gate, "validate_expected_builds", return_value={"candidate": {}})):
            artifact.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "transport file hash mismatch"):
                gate.verify_source_reuse(self.bundle, self.root, measured_sha, current_bundle,
                                         current_sha, "common/candidate-build.json", artifact_sha)
            artifact.write_bytes(original)
            with self.assertRaisesRegex(ValueError, "unsupported build artifact schema"):
                gate.verify_source_reuse(self.bundle, self.root, measured_sha, current_bundle,
                                         current_sha, "common/candidate-build.json", artifact_sha)

    def test_formal_catalog_dispatch_requires_exact84_and_catalog_binding(self) -> None:
        attempts: list[JsonObject] = [{"expected": f"attempt-{index}"} for index in range(84)]
        gate._require_catalog_attempt_set(attempts, {"catalogCondition": {"condition_id": "first"}})
        with self.assertRaisesRegex(ValueError, "all 84 catalog conditions"):
            gate._require_catalog_attempt_set(attempts[:-1], {"catalogCondition": {}})
        with self.assertRaisesRegex(ValueError, "catalog-bound current-source evidence"):
            gate._require_catalog_attempt_set(attempts, {})

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

        self.assertEqual(gate._verify_legacy_diagnostic(self.bundle, self.root, git_query=query)["checks"], 60)
        self.assertTrue(any(args == ("cat-file", "--batch") and input_bytes is not None and not text
                            for args, input_bytes, text in calls))
        self.assertTrue(any(args == ("ls-tree", "-r", "-z", "HEAD") and not text
                            for args, _, text in calls))

    def test_absent_evidence_fails(self) -> None:
        with self.assertRaises(OSError):
            gate._verify_legacy_diagnostic(self.bundle / "absent", self.root)

    def test_only_one_viewport_fails(self) -> None:
        self.transport["attempts"] = json_array_at(self.transport, "attempts")[:1]
        self.save_transport()
        self.reject()

    def test_stale_head_fails(self) -> None:
        self.transport["head"] = "0" * 40
        self.save_transport()
        self.reject()

    def test_shared_source_checks_each_inventory_and_rejects_foreign_source(self) -> None:
        before = gate.read_json(self.bundle / "0/before.json")
        records = gate.current_source(self.root, before)
        shared = gate.SharedCurrentSource(self.root, self.head, records, Path(cast(str, before["root"])))
        self.assertEqual(gate._shared_current_source(self.root, before, shared), records)
        for field, replacement in (("head", "0" * 40), ("root", "relative"),
                                   ("trackedPaths", []), ("untrackedPaths", ["unknown"])):
            changed = dict(before)
            changed[field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                gate._shared_current_source(self.root, changed, shared)
        changed = dict(before)
        changed["files"] = []
        with self.assertRaises(ValueError):
            gate._shared_current_source(self.root, changed, shared)
        with self.assertRaises(ValueError):
            gate._shared_current_source(self.root.parent, before, shared)

    def test_shared_source_does_not_skip_later_snapshot_validation(self) -> None:
        bundle = gate.Bundle(self.bundle)
        expected = bundle.json(gate._object(gate._objects(bundle.manifest["attempts"], "attempts")[0],
                                            "first attempt")["expected"])
        pair = gate._object(expected["sourcePair"], "source pair")
        before = bundle.json(pair["before"])
        shared = gate.SharedCurrentSource(self.root, self.head, gate.current_source(self.root, before),
                                          Path(cast(str, before["root"])))
        gate.source_contract(bundle, self.root, expected, shared_source=shared)
        original = bundle.json
        snapshot_ref = pair["snapshot"]
        def changed_snapshot(reference: object) -> JsonObject:
            value = original(reference)
            return {**value, "digest": "0" * 64} if reference == snapshot_ref else value
        with patch.object(bundle, "json", side_effect=changed_snapshot), self.assertRaisesRegex(ValueError, "snapshot digest differs"):
            gate.source_contract(bundle, self.root, expected, shared_source=shared)

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
                    with self.assertRaisesRegex(ValueError, "paired time/RSS regression"):
                        gate._verify_legacy_diagnostic(self.bundle, self.root)
                    for path in candidates:
                        self.change_candidate_metric(path, phase, metric, baseline)
                    self.reaggregate(0)

    def test_candidate_display_phases_require_each_frame_under_three_seconds(self) -> None:
        candidate = next(path for path in self.report_paths[0] if path.name.startswith("candidate"))
        report = json_object(gate.read_json(candidate))
        expected = json_object(gate.read_json(self.expected_paths[0]))
        for phase in gate.DISPLAY_COMPLETION_PHASES:
            for sample in json_array_at(report, "samples"):
                measured = json_object_at(sample, "phases", phase)
                if phase in ("post_operation", "same_session_resize"):
                    measured = json_object(measured["measurement"])
                measured["elapsed_ms"] = 2999.999
            gate._validate_report(report, expected)
            measured = json_object_at(report, "samples", 0, "phases", phase)
            if phase in ("post_operation", "same_session_resize"):
                measured = json_object(measured["measurement"])
            measured["elapsed_ms"] = 3000
            with self.subTest(phase=phase), self.assertRaisesRegex(ValueError, "under 3000 ms"):
                gate._validate_report(report, expected)
            measured["elapsed_ms"] = 2999.999

    def test_candidate_full_y_max_frame_time_is_finite_nonnegative_and_under_three_seconds(self) -> None:
        candidate = next(path for path in self.report_paths[0] if path.name.startswith("candidate"))
        report = json_object(gate.read_json(candidate))
        expected = json_object(gate.read_json(self.expected_paths[0]))
        samples = json_array_at(report, "samples")
        for sample in samples:
            json_object_at(sample, "phases", "full_y")["max_frame_elapsed_ms"] = 2999.999
        gate._validate_report(report, expected)
        measured = json_object_at(report, "samples", 0, "phases", "full_y")
        measured["max_frame_elapsed_ms"] = 0
        gate._validate_report(report, expected)
        for elapsed in (3000, -1, math.nan, math.inf, True, None):
            measured["max_frame_elapsed_ms"] = elapsed
            with self.subTest(elapsed=elapsed), self.assertRaises(ValueError):
                gate._validate_report(report, expected)
        measured.pop("max_frame_elapsed_ms")
        with self.assertRaisesRegex(ValueError, "must be numeric"):
            gate._validate_report(report, expected)

    def test_candidate_full_y_requires_the_captured_bottom_frame_position(self) -> None:
        candidate = next(path for path in self.report_paths[0] if path.name.startswith("candidate"))
        expected = json_object(gate.read_json(self.expected_paths[0]))
        original = json_object(gate.read_json(candidate))
        gate._validate_report(original, expected)

        for invalid in ("missing", math.nan, 999):
            report = json.loads(json.dumps(original))
            frame = json_object_at(report, "samples", 0, "phases", "full_y", "frame")
            if invalid == "missing":
                frame.pop("scroll_y")
            else:
                frame["scroll_y"] = invalid
            with self.subTest(frame_scroll=invalid), self.assertRaises(ValueError):
                gate._validate_report(report, expected)

        # The legacy baseline has no candidate bottom-frame acceptance contract.
        baseline = next(path for path in self.report_paths[0] if path.name.startswith("baseline"))
        baseline_report = json_object(gate.read_json(baseline))
        json_object_at(
            baseline_report, "samples", 0, "phases", "full_y", "frame"
        ).pop("scroll_y")
        gate._validate_report(baseline_report, expected)

    def test_catalog_candidate_accepts_zero_span_at_the_actual_bottom(self) -> None:
        candidate = next(path for path in self.report_paths[0] if path.name.startswith("candidate"))
        report = json.loads(json.dumps(gate.read_json(candidate)))
        expected = json.loads(json.dumps(gate.read_json(self.expected_paths[0])))
        identity = {
            "condition_id": "zero-span-condition",
            "case_id": "zero-span-case",
            "state": "initial",
            "operation_kind": "none",
        }
        evidence_ref = {"path": "/captured/operation.json", "sha256": "d" * 64}
        expected["catalogCondition"] = identity
        expected["operationEvidenceRef"] = evidence_ref
        report.update(
            condition_id=identity["condition_id"],
            case_id=identity["case_id"],
            state=identity["state"],
            operation_kind=identity["operation_kind"],
            operation_evidence_ref=evidence_ref,
            catalog_condition=identity,
        )
        sample_condition = {
            "profile": "catalog-plan84",
            **identity,
            "operation_evidence_sha256": evidence_ref["sha256"],
        }
        for sample in json_array_at(report, "samples"):
            sample_object = json_object(sample)
            sample_object["catalog_condition"] = sample_condition
            sample_viewport = json_object(sample_object["viewport"])
            full_y = json_object_at(sample_object, "phases", "full_y")
            full_y.update(
                elapsed_ms=0,
                actual_bottom=True,
                scroll_steps=0,
                max_scroll=0,
                final_scroll_y=0,
                max_frame_elapsed_ms=0,
            )
            frame = json_object(full_y["frame"])
            frame.update(
                scroll_y=0,
                content_height=sample_viewport["height"],
                viewport=sample_viewport,
            )
        gate._validate_report(report, expected)
        self.assertEqual(
            gate.aggregate_reports([report], expected)["phases"]["full_y"]["elapsed_ms"][
                "median"
            ],
            0,
        )

        for field, invalid in (
            ("actual_bottom", False),
            ("scroll_steps", 1),
            ("max_scroll", 1),
            ("final_scroll_y", 1),
            ("max_frame_elapsed_ms", 1),
        ):
            invalid_report = json.loads(json.dumps(report))
            invalid_full_y = json_object_at(
                invalid_report, "samples", 0, "phases", "full_y"
            )
            invalid_full_y[field] = invalid
            invalid_frame = json_object(invalid_full_y["frame"])
            if field in ("max_scroll", "final_scroll_y"):
                invalid_frame["scroll_y"] = 1
            elif field == "scroll_steps":
                invalid_full_y.update(max_scroll=1, final_scroll_y=1)
                invalid_frame["scroll_y"] = 1
            with self.subTest(no_scroll_evidence=field), self.assertRaises(ValueError):
                gate._validate_report(invalid_report, expected)

        for phase in set(gate.PHASES) - {"full_y"}:
            invalid_report = json.loads(json.dumps(report))
            measured = json_object_at(invalid_report, "samples", 0, "phases", phase)
            if phase in ("post_operation", "same_session_resize"):
                measured = json_object(measured["measurement"])
            measured["elapsed_ms"] = 0
            with self.subTest(zero_elapsed_phase=phase), self.assertRaisesRegex(
                ValueError, "must be finite and positive"
            ):
                gate._validate_report(invalid_report, expected)

        for sample in json_array_at(report, "samples"):
            json_object_at(sample, "phases", "full_y")["elapsed_ms"] = 10
        full_y = json_object_at(report, "samples", 0, "phases", "full_y")
        for invalid_steps in (-1, 1):
            full_y["scroll_steps"] = invalid_steps
            with self.subTest(scroll_steps=invalid_steps), self.assertRaisesRegex(
                ValueError, "full-Y span incomplete"
            ):
                gate._validate_report(report, expected)

        for sample in json_array_at(report, "samples"):
            full_y = json_object_at(sample, "phases", "full_y")
            full_y.update(scroll_steps=0, max_scroll=1, final_scroll_y=1)
            json_object(full_y["frame"])["scroll_y"] = 1
        with self.assertRaisesRegex(ValueError, "full-Y span incomplete"):
            gate._validate_report(report, expected)

    def test_zero_full_y_time_rejects_missing_proof_and_invalid_elapsed(self) -> None:
        viewport = {"width": 1280, "height": 900, "scale": 1}
        full_y = {
            "elapsed_ms": 0,
            "peak_rss_bytes": 1,
            "actual_bottom": True,
            "scroll_steps": 0,
            "max_scroll": 0,
            "final_scroll_y": 0,
            "max_frame_elapsed_ms": 0,
            "frame": {
                "scroll_y": 0,
                "content_height": 900,
                "viewport": viewport,
            },
        }
        sample = {"viewport": viewport, "phases": {"full_y": full_y}}
        self.assertEqual(
            gate._phase_sample(sample, "full_y"),
            (0.0, 1.0),
        )
        for field in (
            "actual_bottom",
            "scroll_steps",
            "max_scroll",
            "final_scroll_y",
            "max_frame_elapsed_ms",
        ):
            incomplete = {key: value for key, value in full_y.items() if key != field}
            with self.subTest(missing_proof=field), self.assertRaisesRegex(
                ValueError, "must be finite and positive"
            ):
                gate._phase_sample({"viewport": viewport, "phases": {"full_y": incomplete}}, "full_y")
        incomplete_frame = {**full_y, "frame": {}}
        with self.assertRaisesRegex(ValueError, "must be finite and positive"):
            gate._phase_sample(
                {"viewport": viewport, "phases": {"full_y": incomplete_frame}}, "full_y"
            )
        base_frame = json_object(full_y["frame"])
        mismatched_frame = {
            **full_y,
            "frame": {
                **base_frame,
                "viewport": {"width": 1279, "height": 900, "scale": 1},
            },
        }
        with self.assertRaisesRegex(ValueError, "must be finite and positive"):
            gate._phase_sample(
                {"viewport": viewport, "phases": {"full_y": mismatched_frame}}, "full_y"
            )
        too_tall_frame = {
            **full_y,
            "frame": {**base_frame, "content_height": 901},
        }
        with self.assertRaisesRegex(ValueError, "must be finite and positive"):
            gate._phase_sample(
                {"viewport": viewport, "phases": {"full_y": too_tall_frame}}, "full_y"
            )
        for content_height in (0, -1, math.nan, math.inf, True, None):
            invalid_frame = {**full_y, "frame": {**base_frame, "content_height": content_height}}
            with self.subTest(content_height=content_height), self.assertRaisesRegex(
                ValueError, "must be finite and positive"
            ):
                gate._phase_sample(
                    {"viewport": viewport, "phases": {"full_y": invalid_frame}}, "full_y"
                )

        for key, invalid_viewport_value in (("height", 0), ("scale", 0), ("scale", math.inf)):
            invalid_viewport = {**viewport, key: invalid_viewport_value}
            invalid_frame = {**full_y, "frame": {**base_frame, "viewport": invalid_viewport}}
            with self.subTest(viewport_field=key), self.assertRaisesRegex(
                ValueError, "must be finite and positive"
            ):
                gate._phase_sample(
                    {"viewport": invalid_viewport, "phases": {"full_y": invalid_frame}},
                    "full_y",
                )

        for elapsed in (-1, math.nan, math.inf, True, None):
            invalid = {**full_y, "elapsed_ms": elapsed}
            expected_error = (
                "must be numeric"
                if elapsed is True or elapsed is None
                else "must be finite and positive"
            )
            with self.subTest(elapsed_ms=elapsed), self.assertRaisesRegex(
                ValueError, expected_error
            ):
                gate._phase_sample(
                    {"viewport": viewport, "phases": {"full_y": invalid}}, "full_y"
                )

    def test_p95_only_regression_is_rejected(self) -> None:
        path = self.bundle / "0/reports/candidate-4.json"
        self.mutate_report(path, add_cold_open_tail)
        self.reaggregate(0)
        result = gate.read_json(self.result_paths[0])
        delta = result["policy"]["aggregate"]["comparison"]["cold_open"]
        self.assertEqual(delta["elapsed_ms_delta_median"], 0)
        self.assertGreater(delta["elapsed_ms_delta_p95"], 0)
        gate._verify_legacy_diagnostic(self.bundle, self.root)

    def test_false_policy_flag_fails(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: json_object_at(value, "policy").update(policyPass=False),
        )
        self.reject()

    def test_policy_version_fails_when_corrupted(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: json_object_at(value, "policy").update(version="wrong"),
        )
        self.reject()

    def test_policy_method_fails_when_corrupted(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: _set_policy_test_field(value, "method", "wrong"),
        )
        self.reject()

    def test_policy_family_alpha_fails_when_corrupted(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: json_object_at(value, "policy").update(comparisonAlpha=0.05),
        )
        self.reject()

    def test_policy_p_value_fails_when_corrupted(self) -> None:
        self.mutate_json(
            self.result_paths[0],
            lambda value: _set_policy_test_field(value, "pValue", 0.0),
        )
        self.reject()

    def test_missing_phase_fails(self) -> None:
        self.mutate_report(
            self.report_paths[0][0],
            remove_full_y_phase,
        )
        self.reject()

    def test_same_session_resize_requires_new_raster_frame(self) -> None:
        self.mutate_report(
            self.report_paths[0][0],
            lambda value: json_object_at(
                value, "samples", 0, "phases", "same_session_resize", "resize_evidence"
            ).update(real_raster_frame=False),
        )
        self.reject()

    def test_same_session_resize_rejects_one_pixel_width_change(self) -> None:
        def mutate(value: JsonObject) -> None:
            resize = json_object_at(value, "samples", 0, "phases", "same_session_resize")
            evidence = json_object(resize["resize_evidence"])
            evidence["to_viewport"] = {"width": 1281, "height": 900, "scale": 1}
            measurement = json_object(resize["measurement"])
            json_object(measurement["frame"]).update(
                viewport=evidence["to_viewport"], pixel_bytes=1281 * 900 * 4
            )

        self.mutate_report(self.report_paths[0][0], mutate)
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
            gate._verify_legacy_diagnostic(self.bundle, self.root)
        self.mutate_report(
            path,
            lambda value: json_object_at(value, "samples", 0).update(
                viewport={"width": 1280, "height": 900, "scale": 1}
            ),
        )
        candidate_path = next(
            report_path for report_path in self.report_paths[0]
            if report_path.name.startswith("candidate")
        )
        self.mutate_report(
            candidate_path,
            lambda value: json_object_at(value, "samples", 0, "phases", "full_y").update(
                actual_bottom=False
            ),
        )
        with self.assertRaisesRegex(ValueError, "full-Y span incomplete"):
            gate._verify_legacy_diagnostic(self.bundle, self.root)

    def test_quiet_load_violation_fails(self) -> None:
        self.mutate_json(self.manifest_paths[0], violate_quiet_load)
        with self.assertRaisesRegex(ValueError, "worker quiet load limit exceeded"):
            gate._verify_legacy_diagnostic(self.bundle, self.root)

    def test_duplicate_json_key_and_transport_traversal_fail(self) -> None:
        (self.bundle / "bundle.json").write_text('{"schema":"a","schema":"b"}')
        self.reject()
        json_object_at(self.transport, "files", 0)["relativePath"] = "../outside.json"
        self.save_transport()
        self.reject()


class AuthenticatedMeasurementInputRefsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path("/repo")
        self.snapshot: JsonObject = {"path": "/snapshot", "sha256": "a" * 64}
        self.binaries: dict[str, JsonObject] = {
            "baseline": {"path": "/baseline/bin", "sha256": "b" * 64},
            "candidate": {"path": "/candidate/bin", "sha256": "c" * 64},
        }
        self.expected: JsonObject = {
            "operationPlanRef": {"path": "/plan", "sha256": "d" * 64},
            "domStateRef": {"path": "/dom", "sha256": "e" * 64},
            "fixture_sha256": "f" * 64,
            "arms": {
                "baseline-0.4.23": {"binary_sha256": "b" * 64,
                                    "build_artifact_ref": {"path": "/baseline/proof", "sha256": "1" * 64}},
                "candidate": {"binary_sha256": "c" * 64,
                              "build_artifact_ref": {"path": "/candidate/proof", "sha256": "2" * 64}},
            },
        }
        refs = [json_object(self.expected["operationPlanRef"]),
                json_object(self.expected["domStateRef"]), self.snapshot,
                *gate.planned_execution_inputs(self.root, self.expected, self.binaries)]
        self.context: JsonObject = {"inputs": [{**pin, "exists": True} for pin in refs]
                                    + [{"path": "/capacity", "sha256": "3" * 64, "exists": True}]}

    def selected(self) -> dict[str, JsonObject]:
        return canonical_gate.authenticated_measurement_input_refs(
            self.root, self.context, self.expected, self.snapshot, None, [], [], self.binaries,
        )

    def test_returns_only_authenticated_planned_references(self) -> None:
        selected = self.selected()
        self.assertNotIn("/capacity", selected)
        self.assertEqual(selected["/candidate/bin"], self.binaries["candidate"])
        self.assertEqual(selected["/snapshot"], self.snapshot)
        self.assertIn(str(self.root / gate.HARNESS_ROOT / "Cargo.lock"), selected)

    def test_unknown_worker_or_unexpected_legacy_role_is_rejected(self) -> None:
        self.binaries["unknown"] = {"path": "/other", "sha256": "4" * 64}
        with self.assertRaisesRegex(ValueError, "execution binary roles differ"):
            self.selected()
        del self.binaries["unknown"]
        with self.assertRaisesRegex(ValueError, "unexpected measurement input role"):
            gate.authenticated_measurement_input_refs(
                self.root, self.context, self.expected, self.snapshot, None,
                [{"path": "/capacity", "sha256": "3" * 64}], [], self.binaries,
            )

    def test_catalog_refs_include_validated_capture_links_and_provenance(self) -> None:
        self.expected["catalogCondition"] = {"fixture_path": "/fixture"}
        keys = ("operationEvidenceRef", "catalogManifestRef", "catalogPlanRef",
                "catalogCaptureValidationRef", "catalogBinaryProofRef", "catalogBuildReportRef",
                "catalogCaptureBinaryRef", "catalogCurrentBuildReportRef", "chromeCaptureRef",
                "chromeReceiptRef", "fontIdentityRequestRef", "fontIdentityResponseRef",
                "fontIdentityProbeBinary")
        rows = json_array(self.context["inputs"])
        for key in keys:
            pin: JsonObject = {"path": "/" + key, "sha256": "5" * 64}
            self.expected[key] = pin
            rows.append({**pin, "exists": True})
        capture: JsonObject = {"path": "/capture", "sha256": "6" * 64}
        link: JsonObject = {"path": "/link", "sha256": "7" * 64}
        font: JsonObject = {"path": "/font", "sha256": "8" * 64}
        rows.extend({**pin, "exists": True} for pin in (capture, link, font))
        rows.append({"path": "/fixture", "sha256": "f" * 64, "exists": True})
        selected = gate.authenticated_measurement_input_refs(
            self.root, self.context, self.expected, self.snapshot, capture, [link], [font],
            self.binaries,
        )
        self.assertTrue({"/capture", "/link", "/font", "/fixture"} <= set(selected))
        self.assertTrue(all("/" + key in selected for key in keys))
        self.assertNotIn("/capacity", selected)

    def test_missing_or_changed_context_reference_is_rejected(self) -> None:
        rows = json_array(self.context["inputs"])
        row = next(json_object(value) for value in rows if json_object(value)["path"] == "/plan")
        row["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "measurement input context differs"):
            self.selected()
        rows.remove(row)
        with self.assertRaisesRegex(ValueError, "measurement input context is missing"):
            self.selected()


class BaselineArmContractTest(unittest.TestCase):
    def test_fresh_reference_plans_every_source_pin_before_worker(self) -> None:
        pins: list[JsonObject] = [{"path": f"/fresh/{index}", "sha256": str(index) * 64}
                                  for index in range(4)]
        contract: JsonObject = {"role": "fresh-history-source-record", "armId": "fresh-history-source-record",
                                "recordId": "record", "referenceKind": "fresh-history-source",
                                "publishedSdk": False, "originalArm": "candidate", "sourceRefs": pins}
        expected: JsonObject = {"referenceParent": contract, "fixture_sha256": "f" * 64,
            "arms": {"fresh-history-source-record": {"binary_sha256": "a" * 64,
                        "build_artifact_ref": {"path": "/baseline/proof", "sha256": "b" * 64}},
                     "candidate": {"binary_sha256": "c" * 64,
                        "build_artifact_ref": {"path": "/candidate/proof", "sha256": "d" * 64}}}}
        binaries: dict[str, JsonObject] = {"baseline": {"path": "/baseline/bin", "sha256": "a" * 64},
                                            "candidate": {"path": "/candidate/bin", "sha256": "c" * 64}}
        planned = gate.planned_execution_inputs(Path("/repo"), expected, binaries)
        for pin in pins:
            self.assertIn(pin, planned)
        contract["sourceRefs"] = pins[:-1]
        with self.assertRaisesRegex(ValueError, "source ref inventory differs"):
            gate.planned_execution_inputs(Path("/repo"), expected, binaries)

    def test_fresh_reference_keeps_distinct_role_and_no_legacy_exception(self) -> None:
        expected: JsonObject = {"referenceParent": {"role": "fresh-history-source-record",
                                "armId": "fresh-history-source-record", "recordId": "record",
                                "referenceKind": "fresh-history-source", "publishedSdk": False,
                                "originalArm": "candidate"},
                                "catalogCondition": {"condition_id": "navigation/all-toc-links/620x900@1",
                                                     "operation_kind": "navigation-all-pointer-clicks"}}
        self.assertEqual(gate.baseline_arm_id(expected), "fresh-history-source-record")
        self.assertIsNone(gate.non_comparable_baseline_fragment(expected))

    def test_execution_binary_role_rejects_same_hash_at_another_path(self) -> None:
        proof: JsonObject = {"path": "/old/binary", "sha256": "a" * 64}
        row: JsonObject = {"executable": "/old/binary", "executable_sha256": "a" * 64}
        gate.require_execution_binary_role(row, proof)
        row["executable"] = "/new/binary"
        with self.assertRaisesRegex(ValueError, "execution binary differs"):
            gate.require_execution_binary_role(row, proof)

    def test_default_keeps_legacy_public_sdk_arm(self) -> None:
        self.assertEqual(gate.baseline_arm_id({}), "baseline-0.4.23")

    def test_reference_parent_arm_requires_non_sdk_identity(self) -> None:
        expected = {"referenceParent": {
            "role": "reference-parent-371274ed",
            "armId": "reference-parent-371274ed",
            "referenceKind": "correct-parent-source",
            "publishedSdk": False,
        }}
        self.assertEqual(gate.baseline_arm_id(expected), "reference-parent-371274ed")
        expected["referenceParent"]["publishedSdk"] = True
        with self.assertRaisesRegex(ValueError, "identity differs"):
            gate.baseline_arm_id(expected)

    def test_history_reference_never_inherits_legacy_fragment_exception(self) -> None:
        contract: JsonObject = {"role": "reference-history-first", "armId": "reference-history-first",
                                "recordId": "first", "referenceKind": "accepted-history-candidate",
                                "publishedSdk": False, "originalArm": "candidate"}
        expected: JsonObject = {"referenceParent": contract, "catalogCondition": {
            "condition_id": "navigation/all-toc-links/620x900@1",
            "operation_kind": "navigation-all-pointer-clicks"}}
        self.assertEqual(gate.baseline_arm_id(expected), "reference-history-first")
        self.assertIsNone(gate.non_comparable_baseline_fragment(expected))
        for field, value in (("recordId", "other"), ("originalArm", "baseline"), ("publishedSdk", True)):
            with self.subTest(field=field):
                changed = dict(contract)
                changed[field] = value
                with self.assertRaises(ValueError):
                    gate.baseline_arm_id({"referenceParent": changed})


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

    def test_remote_formal_entry_requires_the_full_recomputed_result_to_match_receipt(self) -> None:
        first = gate.read_json(self.fixture.expected_paths[0])
        candidate = gate._object(gate._object(first["arms"], "fixture arms")["candidate"], "candidate arm")
        build: JsonObject = {"binarySha256": candidate["binary_sha256"],
                             "sourceSnapshotSha256": candidate["source_snapshot_sha256"]}
        result: JsonObject = {"head": self.fixture.head, "performanceNonregression": True,
                              "htmlStage90": {"acceptance": True, "fontIdentityEquivalence": "established"}}
        with patch.object(attestation, "verify_remote_attestation"), \
             patch.object(remote_evidence, "extract_remote_transport"), \
             patch.object(remote_evidence, "validate_producer_verification_receipt", return_value=result), \
             patch.object(remote_evidence, "validate_remote_candidate_build", return_value=build), \
             patch.object(remote_evidence, "validate_remote_history_baseline", return_value={}), \
             patch.object(canonical_gate, "_require_catalog_attempt_set"), \
             patch.object(canonical_gate, "_verify", return_value=result) as verify:
            verified = remote_admission.verify_authenticated_remote(
                self.fixture.bundle, self.fixture.root / "artifact.zip", self.fixture.root,
                self.repository, {}, "artifact", self.fixture.head)
            self.assertEqual(verified, result)
            self.assertIsInstance(verify.call_args.kwargs["remote_context"], remote_admission.RemoteEvidenceContext)
            verify.return_value = {**result, "head": "b" * 40}
            with self.assertRaisesRegex(ValueError, "remote formal result differs"):
                remote_admission.verify_authenticated_remote(
                    self.fixture.bundle, self.fixture.root / "artifact.zip", self.fixture.root,
                    self.repository, {}, "artifact", self.fixture.head)

    def test_attestation_requires_exact_signed_run_attempt_and_subject(self) -> None:
        archive = self.fixture.root.parent / "attested.zip"
        archive.write_bytes(b"signed artifact")
        name = "paired-artifact"
        run: JsonObject = {"id": 101, "run_attempt": 2, "head_sha": "a" * 40,
                           "event": "push", "head_branch": "release/v0.4.24"}
        statement: JsonObject = {"predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{"name": name, "digest": {"sha256": gate.sha256(archive)}}],
            "predicate": {"runDetails": {"metadata": {
                "invocationId": "https://github.com/owner/repo/actions/runs/101/attempts/2"}}}}
        source_ref = "refs/heads/release/v0.4.24"
        workflow_uri = "https://github.com/owner/repo/.github/workflows/test-and-build.yml@" + source_ref
        certificate: JsonObject = {"runInvocationURI": "https://github.com/owner/repo/actions/runs/101/attempts/2",
            "sourceRepositoryDigest": "a" * 40, "buildSignerDigest": "b" * 40, "buildConfigDigest": "c" * 40,
            "sourceRepositoryURI": "https://github.com/owner/repo", "sourceRepositoryRef": source_ref,
            "runnerEnvironment": "github-hosted", "buildSignerURI": workflow_uri, "buildConfigURI": workflow_uri}
        row: JsonObject = {"attestation": {"bundle": True}, "verificationResult": {
            "signature": {"certificate": certificate}, "verifiedTimestamps": [], "statement": statement}}
        def verified() -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], 0, stdout=json.dumps([row]), stderr="")
        with patch.object(gate.subprocess, "run", side_effect=lambda *_args, **_kwargs: verified()) as command, \
             patch.object(attestation, "_require_reviewed_workflow_blob"):
            attestation.verify_remote_attestation(archive, self.fixture.root, self.repository, run, name, "a" * 40)
            argv = command.call_args.args[0]
            self.assertIn("--deny-self-hosted-runners", argv)
            run["run_attempt"] = 3
            with self.assertRaisesRegex(ValueError, "certificate run or subject differs"):
                attestation.verify_remote_attestation(archive, self.fixture.root, self.repository, run, name, "a" * 40)
            run["run_attempt"] = 2
            with self.assertRaisesRegex(ValueError, "certificate run or subject differs"):
                attestation.verify_remote_attestation(archive, self.fixture.root, self.repository, run, "other", "a" * 40)

    def test_pr_attestation_uses_merge_certificate_and_independent_checkout_head(self) -> None:
        archive = self.fixture.root.parent / "pr-attested.zip"
        archive.write_bytes(b"signed artifact")
        head, base, merge, signer = (character * 40 for character in "abcd")
        run: JsonObject = {"id": 101, "run_attempt": 1, "head_sha": head, "event": "pull_request",
                           "head_branch": "release/v0.4.24",
                           "pull_requests": [{"number": 7, "base": {"sha": base}, "head": {"sha": head}}]}
        ref = "refs/pull/7/merge"
        uri = "https://github.com/owner/repo/.github/workflows/test-and-build.yml@" + ref
        signer_uri = "https://github.com/owner/repo/.github/workflows/test-and-build.yml@refs/heads/release/v0.4.24"
        certificate: JsonObject = {"runInvocationURI": "https://github.com/owner/repo/actions/runs/101/attempts/1",
            "sourceRepositoryDigest": merge, "buildSignerDigest": signer, "buildConfigDigest": merge,
            "sourceRepositoryURI": "https://github.com/owner/repo", "sourceRepositoryRef": ref,
            "runnerEnvironment": "github-hosted", "buildSignerURI": signer_uri, "buildConfigURI": uri}
        statement: JsonObject = {"predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{"name": "artifact", "digest": {"sha256": gate.sha256(archive)}}],
            "predicate": {"runDetails": {"metadata": {"invocationId": certificate["runInvocationURI"]}}}}
        row: JsonObject = {"attestation": {"bundle": True}, "verificationResult": {
            "signature": {"certificate": certificate}, "verifiedTimestamps": [], "statement": statement}}
        completed = subprocess.CompletedProcess([], 0, stdout=json.dumps([row]), stderr="")
        commit: JsonObject = {"sha": merge, "parents": [{"sha": base}, {"sha": head}]}
        with patch.object(gate.subprocess, "run", return_value=completed), \
             patch.object(attestation, "_github_json", return_value=commit), \
             patch.object(attestation, "_require_reviewed_workflow_blob") as workflow:
            attestation.verify_remote_attestation(archive, self.fixture.root, self.repository, run, "artifact", head)
            self.assertEqual({call.args[2] for call in workflow.call_args_list}, {merge, signer})
            certificate["runInvocationURI"] = "https://github.com/owner/repo/actions/runs/102/attempts/1"
            completed.stdout = json.dumps([row])
            with self.assertRaisesRegex(ValueError, "certificate run or subject differs"):
                attestation.verify_remote_attestation(archive, self.fixture.root, self.repository, run, "artifact", head)

    def test_producer_receipt_requires_bound_head_role_scope_and_manifest(self) -> None:
        directory = self.fixture.root.parent / "receipt-fixture"
        directory.mkdir()
        manifest = directory / "bundle.json"
        manifest.write_text(json.dumps({"schema": "krr-html-performance-release-bundle-v1",
                                        "head": self.fixture.head, "files": [], "attempts": [{}] * 84}))
        result: JsonObject = {"head": self.fixture.head, "conditions": 84, "candidateConditions": 84,
                              "checks": 1680, "performanceNonregression": True,
                              "htmlStage90": {"acceptance": True, "fontIdentityEquivalence": "established"}}
        receipt: JsonObject = {"schema": "krr-html-performance-producer-verification-v1",
                               "role": "producer-formal-verification", "head": self.fixture.head,
                               "bundleManifestSha256": gate.sha256(manifest), "result": result}
        path = directory / "producer-verification.json"
        path.write_text(json.dumps(receipt))
        self.assertEqual(remote_evidence.validate_producer_verification_receipt(directory, self.fixture.head), result)
        for field, value in (("role", "other"), ("head", "b" * 40), ("bundleManifestSha256", "0" * 64)):
            with self.subTest(field=field):
                changed = dict(receipt)
                changed[field] = value
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):
                    remote_evidence.validate_producer_verification_receipt(directory, self.fixture.head)
        path.write_text(json.dumps(receipt))
        manifest.write_text(manifest.read_text() + " ")
        with self.assertRaisesRegex(ValueError, "receipt identity differs"):
            remote_evidence.validate_producer_verification_receipt(directory, self.fixture.head)

    def acquire_case(self, case: str) -> None:
        run: JsonObject = {"run_started_at": "2026-10-10T01:00:00Z", "id": 101, "head_sha": self.fixture.head, "head_repository": {"full_name": self.repository}, "path": ".github/workflows/test-and-build.yml", "status": "completed", "conclusion": "success", "run_attempt": 1}
        if case == "pending":
            run["status"] = "in_progress"
        elif case == "failed":
            run["conclusion"] = "failure"
        elif case == "foreign_repository":
            json_object_at(run, "head_repository")["full_name"] = "foreign/repo"
        producer = case.startswith("producer_")
        manual_local = case.startswith("manual_local_")
        if manual_local:
            run.update(event="pull_request", repository={"full_name": self.repository},
                       pull_requests=[{"number": 7, "head": {"sha": self.fixture.head},
                                       "base": {"sha": "b" * 40}}])
            if case == "manual_local_wrong_head":
                json_object_at(run, "pull_requests", 0, "head")["sha"] = "c" * 40
            if case == "manual_local_no_pr":
                run["pull_requests"] = []
            if case == "manual_local_multiple_pr":
                run["pull_requests"] = [json_object_at(run, "pull_requests", 0)] * 2
            if case == "manual_local_wrong_repo":
                run["repository"] = {"full_name": "foreign/repo"}
            if case == "manual_local_push":
                run["event"] = "push"
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
                    relative = str(file.relative_to(self.fixture.bundle))
                    content = file.read_bytes()
                    if case == "remote_wrong_head" and relative == "bundle.json":
                        document = json.loads(content)
                        document["head"] = "c" * 40
                        content = json.dumps(document).encode()
                    elif case == "remote_changed_payload" and relative != "bundle.json":
                        content = b"modified"
                    zipped.writestr(relative, content)
            if case == "zip_traversal":
                zipped.writestr("../escape", b"bad")
            elif case == "duplicate_zip":
                # 故意の重複ZIPを検証するため、fixture作成時の警告だけを抑える。
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore", UserWarning)
                    zipped.writestr("bundle.json", b"duplicate")
            elif case == "remote_extra_file":
                zipped.writestr("extra", b"unexpected")
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
                if case in {"producer_skipped_dependency", "producer_local_skipped_dependency",
                            "manual_local_skipped_dependency"}:
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

        first = gate.read_json(self.fixture.expected_paths[0])
        candidate = gate._object(gate._object(first["arms"], "fixture arms")["candidate"], "candidate arm")
        candidate_build = {"binarySha256": candidate["binary_sha256"],
                           "sourceSnapshotSha256": candidate["source_snapshot_sha256"]}
        with patch.object(gate.subprocess, "check_output", synthetic_api), \
             patch.object(local_html, "verify_from_api", return_value=(case.startswith("producer_local")
                 or (manual_local and case not in {"manual_local_expired", "manual_local_api_failure"}))), \
             patch.object(attestation, "verify_remote_attestation"), \
             patch.object(remote_evidence, "validate_producer_verification_receipt", return_value={}), \
             patch.object(canonical_gate, "_require_catalog_attempt_set"), \
             patch.object(remote_evidence, "validate_remote_candidate_build", return_value=candidate_build), \
             patch.object(remote_evidence, "validate_remote_history_baseline"), \
             patch.object(canonical_gate, "_verify", return_value={}), \
             patch.object(canonical_gate, "_require_formal_release_result"):
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
        self.assertEqual(gate._verify_legacy_diagnostic(self.destination, self.fixture.root)["checks"], 60)

    def test_current_producer_can_use_verified_local_evidence(self) -> None:
        self.acquire_case("producer_local_success")
        self.assertEqual(gate.read_json(self.destination / "acquisition.json")["kind"],
                         "owner-local-html-performance")
        self.assertFalse(any(path.endswith("/artifacts?per_page=100") for path in self.calls))

    def test_completed_pr_can_use_verified_local_evidence(self) -> None:
        self.acquire_case("manual_local_success")
        self.assertEqual(gate.read_json(self.destination / "acquisition.json")["kind"],
                         "owner-local-html-performance")
        self.assertFalse(any(path.endswith("/artifacts?per_page=100") for path in self.calls))

    def test_completed_pr_local_proof_requires_matching_pr_and_successful_jobs(self) -> None:
        with self.assertRaises(ValueError):
            self.acquire_case("manual_local_skipped_dependency")
        self.assertFalse(self.destination.exists())

    def test_completed_pr_expired_or_unavailable_proof_uses_artifact(self) -> None:
        for case in ("manual_local_expired", "manual_local_api_failure", "manual_local_push",
                     "manual_local_wrong_head", "manual_local_no_pr", "manual_local_multiple_pr",
                     "manual_local_wrong_repo"):
            with self.subTest(case=case):
                self.destination = self.fixture.root.parent / case
                self.calls.clear()
                self.acquire_case(case)
                self.assertTrue(any(path.endswith("/artifacts?per_page=100") for path in self.calls))

    def test_local_evidence_still_requires_every_completed_dependency(self) -> None:
        with self.assertRaisesRegex(ValueError, "dependency did not succeed"):
            self.acquire_case("producer_local_skipped_dependency")
        self.assertFalse(self.destination.exists())

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
        self.assertEqual(gate._verify_legacy_diagnostic(self.destination, self.fixture.root)["checks"], 60)
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

    def test_remote_transport_rejects_wrong_head_changed_payload_and_extra_files(self) -> None:
        for case, message in (("remote_wrong_head", "remote bundle HEAD differs"),
                              ("remote_changed_payload", "transport file hash mismatch"),
                              ("remote_extra_file", "remote bundle archive inventory differs")):
            with self.subTest(case=case):
                self.destination = self.fixture.root.parent / case
                with self.assertRaisesRegex(ValueError, message):
                    self.acquire_case(case)

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
    def test_cached_acquisition_is_reauthenticated_before_formal_gate_reuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            existing = root / "evidence"
            existing.mkdir()
            (existing / "acquisition.json").write_text("{}")
            result: JsonObject = {"head": "a" * 40}
            argv = ["verify_html_performance_evidence.py", "--repo", str(root),
                    "--evidence-dir", str(existing), "--repository", "owner/repo"]
            with patch.object(gate.sys, "argv", argv), \
                 patch.object(gate.sys, "stdout", io.StringIO()) as stdout, \
                 patch.object(gate, "acquire", return_value=result) as acquire, \
                 patch.object(gate, "verify") as local_verify:
                self.assertEqual(gate.main(), 0)
            self.assertEqual(json.loads(stdout.getvalue()), result)
            self.assertNotEqual(acquire.call_args.args[0], existing)
            self.assertEqual(acquire.call_args.args[1:3], (root, "owner/repo"))
            local_verify.assert_not_called()

    def test_local_acquisition_marker_is_reauthenticated_in_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            existing = root / "evidence"
            existing.mkdir()
            (existing / "acquisition.json").write_text('{"kind":"owner-local-html-performance"}')
            argv = ["verify_html_performance_evidence.py", "--repo", str(root),
                    "--evidence-dir", str(existing), "--repository", "owner/repo"]
            environment = {"GITHUB_JOB": "preflight", "GITHUB_EVENT_NAME": "pull_request"}
            with patch.object(gate.sys, "argv", argv), patch.dict(os.environ, environment), \
                 patch.object(gate.sys, "stdout", io.StringIO()), \
                 patch.object(gate, "acquire", return_value={"kind": "owner-local-html-performance"}) as acquire:
                self.assertEqual(gate.main(), 0)
            self.assertTrue(acquire.call_args.args[3])
            with patch.object(gate.sys, "argv", argv), patch.dict(os.environ, environment), \
                 patch.object(gate.sys, "stderr", io.StringIO()), \
                 patch.object(gate, "acquire", side_effect=ValueError("expired")):
                self.assertEqual(gate.main(), 1)

    def test_candidate_build_cli_emits_only_build_identity_pins(self) -> None:
        expected: JsonObject = {
            "binarySha256": "a" * 64,
            "sourceHead": "head",
            "sourceSnapshotSha256": "b" * 64,
            "lockSha256": "c" * 64,
            "buildArtifactSha256": "d" * 64,
            "cargoExecutableSha256": "e" * 64,
        }
        with patch.object(gate.sys, "argv", ["verify_html_performance_evidence.py", "--repo", ".",
                    "--evidence-dir", "bundle", "--candidate-build-proof", "build/candidate.json",
                    "--build-artifact-sha256", "f" * 64, "--bundle-manifest-sha256", "0" * 64]), \
             patch.object(gate.sys, "stdout", io.StringIO()) as stdout, \
             patch.object(gate, "validate_candidate_build_proof", return_value=expected) as verify:
            self.assertEqual(gate.main(), 0)
        verify.assert_called_once_with(Path("bundle"), Path(".").resolve(), "0" * 64,
                                       "build/candidate.json", "f" * 64)
        self.assertEqual(json.loads(stdout.getvalue()), expected)
        self.assertFalse({"accepted", "performancePassed", "htmlAccepted"} & set(json.loads(stdout.getvalue())))

    def test_public_verifier_rejects_diagnostic_html84_result(self) -> None:
        result: JsonObject = {
            "htmlStage90": {"acceptance": False, "fontIdentityEquivalence": "not-established"},
            "performanceNonregression": True,
        }
        with patch.object(gate, "_verify", return_value=result):
            with self.assertRaisesRegex(ValueError, "HTML84 visual acceptance is not established"):
                gate.verify(Path("bundle"), Path("repo"))

    def test_private_catalog_diagnostic_retains_false_acceptance(self) -> None:
        result: JsonObject = {
            "htmlStage90": {"acceptance": False, "fontIdentityEquivalence": "not-established"},
            "performanceNonregression": True,
        }
        with patch.object(gate, "_verify", return_value=result) as verify:
            self.assertIs(gate._verify_catalog_diagnostic(Path("bundle"), Path("repo")), result)
        verify.assert_called_once_with(
            Path("bundle"), Path("repo"), git_query=None,
            require_catalog=True, diagnostic_catalog=True,
        )

    def test_diagnostic_cli_reaches_catalog_diagnostic_verifier(self) -> None:
        result: JsonObject = {
            "htmlStage90": {"acceptance": False, "fontIdentityEquivalence": "not-established"},
            "performanceNonregression": True,
        }
        with patch.object(gate.sys, "argv", [
            "verify_html_performance_evidence.py", "--repo", "repo",
            "--evidence-dir", "bundle", "--diagnostic-only",
        ]), patch.object(gate.sys, "stdout", io.StringIO()) as stdout, \
             patch.object(gate, "_verify_catalog_diagnostic", return_value=result) as verify:
            self.assertEqual(gate.main(), 0)
        verify.assert_called_once_with(Path("bundle"), Path("repo"))
        self.assertEqual(json.loads(stdout.getvalue()), result)

    def test_formal_release_result_requires_visual_font_and_performance_acceptance(self) -> None:
        accepted: JsonObject = {
            "htmlStage90": {"acceptance": True, "fontIdentityEquivalence": "established"},
            "performanceNonregression": True,
        }
        gate._require_formal_release_result(accepted)
        rejected: tuple[tuple[str, object, str], ...] = (
            ("htmlStage90", {"acceptance": False, "fontIdentityEquivalence": "established"},
             "HTML84 visual acceptance"),
            ("htmlStage90", {"acceptance": True, "fontIdentityEquivalence": "not-established"},
             "same-font equivalence"),
            ("performanceNonregression", False, "performance non-regression"),
        )
        for key, value, message in rejected:
            with self.subTest(key=key, value=value):
                candidate = dict(accepted)
                candidate[key] = value
                with self.assertRaisesRegex(ValueError, message):
                    gate._require_formal_release_result(candidate)

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
