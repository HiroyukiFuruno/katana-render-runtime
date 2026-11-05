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
from verify_html_performance_evidence import _v3_authenticate_programs, _verified_reuse_context, committed_records


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True)


class SourceReuseTest(unittest.TestCase):
    def test_reviewed_producer_cohort_reaches_current_worktree(self) -> None:
        root = Path(__file__).resolve().parents[2]
        for name in ("produce_html_performance_evidence.py", "produce_html_performance_evidence_test.py"):
            path = "scripts/release/" + name
            current = (root / path).read_bytes()
            new_record = (hashlib.sha256(current).hexdigest(), len(current))
            new_mode = "100755" if (root / path).stat().st_mode & 0o111 else "100644"
            for head in ("70c3981a0df4f3a3833b4833bb78248a08602ee3", "9e88ede8"):
                old = subprocess.check_output(["git", "-C", str(root), "show", head + ":" + path])
                old_record = (hashlib.sha256(old).hexdigest(), len(old))
                old_mode = git(root, "ls-tree", head, "--", path).split()[0]
                unknown = (hashlib.sha256(current + b" ").hexdigest(), len(current) + 1)
                with self.subTest(path=path, head=head):
                    self.assertTrue(reuse.validation_cohort_change(path, old_record, new_record, old_mode, new_mode))
                    self.assertFalse(reuse.validation_cohort_change(path, old_record, unknown, old_mode, new_mode))
                    self.assertFalse(reuse.validation_cohort_change(path, old_record, new_record, old_mode, "100755"))

    def test_historical_validation_cohort_auth_module_is_retained(self) -> None:
        root = Path(__file__).resolve().parents[2]
        path = "scripts/release/html_performance_source_reuse.py"
        heads = ("70c3981a0df4f3a3833b4833bb78248a08602ee3", "3469ac28", "7d8c4c0ed9bb6ea4fb0efe3f0ab232db40a9976c")
        blobs = [subprocess.check_output(["git", "-C", str(root), "show", head + ":" + path])
                 for head in heads]
        records = [(hashlib.sha256(blob).hexdigest(), len(blob)) for blob in blobs]
        modes = [git(root, "ls-tree", head, "--", path).split()[0] for head in heads]
        for index in (1, 2):
            self.assertTrue(reuse.validation_cohort_change(path, records[0], records[index], modes[0], modes[index]))
            self.assertFalse(reuse.validation_cohort_change(path, records[0],
                             (hashlib.sha256(blobs[index] + b" ").hexdigest(), len(blobs[index]) + 1),
                             modes[0], modes[index]))
            self.assertFalse(reuse.validation_cohort_change(path, records[0], records[index], modes[0], "100755"))

    def test_historical_validation_cohort_actual_git_endpoints(self) -> None:
        root = Path(__file__).resolve().parents[2]
        old_head = "70c3981a0df4f3a3833b4833bb78248a08602ee3"
        new_head = git(root, "rev-parse", "3469ac28").strip()
        prior_head = "7d8c4c0ed9bb6ea4fb0efe3f0ab232db40a9976c"
        def items(head: str) -> tuple[tuple[str, str, int], ...]:
            records = committed_records(root, head)
            return tuple((path, checksum, size) for path, (checksum, size) in sorted(records.items())
                         if path.startswith("scripts/") or path == "Justfile")
        old_items = items(old_head)
        self.assertEqual(_v3_authenticate_programs(root, old_head, old_items,
                         ((new_head, items(new_head)), (prior_head, items(prior_head))),
                         git_query=None), old_items)

    def test_reviewed_fact_cache_program_changes_are_exact(self) -> None:
        root = Path(__file__).resolve().parents[2]
        old_head = "3469ac28"
        new_head = "4d7dabbbfb8f848fcadf2b3ce1c5b6e83789747a"
        for name in ("verify_html_performance_evidence.py", "verify_html_performance_evidence_test.py"):
            path = "scripts/release/" + name
            blobs = [subprocess.check_output(["git", "-C", str(root), "show", head + ":" + path])
                     for head in (old_head, new_head)]
            records = [(hashlib.sha256(blob).hexdigest(), len(blob)) for blob in blobs]
            modes = [git(root, "ls-tree", head, "--", path).split()[0] for head in (old_head, new_head)]
            with self.subTest(path=path):
                self.assertTrue(reuse.validation_cohort_change(path, records[0], records[1], modes[0], modes[1]))
                self.assertFalse(reuse.validation_cohort_change(path, records[0],
                                 (hashlib.sha256(blobs[1] + b" ").hexdigest(), len(blobs[1]) + 1), modes[0], modes[1]))
                self.assertFalse(reuse.validation_cohort_change(path, records[0], records[1], modes[0], "100755"))
        path = "scripts/release/html_performance_source_reuse.py"
        old = subprocess.check_output(["git", "-C", str(root), "show", old_head + ":" + path])
        current = (root / path).read_bytes()
        self.assertTrue(reuse.known_auth_program_change(path, old, current))
        self.assertFalse(reuse.known_auth_program_change(path, old + b" ", current))
        self.assertFalse(reuse.known_auth_program_change(path, old, current + b" "))
        test_path = "scripts/release/html_performance_source_reuse_test.py"
        test_old = subprocess.check_output(["git", "-C", str(root), "show", "7d8c4c0ed9bb6ea4fb0efe3f0ab232db40a9976c:" + test_path])
        test_new = (root / test_path).read_bytes()
        old_record = (hashlib.sha256(test_old).hexdigest(), len(test_old))
        new_record = (hashlib.sha256(test_new).hexdigest(), len(test_new))
        self.assertTrue(reuse.validation_cohort_change(test_path, old_record, new_record, "100644", "100644"))
        self.assertFalse(reuse.validation_cohort_change(test_path, old_record, new_record, "100644", "100755"))

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



