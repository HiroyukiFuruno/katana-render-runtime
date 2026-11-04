from __future__ import annotations

import hashlib
import subprocess
import tempfile
import unittest
from collections.abc import Sequence
from pathlib import Path
from unittest.mock import patch

import html_performance_source_reuse as reuse
from html_performance_source_reuse import validate_reuse_source
from verify_html_performance_evidence import _verified_reuse_context


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True)


class SourceReuseTest(unittest.TestCase):
    def test_release_only_program_changes_preserve_membership_and_modes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            names = sorted(reuse.RELEASE_ONLY_PROGRAMS)
            names.append("scripts/release/worker.py")
            old = {name: (hashlib.sha256(b"old").hexdigest(), 3) for name in names}
            current = dict(old)
            for name in names:
                file = root / name
                file.parent.mkdir(parents=True, exist_ok=True)
                data = b"new" if name in reuse.RELEASE_ONLY_PROGRAMS else b"old"
                file.write_bytes(data)
                current[name] = (hashlib.sha256(data).hexdigest(), len(data))
            modes = {name: "100644" for name in names}
            def tree(selected: dict[str, str]) -> bytes:
                return b"".join(f"{mode} blob abc\t{name}\0".encode()
                                for name, mode in selected.items())
            def text(args: Sequence[str]) -> str:
                return "old" if args[0] == "merge-base" else "new" if args[0] == "rev-parse" else ""
            def validate(records: reuse.Records, new_modes: dict[str, str]) -> reuse.SourceReuseContext:
                return validate_reuse_source(root, "old", records, old, text,
                    lambda args: tree(modes if args[-1] == "old" else new_modes),
                    program_records=old, program_modes=modes)
            self.assertEqual(set(validate(current, modes).changed_paths), reuse.RELEASE_ONLY_PROGRAMS)
            worker = "scripts/release/worker.py"
            with self.assertRaisesRegex(ValueError, "current measurement program differs"):
                validate({**current, worker: ("0" * 64, 3)}, modes)
            removed = dict(current)
            removed.pop(names[0])
            with self.assertRaises(ValueError):
                validate(removed, modes)
            with self.assertRaisesRegex(ValueError, "current measurement program differs"):
                validate(current, {**modes, names[0]: "100755"})

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

    def test_artifact_candidate_requires_test_connections_and_rejects_worker_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            parent = reuse.TEST_PARENT
            test = reuse.TEST_ROOT + "svg_engine_text_layout_spacing_tests.rs"
            sources = {
                parent: '#[cfg(test)]\n#[path = "svg_engine_parser_boundary_tests.rs"]\nmod parser_boundary_tests;\n'
                        '#[cfg(test)]\n#[path = "svg_engine_filter_parser_model_tests.rs"]\nmod filter_parser_model_tests;\n',
                test: "old test",
                reuse.TEST_ROOT + "svg_engine_filter_parser_model_tests.rs": '#[path = "svg_engine_filter_parser_boundary_tests.rs"]\nmod parser_boundary_tests;',
                reuse.TEST_ROOT + "svg_engine_text_model_tests.rs": '#[path = "svg_engine_text_layout_matrix_tests.rs"]\nmod layout_matrix_tests;',
                reuse.TEST_ROOT + "svg_engine_text_layout_matrix_tests.rs": '#[path = "svg_engine_text_layout_spacing_tests.rs"]\nmod spacing_tests;',
                "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/text.rs": '#[cfg(test)]\ncrate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_text_model_tests.rs");',
                reuse.TEST_ROOT + "lib.rs": "root lib",
                "crates/katana-render-runtime/vendor/svg-engine/usvg/lib.rs": "usvg lib",
                "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/mod.rs": "tree mod",
                "scripts/release/worker.py": "worker",
            }
            current = {**sources, parent: sources[parent] + reuse.TEST_LOGGER, test: "new test"}
            def record(value: str) -> tuple[str, int]:
                raw = value.encode()
                return hashlib.sha256(raw).hexdigest(), len(raw)
            for path, value in current.items():
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(value)
            tree = b"".join(b"100644 blob abc\t" + path.encode() + b"\0" for path in current)
            def text(args: Sequence[str]) -> str:
                if args[0] == "show":
                    return sources[parent]
                return "old" if args[0] == "merge-base" else "new" if args[0] == "rev-parse" else ""
            old_records = {path: record(value) for path, value in sources.items()}
            current_records = {path: record(value) for path, value in current.items()}
            def validate(records: reuse.Records, allow: bool) -> reuse.SourceReuseContext:
                return validate_reuse_source(root, "old", records, old_records, text,
                                             lambda _args: tree, allow_test_changes=allow)
            anchor_paths = (set(sources) - {parent, test, "scripts/release/worker.py"})
            anchors = {path: record(sources[path])[0] for path in anchor_paths}
            with patch.object(reuse, "TEST_ANCHOR_SHA", anchors), \
                 patch.object(reuse, "TEST_PARENT_SHA", {record(sources[parent])[0]}):
                self.assertTrue(validate(current_records, True).artifact_equivalence)
                with self.assertRaises(ValueError):
                    validate(current_records, False)
                worker = root / "scripts/release/worker.py"
                worker.write_text("changed")
                changed = {**current_records, "scripts/release/worker.py": record("changed")}
                with self.assertRaises(ValueError):
                    validate(changed, True)
                worker.write_text("worker")
                for injected in ("production change", '/* #[cfg(test)] mod fake; */',
                                 'const FAKE: &str = "#[cfg(test)] mod fake;";'):
                    mutated = current[parent] + injected
                    (root / parent).write_text(mutated)
                    changed = {**current_records, parent: record(mutated)}
                    with self.subTest(injected=injected), self.assertRaises(ValueError):
                        validate(changed, True)
                (root / parent).write_text(current[parent])
                for forged in ("// " + sources[parent], sources[parent].replace("#[cfg(test)]\n", "")):
                    old_records[parent] = record(forged)
                    with self.subTest(forged=forged), self.assertRaises(ValueError):
                        validate(current_records, True)



if __name__ == "__main__":
    unittest.main()
