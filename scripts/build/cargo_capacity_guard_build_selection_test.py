from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import sys
from types import ModuleType
import unittest
from unittest.mock import patch

import cargo_capacity_guard as guard

class BuildSelectionTests(unittest.TestCase):
    root: Path
    home: Path
    environment: dict[str, str]

    def setUp(self) -> None:
        fixture_root = tempfile.TemporaryDirectory(prefix="cargo-capacity-guard-pure-")
        self.addCleanup(fixture_root.cleanup)
        test_root = Path(fixture_root.name) / self._testMethodName
        self.root = test_root / "workspace"
        self.home = test_root / "cargo-home"
        self.root.mkdir(parents=True, exist_ok=True)
        self.home.mkdir(parents=True, exist_ok=True)
        self.environment = {"CARGO_HOME": str(self.home)}

    def config(self, base: Path, text: str, name: str = "config.toml") -> Path:
        base.mkdir(parents=True, exist_ok=True)
        path = base / name
        path.write_text(text)
        return path

    def rejected(self) -> None:
        with self.assertRaises(RuntimeError):
            guard.validate_build_selection(self.root, self.environment)

    def test_absent_target_selection_is_host(self) -> None:
        guard.validate_build_selection(self.root, self.environment)

    def test_environment_target_and_empty_value_are_rejected(self) -> None:
        for value in ("aarch64-apple-darwin", ""):
            self.environment["CARGO_BUILD_TARGET"] = value
            self.rejected()

    def test_workspace_target_config_is_rejected(self) -> None:
        self.config(self.root / ".cargo", '[build]\ntarget="aarch64-apple-darwin"\n')
        self.rejected()

    def test_ancestor_target_config_is_rejected(self) -> None:
        self.config(self.root.parent / ".cargo", '[build]\ntarget="aarch64-apple-darwin"\n')
        self.rejected()

    def test_cargo_home_target_config_is_rejected(self) -> None:
        self.config(self.home, '[build]\ntarget="aarch64-apple-darwin"\n')
        self.rejected()

    def test_multi_target_and_custom_target_are_rejected(self) -> None:
        for value in ('["aarch64-apple-darwin", "x86_64-apple-darwin"]', '"targets/custom.json"'):
            self.config(self.root / ".cargo", '[build]\ntarget=' + value + '\n')
            self.rejected()

    def test_config_include_is_unknown_and_rejected(self) -> None:
        self.config(self.root / ".cargo", 'include=["other.toml"]\n')
        self.rejected()

    def test_invalid_config_is_rejected(self) -> None:
        self.config(self.root / ".cargo", '[build\n')
        self.rejected()

    def test_read_failure_is_rejected(self) -> None:
        self.config(self.home, '[build]\njobs=1\n')
        with patch.object(Path, "read_text", side_effect=OSError("unreadable")):
            self.rejected()

    def test_relative_cargo_home_is_rejected(self) -> None:
        self.environment["CARGO_HOME"] = "relative"
        self.rejected()

    def test_legacy_config_takes_precedence_over_toml(self) -> None:
        self.config(self.home, '[build]\njobs=1\n', "config")
        ignored = self.config(self.home, '[build]\ntarget="aarch64-apple-darwin"\n')
        self.assertNotIn(ignored, guard.cargo_configuration_paths(self.root, self.environment))
        guard.validate_build_selection(self.root, self.environment)

    def test_known_split_directory_and_target_specific_flags_are_admitted(self) -> None:
        self.config(self.root / ".cargo", '[build]\nbuild-dir="target/build"\n'
                    '[target.aarch64-apple-darwin]\nrustflags=["-Copt-level=1"]\n')
        guard.validate_build_selection(self.root, self.environment)

    def test_shared_parser_restores_module_search_path(self) -> None:
        self.config(self.home, '[build]\njobs=1\n')
        original = list(sys.path)
        guard.validate_build_selection(self.root, self.environment)
        self.assertEqual(sys.path, original)

    def test_foreign_cached_parser_is_rejected_before_calling_it(self) -> None:
        self.config(self.home, '[build]\njobs=1\n')
        foreign = ModuleType("toml_loader")
        foreign.__dict__["load_toml_parser"] = lambda: self.fail("foreign parser called")
        with patch.dict(sys.modules, {"toml_loader": foreign}):
            self.rejected()

    def test_foreign_lazy_cache_is_rejected_without_attribute_hooks(self) -> None:
        self.config(self.home, '[build]\njobs=1\n')
        calls: list[str] = []
        foreign = ModuleType("toml_loader")

        def attribute(name: str) -> object:
            calls.append(name)
            raise AttributeError(name)

        foreign.__dict__["__getattr__"] = attribute
        original = list(sys.path)
        with patch.dict(sys.modules, {"toml_loader": foreign}):
            self.rejected()
        self.assertEqual(calls, [])
        self.assertEqual(sys.path, original)

    def test_cli_selection_and_release_clusters_are_rejected(self) -> None:
        for option in ("--target=aarch64-apple-darwin", "--target", "--profile=custom",
                       "--config=build.target=host-tuple", "--manifest-path=other/Cargo.toml",
                       "-Cother", "-qr", "-rq"):
            with self.subTest(option=option), self.assertRaises(RuntimeError):
                guard.validate_build_profile("debug", ["cargo", "build", option])

    def test_existing_standard_debug_and_release_commands_are_admitted(self) -> None:
        for profile, command in (("debug", ["rtk", "cargo", "build", "--locked", "-p", "consumer"]),
                                 ("debug", ["rtk", "proxy", "cargo", "build"]),
                                 ("release", ["cargo", "build", "--release"]),
                                 ("release", ["cargo", "build", "-r"])):
            guard.validate_build_profile(profile, command)

    def assert_effects_blocked(self, expected: str, command: list[str] | None = None) -> None:
        args = argparse.Namespace(target=None, target_dir=None, retain_profile=[],
                                  retain_build_profile="debug", protected_ref=[],
                                  command=command or ["cargo", "build"])
        self.environment["CARGO_CAPACITY_GUARD_ROOT"] = str(self.root)
        with patch.dict(os.environ, self.environment, clear=True), \
                patch.object(guard, "resolve_target", side_effect=AssertionError("metadata called")), \
                patch.object(guard, "GuardLock", side_effect=AssertionError("guard lock called")), \
                patch.object(guard, "ProfileLocks", side_effect=AssertionError("profile lock called")), \
                patch.object(guard, "evict_artifacts", side_effect=AssertionError("cleanup called")), \
                patch.object(subprocess, "run", side_effect=AssertionError("child called")):
            with self.assertRaisesRegex(RuntimeError, expected):
                guard.run(args)

    def test_unknown_target_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        self.environment["CARGO_BUILD_TARGET"] = "aarch64-apple-darwin"
        self.assert_effects_blocked("environment target selection")

    def test_config_target_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        self.config(self.root / ".cargo", '[build]\ntarget="aarch64-apple-darwin"\n')
        self.assert_effects_blocked("configured or unknown target selection")

    def test_cli_target_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        self.assert_effects_blocked("profile/directory/config overrides", ["cargo", "build", "--target=host-tuple"])


    def test_explicit_toolchain_rejected_across_supported_prefixes_and_argument_scope(self) -> None:
        for prefix in (["cargo"], ["rtk", "cargo"], ["rtk", "proxy", "cargo"], ["/usr/bin/cargo"]):
            for arguments in (["+nightly", "build"], ["build", "+nightly"]):
                with self.subTest(prefix=prefix, arguments=arguments), self.assertRaises(RuntimeError):
                    guard.validate_build_profile("debug", [*prefix, *arguments])

    def test_unstable_options_rejected_in_all_argument_positions_and_forms(self) -> None:
        for prefix in (["cargo"], ["rtk", "cargo"], ["rtk", "proxy", "cargo"]):
            for options in (["-Zbuild-dir"], ["-Z", "build-dir"], ["-Z=build-dir"], ["-qZbuild-dir"]):
                for arguments in ([*options, "build"], ["build", *options]):
                    with self.subTest(prefix=prefix, arguments=arguments), self.assertRaises(RuntimeError):
                        guard.validate_build_profile("debug", [*prefix, *arguments])

    def test_toolchain_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        for prefix in (["cargo"], ["rtk", "cargo"], ["rtk", "proxy", "cargo"]):
            self.assert_effects_blocked("toolchain or unstable options", [*prefix, "+nightly", "build"])

    def test_unstable_option_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        for prefix in (["cargo"], ["rtk", "cargo"], ["rtk", "proxy", "cargo"]):
            self.assert_effects_blocked("toolchain or unstable options", [*prefix, "build", "-Zbuild-dir"])


    def test_nonempty_unstable_config_is_rejected(self) -> None:
        for setting in ('build-dir-new-layout=true', 'fine-grain-locking=true', 'unknown-setting=false'):
            self.config(self.root / ".cargo", '[unstable]\n' + setting + '\n')
            self.rejected()

    def test_empty_unstable_config_is_admitted(self) -> None:
        self.config(self.root / ".cargo", '[unstable]\n')
        guard.validate_build_selection(self.root, self.environment)

    def test_unstable_config_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        self.config(self.root / ".cargo", '[unstable]\nbuild-dir-new-layout=true\n')
        self.assert_effects_blocked("unstable Cargo configuration")


    def fake_cargo(self, directory: Path) -> Path:
        if os.name == "nt":
            self.skipTest("POSIX fake executable permission fixture")
        directory.mkdir(parents=True, exist_ok=True)
        cargo = directory / "cargo"
        cargo.write_text("fixture executable; never invoked\n")
        cargo.chmod(0o755)
        return cargo

    def test_metadata_child_executable_matches_with_existing_prefixes(self) -> None:
        cargo = self.fake_cargo(self.root / "bin")
        with patch.dict(os.environ, {"PATH": str(cargo.parent), "RUSTUP_TOOLCHAIN": "same-env"}, clear=True):
            for prefix in (["cargo"], ["rtk", "cargo"], ["rtk", "proxy", "cargo"], [str(cargo)]):
                guard.validate_build_executable(self.root, [*prefix, "build"])

    def test_alternative_absolute_cargo_is_rejected(self) -> None:
        expected = self.fake_cargo(self.root / "bin")
        other = self.fake_cargo(self.root / "other")
        with patch.dict(os.environ, {"PATH": str(expected.parent)}, clear=True), self.assertRaises(RuntimeError):
            guard.validate_build_executable(self.root, [str(other), "build"])

    def test_missing_cargo_resolution_is_rejected(self) -> None:
        with patch.dict(os.environ, {"PATH": str(self.root / "absent")}, clear=True), self.assertRaises(RuntimeError):
            guard.validate_build_executable(self.root, ["cargo", "build"])

    def test_relative_path_entries_bind_child_root(self) -> None:
        cargo = self.fake_cargo(self.root / "bin")
        with patch.dict(os.environ, {"PATH": "bin"}, clear=True):
            guard.validate_build_executable(self.root, [str(cargo), "build"])
            guard.validate_build_executable(self.root, ["bin/cargo", "build"])

    def test_dot_relative_executable_binds_child_root(self) -> None:
        cargo = self.fake_cargo(self.root / "bin")
        with patch.dict(os.environ, {"PATH": "bin"}, clear=True):
            guard.validate_build_executable(self.root, ["./bin/cargo", "build"])
        self.fake_cargo(self.root)
        with patch.dict(os.environ, {"PATH": str(cargo.parent)}, clear=True), self.assertRaises(RuntimeError):
            guard.validate_build_executable(self.root, ["./cargo", "build"])

    def test_dot_relative_executable_matching_path_cargo_is_admitted(self) -> None:
        self.fake_cargo(self.root / "bin")
        with patch.dict(os.environ, {"PATH": "bin"}, clear=True):
            guard.validate_build_executable(self.root, ["./bin/cargo", "build"])

    def test_alternative_cargo_rejected_before_metadata_locks_cleanup_or_child(self) -> None:
        expected = self.fake_cargo(self.root / "bin")
        other = self.fake_cargo(self.root / "other")
        self.environment["PATH"] = str(expected.parent)
        self.assert_effects_blocked("Cargo executable differs from metadata", [str(other), "build"])


if __name__ == "__main__":
    unittest.main()
