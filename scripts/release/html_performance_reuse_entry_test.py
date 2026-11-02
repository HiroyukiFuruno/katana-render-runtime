from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_source_reuse as reuse
import verify_html_performance_evidence as gate


class ReuseEntryTest(unittest.TestCase):
    def test_formal_failure_and_current_binary_mismatch_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            old = root / "old"
            new = root / "new"
            old.mkdir()
            new.mkdir()
            payload = b"{}"
            for directory in (old, new):
                (directory / "bundle.json").write_bytes(payload)
            checksum = hashlib.sha256(payload).hexdigest()
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


if __name__ == "__main__":
    unittest.main()
