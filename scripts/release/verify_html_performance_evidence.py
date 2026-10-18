#!/usr/bin/env python3
"""実測paired evidenceを再集計し、公開前の時間・RSS非劣化を検証する。"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import zipfile
from pathlib import Path
from datetime import datetime
from typing import Protocol, TypedDict, cast
from collections.abc import Sequence

JsonObject = dict[str, object]
SourceRecords = dict[str, tuple[str, int]]

class GitQuery(Protocol):
    def __call__(self, root: Path, args: Sequence[str], input_bytes: bytes | None,
                 text: bool) -> str | bytes: ...


class SourceContract(TypedDict):
    head: str
    source: SourceRecords
    snapshot: str


class Percentiles(TypedDict):
    median: float
    p95: float


class RssPercentiles(Percentiles):
    max: float


class PhaseAggregate(TypedDict):
    sample_count: int
    elapsed_ms: Percentiles
    peak_rss_bytes: RssPercentiles


class ArmAggregate(TypedDict):
    arm: str | None
    viewport: object
    sample_count: int
    phases: dict[str, PhaseAggregate]


class ComparisonAggregate(TypedDict):
    measurement_executed: bool
    acceptance: bool
    reason: str
    viewport: object
    arms: dict[str, ArmAggregate]
    comparison: dict[str, dict[str, float]]

VIEWPORTS = ((1280, 900), (960, 720), (620, 900))
LOCK_SHA = "d55bdde3425b7af3057eb758a2e4241196c7d8e187320b506334aee76af0a4e5"
SOURCE_KEYS = ("inventoryDigest", "head", "index", "files", "trackedPaths", "untrackedPaths", "missingTrackedPaths")
ENDPOINT_FLAGS = ("sourceBeforeEqualsEnd", "inputsBeforeEqualsEnd", "registryBeforeEqualsEnd", "cacheBeforeEqualsEnd", "cacheUnionBeforeEqualsEnd", "cacheUnionScopeRestored", "configBeforeEqualsEnd", "preferenceBeforeEqualsEnd", "toolContextBeforeEqualsEnd")
HARNESS_ROOT = "scripts/html/performance-harness/"


class PendingEvidence(ValueError):
    pass


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def digest(value: object) -> None:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None, "invalid SHA256")


def _object(value: object, label: str) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise ValueError(f"{label} must be an object")
    return cast(JsonObject, value)


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise ValueError(f"{label} must be an array")
    return cast("list[object]", value)


def _objects(value: object, label: str) -> list[JsonObject]:
    return [_object(item, label) for item in _array(value, label)]


def _text(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be text")
    return value


def _integer(value: object, label: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{label} must be an integer")
    return value


def _numeric(value: object, label: str) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    return value


def _fixture_expected_state(fixture: Path) -> object:
    matches = re.findall(
        r'<script type="application/json" id="expected-state">([^<]*)</script>',
        fixture.read_text(encoding="utf-8"),
    )
    require(len(matches) == 1, "catalog fixture must contain one expected-state JSON script")
    return json.loads(matches[0])


def _same_json(left: object, right: object) -> bool:
    return json.dumps(left, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
                      allow_nan=False) == json.dumps(
        right, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    )


def catalog_operation_stream(operation_kind: str, evidence_value: object,
                             expected_state: object | None = None,
                             report_state: object | None = None) -> list[JsonObject]:
    evidence = _object(evidence_value, "typed operation evidence")
    steps = _objects(evidence.get("steps", []), "catalog operation step")
    if operation_kind in {"load-and-observe-target", "script-load-observe",
                          "input-state-empty", "input-state-disabled-observe"}:
        require(not steps, "catalog observation/reopen operation unexpectedly dispatched inputs")
        expected_delivery = {
            "load-and-observe-target": "session-load",
            "script-load-observe": "session-load",
        }.get(operation_kind)
        if expected_delivery is not None:
            require(evidence.get("delivery") == expected_delivery,
                    "catalog observation operation delivery evidence differs")
        return []
    if operation_kind == "script-await-mutation-delivery":
        state = _object(evidence.get("state"), "mutation operation state")
        require("before" in state and "after" in state,
                "mutation operation state transition is missing")
        if evidence.get("delivery") == "mutation-observer":
            require(not _same_json(state["before"], state["after"]),
                    "mutation observer did not prove a state transition")
            return []
        require(evidence.get("delivery") == "session-load-settled"
                and set(state) == {"before", "after"}
                and set(evidence) == {
                    "schema", "kind", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery",
                }
                and len(steps) == 1
                and set(steps[0]) == {"kind", "stateBefore", "stateAfter"}
                and steps[0].get("kind") == "session-load-settled-observer-state"
                and expected_state is not None and report_state is not None
                and all(_same_json(value, expected_state) for value in (
                    state["before"], state["after"], steps[0]["stateBefore"],
                    steps[0]["stateAfter"], report_state,
                )),
                "settled mutation evidence differs from its fixture callback log")
        return []
    if operation_kind in {"input-state-selected", "input-state-expanded", "navigation-pointer-click"}:
        stream = _catalog_pointer_stream(steps)
        if operation_kind == "navigation-pointer-click":
            require(isinstance(evidence.get("hashBefore"), str)
                    and isinstance(evidence.get("hashAfter"), str)
                    and evidence.get("hashAfter") != evidence.get("hashBefore")
                    and evidence.get("destination") == evidence.get("hashAfter"),
                    "navigation operation lacks its actual requested hash transition")
        return stream
    if operation_kind == "input-state-filled-save":
        require(len(steps) == 7 and [step.get("kind") for step in steps] ==
                ["move", "down", "up", "text", "move", "down", "up"],
                "filled/save operation evidence is missing its exact pointer/text sequence")
        stream = _catalog_pointer_stream(steps[:3])
        text_step = steps[3]
        require(set(text_step) == {"kind", "targetNodeId", "text", "before", "after"}
                and text_step.get("text") == "架空の名称"
                and _integer(text_step.get("targetNodeId"), "text target node") > 0
                and text_step.get("targetNodeId") == steps[0].get("targetNodeId")
                and text_step.get("after") == text_step.get("text")
                and isinstance(text_step.get("before"), str),
                "filled/save text evidence differs")
        stream.append({"kind": "text", "text": text_step["text"]})
        stream.extend(_catalog_pointer_stream(steps[4:]))
        return stream
    if operation_kind == "input-state-focus-visible-tab":
        require(len(steps) == 1 and steps[0].get("kind") == "key" and steps[0].get("key") == "Tab"
                and _integer(steps[0].get("targetNodeId"), "Tab target node") > 0,
                "focus-visible operation is missing its actual Tab key step")
        return [{"kind": "key_down", "key": "Tab"}]
    if operation_kind == "navigation-all-pointer-clicks":
        require(evidence.get("delivery") == "session-load"
                and set(evidence) == {
                    "schema", "kind", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery", "links",
                }
                and evidence.get("sourceSha256Before") == evidence.get("sourceSha256After"),
                "all-TOC operation evidence is not bound to one fresh fixture source")
        links = _objects(evidence.get("links"), "catalog TOC link")
        require(len(links) == 3 and [link.get("destination") for link in links] == ["#a", "#b", "#c"],
                "all-TOC evidence does not contain the exact ordered three links")
        traces = [_objects(link.get("pointerTrace"), "TOC pointer trace") for link in links]
        require(all(len(trace) == 3 for trace in traces),
                "TOC link is missing its nested actual pointer trace")
        flattened = [step for trace in traces for step in trace]
        require(steps == flattened, "all-TOC top-level steps differ from nested link traces")
        previous_hash: str | None = None
        capture_paths: set[str] = set()
        target_node_ids: set[int] = set()
        for link_index, (destination, link) in enumerate(zip(("#a", "#b", "#c"), links, strict=True)):
            require(set(link) == {
                "destination", "hash", "scrollY", "capturePath", "captureSha256",
                "targetVisible", "svgPath", "svgSha256", "hashBefore", "hashAfter",
                "frame", "pointerTrace", "sourceSha256",
            }, "all-TOC link evidence fields differ")
            hash_before = _text(link.get("hashBefore"), "TOC hash before")
            require(link.get("hash") == destination and link.get("hashAfter") == destination
                    and hash_before != destination
                    and (previous_hash is None or hash_before == previous_hash),
                    "all-TOC actual hash transition differs")
            previous_hash = destination
            require(link.get("targetVisible") is True
                    and link.get("sourceSha256") == evidence.get("sourceSha256Before"),
                    "all-TOC target visibility or fixture source differs")
            capture_path = _text(link.get("capturePath"), "TOC capture path")
            require(Path(capture_path).is_absolute(), "all-TOC capture path must be absolute")
            capture_paths.add(capture_path)
            digest(link.get("captureSha256"))
            scroll_y = _numeric(link.get("scrollY"), "TOC captured scroll")
            frame = _object(link.get("frame"), "TOC captured frame")
            require(set(frame) == {"generation", "contentHeight", "scrollY"}
                    and _integer(frame.get("generation"), "TOC frame generation") > 0
                    and math.isfinite(_numeric(frame.get("contentHeight"), "TOC content height"))
                    and _numeric(frame.get("contentHeight"), "TOC content height") > 0
                    and math.isfinite(scroll_y) and scroll_y >= 0
                    and _numeric(frame.get("scrollY"), "TOC frame scroll") == scroll_y,
                    "all-TOC actual frame/scroll evidence differs")
            for metadata_path, metadata_sha in ((link.get("svgPath"), link.get("svgSha256")),):
                require(Path(_text(metadata_path, "TOC SVG path")).is_absolute(),
                        "all-TOC SVG path must be absolute")
                digest(metadata_sha)
            first_pointer = traces[link_index][0]
            target_node = _integer(first_pointer.get("targetNodeId"), "TOC target node")
            require(target_node not in target_node_ids, "TOC links reused an actual target node")
            target_node_ids.add(target_node)
            final_pointer = traces[link_index][2]
            final_frame = _object(final_pointer.get("postFrame"), "final TOC pointer frame")
            require(final_frame.get("generation") == frame.get("generation")
                    and final_frame.get("scrollY") == frame.get("scrollY"),
                    "TOC screenshot frame differs from the actual final pointer frame")
        require(len(capture_paths) == 3, "all-TOC links must have separate capture files")
        return _catalog_pointer_stream(steps)
    if operation_kind == "script-await-timer-microtask":
        require(len(steps) == 1 and set(steps[0]) == {
                    "kind", "elapsedMs", "timerRan", "microtaskCheckpoint"
                } and steps[0].get("kind") == "host-time-advance"
                and steps[0].get("timerRan") is True
                and steps[0].get("microtaskCheckpoint") == "runtime-advance-timers",
                "timer operation has no observed timer dispatch")
        elapsed = _numeric(steps[0].get("elapsedMs"), "timer elapsed milliseconds")
        require(math.isfinite(elapsed) and 0 <= elapsed <= 60_000,
                "timer operation elapsed time is outside the supported range")
        return [{"kind": "advance_time", "elapsed_ms": elapsed}]
    if operation_kind == "script-real-events":
        require(evidence.get("delivery") == "session-load"
                and set(evidence) == {
                    "schema", "kind", "sourceSha256Before", "sourceSha256After",
                    "preFrame", "postFrame", "target", "state", "steps", "delivery", "events",
                }
                and evidence.get("sourceSha256Before") == evidence.get("sourceSha256After"),
                "script event operation does not prove one fresh fixture load")
        require(len(steps) == 9
                and [step.get("kind") for step in steps] == [
                    "move", "down", "up", "text", "key_down", "key_up", "move", "down", "up",
                ], "script event operation steps differ from the actual dispatch order")
        events = _objects(evidence.get("events"), "script event")
        event_types = ["focus", "input", "keydown", "keyup", "click", "submit"]
        require(len(events) == len(event_types)
                and [event.get("type") for event in events] == event_types,
                "script event evidence omits or reorders a dispatched event")
        script_first_pointer = _catalog_pointer_stream(steps[:3])
        script_last_pointer = _catalog_pointer_stream(steps[6:])
        field_id = _integer(steps[0].get("targetNodeId"), "script event field node")
        text_step, down_step, up_step = steps[3:6]
        require(set(text_step) == {"kind", "targetNodeId", "text", "stateBefore", "stateAfter"}
                and text_step.get("text") == "入力"
                and text_step.get("targetNodeId") == field_id
                and set(down_step) == {"kind", "targetNodeId", "key", "stateBefore", "stateAfter"}
                and down_step.get("targetNodeId") == field_id and down_step.get("key") == "Shift"
                and set(up_step) == {"kind", "targetNodeId", "key", "stateBefore", "stateAfter"}
                and up_step.get("targetNodeId") == field_id and up_step.get("key") == "Shift",
                "script event typed input steps differ")
        expected_fields = {
            "focus": {"type", "targetNodeId", "focused", "stateBefore", "stateAfter"},
            "input": {"type", "targetNodeId", "stateBefore", "stateAfter"},
            "keydown": {"type", "targetNodeId", "key", "stateBefore", "stateAfter"},
            "keyup": {"type", "targetNodeId", "key", "stateBefore", "stateAfter"},
            "click": {"type", "targetNodeId", "stateBefore", "stateAfter"},
            "submit": {"type", "targetNodeId", "stateBefore", "stateAfter"},
        }
        require(all(set(event) == expected_fields.get(_text(event.get("type"), "script event type"))
                    and _integer(event.get("targetNodeId"), "script event target") > 0
                    and "stateBefore" in event and "stateAfter" in event
                    for event in events), "script event fields or observed states differ")
        require(events[0].get("focused") is True
                and events[0].get("targetNodeId") == field_id
                and events[1].get("targetNodeId") == field_id
                and events[2].get("targetNodeId") == field_id
                and events[3].get("targetNodeId") == field_id
                and events[2].get("key") == down_step.get("key")
                and events[3].get("key") == up_step.get("key")
                and events[4].get("targetNodeId") == _integer(steps[6].get("targetNodeId"), "submit button node")
                and _same_json(events[4].get("stateBefore"), events[5].get("stateBefore"))
                and _same_json(events[4].get("stateAfter"), events[5].get("stateAfter")),
                "script event records do not bind to the observed dispatch steps")
        for event, step in ((events[1], text_step), (events[2], down_step), (events[3], up_step)):
            require(_same_json(event.get("stateBefore"), step.get("stateBefore"))
                    and _same_json(event.get("stateAfter"), step.get("stateAfter")),
                    "script event state differs from its actual typed step")
        state = _object(evidence.get("state"), "script event operation state")
        require(set(state) == {"before", "after"}
                and _same_json(state["before"], events[0].get("stateBefore"))
                and _same_json(state["after"], events[5].get("stateAfter"))
                and expected_state is not None and report_state is not None
                and all(_same_json(left, right) for left, right in (
                    (events[0].get("stateAfter"), events[1].get("stateBefore")),
                    (events[1].get("stateBefore"), text_step.get("stateBefore")),
                    (events[1].get("stateAfter"), text_step.get("stateAfter")),
                    (events[1].get("stateAfter"), events[2].get("stateBefore")),
                    (events[2].get("stateBefore"), down_step.get("stateBefore")),
                    (events[2].get("stateAfter"), down_step.get("stateAfter")),
                    (events[2].get("stateAfter"), events[3].get("stateBefore")),
                    (events[3].get("stateBefore"), up_step.get("stateBefore")),
                    (events[3].get("stateAfter"), up_step.get("stateAfter")),
                    (events[3].get("stateAfter"), events[4].get("stateBefore")),
                    (events[4].get("stateBefore"), events[5].get("stateBefore")),
                    (events[4].get("stateAfter"), events[5].get("stateAfter")),
                    (state["after"], report_state),
                    (report_state, expected_state),
                    (events[5].get("stateAfter"), expected_state),
                )),
                "script event operation state differs from its observed event trace")
        text_input: JsonObject = {"kind": "text", "text": text_step["text"]}
        key_down = {"kind": "key_down", "key": down_step["key"]}
        key_up = {"kind": "key_up", "key": up_step["key"]}
        return [*script_first_pointer, text_input, key_down, key_up, *script_last_pointer]
    if operation_kind == "script-await-resource-font":
        raise ValueError("resource/font operation has no complete observed operation evidence")
    raise ValueError(f"unsupported finite catalog operation: {operation_kind}")


def _catalog_pointer_stream(steps: list[JsonObject]) -> list[JsonObject]:
    require(len(steps) >= 3 and len(steps) % 3 == 0,
            "catalog pointer trace must contain complete move/down/up groups")
    stream: list[JsonObject] = []
    for index in range(0, len(steps), 3):
        group = steps[index:index + 3]
        move, down, up = group
        require([step.get("kind") for step in group] == ["move", "down", "up"],
                "catalog pointer trace order differs")
        for step in group:
            expected_fields = {"kind", "targetNodeId", "x", "y"}
            if "preFrame" in step or "postFrame" in step:
                require({"preFrame", "postFrame"} <= set(step)
                        and set(step) <= expected_fields | {"preFrame", "postFrame"},
                        "catalog pointer frame fields differ")
                for frame_key in ("preFrame", "postFrame"):
                    frame = _object(step.get(frame_key), "pointer frame")
                    require(set(frame) in ({"generation", "scrollY"},
                                           {"generation", "contentHeight", "scrollY"})
                            and _integer(frame.get("generation"), "pointer generation") > 0
                            and math.isfinite(_numeric(frame.get("scrollY"), "pointer scroll"))
                            and _numeric(frame.get("scrollY"), "pointer scroll") >= 0
                            and ("contentHeight" not in frame or (
                                math.isfinite(_numeric(frame.get("contentHeight"), "pointer content height"))
                                and _numeric(frame.get("contentHeight"), "pointer content height") >= 0)),
                            "catalog pointer frame identity is incomplete")
            else:
                require(set(step) == expected_fields, "catalog pointer dispatch fields differ")
            require(_integer(step.get("targetNodeId"), "pointer target node") > 0,
                    "catalog pointer trace has no target node")
            for coordinate in ("x", "y"):
                number = _numeric(step.get(coordinate), "pointer " + coordinate)
                require(math.isfinite(number), "catalog pointer coordinate is not finite")
            require(move["targetNodeId"] == down["targetNodeId"] == up["targetNodeId"]
                and move["x"] == down["x"] == up["x"]
                and move["y"] == down["y"] == up["y"],
                "catalog pointer trace target or coordinates differ")
        stream.extend([
            {"kind": "pointer_move", "x": move["x"], "y": move["y"]},
            {"kind": "pointer_down", "x": down["x"], "y": down["y"], "button": 0},
            {"kind": "pointer_up", "x": up["x"], "y": up["y"], "button": 0},
        ])
    return stream


def unique_object(pairs: list[tuple[str, object]]) -> JsonObject:
    result = dict(pairs)
    require(len(result) == len(pairs), "duplicate JSON keys")
    return result


def read_json(path: Path) -> JsonObject:
    return _object(json.loads(path.read_bytes(), object_pairs_hook=unique_object), "JSON document")


def _git_output(root: Path, args: Sequence[str], *, git_query: GitQuery | None,
                input_bytes: bytes | None = None, text: bool = False) -> str | bytes:
    if git_query is not None:
        output = git_query(root, args, input_bytes, text)
    else:
        command = ["git", "-C", str(root), *args]
        if input_bytes is None:
            output = subprocess.check_output(command, text=text)
        else:
            output = subprocess.check_output(command, input=input_bytes, text=text)
    if text:
        require(isinstance(output, str), "Git text query returned binary output")
    else:
        require(isinstance(output, bytes), "Git binary query returned text output")
    return output


def git(root: Path, *args: str, git_query: GitQuery | None = None) -> str:
    output = _git_output(root, args, git_query=git_query, text=True)
    if not isinstance(output, str):
        raise ValueError("Git text query returned binary output")
    return output


def records_map(rows: object) -> SourceRecords:
    result: SourceRecords = {}
    for row in _objects(rows, "source records"):
        key = _text(row["path"], "source path")
        require(isinstance(key, str) and key not in result, "duplicate source path")
        checksum = _text(row["sha256"], "source SHA256")
        digest(checksum)
        size = _integer(row["size"], "source size")
        require(size >= 0, "invalid source size")
        result[key] = (checksum, size)
    return result


class Bundle:
    # 絶対パスを含む元証跡は書き換えず、輸送索引だけでファイルを再配置する。
    def __init__(self, directory: Path) -> None:
        self.directory = directory.resolve()
        self.manifest = read_json(self.directory / "bundle.json")
        require(self.manifest["schema"] == "krr-html-performance-release-bundle-v1", "unsupported transport schema")
        self.files: dict[str, tuple[Path, str]] = {}
        used = set()
        for row in _objects(self.manifest["files"], "transport files"):
            original = _text(row["originalPath"], "original path")
            relative = Path(_text(row["relativePath"], "transport path"))
            path = (self.directory / relative).resolve()
            require(Path(original).is_absolute() and original not in self.files, "duplicate/nonabsolute original path")
            require(not relative.is_absolute() and path.is_relative_to(self.directory) and path not in used, "invalid transport path")
            checksum = _text(row["sha256"], "transport SHA256")
            digest(checksum)
            require(path.is_file() and sha256(path) == checksum, "transport file hash mismatch")
            self.files[original] = (path, checksum)
            used.add(path)

    def file(self, reference: object) -> Path:
        ref = _object(reference, "file reference")
        digest(ref["sha256"])
        path, recorded = self.files[_text(ref["path"], "file reference path")]
        require(recorded == ref["sha256"] and sha256(path) == recorded, "original pin mismatch")
        return path

    def json(self, ref: object) -> JsonObject:
        return read_json(self.file(ref))


def committed_records(root: Path, ref: str = "HEAD", *, git_query: GitQuery | None = None) -> SourceRecords:
    tree = _git_output(root, ("ls-tree", "-r", "-z", ref), git_query=git_query)
    if not isinstance(tree, bytes):
        raise ValueError("Git tree query returned text output")
    rows = []
    for entry in tree.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, kind, oid = metadata.decode().split()
        require(kind == "blob" and mode in ("100644", "100755"), "unsupported committed source type")
        rows.append((os.fsdecode(raw_path), oid))
    payload = _git_output(root, ("cat-file", "--batch"), git_query=git_query,
                          input_bytes="".join(oid + "\n" for _, oid in rows).encode())
    if not isinstance(payload, bytes):
        raise ValueError("Git blob query returned text output")
    result: SourceRecords = {}
    offset = 0
    for name, oid in rows:
        header_end = payload.index(b"\n", offset)
        actual_oid, kind, size = payload[offset:header_end].decode().split()
        require(actual_oid == oid and kind == "blob", "committed blob query differs")
        start, length = header_end + 1, int(size)
        data = payload[start:start + length]
        require(len(data) == length and payload[start + length:start + length + 1] == b"\n", "truncated committed blob")
        result[name] = (hashlib.sha256(data).hexdigest(), length)
        offset = start + length + 1
    require(offset == len(payload), "extra committed blob data")
    return result


def current_source(root: Path, before: JsonObject, *, git_query: GitQuery | None = None) -> SourceRecords:
    require(before["schema"] == "krr-native-build-source-inventory-v1", "unsupported source schema")
    require(before["sourceManifestSchema"] == "krr-native-build-source-manifest-v1", "unsupported source manifest")
    require(before["head"] == git(root, "rev-parse", "HEAD", git_query=git_query).strip(), "stale source HEAD")
    require(not git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query), "release source is dirty or untracked")
    origin = Path(_text(before["root"], "source root"))
    require(origin.is_absolute(), "absolute source root required")
    def relative(name: str) -> str:
        return str(Path(name).relative_to(origin))
    committed = committed_records(root, git_query=git_query)
    saved_tracked = [relative(_text(p, "tracked path")) for p in _array(before["trackedPaths"], "tracked paths")]
    require(len(set(saved_tracked)) == len(saved_tracked) and set(saved_tracked) == set(committed), "committed source membership differs")
    require(before["untrackedPaths"] == [] and before["missingTrackedPaths"] == [], "measured source is untracked or missing")
    saved = {relative(p): value for p, value in records_map(before["files"]).items()}
    require(saved == committed, "measured source differs from complete committed HEAD blobs")
    for name, (checksum, size) in committed.items():
        path = root / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size == size and sha256(path) == checksum, "working source differs from committed HEAD: " + name)
    return saved


def source_contract(bundle: Bundle, root: Path, expected: JsonObject, *,
                    git_query: GitQuery | None = None) -> SourceContract:
    contract = _object(expected["sourcePair"], "source pair")
    before, end, approved = (bundle.json(contract[k]) for k in ("before", "end", "approvedSourceRef"))
    require(all(document.get("stableEndpoints") is True for document in (before, end, approved)), "unstable source endpoint")
    require(all(before[k] == end[k] == approved[k] for k in SOURCE_KEYS), "source epoch differs")
    require(before["inventoryDigest"] == contract["approvedInventoryDigest"], "approved inventory differs")
    approved_ref = _object(contract["approvedSourceRef"], "approved source reference")
    matches = [p for p in _objects(contract["inputPins"], "source input pins") if p.get("path") == approved_ref["path"]]
    require(len(matches) == 1 and matches[0].get("sha256") == approved_ref["sha256"], "approved source pin missing")
    saved = current_source(root, before, git_query=git_query)
    snapshot = bundle.json(contract["snapshot"])
    rows = snapshot["records"]
    actual = records_map(rows)
    require(set(actual) == set(saved) and all(actual[p] == value for p, value in saved.items()), "snapshot source subset/extra")
    require(actual[HARNESS_ROOT + "Cargo.lock"][0] == LOCK_SHA, "harness lock differs")
    require(snapshot["digest"] == hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), "snapshot digest differs")
    snapshot_ref = _object(contract["snapshot"], "source snapshot reference")
    for arm_value in _object(expected["arms"], "expected arms").values():
        arm = _object(arm_value, "expected arm")
        require(arm["source_snapshot_sha256"] == snapshot_ref["sha256"] and arm["lock_sha256"] == LOCK_SHA, "shared build provenance differs")
    return {"head": _text(before["head"], "source HEAD"), "source": saved, "snapshot": _text(snapshot_ref["sha256"], "snapshot SHA256")}


PHASES = ("cold_open", "warm_open", "re_display", "full_y", "post_operation", "same_session_resize")
DISPLAY_COMPLETION_PHASES = (
    "cold_open",
    "warm_open",
    "re_display",
    "post_operation",
    "same_session_resize",
)
DISPLAY_COMPLETION_LIMIT_MS = 3_000.0


def _number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise ValueError(f"{label} must be finite and positive")
    return number


def _nonnegative_number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise ValueError(f"{label} must be finite and nonnegative")
    return number


def _percentile(values: list[float], percentile: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * percentile
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def _phase_sample(sample: JsonObject, phase: str) -> tuple[float, float]:
    phases = sample.get("phases")
    if not isinstance(phases, dict) or phase not in phases:
        raise ValueError(f"sample is missing phase {phase}")
    value = phases[phase]
    if phase in ("post_operation", "same_session_resize"):
        if not isinstance(value, dict):
            raise ValueError("post_operation must be an object")
        value = value.get("measurement")
    if not isinstance(value, dict):
        raise ValueError(f"phase {phase} must be an object")
    elapsed = _number(value.get("elapsed_ms"), f"{phase}.elapsed_ms")
    rss = _number(value.get("peak_rss_bytes"), f"{phase}.peak_rss_bytes")
    return elapsed, rss


def _validate_report(report: JsonObject, expected: JsonObject) -> tuple[str, list[JsonObject]]:
    arm = report.get("arm")
    expected_arms = _object(expected["arms"], "expected arms")
    if not isinstance(arm, str) or arm not in expected_arms:
        raise ValueError(f"unknown arm provenance: {arm!r}")
    source_sha = report.get("source_sha256")
    if source_sha != expected["fixture_sha256"]:
        raise ValueError("report fixture SHA does not match expected provenance")
    viewport = report.get("viewport")
    if not isinstance(viewport, dict) or set(("width", "height", "scale")) - viewport.keys():
        raise ValueError("report has incomplete viewport provenance")
    if viewport != expected["viewport"]:
        raise ValueError("report viewport does not match expected provenance")
    for key in ("operation_plan_sha256", "operation", "expected_dom_state_sha256"):
        if report.get(key) != expected[key]:
            raise ValueError(f"report metadata mismatch for {key}")
    provenance = report.get("provenance")
    expected_arm = _object(expected_arms[arm], "expected arm")
    if not isinstance(provenance, dict):
        raise ValueError("report has no provenance")
    for key in ("binary_sha256", "source_snapshot_sha256", "deps_lock_sha256"):
        expected_key = {"binary_sha256": "binary_sha256", "source_snapshot_sha256": "source_snapshot_sha256", "deps_lock_sha256": "lock_sha256"}[key]
        if provenance.get(key) != expected_arm.get(expected_key, expected.get(expected_key)):
            raise ValueError(f"{arm} provenance mismatch for {key}")
    for key in ("rustc", "cargo", "environment_sha256"):
        if provenance.get(key) != _object(expected["provenance"], "expected provenance")[key]:
            raise ValueError(f"{arm} provenance mismatch for {key}")
    catalog = expected.get("catalogCondition")
    if catalog is not None:
        identity = _object(catalog, "expected catalog condition")
        evidence_ref = _object(expected["operationEvidenceRef"], "operation evidence reference")
        if (
            report.get("condition_id") != identity["condition_id"]
            or report.get("case_id") != identity["case_id"]
            or report.get("state") != identity["state"]
            or report.get("operation_kind") != identity["operation_kind"]
            or report.get("operation_evidence_ref") != evidence_ref
            or report.get("catalog_condition") != identity
        ):
            raise ValueError("report catalog condition identity differs")
    samples = report.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError("report has no samples")
    if report.get("sample_count") != len(samples):
        raise ValueError("sample_count does not match samples")
    sample_objects = _objects(samples, "sample")
    for sample in sample_objects:
        for phase in PHASES:
            elapsed, _ = _phase_sample(sample, phase)
            require(arm != "candidate" or phase not in DISPLAY_COMPLETION_PHASES
                    or elapsed < DISPLAY_COMPLETION_LIMIT_MS,
                    f"candidate {phase} frame completion must be under 3000 ms")
        full_y = _object(_object(sample["phases"], "sample phases")["full_y"], "full-Y phase")
        if arm == "candidate":
            maximum_frame_elapsed = _nonnegative_number(
                full_y.get("max_frame_elapsed_ms"), "full-Y max frame elapsed"
            )
            require(maximum_frame_elapsed < DISPLAY_COMPLETION_LIMIT_MS,
                    "candidate full-Y frame completion must be under 3000 ms")
            scroll_steps = _integer(full_y["scroll_steps"], "full-Y scroll steps")
            max_scroll = _nonnegative_number(full_y.get("max_scroll"), "full-Y max scroll")
            final_scroll = _nonnegative_number(full_y.get("final_scroll_y"), "full-Y final scroll")
            bottom_frame = _object(full_y.get("frame"), "full-Y bottom frame")
            frame_scroll = _nonnegative_number(
                bottom_frame.get("scroll_y"), "full-Y bottom frame scroll"
            )
            full_y_ok = (
                full_y["actual_bottom"] is True
                and max_scroll == final_scroll == frame_scroll
                and scroll_steps >= 0
                and (
                    (scroll_steps > 0 and max_scroll > 0)
                    or (catalog is not None and scroll_steps == 0 and max_scroll == 0)
                )
            )
            require(full_y_ok, "full-Y span incomplete")
        resize = _object(_object(sample["phases"], "sample phases")["same_session_resize"],
                         "same-session resize")
        evidence = _object(resize.get("resize_evidence"), "resize evidence")
        from_viewport = _object(evidence.get("from_viewport"), "resize source viewport")
        to_viewport = _object(evidence.get("to_viewport"), "resize target viewport")
        from_width = _integer(from_viewport.get("width"), "resize source width")
        from_height = _integer(from_viewport.get("height"), "resize source height")
        resize_width = _integer(to_viewport.get("width"), "resize target width")
        resize_height = _integer(to_viewport.get("height"), "resize target height")
        generation_before = _integer(evidence.get("generation_before"), "resize generation before")
        generation_after = _integer(evidence.get("generation_after"), "resize generation after")
        require(from_viewport == expected["viewport"] and to_viewport != from_viewport,
                "same-session resize viewport transition differs")
        expected_resize_width = (from_width - max(from_width // 4, 1)) if from_width > 1 else 2
        require(to_viewport.get("scale") == from_viewport.get("scale")
                and resize_width == expected_resize_width and resize_height == from_height,
                "same-session resize target viewport is invalid")
        require(generation_after > generation_before
                and evidence.get("real_raster_frame") is True,
                "same-session resize did not produce a new raster frame")
        operation = _object(_object(sample["phases"], "sample phases")["post_operation"], "sample operation")
        catalog = expected.get("catalogCondition")
        if catalog is None:
            if operation["operation"] != expected["operation"]:
                raise ValueError("sample operation does not match operation plan")
            if operation["expected_dom_state_sha256"] != expected["expected_dom_state_sha256"]:
                raise ValueError("sample expected-state SHA does not match operation plan")
        else:
            identity = _object(catalog, "expected catalog condition")
            if sample.get("catalog_condition") != {
                "profile": "catalog-plan84",
                "condition_id": identity["condition_id"],
                "case_id": identity["case_id"],
                "state": identity["state"],
                "operation_kind": identity["operation_kind"],
                "operation_evidence_sha256": _object(expected["operationEvidenceRef"], "operation evidence reference")["sha256"],
            }:
                raise ValueError("sample catalog condition identity differs")
    return arm, sample_objects


def aggregate_reports(reports: list[JsonObject], expected: JsonObject) -> ArmAggregate:
    if not reports:
        raise ValueError("at least one report is required")
    samples: list[JsonObject] = []
    arm: str | None = None
    viewport: object = None
    for report in reports:
        report_arm, report_samples = _validate_report(report, expected)
        if arm is None:
            arm = report_arm
            viewport = report["viewport"]
        elif report_arm != arm or report["viewport"] != viewport:
            raise ValueError("reports do not share arm and viewport provenance")
        samples.extend(report_samples)
    aggregates: dict[str, PhaseAggregate] = {}
    for phase in PHASES:
        elapsed = [_phase_sample(sample, phase)[0] for sample in samples]
        rss = [_phase_sample(sample, phase)[1] for sample in samples]
        aggregates[phase] = {
            "sample_count": len(elapsed),
            "elapsed_ms": {"median": _percentile(elapsed, 0.5), "p95": _percentile(elapsed, 0.95)},
            "peak_rss_bytes": {"median": _percentile(rss, 0.5), "p95": _percentile(rss, 0.95), "max": max(rss)},
        }
    return {"arm": arm, "viewport": viewport, "sample_count": len(samples), "phases": aggregates}


def aggregate_comparison(report_paths: list[Path], expected: JsonObject) -> ComparisonAggregate:
    """交互順序の5組が揃ったときだけ集計する。"""
    if len(report_paths) != 10 or len({path.resolve() for path in report_paths}) != 10:
        raise ValueError("exactly ten distinct pair reports are required")
    grouped: dict[str, dict[int, JsonObject]] = {"baseline": {}, "candidate": {}}
    report_arm_for_filename = {"baseline": "baseline-0.4.23", "candidate": "candidate"}
    pattern = re.compile(r"^(baseline|candidate)-(\d+)\.json$")
    for path in report_paths:
        match = pattern.match(path.name)
        if not match:
            raise ValueError(f"report filename must identify arm and pair: {path.name}")
        arm, pair_text = match.groups()
        pair = int(pair_text)
        if pair not in range(5) or pair in grouped[arm]:
            raise ValueError("pair reports must contain each arm exactly once for pairs 0..4")
        report = read_json(path)
        report_arm, samples = _validate_report(report, expected)
        if report_arm != report_arm_for_filename[arm] or report.get("pair_index") != pair:
            raise ValueError("report arm or pair index does not match its filename")
        sample_ids = [_integer(sample.get("sample"), "sample id") for sample in samples]
        if sample_ids != list(range(5)) or len(set(sample_ids)) != 5:
            raise ValueError("each pair must contain unique samples 0..4")
        grouped[arm][pair] = report
    if any(set(pairs) != set(range(5)) for pairs in grouped.values()):
        raise ValueError("both arms must contain all five pair reports")
    arms = {
        arm: aggregate_reports([grouped[arm][pair] for pair in range(5)], expected)
        for arm in grouped
    }
    comparisons: dict[str, dict[str, float]] = {}
    for phase in PHASES:
        base = arms["baseline"]["phases"][phase]
        cand = arms["candidate"]["phases"][phase]
        comparisons[phase] = {
            "elapsed_ms_delta_median": cand["elapsed_ms"]["median"] - base["elapsed_ms"]["median"],
            "elapsed_ms_delta_p95": cand["elapsed_ms"]["p95"] - base["elapsed_ms"]["p95"],
            "peak_rss_bytes_delta_median": cand["peak_rss_bytes"]["median"] - base["peak_rss_bytes"]["median"],
            "peak_rss_bytes_delta_p95": cand["peak_rss_bytes"]["p95"] - base["peak_rss_bytes"]["p95"],
        }
    return {
        "measurement_executed": all(report.get("measurement_executed") is True for reports in grouped.values() for report in reports.values()),
        "acceptance": False,
        "reason": "non-regression acceptance remains false until paired measurement policy is applied",
        "viewport": expected["viewport"],
        "arms": arms,
        "comparison": comparisons,
    }



def validate_attempt(bundle: Bundle, root: Path, attempt: JsonObject, *,
                     git_query: GitQuery | None = None) -> JsonObject:
    expected = bundle.json(attempt["expected"])
    result = bundle.json(attempt["result"])
    manifest = bundle.json(attempt["pairManifest"])
    require(result.get("measurementExecuted") is True and all(result.get(k) is True for k in ENDPOINT_FLAGS), "measurement/endpoint proof absent or false")
    require(not result.get("failure") and not result.get("endpointFailure"), "failed measurement attempt")
    expected_arms = _object(expected["arms"], "expected arms")
    provenance = _object(expected["provenance"], "expected provenance")
    source_pair = _object(expected["sourcePair"], "source pair")
    require(set(expected_arms) == {"baseline-0.4.23", "candidate"}, "versioned baseline/candidate required")
    catalog_expected = expected.get("catalogCondition")
    for key in ("fixture_sha256", "operation_plan_sha256"):
        digest(expected[key])
    if catalog_expected is None:
        digest(expected["expected_dom_state_sha256"])
    for arm_value in expected_arms.values():
        expected_arm = _object(arm_value, "expected arm")
        for key in ("binary_sha256", "source_snapshot_sha256", "lock_sha256"):
            digest(expected_arm[key])
    digest(provenance["environment_sha256"])
    for tool in ("rustc", "cargo"):
        tool_version = _text(provenance[tool], "tool provenance")
        require(tool_version.startswith(tool + " ") and "\n" in tool_version, "actual tool provenance absent")
    viewport = _object(expected["viewport"], "viewport")
    width, height = (_integer(viewport[key], "viewport dimension") for key in ("width", "height"))
    require(set(viewport) == {"width", "height", "scale"} and type(viewport["scale"]) in (int, float) and viewport["scale"] == 1, "invalid viewport types/scale")
    require(isinstance(expected["operation"], str) and bool(expected["operation"]), "missing operation")
    source = source_contract(bundle, root, expected, git_query=git_query)
    endpoints = _object(expected["endpointRefs"], "endpoint references")
    before_context, end_context = (bundle.json(endpoints[k]) for k in ("before", "end"))
    mapping = {"sourceBeforeEqualsEnd": "source", "inputsBeforeEqualsEnd": "inputs", "registryBeforeEqualsEnd": "registry", "cacheBeforeEqualsEnd": "cache", "cacheUnionBeforeEqualsEnd": "cacheUnion", "cacheUnionScopeRestored": "scope", "configBeforeEqualsEnd": "config", "preferenceBeforeEqualsEnd": "preference", "toolContextBeforeEqualsEnd": "toolContext"}
    require(all(before_context[key] == end_context[key] for key in mapping.values()), "raw endpoint context changed")
    require(before_context["source"] == bundle.json(source_pair["before"]) and end_context["source"] == bundle.json(source_pair["end"]), "source/context endpoint differs")
    require(provenance["environment_sha256"] == _object(endpoints["before"], "before endpoint reference")["sha256"], "measurement context pin differs")
    require(before_context["cacheUnion"] == {"registry": before_context["registry"], "target": before_context["cache"]}, "cache union proof differs")
    for tool in ("rustc", "cargo"):
        require(_object(before_context["toolContext"], "tool context")[tool] == provenance[tool], "measured tool context differs")
    plan = bundle.json(expected["operationPlanRef"])
    dom = bundle.json(expected["domStateRef"])
    operation_ref = _object(expected["operationPlanRef"], "operation reference")
    dom_ref = _object(expected["domStateRef"], "DOM reference")
    require(operation_ref["sha256"] == expected["operation_plan_sha256"], "operation plan pin differs")
    if catalog_expected is None:
        require(dom_ref["sha256"] == expected["expected_dom_state_sha256"], "DOM pin differs")
        require(plan["fixture_sha256"] == expected["fixture_sha256"] and plan["operation"] == expected["operation"] and plan["expected_dom_state_sha256"] == expected["expected_dom_state_sha256"] and plan["expected_dom_state_path"] == dom_ref["path"], "operation plan differs")
        require(source["source"]["tests/fixtures/html/interaction-fidelity.html"][0] == expected["fixture_sha256"], "performance fixture is not committed source")
        require(dom["inputSha256"] == expected["fixture_sha256"] and _object(dom["states"], "DOM states")["js"] == {"expanded": True, "result": "opened"}, "actual interaction DOM evidence absent")
    else:
        catalog = _object(catalog_expected, "catalog condition")
        evidence_ref = _object(expected.get("operationEvidenceRef"), "operation evidence reference")
        require(dom_ref == evidence_ref and plan.get("profile") == "catalog-plan84"
                and plan.get("conditionId") == catalog["condition_id"]
                and plan.get("caseId") == catalog["case_id"]
                and plan.get("state") == catalog["state"]
                and plan.get("operationKind") == catalog["operation_kind"]
                and plan.get("fixtureSha256") == expected["fixture_sha256"]
                and plan.get("operationEvidenceSha256") == evidence_ref["sha256"],
                "catalog operation plan/evidence binding differs")
        fixture_relative = Path(_text(catalog["fixture_path"], "catalog fixture path")).relative_to(root).as_posix()
        require(source["source"].get(fixture_relative, (None,))[0] == expected["fixture_sha256"],
                "catalog fixture is not committed source")
        require(dom.get("status") == "observed" and dom.get("conditionId") == catalog["condition_id"]
                and dom.get("caseId") == catalog["case_id"] and dom.get("boundaryState") == catalog["state"]
                and dom.get("operationKind") == catalog["operation_kind"]
                and dom.get("htmlSha256") == expected["fixture_sha256"],
                "catalog operation evidence is not observed or is mismatched")
        raw_viewport = _object(dom.get("viewport"), "raw catalog viewport")
        evidence_path = _text(evidence_ref["path"], "operation evidence path")
        evidence_sha = _text(evidence_ref["sha256"], "operation evidence SHA")
        operation_evidence = _object(dom.get("operationEvidence"), "raw operation evidence")
        require(raw_viewport == viewport
                and dom.get("binarySha256") == plan.get("evidenceBinarySha256")
                and manifest.get("binarySha256") == plan.get("evidenceBinarySha256")
                and plan.get("operationEvidencePath") == evidence_path
                and plan.get("operationEvidenceSha256") == evidence_sha
                and operation_evidence.get("schema") == "krr-html84-operation-evidence-v1"
                and operation_evidence.get("kind") == catalog.get("operation_kind")
                and operation_evidence.get("sourceSha256Before") == expected["fixture_sha256"]
                and operation_evidence.get("sourceSha256After") == expected["fixture_sha256"]
                and plan.get("operationStream") == catalog_operation_stream(
                    _text(catalog["operation_kind"], "catalog operation kind"), operation_evidence,
                    (_fixture_expected_state(root / fixture_relative)
                     if catalog.get("operation_kind") in {
                         "script-await-mutation-delivery", "script-real-events",
                     } else None),
                    dom.get("state"),
                ),
                "catalog raw report, binary, or replayable operation stream differs")
        capture_path = _text(dom.get("capturePath"), "raw report capture path")
        capture_sha = _text(dom.get("captureSha256"), "raw report capture SHA")
        digest(capture_sha)
        capture_ref: JsonObject = {"path": capture_path, "sha256": capture_sha}
        bundle.file(capture_ref)
        link_capture_refs: list[JsonObject] = []
        if catalog.get("operation_kind") == "navigation-all-pointer-clicks":
            links = _objects(operation_evidence.get("links"), "raw TOC links")
            for link in links:
                link_path = _text(link.get("capturePath"), "TOC capture path")
                link_sha = _text(link.get("captureSha256"), "TOC capture SHA")
                digest(link_sha)
                reference: JsonObject = {"path": link_path, "sha256": link_sha}
                bundle.file(reference)
                link_capture_refs.append(reference)
            require(len(link_capture_refs) == 3, "catalog TOC capture count differs")
        target = _object(operation_evidence.get("target"), "raw operation target")
        require(target.get("visible") is True
                and _integer(target.get("nodeId"), "raw target node") > 0
                and (catalog.get("target_selector") is None
                     or target.get("selector") == catalog.get("target_selector")),
                "catalog raw operation target differs")
    context_rows = _objects(before_context["inputs"], "context inputs")
    context_inputs = {_text(row["path"], "context input path"): row for row in context_rows}
    require(len(context_inputs) == len(context_rows), "duplicate context input")
    input_refs = [operation_ref, dom_ref, _object(source_pair["snapshot"], "snapshot reference")]
    if catalog_expected is not None:
        input_refs.extend([
            _object(expected["operationEvidenceRef"], "operation evidence reference"),
            _object(expected["catalogManifestRef"], "catalog manifest reference"),
            _object(expected["catalogPlanRef"], "catalog plan reference"),
            _object(expected["catalogCaptureValidationRef"], "Chrome validation reference"),
            _object(expected["chromeCaptureRef"], "Chrome capture reference"),
            _object(expected["chromeReceiptRef"], "Chrome receipt reference"),
            capture_ref,
            *link_capture_refs,
        ])
    for ref in input_refs:
        context_input = context_inputs[_text(ref["path"], "input reference path")]
        require(context_input["exists"] is True and context_input["sha256"] == ref["sha256"], "measurement input context differs")
    require(manifest["measurement_executed"] is True and manifest["input_sha256"] == expected["fixture_sha256"] and manifest["environment_sha256"] == provenance["environment_sha256"], "runner context differs")
    if catalog_expected is not None:
        catalog = _object(catalog_expected, "catalog condition")
        require(manifest.get("condition_id") == catalog["condition_id"]
                and manifest.get("operation_evidence_ref") == expected["operationEvidenceRef"],
                "catalog pair manifest condition/evidence differs")
    order, pins = _objects(manifest["order"], "pair order"), _objects(expected["reportPins"], "report pins")
    require(len(order) == len(pins) == 10, "five paired runs required")
    paths = []
    for index in range(len(order)):
        row, pin = order[index], pins[index]
        pair = index // 2
        arm = ("baseline", "candidate")[index % 2] if pair % 2 == 0 else ("candidate", "baseline")[index % 2]
        raw_arm = "baseline-0.4.23" if arm == "baseline" else arm
        binary_sha = _object(expected_arms[raw_arm], "expected arm")["binary_sha256"]
        require(row["arm"] == arm and type(row["pair_index"]) is int and row["pair_index"] == pair and row["executable_sha256"] == binary_sha, "pair order/binary differs")
        bundle.file({"path": row["executable"], "sha256": binary_sha})
        binary_context = context_inputs[_text(row["executable"], "executable path")]
        require(binary_context["exists"] is True and binary_context["sha256"] == binary_sha, "binary context proof differs")
        require(row["output"] == pin["path"] and Path(_text(pin["path"], "report path")).name == f"{arm}-{pair}.json", "report path/order differs")
        path = bundle.file(pin)
        require(path.name == f"{arm}-{pair}.json", "transport must retain report basename")
        report = read_json(path)
        require(report["measurement_executed"] is True and type(report["pair_index"]) is int and type(report["sample_count"]) is int and report["sample_count"] == 5, "actual five samples required")
        if catalog_expected is not None:
            catalog = _object(catalog_expected, "catalog condition")
            require(report.get("condition_id") == catalog["condition_id"]
                    and report.get("case_id") == catalog["case_id"]
                    and report.get("state") == catalog["state"]
                    and report.get("operation_kind") == catalog["operation_kind"]
                    and report.get("operation_evidence_ref") == expected["operationEvidenceRef"]
                    and report.get("catalog_condition") == catalog,
                    "catalog raw performance report identity differs")
        samples = _objects(report["samples"], "report samples")
        require([s["sample"] for s in samples] == list(range(5)) and all(type(s["sample"]) is int and set(_object(s["phases"], "sample phases")) == set(PHASES) for s in samples), "incomplete sample/phase coverage")
        for sample in samples:
            require(sample["viewport"] == viewport, "worker viewport differs")
            phases = _object(sample["phases"], "sample phases")
            for phase in PHASES:
                measured = _object(phases[phase], "measured phase")
                if phase in ("post_operation", "same_session_resize"):
                    measured = _object(measured["measurement"], "post-operation measurement")
                frame = _object(measured["frame"], "measured frame")
                expected_frame_viewport = viewport
                if phase == "same_session_resize":
                    resize = _object(phases[phase], "same-session resize")
                    resize_evidence = _object(resize["resize_evidence"], "resize evidence")
                    expected_frame_viewport = _object(resize_evidence["to_viewport"], "resize target viewport")
                expected_pixels = (_integer(expected_frame_viewport["width"], "frame viewport width")
                                   * _integer(expected_frame_viewport["height"], "frame viewport height") * 4)
                require(frame["viewport"] == expected_frame_viewport and frame["pixel_bytes"] == expected_pixels, "actual rendered viewport differs")
        if arm == "candidate":
            operation = _object(phases["post_operation"], "post operation")
            if catalog_expected is None:
                digest(operation["pixel_sha_before"])
                digest(operation["pixel_sha_after"])
                require(_numeric(operation["generation_after"], "generation after") > _numeric(operation["generation_before"], "generation before") and operation["pixel_sha_before"] != operation["pixel_sha_after"] and operation["visual_change_observed"] is True, "operation did not produce a changed frame")
            else:
                operation_evidence = _object(operation.get("catalog_operation"), "catalog operation result")
                catalog = _object(catalog_expected, "catalog condition")
                operation_plan = _object(plan, "catalog operation plan")
                require(operation_evidence.get("condition_id") == catalog["condition_id"]
                        and operation_evidence.get("case_id") == catalog["case_id"]
                        and operation_evidence.get("state") == catalog["state"]
                        and operation_evidence.get("operation_kind") == catalog["operation_kind"]
                        and operation_evidence.get("operation_evidence_sha256") == _object(expected["operationEvidenceRef"], "operation evidence ref")["sha256"],
                        "catalog post-operation identity differs")
                if not operation_plan["operationStream"]:
                    require(operation_evidence.get("delivery") == "measured-close-and-new-session-open",
                            "catalog static state did not use a measured close/reopen")
                else:
                    dispatched = _array(operation_evidence.get("dispatched_inputs"), "dispatched catalog inputs")
                    expected_kinds = [{"pointer_move": "pointer_move", "pointer_down": "pointer_down",
                                       "pointer_up": "pointer_up", "text": "text", "key_down": "key_down",
                                       "key_up": "key_up", "scroll": "scroll", "advance_time": "advance_time"}[_text(_object(item, "catalog operation stream item")["kind"], "catalog operation kind")]
                                      for item in _array(operation_plan["operationStream"], "operation stream")]
                    require(dispatched == expected_kinds, "catalog dispatched inputs differ from observed operation stream")
        loads = _objects(row["workerLoads"], "worker loads")
        require(len(loads) == 5, "all five worker load observations required")
        for load in loads:
            require(set(load) == {"before", "after"} and all(type(load[k]) in (int, float) and math.isfinite(_numeric(load[k], "worker load")) and 0 <= _numeric(load[k], "worker load") <= 0.25 for k in ("before", "after")), "worker quiet load limit exceeded")
        require(row["load_ratio_before"] == max(_numeric(x["before"], "worker before load") for x in loads) and row["load_ratio_after"] == max(_numeric(x["after"], "worker after load") for x in loads), "worker load aggregate differs")
        for key in ("load_ratio_before", "load_ratio_after"):
            value = _numeric(row[key], "quiet load")
            require(type(value) in (int, float) and math.isfinite(value) and 0 <= value <= 0.25, "quiet load limit exceeded")
        paths.append(path)
    aggregate = aggregate_comparison(paths, expected)
    require(aggregate["measurement_executed"] is True, "actual measurements absent")
    saved = _object(result["policy"], "measurement policy")
    phase_records = len(PHASES) * 50
    require(saved["aggregate"] == aggregate and saved["deltaThreshold"] == 0 and saved["quietLoadLimit"] == 0.25 and saved["phaseRecords"] == phase_records, "saved policy/aggregate differs from raw reports")
    policy = {phase: {key: value <= 0 for key, value in deltas.items()} for phase, deltas in aggregate["comparison"].items()}
    require(saved["policy"] == policy and saved["policyPass"] is True and all(ok for phase in policy.values() for ok in phase.values()), "time/RSS median/p95 regression")
    require(all(a["sample_count"] == 25 and all(p["sample_count"] == 25 for p in a["phases"].values()) for a in aggregate["arms"].values()), f"{phase_records} phase records required")
    # 診断用acceptance:falseは許可し、生データの20条件だけで受理する。
    result = {"viewport": expected["viewport"], "source": source, "fixture": expected["fixture_sha256"], "operation": expected["operation"], "plan": expected["operation_plan_sha256"], "dom": expected["expected_dom_state_sha256"], "provenance": expected["provenance"], "arms": expected["arms"]}
    if catalog_expected is not None:
        result["catalogCondition"] = catalog_expected
        result["catalogManifestRef"] = expected["catalogManifestRef"]
        result["catalogPlanRef"] = expected["catalogPlanRef"]
        result["operationEvidenceRef"] = expected["operationEvidenceRef"]
        result["catalogCaptureValidationRef"] = expected["catalogCaptureValidationRef"]
        result["chromeCaptureRef"] = expected["chromeCaptureRef"]
        result["chromeReceiptRef"] = expected["chromeReceiptRef"]
        result["fontIdentityRequestRef"] = expected["fontIdentityRequestRef"]
        result["fontIdentityResponseRef"] = expected["fontIdentityResponseRef"]
        result["fontIdentityProbeBinary"] = expected["fontIdentityProbeBinary"]
    return result


def _require_catalog_attempt_set(attempts: list[JsonObject], first_expected: JsonObject) -> None:
    require(len(attempts) == 84, "formal HTML performance requires all 84 catalog conditions")
    require("catalogCondition" in first_expected,
            "formal HTML performance requires catalog-bound current-source evidence")


def _require_html84_acceptance(validation: JsonObject, *, diagnostic: bool = False) -> None:
    if diagnostic:
        require(validation.get("acceptance") is False
                and validation.get("fontIdentityEquivalence") == "not-established",
                "HTML84 diagnostic evidence must remain unaccepted with font identity unresolved")
        return
    require(validation.get("acceptance") is True,
            "HTML84 visual acceptance is not established")
    require(validation.get("fontIdentityEquivalence") == "established",
            "HTML84 same-font equivalence is not established")


def _require_formal_release_result(result: JsonObject) -> None:
    stage90 = _object(result.get("htmlStage90"), "HTML84 stage90 result")
    _require_html84_acceptance(stage90)
    require(result.get("performanceNonregression") is True,
            "HTML performance non-regression is not established")


def _verify(directory: Path, root: Path, *, git_query: GitQuery | None = None,
            require_catalog: bool, diagnostic_catalog: bool = False) -> JsonObject:
    bundle = Bundle(directory)
    head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(bundle.manifest["head"] == head, "bundle is not current HEAD")
    attempts = _objects(bundle.manifest["attempts"], "viewport attempts")
    first_expected = bundle.json(attempts[0]["expected"]) if attempts else {}
    if require_catalog:
        _require_catalog_attempt_set(attempts, first_expected)
    if "catalogCondition" in first_expected:
        require(len(attempts) == 84, "catalog performance requires all 84 conditions")
        values = [validate_attempt(bundle, root, attempt, git_query=git_query) for attempt in attempts]
        manifest_refs = {json.dumps(value["catalogManifestRef"], sort_keys=True) for value in values}
        plan_refs = {json.dumps(value["catalogPlanRef"], sort_keys=True) for value in values}
        validation_refs = {json.dumps(value["catalogCaptureValidationRef"], sort_keys=True) for value in values}
        font_request_refs = {json.dumps(value["fontIdentityRequestRef"], sort_keys=True) for value in values}
        font_response_refs = {json.dumps(value["fontIdentityResponseRef"], sort_keys=True) for value in values}
        font_probe_binaries = {json.dumps(value["fontIdentityProbeBinary"], sort_keys=True) for value in values}
        require(len(manifest_refs) == len(plan_refs) == len(validation_refs) ==
                len(font_request_refs) == len(font_response_refs) == len(font_probe_binaries) == 1,
                "catalog attempts do not share one manifest, plan and Chrome validation")
        catalog_manifest_ref = _object(values[0]["catalogManifestRef"], "catalog manifest reference")
        catalog_plan_ref = _object(values[0]["catalogPlanRef"], "catalog plan reference")
        validation_ref = _object(values[0]["catalogCaptureValidationRef"], "Chrome validation reference")
        manifest = bundle.json(catalog_manifest_ref)
        catalog_plan = bundle.json(catalog_plan_ref)
        validation = bundle.json(validation_ref)
        request_ref = _object(values[0]["fontIdentityRequestRef"], "font identity request reference")
        response_ref = _object(values[0]["fontIdentityResponseRef"], "font identity response reference")
        probe_binary = _object(values[0]["fontIdentityProbeBinary"], "font identity probe binary")
        manifest_probe = _object(manifest.get("fontIdentityProbe"), "manifest font identity probe")
        validation_probe = _object(validation.get("fontIdentityProbe"), "validated font identity probe")
        for key, reference in (("request", request_ref), ("response", response_ref)):
            declared = _object(manifest_probe.get(key), "manifest font probe file pin")
            validated = _object(validation_probe.get(key), "validated font probe file pin")
            transported = bundle.file(reference)
            require(declared.get("path") == reference.get("path")
                    and declared.get("sha256") == reference.get("sha256")
                    and declared.get("sizeBytes") == reference.get("size")
                    and validated == reference
                    and transported.stat().st_size == _integer(reference.get("size"), "font probe file size"),
                    f"font identity {key} transport pin differs")
        require(_object(manifest_probe.get("binary"), "manifest font probe binary") == probe_binary
                and _object(validation_probe.get("binary"), "validated font probe binary") == probe_binary,
                "font identity probe binary pin differs")
        require(validation_probe.get("inventoryMethod") == manifest_probe.get("inventoryMethod")
                and validation_probe.get("inventoryLimit") == manifest_probe.get("inventoryLimit")
                and bool(_text(validation_probe.get("inventoryMethod"), "font inventory method"))
                and bool(_text(validation_probe.get("inventoryLimit"), "font inventory limit")),
                "font identity inventory method/limit differs")
        require(validation.get("schema") == "krr-html84-independent-stage90-validation-v1"
                and validation.get("threshold") == 90
                and validation.get("measurement") == "not-performed"
                and validation.get("manifestPath") == catalog_manifest_ref["path"]
                and validation.get("manifestSha256") == catalog_manifest_ref["sha256"]
                and validation.get("planPath") == catalog_plan_ref["path"]
                and validation.get("planSha256") == catalog_plan_ref["sha256"],
                "transported strict Chrome validation does not bind this catalog manifest/plan")
        _require_html84_acceptance(validation, diagnostic=diagnostic_catalog)
        catalog_path = Path(_text(manifest.get("catalogPath"), "feature catalog path"))
        catalog_sha = _text(manifest.get("catalogSha256"), "feature catalog SHA")
        digest(catalog_sha)
        catalog_relative = catalog_path.relative_to(root).as_posix()
        require(manifest.get("profile") == "catalog-stage90-captures"
                and manifest.get("acceptance") is False
                and catalog_plan.get("catalogPath") == str(catalog_path)
                and catalog_plan.get("catalogSha256") == catalog_sha
                and catalog_plan.get("profile") == "krr-html84-condition-plan-v1"
                and catalog_plan.get("acceptance") is False
                and manifest.get("planPath") == catalog_plan_ref["path"]
                and manifest.get("planSha256") == catalog_plan_ref["sha256"],
                "catalog manifest/plan binding differs")
        source = cast(SourceContract, values[0]["source"])
        require(source["source"].get(catalog_relative, (None,))[0] == catalog_sha,
                "catalog feature source is not current committed source")
        expected_catalog = _objects(catalog_plan.get("conditions"), "planned catalog condition")
        captured_catalog = _objects(manifest.get("conditions"), "captured catalog condition")
        validated_chrome = _objects(validation.get("conditions"), "validated Chrome conditions")
        require(len(expected_catalog) == len(captured_catalog) == len(validated_chrome) == 84,
                "catalog manifest/plan/Chrome validation coverage differs")
        actual_conditions = [_object(value["catalogCondition"], "measured catalog condition") for value in values]
        evidence_by_id = {
            _text(_object(value["catalogCondition"], "catalog condition")["condition_id"], "condition id"):
                _object(value["operationEvidenceRef"], "evidence reference")
            for value in values
        }
        require([_text(item.get("condition_id"), "measured condition id") for item in actual_conditions]
                == [_text(item.get("conditionId"), "planned condition id") for item in expected_catalog],
                "measured condition order or coverage differs")
        captured_by_id = {_text(item.get("conditionId"), "captured condition id"): item for item in captured_catalog}
        chrome_by_id = {_text(item.get("conditionId"), "validated Chrome condition id"): item
                        for item in validated_chrome}
        require(len(captured_by_id) == len(captured_catalog) == len(chrome_by_id) == len(validated_chrome),
                "catalog capture or Chrome validation ids are duplicated")
        for expected in expected_catalog:
            condition_id = _text(expected.get("conditionId"), "planned condition id")
            captured_value = captured_by_id.get(condition_id)
            if captured_value is None:
                raise ValueError(f"captured catalog condition is missing: {condition_id}")
            captured = captured_value
            match = next(item for item in actual_conditions if item["condition_id"] == condition_id)
            for plan_key, output_key in (("caseId", "case_id"), ("state", "state"),
                                         ("operationKind", "operation_kind"),
                                         ("targetSelector", "target_selector"),
                                         ("sourceSha256", "fixture_sha256")):
                require(expected.get(plan_key) == match.get(output_key),
                        f"measured catalog {output_key} differs: {condition_id}")
            require(expected.get("viewport") == match.get("viewport")
                    and captured.get("caseId") == expected.get("caseId")
                    and captured.get("state") == expected.get("state")
                    and captured.get("operationKind") == expected.get("operationKind")
                    and captured.get("sourceSha256") == expected.get("sourceSha256")
                    and captured.get("viewport") == expected.get("viewport"),
                    f"measured/captured catalog tuple differs: {condition_id}")
            evidence_ref = evidence_by_id[condition_id]
            require(captured.get("krrReportPath") == evidence_ref["path"]
                    and captured.get("krrReportSha256") == evidence_ref["sha256"],
                    f"measured catalog evidence ref differs: {condition_id}")
            chrome_row = chrome_by_id.get(condition_id)
            if chrome_row is None:
                raise ValueError(f"strict Chrome validation is missing: {condition_id}")
            require(captured.get("status") == "captured"
                    and _same_json(captured.get("chromeState"), captured.get("krrState")),
                    f"catalog Chrome/KRR state is not captured and identical: {condition_id}")
            match_value = next(value for value in values
                               if _object(value["catalogCondition"], "catalog condition")["condition_id"] == condition_id)
            chrome_capture_ref = _object(match_value["chromeCaptureRef"], "Chrome capture reference")
            chrome_receipt_ref = _object(match_value["chromeReceiptRef"], "Chrome receipt reference")
            for row, kind, ref in ((chrome_row, "capture", chrome_capture_ref),
                                   (chrome_row, "receipt", chrome_receipt_ref)):
                path_key, sha_key, size_key = kind + "Path", kind + "Sha256", kind + "Size"
                transported_path = bundle.file(ref)
                require(row.get(path_key) == ref["path"] and row.get(sha_key) == ref["sha256"]
                        and row.get(size_key) == ref.get("size")
                        and _integer(row.get(size_key), kind + " size") == transported_path.stat().st_size,
                        f"strict Chrome {kind} transport binding differs: {condition_id}")
            require(_same_json(chrome_row.get("state"), captured.get("chromeState")),
                    f"strict Chrome state differs from catalog condition: {condition_id}")
            krr_capture_ref = {"path": captured["krrCapturePath"], "sha256": captured["krrCaptureSha256"]}
            krr_capture_file = bundle.file(krr_capture_ref)
            require(chrome_row.get("krrCapturePath") == krr_capture_ref["path"]
                    and chrome_row.get("krrCaptureSha256") == krr_capture_ref["sha256"]
                    and _integer(chrome_row.get("krrCaptureSize"), "KRR capture size") == krr_capture_file.stat().st_size,
                    f"independent stage90 KRR capture binding differs: {condition_id}")
            score = _numeric(chrome_row.get("score"), "independent stage90 score")
            require(math.isfinite(score) and score >= 90,
                    f"independent stage90 score is below 90: {condition_id}")
        minimum_score = min(_numeric(row.get("score"), "independent stage90 score")
                            for row in validated_chrome)
        return {"head": head, "checks": 1680, "phaseRecords": 84 * len(PHASES) * 50, "conditions": 84,
                "htmlStage90": {"threshold": 90, "minimumScore": minimum_score,
                                "acceptance": validation["acceptance"],
                                "fontIdentityEquivalence": validation["fontIdentityEquivalence"]},
                "performanceNonregression": True}
    require(len(attempts) == 3, "all three viewports required")
    values = [validate_attempt(bundle, root, attempt, git_query=git_query) for attempt in attempts]
    require([v["viewport"] for v in values] == [{"width": w, "height": h, "scale": 1} for w, h in VIEWPORTS], "viewport coverage/order differs")
    shared = [{k: v[k] for k in v if k != "viewport"} for v in values]
    require(shared[0] == shared[1] == shared[2], "three viewports do not share exact source/fixture/state/build/context")
    return {"head": head, "checks": 60, "phaseRecords": len(VIEWPORTS) * len(PHASES) * 50, "performanceNonregression": True}


def verify(directory: Path, root: Path, *, git_query: GitQuery | None = None) -> JsonObject:
    result = _verify(directory, root, git_query=git_query, require_catalog=True)
    _require_formal_release_result(result)
    return result


def _verify_catalog_diagnostic(directory: Path, root: Path, *,
                               git_query: GitQuery | None = None) -> JsonObject:
    result = _verify(directory, root, git_query=git_query, require_catalog=True,
                     diagnostic_catalog=True)
    _require_html84_acceptance(
        _object(result.get("htmlStage90"), "HTML84 diagnostic result"), diagnostic=True,
    )
    return result


def _verify_legacy_diagnostic(directory: Path, root: Path, *,
                              git_query: GitQuery | None = None) -> JsonObject:
    return _verify(directory, root, git_query=git_query, require_catalog=False)


def latest_acquisition_generation(runs: list[JsonObject]) -> JsonObject:
    if not runs:
        raise PendingEvidence("current-source CI run has not started")
    for run in runs:
        require(_integer(run.get("id"), "CI run id") > 0 and _integer(run.get("run_attempt"), "CI attempt") > 0, "invalid CI generation")
        started = _text(run.get("run_started_at"), "CI generation start time")
        require(re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", started) is not None, "invalid CI generation start time")
        try:
            datetime.strptime(started, "%Y-%m-%dT%H:%M:%SZ")
        except ValueError as error:
            raise ValueError("invalid CI generation start time") from error
    latest_start = max(_text(run["run_started_at"], "CI generation start time") for run in runs)
    latest = [run for run in runs if run["run_started_at"] == latest_start]
    require(len(latest) == 1, "ambiguous latest CI generation")
    return latest[0]


def acquire(directory: Path, root: Path, repository: str, current_producer: bool = False, *,
            git_query: GitQuery | None = None) -> None:
    # 同一SHAの成功CI由来だけを取得し、証跡の欠落を環境変数で免除しない。
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None, "invalid repository")
    head = git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    require(not directory.exists(), "acquisition destination already exists")
    def api(path: str) -> JsonObject:
        return _object(json.loads(subprocess.check_output(["gh", "api", path]), object_pairs_hook=unique_object), "GitHub response")
    pull: JsonObject
    pull_head: JsonObject
    pull_base: JsonObject
    if current_producer:
        # 自分のCIのpreflightだけが、既に成功した依存jobを全workflow完了前に読む。
        # 公開経路の取得は従来どおりcompleted/successのworkflowだけを受理する。
        require(os.environ.get("GITHUB_JOB") == "preflight" and os.environ.get("GITHUB_EVENT_NAME") == "pull_request", "current producer acquisition is only for PR preflight")
        event = read_json(Path(os.environ["GITHUB_EVENT_PATH"]))
        pull = _object(event["pull_request"], "pull request")
        pull_head = _object(pull["head"], "PR head")
        pull_base = _object(pull["base"], "PR base")
        require(pull["draft"] is False and pull_head["sha"] == head and _object(pull_head["repo"], "PR repository")["full_name"] == repository and pull_base["ref"] == "master" and re.fullmatch(r"release/v[0-9]+\.[0-9]+\.[0-9]+", _text(pull_head["ref"], "PR branch")) is not None, "current producer PR identity mismatch")
        run_id, attempt = int(os.environ["GITHUB_RUN_ID"]), int(os.environ["GITHUB_RUN_ATTEMPT"])
        require(run_id > 0 and attempt > 0, "invalid current CI generation")
        run = api(f"repos/{repository}/actions/runs/{run_id}")
        require(run["id"] == run_id and run["run_attempt"] == attempt and run["event"] == "pull_request" and run["path"] == ".github/workflows/test-and-build.yml" and _object(run["repository"], "CI repository")["full_name"] == repository and _object(run["head_repository"], "CI head repository")["full_name"] == repository, "current CI generation identity mismatch")
        require(any(binding["number"] == pull["number"] and _object(binding["head"], "CI PR head")["sha"] == head and _object(binding["base"], "CI PR base")["sha"] == pull_base["sha"] for binding in _objects(run["pull_requests"], "CI PR bindings")), "current CI PR head/base binding mismatch")
        jobs = api(f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100")
        job_rows = _objects(jobs["jobs"], "CI jobs")
        require(jobs["total_count"] == len(job_rows), "incomplete current CI jobs")
        for name in ("Test and Build (ubuntu-latest, linux64)", "Test and Build (macos-15, mac-arm64)", "Test and Build (windows-latest, win64)", "Native paired HTML time and RSS nonregression"):
            matches = [job for job in job_rows if job["name"] == name]
            require(len(matches) == 1 and matches[0]["run_id"] == run_id and matches[0]["status"] == "completed" and matches[0]["conclusion"] == "success", f"current producer dependency did not succeed: {name}")
    else:
        snapshot = api(f"repos/{repository}/actions/workflows/test-and-build.yml/runs?head_sha={head}&per_page=100")
        runs = _objects(snapshot["workflow_runs"], "workflow runs")
        require(type(snapshot["total_count"]) is int and snapshot["total_count"] == len(runs), "incomplete current-source workflow run page")
        runs = [r for r in runs if r["head_sha"] == head and _object(r["head_repository"], "CI head repository")["full_name"] == repository and r["path"] == ".github/workflows/test-and-build.yml"]
        run = latest_acquisition_generation(runs)
        if run["status"] != "completed":
            raise PendingEvidence("current-source CI/measurement is still running")
        require(run["conclusion"] == "success" and _integer(run["run_attempt"], "CI attempt") > 0, "latest exact-source CI attempt failed")
    if current_producer:
        runs_path = f"repos/{repository}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch={pull_head['ref']}&per_page=100"
    else:
        runs_path = f"repos/{repository}/actions/workflows/test-and-build.yml/runs?head_sha={head}&per_page=100"
    def current_latest() -> JsonObject:
        snapshot = api(runs_path)
        rows = _objects(snapshot["workflow_runs"], "workflow runs")
        require(type(snapshot["total_count"]) is int and snapshot["total_count"] == len(rows), "incomplete current-source workflow run page")
        rows = [row for row in rows if _object(row["head_repository"], "CI head repository")["full_name"] == repository and row["path"] == ".github/workflows/test-and-build.yml"]
        if current_producer:
            rows = [row for row in rows if row.get("event") == "pull_request" and _object(row.get("repository", {}), "CI repository").get("full_name") == repository and any(binding.get("number") == pull["number"] and _object(binding.get("head", {}), "CI PR head").get("sha") == head and _object(binding.get("base", {}), "CI PR base").get("sha") == pull_base["sha"] for binding in _objects(row.get("pull_requests", []), "CI PR bindings"))]
        else:
            rows = [row for row in rows if row["head_sha"] == head]
        return latest_acquisition_generation(rows)
    if current_producer:
        require(current_latest() == run, "current producer is not latest exact-PR CI generation")
    artifact_snapshot = api(f"repos/{repository}/actions/runs/{run['id']}/artifacts?per_page=100")
    artifacts = _objects(artifact_snapshot["artifacts"], "CI artifacts")
    require(artifact_snapshot["total_count"] == len(artifacts), "incomplete artifact page")
    matches = [a for a in artifacts if a["name"] == f"html-performance-paired-{head}-{run['id']}-{run['run_attempt']}" and a["expired"] is False]
    require(len(matches) == 1, "one immutable current-source performance artifact required")
    directory.mkdir(parents=True)
    archive = directory / "artifact.zip"
    archive.write_bytes(subprocess.check_output(["gh", "api", f"repos/{repository}/actions/artifacts/{matches[0]['id']}/zip"]))
    artifact_digest = matches[0].get("digest", "")
    require(artifact_digest == "sha256:" + sha256(archive), "GitHub artifact digest absent/mismatch")
    with zipfile.ZipFile(archive) as zipped:
        seen = set()
        for entry in zipped.infolist():
            path = (directory / entry.filename).resolve()
            require(path.is_relative_to(directory.resolve()) and path not in seen and (entry.external_attr >> 16) & 0o170000 != 0o120000, "unsafe/duplicate archive path")
            seen.add(path)
        zipped.extractall(directory)
    latest = api(f"repos/{repository}/actions/runs/{run['id']}")
    require(all(latest[k] == run[k] for k in ("id", "head_sha", "run_attempt", "status", "conclusion")), "CI run changed during acquisition")
    # ZIP取得中の新規runも照合し、古い成功世代だけのGETで取得を完了しない。
    require(current_latest() == run, "latest CI generation changed during acquisition")
    (directory / "acquisition.json").write_text(json.dumps({"repository": repository, "head": head, "runId": run["id"], "runAttempt": run["run_attempt"], "artifactId": matches[0]["id"], "artifactDigest": artifact_digest}, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--evidence-dir", type=Path, default=Path("tmp/html-performance-release-evidence"))
    parser.add_argument("--acquire", action="store_true")
    parser.add_argument("--acquire-current-producer", action="store_true")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    args = parser.parse_args()
    try:
        require(not (args.acquire and args.acquire_current_producer), "ambiguous acquisition mode")
        if args.acquire or args.acquire_current_producer:
            acquire(args.evidence_dir, args.repo, args.repository, args.acquire_current_producer)
        print(json.dumps(verify(args.evidence_dir, args.repo), sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"HTML performance release gate failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