class HtmlReleaseReuseTest(unittest.TestCase):
    def fixture(self, root: Path, case: str) -> tuple[str, str]:
        repository = Path(__file__).resolve().parents[2]
        baseline = "b9cf37535ba374b62fee4e889f9c58f58f1ab8dc"
        parent = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session.rs"
        child = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session_resource_reuse_tests.rs"
        document = "docs/release.md"
        def original(path: str) -> bytes:
            return subprocess.check_output(["git", "-C", str(repository), "show", baseline + ":" + path])
        def write(path: str, content: bytes) -> None:
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
        previous_parent = original(parent)
        previous_document = original(document)
        child_bytes = (repository / child).read_bytes()
        suffix = b'\n#[cfg(test)]\n#[path = "session_resource_reuse_tests.rs"]\nmod resource_reuse_tests;\n'
        removed_line = b'- \x60Test and Build (macos-15-intel, mac-x64)\x60\n'
        write(parent, previous_parent)
        write(document, previous_document)
        write("other.rs", b"unchanged")
        if case == "existing_child":
            write(child, child_bytes)
        git(root, "init", "-q")
        git(root, "add", ".")
        git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
            "commit", "-qm", "baseline")
        measured_head = git(root, "rev-parse", "HEAD").strip()
        if case != "doc_only":
            if case != "child_only":
                effective_suffix = suffix
                if case == "bad_cfg":
                    effective_suffix = suffix.replace(b"cfg(test)", b"cfg(not(test))")
                if case == "wrong_module":
                    effective_suffix = suffix.replace(b"mod resource_reuse_tests;", b"mod other;")
                if case == "duplicate":
                    effective_suffix += suffix
                if case == "parent_body":
                    effective_suffix += b"fn injected() {}\n"
                write(parent, previous_parent + effective_suffix)
            if case != "parent_only":
                write(child, child_bytes + (b" " if case == "bad_child" else b""))
        if case != "html_only":
            changed_document = previous_document.replace(removed_line, b"", 1)
            if case == "doc_extra":
                changed_document += b" "
            write(document, changed_document)
        if case == "unknown_add":
            write("other2.rs", b"unexpected")
        if case == "unknown_remove":
            (root / "other.rs").unlink()
        if case in {"new_mode", "doc_mode"}:
            (root / (child if case == "new_mode" else document)).chmod(0o755)
        git(root, "add", "-A")
        if case in {"new_mode", "doc_mode"}:
            git(root, "update-index", "--chmod=+x", child if case == "new_mode" else document)
        git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
            "commit", "-qm", "candidate")
        if case in {"new_mode", "doc_mode"}:
            path = child if case == "new_mode" else document
            self.assertTrue(git(root, "ls-tree", "HEAD", "--", path).startswith("100755 blob "))
        return measured_head, git(root, "rev-parse", "HEAD").strip()

    def test_exact_html_and_document_changes_are_independently_accepted(self) -> None:
        for case in ("both", "html_only", "doc_only"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                measured, current = self.fixture(root, case)
                result = _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)
                if not isinstance(result, reuse.SourceReuseContext):
                    raise AssertionError("source reuse context differs")
                self.assertEqual((result.measured_head, result.eligible_head), (measured, current))
                self.assertEqual(result.artifact_equivalence, case != "doc_only")

    def test_unknown_html_document_membership_and_mode_are_rejected(self) -> None:
        cases = ("parent_only", "child_only", "bad_cfg", "wrong_module", "duplicate", "parent_body",
                 "bad_child", "doc_extra", "unknown_add", "unknown_remove", "new_mode", "doc_mode",
                 "existing_child")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                measured, _ = self.fixture(root, case)
                messages = {"new_mode": "HTML test membership or mode differs",
                            "doc_mode": "release document delta differs"}
                rejection = (self.assertRaisesRegex(ValueError, messages[case]) if case in messages
                             else self.assertRaises(ValueError))
                with rejection:
                    _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)


