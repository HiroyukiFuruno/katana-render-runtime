"""Definitions leaf physical input closure and drift rejection."""
from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_definitions_inputs as definitions


class DefinitionsInputTests(unittest.TestCase):
    def test_physical_sources_and_exact_resolution_are_bound(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source.ts"
            probe = root / "probe.ts"
            source.write_text("export const value = 1;", encoding="utf8")
            probe.write_text("probe", encoding="utf8")
            def digest(path: Path) -> str:
                return hashlib.sha256(path.read_bytes()).hexdigest()
            expected = {"sources": [str(source)], "tools": [], "dedicatedTools": []}
            completed = subprocess.CompletedProcess([], 0, json.dumps(expected).encode(), b"")
            with (patch.object(definitions, "REVIEWED", {"source.ts": digest(source)}),
                  patch.object(definitions, "COLLECTOR", {"probe.ts": digest(probe)}),
                  patch.object(subprocess, "run", return_value=completed)):
                first = definitions.collect(root)
                (root / "unrelated.rs").write_text("changed", encoding="utf8")
                self.assertEqual(definitions.collect(root), first)
                source.write_text("export const value = 2;", encoding="utf8")
                with self.assertRaisesRegex(ValueError, "reviewed_source_changed"):
                    definitions.collect(root)

    def test_unexpected_resolved_tool_or_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source.ts"
            source.write_text("fixture", encoding="utf8")
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            for key in ("sources", "tools", "dedicatedTools"):
                with self.subTest(key=key):
                    result = {"sources": [str(source)], "tools": [], "dedicatedTools": []}
                    result[key].append(str(root / "unexpected"))
                    completed = subprocess.CompletedProcess([], 0, json.dumps(result).encode(), b"")
                    with (patch.object(definitions, "REVIEWED", {"source.ts": digest}),
                          patch.object(definitions, "COLLECTOR", {}),
                          patch.object(subprocess, "run", return_value=completed)):
                        with self.assertRaisesRegex(ValueError, "import_inventory_changed"):
                            definitions.collect(root)
