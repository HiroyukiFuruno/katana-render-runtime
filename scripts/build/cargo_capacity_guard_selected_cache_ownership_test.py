from __future__ import annotations

import os
from pathlib import Path
import stat
import subprocess
from types import SimpleNamespace
import tempfile
import unittest
from collections.abc import Callable
from typing import cast
from unittest.mock import patch

from cargo_capacity_guard import _array, _object
from cargo_capacity_guard_build_script_ownership import BuildScriptOwnership, SourcePin, source_pin
from cargo_capacity_guard_lock import ProfileLocks, evict_artifacts
from cargo_capacity_guard_selected_cache_ownership import (
    SelectedCacheOwnership,
    _tool_pin,
    binary_cache_members,
    create_selected_cache_ownership,
)


class SelectedCacheOwnershipTest(unittest.TestCase):
    @staticmethod
    def fake_tool(tool: Path, version: str = "tool 1.97.1") -> Callable[..., tuple[Path, SourcePin, str]]:
        def query(executable: str, args: tuple[str, ...], *role: str | Path | None) -> tuple[Path, SourcePin, str]:
            return tool, source_pin(tool), version
        return query

    @staticmethod
    def packages(metadata: dict[str, object]) -> list[dict[str, object]]:
        return [_object(item, "package") for item in _array(metadata["packages"], "packages")]

    def fixture(self) -> tuple[Path, dict[str, object]]:
        directory = Path(tempfile.mkdtemp())
        root = (directory / "repo").resolve()
        (root / "crates/kdr-linter/src").mkdir(parents=True)
        (root / "crates/foreign/src").mkdir(parents=True)
        (root / "target/debug").mkdir(parents=True)
        (root / "Cargo.toml").write_text("[workspace]\nmembers=[]\n")
        (root / "Cargo.lock").write_text("lock\n")
        (root / "crates/kdr-linter/Cargo.toml").write_text("[package]\nname='kdr-linter'\n")
        (root / "crates/foreign/Cargo.toml").write_text("[package]\nname='foreign'\n")
        (root / "crates/kdr-linter/src/main.rs").write_text("fn main() {}\n")
        (root / "crates/foreign/src/lib.rs").write_text("pub fn f() {}\n")
        packages: list[dict[str, object]] = [
            {"id": "kdr", "name": "kdr-linter", "manifest_path": str(root / "crates/kdr-linter/Cargo.toml"),
             "targets": [{"name": "mathjax_consumer_facts", "kind": ["bin"], "src_path": str(root / "crates/kdr-linter/src/main.rs")}]},
            {"id": "foreign", "name": "foreign", "manifest_path": str(root / "crates/foreign/Cargo.toml"),
             "targets": [{"name": "foreign", "kind": ["lib"], "src_path": str(root / "crates/foreign/src/lib.rs")}]},
        ]
        metadata: dict[str, object] = {"target_directory": str(root / "target"), "build_directory": str(root / "target"),
                    "workspace_members": ["kdr", "foreign"], "packages": packages,
                    "resolve": {"nodes": [{"id": "kdr", "deps": []}, {"id": "foreign", "deps": []}]}}
        return root, metadata

    def test_owner_is_typed_and_pins_manifest_and_source(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            self.assertEqual([(owner.prefix, owner.kind, owner.package_id) for owner in proof.owners], [("mathjax_consumer_facts-", "binary", "kdr")])
            proof.verify()

    def test_ambiguous_namespace_is_omitted_and_unknown_kind_fails(self) -> None:
        root, metadata = self.fixture()
        _array(self.packages(metadata)[0]["targets"], "targets").append({"name": "foreign", "kind": ["bin"], "src_path": str(root / "crates/kdr-linter/src/main.rs")})
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        self.assertEqual([(owner.prefix, owner.package_id) for owner in proof.owners], [("mathjax_consumer_facts-", "kdr")])
        _object(_array(self.packages(metadata)[1]["targets"], "targets")[0], "target")["kind"] = ["mystery"]
        with self.assertRaisesRegex(RuntimeError, "unknown"):
            create_selected_cache_ownership(root, root / "target", metadata, lambda: None)

    def test_source_or_environment_drift_is_rejected(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        (root / "crates/kdr-linter/src/main.rs").write_text("fn main() { changed(); }\n")
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            with self.assertRaisesRegex(RuntimeError, "source"):
                proof.verify()

    def test_manifest_drift_is_rejected(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            (root / "crates/kdr-linter/Cargo.toml").write_text("[package]\nname='changed'\n")
            with self.assertRaisesRegex(RuntimeError, "source"):
                proof.verify()

    def test_environment_drift_is_rejected_independently(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            with patch.dict(os.environ, {"KRR_SELECTED_CACHE_TEST_DRIFT": "1"}):
                with self.assertRaisesRegex(RuntimeError, "environment"):
                    proof.verify()

    def test_tool_executable_drift_is_rejected_independently(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            tool.write_bytes(b"changed executable")
            with self.assertRaisesRegex(RuntimeError, "tool"):
                proof.verify()

    def test_tool_version_drift_is_rejected_independently(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        initial = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", initial):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        changed = self.fake_tool(tool, "tool 1.98.0")
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", changed):
            with self.assertRaisesRegex(RuntimeError, "version"):
                proof.verify()

    def test_tool_pin_preserves_symlink_invocation_and_detects_retarget(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            first = directory / "cargo-real-a"
            second = directory / "cargo-real-b"
            for path, body in ((first, "cargo 1.97.1\n"), (second, "cargo 1.97.1\nchanged\n")):
                path.write_text(f"#!/bin/sh\nprintf '%s' '{body}'\n")
                path.chmod(stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
            alias = directory / "cargo"
            alias.symlink_to(first.name)
            pinned = _tool_pin(str(alias), ("-Vv",), "cargo")
            self.assertEqual(pinned[0], alias)
            self.assertEqual(pinned[1].path, first.resolve())
            alias.unlink()
            alias.symlink_to(second.name)
            retargeted = _tool_pin(str(alias), ("-Vv",), "cargo")
            self.assertNotEqual(pinned, retargeted)
            self.assertEqual(retargeted[1].path, second.resolve())

    def test_tool_pin_rejects_timeout_oserror_and_stderr_only_version(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tool = Path(temporary) / "rustc"
            tool.write_text("tool")
            with patch("cargo_capacity_guard_selected_cache_ownership.subprocess.run",
                       side_effect=subprocess.TimeoutExpired("rustc", 10)):
                with self.assertRaisesRegex(RuntimeError, "unavailable"):
                    _tool_pin(str(tool), ("-vV",), "rustc")
            with patch("cargo_capacity_guard_selected_cache_ownership.subprocess.run",
                       side_effect=OSError("exec failed")):
                with self.assertRaisesRegex(RuntimeError, "unavailable"):
                    _tool_pin(str(tool), ("-vV",), "rustc")
            result = subprocess.CompletedProcess([str(tool)], 0, stdout="", stderr="rustc 1.97.1")
            with patch("cargo_capacity_guard_selected_cache_ownership.subprocess.run", return_value=result):
                with self.assertRaisesRegex(RuntimeError, "unexpected identity"):
                    _tool_pin(str(tool), ("-vV",), "rustc")

    def test_duplicate_package_and_resolve_metadata_fail_closed(self) -> None:
        root, metadata = self.fixture()
        packages = self.packages(metadata)
        packages.append(dict(packages[1]))
        metadata["packages"] = packages
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        root, metadata = self.fixture()
        resolve = _object(metadata["resolve"], "resolve")
        nodes = _array(resolve["nodes"], "nodes")
        nodes.append(dict(_object(nodes[1], "node")))
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            create_selected_cache_ownership(root, root / "target", metadata, lambda: None)

    def test_library_and_binary_kinds_are_typed_and_custom_build_is_omitted(self) -> None:
        root, metadata = self.fixture()
        package = self.packages(metadata)[0]
        _array(package["targets"], "targets").extend([
            {"name": "foreign_bin", "kind": ["bin"], "src_path": str(root / "crates/kdr-linter/src/main.rs")},
            {"name": "foreign_test", "kind": ["test"], "src_path": str(root / "crates/kdr-linter/src/main.rs")},
            {"name": "foreign_build", "kind": ["custom-build"], "src_path": str(root / "crates/kdr-linter/src/main.rs")},
        ])
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        self.assertEqual({(owner.prefix, owner.kind) for owner in proof.owners},
                         {("mathjax_consumer_facts-", "binary"), ("foreign_bin-", "binary"), ("foreign_test-", "binary")})
        self.assertNotIn("foreign-", {owner.prefix for owner in proof.owners})

    def test_custom_build_namespace_collision_excludes_selected_target(self) -> None:
        root, metadata = self.fixture()
        _array(self.packages(metadata)[1]["targets"], "targets").append(
            {"name": "mathjax_consumer_facts", "kind": ["custom-build"],
             "src_path": str(root / "crates/foreign/src/lib.rs")})
        tool = root / "tool"
        tool.write_bytes(b"tool")
        fake = self.fake_tool(tool)
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", fake):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        self.assertNotIn("mathjax_consumer_facts-", {owner.prefix for owner in proof.owners})

    def test_same_package_same_class_target_namespace_is_ambiguous(self) -> None:
        root, metadata = self.fixture()
        host = self.packages(metadata)[0]
        _array(host["targets"], "targets").append(
            {"name": "mathjax_consumer_facts", "kind": ["test"],
             "src_path": str(root / "crates/kdr-linter/src/other.rs")})
        tool = root / "tool"
        tool.write_bytes(b"tool")
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        self.assertNotIn("mathjax_consumer_facts-", {owner.prefix for owner in proof.owners})

    def test_legal_numeric_external_example_is_inventory_only(self) -> None:
        root, metadata = self.fixture()
        package: dict[str, object] = {
            "id": "registry-clap", "name": "clap", "manifest_path": str(root / "external/Cargo.toml"),
            "targets": [{"name": "01_quick", "kind": ["example"],
                         "src_path": str(root / "external/examples/01_quick.rs")}],
        }
        _array(metadata["packages"], "packages").append(package)
        _array(_object(metadata["resolve"], "resolve")["nodes"], "nodes").append(
            {"id": "registry-clap", "deps": []})
        tool = root / "tool"
        tool.write_bytes(b"tool")
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
        self.assertEqual({owner.prefix for owner in proof.owners}, {"mathjax_consumer_facts-"})

    def test_tool_selection_uses_owned_root_not_invoking_process_directory(self) -> None:
        root, metadata = self.fixture()
        tool = root / "tool"
        tool.write_bytes(b"tool")
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", side_effect=self.fake_tool(tool)) as query:
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            proof.verify()
        self.assertTrue(query.call_args_list)
        self.assertTrue(all(call.args[3] == root for call in query.call_args_list))

    def test_binary_cache_rejects_rmeta_unknown_and_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin", "thing.o"):
                (directory / name).write_bytes(b"cache")
            (directory / "metadata.rmeta").write_bytes(b"rmeta")
            (directory / "alias.o").symlink_to(directory / "thing.o")
            fd = os.open(directory, os.O_RDONLY)
            try:
                self.assertFalse(binary_cache_members(fd, ["dep-graph.bin", "query-cache.bin", "work-products.bin", "metadata.rmeta"]))
                self.assertFalse(binary_cache_members(fd, ["dep-graph.bin", "query-cache.bin", "work-products.bin", "alias.o"]))
                self.assertFalse(binary_cache_members(fd, ["dep-graph.bin", "query-cache.bin", "work-products.bin", "unknown.bin"]))
            finally:
                os.close(fd)

    def test_binary_cache_rejects_protected_index_flags_or_links(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin"):
                (directory / name).write_bytes(b"cache")
            fd = os.open(directory, os.O_RDONLY)
            original_stat = os.stat
            try:
                original = original_stat(directory / "query-cache.bin", follow_symlinks=False)
                fake = SimpleNamespace(st_mode=original.st_mode, st_nlink=2, st_flags=1)
                original_call = cast(Callable[..., object], original_stat)

                def stat_with_protected(path: str | Path, *args: object, **kwargs: object) -> object:
                    if path == "query-cache.bin" and kwargs.get("dir_fd") == fd:
                        return fake
                    return original_call(path, *args, **kwargs)
                with patch("cargo_capacity_guard_selected_cache_ownership.os.stat", side_effect=stat_with_protected):
                    self.assertFalse(binary_cache_members(fd, ["dep-graph.bin", "query-cache.bin", "work-products.bin"]))
            finally:
                os.close(fd)

    def test_binary_cache_rejects_present_but_incomplete_graph(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ("dep-graph.bin", "work-products.bin"):
                (directory / name).write_bytes(b"cache")
            fd = os.open(directory, os.O_RDONLY)
            try:
                self.assertFalse(binary_cache_members(fd, ["dep-graph.bin", "work-products.bin"]))
            finally:
                os.close(fd)

    def test_binary_cache_members_accepts_full_and_graphless_residual(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ("dep-graph.bin", "query-cache.bin", "work-products.bin", "thing.o"):
                (directory / name).write_bytes(b"cache")
            fd = os.open(directory, os.O_RDONLY)
            try:
                self.assertTrue(binary_cache_members(fd, ["dep-graph.bin", "query-cache.bin", "work-products.bin", "thing.o"]))
                (directory / "dep-graph.bin").unlink()
                self.assertTrue(binary_cache_members(fd, ["query-cache.bin", "work-products.bin", "thing.o"]))
                (directory / "work-products.bin").unlink()
                self.assertFalse(binary_cache_members(fd, ["query-cache.bin", "thing.o"]))
            finally:
                os.close(fd)

    def library_test_cache_fixture(self) -> tuple[Path, dict[str, object], list[Path], list[Path], Path]:
        root, metadata = self.fixture()
        _object(_array(_object(metadata["resolve"], "resolve")["nodes"], "nodes")[0], "kdr")["deps"] = [{"pkg": "foreign"}]
        profile = root / "target/debug"
        (profile / "build").mkdir()
        fingerprint = profile / ".fingerprint/foreign-308913bb6327ec3c"
        fingerprint.mkdir(parents=True)
        (fingerprint / "test-lib-foreign.json").write_bytes(b"retained fingerprint")
        deps = profile / "deps"
        deps.mkdir()
        published = deps / "foreign-308913bb6327ec3c"
        published.write_bytes(b"retained test binary")
        owner = profile / "incremental/foreign-2hnmc5d50qhlg"
        owner.mkdir(parents=True)
        sessions = []
        for name in ["s-hn6xiarwn4-13w5ba9-en6yy2sj3e29md0s69q217swc",
                     "s-hn6xtgc7ia-180k07y-en6yy2sj3e29md0s69q217swc"]:
            session = owner / name
            session.mkdir()
            (owner / ("-".join(name.split("-")[:3]) + ".lock")).write_bytes(b"")
            for index in ("dep-graph.bin", "query-cache.bin", "work-products.bin"):
                (session / index).write_bytes(b"cache index")
            sessions.append(session)
        (sessions[0] / "thing.o").write_bytes(b"retained object")
        os.link(sessions[0] / "thing.o", sessions[1] / "thing.o")
        os.link(sessions[0] / "thing.o", deps / "foreign-308913bb6327ec3c.thing.rcgu.o")
        tool = root / "tool"
        tool.write_bytes(b"tool")
        with ProfileLocks(root / "target"):
            pass
        return root, metadata, sessions, [published], tool

    def independent_runtime_fixture(self) -> tuple[Path, dict[str, object], dict[str, object], Path]:
        root, metadata = self.fixture()
        package = root / "crates/katana-render-runtime"
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_text("[package]\nname='katana-render-runtime'\n")
        (package / "src/lib.rs").write_text("pub fn runtime() {}\n")
        runtime: dict[str, object] = {
            "id": "runtime", "name": "katana-render-runtime", "manifest_path": str(package / "Cargo.toml"),
            "targets": [{"name": "katana_render_runtime", "kind": ["lib"], "src_path": str(package / "src/lib.rs")}],
        }
        _array(metadata["packages"], "packages").append(runtime)
        _array(metadata["workspace_members"], "members").append("runtime")
        _array(_object(metadata["resolve"], "resolve")["nodes"], "nodes").append({"id": "runtime", "deps": []})
        tool = root / "tool"
        tool.write_bytes(b"tool")
        return root, metadata, runtime, tool

    def test_alias_domain_adds_independent_runtime_without_changing_host_default(self) -> None:
        root, metadata, _runtime, tool = self.independent_runtime_fixture()
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
            host = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            alias = create_selected_cache_ownership(root, root / "target", metadata, lambda: None, library_alias=True)
            host.verify()
            alias.verify()
        self.assertEqual({owner.prefix for owner in host.owners}, {"mathjax_consumer_facts-"})
        self.assertEqual({(owner.prefix, owner.kind, owner.package_id) for owner in alias.owners},
                         {("mathjax_consumer_facts-", "binary", "kdr"), ("katana_render_runtime-", "library", "runtime")})

    def test_alias_domain_requires_exact_canonical_runtime_root(self) -> None:
        for mutation in ("missing", "nonmember", "wrong-name", "wrong-manifest", "duplicate", "source-escape"):
            with self.subTest(mutation=mutation):
                root, metadata, runtime, tool = self.independent_runtime_fixture()
                if mutation == "missing":
                    _array(metadata["packages"], "packages").remove(runtime)
                elif mutation == "nonmember":
                    _array(metadata["workspace_members"], "members").remove("runtime")
                elif mutation == "wrong-name":
                    runtime["name"] = "another-runtime"
                elif mutation == "wrong-manifest":
                    runtime["manifest_path"] = str(root / "crates/foreign/Cargo.toml")
                elif mutation == "duplicate":
                    duplicate = {**runtime, "id": "runtime-copy"}
                    _array(metadata["packages"], "packages").append(duplicate)
                    _array(metadata["workspace_members"], "members").append("runtime-copy")
                    _array(_object(metadata["resolve"], "resolve")["nodes"], "nodes").append({"id": "runtime-copy", "deps": []})
                else:
                    _object(_array(runtime["targets"], "targets")[0], "target")["src_path"] = str(root / "crates/foreign/src/lib.rs")
                with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
                    with self.assertRaises(RuntimeError):
                        create_selected_cache_ownership(root, root / "target", metadata, lambda: None, library_alias=True)

    def test_alias_domain_keeps_ambiguous_namespace_excluded(self) -> None:
        root, metadata, _runtime, tool = self.independent_runtime_fixture()
        foreign = _object(_array(self.packages(metadata)[1]["targets"], "targets")[0], "target")
        foreign["name"] = "katana_render_runtime"
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None, library_alias=True)
        self.assertEqual({owner.prefix for owner in proof.owners}, {"mathjax_consumer_facts-"})

    def test_alias_domain_runtime_inputs_and_tools_recheck(self) -> None:
        for mutation in ("source", "manifest", "tool", "environment"):
            with self.subTest(mutation=mutation):
                root, metadata, _runtime, tool = self.independent_runtime_fixture()
                with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
                    proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None, library_alias=True)
                    if mutation in ("source", "manifest"):
                        (root / "crates/katana-render-runtime" / ("src/lib.rs" if mutation == "source" else "Cargo.toml")).write_bytes(b"changed")
                    elif mutation == "tool":
                        tool.write_bytes(b"changed tool")
                    with patch.dict(os.environ, {"KRR_ALIAS_CACHE_TEST_DRIFT": "1"} if mutation == "environment" else {}):
                        with self.assertRaisesRegex(RuntimeError, "source|tool|environment"):
                            proof.verify()

    @staticmethod
    def cache_tree(target: Path) -> dict[str, tuple[int, int, int, bytes]]:
        return {str(path.relative_to(target)): (path.stat().st_dev, path.stat().st_ino,
                path.stat().st_nlink, path.read_bytes())
                for path in target.rglob("*") if path.is_file() and not path.is_symlink()}

    @staticmethod
    def cache_index_bytes(sessions: list[Path]) -> int:
        return sum((session / name).stat().st_blocks * 512
                   for session in sessions for name in ("dep-graph.bin", "query-cache.bin"))

    def evict_library_test_cache(
        self, root: Path, metadata: dict[str, object], protected: list[Path], tool: Path,
        needed: int, *, dry_run: bool = False,
    ) -> tuple[int, dict[str, object], SelectedCacheOwnership]:
        with patch("cargo_capacity_guard_selected_cache_ownership._tool_pin", self.fake_tool(tool)):
            proof = create_selected_cache_ownership(root, root / "target", metadata, lambda: None)
            files = {(value.st_dev, value.st_ino): value.st_blocks * 512
                     for path in (root / "target").rglob("*") if path.is_file()
                     for value in (path.stat(),)}
            current = sum(files.values())
            host = BuildScriptOwnership(root, proof.target, proof.profile, (), proof.verify)
            with ProfileLocks(proof.target, proof.verify) as epoch:
                code, report = evict_artifacts(proof.target, current - needed, current, dry_run,
                    protected, epoch, retained_profiles=(proof.profile,),
                    selected_package_profiles=(proof.profile,), build_script_ownership=host,
                    selected_cache_ownership=proof)
            return code, dict(report), proof

    @unittest.skipIf(os.name == "nt", "POSIX library test-mode optional cache")
    def test_library_target_test_cache_preserves_objects_and_workproducts(self) -> None:
        for dry_run in (True, False):
            with self.subTest(dry_run=dry_run):
                root, metadata, sessions, protected, tool = self.library_test_cache_fixture()
                before = self.cache_tree(root / "target")
                needed = self.cache_index_bytes(sessions)
                code, report, proof = self.evict_library_test_cache(root, metadata, protected, tool, needed, dry_run=dry_run)
                self.assertEqual(code, 0, report)
                self.assertEqual(next(owner.kind for owner in proof.owners if owner.prefix == "foreign-"), "library")
                wanted = {str((session / name).relative_to(root / "target"))
                          for session in sessions for name in ("dep-graph.bin", "query-cache.bin")}
                self.assertEqual(set(cast(list[str], report["candidate_paths"])), wanted)
                self.assertEqual(report["invalidated_profiles"], [])
                self.assertEqual(report["estimated_candidate_bytes"], needed)
                self.assertEqual(report["deleted_files"], 0 if dry_run else 4)
                self.assertEqual(self.cache_tree(root / "target"), before if dry_run else
                    {name: value for name, value in before.items() if name not in wanted})

    @unittest.skipIf(os.name == "nt", "POSIX library test-mode optional cache")
    def test_library_target_test_cache_requires_finite_owned_session(self) -> None:
        for mutation in ("unknown-owner", "unknown-member", "partial-graph", "missing-work-products",
                         "index-alias", "index-symlink", "object-symlink", "protected-index", "missing-lock", "binary-mixed"):
            with self.subTest(mutation=mutation):
                root, metadata, sessions, protected, tool = self.library_test_cache_fixture()
                needed = self.cache_index_bytes(sessions)
                if mutation == "unknown-owner":
                    sessions[0].parent.rename(sessions[0].parent.with_name("unowned-2hnmc5d50qhlg"))
                elif mutation == "binary-mixed":
                    _object(_array(self.packages(metadata)[1]["targets"], "targets")[0], "target")["kind"] = ["bin"]
                    for session in sessions:
                        (session / "metadata.rmeta").write_bytes(b"mixed library metadata")
                else:
                    for session in sessions:
                        if mutation == "unknown-member":
                            (session / "unknown.bin").write_bytes(b"unknown")
                        elif mutation in ("partial-graph", "missing-work-products"):
                            (session / ("query-cache.bin" if mutation == "partial-graph" else "work-products.bin")).unlink()
                        elif mutation == "index-alias":
                            os.link(session / "query-cache.bin", root / (session.name + "-alias"))
                        elif mutation in ("index-symlink", "object-symlink"):
                            file = session / ("query-cache.bin" if mutation == "index-symlink" else "thing.o")
                            file.unlink()
                            file.symlink_to(protected[0])
                        elif mutation == "protected-index":
                            protected.append(session / "query-cache.bin")
                        elif mutation == "missing-lock":
                            (session.parent / ("-".join(session.name.split("-")[:3]) + ".lock")).unlink()
                before = self.cache_tree(root / "target")
                code, report, _proof = self.evict_library_test_cache(root, metadata, protected, tool, needed)
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertEqual(self.cache_tree(root / "target"), before)

    @unittest.skipUnless(hasattr(os, "chflags"), "BSD cache flags")
    def test_library_target_test_cache_preserves_flagged_indices(self) -> None:
        root, metadata, sessions, protected, tool = self.library_test_cache_fixture()
        needed = self.cache_index_bytes(sessions)
        indices = [session / "query-cache.bin" for session in sessions]
        before = self.cache_tree(root / "target")
        try:
            for index in indices:
                os.chflags(index, stat.UF_NODUMP)
            code, report, _proof = self.evict_library_test_cache(root, metadata, protected, tool, needed)
            self.assertEqual(code, 1, report)
            self.assertEqual(report["deleted_files"], 0)
            self.assertEqual(self.cache_tree(root / "target"), before)
        finally:
            for index in indices:
                os.chflags(index, 0)


if __name__ == "__main__":
    unittest.main()
