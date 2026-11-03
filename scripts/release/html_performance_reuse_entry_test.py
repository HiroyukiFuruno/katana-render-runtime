from __future__ import annotations

import hashlib
import json
import os
import platform
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_source_reuse as reuse
import verify_html_performance_evidence as gate


def reuse_directories(root: Path) -> tuple[Path, Path, str]:
    old, new = root / "old", root / "new"
    for directory in (old, new):
        directory.mkdir()
        (directory / "bundle.json").write_bytes(b"{}")
    return old, new, hashlib.sha256(b"{}").hexdigest()


def measurement_tool() -> dict[str, object]:
    environment = {**os.environ, "GIT_OPTIONAL_LOCKS": "0"}
    raw = json.dumps(environment, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return {"environmentSha256": hashlib.sha256(raw.encode()).hexdigest(),
            "platform": platform.platform(), "cpuCount": os.cpu_count()}


def build_facts() -> dict[str, str]:
    return {key: key for key in ("binarySha256", "lockSha256", "cargoExecutableSha256",
            "rustcExecutableSha256", "buildEnvironmentSha256", "toolVersions", "cargoConfigPins",
            "cargoLauncher", "rustcLauncher", "guardPython")}


class ReuseEntryTest(unittest.TestCase):
    def test_program_source_reuse_requires_current_binary_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            old, new, checksum = reuse_directories(root)
            old_head, new_head = "a" * 40, "b" * 40
            context = reuse.SourceReuseContext(old_head, new_head, {}, ("scripts/worker.py",),
                                                program_equivalence=True)
            tool = measurement_tool()
            facts = build_facts()
            current = {**facts, "head": new_head}
            class FakeBundle:
                def __init__(self, directory: Path) -> None:
                    self.manifest = {"head": old_head if directory == old else new_head,
                                     "attempts": [{"expected": "expected", "result": "result"}]}
                def json(self, ref: object) -> dict[str, object]:
                    if ref == "expected":
                        return {"sourcePair": {"schema": "krr-html-performance-source-epoch-v3"},
                                "endpointRefs": {"before": "before", "end": "end"}}
                    return {"toolContext": tool}
                def relative_reference(self, _path: str, _sha: str) -> dict[str, object]:
                    return {}
            with (patch.object(gate, "Bundle", FakeBundle),
                  patch.object(gate, "git", return_value=new_head),
                  patch.object(gate, "_verified_v3_reuse_context", return_value=context),
                  patch.object(gate, "_verify", return_value={"conditions": 84}),
                  patch.object(gate, "_require_formal_release_result"),
                  patch.object(gate, "_v3_origin_and_endpoints", return_value=(context, facts)),
                  patch.object(gate, "_validate_build_proof", return_value=current)):
                args = (old, root, checksum, new, checksum, "artifact", checksum)
                result = gate.verify_source_reuse(*args)
                self.assertEqual(result["reuseKind"], "program-source-artifact-equivalence-v1")
                for key in ("binarySha256", "cargoConfigPins", "buildEnvironmentSha256"):
                    saved = current[key]
                    current[key] = "changed"
                    with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                        gate.verify_source_reuse(*args)
                    current[key] = saved

    def test_formal_failure_and_current_binary_mismatch_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            old, new, checksum = reuse_directories(root)
            measured_head, eligible_head = "a" * 40, "b" * 40
            context = reuse.SourceReuseContext(measured_head, eligible_head, {}, ())
            old_build = {"binarySha256": "a", "lockSha256": "l",
                         "cargoExecutableSha256": "c", "rustcExecutableSha256": "r"}
            new_build = {**old_build, "binarySha256": "different", "head": eligible_head}

            class FakeBundle:
                def __init__(self, directory: Path) -> None:
                    self.manifest = {"head": measured_head if directory == old else eligible_head,
                                     "attempts": [{"expected": {}}]}

                def json(self, _reference: object) -> dict[str, object]:
                    return {}

                def relative_reference(self, _path: str, _sha: str) -> dict[str, object]:
                    return {}

            with (patch.object(gate, "Bundle", FakeBundle),
                  patch.object(gate, "git", return_value=eligible_head),
                  patch.object(gate, "committed_records", return_value={}),
                  patch.object(reuse, "validate_reuse_source", return_value=context),
                  patch.object(gate, "_verify", return_value={}),
                  patch.object(gate, "validate_expected_builds", return_value={"candidate": old_build}),
                  patch.object(gate, "_validate_build_proof", return_value=new_build)):
                arguments = (old, root, checksum, new, checksum, "artifact", checksum)
                with self.assertRaisesRegex(ValueError, "HTML84 stage90 result must be an object"):
                    gate.verify_source_reuse(*arguments)
                with patch.object(gate, "_require_formal_release_result"):
                    with self.assertRaisesRegex(ValueError, "binarySha256"):
                        gate.verify_source_reuse(*arguments)

    def test_test_only_reuse_binds_environment_build_and_formal_result(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            old, new, checksum = reuse_directories(root)
            old_head, new_head = "a" * 40, "b" * 40
            context = reuse.SourceReuseContext(old_head, new_head, {}, ("test",), True)
            tool = measurement_tool()
            original_environment_sha = tool["environmentSha256"]
            facts = build_facts()
            current = {**facts, "head": new_head}
            class FakeBundle:
                def __init__(self, directory: Path) -> None:
                    self.manifest = {"head": old_head if directory == old else new_head,
                                     "attempts": [{"expected": "expected"}]}
                def json(self, ref: object) -> dict[str, object]:
                    return {"endpointRefs": {"before": "before"}} if ref == "expected" else {"toolContext": tool}
                def relative_reference(self, _path: str, _sha: str) -> dict[str, object]:
                    return {}
            with (patch.object(gate, "Bundle", FakeBundle),
                  patch.object(gate, "git", return_value=new_head),
                  patch.object(gate, "_verified_reuse_context", return_value=context),
                  patch.object(gate, "_verify", return_value={"conditions": 84}) as formal,
                  patch.object(gate, "_require_formal_release_result"),
                  patch.object(gate, "validate_expected_builds", return_value={"candidate": facts}),
                  patch.object(gate, "_validate_build_proof", return_value=current)):
                args = (old, root, checksum, new, checksum, "artifact", checksum)
                result = gate.verify_source_reuse(*args)
                self.assertEqual(result["reuseKind"], "test-source-artifact-equivalence-v1")
                formal.assert_called_once()
                for key in ("binarySha256", "buildEnvironmentSha256", "cargoConfigPins"):
                    saved = current[key]
                    current[key] = "different"
                    with self.subTest(key=key), self.assertRaises(ValueError):
                        gate.verify_source_reuse(*args)
                    current[key] = saved
                tool["environmentSha256"] = "changed"
                with self.assertRaisesRegex(ValueError, "measurement execution environment differs"):
                    gate.verify_source_reuse(*args)
                tool["environmentSha256"] = original_environment_sha
                formal.side_effect = ValueError("incomplete 84-condition evidence")
                with self.assertRaisesRegex(ValueError, "incomplete"):
                    gate.verify_source_reuse(*args)


if __name__ == "__main__":
    unittest.main()
