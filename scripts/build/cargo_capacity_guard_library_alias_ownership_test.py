from __future__ import annotations

from dataclasses import replace
import json
import os
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from typing import cast
from unittest.mock import patch

from cargo_capacity_guard_debug_object_ownership import DebugObjectOwnership
from cargo_capacity_guard_different_flags_ownership import DifferentFlagsAnchor, DifferentFlagsOwnership
from cargo_capacity_guard_library_alias_ownership import (
    LibraryAliasGroup, LibraryAliasOwnership, _combined_verification, classifier_paths, create_library_alias_ownership,
    library_alias_groups,
)
from cargo_capacity_guard_lock import EvictionReport, ProfileLocks, _fingerprint_files, evict_artifacts
from cargo_capacity_guard_build_script_ownership import file_stamp, source_pin
from cargo_capacity_guard_selected_cache_ownership import SelectedCacheOwner, SelectedCacheOwnership
from cargo_capacity_guard_debug_object_ownership_test import _macho as debug_macho, _symtab as debug_symtab


class LibraryAliasTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        self.target = self.root / "target"
        self.profile = self.target / "debug"
        self.deps = self.profile / "deps"
        self.deps.mkdir(parents=True)
        (self.profile / ".fingerprint").mkdir()
        self.owner = self.profile / "incremental" / "library-123456789abcd"
        self.session = self.owner / ("s-0000000001-0000001-" + "a" * 25)
        self.session.mkdir(parents=True)
        self.lock = self.owner / "s-0000000001-0000001.lock"
        self.lock.write_bytes(b"")
        self.metadata = self.session / "metadata.rmeta"
        self.metadata.write_bytes(b"optional metadata")
        self.alias = self.deps / "liblibrary-1234567890abcdef.rmeta"
        os.link(self.metadata, self.alias)
        self.object = self.session / "output.o"
        self.object.write_bytes(b"object work product")
        for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"):
            (self.session / name).write_bytes(b"cache index")
        self.protected = self.profile / "current"
        self.protected.write_bytes(b"current compiled output")
        self.proof = LibraryAliasOwnership(self.root, self.target, frozenset({self.metadata}),
                                           frozenset({self.alias}), lambda: None)

    def census(self) -> list[tuple[Path, os.stat_result]]:
        return _fingerprint_files(self.target, self.target.stat().st_dev)[0]

    def groups(self, proof: LibraryAliasOwnership | None = None) -> int:
        return len(library_alias_groups(proof or self.proof, self.census(), set(), []))

    def base_object_group(self) -> tuple[LibraryAliasOwnership, frozenset[Path]]:
        self.object.write_bytes(debug_macho(1))
        anchor = self.deps / "library-1234567890abcdef.output.abc.rcgu.o"
        os.link(self.object, anchor)
        proof = replace(self.proof,
            cargo_selected=self.proof.cargo_selected | {self.object},
            sweep_selected=self.proof.sweep_selected | {anchor})
        return proof, frozenset({self.object, anchor})

    def old_flags_object_groups(self, count: int) -> tuple[LibraryAliasOwnership, tuple[frozenset[Path], ...]]:
        anchors: dict[Path, DifferentFlagsAnchor] = {}
        incremental_paths: set[Path] = set()
        object_groups: list[frozenset[Path]] = []
        for index in range(count):
            crate = f"oldflags{index}"
            digest = f"{index + 1:016x}"
            anchor = self.deps / f"{crate}-{digest}.0.rcgu.o"
            anchor.write_bytes(debug_macho(1))
            owner = self.profile / "incremental" / f"{crate}-{'a' * 13}"
            session = owner / f"s-0000000001-0000001-{'b' * 25}"
            session.mkdir(parents=True)
            incremental = session / "output.o"
            os.link(anchor, incremental)
            for name in ("dep-graph.bin", "query-cache.bin"):
                (session / name).write_bytes(b"fixture index")
            fingerprint = owner / "lib-oldflags.json"
            record = DifferentFlagsAnchor(fingerprint, crate, "object")
            anchors[anchor] = record
            incremental_paths.add(incremental)
            object_groups.append(frozenset({anchor, incremental}))
        different_flags = DifferentFlagsOwnership(self.target, anchors, frozenset(incremental_paths),
            lambda: None, anchors)
        return replace(self.proof, different_flags=different_flags), tuple(object_groups)

    def different_flags_library_group(self, role: str) -> tuple[LibraryAliasOwnership, Path]:
        crate = "oldflags" + role
        digest = "1" * 16
        anchor = self.deps / f"lib{crate}-{digest}.{role}"
        anchor.write_bytes(b"!<arch>\n" if role == "rlib" else debug_macho(6))
        fingerprint = self.owner / f"lib-{crate}.json"
        flags = DifferentFlagsOwnership(
            self.target, {anchor: DifferentFlagsAnchor(fingerprint, crate, role)}, frozenset(),
            lambda: None,
        )
        return replace(self.proof, different_flags=flags), anchor

    def archive_flag_fixture(
        self, records: tuple[tuple[str, str, str], ...],
    ) -> tuple[LibraryAliasOwnership, dict[Path, DifferentFlagsAnchor]]:
        anchors: dict[Path, DifferentFlagsAnchor] = {}
        for index, (suffix, role, crate) in enumerate(records):
            path = self.deps / f"lib{crate}-{index + 1:016x}.{suffix}"
            path.write_bytes(b"!<arch>\narchive" if suffix in {"rlib", "a"} else b"candidate")
            anchors[path] = DifferentFlagsAnchor(self.owner / f"lib-{crate}.json", crate, role)
        different = DifferentFlagsOwnership(self.target, anchors, frozenset(), lambda: None)
        return replace(self.proof, different_flags=different, archives_only=True), anchors

    def fake_archive_consumer(self, eligible: set[Path]) -> SimpleNamespace:
        return SimpleNamespace(
            snapshot=None,
            capture_archives=lambda _files, _references, groups, _needed: frozenset(eligible),
        )

    def with_debug_capture(self, proof: LibraryAliasOwnership,
                           consumed: tuple[Path, ...] = ()) -> tuple[LibraryAliasOwnership, DebugObjectOwnership]:
        tool = self.root / "dsymutil"
        tool.write_bytes(b"fixture tool")
        endpoint = self.deps / "libconsumer.dylib"
        endpoint.write_bytes(debug_macho(2))

        def query(_argv: list[str]) -> str:
            return debug_symtab(*(str(path) for path in consumed))

        capture = DebugObjectOwnership(self.target, source_pin(tool), query, proof.verify)
        return replace(proof, debug_objects=capture), capture

    @staticmethod
    def allocated_object_bytes(group: frozenset[Path]) -> int:
        path = next(iter(group))
        return path.stat().st_blocks * 512

    def metadata_groups(self, groups: tuple[LibraryAliasGroup, ...]) -> bool:
        return any({self.metadata, self.alias}.issubset({path for path, _ in group.files}) for group in groups)

    def factory_metadata_opt_in(self, *, metadata_only: bool, archives_only: bool) -> bool:
        from types import ModuleType
        from unittest.mock import Mock

        root = self.root / f"factory-{metadata_only}-{archives_only}"
        root.mkdir()
        target = root / "target"
        toolchain = root / ".rustup/toolchains/test/bin"
        toolchain.mkdir(parents=True)
        cargo = toolchain / "cargo"
        rustc = toolchain / "rustc"
        sweep = root / "cargo-sweep"
        for path in (cargo, rustc, sweep):
            path.write_text("tool")
        packages: list[dict[str, object]] = []
        for name in ("kdr-linter", "katana-render-runtime"):
            package_root = root / "crates" / name
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            source = package_root / "lib.rs"
            manifest.write_text("[package]")
            source.write_text("// fixture")
            package_id = f"path+file:///{name}#0.1.0"
            packages.append({
                "id": package_id, "name": name, "manifest_path": str(manifest),
                "targets": [{"src_path": str(source)}],
            })
        workspace: dict[str, object] = {
            "target_directory": str(target), "build_directory": str(target),
            "workspace_members": [package["id"] for package in packages], "packages": packages,
        }
        logs = root / "logs"
        logs.mkdir()
        parser_path = Path(__file__).resolve().parents[1] / "release/produce_html_performance_evidence.py"
        parser_module = ModuleType("produce_html_performance_evidence")
        flags_factory = Mock(return_value=DifferentFlagsOwnership(target, {}, frozenset(), lambda: None))

        def command(argv: list[str], **_kwargs: object) -> str:
            if argv[0] == str(sweep) and argv[1:] == ["--version"]:
                return "cargo-sweep 0.8.0\n"
            if argv[0] == str(sweep):
                return "\n".join((
                    f"[DEBUG] cleaning: {json.dumps(str(target))} with "
                    + ("remove_not_built_with" if "--toolchains" in argv else "remove_older_than"),
                    f"[DEBUG] cleaning: {json.dumps(str(target / 'debug'))} with remove_not_built_with_in_a_profile",
                    *([f"[INFO] Using specified installed toolchains: {json.dumps(['test'])}"]
                      if "--toolchains" in argv else []),
                ))
            if "metadata" in argv:
                return json.dumps(workspace)
            return ""

        parser_module.__dict__["_dispatch"] = lambda argv, _kwargs: command(argv)
        exec(compile("def command(argv, **kwargs):\n    return _dispatch(argv, kwargs)",
                     str(parser_path), "exec"), parser_module.__dict__)
        with (
            patch.dict(os.environ, {"RUSTFLAGS": "-D warnings"}, clear=False),
            patch("cargo_capacity_guard.selected_namespace_supported", return_value=True),
            patch("cargo_capacity_guard._selected_input_check", return_value=lambda: None),
            patch("cargo_capacity_guard_library_alias_ownership.shutil.which",
                  side_effect=lambda name: str(cargo if name == "cargo" else rustc if name == "rustc" else sweep)),
            patch.dict("sys.modules", {"produce_html_performance_evidence": parser_module}),
            patch("cargo_capacity_guard_library_alias_ownership.create_selected_cache_ownership",
                  return_value=SelectedCacheOwnership(root, target, target / "debug", (), lambda: None)),
            patch("cargo_capacity_guard_library_alias_ownership.create_different_flags_ownership", flags_factory),
            patch("cargo_capacity_guard_library_alias_ownership.create_debug_object_ownership"),
        ):
            create_library_alias_ownership(
                root, target, workspace, str(cargo), logs,
                metadata_only=metadata_only, archives_only=archives_only,
            )
        value = flags_factory.call_args.kwargs["include_standard_metadata"]
        if not isinstance(value, bool):
            raise AssertionError("standard metadata opt-in was not boolean")
        return value

    def test_factory_enables_standard_metadata_only_for_metadata_mode(self) -> None:
        modes = ((False, False, False), (False, True, False), (True, False, True))
        for metadata_only, archives_only, expected in modes:
            with self.subTest(metadata_only=metadata_only, archives_only=archives_only):
                self.assertEqual(
                    self.factory_metadata_opt_in(metadata_only=metadata_only, archives_only=archives_only),
                    expected,
                )

    @staticmethod
    def selected_object_paths(groups: tuple[LibraryAliasGroup, ...]) -> set[Path]:
        return {path for group in groups for path, _ in group.files if path.suffix == ".o"}

    def evict(self, dry_run: bool, references: tuple[Path, ...] = ()) -> EvictionReport:
        with ProfileLocks(self.target) as epoch:
            epoch.input_check = self.proof.verify
            _, result = evict_artifacts(
                self.target, 0, 1, dry_run, (self.protected, *references), epoch,
                retained_profiles=(self.profile,), selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof,
                selected_cache_ownership=self.proof.cache,
            )
            return result

    def test_positive_metadata_alias_has_no_fingerprint_requirement(self) -> None:
        self.assertFalse(self.proof.archives_only)
        self.assertEqual(self.groups(), 1)

    def test_combined_recheck_keeps_the_original_owner_and_propagates_failure(self) -> None:
        calls: list[str] = []
        cache = SelectedCacheOwnership(self.root, self.target, self.profile, (), lambda: calls.append("cache"))
        combined = _combined_verification(cache, lambda: calls.append("alias"))
        combined()
        self.assertEqual(calls, ["cache", "alias"])
        def fail() -> None:
            raise RuntimeError("original cache rejected")
        with self.assertRaisesRegex(RuntimeError, "original cache rejected"):
            _combined_verification(replace(cache, verify=fail), lambda: self.fail("alias must not hide rejection"))()
        with self.assertRaisesRegex(RuntimeError, "original cache rejected"):
            _combined_verification(cache, fail)()

    def test_compiler_configuration_rejects_before_classifier_queries(self) -> None:
        from cargo_capacity_guard import selected_namespace_supported

        directory = self.root / ".cargo"
        directory.mkdir()
        config = directory / "config.toml"
        variants = [f'[build]\n{key} = "unsupported"\n' for key in (
            "rustc", "rustc-wrapper", "rustc-workspace-wrapper", "rustflags")]
        variants.append('[target.host]\nrustflags = []\n')
        with patch.dict(os.environ, {"CARGO_HOME": str(self.root / "cargo-home")}, clear=True):
            for content in variants:
                with self.subTest(content=content):
                    config.write_text(content)
                    self.assertFalse(selected_namespace_supported(self.root))
                    with patch("cargo_capacity_guard._selected_input_check") as check:
                        with self.assertRaisesRegex(RuntimeError, "compiler overrides"):
                            create_library_alias_ownership(self.root, self.target,
                                {"target_directory": str(self.target)}, "unused", self.root / "logs")
                        check.assert_not_called()
            config.write_text('[build]\njobs = 1\n')
            self.assertTrue(selected_namespace_supported(self.root))

    def test_classifier_requires_canonical_existing_target_paths(self) -> None:
        self.assertEqual(classifier_paths(str(self.metadata), self.target, sweep=False),
                         frozenset({self.metadata}))
        for value in (str(self.root), "relative", str(self.target / "absent")):
            with self.subTest(value=value), self.assertRaises((RuntimeError, FileNotFoundError)):
                classifier_paths(value, self.target, sweep=False)

    def test_sweep_debug_json_paths_are_decoded_without_evaluation(self) -> None:
        raw = "[TRACE] other\n[DEBUG] Would remove: " + json.dumps(str(self.alias))
        self.assertEqual(classifier_paths(raw, self.target, sweep=True), frozenset({self.alias}))
        for raw in ('[INFO] Would remove: "unknown"', '[DEBUG] Would remove: 0'):
            with self.subTest(raw=raw), self.assertRaises(RuntimeError):
                classifier_paths(raw, self.target, sweep=True)

    def test_duplicate_and_symlink_classifier_paths_reject(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "repeats"):
            classifier_paths(str(self.metadata) + "\n" + str(self.metadata), self.target, sweep=False)
        link = self.deps / "link"
        link.symlink_to(self.metadata)
        with self.assertRaises(RuntimeError):
            classifier_paths(str(link), self.target, sweep=False)

    def test_either_classifier_missing_keeps_unknown_owner(self) -> None:
        for proof in (replace(self.proof, cargo_selected=frozenset()),
                      replace(self.proof, sweep_selected=frozenset())):
            with self.subTest(proof=proof):
                self.assertEqual(self.groups(proof), 0)

    def test_external_hardlink_prevents_alias_closure(self) -> None:
        os.link(self.metadata, self.root / "outside-target")
        self.assertEqual(self.groups(), 0)

    def test_unknown_alias_role_is_retained(self) -> None:
        os.link(self.metadata, self.profile / "unknown")
        self.assertEqual(self.groups(), 0)

    def test_incremental_only_inode_is_not_the_admitted_two_role_contract(self) -> None:
        self.alias.unlink()
        self.assertEqual(self.groups(), 0)

    def test_working_session_is_not_admitted_even_with_finalized_payload(self) -> None:
        working = self.session.with_name("s-0000000001-0000001-working")
        self.session.rename(working)
        proof = replace(self.proof, cargo_selected=frozenset({working / "metadata.rmeta"}))
        self.assertEqual(self.groups(proof), 0)

    def test_unselected_incremental_alias_is_not_inferred_from_prefix(self) -> None:
        extra = self.session / "alias.o"
        os.link(self.metadata, extra)
        self.assertEqual(self.groups(), 0)

    def test_protected_inode_and_directory_descendants_are_retained(self) -> None:
        files = self.census()
        value = self.metadata.stat()
        self.assertFalse(library_alias_groups(self.proof, files, {(value.st_dev, value.st_ino)}, []))
        self.assertFalse(library_alias_groups(self.proof, files, set(), [self.deps]))

    def test_unavailable_debug_capture_preserves_metadata_and_omits_base_objects(self) -> None:
        proof, _objects = self.base_object_group()
        groups = library_alias_groups(proof, self.census(), set(), [])
        self.assertTrue(self.metadata_groups(groups))
        self.assertFalse(self.selected_object_paths(groups))

    def test_unclassified_archive_alias_is_not_admitted_in_metadata_only_fallback(self) -> None:
        archive = self.session / "output.rlib"
        os.link(self.metadata, archive)
        proof = replace(self.proof, cargo_selected=self.proof.cargo_selected | {archive})
        self.assertFalse(library_alias_groups(proof, self.census(), set(), []))

    def test_metadata_only_excludes_different_flags_archives_and_dylibs_but_default_keeps_them(self) -> None:
        for role in ("rlib", "dylib"):
            with self.subTest(role=role):
                proof, anchor = self.different_flags_library_group(role)
                files = self.census()
                default_groups = library_alias_groups(proof, files, set(), [])
                self.assertTrue(any(anchor in {path for path, _ in group.files} for group in default_groups))
                metadata_groups = library_alias_groups(replace(proof, metadata_only=True), files, set(), [])
                self.assertTrue(self.metadata_groups(metadata_groups))
                self.assertFalse(any(anchor in {path for path, _ in group.files} for group in metadata_groups))

    def test_metadata_only_excludes_mixed_object_aliases(self) -> None:
        crate = "oldflagsobject"
        digest = "3" * 16
        anchor = self.deps / f"{crate}-{digest}.0.rcgu.o"
        anchor.write_bytes(b"object output")
        session_owner = self.profile / "incremental" / f"{crate}-{'c' * 13}"
        session = session_owner / ("s-0000000003-0000003-" + "d" * 25)
        session.mkdir(parents=True)
        output = session / "output.o"
        os.link(anchor, output)
        fingerprint = session_owner / f"lib-{crate}.json"
        flags = DifferentFlagsOwnership(
            self.target, {anchor: DifferentFlagsAnchor(fingerprint, crate, "object")},
            frozenset({output}), lambda: None,
        )
        proof = replace(
            self.proof, cargo_selected=self.proof.cargo_selected | {output},
            sweep_selected=self.proof.sweep_selected | {anchor}, different_flags=flags, metadata_only=True,
        )
        groups = library_alias_groups(proof, self.census(), set(), [])
        self.assertTrue(self.metadata_groups(groups))
        self.assertFalse(any({anchor, output} & {path for path, _ in group.files} for group in groups))

    def test_archives_only_requires_llvm_and_authoritative_rlib_role(self) -> None:
        proof, anchors = self.archive_flag_fixture((
            ("rlib", "rlib", "archive-good"), ("a", "rlib", "archive-a"),
            ("dylib", "dylib", "archive-dylib"), ("o", "object", "archive-object"),
            ("rmeta", "rmeta", "archive-metadata"),
        ))
        all_paths = set(anchors)
        unproved = library_alias_groups(proof, self.census(), set(), [])
        self.assertFalse(unproved)

        capture_calls: list[tuple[frozenset[Path], ...]] = []

        def capture(_files: object, _references: object, groups: tuple[frozenset[Path], ...], _needed: int) -> frozenset[Path]:
            capture_calls.append(groups)
            return frozenset(all_paths)

        proof = replace(proof, debug_objects=cast(DebugObjectOwnership,
            SimpleNamespace(snapshot=None, capture_archives=capture)))
        groups = library_alias_groups(proof, self.census(), set(), [])
        good = next(path for path in anchors if path.suffix == ".rlib")
        self.assertEqual(capture_calls, [(frozenset({good}),)])
        self.assertEqual({path for group in groups for path, _ in group.files}, {good})

    def test_archives_only_requires_complete_authoritative_alias_group(self) -> None:
        archive = self.deps / "libarchive-1111111111111111.rlib"
        archive.write_bytes(b"!<arch>\narchive")
        alias = self.deps / "libarchive-alias-1111111111111111.rlib"
        os.link(archive, alias)
        fingerprint = self.owner / "lib-archive.json"
        one_anchor = DifferentFlagsOwnership(
            self.target, {archive: DifferentFlagsAnchor(fingerprint, "archive", "rlib")},
            frozenset(), lambda: None,
        )
        capture_calls: list[tuple[frozenset[Path], ...]] = []

        def capture(_files: object, _references: object, groups: tuple[frozenset[Path], ...], _needed: int) -> frozenset[Path]:
            capture_calls.append(groups)
            return frozenset({archive, alias})

        proof = replace(self.proof, different_flags=one_anchor, archives_only=True,
                        debug_objects=cast(DebugObjectOwnership,
                            SimpleNamespace(snapshot=None, capture_archives=capture)))
        self.assertFalse(library_alias_groups(proof, self.census(), set(), []))
        self.assertFalse(capture_calls)

        both_anchors = DifferentFlagsOwnership(
            self.target,
            {path: DifferentFlagsAnchor(fingerprint, "archive", "rlib") for path in (archive, alias)},
            frozenset(), lambda: None,
        )
        partial_proof = replace(proof, different_flags=both_anchors)
        partial = SimpleNamespace(snapshot=None, capture_archives=lambda *_args: frozenset({archive}))
        self.assertFalse(library_alias_groups(replace(partial_proof, debug_objects=cast(DebugObjectOwnership, partial)),
                                              self.census(), set(), []))

        complete = SimpleNamespace(snapshot=None, capture_archives=lambda *_args: frozenset({archive, alias}))
        admitted = library_alias_groups(replace(partial_proof, debug_objects=cast(DebugObjectOwnership, complete)),
                                        self.census(), set(), [])
        self.assertEqual({path for group in admitted for path, _ in group.files}, {archive, alias})

    def test_archives_only_uses_snapshot_union_and_suppresses_generic_cache(self) -> None:
        archive = self.deps / "libarchive-2222222222222222.rlib"
        archive.write_bytes(b"!<arch>\narchive")
        alias = self.deps / "libarchive-alias-2222222222222222.rlib"
        os.link(archive, alias)
        fingerprint = self.owner / "lib-archive.json"
        flags = DifferentFlagsOwnership(
            self.target,
            {path: DifferentFlagsAnchor(fingerprint, "archive", "rlib") for path in (archive, alias)},
            frozenset(), lambda: None,
        )

        class SnapshotCapture:
            snapshot: SimpleNamespace | None = None

            def capture_archives(self, _files: object, _references: object,
                                 groups: tuple[frozenset[Path], ...], _needed: int) -> frozenset[Path]:
                paths = frozenset(path for group in groups for path in group)
                self.snapshot = SimpleNamespace(
                    objects=(), indexes=(),
                    archives=tuple((path, file_stamp(path.lstat())) for path in sorted(paths)),
                    archive_groups=groups,
                )
                return paths

            def validate_selection(self, paths: frozenset[Path]) -> None:
                if paths != frozenset({archive, alias}):
                    raise RuntimeError("archive group is incomplete")

        consumer = SnapshotCapture()
        source = self.root / "src.rs"
        source.write_text("archive consumer fixture")
        pin = source_pin(source)
        verification = self.proof.verify
        cache = SelectedCacheOwnership(
            self.root, self.target, self.profile,
            (SelectedCacheOwner("library-", "library", "unit-package", pin, pin),), verification,
        )
        self.proof = replace(
            self.proof, different_flags=flags, archives_only=True,
            debug_objects=cast(DebugObjectOwnership, consumer), cache=cache,
        )
        self.assertIs(self.proof.verify, cache.verify)
        self.assertIs(self.proof.cache, cache)
        result = self.evict(True)
        self.assertNotIn("error", result)
        selected = {self.target / path for path in result["candidate_paths"]}
        self.assertEqual(selected, {archive, alias})
        self.assertNotIn(self.metadata, selected)
        self.assertNotIn(self.session / "dep-graph.bin", selected)
        self.assertIsNotNone(consumer.snapshot)
        assert consumer.snapshot is not None
        self.assertEqual(consumer.snapshot.objects, ())
        self.assertEqual({path for path, _ in consumer.snapshot.archives}, {archive, alias})

    def test_metadata_only_keeps_paired_different_flags_rmeta(self) -> None:
        crate = "oldflagsmetadata"
        digest = "2" * 16
        anchor = self.deps / f"lib{crate}-{digest}.rmeta"
        anchor.write_bytes(b"metadata output")
        session_owner = self.profile / "incremental" / f"{crate}-{'a' * 13}"
        session = session_owner / ("s-0000000002-0000002-" + "c" * 25)
        session.mkdir(parents=True)
        metadata = session / "metadata.rmeta"
        os.link(anchor, metadata)
        fingerprint = session_owner / f"lib-{crate}.json"
        flags = DifferentFlagsOwnership(
            self.target, {anchor: DifferentFlagsAnchor(fingerprint, crate, "rmeta")}, frozenset({metadata}),
            lambda: None,
        )
        proof = replace(
            self.proof, cargo_selected=self.proof.cargo_selected | {metadata},
            sweep_selected=self.proof.sweep_selected | {anchor}, different_flags=flags, metadata_only=True,
        )
        groups = library_alias_groups(proof, self.census(), set(), [])
        self.assertTrue(any({anchor, metadata}.issubset({path for path, _ in group.files}) for group in groups))
        self.assertTrue(all(path.suffix == ".rmeta" for group in groups for path, _ in group.files))

    def test_insufficient_safe_objects_do_not_fall_back_to_old_flags_groups(self) -> None:
        proof, object_groups = self.old_flags_object_groups(1)
        proof, capture = self.with_debug_capture(proof)
        needed_bytes = self.allocated_object_bytes(object_groups[0]) + 1
        groups = library_alias_groups(proof, self.census(), set(), [], needed_bytes=needed_bytes)
        self.assertIsNone(capture.snapshot)
        self.assertTrue(self.metadata_groups(groups))
        self.assertFalse(self.selected_object_paths(groups))

    def test_consumed_old_flags_objects_and_insufficient_safe_capacity_do_not_fall_back(self) -> None:
        proof, object_groups = self.old_flags_object_groups(2)
        consumed = next(path for path in object_groups[0] if path.parent == self.deps)
        proof, capture = self.with_debug_capture(proof, (consumed,))
        needed_bytes = self.allocated_object_bytes(object_groups[1]) + 1
        groups = library_alias_groups(proof, self.census(), set(), [], needed_bytes=needed_bytes)
        self.assertIsNone(capture.snapshot)
        self.assertTrue(self.metadata_groups(groups))
        self.assertFalse(self.selected_object_paths(groups))

    def test_positive_debug_capture_keeps_safe_base_object_group(self) -> None:
        proof, objects = self.base_object_group()
        proof, capture = self.with_debug_capture(proof)
        groups = library_alias_groups(proof, self.census(), set(), [], needed_bytes=0)
        self.assertIsNotNone(capture.snapshot)
        self.assertEqual(self.selected_object_paths(groups), set(objects))

    def test_source_input_drift_is_not_hidden(self) -> None:
        def changed() -> None:
            raise RuntimeError("input drift")
        with self.assertRaisesRegex(RuntimeError, "input drift"):
            self.groups(replace(self.proof, verify=changed))

    def test_dryrun_counts_closed_inode_once_and_preserves_bytes(self) -> None:
        result = self.evict(True)
        self.assertNotIn("error", result)
        self.assertEqual(result["deleted_files"], 0)
        self.assertEqual(set(result["candidate_paths"]), {
            str(path.relative_to(self.target)) for path in (
                self.alias, self.metadata, self.session / "dep-graph.bin", self.session / "query-cache.bin",
            )
        })
        self.assertTrue(self.metadata.exists())

    def test_busy_session_lock_keeps_the_metadata_group(self) -> None:
        with patch("cargo_capacity_guard_lock._acquire_incremental_session_lock", return_value=None):
            self.assertIn("insufficient", self.evict(True).get("error", ""))

    def test_protected_session_work_product_or_object_keeps_all(self) -> None:
        for path in (self.object, self.session / "work-products.bin", self.alias):
            with self.subTest(path=path):
                self.assertIn("insufficient", self.evict(True, (path,)).get("error", ""))

    def test_directory_protection_blocks_current_shared_library_alias(self) -> None:
        self.assertIn("insufficient", self.evict(True, (self.deps,)).get("error", ""))

    def test_unknown_or_incomplete_session_schema_is_retained(self) -> None:
        (self.session / "unexpected").write_bytes(b"unknown")
        self.assertIn("insufficient", self.evict(True).get("error", ""))

    def test_epoch_and_owner_identity_are_mandatory(self) -> None:
        with ProfileLocks(self.target) as epoch:
            code, result = evict_artifacts(self.target, 0, 1, True, (self.protected,), epoch,
                retained_profiles=(self.profile,), selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof)
            self.assertEqual(code, 1)
            self.assertIn("bound", result.get("error", ""))

    def test_fixture_purge_invalidates_indexes_before_metadata_and_preserves_outputs(self) -> None:
        unlinked: list[str] = []
        original = os.unlink

        def observe(path: str, *, dir_fd: int | None = None) -> None:
            unlinked.append(path)
            original(path, dir_fd=dir_fd)

        with patch("cargo_capacity_guard_lock.os.unlink", side_effect=observe):
            self.assertNotIn("error", self.evict(False))
        self.assertLess(unlinked.index("dep-graph.bin"), unlinked.index("metadata.rmeta"))
        self.assertLess(unlinked.index("query-cache.bin"), unlinked.index(self.alias.name))
        self.assertTrue(self.object.exists())
        self.assertTrue((self.session / "work-products.bin").exists())
        self.assertTrue(self.protected.exists())
        self.assertTrue((self.profile / ".fingerprint").is_dir())

    def test_combined_transaction_adds_latest_optional_indices_without_duplicate_aliases(self) -> None:
        source = self.root / "lib.rs"
        source.write_bytes(b"unit graph source")
        cache = SelectedCacheOwnership(self.root, self.target, self.profile,
            (SelectedCacheOwner("library-", "library", "unit-package", source_pin(source), source_pin(source)),),
            self.proof.verify)
        self.proof = replace(self.proof, cache=cache)
        latest = self.owner / ("s-0000000002-0000002-" + "b" * 25)
        latest.mkdir()
        (self.owner / "s-0000000002-0000002.lock").write_bytes(b"")
        for name in ("metadata.rmeta", "work-products.bin", "saved.o"):
            (latest / name).write_bytes(b"preserved latest output")
        for name in ("dep-graph.bin", "query-cache.bin"):
            (latest / name).write_bytes(b"optional index" * 4096)
        with ProfileLocks(self.target) as epoch:
            epoch.input_check = self.proof.verify
            code, result = evict_artifacts(self.target, 0, 20000, True, (self.protected,), epoch,
                retained_profiles=(self.profile,), selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof, selected_cache_ownership=cache)
        self.assertEqual(code, 0, result.get("error"))
        paths = result["candidate_paths"]
        self.assertEqual(len(paths), len(set(paths)))
        self.assertIn(str((latest / "query-cache.bin").relative_to(self.target)), paths)
        self.assertNotIn(str((latest / "metadata.rmeta").relative_to(self.target)), paths)
        expected = {self.alias, self.metadata, self.session / "dep-graph.bin", self.session / "query-cache.bin",
                    latest / "dep-graph.bin", latest / "query-cache.bin"}
        inodes = {(p.stat().st_dev, p.stat().st_ino): p.stat().st_blocks * 512 for p in expected}
        self.assertEqual(result["estimated_candidate_bytes"], sum(inodes.values()))
        with ProfileLocks(self.target) as epoch:
            epoch.input_check = self.proof.verify
            code, result = evict_artifacts(self.target, 0, 20000, False, (self.protected,), epoch,
                retained_profiles=(self.profile,), selected_package_profiles=(self.profile,),
                library_alias_ownership=self.proof, selected_cache_ownership=cache)
        self.assertEqual(code, 0, result.get("error"))
        for name in ("metadata.rmeta", "work-products.bin", "saved.o"):
            self.assertTrue((latest / name).exists())
        self.assertFalse((latest / "query-cache.bin").exists())


if __name__ == "__main__":
    unittest.main()
