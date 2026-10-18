from __future__ import annotations

import errno
import hashlib
import os
import unittest
from collections.abc import Iterator
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import native_coverage_local_context as local_context


class LocalCoverageContextTests(unittest.TestCase):
    def test_context_digest_covers_every_field_except_itself(self) -> None:
        payload: dict[str, object] = {
            "schema": local_context.SCHEMA, "cacheable": True, "missReasons": [],
            "host": {"os": "macOS", "productVersion": "27.0", "buildVersion": "26A1", "architecture": "arm64"},
            "tools": {key: "version" for key in local_context.TOOLS},
            "environmentSha256": "a" * 64, "resourcesSha256": "b" * 64,
        }
        expected = hashlib.sha256(local_context._canonical(payload)).hexdigest()
        with patch.object(local_context, "_collect", return_value=payload):
            result = local_context.collect_local_context(Path("."), {})
        self.assertEqual(result["hostContextSha256"], expected)
        self.assertEqual(set(result), set(payload) | {"hostContextSha256"})

    def test_unavailable_context_fails_closed_without_exposing_error_detail(self) -> None:
        with patch.object(local_context, "_collect", side_effect=local_context.LocalCoverageContextError("private path")):
            result = local_context.collect_local_context(Path("."), {})
        self.assertFalse(result["cacheable"])
        self.assertEqual(result["missReasons"], ["host_context_unavailable"])
        self.assertNotIn("private path", str(result))

    def test_environment_hash_ignores_run_nonce_but_binds_safe_inputs(self) -> None:
        moon_values = {name: f"first-{name}" for name in local_context.VOLATILE_ENV if name.startswith("MOON_")}
        first = {
            "PATH": "/tools", "LANG": "en_US.UTF-8", "GITHUB_RUN_ID": "1", "API_TOKEN": "secret-a",
            "TMPDIR": "/tmp/one-run",
            "RUSTFLAGS": "-D warnings", "CARGO_TARGET_DIR": "/repo/target",
            "CARGO_LLVM_COV_TARGET_DIR": "/repo/target/llvm-cov-target",
            "KRR_LOCAL_COVERAGE_PROFILE_PATH": "/tmp/profile-one.json", **moon_values,
        }
        same = {
            **first, "GITHUB_RUN_ID": "2",
            "_": "/different/last/command",
            "KRR_LOCAL_COVERAGE_PROFILE_PATH": "/tmp/profile-two.json",
            **{name: f"second-{name}" for name in moon_values},
        }
        changed = {**same, "LANG": "ja_JP.UTF-8"}
        self.assertEqual(local_context._record_environment(first), local_context._record_environment(same))
        self.assertNotEqual(local_context._record_environment(first), local_context._record_environment(changed))
        secret_digest = local_context._record_environment(first)
        self.assertNotEqual(secret_digest, local_context._record_environment({**first, "API_TOKEN": "secret-b"}))
        self.assertNotIn("secret-a", secret_digest)
        self.assertNotEqual(secret_digest, local_context._record_environment({**first, "TMPDIR": "/tmp/other"}))
        self.assertNotEqual(
            secret_digest,
            local_context._record_environment({**first, "CARGO_TARGET_DIR": "/other/target"}),
        )
        with self.assertRaisesRegex(local_context.LocalCoverageContextError, "unsupported_tool_override"):
            local_context._record_environment({"RUSTFLAGS": "-C opt-level=3"})
        with self.assertRaisesRegex(local_context.LocalCoverageContextError, "unsupported_tool_override"):
            local_context._record_environment({"LLVM_COV": "/unreviewed/llvm-cov"})

    def test_cargo_metadata_commands_cover_exact_host_domains_offline(self) -> None:
        with TemporaryDirectory() as temporary:
            repo = Path(temporary)
            commands = local_context._cargo_metadata_commands(repo, "aarch64-apple-darwin")
        self.assertEqual([label for label, _ in commands], [
            "root", "performance-baseline", "performance-candidate",
        ])
        for _, argv in commands:
            self.assertIn("--locked", argv)
            self.assertIn("--offline", argv)
            self.assertIn("--filter-platform", argv)
            self.assertIn("aarch64-apple-darwin", argv)
        self.assertIn("--all-features", commands[0][1])
        self.assertEqual(commands[1][1][-2:], ["--features", "baseline"])
        self.assertEqual(commands[2][1][-2:], ["--features", "candidate"])

    def test_resolved_external_crate_bytes_change_resource_identity(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            package_root = root / "registry" / "fixture-1.0.0"
            (repo / "crates").mkdir(parents=True)
            (package_root / "src").mkdir(parents=True)
            (package_root / "Cargo.toml").write_text("[package]\nname='fixture'\nversion='1.0.0'\n")
            source = package_root / "src" / "lib.rs"
            source.write_bytes(b"pub const VALUE: u8 = 1;\n")
            package_id = "registry+https://github.com/rust-lang/crates.io-index#fixture@1.0.0"
            metadata: dict[str, object] = {
                "packages": [{"id": package_id, "name": "fixture", "version": "1.0.0",
                              "source": "registry+https://github.com/rust-lang/crates.io-index",
                              "manifest_path": str(package_root / "Cargo.toml")}],
                "resolve": {"nodes": [{"id": package_id, "features": [], "deps": []}]},
            }
            before: dict[str, dict[str, object]] = {}
            local_context._record_cargo_metadata(repo, "root", metadata, before, {})
            source.write_bytes(b"pub const VALUE: u8 = 2;\n")
            after: dict[str, dict[str, object]] = {}
            local_context._record_cargo_metadata(repo, "root", metadata, after, {})
        self.assertNotEqual(before, after)

    def test_duplicate_package_ids_fail_closed(self) -> None:
        with TemporaryDirectory() as temporary:
            repo = Path(temporary) / "repo"
            package_root = Path(temporary) / "registry" / "fixture-1.0.0"
            repo.mkdir()
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='fixture'\nversion='1.0.0'\n")
            package = {"id": "fixture-id", "name": "fixture", "version": "1.0.0",
                       "source": "registry+https://example.invalid#index",
                       "manifest_path": str(manifest)}
            metadata: dict[str, object] = {
                "packages": [package, dict(package)],
                "resolve": {"nodes": [{"id": "fixture-id", "features": [], "deps": []}]},
            }
            with self.assertRaisesRegex(local_context.LocalCoverageContextError,
                                        "cargo_dependency_graph_invalid"):
                local_context._record_cargo_metadata(repo, "root", metadata, {}, {})

    def test_symlinked_resolved_package_root_fails_closed(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            package_root = root / "registry" / "fixture-1.0.0"
            alias_root = root / "registry" / "alias"
            repo.mkdir()
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='fixture'\nversion='1.0.0'\n")
            alias_root.symlink_to(package_root, target_is_directory=True)
            package_id = "registry+https://example.invalid#index#fixture@1.0.0"
            metadata: dict[str, object] = {
                "packages": [{"id": package_id, "name": "fixture", "version": "1.0.0",
                              "source": "registry+https://example.invalid#index",
                              "manifest_path": str(alias_root / "Cargo.toml")}],
                "resolve": {"nodes": [{"id": package_id, "features": [], "deps": []}]},
            }
            with self.assertRaisesRegex(local_context.LocalCoverageContextError,
                                        "cargo_dependency_source_symlink"):
                local_context._record_cargo_metadata(repo, "root", metadata, {}, {})

    def test_nonresolved_external_crate_does_not_change_resource_identity(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            selected = root / "registry" / "selected-1.0.0"
            unrelated = root / "registry" / "unrelated-1.0.0"
            (repo / "crates").mkdir(parents=True)
            for package_root, name in ((selected, "selected"), (unrelated, "unrelated")):
                (package_root / "src").mkdir(parents=True)
                (package_root / "Cargo.toml").write_text(
                    f"[package]\nname='{name}'\nversion='1.0.0'\n")
                (package_root / "src" / "lib.rs").write_text(f"pub const NAME: &str = '{name}';\n")
            selected_id = "registry+https://example.invalid#index#selected@1.0.0"
            unrelated_id = "registry+https://example.invalid#index#unrelated@1.0.0"
            metadata: dict[str, object] = {
                "packages": [
                    {"id": selected_id, "name": "selected", "version": "1.0.0",
                     "source": "registry+https://example.invalid#index",
                     "manifest_path": str(selected / "Cargo.toml")},
                    {"id": unrelated_id, "name": "unrelated", "version": "1.0.0",
                     "source": "registry+https://example.invalid#index",
                     "manifest_path": str(unrelated / "Cargo.toml")},
                ],
                "resolve": {"nodes": [{"id": selected_id, "features": [], "deps": []}]},
            }
            before: dict[str, dict[str, object]] = {}
            local_context._record_cargo_metadata(repo, "root", metadata, before, {})
            (unrelated / "src" / "lib.rs").write_text("changed but not selected\n")
            after: dict[str, dict[str, object]] = {}
            local_context._record_cargo_metadata(repo, "root", metadata, after, {})
        self.assertEqual(before, after)

    def test_external_dependency_symlinks_and_missing_sources_fail_closed(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            package_root = root / "registry" / "fixture-1.0.0"
            (repo / "crates").mkdir(parents=True)
            (package_root / "src").mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='fixture'\nversion='1.0.0'\n")
            (package_root / "src" / "lib.rs").write_text("pub const VALUE: u8 = 1;\n")
            package_id = "registry+https://example.invalid#index#fixture@1.0.0"
            metadata: dict[str, object] = {
                "packages": [{"id": package_id, "name": "fixture", "version": "1.0.0",
                              "source": "registry+https://example.invalid#index",
                              "manifest_path": str(manifest)}],
                "resolve": {"nodes": [{"id": package_id, "features": [], "deps": []}]},
            }
            (package_root / "src" / "linked.rs").symlink_to(package_root / "src" / "lib.rs")
            with self.assertRaisesRegex(local_context.LocalCoverageContextError,
                                        "cargo_dependency_source_symlink"):
                local_context._record_cargo_metadata(repo, "root", metadata, {}, {})
            (package_root / "src" / "linked.rs").unlink()
            (package_root / "src" / "lib.rs").unlink()
            manifest.unlink()
            with self.assertRaisesRegex(local_context.LocalCoverageContextError,
                                        "cargo_dependency_source_unavailable"):
                local_context._record_cargo_metadata(repo, "root", metadata, {}, {})

    def test_dependency_tree_digest_closes_scandir_descriptors(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text("[package]\nname='fixture'\n")
            scanned_fds: list[int] = []
            original_scandir = os.scandir

            def capture_scandir(path: int) -> Iterator[os.DirEntry[str]]:
                scanned_fds.append(path)
                return original_scandir(path)

            with patch.object(os, "scandir", side_effect=capture_scandir):
                local_context._dependency_tree_digest(root)
            self.assertTrue(scanned_fds)
            for descriptor in scanned_fds:
                with self.subTest(descriptor=descriptor), self.assertRaises(OSError) as error:
                    os.fstat(descriptor)
                self.assertEqual(error.exception.errno, errno.EBADF)

    def test_dependency_roots_are_hashed_once_across_resolved_domains(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            package_root = root / "registry" / "fixture-1.0.0"
            repo.mkdir()
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='fixture'\nversion='1.0.0'\n")
            package_id = "registry+https://example.invalid#index#fixture@1.0.0"
            metadata: dict[str, object] = {
                "packages": [{"id": package_id, "name": "fixture", "version": "1.0.0",
                              "source": "registry+https://example.invalid#index",
                              "manifest_path": str(manifest)}],
                "resolve": {"nodes": [{"id": package_id, "features": [], "deps": []}]},
            }
            digest_cache: dict[Path, tuple[str, int, int]] = {}
            records: dict[str, dict[str, object]] = {}
            with patch.object(local_context, "_dependency_tree_digest",
                              return_value=("a" * 64, 1, 0)) as digest:
                for label in ("root", "baseline", "candidate"):
                    local_context._record_cargo_metadata(repo, label, metadata, records, digest_cache)
            digest.assert_called_once_with(package_root.resolve())

    def test_font_rows_bind_face_inventory_and_physical_file_bytes(self) -> None:
        with TemporaryDirectory() as temporary:
            font = Path(temporary) / "font.ttc"
            font.write_bytes(b"physical-font-v1")
            rows: list[object] = [{
                "enabled": "yes", "valid": "yes", "type": "TrueType Collection", "path": str(font),
                "typefaces": [{"family": "Fixture", "style": "Regular", "valid": "yes"}],
            }]
            first_records: dict[str, dict[str, object]] = {}
            self.assertEqual(local_context._record_font_rows(rows, first_records), 1)
            font.write_bytes(b"physical-font-v2")
            second_records: dict[str, dict[str, object]] = {}
            local_context._record_font_rows(rows, second_records)
            self.assertNotEqual(first_records, second_records)
            self.assertNotIn(str(font), str(first_records))

    def test_font_inventory_rejects_disabled_invalid_and_missing_faces(self) -> None:
        with TemporaryDirectory() as temporary:
            font = Path(temporary) / "font.ttc"
            font.write_bytes(b"font")
            base = {"enabled": "yes", "valid": "yes", "path": str(font),
                    "typefaces": [{"family": "Fixture", "valid": "yes"}]}
            for row in ({**base, "enabled": "no"}, {**base, "typefaces": []},
                        {**base, "typefaces": [{"valid": "no"}]},
                        {**base, "path": str(font.with_name("missing.ttc"))}):
                with self.subTest(row=row), self.assertRaises(local_context.LocalCoverageContextError):
                    local_context._record_font_rows([row], {})


if __name__ == "__main__":
    unittest.main()
