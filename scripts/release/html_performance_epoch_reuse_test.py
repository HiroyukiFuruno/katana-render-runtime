from __future__ import annotations

import hashlib
import json
import os
import platform
import subprocess
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_epoch_reuse as epoch
import native_coverage_local_context as native_context
import verify_html_performance_evidence as gate


class DeclaredInputsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name).resolve()
        self.pins: dict[str, gate.JsonObject] = {}
        files: list[gate.JsonObject] = []
        for name in ("baseline", "candidate", "program", "fixture", "resource", "font"):
            path = root / name
            path.write_bytes(name.encode())
            checksum = hashlib.sha256(name.encode()).hexdigest()
            self.pins[name] = {"path": str(path), "sha256": checksum}
            files.append({"originalPath": str(path), "relativePath": name, "sha256": checksum})
        lock = root / "lock"
        lock.write_bytes(b"lock")
        lock_sha = hashlib.sha256(b"lock").hexdigest()
        logical_lock = root / gate.HARNESS_ROOT / "Cargo.lock"
        self.pins["lock"] = {"path": str(logical_lock), "sha256": lock_sha}
        files.append({"originalPath": str(logical_lock), "relativePath": "lock", "sha256": lock_sha})
        snapshot = root / "snapshot"
        snapshot.write_text(json.dumps({"records": [{"path": "program",
            "sha256": self.pins["program"]["sha256"], "size": 7}]}))
        snapshot_sha = gate.sha256(snapshot)
        self.pins["snapshot"] = {"path": str(snapshot), "sha256": snapshot_sha}
        files.append({"originalPath": str(snapshot), "relativePath": "snapshot", "sha256": snapshot_sha})
        for name in ("binding", "state", "registry", "capacity", "baseline-proof", "candidate-proof", "source-before"):
            path = root / name
            self.pins[name] = {"path": str(path), "sha256": ""}
        def save_pin(name: str, value: object) -> None:
            path = root / name
            path.write_text(json.dumps(value, sort_keys=True))
            self.pins[name]["sha256"] = gate.sha256(path)
            files.append({"originalPath": str(path), "relativePath": name,
                          "sha256": self.pins[name]["sha256"]})
        save_pin("source-before", {"head": "a" * 40})
        save_pin("state", {"baseline": 1})
        save_pin("registry", {"refs": []})
        save_pin("binding", {"schema": "krr-html-performance-capacity-binding-v1",
                             "state": self.pins["state"], "protectedRegistry": self.pins["registry"]})
        save_pin("capacity", {"sourceBeforeRef": self.pins["source-before"],
                              "bindingRef": self.pins["binding"], "stateRef": self.pins["state"],
                              "protectedRegistryRef": self.pins["registry"]})
        save_pin("baseline-proof", {"schema": "krr-html-performance-build-artifact-v3", "arm": "baseline"})
        save_pin("candidate-proof", {"schema": "krr-html-performance-build-artifact-v3",
                                    "arm": "candidate", "capacityInputRef": self.pins["capacity"],
                                    "frozenBinary": self.pins["candidate"],
                                    "sourceSnapshotRef": self.pins["snapshot"],
                                    "sourceBeforeRef": self.pins["source-before"]})
        (root / "bundle.json").write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1", "files": files,
        }))
        self.bundle = gate.Bundle(root)

    def declared(self) -> epoch.DeclaredEpochInputs:
        return {
            "workers": {"baseline": self.pins["baseline"], "candidate": self.pins["candidate"]},
            "programs": {"producer": self.pins["program"]},
            "inputs": {"fixture": self.pins["fixture"]},
            "resources": {"image": self.pins["resource"]},
            "fonts": {"font": self.pins["font"]},
            "tool_context": {"environmentSha256": "a" * 64, "platform": "example", "cpuCount": 4},
        }

    def test_measurement_capacity_is_distinct_and_strictly_bound(self) -> None:
        root = self.bundle.directory

        def pin(name: str, value: object) -> gate.JsonObject:
            file = root / name
            file.write_text(json.dumps(value, sort_keys=True))
            result: gate.JsonObject = {"path": str(file), "sha256": gate.sha256(file)}
            self.bundle.files[str(file)] = (file, str(result["sha256"]))
            return result

        state_value = {"target": str(root / "target"), "baseline_bytes": 2363047936,
                       "factor": 2, "metric": "unique-inode-allocated-bytes", "metric_version": 1}
        reference_path = root / "program"
        info = reference_path.stat()
        reference = {"path": str(reference_path), "sha256": gate.sha256(reference_path),
                     "size": info.st_size, "dev": info.st_dev, "ino": info.st_ino, "mode": info.st_mode}
        registry_value = {"schema": "krr-protected-refs-successor-v1", "physicalRefCount": 1,
                          "refs": [reference]}
        state = pin("measurement-state", state_value)
        registry = pin("measurement-registry", registry_value)
        binding = pin("measurement-binding", {
            "schema": "krr-html-performance-capacity-binding-v1",
            "state": state, "protectedRegistry": registry,
        })
        context: gate.JsonObject = {"inputs": [{**item, "exists": True}
                                               for item in (binding, state, registry)]}
        proof = self.bundle.json(self.pins["candidate-proof"])
        result = epoch._capacity_pins(self.bundle, root, proof, context, binding)
        self.assertEqual(result["bindingRef"], self.pins["binding"])
        self.assertEqual(result["measurementBindingRef"], binding)
        self.assertEqual(result["measurementStateRef"], state)
        self.assertEqual(result["measurementProtectedRegistryRef"], registry)
        for index in range(3):
            rows = list(gate._objects(context["inputs"], "inputs"))
            rows[index] = {**rows[index], "sha256": "0" * 64}
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, "capacity context differs"):
                epoch._capacity_pins(self.bundle, root, proof, {"inputs": rows}, binding)
        with self.assertRaisesRegex(ValueError, "capacity context differs"):
            epoch._capacity_pins(self.bundle, root, proof, {"inputs": []}, binding)
        bad = pin("unknown-measurement-binding", {
            "schema": "krr-html-performance-capacity-binding-v1", "state": state,
            "protectedRegistry": registry, "unknown": True,
        })
        with self.assertRaisesRegex(ValueError, "measurement capacity binding differs"):
            epoch._capacity_pins(self.bundle, root, proof, context, bad)
        for name, invalid_state, invalid_registry in (
            ("baseline", {**state_value, "baseline_bytes": 1}, registry_value),
            ("target", {**state_value, "target": str(root / "other")}, registry_value),
            ("metric", {**state_value, "metric": "other"}, registry_value),
            ("factor", {**state_value, "factor": 3}, registry_value),
            ("registry-schema", state_value, {**registry_value, "schema": "other"}),
            ("registry-count", state_value, {**registry_value, "physicalRefCount": 2}),
            ("registry-identity", state_value, {**registry_value, "refs": [{**reference, "ino": info.st_ino + 1}]}),
        ):
            bad_state = pin(f"{name}-state", invalid_state)
            bad_registry = pin(f"{name}-registry", invalid_registry)
            bad_binding = pin(f"{name}-binding", {
                "schema": "krr-html-performance-capacity-binding-v1",
                "state": bad_state, "protectedRegistry": bad_registry,
            })
            bad_context: gate.JsonObject = {"inputs": [{**item, "exists": True}
                                                       for item in (bad_binding, bad_state, bad_registry)]}
            with self.subTest(name=name), self.assertRaises((ValueError, RuntimeError)):
                epoch._capacity_pins(self.bundle, root, proof, bad_context, bad_binding)
        (root / "measurement-binding").write_text("changed")
        with self.assertRaises(ValueError):
            epoch._capacity_pins(self.bundle, root, proof, context, binding)

    def test_system_only_observation_uses_actual_pinned_worker(self) -> None:
        path = self.bundle.file(self.pins["candidate"])
        path.write_bytes(struct.pack("<IIIIIIII", 0xFEEDFACF, 0x0100000C, 0, 2, 0, 0, 0, 0))
        pin: gate.JsonObject = {"path": str(path), "sha256": gate.sha256(path)}
        self.bundle.files[str(path)] = (path, str(pin["sha256"]))
        tool: gate.JsonObject = {"platform": "test-host",
                                 "environmentSha256": epoch._inventory_digest(dict(os.environ))}
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "platform", return_value="test-host"), \
             patch.object(native_context, "_run", side_effect=[f"{path}:", "", ""] * 2):
            result = epoch.observe_system_only_worker(pin, tool, os.environ)
            saved_path = path.parent / "worker-observation.json"
            saved_path.write_text(json.dumps(result))
            restored = json.loads(saved_path.read_text())
            repeated = epoch.observe_system_only_worker(pin, tool, os.environ)
        self.assertEqual(result["worker"], pin)
        self.assertIsInstance(result["stamp"], list)
        self.assertEqual(result["stamp"][3], path.stat().st_size)
        self.assertEqual(restored, repeated)

    def test_system_only_observation_rejects_rpath_and_loader_override(self) -> None:
        path = self.bundle.file(self.pins["candidate"])
        command = struct.pack("<III", 0x8000001C, 16, 12) + b"/x\0\0"
        path.write_bytes(struct.pack("<IIIIIIII", 0xFEEDFACF, 0x0100000C, 0, 2, 1, len(command), 0, 0) + command)
        pin: gate.JsonObject = {"path": str(path), "sha256": gate.sha256(path)}
        self.bundle.files[str(path)] = (path, str(pin["sha256"]))
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "platform", return_value="test-host"):
            tool: gate.JsonObject = {"platform": "test-host",
                                     "environmentSha256": epoch._inventory_digest(dict(os.environ))}
            with self.assertRaisesRegex(ValueError, "has rpaths"):
                epoch.observe_system_only_worker(pin, tool, os.environ)
            with patch.dict(os.environ, {"DYLD_INSERT_LIBRARIES": "/tmp/unknown"}):
                tool["environmentSha256"] = epoch._inventory_digest(dict(os.environ))
                with self.assertRaisesRegex(ValueError, "loader override"):
                    epoch.observe_system_only_worker(pin, tool, os.environ)

    def test_system_only_observation_rejects_changed_or_external_worker(self) -> None:
        path = self.bundle.file(self.pins["candidate"])
        def header(count: int, length: int) -> bytes:
            return struct.pack(
                "<IIIIIIII", 0xFEEDFACF, 0x0100000C, 0, 2, count, length, 0, 0
            )
        path.write_bytes(header(0, 0))
        pin: gate.JsonObject = {"path": str(path), "sha256": gate.sha256(path)}
        tool: gate.JsonObject = {"platform": "test-host",
                                 "environmentSha256": epoch._inventory_digest(dict(os.environ))}
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "platform", return_value="test-host"):
            path.write_bytes(header(0, 0) + b"changed")
            with self.assertRaisesRegex(ValueError, "worker bytes differ"):
                epoch.observe_system_only_worker(pin, tool, os.environ)
            name = b"/tmp/external.dylib\0"
            size = (24 + len(name) + 7) // 8 * 8
            command = struct.pack("<III", 0xC, size, 24) + b"\0" * 12 + name
            path.write_bytes(header(1, size) + command.ljust(size, b"\0"))
            pin["sha256"] = gate.sha256(path)
            with self.assertRaisesRegex(ValueError, "non-system dependency"):
                epoch.observe_system_only_worker(pin, tool, os.environ)
            path.write_bytes(header(0, 0))
            pin["sha256"] = gate.sha256(path)
            with patch.object(native_context, "_run", side_effect=native_context.LocalCoverageContextError("macho_dependency_unresolved")):
                with self.assertRaises(native_context.LocalCoverageContextError):
                    epoch.observe_system_only_worker(pin, tool, os.environ)

    def test_identical_transport_pins_and_context_are_accepted(self) -> None:
        epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, self.declared())

    def test_reviewed_added_programs_require_exact_transport_membership(self) -> None:
        from html_performance_source_reuse import VALIDATION_COHORT_ADDITIONS

        repo = Path(__file__).resolve().parents[2]
        root = self.bundle.directory
        document = gate.read_json(root / "bundle.json")
        files = gate._array(document["files"], "files")
        additions: dict[str, gate.JsonObject] = {}
        for index, (name, (checksum, size)) in enumerate(VALIDATION_COHORT_ADDITIONS.items()):
            data = subprocess.run(["git", "-C", str(repo), "show", "637cd465:" + name],
                                  check=True, capture_output=True).stdout
            self.assertEqual((hashlib.sha256(data).hexdigest(), len(data)), (checksum, size))
            relative = f"added-{index}"
            (root / relative).write_bytes(data)
            logical = str(root / "tmp/html-performance-programs" / checksum / name)
            files.append({"originalPath": logical, "relativePath": relative, "sha256": checksum})
            additions[name] = {"path": logical, "sha256": checksum}
        (root / "bundle.json").write_text(json.dumps(document))
        reader = gate.Bundle(root)
        old = self.declared()
        new = self.declared()
        new["programs"] = {**new["programs"], **additions}
        epoch.compare_declared_inputs(reader, old, reader, new)
        new["programs"] = {key: value for key, value in new["programs"].items()
                           if key != next(iter(additions))}
        with self.assertRaisesRegex(ValueError, "epoch programs differs"):
            epoch.compare_declared_inputs(reader, old, reader, new)
        new["programs"] = {**new["programs"], "scripts/unknown.py": self.pins["program"]}
        with self.assertRaisesRegex(ValueError, "epoch programs differs"):
            epoch.compare_declared_inputs(reader, old, reader, new)

    def test_changed_role_pin_or_inventory_is_rejected(self) -> None:
        for group, role in (("workers", "candidate"), ("programs", "producer"),
                            ("inputs", "fixture"), ("resources", "image"), ("fonts", "font")):
            with self.subTest(group=group):
                changed = self.declared()
                if group == "workers":
                    changed["workers"] = {**changed["workers"], role: self.pins["baseline"]}
                elif group == "programs":
                    changed["programs"] = {role: self.pins["baseline"]}
                elif group == "inputs":
                    changed["inputs"] = {role: self.pins["baseline"]}
                elif group == "resources":
                    changed["resources"] = {role: self.pins["baseline"]}
                else:
                    changed["fonts"] = {role: self.pins["baseline"]}
                with self.assertRaises(ValueError):
                    epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, changed)

    def test_changed_context_or_missing_worker_is_rejected(self) -> None:
        changed = self.declared()
        changed["tool_context"] = {**changed["tool_context"], "cpuCount": 8}
        with self.assertRaises(ValueError):
            epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, changed)
        changed = self.declared()
        changed["workers"] = {"candidate": self.pins["candidate"]}
        with self.assertRaises(ValueError):
            epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, changed)

    def test_mutated_transported_bytes_are_rejected(self) -> None:
        path = self.bundle.file(self.pins["fixture"])
        path.write_bytes(b"changed")
        with self.assertRaises(ValueError):
            epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, self.declared())


    def test_projection_authenticates_refs_and_binds_unknown_context(self) -> None:
        context: gate.JsonObject = {
            "inputs": [{**pin, "exists": True} for pin in self.pins.values()]
                      + [{"path": "/unknown", "sha256": "f" * 64, "exists": True}],
            "toolContext": {**self.declared()["tool_context"], "rustc": "rustc", "cargo": "cargo"},
        }
        expected: gate.JsonObject = {
            "operationPlanRef": self.pins["fixture"], "domStateRef": self.pins["fixture"],
            "fixture_sha256": self.pins["fixture"]["sha256"],
            "catalogCondition": {"fixture_path": self.pins["fixture"]["path"]},
            "sourcePair": {"before": self.pins["source-before"]},
            "arms": {
                "baseline-0.4.23": {"binary_sha256": self.pins["baseline"]["sha256"],
                                    "build_artifact_ref": self.pins["baseline-proof"]},
                "candidate": {"binary_sha256": self.pins["candidate"]["sha256"],
                              "build_artifact_ref": self.pins["candidate-proof"]},
            },
        }
        for key in ("catalogManifestRef", "catalogPlanRef", "catalogCaptureValidationRef",
                    "catalogBinaryProofRef", "catalogBuildReportRef", "catalogCaptureBinaryRef",
                    "catalogCurrentBuildReportRef", "chromeCaptureRef", "chromeReceiptRef",
                    "fontIdentityRequestRef", "fontIdentityResponseRef", "fontIdentityProbeBinary",
                    "operationEvidenceRef"):
            expected[key] = self.pins["resource"]
        with patch.object(gate, "LOCK_SHA", self.pins["lock"]["sha256"]):
            result = epoch.declared_from_authenticated_refs(
                self.bundle, self.bundle.directory, context, expected, self.pins["snapshot"],
                self.pins["resource"], [], {"image": self.pins["resource"]},
                {"font": self.pins["font"]}, self.declared()["workers"],
                {"producer": self.pins["program"]},
            )
            self.assertIn(str(self.pins["fixture"]["path"]), result["declared"]["inputs"])
            self.assertNotIn(str(self.pins["snapshot"]["path"]), result["declared"]["inputs"])
            self.assertEqual(result["operational_pins"]["sourceSnapshot"], self.pins["snapshot"])
            self.assertEqual(result["operational_pins"]["stateRef"], self.pins["state"])
            self.assertNotEqual(self.pins["candidate"], self.pins["candidate-proof"])
            self.assertNotEqual(self.pins["snapshot"], self.pins["program"])
            self.assertEqual(result["declared"]["programs"]["producer"], self.pins["program"])
            self.assertNotIn(str(self.pins["state"]["path"]), result["declared"]["inputs"])
            bad_context = {**context, "inputs": [
                ({**pin, "sha256": "0" * 64} if pin["path"] == self.pins["state"]["path"] else pin)
                for pin in [{**value, "exists": True} for value in self.pins.values()]
            ]}
            missing_tool = {**context, "toolContext": self.declared()["tool_context"]}
            with self.assertRaisesRegex(ValueError, "epoch tool context is incomplete"):
                epoch.declared_from_authenticated_refs(
                    self.bundle, self.bundle.directory, missing_tool, expected, self.pins["snapshot"],
                    self.pins["resource"], [], {"image": self.pins["resource"]},
                    {"font": self.pins["font"]}, self.declared()["workers"],
                    {"producer": self.pins["program"]},
                )
            with self.assertRaisesRegex(ValueError, "capacity context differs"):
                epoch.declared_from_authenticated_refs(
                    self.bundle, self.bundle.directory, bad_context, expected, self.pins["snapshot"],
                    self.pins["resource"], [], {"image": self.pins["resource"]},
                    {"font": self.pins["font"]}, self.declared()["workers"],
                    {"producer": self.pins["program"]},
                )
            epoch.compare_epoch_projections(self.bundle, result, self.bundle, result)
            forged: epoch.EpochProjection = {**result, "operational_digest": "0" * 64}
            with self.assertRaisesRegex(ValueError, "operational role inventory differs"):
                epoch.compare_epoch_projections(self.bundle, result, self.bundle, forged)
            original_digest = result["unclassified_context_digest"]
            context["inputs"] = [{**pin, "exists": True} for pin in self.pins.values()]
            changed = epoch.declared_from_authenticated_refs(
                self.bundle, self.bundle.directory, context, expected, self.pins["snapshot"],
                self.pins["resource"], [], {"image": self.pins["resource"]},
                {"font": self.pins["font"]}, self.declared()["workers"],
                {"producer": self.pins["program"]},
            )
            self.assertNotEqual(original_digest, changed["unclassified_context_digest"])
            with self.assertRaisesRegex(ValueError, "epoch role or unclassified context differs"):
                epoch.compare_epoch_projections(self.bundle, result, self.bundle, changed)
            with self.assertRaises(ValueError):
                epoch.declared_from_authenticated_refs(
                    self.bundle, self.bundle.directory, context, expected, self.pins["snapshot"],
                    None, [], {"image": self.pins["resource"]}, {}, self.declared()["workers"],
                    {"producer": self.pins["program"]},
                )
            with self.assertRaisesRegex(ValueError, "program source snapshot differs"):
                epoch.declared_from_authenticated_refs(
                    self.bundle, self.bundle.directory, context, expected, self.pins["snapshot"],
                    self.pins["resource"], [], {"image": self.pins["resource"]},
                    {"font": self.pins["font"]}, self.declared()["workers"],
                    {"producer": self.pins["font"]},
                )


if __name__ == "__main__":
    unittest.main()
