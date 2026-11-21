from __future__ import annotations

from dataclasses import replace
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from cargo_capacity_guard_different_flags_ownership import (
    DifferentFlagsOwnership, create_different_flags_ownership, different_flags_groups,
)
from cargo_capacity_guard_library_alias_ownership import LibraryAliasOwnership, _checked_sweep_paths
from cargo_capacity_guard_lock import ProfileLocks, _fingerprint_files, evict_artifacts


class DifferentFlagsOwnershipTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.target = self.root / "target"
        self.profile = self.target / "debug"
        self.deps = self.profile / "deps"
        self.deps.mkdir(parents=True)
        self.manifest = self.root / "Cargo.toml"
        self.source = self.root / "lib.rs"
        self.manifest.write_text("[package]\nname='demo'\nversion='0.1.0'\n")
        self.source.write_text("pub fn demo() {}\n")
        self.metadata: dict[str, object] = {"packages": [{"name": "demo", "manifest_path": str(self.manifest),
            "targets": [{"name": "demo", "kind": ["lib"], "src_path": str(self.source)}]}]}
        self.standard = self.fingerprint("1" * 16, ["-D", "warnings"])
        self.empty = self.fingerprint("2" * 16, [])
        self.archive = self.deps / ("libdemo-" + "2" * 16 + ".rlib")
        self.archive.write_bytes(b"reproducible library cache")
        self.protected = self.deps / ("libdemo-" + "1" * 16 + ".rlib")
        self.protected.write_bytes(b"standard required output")
        self.standard_dylib = self.deps / ("libdemo-" + "1" * 16 + ".dylib")
        self.standard_dylib.write_bytes(b"standard dynamic library")
        self.linked_metadata = self.deps / ("libdemo-" + "1" * 16 + ".rmeta")
        self.linked_metadata.write_bytes(b"metadata paired with a link output")
        self.standard_metadata_fingerprint = self.fingerprint("4" * 16, ["-D", "warnings"])
        self.standard_metadata = self.deps / ("libdemo-" + "4" * 16 + ".rmeta")
        self.standard_metadata.write_bytes(b"standard metadata output")
        self.session = self.profile / "incremental/demo-0000000000001" / ("s-0000000001-0000001-" + "a" * 25)
        self.session.mkdir(parents=True)
        (self.session.parent / "s-0000000001-0000001.lock").write_bytes(b"")
        self.metadata_cache = self.session / "metadata.rmeta"
        self.metadata_cache.write_bytes(b"optional metadata")
        self.metadata_alias = self.deps / ("libdemo-" + "2" * 16 + ".rmeta")
        os.link(self.metadata_cache, self.metadata_alias)
        for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"):
            (self.session / name).write_bytes(b"index")
        self.cargo = frozenset({self.standard, self.empty, self.metadata_cache, self.standard_metadata_fingerprint})
        self.sweep = frozenset({self.archive, self.metadata_alias, self.linked_metadata,
                                self.standard_metadata, self.standard_dylib})
        self.environment = patch.dict(os.environ, {"RUSTFLAGS": "-D warnings"})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def fingerprint(self, digest: str, flags: list[str], rustc: int = 7) -> Path:
        parent = self.profile / ".fingerprint" / ("demo-" + digest)
        parent.mkdir(parents=True)
        path = parent / "lib-demo.json"
        path.write_text(json.dumps({"rustflags": flags, "rustc": rustc, "compile_kind": 0}))
        return path

    def proof(self, cargo: frozenset[Path] | None = None, sweep: frozenset[Path] | None = None,
              *, include_standard_metadata: bool = False) -> DifferentFlagsOwnership:
        return create_different_flags_ownership(self.root, self.target, self.metadata,
            self.cargo if cargo is None else cargo, self.sweep if sweep is None else sweep,
            lambda: None, include_standard_metadata=include_standard_metadata)

    def groups(self, proof: DifferentFlagsOwnership | None = None,
               protected: set[tuple[int, int]] | None = None, references: tuple[Path, ...] = ()) -> int:
        files = _fingerprint_files(self.target, self.target.stat().st_dev)[0]
        return len(different_flags_groups(self.proof() if proof is None else proof, files,
                                         set() if protected is None else protected, references))

    def test_official_sweep_and_cargo_fingerprint_admit_only_empty_library_and_closed_metadata(self) -> None:
        self.assertEqual(self.groups(), 2)
        self.assertNotIn(self.protected, self.proof().anchors)
        self.assertEqual(self.groups(self.proof(cargo=frozenset({self.standard}))), 0)
        self.assertEqual(self.groups(self.proof(sweep=frozenset())), 0)
        self.assertEqual(self.groups(self.proof(cargo=self.cargo - {self.metadata_cache})), 1)

    def test_nonempty_flags_unknown_roles_and_binaries_are_retained(self) -> None:
        executable = self.deps / ("demo-" + "2" * 16)
        executable.write_bytes(b"keep current executable")
        unknown = self.deps / ("libdemo-" + "2" * 16 + ".unknown")
        unknown.write_bytes(b"unknown")
        proof = self.proof(sweep=self.sweep | {executable, unknown, self.protected})
        self.assertNotIn(executable, proof.anchors)
        self.assertNotIn(unknown, proof.anchors)
        self.assertNotIn(self.protected, proof.anchors)
        self.empty.write_text(json.dumps({"rustflags": ["-C", "opt-level=1"], "rustc": 7, "compile_kind": 0}))
        self.assertEqual(self.groups(), 0)

    def test_standard_metadata_is_opt_in_and_standard_link_outputs_remain_unowned(self) -> None:
        default = self.proof()
        self.assertNotIn(self.standard_metadata, default.anchors)
        enabled = self.proof(include_standard_metadata=True)
        self.assertIn(self.standard_metadata, enabled.anchors)
        self.assertEqual(enabled.anchors[self.standard_metadata].role, "rmeta")
        self.assertNotIn(self.protected, enabled.anchors)
        self.assertNotIn(self.linked_metadata, enabled.anchors)
        self.assertNotIn(self.standard_dylib, enabled.anchors)
        self.assertNotIn(self.standard_metadata, self.proof(
            sweep=self.sweep - {self.standard_metadata}, include_standard_metadata=True
        ).anchors)

        so_fingerprint = self.fingerprint("5" * 16, ["-D", "warnings"])
        so_metadata = self.deps / ("libdemo-" + "5" * 16 + ".rmeta")
        so_metadata.write_bytes(b"metadata paired with a Linux shared object")
        so_path = so_metadata.with_suffix(".so")
        so_path.write_bytes(b"Linux shared object")
        so_proof = self.proof(
            cargo=self.cargo | {so_fingerprint},
            sweep=self.sweep | {so_metadata, so_path},
            include_standard_metadata=True,
        )
        self.assertNotIn(so_metadata, so_proof.anchors)

        standard_object = self.deps / ("demo-" + "1" * 16 + ".old.rcgu.o")
        standard_object.write_bytes(b"standard object")
        nonstandard_fingerprint = self.fingerprint("3" * 16, ["-C", "opt-level=1"])
        nonstandard_metadata = self.deps / ("libdemo-" + "3" * 16 + ".rmeta")
        nonstandard_metadata.write_bytes(b"nonstandard metadata")
        enabled_with_other_roles = self.proof(
            cargo=self.cargo | {nonstandard_fingerprint},
            sweep=self.sweep | {self.protected, standard_object, nonstandard_metadata},
            include_standard_metadata=True,
        )
        self.assertIn(self.standard_metadata, enabled_with_other_roles.anchors)
        self.assertNotIn(self.protected, enabled_with_other_roles.anchors)
        self.assertNotIn(standard_object, enabled_with_other_roles.anchors)
        self.assertNotIn(nonstandard_metadata, enabled_with_other_roles.anchors)

        self.assertEqual(self.groups(enabled), 3)
        value = self.standard_metadata.stat()
        self.assertEqual(self.groups(enabled, protected={(value.st_dev, value.st_ino)}), 2)

        for suffix in (".rlib", ".dylib", ".so"):
            proof = self.proof(include_standard_metadata=True)
            counterpart = self.standard_metadata.with_suffix(suffix)
            counterpart.write_bytes(b"linked output appeared after proof capture")
            with self.subTest(suffix=suffix), self.assertRaisesRegex(
                RuntimeError, "standard metadata linked counterpart appeared"
            ):
                proof.verify()
            counterpart.unlink()

    def test_external_alias_protection_and_unknown_internal_alias_fail_closed(self) -> None:
        value = self.archive.stat()
        self.assertEqual(self.groups(protected={(value.st_dev, value.st_ino)}), 1)
        self.assertEqual(self.groups(references=(self.archive,)), 1)
        os.link(self.archive, self.root / "outside")
        self.assertEqual(self.groups(), 1)
        os.link(self.metadata_alias, self.profile / "unknown-alias")
        self.assertEqual(self.groups(), 0)

    def test_fingerprint_source_or_environment_change_rejects_captured_proof(self) -> None:
        for path in (self.empty, self.standard, self.standard_metadata_fingerprint, self.source, self.manifest):
            proof = self.proof(include_standard_metadata=path in (self.standard, self.standard_metadata_fingerprint))
            before = path.read_bytes()
            path.write_bytes(before + b" ")
            with self.subTest(path=path), self.assertRaisesRegex(RuntimeError, "fingerprint or source changed"):
                proof.verify()
            path.write_bytes(before)
        proof = self.proof()
        with patch.dict(os.environ, {"RUSTFLAGS": ""}), self.assertRaisesRegex(RuntimeError, "environment changed"):
            proof.verify()

    def test_compiler_ambiguity_or_different_current_compiler_is_not_admitted(self) -> None:
        extra = self.fingerprint("3" * 16, ["-D", "warnings"], 8)
        with self.assertRaisesRegex(RuntimeError, "compiler identity is ambiguous"):
            self.proof(cargo=self.cargo | {extra})
        self.empty.write_text(json.dumps({"rustflags": [], "rustc": 9, "compile_kind": 0}))
        self.assertEqual(self.groups(), 0)

    def test_object_role_requires_a_positive_sweep_anchor_and_macro_dylib_requires_metadata_kind(self) -> None:
        object_path = self.deps / ("demo-" + "2" * 16 + ".work.rcgu.o")
        object_path.write_bytes(b"optional compiler object")
        codegen = self.deps / ("demo-" + "2" * 16 + ".demo." + "3" * 16 + "-cgu.00.rcgu.o")
        codegen.write_bytes(b"optional nonincremental compiler object")
        dylib = self.deps / ("libdemo-" + "2" * 16 + ".dylib")
        dylib.write_bytes(b"optional compiler macro")
        proof = self.proof(sweep=self.sweep | {object_path, codegen, dylib})
        self.assertIn(object_path, proof.anchors)
        self.assertIn(codegen, proof.anchors)
        self.assertNotIn(dylib, proof.anchors)
        self.assertNotIn(object_path, self.proof().anchors)
        self.metadata = {"packages": [{"name": "demo", "manifest_path": str(self.manifest),
            "targets": [{"name": "demo", "kind": ["proc-macro"], "src_path": str(self.source)}]}]}
        macro = self.proof(sweep=self.sweep | {dylib})
        self.assertIn(dylib, macro.anchors)
        self.assertNotIn(self.archive, macro.anchors)

    def test_existing_guarded_transaction_counts_union_and_keeps_standard_references(self) -> None:
        different = self.proof()
        proof = LibraryAliasOwnership(self.root, self.target, frozenset(), frozenset(), different.verify,
                                      different_flags=different)
        with ProfileLocks(self.target, proof.verify) as epoch:
            needed = self.archive.stat().st_blocks * 512 + 1
            _, report = evict_artifacts(self.target, 0, needed, True, (self.protected,), epoch,
                retained_profiles=(self.profile,), selected_package_profiles=(self.profile,),
                library_alias_ownership=proof)
        self.assertIn(str(self.archive.relative_to(self.target)), report["candidate_paths"])
        self.assertIn(str(self.metadata_alias.relative_to(self.target)), report["candidate_paths"])
        self.assertEqual(report["deleted_files"], 0)
        self.assertEqual(report["invalidated_profiles"], [])
        self.assertTrue(self.archive.exists())
        self.assertTrue(self.protected.exists())
        self.assertEqual(replace(proof, different_flags=None).different_flags, None)

    def test_standard_object_requires_exact_selected_library_fingerprint_and_sweep(self) -> None:
        object_path = self.deps / ("demo-" + "1" * 16 + ".old.rcgu.o")
        object_path.write_bytes(b"object input")
        proof = self.proof(sweep=self.sweep | {object_path, self.protected})
        self.assertIn(object_path, proof.standalone_objects)
        self.assertNotIn(object_path, proof.anchors)
        self.assertNotIn(self.protected, proof.anchors)
        self.assertNotIn(self.protected, proof.standalone_objects)
        self.assertNotIn(object_path, self.proof().standalone_objects)
        with self.assertRaisesRegex(RuntimeError, "compiler identity is ambiguous"):
            self.proof(cargo=self.cargo - {self.standard, self.standard_metadata_fingerprint},
                       sweep=self.sweep | {object_path})

    def test_standalone_object_does_not_expand_binary_or_wrong_hash_ownership(self) -> None:
        correct = self.deps / ("demo-" + "1" * 16 + ".old.rcgu.o")
        wrong = self.deps / ("demo-" + "3" * 16 + ".old.rcgu.o")
        for path in (correct, wrong):
            path.write_bytes(b"object input")
        proof = self.proof(sweep=self.sweep | {correct, wrong})
        self.assertIn(correct, proof.standalone_objects)
        self.assertNotIn(wrong, proof.standalone_objects)
        self.metadata = {"packages": [{"name": "demo", "manifest_path": str(self.manifest),
            "targets": [{"name": "demo", "kind": ["bin"], "src_path": str(self.source)}]}]}
        with self.assertRaisesRegex(RuntimeError, "compiler identity is ambiguous"):
            self.proof(sweep=self.sweep | {correct})

    def test_standalone_object_current_compiler_join_rejects_obsolete_or_unknown_json(self) -> None:
        object_path = self.deps / ("demo-" + "1" * 16 + ".old.rcgu.o")
        object_path.write_bytes(b"object input")
        with patch.dict(os.environ, {"RUSTFLAGS": "-D warnings"}):
            proof = create_different_flags_ownership(self.root, self.target, self.metadata,
                self.cargo, self.sweep | {object_path}, lambda: None, frozenset({object_path}))
        self.assertNotIn(object_path, proof.standalone_objects)
        proof = self.proof(sweep=self.sweep | {object_path})
        extra = self.standard.parent / "unknown.json"
        extra.write_text("{}")
        self.assertNotIn(object_path, self.proof(sweep=self.sweep | {object_path}).standalone_objects)
        with self.assertRaisesRegex(RuntimeError, "JSON inventory"):
            proof.verify()

    def test_sweep_current_join_requires_complete_toolchain_profile_framing(self) -> None:
        banner = '[INFO] Using specified installed toolchains: ["selected"]'
        root = f'[DEBUG] cleaning: {json.dumps(str(self.target))} with remove_not_built_with'
        profile = f'[DEBUG] cleaning: {json.dumps(str(self.profile))} with remove_not_built_with_in_a_profile'
        raw = "\n".join((banner, root, profile))
        self.assertEqual(_checked_sweep_paths(raw, self.target, "selected"), frozenset())
        for invalid in (raw + "\n[ERROR] lookup failed", raw + "\n[WARN] lookup failed",
                        "\n".join((root, profile)), "\n".join((banner, root))):
            with self.subTest(raw=invalid), self.assertRaisesRegex(RuntimeError, "classification is incomplete"):
                _checked_sweep_paths(invalid, self.target, "selected")


if __name__ == "__main__":
    unittest.main()
