from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from schema import TreeSchemaError, canonical_entries
from tree_digest import SCHEMA, canonical_json_bytes, digest_input_tree


OID_A = "a" * 40
OID_B = "b" * 40


def tree(*entries: object, truncated: bool = False) -> dict[str, object]:
    return {"truncated": truncated, "tree": list(entries)}


def blob(path: str, oid: str = OID_A, mode: str = "100644", **extra: object) -> dict[str, object]:
    return {"path": path, "mode": mode, "type": "blob", "sha": oid, **extra}


def directory(path: str, oid: str = OID_A, mode: str = "040000", **extra: object) -> dict[str, object]:
    return {"path": path, "mode": mode, "type": "tree", "sha": oid, **extra}


class TreeDigestTests(unittest.TestCase):
    def test_canonical_digest_is_order_independent_and_excludes_only_evidence_directory(self) -> None:
        source = tree(
            blob("src/日本語.rs", OID_B),
            blob("ci-evidence/run.json", OID_A),
            blob("README.md", OID_A),
            blob("ci-evidence-not-excluded.json", OID_B),
            directory("src", OID_B),
        )
        reversed_source = tree(*reversed(source["tree"]))

        result = digest_input_tree(source)
        self.assertEqual(result, digest_input_tree(reversed_source))
        self.assertEqual(result["schema"], SCHEMA)
        paths = [entry["path"] for entry in result["input_tree"]["entries"]]
        self.assertEqual(paths, ["README.md", "ci-evidence-not-excluded.json", "src/日本語.rs"])
        self.assertEqual(result["digest"], hashlib.sha256(canonical_json_bytes(result["input_tree"])).hexdigest())

    def test_rejects_truncation_duplicates_nonregular_and_invalid_oids(self) -> None:
        invalid_cases = (
            tree(blob("a", OID_A), truncated=True),
            tree(blob("a", OID_A), blob("a", OID_B)),
            tree(blob("link", OID_A, mode="120000")),
            tree({"path": "submodule", "mode": "160000", "type": "commit", "sha": OID_A}),
            tree(blob("dir", OID_A, mode="040000")),
            tree(directory("dir", OID_A, mode="100644")),
            tree(blob("bad-oid", "A" * 40)),
            tree(blob("bad-length", "a" * 39)),
            tree(blob("ci-evidence/bad-link", OID_A, mode="120000")),
        )
        for source in invalid_cases:
            with self.subTest(source=source):
                with self.assertRaises(TreeSchemaError):
                    digest_input_tree(source)

    def test_rejects_unsafe_paths_and_schema_shape(self) -> None:
        invalid_cases = (
            tree(blob("../outside", OID_A)),
            tree(blob("/absolute", OID_A)),
            tree(blob("nested//empty", OID_A)),
            tree(blob("contains\x00nul", OID_A)),
            {"truncated": 0, "tree": []},
            {"truncated": False, "tree": "not-array"},
            {"truncated": False, "tree": [{"path": "a"}]},
        )
        for source in invalid_cases:
            with self.subTest(source=source):
                with self.assertRaises(TreeSchemaError):
                    canonical_entries(source)

    def test_canonical_json_is_compact_sorted_and_utf8(self) -> None:
        encoded = canonical_json_bytes({"z": "日本語", "a": [1, 2]})
        self.assertEqual(encoded, b'{"a":[1,2],"z":"' + "日本語".encode() + b'"}')
        self.assertFalse(encoded.startswith(b"\xef\xbb\xbf"))

    def test_cli_fails_closed_and_writes_canonical_output(self) -> None:
        script = Path(__file__).with_name("tree_digest.py")
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary = Path(temporary_directory)
            source_path = temporary / "tree.json"
            output_path = temporary / "digest.json"
            source_path.write_text(json.dumps(tree(blob("src/lib.rs", OID_A))), encoding="utf-8")
            completed = subprocess.run(
                [sys.executable, str(script), "--input", str(source_path), "--output", str(output_path)],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            expected = digest_input_tree(tree(blob("src/lib.rs", OID_A)))["digest"]
            self.assertEqual(json.loads(output_path.read_text(encoding="utf-8"))["digest"], expected)

            source_path.write_text(json.dumps(tree(blob("link", OID_A, mode="120000"))), encoding="utf-8")
            rejected = subprocess.run(
                [sys.executable, str(script), "--input", str(source_path)],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("regular-file mode", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
