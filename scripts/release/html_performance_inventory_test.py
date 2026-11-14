"""HTML性能用inventoryの並列hash契約を検証する。"""
from __future__ import annotations

import hashlib
import os
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from unittest.mock import patch

import html_performance_inventory as inventory_module
import verify_html_performance_evidence as gate


class HtmlPerformanceInventoryTests(unittest.TestCase):
    def test_missing_empty_and_regular_file_roots_have_fixed_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            missing = root / "missing"
            empty = root / "empty"
            empty.mkdir()
            regular = root / "single-file"
            regular.write_bytes(b"not a directory")
            expected = (
                {"root": str(missing), "exists": False, "files": []},
                {"root": str(empty), "exists": True, "files": []},
                {"root": str(regular), "exists": True, "files": []},
            )
            for path, result in zip((missing, empty, regular), expected, strict=True):
                with self.subTest(path=path.name):
                    self.assertEqual(inventory_module.inventory(path), result)

    def test_member_rows_match_fixed_hash_link_size_and_path_order(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "tree"
            nested = root / "a"
            nested.mkdir(parents=True)
            (root / "b.txt").write_bytes(b"beta")
            (nested / "x.txt").write_bytes(b"alpha")
            (root / "a-z.txt").write_bytes(b"alpha suffix")
            (root / "link.txt").symlink_to("b.txt")
            actual = inventory_module.inventory(root)
            expected = {
                "root": str(root),
                "exists": True,
                "files": [
                    {
                        "path": "a/x.txt",
                        "sha256": hashlib.sha256(b"alpha").hexdigest(),
                        "size": 5,
                    },
                    {
                        "path": "a-z.txt",
                        "sha256": hashlib.sha256(b"alpha suffix").hexdigest(),
                        "size": 12,
                    },
                    {
                        "path": "b.txt",
                        "sha256": hashlib.sha256(b"beta").hexdigest(),
                        "size": 4,
                    },
                    {"path": "link.txt", "link": "b.txt"},
                ],
            }
            self.assertEqual(actual, expected)

    def test_hash_failure_propagates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "file").write_bytes(b"content")
            with patch.object(gate, "sha256", side_effect=OSError("read failed")):
                with self.assertRaisesRegex(OSError, "read failed"):
                    inventory_module.inventory(root)

    def test_symlink_read_failure_propagates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "link").symlink_to("missing-target")
            with patch.object(os, "readlink", side_effect=OSError("link failed")):
                with self.assertRaisesRegex(OSError, "link failed"):
                    inventory_module.inventory(root)

    def test_file_change_during_hash_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "file"
            source.write_bytes(b"initial")
            original_hash = gate.sha256

            def rewrite(path: Path) -> str:
                digest = original_hash(path)
                path.write_bytes(b"changed")
                return digest

            with patch.object(gate, "sha256", side_effect=rewrite):
                with self.assertRaisesRegex(ValueError, "changed during hash"):
                    inventory_module.inventory(root)

    def test_hash_pool_is_bounded_at_eight_workers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "file").write_bytes(b"content")
            with patch.object(inventory_module, "ThreadPoolExecutor", wraps=ThreadPoolExecutor) as pool:
                inventory_module.inventory(root)
            pool.assert_called_once_with(max_workers=8)


if __name__ == "__main__":
    unittest.main()
