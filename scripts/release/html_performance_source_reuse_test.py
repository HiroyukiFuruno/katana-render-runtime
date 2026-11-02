from __future__ import annotations

import hashlib
import subprocess
import tempfile
import unittest
from collections.abc import Sequence
from pathlib import Path

from html_performance_source_reuse import validate_reuse_source
from verify_html_performance_evidence import _verified_reuse_context


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True)


class SourceReuseTest(unittest.TestCase):
    def test_real_git_rejects_rename_mode_and_unrelated_source(self) -> None:
        for change in ("rename", "mode", "source"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                history = root / "docs/performance/html-performance-history.json"
                history.parent.mkdir(parents=True)
                history.write_text("before")
                source = root / "source.rs"
                source.write_text("before")
                git(root, "init", "-q")
                git(root, "add", ".")
                git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
                    "commit", "-qm", "before")
                before = git(root, "rev-parse", "HEAD").strip()
                if change == "rename":
                    git(root, "mv", "docs/performance/html-performance-history.json",
                        "docs/performance/README.md")
                elif change == "mode":
                    history.chmod(0o755)
                    git(root, "update-index", "--chmod=+x", "docs/performance/html-performance-history.json")
                else:
                    source.write_text("after")
                    git(root, "add", ".")
                git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
                    "commit", "-qm", "after")
                with self.assertRaises(ValueError):
                    _verified_reuse_context(root, before, git_query=None)

    def test_document_only_change_is_admitted_and_preserves_measured_head(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            history = "docs/performance/html-performance-history.json"
            (root / history).parent.mkdir(parents=True)
            (root / history).write_bytes(b"new")
            old = {history: (hashlib.sha256(b"old").hexdigest(), 3)}
            current = {history: (hashlib.sha256(b"new").hexdigest(), 3)}
            tree = b"100644 blob abc\t" + history.encode() + b"\0"
            def text(args: Sequence[str]) -> str:
                return "old" if args[0] == "merge-base" else "new" if args[0] == "rev-parse" else ""
            context = validate_reuse_source(root, "old", current, old, text, lambda _args: tree)
            self.assertEqual((context.measured_head, context.eligible_head), ("old", "new"))
            self.assertEqual(context.changed_paths, (history,))

    def test_rejects_non_document_dirty_nonancestor_and_mode_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = "src.rs"
            (root / path).write_bytes(b"new")
            old = {path: (hashlib.sha256(b"old").hexdigest(), 3)}
            current = {path: (hashlib.sha256(b"new").hexdigest(), 3)}
            tree = b"100644 blob abc\tsrc.rs\0"
            def text(args: Sequence[str]) -> str:
                return "old" if args[0] == "merge-base" else "new" if args[0] == "rev-parse" else ""
            with self.assertRaises(ValueError):
                validate_reuse_source(root, "old", current, old, text, lambda _args: tree)
            for bad_text in (
                lambda args: "other" if args[0] == "merge-base" else text(args),
                lambda args: " M src.rs" if args[0] == "status" else text(args),
            ):
                with self.assertRaises(ValueError):
                    validate_reuse_source(root, "old", current, current, bad_text, lambda _args: tree)
            def changed_mode(args: Sequence[str]) -> bytes:
                return tree if args[-1] == "old" else b"100755 blob abc\tsrc.rs\0"
            with self.assertRaises(ValueError):
                validate_reuse_source(root, "old", current, current, text, changed_mode)
            with self.assertRaises(ValueError):
                validate_reuse_source(root, "old", {}, current, text, lambda _args: tree)
            (root / path).write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                validate_reuse_source(root, "old", current, current, text, lambda _args: tree)


if __name__ == "__main__":
    unittest.main()
