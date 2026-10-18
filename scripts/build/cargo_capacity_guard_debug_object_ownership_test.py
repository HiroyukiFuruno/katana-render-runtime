from __future__ import annotations

from contextlib import redirect_stderr
import io
import logging
import os
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Callable, MutableMapping, Sequence
from types import FrameType
from typing import cast
from unittest.mock import patch

import cargo_capacity_guard_debug_object_ownership as debug_ownership
from cargo_capacity_guard_build_script_ownership import file_stamp, source_pin
from cargo_capacity_guard_debug_object_ownership import (
    DebugObjectOwnership,
    debug_object_paths,
    macho_kind,
    create_debug_object_ownership,
)
from cargo_capacity_guard_library_alias_ownership import create_library_alias_ownership
from cargo_capacity_guard_lock import ProfileLocks, _fingerprint_files, evict_artifacts


def _macho(cputype: int) -> bytes:
    return b"\xcf\xfa\xed\xfe" + b"\0" * 8 + cputype.to_bytes(4, "little")


def _symtab(*paths: str) -> str:
    rows = [
        "----------------------------------------------------------------------\nSymbol table for:"
    ]
    rows.extend(
        f"[ {index}]	00000000 66 (N_OSO) 00000000 00000000 00000000 '{path}'"
        for index, path in enumerate(paths)
    )
    return "\n".join(rows) + "\n"


class DebugObjectOwnershipTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.target = self.root / "target"
        self.deps = self.target / "debug" / "deps"
        self.incremental = self.target / "debug" / "incremental" / "unit"
        self.deps.mkdir(parents=True)
        self.incremental.mkdir(parents=True)
        self.tool = self.root / "dsymutil"
        self.tool.write_bytes(b"tool")
        self.tool_pin = source_pin(self.tool)
        self.archive_tool = self.root / "ar"
        self.archive_tool.write_bytes(b"archive reader")
        self.archive_tool_pin = source_pin(self.archive_tool)
        self.endpoint = self.deps / "libconsumer.dylib"
        self.endpoint.write_bytes(_macho(2))
        self.object = self.deps / "libconsumer.foo.o"
        self.object.write_bytes(_macho(1))
        self.archive = self.deps / "libconsumer.rlib"
        self.archive.write_bytes(b"!<arch>\narchive")
        self.work_product = self.incremental / "work-products.bin"
        self.work_product.write_bytes(b"work product")
        self.protected = self.deps / "protected.o"
        self.protected.write_bytes(_macho(1))
        self.group_number = 0

    def census(self) -> list[tuple[Path, os.stat_result]]:
        return _fingerprint_files(self.target, self.target.stat().st_dev)[0]

    def proof(
        self, query: Callable[[list[str]], str] | None = None
    ) -> DebugObjectOwnership:
        return DebugObjectOwnership(
            self.target, self.tool_pin, query or (lambda _: _symtab()), lambda: None
        )

    def archive_proof(
        self, query: Callable[[list[str]], str] | None = None
    ) -> DebugObjectOwnership:
        def run(argv: list[str]) -> str:
            if len(argv) >= 2 and argv[-2] == "-t":
                return "member.o\n"
            return _symtab()

        return DebugObjectOwnership(
            self.target,
            self.tool_pin,
            query or run,
            lambda: None,
            archive_tool=self.archive_tool_pin,
        )

    def capture(
        self,
        proof: DebugObjectOwnership,
        references: Sequence[Path] = (),
        candidates: frozenset[Path] | None = None,
        groups: Sequence[tuple[frozenset[Path], frozenset[Path]]] = (),
    ) -> frozenset[Path]:
        files = self.census()
        selected = (
            candidates
            if candidates is not None
            else frozenset({self.object, self.protected})
        )
        return proof.capture(files, references, selected, groups=groups)

    def closed_alias_group(
        self,
    ) -> tuple[
        frozenset[Path],
        frozenset[Path],
        tuple[tuple[frozenset[Path], frozenset[Path]], ...],
    ]:
        group_number = self.group_number
        self.group_number += 1
        session = self.incremental / f"group-{group_number}"
        session.mkdir()
        self.group_object = session / "libconsumer.foo.o"
        self.group_object.write_bytes(_macho(1))
        alias = session / "libconsumer-alias.foo.o"
        os.link(self.group_object, alias)
        dep_graph = session / "dep-graph.bin"
        query_cache = session / "query-cache.bin"
        dep_graph.write_bytes(b"dependency graph")
        query_cache.write_bytes(b"query cache")
        objects = frozenset({self.group_object, alias})
        indexes = frozenset({dep_graph, query_cache})
        return objects, indexes, ((objects, indexes),)

    def selected_alias_group(
        self,
    ) -> tuple[DebugObjectOwnership, frozenset[Path], frozenset[Path], Path]:
        objects, indexes, groups = self.closed_alias_group()
        proof = self.proof()
        self.capture(proof, candidates=objects, groups=groups)
        proof.validate_selection(objects | indexes)
        alias = next(iter(objects - {self.group_object}))
        return proof, objects, indexes, alias

    def record_first_owned_unlink(
        self, proof: DebugObjectOwnership, alias: Path
    ) -> None:
        self.group_object.unlink()
        proof.record_owned_unlink(self.group_object, file_stamp(alias.lstat()))
        proof.verify()

    def test_macho_kind_distinguishes_object_endpoint_and_non_macho(self) -> None:
        self.assertEqual(macho_kind(self.object), 1)
        self.assertEqual(macho_kind(self.endpoint), 2)
        text = self.root / "text"
        text.write_bytes(b"plain")
        self.assertIsNone(macho_kind(text))

    def test_debug_paths_accept_exact_n_oso_and_ignore_archive_members(self) -> None:
        direct = self.root / "direct.o"
        archive = self.root / "libx.rlib(member.o)"
        result = debug_object_paths(_symtab(str(direct), str(archive)))
        self.assertEqual(result, frozenset({direct}))

    def test_debug_paths_preserve_archive_member_literal(self) -> None:
        archive = self.deps / "libx.rlib"
        direct, archives = debug_ownership._debug_paths(_symtab(f"{archive}(member.o)"))
        self.assertEqual(direct, frozenset())
        self.assertEqual(
            archives, frozenset({(archive, "member.o", Path(f"{archive}(member.o)"))})
        )

    def test_capture_archives_admits_only_unconsumed_rlib_and_owned_unlink(
        self,
    ) -> None:
        candidate = self.deps / "libunconsumed.rlib"
        candidate.write_bytes(self.archive.read_bytes())

        def consumed_query(argv: list[str]) -> str:
            return (
                _symtab(f"{self.archive}(member.o)")
                if "--symtab" in argv
                else "member.o\n"
            )

        proof = self.archive_proof(consumed_query)
        files = self.census()
        group = frozenset({candidate})
        self.assertEqual(
            proof.capture_archives(files, (self.endpoint,), (group,)), group
        )
        snapshot = proof.snapshot
        self.assertIsNotNone(snapshot)
        if snapshot is None:
            self.fail("archive consumer snapshot did not activate")
        self.assertEqual(snapshot.archive_groups, (group,))
        self.assertEqual(
            dict(snapshot.archives)[candidate], file_stamp(candidate.lstat())
        )
        self.assertIn(
            (
                self.endpoint,
                self.archive,
                "member.o",
                Path(f"{self.archive}(member.o)"),
            ),
            snapshot.archive_references,
        )
        self.assertIn(self.archive, dict(snapshot.retained))
        self.assertNotIn(candidate, dict(snapshot.retained))
        proof.validate_selection(group)
        candidate.unlink()
        proof.record_owned_unlink(candidate)
        proof.verify()
        self.assertIn(candidate, proof.absent)

    def test_capture_archives_rejects_any_linked_archive_alias(self) -> None:
        candidate = self.deps / "liblinked.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        alias = self.deps / "liblinked-alias.rlib"
        os.link(candidate, alias)

        def query(argv: list[str]) -> str:
            if "--symtab" in argv:
                return _symtab(f"{alias}(member.o)")
            return "member.o\n"

        proof = self.archive_proof(query)
        group = frozenset({candidate, alias})
        self.assertEqual(
            proof.capture_archives(self.census(), (), (group,)), frozenset()
        )
        self.assertIsNone(proof.snapshot)

    def test_capture_archives_requires_complete_exact_group_selection(self) -> None:
        candidate = self.deps / "libalias.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        alias = self.deps / "libalias-copy.rlib"
        os.link(candidate, alias)
        group = frozenset({candidate, alias})
        proof = self.archive_proof()
        self.assertEqual(proof.capture_archives(self.census(), (), (group,)), group)
        with self.assertRaisesRegex(RuntimeError, "complete archive groups"):
            proof.validate_selection(frozenset({candidate}))
        proof.validate_selection(group)
        candidate.unlink()
        with self.assertRaisesRegex(
            RuntimeError, "transition is missing or unexpected"
        ):
            proof.record_owned_unlink(candidate)
        proof.record_owned_unlink(candidate, file_stamp(alias.lstat()))
        proof.verify()
        alias.unlink()
        proof.record_owned_unlink(alias)
        proof.verify()

    def test_capture_archives_rejects_candidate_type_and_incomplete_alias_closure(
        self,
    ) -> None:
        for suffix in (".a", ".dylib", ".o"):
            candidate = self.deps / f"unowned{suffix}"
            candidate.write_bytes(b"!<arch>\n" if suffix == ".a" else _macho(1))
            with (
                self.subTest(suffix=suffix),
                self.assertRaisesRegex(RuntimeError, "rlib ownership"),
            ):
                self.archive_proof().capture_archives(
                    self.census(), (), (frozenset({candidate}),)
                )
        candidate = self.deps / "libexternal-alias.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        external = self.root / "external-alias.rlib"
        os.link(candidate, external)
        with self.assertRaisesRegex(RuntimeError, "alias closure"):
            self.archive_proof().capture_archives(
                self.census(), (), (frozenset({candidate}),)
            )

    def test_capture_archives_rejects_missing_protected_and_nonmatching_member(
        self,
    ) -> None:
        candidate = self.deps / "libcandidate.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        group = frozenset({candidate})
        proof = self.archive_proof()
        with self.assertRaisesRegex(RuntimeError, "census is incomplete"):
            proof.capture_archives(
                self.census(), (), (frozenset({self.deps / "missing.rlib"}),)
            )
        proof = self.archive_proof()
        self.assertEqual(
            proof.capture_archives(self.census(), (candidate,), (group,)), frozenset()
        )
        self.assertIsNone(proof.snapshot)

        def mismatch(argv: list[str]) -> str:
            if "--symtab" in argv:
                return _symtab(f"{self.archive}(other.o)")
            return "member.o\n"

        with self.assertRaisesRegex(RuntimeError, "member is missing"):
            self.archive_proof(mismatch).capture_archives(self.census(), (), (group,))

    def test_capture_archives_rejects_invalid_archive_and_propagates_reader_failure(
        self,
    ) -> None:
        candidate = self.deps / "libthin.rlib"
        candidate.write_bytes(b"!<thin>\nmember")
        with self.assertRaisesRegex(RuntimeError, "not a full archive"):
            self.archive_proof().capture_archives(
                self.census(), (), (frozenset({candidate}),)
            )
        candidate.write_bytes(b"!<arch>\n")
        with self.assertRaisesRegex(RuntimeError, "member listing is invalid"):
            self.archive_proof(
                lambda argv: "" if argv[-2] == "-t" else _symtab()
            ).capture_archives(self.census(), (), (frozenset({candidate}),))
        candidate.write_bytes(self.archive.read_bytes())

        def failed_reader(argv: list[str]) -> str:
            if argv[-2] == "-t":
                raise RuntimeError("ar -t failed")
            return _symtab()

        with self.assertRaisesRegex(RuntimeError, "ar -t failed"):
            self.archive_proof(failed_reader).capture_archives(
                self.census(), (), (frozenset({candidate}),)
            )

    def test_capture_archives_tool_and_source_pins_are_verified(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "archive reader is unavailable"):
            self.proof().capture_archives(
                self.census(), (), (frozenset({self.archive}),)
            )
        source = self.root / "source-pin"
        source.write_bytes(b"stable")

        def check_source() -> None:
            if source.read_bytes() != b"stable":
                raise RuntimeError("source input changed")

        def query(argv: list[str]) -> str:
            return "member.o\n" if argv[-2] == "-t" else _symtab()

        proof = DebugObjectOwnership(
            self.target,
            self.tool_pin,
            query,
            check_source,
            archive_tool=self.archive_tool_pin,
        )
        candidate = self.deps / "libsource.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        group = frozenset({candidate})
        proof.capture_archives(self.census(), (), (group,))
        source.write_bytes(b"changed")
        with self.assertRaisesRegex(RuntimeError, "source input changed"):
            proof.verify()

        proof = self.archive_proof()
        candidate = self.deps / "libtool-pin.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        proof.capture_archives(self.census(), (), (frozenset({candidate}),))
        self.archive_tool.write_bytes(b"changed archive reader")
        with self.assertRaisesRegex(RuntimeError, "LLVM tool changed"):
            proof.verify()

    def test_capture_archives_pins_missing_archive_and_literal_pair(self) -> None:
        candidate = self.deps / "libunrelated.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        missing = self.deps / "missing-consumer.a"
        literal = Path(f"{missing}(member.o)")

        def query(argv: list[str]) -> str:
            if argv[-2] == "-t":
                return "member.o\n"
            return _symtab(str(literal))

        proof = self.archive_proof(query)
        group = frozenset({candidate})
        self.assertEqual(proof.capture_archives(self.census(), (), (group,)), group)
        snapshot = proof.snapshot
        self.assertIsNotNone(snapshot)
        if snapshot is None:
            self.fail("archive snapshot did not activate")
        self.assertTrue({missing, literal}.issubset(snapshot.missing))
        self.assertIn(
            (self.endpoint, missing, "member.o", literal), snapshot.archive_references
        )
        missing.write_bytes(b"appeared")
        with self.assertRaisesRegex(RuntimeError, "previously missing input appeared"):
            proof.verify()

    def test_archive_snapshot_rejects_unexpected_namespace_entry(self) -> None:
        candidate = self.deps / "libnamespace.rlib"
        candidate.write_bytes(self.archive.read_bytes())
        proof = self.archive_proof()
        proof.capture_archives(self.census(), (), (frozenset({candidate}),))
        (self.deps / "unexpected-entry").write_bytes(b"namespace drift")
        with self.assertRaisesRegex(RuntimeError, "namespace changed"):
            proof.verify()

    def test_debug_paths_reject_bad_framing_records_and_noncanonical_paths(
        self,
    ) -> None:
        cases = (
            "bad\n",
            "----------------------------------------------------------------------\nSymbol table for:\n[ 1] 0 66 (N_OSO) malformed\n",
            "----------------------------------------------------------------------\nSymbol table for:\n[ 1] 0 66 N_OSO malformed\n",
            _symtab("relative.o"),
            _symtab(str(self.root / ".." / self.root.name / "x.o")),
        )
        for raw in cases:
            with self.subTest(raw=raw), self.assertRaises(RuntimeError):
                debug_object_paths(raw)

    def test_debug_paths_reject_unsupported_universal_macho(self) -> None:
        universal = self.root / "universal"
        universal.write_bytes(b"\xca\xfe\xba\xbe" + b"\0" * 12)
        with self.assertRaisesRegex(RuntimeError, "universal"):
            macho_kind(universal)

    def test_capture_admits_only_unconsumed_single_link_objects(self) -> None:
        consumed = self.root / "consumed.o"
        consumed.write_bytes(_macho(1))

        def query(argv: list[str]) -> str:
            return _symtab(str(consumed))

        proof = self.proof(query)
        with patch.object(proof, "query", query):
            eligible = self.capture(proof, (self.endpoint, self.archive))
        self.assertEqual(eligible, frozenset({self.object, self.protected}))
        stale_files = self.census()
        os.link(self.object, self.deps / "outside.o")
        proof = self.proof()
        with self.assertRaisesRegex(RuntimeError, "input census"):
            proof.capture(stale_files, (), frozenset({self.object, self.protected}))

    def test_progress_logging_is_silent_by_default_even_with_info_root_logger(
        self,
    ) -> None:
        root_logger = logging.getLogger()
        previous_level = root_logger.level
        root_logger.setLevel(logging.INFO)
        self.addCleanup(root_logger.setLevel, previous_level)
        output = io.StringIO()
        with redirect_stderr(output):
            self.capture(self.proof())
        self.assertEqual(output.getvalue(), "")

    def test_progress_logging_reports_bounded_counts_without_paths(self) -> None:
        logger = logging.getLogger(debug_ownership.PROGRESS_LOGGER_NAME)
        previous_level = logger.level
        logger.setLevel(logging.INFO)
        self.addCleanup(logger.setLevel, previous_level)
        output = io.StringIO()
        handler = logging.StreamHandler(output)
        logger.addHandler(handler)
        self.addCleanup(logger.removeHandler, handler)
        objects, _, groups = self.closed_alias_group()
        proof = self.proof(lambda _: _symtab(str(self.group_object)))

        self.capture(proof, candidates=objects, groups=groups)

        lines = output.getvalue().splitlines()
        self.assertLessEqual(len(lines), 8)
        self.assertTrue(
            any("phase=classify_headers state=complete" in line for line in lines)
        )
        self.assertTrue(
            any(
                "phase=llvm_queries state=complete processed=1 total=1" in line
                for line in lines
            )
        )
        self.assertTrue(any("direct_n_oso_object_groups=1" in x for x in lines))
        self.assertFalse(any(str(self.root) in line for line in lines))

    def test_progress_logging_counts_n_oso_rejected_archive_groups(self) -> None:
        candidate = self.deps / "candidate.rlib"
        candidate.write_bytes(self.archive.read_bytes())

        def query(argv: list[str]) -> str:
            return (
                "member.o\n" if argv[-2] == "-t" else _symtab(f"{candidate}(member.o)")
            )

        logger = logging.getLogger(debug_ownership.PROGRESS_LOGGER_NAME)
        previous_level = logger.level
        logger.setLevel(logging.INFO)
        self.addCleanup(logger.setLevel, previous_level)
        output = io.StringIO()
        handler = logging.StreamHandler(output)
        logger.addHandler(handler)
        self.addCleanup(logger.removeHandler, handler)
        self.archive_proof(query).capture_archives(
            self.census(), (), (frozenset({candidate}),)
        )

        self.assertIn("n_oso_archive_groups=1", output.getvalue())

    def test_capture_rejects_replaying_an_existing_consumer_snapshot(self) -> None:
        proof = self.proof()
        self.capture(proof)
        with self.assertRaisesRegex(RuntimeError, "snapshot cannot be replayed"):
            self.capture(proof)

    def test_capture_reads_closed_inode_once_and_preserves_every_alias(self) -> None:
        alias = self.deps / "libconsumer-alias.dylib"
        os.link(self.endpoint, alias)
        external_archive = self.root / "libexternal.a"
        external_archive.write_bytes(b"!<arch>\nmember")
        queries: list[Path] = []

        def query(argv: list[str]) -> str:
            queries.append(Path(argv[-1]))
            return _symtab(f"{external_archive}(member.o)")

        files = self.census()
        expected_reads = len({(s.st_dev, s.st_ino) for _, s in files}) + 1
        proof = self.proof(query)
        with patch.object(
            debug_ownership, "_header", wraps=debug_ownership._header
        ) as read:
            self.assertEqual(
                self.capture(proof), frozenset({self.object, self.protected})
            )
        self.assertEqual(read.call_count, expected_reads)
        self.assertTrue(all(call.args[1] == 16 for call in read.call_args_list))
        self.assertEqual(set(queries), {alias, self.endpoint})
        self.assertIsNotNone(proof.snapshot)
        if proof.snapshot is None:
            self.fail("finite proof did not activate")
        retained = dict(proof.snapshot.retained)
        self.assertTrue({alias, self.endpoint, external_archive}.issubset(retained))
        alias.write_bytes(_macho(2) + b"changed")
        with self.assertRaisesRegex(RuntimeError, "retained input"):
            proof.verify()

    def test_capture_rejects_same_inode_drift_before_alias_reuse(self) -> None:
        first = self.deps / "aaa-alias"
        second = self.deps / "aab-alias"
        first.write_bytes(b"not native")
        os.link(first, second)
        classify = debug_ownership._macho_kind

        def mutate(header: bytes) -> int | None:
            result = classify(header)
            first.write_bytes(b"new consumer contents")
            return result

        with (
            patch.object(debug_ownership, "_macho_kind", side_effect=mutate),
            self.assertRaisesRegex(RuntimeError, "input changed while reading"),
        ):
            self.capture(self.proof())

    def test_capture_does_not_share_inode_headers_for_unclosed_external_aliases(
        self,
    ) -> None:
        external = self.root / "external-consumer"
        os.link(self.endpoint, external)
        with patch.object(
            debug_ownership, "_header", wraps=debug_ownership._header
        ) as read:
            self.capture(self.proof(), (external,))
        endpoint_reads = [
            call.args[0]
            for call in read.call_args_list
            if call.args[0] in {self.endpoint, external}
        ]
        self.assertEqual(set(endpoint_reads), {self.endpoint, external})
        self.assertEqual(len(endpoint_reads), 2)

    def test_snapshot_allows_admitted_deletion_but_rejects_reappearance(self) -> None:
        proof = self.proof()
        self.capture(proof)
        proof.validate_selection(frozenset({self.object}))
        self.object.unlink()
        proof.record_owned_unlink(self.object)
        proof.verify()
        self.object.write_bytes(_macho(1))
        with self.assertRaisesRegex(RuntimeError, "reappeared"):
            proof.verify()

    def test_closed_alias_group_requires_exact_indexes_and_owned_unlink_records(
        self,
    ) -> None:
        objects, indexes, groups = self.closed_alias_group()
        proof = self.proof()
        self.assertEqual(
            self.capture(proof, candidates=objects, groups=groups), objects
        )
        expected_aliases: dict[tuple[int, int], set[Path]] = {}
        for path, stamp in proof.expected.items():
            expected_aliases.setdefault(stamp[:2], set()).add(path)
        self.assertEqual(
            proof.expected_aliases,
            {inode: frozenset(paths) for inode, paths in expected_aliases.items()},
        )
        with self.assertRaisesRegex(RuntimeError, "complete object/index groups"):
            proof.validate_selection(objects)
        with self.assertRaisesRegex(RuntimeError, "complete object/index groups"):
            proof.validate_selection(frozenset({self.group_object}) | indexes)
        with self.assertRaisesRegex(
            RuntimeError, "object-only selection and its exact indexes"
        ):
            proof.validate_selection(objects | indexes | {self.work_product})
        selected = objects | indexes
        proof.validate_selection(selected)
        for path in (self.group_object, next(iter(objects - {self.group_object}))):
            path.unlink()
            remaining_aliases = [
                alias for alias in objects if alias != path and alias.exists()
            ]
            transition = (
                file_stamp(remaining_aliases[0].lstat()) if remaining_aliases else None
            )
            proof.record_owned_unlink(path, transition)
            proof.verify()
        for path in indexes:
            path.unlink()
            proof.record_owned_unlink(path)
            proof.verify()

    def test_inode_alias_index_is_immutable_and_rejects_stale_expected_alias(
        self,
    ) -> None:
        proof, objects, _indexes, alias = self.selected_alias_group()
        inode = proof.expected[self.group_object][:2]
        self.assertEqual(proof.expected_aliases[inode], objects)
        with self.assertRaises(TypeError):
            cast(
                MutableMapping[tuple[int, int], frozenset[Path]], proof.expected_aliases
            )[inode] = frozenset()

        self.group_object.unlink()
        alias_stamp = proof.expected[alias]
        proof.expected[alias] = (alias_stamp[0], alias_stamp[1] + 1, *alias_stamp[2:])
        with self.assertRaisesRegex(
            RuntimeError, "alias index differs from expected ownership"
        ):
            proof.record_owned_unlink(self.group_object, file_stamp(alias.lstat()))

    def test_inode_alias_index_keeps_absent_paths_across_valid_transitions(
        self,
    ) -> None:
        proof, objects, _indexes, alias = self.selected_alias_group()
        inode = proof.expected[self.group_object][:2]
        original_aliases = proof.expected_aliases
        expected_paths = frozenset(proof.expected)
        self.record_first_owned_unlink(proof, alias)
        self.assertEqual(original_aliases[inode], objects)
        self.assertEqual(proof.expected_aliases, original_aliases)
        self.assertEqual(frozenset(proof.expected), expected_paths)
        self.assertIn(self.group_object, proof.absent)

        alias.unlink()
        proof.record_owned_unlink(alias)
        proof.verify()
        self.assertEqual(proof.expected_aliases, original_aliases)
        self.assertTrue(objects.issubset(proof.absent))

    def test_absent_parent_index_keeps_same_names_separate_across_sessions(
        self,
    ) -> None:
        first_objects, first_indexes, first_groups = self.closed_alias_group()
        second_objects, second_indexes, second_groups = self.closed_alias_group()
        proof = self.proof()
        objects = first_objects | second_objects
        indexes = first_indexes | second_indexes
        groups = first_groups + second_groups
        self.assertEqual(
            self.capture(proof, candidates=objects, groups=groups), objects
        )
        proof.validate_selection(objects | indexes)
        object_paths = [
            next(path for path in object_group if path.name == "libconsumer.foo.o")
            for object_group, _ in groups
        ]
        self.assertEqual(len(object_paths), 2)
        self.assertEqual(object_paths[0].name, object_paths[1].name)
        self.assertNotEqual(object_paths[0].parent, object_paths[1].parent)

        def verify_linear_absent_parent_checks() -> None:
            parent_lookups = 0
            previous_profile = sys.getprofile()

            def count_absent_parents(frame: FrameType, event: str, arg: object) -> None:
                nonlocal parent_lookups
                if (
                    event == "call"
                    and frame.f_code.co_name == "parent"
                    and frame.f_locals.get("self") in proof.absent
                ):
                    parent_lookups += 1

            sys.setprofile(count_absent_parents)
            try:
                proof.verify()
            finally:
                sys.setprofile(previous_profile)
            self.assertLessEqual(parent_lookups, len(proof.absent))

        verify_linear_absent_parent_checks()
        for object_path in object_paths:
            alias = next(
                path
                for path in objects
                if path.parent == object_path.parent and path != object_path
            )
            object_path.unlink()
            proof.record_owned_unlink(object_path, file_stamp(alias.lstat()))
            verify_linear_absent_parent_checks()

        unexpected = object_paths[0].parent / "unexpected-entry"
        unexpected.write_bytes(b"namespace drift")
        with self.assertRaisesRegex(RuntimeError, "namespace changed"):
            proof.verify()

    def test_capture_rejects_open_or_nonidentical_object_groups_and_missing_index(
        self,
    ) -> None:
        objects, indexes, groups = self.closed_alias_group()
        external_alias = self.root / "external-alias.o"
        os.link(self.group_object, external_alias)
        with self.assertRaisesRegex(RuntimeError, "alias closure"):
            self.capture(self.proof(), candidates=objects, groups=groups)

        external_alias.unlink()
        proof = self.proof()
        mismatched = ((frozenset({self.group_object, self.endpoint}), indexes),)
        with self.assertRaisesRegex(RuntimeError, "identity differs"):
            self.capture(proof, candidates=objects | {self.endpoint}, groups=mismatched)

        proof = self.proof()
        missing_index = ((objects, frozenset({next(iter(indexes))})),)
        with self.assertRaisesRegex(RuntimeError, "index closure differs"):
            self.capture(proof, candidates=objects, groups=missing_index)

    def test_capture_rejects_protected_or_directly_consumed_alias_as_whole_group(
        self,
    ) -> None:
        objects, indexes, groups = self.closed_alias_group()
        alias = next(iter(objects - {self.group_object}))
        proof = self.proof()
        self.assertEqual(
            self.capture(proof, references=(alias,), candidates=objects, groups=groups),
            frozenset(),
        )
        self.assertIsNone(proof.snapshot)

        for consumed in objects:

            def query(_: list[str], consumed: Path = consumed) -> str:
                return _symtab(str(consumed))

            proof = self.proof(query)
            with self.subTest(consumed=consumed):
                self.assertEqual(
                    self.capture(proof, candidates=objects, groups=groups), frozenset()
                )
                self.assertIsNone(proof.snapshot)

    def test_owned_unlink_rejects_unselected_or_unrecorded_absence(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        with self.assertRaisesRegex(RuntimeError, "outside exact selected ownership"):
            proof.record_owned_unlink(self.work_product)

        self.group_object.unlink()
        with self.assertRaisesRegex(
            RuntimeError, "admitted object changed or reappeared"
        ):
            proof.verify()
        proof.record_owned_unlink(self.group_object, file_stamp(alias.lstat()))
        proof.verify()

    def test_owned_unlink_detects_external_link_drift(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.record_first_owned_unlink(proof, alias)
        external = self.root / "external-after-owned-unlink.o"
        os.link(alias, external)
        with self.assertRaisesRegex(
            RuntimeError, "retained input changed|admitted object changed"
        ):
            proof.verify()
        external.unlink()

    def test_owned_unlink_detects_ctime_drift_after_record(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.record_first_owned_unlink(proof, alias)
        external = self.root / "temporary-ctime-alias.o"
        os.link(alias, external)
        external.unlink()
        with self.assertRaisesRegex(
            RuntimeError, "admitted object changed or reappeared"
        ):
            proof.verify()

    def test_owned_unlink_rejects_transition_ctime_drift_before_record(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.group_object.unlink()
        transition = file_stamp(alias.lstat())
        external = self.root / "transition-race-alias.o"
        os.link(alias, external)
        external.unlink()
        with self.assertRaisesRegex(RuntimeError, "alias changed after owned unlink"):
            proof.record_owned_unlink(self.group_object, transition)

    def test_owned_unlink_detects_replacement_drift(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.record_first_owned_unlink(proof, alias)
        alias.unlink()
        alias.write_bytes(_macho(1))
        with self.assertRaisesRegex(
            RuntimeError, "admitted object changed or reappeared"
        ):
            proof.verify()

    def test_owned_unlink_detects_tool_drift(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.record_first_owned_unlink(proof, alias)
        self.tool.write_bytes(b"changed tool")
        with self.assertRaisesRegex(RuntimeError, "LLVM tool changed"):
            proof.verify()

    def test_owned_unlink_detects_source_pin_drift(self) -> None:
        proof, _objects, _indexes, alias = self.selected_alias_group()
        self.record_first_owned_unlink(proof, alias)
        proof.input_check = lambda: (_ for _ in ()).throw(
            RuntimeError("source pin changed")
        )
        with self.assertRaisesRegex(RuntimeError, "source pin changed"):
            proof.verify()

    def test_snapshot_pins_endpoint_archive_incremental_and_namespace(self) -> None:
        proof = self.proof()
        self.capture(proof, (self.endpoint, self.archive, self.work_product))
        for path in (self.endpoint, self.archive, self.work_product):
            original = path.read_bytes()
            path.write_bytes(original + b"changed")
            with (
                self.subTest(path=path),
                self.assertRaisesRegex(RuntimeError, "retained input"),
            ):
                proof.verify()
            path.write_bytes(original)
        proof = self.proof()
        self.capture(proof, (self.endpoint, self.archive, self.work_product))
        (self.deps / "new.o").write_bytes(_macho(1))
        with self.assertRaisesRegex(RuntimeError, "namespace"):
            proof.verify()

    def test_snapshot_rejects_symlink_and_hardlink_changes_to_retained_input(
        self,
    ) -> None:
        proof = self.proof()
        self.capture(proof, (self.endpoint,))
        replacement = self.root / "replacement"
        replacement.write_bytes(self.endpoint.read_bytes())
        self.endpoint.unlink()
        self.endpoint.symlink_to(replacement)
        with self.assertRaisesRegex(RuntimeError, "retained input"):
            proof.verify()

    def test_capture_rejects_query_error(self) -> None:
        proof = self.proof(
            lambda _: (_ for _ in ()).throw(RuntimeError("LLVM query failed"))
        )
        with self.assertRaisesRegex(RuntimeError, "LLVM query failed"):
            self.capture(proof, (self.endpoint,))

    def test_factory_requires_darwin_and_both_llvm_tools(self) -> None:
        for platform, available, expected in (
            ("linux", True, None),
            ("darwin", False, None),
        ):
            with (
                self.subTest(platform=platform, available=available),
                patch(
                    "cargo_capacity_guard_debug_object_ownership.sys.platform", platform
                ),
                patch(
                    "cargo_capacity_guard_debug_object_ownership.shutil.which",
                    return_value="/usr/bin/tool" if available else None,
                ),
            ):
                self.assertIs(
                    create_debug_object_ownership(
                        self.target, lambda _: str(self.tool), lambda: None
                    ),
                    expected,
                )

    def test_factory_resolves_archive_reader_only_when_enabled(self) -> None:
        calls: list[list[str]] = []

        def query(argv: list[str]) -> str:
            calls.append(argv)
            return str(self.archive_tool if argv[-1] == "ar" else self.tool)

        with (
            patch.object(sys, "platform", "darwin"),
            patch.object(shutil, "which", return_value="/usr/bin/tool"),
        ):
            ordinary = create_debug_object_ownership(self.target, query, lambda: None)
            self.assertIsNotNone(ordinary)
            if ordinary is None:
                self.fail("ordinary LLVM proof was unavailable")
            self.assertIsNone(ordinary.archive_tool)
            self.assertFalse(any(argv[-1] == "ar" for argv in calls))

            calls.clear()
            archive_enabled = create_debug_object_ownership(
                self.target, query, lambda: None, archives=True
            )
            self.assertIsNotNone(archive_enabled)
            if archive_enabled is None:
                self.fail("archive LLVM proof was unavailable")
            self.assertEqual(archive_enabled.archive_tool, self.archive_tool_pin)
            self.assertTrue(any(argv[-1] == "ar" for argv in calls))

    def test_non_native_file_and_external_object_changes_are_pinned(self) -> None:
        unknown = self.deps / "unknown"
        unknown.write_bytes(b"not native")
        external = self.root / "external.o"
        external.write_bytes(_macho(1))
        proof = self.proof()
        self.capture(proof, (external,))
        unknown.write_bytes(_macho(2))
        with self.assertRaisesRegex(RuntimeError, "retained input"):
            proof.verify()
        proof = self.proof()
        self.capture(proof, (external,))
        external.write_bytes(_macho(1) + b"changed")
        with self.assertRaisesRegex(RuntimeError, "retained input"):
            proof.verify()

    def test_mixed_index_selection_rejects_before_any_unlink(self) -> None:
        proof = self.proof()
        self.capture(proof)
        with self.assertRaisesRegex(RuntimeError, "object-only selection"):
            proof.validate_selection(frozenset({self.object, self.work_product}))
        self.assertTrue(self.object.exists())
        self.assertTrue(self.work_product.exists())

    def test_external_thin_archive_and_dangling_alias_fail_closed(self) -> None:
        thin = self.root / "external.a"
        thin.write_bytes(b"!<thin>\n")
        proof = self.proof(lambda _: _symtab(str(thin) + "(member.o)"))
        with self.assertRaisesRegex(RuntimeError, "not a full archive"):
            self.capture(proof)
        alias = self.root / "alias.o"
        alias.symlink_to(self.root / "missing.o")
        with self.assertRaisesRegex(RuntimeError, "untracked alias"):
            debug_object_paths(_symtab(str(alias)))

    def test_insufficient_objects_do_not_activate_or_admit_partial_cleanup(
        self,
    ) -> None:
        proof = self.proof()
        with patch.object(
            debug_ownership,
            "_header",
            side_effect=AssertionError("unneeded header read"),
        ):
            result = proof.capture(
                self.census(),
                (),
                frozenset({self.object}),
                needed_bytes=self.object.stat().st_blocks * 512 + 1,
            )
        self.assertEqual(result, frozenset())
        self.assertIsNone(proof.snapshot)
        proof.validate_selection(frozenset({self.work_product}))
        self.assertTrue(self.object.exists())

    def test_missing_target_archive_and_literal_are_pinned_against_reappearance(
        self,
    ) -> None:
        archive = self.deps / "missing.a"
        literal = self.deps / "missing.a(member.o)"
        for revived in (archive, literal):
            proof = self.proof(lambda _: _symtab(str(literal)))
            self.capture(proof)
            self.assertIsNotNone(proof.snapshot)
            if proof.snapshot is None:
                self.fail("finite proof did not activate")
            self.assertTrue({archive, literal}.issubset(proof.snapshot.missing))
            revived.write_bytes(_macho(1))
            with self.assertRaisesRegex(
                RuntimeError, "previously missing input appeared"
            ):
                proof.verify()
            revived.unlink()

    def test_missing_archive_rejects_existing_or_symlink_literal_fallback(self) -> None:
        literal = self.deps / "missing.a(member.o)"
        literal.write_bytes(_macho(1))
        with self.assertRaisesRegex(RuntimeError, "literal fallback exists"):
            self.capture(self.proof(lambda _: _symtab(str(literal))))
        literal.unlink()
        literal.symlink_to(self.object)
        with self.assertRaises(RuntimeError):
            self.capture(self.proof(lambda _: _symtab(str(literal))))

    def test_missing_external_archive_and_permission_errors_fail_closed(self) -> None:
        literal = self.root / "missing.a(member.o)"
        with self.assertRaisesRegex(RuntimeError, "missing archive is outside target"):
            self.capture(self.proof(lambda _: _symtab(str(literal))))
        archive = self.deps / "missing.a"
        literal = self.deps / "missing.a(member.o)"
        lstat = Path.lstat

        def denied(path: Path) -> os.stat_result:
            if path == archive:
                raise PermissionError("archive unreadable")
            return lstat(path)

        with patch.object(Path, "lstat", denied), self.assertRaises(PermissionError):
            self.capture(self.proof(lambda _: _symtab(str(literal))))

    def test_missing_archive_noncanonical_literal_fallback_fails_closed(self) -> None:
        literal = str(self.deps / "missing.a") + "(member/../../outside.o)"
        with self.assertRaisesRegex(RuntimeError, "literal is not canonical"):
            self.capture(self.proof(lambda _: _symtab(literal)))


@unittest.skipUnless(
    sys.platform == "darwin" and shutil.which("cargo-sweep") and shutil.which("xcrun"),
    "actual LLVM finite transaction requires macOS developer tools",
)
class ActualLLVMFiniteTransactionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.target = self.root / "target"
        self.profile = self.target / "debug"
        self.cargo = Path(self.run_tool(["rustup", "which", "cargo"]).strip()).resolve()
        environment = dict(os.environ)
        environment.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
        for name in (
            "CARGO_BUILD_TARGET",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_INCREMENTAL",
        ):
            environment.pop(name, None)
        environment.update(
            {
                "PATH": str(self.cargo.parent) + os.pathsep + environment["PATH"],
                "RUSTFLAGS": "-D warnings",
                "CARGO_TARGET_DIR": str(self.target),
                "CARGO": "rtk cargo",
            }
        )
        self.enterContext(patch.dict(os.environ, environment, clear=True))
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers=["crates/katana-render-runtime",'
            '"crates/kdr-linter"]\nresolver="3"\n[profile.dev]\nsplit-debuginfo="unpacked"\n'
        )
        for name in ("katana-render-runtime", "kdr-linter"):
            crate = self.root / "crates" / name
            (crate / "src").mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                f'[package]\nname="{name}"\nversion="0.0.0"\nedition="2021"\n'
            )
            (crate / "src/lib.rs").write_text(
                '#[no_mangle]\npub extern "C" fn fixture_export() -> i32 {42}\n'
            )
        binary = self.root / "crates/kdr-linter/src/bin"
        binary.mkdir()
        (binary / "mathjax_consumer_facts.rs").write_text("fn main() {}\n")
        self.run_tool([str(self.cargo), "build", "--workspace", "--offline"])
        archive = next((self.profile / "deps").glob("libkatana_render_runtime-*.rlib"))
        original = next((self.profile / "deps").glob("katana_render_runtime-*.rcgu.o"))
        self.object = original.with_name(
            original.name.removesuffix(".rcgu.o") + ".retired.rcgu.o"
        )
        shutil.copyfile(original, self.object)
        source = self.root / "main.c"
        source.write_text(
            "extern int fixture_export(void); int main(void) {return fixture_export();}\n"
        )
        self.run_tool(
            ["xcrun", "clang", "-g", "-c", str(source), "-o", str(self.root / "main.o")]
        )
        self.endpoint = self.profile / "native_consumer"
        self.run_tool(
            [
                "xcrun",
                "clang",
                "-g",
                str(self.root / "main.o"),
                str(archive),
                "-o",
                str(self.endpoint),
            ]
        )
        metadata: object = json.loads(
            self.run_tool(
                [
                    str(self.cargo),
                    "metadata",
                    "--offline",
                    "--locked",
                    "--format-version",
                    "1",
                ]
            )
        )
        if not isinstance(metadata, dict):
            raise RuntimeError("fixture metadata is not an object")
        (self.root / "logs").mkdir()
        self.proof = create_library_alias_ownership(
            self.root, self.target, metadata, str(self.cargo), self.root / "logs"
        )

    def run_tool(self, argv: list[str]) -> str:
        return subprocess.run(
            argv, cwd=self.root, check=True, capture_output=True, text=True
        ).stdout

    def fixture_object_index_groups(
        self, files: Sequence[tuple[Path, os.stat_result]]
    ) -> tuple[frozenset[Path], ...]:
        flags = self.proof.different_flags
        if flags is None:
            return ()
        inodes: dict[tuple[int, int], set[Path]] = {}
        values: dict[Path, os.stat_result] = {}
        for path, value in files:
            inodes.setdefault((value.st_dev, value.st_ino), set()).add(path)
            values[path] = value
        groups = []
        for _identity, aliases in inodes.items():
            if len(aliases) != values[next(iter(aliases))].st_nlink:
                continue
            anchor = next(
                (path for path in aliases if path in flags.standalone_objects), None
            )
            objects = {
                path
                for path in aliases
                if path in flags.cargo_selected
                and path.suffix == ".o"
                and path.parent.parent.parent.name == "incremental"
            }
            if anchor is None or not objects:
                continue
            sessions = {path.parent for path in objects}
            indexes = {
                path
                for path in values
                if path.parent in sessions
                and path.name in {"dep-graph.bin", "query-cache.bin"}
            }
            if all(
                {path.name for path in indexes if path.parent == session}
                == {"dep-graph.bin", "query-cache.bin"}
                for session in sessions
            ):
                groups.append(frozenset(aliases | indexes))
        return tuple(groups)

    def test_actual_llvm_archive_map_and_finite_transaction_preserve_every_other_artifact(
        self,
    ) -> None:
        self.assertIsNotNone(self.proof.debug_objects)
        self.assertEqual(self.object.stat().st_nlink, 1)
        with ProfileLocks(self.target, self.proof.verify) as epoch:
            files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
            before = {path: path.read_bytes() for path, _ in files}
            fixture_groups = self.fixture_object_index_groups(files)
            fixture_paths = set().union(*fixture_groups) if fixture_groups else set()
            allocated = {(s.st_dev, s.st_ino): s.st_blocks * 512 for _, s in files}
            current = sum(allocated.values())
            freed = self.object.stat().st_blocks * 512
            code, report = evict_artifacts(
                self.target,
                current - freed,
                current,
                False,
                (self.endpoint,),
                epoch,
                retained_profiles=(self.profile,),
                selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof,
                selected_cache_ownership=self.proof.cache,
            )
            self.assertEqual(code, 0, report)
            selected = {self.target / path for path in report["candidate_paths"]}
            selected_names = [Path(path).name for path in report["candidate_paths"]]
            self.assertTrue(fixture_groups)
            self.assertTrue(any(group.issubset(selected) for group in fixture_groups))
            self.assertTrue(selected.issubset(fixture_paths), report["candidate_paths"])
            self.assertLess(
                selected_names.index("dep-graph.bin"),
                selected_names.index("query-cache.bin"),
            )
            self.assertLess(
                selected_names.index("query-cache.bin"),
                next(
                    index
                    for index, name in enumerate(selected_names)
                    if name.endswith(".o")
                ),
            )
            self.assertEqual(report["deleted_files"], len(selected))
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertTrue(all(not path.exists() for path in selected))
            self.assertTrue(self.object.exists())
            self.assertTrue(
                all(
                    path.read_bytes() == content
                    for path, content in before.items()
                    if path not in selected
                )
            )
            self.proof.verify()

    def test_actual_llvm_archive_consumer_rejects_linked_and_unlinks_unconsumed_rlib(
        self,
    ) -> None:
        archive = next((self.profile / "deps").glob("libkatana_render_runtime-*.rlib"))
        symtab = self.run_tool(["xcrun", "dsymutil", "--symtab", str(self.endpoint)])
        self.assertIn(str(archive) + "(", symtab)
        consumed_proof = create_debug_object_ownership(
            self.target, self.run_tool, self.proof.verify, archives=True
        )
        self.assertIsNotNone(consumed_proof)
        if consumed_proof is None:
            self.fail("actual archive consumer proof is unavailable")
        files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
        self.assertEqual(
            consumed_proof.capture_archives(
                files, (self.endpoint,), (frozenset({archive}),)
            ),
            frozenset(),
        )
        self.assertIsNone(consumed_proof.snapshot)

        unconsumed = self.profile / "deps" / "libfixture-unconsumed.rlib"
        shutil.copyfile(archive, unconsumed)
        unconsumed_proof = create_debug_object_ownership(
            self.target, self.run_tool, self.proof.verify, archives=True
        )
        self.assertIsNotNone(unconsumed_proof)
        if unconsumed_proof is None:
            self.fail("actual archive consumer proof is unavailable")
        files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
        group = frozenset({unconsumed})
        self.assertEqual(
            unconsumed_proof.capture_archives(files, (self.endpoint,), (group,)), group
        )
        snapshot = unconsumed_proof.snapshot
        self.assertIsNotNone(snapshot)
        if snapshot is None:
            self.fail("actual unconsumed archive snapshot did not activate")
        self.assertIn(archive, dict(snapshot.retained))
        self.assertIn(unconsumed, dict(snapshot.archives))
        self.assertTrue(
            any(reference[1] == archive for reference in snapshot.archive_references)
        )
        unconsumed_proof.validate_selection(group)
        unconsumed.unlink()
        unconsumed_proof.record_owned_unlink(unconsumed)
        unconsumed_proof.verify()
        self.assertIn(unconsumed, unconsumed_proof.absent)

    def test_actual_llvm_incremental_alias_group_uses_index_first_owned_purge(
        self,
    ) -> None:
        source = self.root / "crates/katana-render-runtime/src/lib.rs"
        source.write_text(
            '#[no_mangle]\npub extern "C" fn fixture_export() -> i32 {43}\n'
        )
        self.run_tool([str(self.cargo), "build", "--workspace", "--offline"])
        metadata: object = json.loads(
            self.run_tool(
                [
                    str(self.cargo),
                    "metadata",
                    "--offline",
                    "--locked",
                    "--format-version",
                    "1",
                ]
            )
        )
        if not isinstance(metadata, dict):
            self.fail("fixture metadata is not an object after the second Cargo build")
        self.proof = create_library_alias_ownership(
            self.root, self.target, metadata, str(self.cargo), self.root / "logs"
        )
        flags = self.proof.different_flags
        self.assertIsNotNone(flags)
        if flags is None:
            self.fail("fixture has no compiler-output ownership")
        anchors = {
            path: anchor
            for path, anchor in flags.standalone_objects.items()
            if anchor.crate == "katana_render_runtime" and path.exists()
        }
        incremental = [
            path
            for path in flags.cargo_selected
            if path.suffix == ".o"
            and path.parent.parent.parent.name == "incremental"
            and path.parent.parent.name.startswith("katana_render_runtime-")
        ]
        sessions = {path.parent for path in incremental}
        latest = max((session.name for session in sessions), default="")
        old_objects = [path for path in incremental if path.parent.name < latest]
        self.assertTrue(
            old_objects,
            "second Cargo build did not retain a finalized incremental object session",
        )
        alias_pairs = [
            (anchor, obj)
            for anchor in anchors
            for obj in old_objects
            if (anchor.stat().st_dev, anchor.stat().st_ino)
            == (obj.stat().st_dev, obj.stat().st_ino)
        ]
        self.assertTrue(
            alias_pairs,
            "fixture has no actual Cargo/LLVM deps-to-incremental hardlink group",
        )
        anchor_path, object_path = min(alias_pairs)
        self.assertGreaterEqual(object_path.stat().st_nlink, 2)
        os.utime(anchor_path, ns=(1, 1))

        with ProfileLocks(self.target, self.proof.verify) as epoch:
            files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
            before = {path: path.read_bytes() for path, _ in files}
            allocated = {
                (value.st_dev, value.st_ino): value.st_blocks * 512
                for _, value in files
            }
            current = sum(allocated.values())
            needed = anchor_path.stat().st_blocks * 512
            code, report = evict_artifacts(
                self.target,
                current - needed,
                current,
                False,
                (self.endpoint,),
                epoch,
                retained_profiles=(self.profile,),
                selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof,
                selected_cache_ownership=self.proof.cache,
            )
            self.assertEqual(code, 0, report)
            selected = [self.target / path for path in report["candidate_paths"]]
            selected_names = [path.name for path in selected]
            dep_graph = selected_names.index("dep-graph.bin")
            query_cache = selected_names.index("query-cache.bin")
            self.assertTrue(
                any(path.suffix == ".o" for path in selected), report["candidate_paths"]
            )
            first_object = next(
                index for index, path in enumerate(selected) if path.suffix == ".o"
            )
            self.assertLess(dep_graph, query_cache)
            self.assertLess(query_cache, first_object)
            self.assertIn(object_path, selected)
            self.assertIn(anchor_path, selected)
            self.assertGreaterEqual(report["deleted_files"], 4)
            self.assertTrue(all(not path.exists() for path in selected))
            for path, content in before.items():
                if path not in selected:
                    self.assertEqual(
                        path.read_bytes(), content, str(path.relative_to(self.target))
                    )
            self.assertTrue(self.endpoint.exists())
            self.proof.verify()
        source.write_text(
            '#[no_mangle]\npub extern "C" fn fixture_export() -> i32 {44}\n'
        )
        self.run_tool([str(self.cargo), "build", "--workspace", "--offline"])

    def test_insufficient_goal_still_proves_unconsumed_objects_and_preserves_consumers(
        self,
    ) -> None:
        object_before = self.object.read_bytes()
        with ProfileLocks(self.target, self.proof.verify) as epoch:
            files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
            preserved = {p: p.read_bytes() for p, _ in files}
            allocated = {
                (value.st_dev, value.st_ino): value.st_blocks * 512
                for _, value in files
            }
            current = sum(allocated.values())
            # The goal requires reclaiming nearly the whole target; the protected
            # consumer and archive make that goal unreachable even with every safe
            # object and index selected, exercising true partial cleanup.
            needed = current - 1
            code, report = evict_artifacts(
                self.target,
                current - needed,
                current,
                False,
                (self.endpoint,),
                epoch,
                retained_profiles=(self.profile,),
                selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof,
                selected_cache_ownership=self.proof.cache,
            )
            self.assertEqual(code, 0, report)
            self.assertLess(report["estimated_candidate_bytes"], needed)
            self.assertGreater(report["deleted_files"], 0)
            selected = {self.target / path for path in report["candidate_paths"]}
            self.assertTrue(selected)
            selected_objects = {path for path in selected if path.suffix == ".o"}
            self.assertTrue(selected_objects, report["candidate_paths"])
            self.assertTrue(
                all(
                    path.name in {"dep-graph.bin", "query-cache.bin"}
                    or path.suffix == ".o"
                    for path in selected
                )
            )
            self.assertTrue(
                all(
                    p.read_bytes() == content
                    for p, content in preserved.items()
                    if p not in selected
                )
            )
            self.assertIsNotNone(self.proof.debug_objects)
            debug = self.proof.debug_objects
            if debug is None or debug.snapshot is None:
                self.fail(
                    "debug consumer ownership proof did not run below the cleanup goal"
                )
            admitted_paths = {
                path for path, _ in (*debug.snapshot.objects, *debug.snapshot.indexes)
            }
            self.assertTrue(
                selected.issubset(admitted_paths), report["candidate_paths"]
            )
            proved_objects = {path for path, _ in debug.snapshot.objects}
            retained_consumers = {path for path, _ in debug.snapshot.retained}
            if self.object in selected:
                self.assertIn(self.object, proved_objects)
                self.assertNotIn(self.object, retained_consumers)
                self.assertIn(
                    str(self.object.relative_to(self.target)), report["candidate_paths"]
                )
                self.assertFalse(self.object.exists())
            else:
                self.assertEqual(self.object.read_bytes(), object_before)
            selected_groups = [
                frozenset(objects | indexes)
                for objects, indexes in debug.snapshot.groups
                if selected.intersection(objects)
            ]
            self.assertTrue(selected_groups)
            self.assertTrue(all(group.issubset(selected) for group in selected_groups))
            admitted_indexes = {path for path, _ in debug.snapshot.indexes}
            cache_only_indexes = {
                path
                for path, _ in files
                if path.name in {"dep-graph.bin", "query-cache.bin"}
                and path not in admitted_indexes
            }
            self.assertTrue(
                cache_only_indexes, "fixture must include cache-only sessions"
            )
            for path in cache_only_indexes:
                self.assertTrue(path.exists(), str(path.relative_to(self.target)))
                self.assertEqual(
                    path.read_bytes(),
                    preserved[path],
                    str(path.relative_to(self.target)),
                )
            archive = next(
                (self.profile / "deps").glob("libkatana_render_runtime-*.rlib")
            )
            self.assertTrue(selected_objects.issubset(proved_objects))
            self.assertIn(self.endpoint, retained_consumers)
            self.assertIn(archive, retained_consumers)
            self.assertNotIn(self.endpoint, selected)
            self.assertNotIn(archive, selected)
            self.assertTrue(all(not path.exists() for path in selected))
            self.proof.verify()

    def test_actual_llvm_missing_archive_pins_both_absences_during_finite_transaction(
        self,
    ) -> None:
        archive = self.profile / "retired-consumer.a"
        shutil.copyfile(
            next((self.profile / "deps").glob("libkatana_render_runtime-*.rlib")),
            archive,
        )
        endpoint = self.profile / "retired_consumer"
        self.run_tool(
            [
                "xcrun",
                "clang",
                "-g",
                str(self.root / "main.o"),
                str(archive),
                "-o",
                str(endpoint),
            ]
        )
        symtab = self.run_tool(["xcrun", "dsymutil", "--symtab", str(endpoint)])
        self.assertIn(str(archive) + "(", symtab)
        archive.unlink()
        with ProfileLocks(self.target, self.proof.verify) as epoch:
            files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
            preserved = {p: p.read_bytes() for p, _ in files}
            fixture_groups = self.fixture_object_index_groups(files)
            fixture_paths = set().union(*fixture_groups) if fixture_groups else set()
            current = sum(
                {(s.st_dev, s.st_ino): s.st_blocks * 512 for _, s in files}.values()
            )
            needed = self.object.stat().st_blocks * 512
            code, report = evict_artifacts(
                self.target,
                current - needed,
                current,
                False,
                (self.endpoint, endpoint),
                epoch,
                retained_profiles=(self.profile,),
                selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof,
                selected_cache_ownership=self.proof.cache,
            )
            self.assertEqual(code, 0)
            selected = {self.target / path for path in report["candidate_paths"]}
            self.assertTrue(fixture_groups)
            self.assertTrue(any(group.issubset(selected) for group in fixture_groups))
            self.assertTrue(selected.issubset(fixture_paths), report["candidate_paths"])
            self.assertEqual(report["deleted_files"], len(selected))
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertTrue(any(path.suffix == ".o" for path in selected))
            self.assertTrue(all(not path.exists() for path in selected))
            self.assertTrue(
                all(
                    p.read_bytes() == data
                    for p, data in preserved.items()
                    if p not in selected
                )
            )
            debug = self.proof.debug_objects
            self.assertIsNotNone(debug)
            if debug is None or debug.snapshot is None:
                self.fail("finite missing-archive proof did not activate")
            self.assertIn(archive, debug.snapshot.missing)
            self.assertTrue(
                any(
                    str(p).startswith(str(archive) + "(")
                    for p in debug.snapshot.missing
                )
            )


if __name__ == "__main__":
    unittest.main()
