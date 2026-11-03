from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