class ReleaseToolingReuseTest(unittest.TestCase):
    tooling = (".github/workflows/release-preflight.yml", ".github/workflows/release.yml",
               ".github/workflows/test-and-build.yml", "package.json", "bun.lock", "biome.json")

    def fixture(self, root: Path, selected: tuple[str, ...], case: str = "valid") -> str:
        repository = Path(__file__).resolve().parents[2]
        baseline = "b9cf37535ba374b62fee4e889f9c58f58f1ab8dc"
        parent = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session.rs"
        child = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session_resource_reuse_tests.rs"
        document = "docs/release.md"
        for path in self.tooling:
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            old = subprocess.check_output(["git", "-C", str(repository), "show", baseline + ":" + path])
            target.write_bytes(old + (b" " if case == "old_bytes" and path == selected[0] else b""))
            if case == "old_missing" and path == selected[0]:
                target.unlink()
        if case == "combined":
            for path in (parent, document):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(subprocess.check_output(["git", "-C", str(repository), "show", baseline + ":" + path]))
        if case == "old_mode":
            (root / selected[0]).chmod(0o755)
        git(root, "init", "-q")
        git(root, "add", ".")
        if case == "old_mode":
            git(root, "update-index", "--chmod=+x", selected[0])
        git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid", "commit", "-qm", "old")
        measured = git(root, "rev-parse", "HEAD").strip()
        if case == "old_mode":
            self.assertTrue(git(root, "ls-tree", measured, "--", selected[0]).startswith("100755 blob "))
            (root / selected[0]).chmod(0o644)
        if case == "combined":
            (root / parent).write_bytes((root / parent).read_bytes() +
                b'\n#[cfg(test)]\n#[path = "session_resource_reuse_tests.rs"]\nmod resource_reuse_tests;\n')
            (root / child).write_bytes((repository / child).read_bytes())
            (root / document).write_bytes((root / document).read_bytes().replace(
                b'- `Test and Build (macos-15-intel, mac-x64)`\n', b"", 1))
        for path in selected:
            target = root / path
            if case == "removed" and path == selected[0]:
                target.unlink()
            else:
                value = (repository / path).read_bytes()
                target.write_bytes(value + (b" " if case == "new_bytes" and path == selected[0] else b""))
        if case == "unknown":
            (root / ".github/workflows/unknown.yml").write_text("unexpected\n")
        if case == "new_mode":
            (root / selected[0]).chmod(0o755)
        git(root, "add", "-A")
        if case == "new_mode":
            git(root, "update-index", "--chmod=+x", selected[0])
        git(root, "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid", "commit", "-qm", "new")
        if case == "new_mode":
            self.assertTrue(git(root, "ls-tree", "HEAD", "--", selected[0]).startswith("100755 blob "))
        if case == "dirty":
            (root / selected[0]).write_bytes(b"uncommitted")
        return measured

    def test_exact_tooling_pairs_are_independently_accepted(self) -> None:
        for selected in (*((path,) for path in self.tooling), self.tooling):
            with self.subTest(selected=selected), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                measured = self.fixture(root, selected)
                result = _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)
                if not isinstance(result, reuse.SourceReuseContext):
                    raise AssertionError("source reuse context differs")
                self.assertEqual(result.eligible_head, git(root, "rev-parse", "HEAD").strip())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            measured = self.fixture(root, self.tooling, "combined")
            _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)

    def test_unknown_bytes_membership_mode_and_dirty_source_are_rejected(self) -> None:
        for path in self.tooling:
            for case in ("old_bytes", "new_bytes", "old_mode", "new_mode", "old_missing", "removed", "dirty"):
                with self.subTest(path=path, case=case), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary).resolve()
                    measured = self.fixture(root, (path,), case)
                    if case != "dirty":
                        self.assertEqual(git(root, "status", "--porcelain=v1"), "")
                    rejection = (self.assertRaisesRegex(ValueError, "release tooling membership or mode differs")
                                 if case in {"old_mode", "new_mode"} else self.assertRaises(ValueError))
                    with rejection:
                        _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            measured = self.fixture(root, self.tooling, "unknown")
            self.assertEqual(git(root, "status", "--porcelain=v1"), "")
            with self.assertRaises(ValueError):
                _verified_reuse_context(root, measured, git_query=None, allow_test_changes=True)


if __name__ == "__main__":
    unittest.main()
