from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_epoch_reuse as epoch
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

    def test_identical_transport_pins_and_context_are_accepted(self) -> None:
        epoch.compare_declared_inputs(self.bundle, self.declared(), self.bundle, self.declared())

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
