from __future__ import annotations

import json
import os
from pathlib import Path
import tempfile
import unittest
import stat

from cargo_capacity_guard import JsonObject, _selected_input_check
from cargo_capacity_guard_build_script_ownership import (
    BuildScriptOwnership, build_script_ownership, orphan_build_scripts, verify_orphan_build_scripts,
)
from cargo_capacity_guard_lock import ProfileLocks, evict_artifacts


class BuildScriptOwnershipTests(unittest.TestCase):
    def fixture(self, root: Path) -> tuple[Path, Path, JsonObject]:
        target = root / "target"
        directory = target / "debug/build/worker-0123456789abcdef"
        directory.mkdir(parents=True)
        (target / "debug/.fingerprint").mkdir()
        for package in ("kdr-linter", "worker"):
            base = root / "crates" / package
            base.mkdir(parents=True)
            (base / "Cargo.toml").write_text(f'[package]\nname="{package}"\nversion="0.1.0"\n')
            (base / "build.rs").write_text("fn main() {}\n")
        (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/*"]\n')
        (root / "Cargo.lock").write_text("version = 4\n")
        binary = directory / "build_script_build-0123456789abcdef"
        binary.write_bytes(b"compiled script" * 2048)
        (directory / "build-script-build").write_bytes(binary.read_bytes())
        dep = binary.with_suffix(".d")
        source = root / "crates/worker/build.rs"
        dep.write_text(f"{dep}: {source}\n\n{binary}: {source}\n\n{source}:\n")
        metadata: JsonObject = {
            "target_directory": str(target), "build_directory": str(target),
            "workspace_members": ["host", "worker"],
            "packages": [
                {"id": "host", "name": "kdr-linter", "manifest_path": str(root / "crates/kdr-linter/Cargo.toml"),
                 "targets": [{"name": "mathjax_consumer_facts", "kind": ["bin"]}]},
                {"id": "worker", "name": "worker", "manifest_path": str(root / "crates/worker/Cargo.toml"),
                 "targets": [{"name": "worker", "kind": ["lib"]},
                             {"name": "build-script-build", "kind": ["custom-build"], "src_path": str(source)}]},
            ],
            "resolve": {"nodes": [{"id": "host", "deps": []}, {"id": "worker", "deps": []}]},
        }
        return target, directory, metadata

    def proof(self, root: Path, target: Path, metadata: JsonObject) -> BuildScriptOwnership:
        return build_script_ownership(root, target, metadata, _selected_input_check(root, metadata))

    def candidates(self, proof: BuildScriptOwnership, protected: set[tuple[int, int]] | None = None) -> int:
        profile_fd = os.open(proof.profile, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        build_fd = os.open("build", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=profile_fd)
        try:
            return len(orphan_build_scripts(proof, profile_fd, build_fd, proof.target.stat().st_dev,
                                           set() if protected is None else protected))
        finally:
            os.close(build_fd)
            os.close(profile_fd)

    def test_metadata_dep_info_and_absent_fingerprint_prove_exact_three_leaves(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value).resolve()
            target, _, metadata = self.fixture(root)
            self.assertEqual(self.candidates(self.proof(root, target, metadata)), 1)

    def test_unselected_owner_and_dep_info_boundaries(self) -> None:
        for mutation in ("selected", "unknown-leaf", "foreign-target", "foreign-source", "malformed-dep",
                         "source-omitted", "different-copy", "fingerprint", "outside-alias", "symlink"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, directory, metadata = self.fixture(root)
                binary = directory / "build_script_build-0123456789abcdef"
                dep = binary.with_suffix(".d")
                if mutation == "selected":
                    metadata["resolve"] = {"nodes": [{"id": "host", "deps": [{"pkg": "worker"}]},
                                                       {"id": "worker", "deps": []}]}
                elif mutation == "unknown-leaf":
                    (directory / "unknown").write_bytes(b"retain")
                elif mutation == "foreign-target":
                    dep.write_text(dep.read_text().replace(str(binary), str(root / "foreign")))
                elif mutation == "foreign-source":
                    foreign = root / "outside.rs"
                    foreign.write_text("fn main() {}")
                    dep.write_text(f"{dep}: {foreign}\n")
                elif mutation == "malformed-dep":
                    dep.write_text("unproven\\ target: x\n")
                elif mutation == "source-omitted":
                    dep.write_text(f"{dep}:\n")
                elif mutation == "different-copy":
                    (directory / "build-script-build").write_bytes(b"foreign")
                elif mutation == "fingerprint":
                    (target / "debug/.fingerprint" / directory.name).mkdir()
                elif mutation == "outside-alias":
                    os.link(binary, root / "unknown-link")
                elif mutation == "symlink":
                    binary.rename(root / "foreign-binary")
                    binary.symlink_to(root / "foreign-binary")
                self.assertEqual(self.candidates(self.proof(root, target, metadata)), 0)

    def test_protected_leaf_prevents_whole_owner_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value).resolve()
            target, directory, metadata = self.fixture(root)
            binary = directory / "build-script-build"
            identity = binary.stat().st_dev, binary.stat().st_ino
            self.assertEqual(self.candidates(self.proof(root, target, metadata), {identity}), 0)

    def test_source_input_drift_invalidates_proof(self) -> None:
        for mutation in ("source", "manifest", "Cargo.lock"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, _, metadata = self.fixture(root)
                proof = self.proof(root, target, metadata)
                path = root / ("crates/worker/build.rs" if mutation == "source" else
                               "crates/worker/Cargo.toml" if mutation == "manifest" else "Cargo.lock")
                path.write_text(path.read_text() + "\n")
                with self.assertRaises(RuntimeError):
                    self.candidates(proof)

    def test_new_fingerprint_and_namespace_or_leaf_drift_rejected_after_selection(self) -> None:
        for mutation in ("fingerprint", "fingerprint-root", "unknown", "leaf", "source"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, directory, metadata = self.fixture(root)
                proof = self.proof(root, target, metadata)
                profile_fd = os.open(proof.profile, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                build_fd = os.open(proof.profile / "build", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                owner_fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                try:
                    rows = orphan_build_scripts(proof, profile_fd, build_fd, target.stat().st_dev, set())
                    if mutation == "fingerprint":
                        (target / "debug/.fingerprint" / directory.name).mkdir()
                    elif mutation == "fingerprint-root":
                        (target / "debug/.fingerprint").rename(target / "old-fingerprint-root")
                        (target / "debug/.fingerprint").mkdir()
                    elif mutation == "unknown":
                        (directory / "new").write_bytes(b"new")
                    elif mutation == "leaf":
                        (directory / "build-script-build").write_bytes(b"replacement")
                    else:
                        (root / "crates/worker/build.rs").write_text("fn changed() {}")
                    with self.assertRaises(RuntimeError):
                        verify_orphan_build_scripts(proof, profile_fd, {directory: owner_fd}, rows, set())
                finally:
                    os.close(owner_fd)
                    os.close(build_fd)
                    os.close(profile_fd)

    def test_insufficient_capacity_and_unbound_caller_cannot_unlink(self) -> None:
        for bound in (False, True):
            with self.subTest(bound=bound), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, directory, metadata = self.fixture(root)
                reference = root / "consumer"
                reference.write_bytes(b"keep")
                proof = self.proof(root, target, metadata)
                before = {path.name: path.read_bytes() for path in directory.iterdir()}
                with ProfileLocks(target, proof.verify if bound else None) as epoch:
                    code, report = evict_artifacts(target, 0, 10**9, False, [reference], epoch,
                                                   retained_profiles=[proof.profile],
                                                   selected_package_profiles=[proof.profile],
                                                   build_script_ownership=proof)
                self.assertEqual(code, 1, json.dumps(report))
                self.assertEqual(report["deleted_files"], 0)
                self.assertEqual(before, {path.name: path.read_bytes() for path in directory.iterdir()})

    @unittest.skipUnless(hasattr(os, "chflags"), "native file flags")
    def test_native_flags_on_leaf_or_owner_prevent_cleanup(self) -> None:
        for directory_flag in (False, True):
            with self.subTest(directory_flag=directory_flag), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, directory, metadata = self.fixture(root)
                proof = self.proof(root, target, metadata)
                flagged = directory if directory_flag else directory / "build-script-build"
                os.chflags(flagged, stat.UF_IMMUTABLE)
                try:
                    if directory_flag:
                        with self.assertRaises(RuntimeError):
                            self.candidates(proof)
                    else:
                        self.assertEqual(self.candidates(proof), 0)
                finally:
                    os.chflags(flagged, 0)

    def test_partial_purge_fails_closed_without_changing_fingerprints_or_consumers(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value).resolve()
            target, directory, metadata = self.fixture(root)
            reference = root / "consumer"
            reference.write_bytes(b"keep")
            original_check = _selected_input_check(root, metadata)
            armed = False

            def check() -> None:
                original_check()
                if armed and len(list(directory.iterdir())) != 3:
                    raise RuntimeError("injected interruption after the first owned unlink")

            proof = build_script_ownership(root, target, metadata, check)
            armed = True
            before = sum(path.stat().st_blocks * 512 for path in directory.iterdir())
            with ProfileLocks(target, proof.verify) as epoch:
                code, report = evict_artifacts(target, 0, before, False, [reference], epoch,
                                               retained_profiles=[proof.profile],
                                               selected_package_profiles=[proof.profile],
                                               build_script_ownership=proof)
            self.assertEqual(code, 1, report)
            self.assertEqual(report["deleted_files"], 1)
            self.assertEqual(report["invalidated_profiles"], [])
            self.assertEqual(reference.read_bytes(), b"keep")
            self.assertEqual(len(list(directory.iterdir())), 2)
            self.assertEqual(list((target / "debug/.fingerprint").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
