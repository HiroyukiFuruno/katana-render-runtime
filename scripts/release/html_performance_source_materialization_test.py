"""Historical source materialization preserves the exact selected Git tree."""
from __future__ import annotations

import tempfile
import unittest
import hashlib
import json
from collections.abc import Sequence
from pathlib import Path
from unittest.mock import patch

import html_performance_reference as reference
import html_performance_source_materialization as materialization
import verify_html_performance_evidence as gate
from verify_html_performance_evidence import GitQuery


class HistoricalSourceMaterializationTest(unittest.TestCase):
    def test_selected_source_uses_shared_harness_authentication(self) -> None:
        selected = reference.ActiveReference("record", "a" * 40, "b" * 64, "c" * 64,
                                             {"lockSha256": "d" * 64})
        lock = gate.HARNESS_ROOT + "Cargo.lock"
        records: gate.SourceRecords = {lock: ("d" * 64, 4)}
        def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str:
            del root, input_bytes, text
            if args[0] == "rev-parse":
                return "f" * 40
            if args[0] in {"status", "merge-base"}:
                return ""
            raise ValueError("unexpected Git query")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            destination = root / "tmp/reference"
            for file in (root / lock, destination / lock):
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b"lock")
            with patch.object(reference, "active_reference_at_commit", return_value=selected), \
                 patch.object(materialization, "verify_materialized", return_value=records), \
                 patch.object(gate, "committed_records", return_value=records), \
                 patch.object(materialization, "_verify_harness") as harness, \
                 patch.object(materialization, "verify_harness_closure"):
                self.assertEqual(materialization.verify_selected_source(
                    root, destination, selected, git_query=query), records)
                harness.assert_called_once_with(root, selected.source_head, records, records, query)

    def test_transported_source_proof_uses_git_tree_without_producer_host_path(self) -> None:
        selected = reference.ActiveReference("record", "a" * 40, "b" * 64, "c" * 64, {"lockSha256": "d" * 64})
        lock = gate.HARNESS_ROOT + "Cargo.lock"
        source = gate.HARNESS_ROOT + "src/main.rs"
        records: gate.SourceRecords = {lock: ("d" * 64, 4), source: ("e" * 64, 4)}
        rows = [{"path": name, "mode": "0644", "sha256": checksum, "size": size}
                for name, (checksum, size) in records.items()]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            local_lock = root / lock
            local_lock.parent.mkdir(parents=True)
            local_lock.write_bytes(b"lock")
            controller = root / "controller.json"
            controller.write_bytes(b"controller")
            pin = {"path": "/producer/controller.json", "sha256": gate.sha256(controller)}
            proof: dict[str, object] = {"schema": "krr-html-performance-selected-source-materialization-v1",
                "selection": materialization._selection_value(selected), "controllerSourceProof": pin,
                "materializedRoot": "/producer/repo/tmp/reference", "sourceRecords": rows}
            proof["digest"] = hashlib.sha256(json.dumps(proof, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str:
                del root, input_bytes, text
                if args[0] == "rev-parse":
                    return "f" * 40
                if args[0] == "show":
                    return "lock"
                if args[0] == "merge-base":
                    return ""
                raise ValueError("unexpected Git query")
            with patch.object(reference, "active_reference_at_commit", return_value=selected), \
                 patch.object(gate, "committed_records", return_value=records), \
                 patch.object(materialization, "_tree_records", return_value=rows), \
                 patch.object(materialization, "_verify_harness") as harness, \
                 patch.object(materialization, "verify_harness_closure"):
                checked = materialization.verify_transported_source_proof(
                    root, selected, pin, proof, Path("/producer/repo"), file_resolver=lambda _pin: controller,
                    git_query=query)
                self.assertEqual(checked, records)
                harness.assert_called_once_with(root, selected.source_head, records, records, query)
                altered = dict(proof)
                altered["sourceRecords"] = [{**rows[0], "mode": "0755"}, rows[1]]
                unsigned = {key: value for key, value in altered.items() if key != "digest"}
                altered["digest"] = hashlib.sha256(json.dumps(
                    unsigned, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
                with self.assertRaisesRegex(ValueError, "historical source records differ"):
                    materialization.verify_transported_source_proof(
                        root, selected, pin, altered, Path("/producer/repo"),
                        file_resolver=lambda _pin: controller, git_query=query)

    def test_harness_modes_require_shared_identity_and_regular_additions(self) -> None:
        main = gate.HARNESS_ROOT + "src/main.rs"
        added = gate.HARNESS_ROOT + "src/rss.rs"
        old = {main: ("a" * 64, 1)}
        current = {**old, added: ("b" * 64, 1)}
        root = Path("/fixture")
        old_rows = [(main, "a" * 40, 0o644)]
        for mode_main, mode_added, valid in ((0o644, 0o644, True),
                                              (0o755, 0o644, False),
                                              (0o644, 0o755, False)):
            with self.subTest(mode_main=mode_main, mode_added=mode_added):
                rows = [(main, "a" * 40, mode_main), (added, "b" * 40, mode_added)]
                with patch.object(materialization, "_tree", side_effect=[old_rows, rows]), \
                     patch.object(materialization, "verify_harness_sources") as verify:
                    if valid:
                        materialization._verify_harness(root, "old", old, current, None)
                        verify.assert_called_once()
                    else:
                        with self.assertRaisesRegex(ValueError, "harness mode differs"):
                            materialization._verify_harness(root, "old", old, current, None)
                        verify.assert_not_called()

    def test_selected_source_proof_binds_selection_controller_and_tree(self) -> None:
        selected = reference.ActiveReference("active-record", "a" * 40, "b" * 64,
                                             "c" * 64, {"lockSha256": "d" * 64})
        records: gate.SourceRecords = {"src/lib.rs": ("e" * 64, 4)}
        tree = [{"path": "src/lib.rs", "mode": "0644", "sha256": "e" * 64, "size": 4}]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "tmp").mkdir()
            destination = root / "tmp/materialized"
            destination.mkdir()
            controller_pin_path = root / "controller-source.json"
            controller_pin_path.write_text("pinned controller proof", encoding="utf8")
            controller_pin = {"path": str(controller_pin_path),
                              "sha256": gate.sha256(controller_pin_path)}
            with patch.object(materialization, "verify_selected_source", return_value=records), \
                 patch.object(materialization, "_tree_records", return_value=tree):
                proof = materialization.create_selected_source_proof(
                    root, destination, selected, controller_pin)
                selection_value = gate._object(proof["selection"], "source proof selection")
                self.assertEqual(selection_value["recordId"], "active-record")
                self.assertEqual(selection_value["sourceCommit"], "a" * 40)
                self.assertEqual(proof["controllerSourceProof"], controller_pin)
                self.assertEqual(proof["sourceRecords"], tree)
                self.assertEqual(materialization.verify_selected_source_proof(
                    root, destination, selected, controller_pin, proof), records)
                altered = dict(proof)
                altered["sourceRecords"] = [{**tree[0], "mode": "0755"}]
                with self.assertRaisesRegex(ValueError, "digest differs"):
                    materialization.verify_selected_source_proof(
                        root, destination, selected, controller_pin, altered)
                controller_pin_path.write_text("changed", encoding="utf8")
                with self.assertRaisesRegex(ValueError, "controller source proof pin differs"):
                    materialization.verify_selected_source_proof(root, destination, selected, controller_pin, proof)

    def test_rejects_unsupported_tree_entries(self) -> None:
        def entry_query(selected: bytes) -> GitQuery:
            def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> bytes:
                del root, args, input_bytes, text
                return selected
            return query

        for entry in (b"120000 blob " + b"b" * 40 + b"\tlink\0",
                      b"100644 blob " + b"b" * 40 + b"\t../escape\0",
                      b"160000 commit " + b"b" * 40 + b"\tsubmodule\0"):
            with self.subTest(entry=entry):
                with self.assertRaisesRegex(ValueError, "unsupported historical source entry"):
                    materialization._tree(Path("/fixture"), "a" * 40, entry_query(entry))

    def test_materializes_full_tree_and_rejects_mutation(self) -> None:
        revision = "a" * 40
        entries = (("100644", "b" * 40, "nested/source", b"source"),
                   ("100755", "c" * 40, "script", b"script"))

        def query(root: Path, args: Sequence[str], input_bytes: bytes | None,
                  text: bool) -> str | bytes:
            del root
            if tuple(args) == ("status", "--porcelain=v1", "--untracked-files=all") and text:
                return ""
            if tuple(args) == ("merge-base", "--is-ancestor", revision, "HEAD") and text:
                return ""
            if tuple(args) == ("rev-parse", "--verify", revision + "^{commit}") and text:
                return revision
            if tuple(args) == ("ls-tree", "-r", "-z", revision) and not text:
                return b"".join(f"{mode} blob {oid}\t{name}".encode() + b"\0"
                                for mode, oid, name, _ in entries)
            if tuple(args) == ("cat-file", "--batch") and not text:
                if input_bytes != "".join(oid + "\n" for _, oid, _, _ in entries).encode():
                    raise ValueError("unexpected object request")
                return b"".join(f"{oid} blob {len(data)}\n".encode() + data + b"\n"
                                for _, oid, _, data in entries)
            raise ValueError("unexpected Git query")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "tmp").mkdir()
            destination = root / "tmp/reference"
            with patch.object(materialization, "_preflight_harness", side_effect=ValueError("harness differs")), \
                 self.assertRaisesRegex(ValueError, "harness differs"):
                materialization.materialize(root, destination, revision, git_query=query)
            self.assertFalse(destination.exists())
            with patch.object(materialization, "_preflight_harness") as preflight:
                records = materialization.materialize(root, destination, revision, git_query=query)
                preflight.assert_called_once()
            self.assertEqual(set(records), {"nested/source", "script"})
            self.assertEqual((destination / "nested/source").read_bytes(), b"source")
            self.assertEqual((destination / "script").stat().st_mode & 0o777, 0o755)
            with patch.object(materialization, "_preflight_harness"), self.assertRaises(FileExistsError):
                materialization.materialize(root, destination, revision, git_query=query)
            (destination / "script").chmod(0o644)
            with self.assertRaisesRegex(ValueError, "historical source file differs"):
                materialization.verify_materialized(root, destination, revision, git_query=query)
            (destination / "script").chmod(0o755)
            (destination / "nested/source").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "historical source file differs"):
                materialization.verify_materialized(root, destination, revision, git_query=query)
            (destination / "nested/source").write_bytes(b"source")
            (destination / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "historical source file inventory differs"):
                materialization.verify_materialized(root, destination, revision, git_query=query)
