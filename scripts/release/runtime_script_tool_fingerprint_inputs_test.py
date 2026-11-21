"""Diagnostic inventory tests; no result is admitted to the production cache."""
from __future__ import annotations

import json
import hashlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_tool_fingerprint_inputs as inputs


class ToolFingerprintInputsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve() / "repo"
        self.root.mkdir()
        self.home = Path(self.temporary.name).resolve() / "home"
        self.home.mkdir()
        for name in inputs.SOURCE_FILES:
            self.write(name, b"source")
        reviewed = {name: hashlib.sha256(b"source").hexdigest() for name in inputs.SOURCE_FILES[:3]}
        reviewed_patch = patch.object(inputs, "REVIEWED_SOURCES", reviewed)
        reviewed_patch.start()
        self.addCleanup(reviewed_patch.stop)
        self.write("scripts/release/adapter.py", b"source")
        for name in ("node_modules/zod/index.js", "node_modules/@moonrepo/cli/utils.js",
                     "node_modules/@moonrepo/cli/package.json", "node_modules/@moonrepo/core-test/moon",
                     "node_modules/@moonrepo/core-test/package.json"):
            self.write(name, b"package")
        libc = None
        if sys.platform == "linux":
            libc = str(self.write("node_modules/detect-libc/index.js", b"package"))
        self.resolved = {"zod": str(self.root / "node_modules/zod/index.js"),
                         "utils": str(self.root / "node_modules/@moonrepo/cli/utils.js"),
                         "cli": str(self.root / "node_modules/@moonrepo/cli/package.json"),
                         "moon": str(self.root / "node_modules/@moonrepo/core-test/moon"),
                         "libc": libc, "bun": str(self.write("tools/bun", b"bun")),
                         "platform": "linux" if sys.platform == "linux" else "darwin",
                         "arch": "fixture"}
        self.environment = {"HOME": str(self.home), "PATH": "/fixture/bin"}

    def write(self, name: str, content: bytes) -> Path:
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return path

    def collect(self) -> dict[str, object]:
        process = subprocess.CompletedProcess([], 0, json.dumps(self.resolved).encode(), b"")
        with patch.object(subprocess, "run", return_value=process):
            return inputs.collect(self.root, self.environment)

    def test_inventory_changes_for_transitive_package_addition_and_bytes(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        self.assertFalse(first["cacheable"])
        self.write("node_modules/zod/nested/module.js", b"first")
        second = self.collect()
        self.write("node_modules/zod/nested/module.js", b"second")
        third = self.collect()
        self.assertNotEqual(first["inventorySha256"], second["inventorySha256"])
        self.assertNotEqual(second["inventorySha256"], third["inventorySha256"])

    def test_unreviewed_test_source_change_is_rejected(self) -> None:
        self.write(inputs.SOURCE_FILES[0], b"new dependency")
        self.assertEqual(self.collect()["missReason"], "reviewed_source_changed")

    def test_quality_inputs_bind_package_bytes_and_environment_without_disclosure(self) -> None:
        for name in (
            "@swc/core/index.js", "@swc/types/index.js", "@swc/counter/index.js",
            "@swc/core-test/native.node", "typescript/index.js",
            "@typescript/typescript-test/compiler",
        ):
            self.write(f"node_modules/{name}", b"original")
        environment = {"HOME": str(self.home), "PATH": "/bin", "QUALITY_SECRET": "private-value"}
        with (patch.object(inputs, "_quality_git", return_value="git-identity"),
              patch.object(inputs, "_quality_python", return_value="python-identity"),
              patch.object(tempfile, "gettempdir", return_value=str(self.home))):
            before = inputs.collect_quality_inputs(self.root, environment)
            self.assertNotIn("private-value", before)
            for scope in ("run-one", "run-two"):
                with self.subTest(cleanup_scope=scope):
                    self.assertEqual(before, inputs.collect_quality_inputs(
                        self.root, {**environment, "WORKSPACE_RUST_CLEANUP_SCOPE": scope}))
            for change in ({"WORKSPACE_RUST_CLEANUP_SCOPE_OTHER": "run-one"},
                           {"CARGO_CAPACITY_PROTECTED_REFS": "protected"}):
                with self.subTest(changed_key=next(iter(change))):
                    self.assertNotEqual(before, inputs.collect_quality_inputs(self.root, {**environment, **change}))
            changed_env = inputs.collect_quality_inputs(self.root, {**environment, "QUALITY_SECRET": "other"})
            self.assertNotEqual(before, changed_env)
            self.write("node_modules/@swc/core-test/native.node", b"changed")
            self.assertNotEqual(before, inputs.collect_quality_inputs(self.root, environment))
            self.write("bunfig.toml", b"preload = []")
            with self.assertRaisesRegex(ValueError, "configuration_or_preload_not_admitted"):
                inputs.collect_quality_inputs(self.root, environment)

    def test_quality_inputs_reject_missing_native_package(self) -> None:
        with self.assertRaises((OSError, ValueError)):
            inputs._quality_packages(self.root)

    def test_quality_git_hashes_effective_configuration_without_disclosure(self) -> None:
        binary = self.write("git", b"fixture git")
        environment = {"PATH": str(self.root)}
        first = subprocess.CompletedProcess([], 0, stdout=b"secret-one", stderr=b"")
        second = subprocess.CompletedProcess([], 0, stdout=b"secret-two", stderr=b"")
        with (patch.object(shutil, "which", return_value=str(binary)),
              patch.object(inputs, "_quality_git_files", return_value=[]),
              patch.object(inputs, "_quality_git_conditions"),
              patch.object(subprocess, "run", side_effect=[first, second])):
            before = inputs._quality_git(self.root, environment)
            after = inputs._quality_git(self.root, environment)
        self.assertNotEqual(before, after)
        self.assertNotIn("secret", before)

    def test_quality_git_rejects_conditional_includes_without_disclosure(self) -> None:
        result = subprocess.CompletedProcess([], 0, stdout=b"includeif.onbranch:private.path\0", stderr=b"")
        with patch.object(subprocess, "run", return_value=result):
            with self.assertRaisesRegex(ValueError, "quality_git_conditional_include_not_admitted"):
                inputs._quality_git_conditions(self.root / "git", self.root, {"HOME": str(self.home)})

    def test_quality_git_conditions_only_accept_disjoint_absolute_anchors(self) -> None:
        temp = self.root / "temporary"
        roots = {temp, temp.resolve(strict=False)}
        environment = {"HOME": str(self.home)}
        workspace = (self.home / "work").as_posix()
        temp_path = temp.as_posix()
        anchor = Path(temp.anchor).as_posix()
        for condition in (f"gitdir:{workspace}/**", "gitdir/i:~/work/**"):
            inputs._quality_git_condition(f"includeif.{condition}.path", roots, environment)
        for condition in (f"gitdir:{temp_path}/**", f"gitdir/i:{temp_path.upper()}/**",
                          f"gitdir:{temp_path}/child/**", f"gitdir:{anchor}**",
                          "gitdir:relative/**", f"gitdir:{workspace}/../tmp/**", "onbranch:main"):
            with self.subTest(condition=condition):
                with self.assertRaisesRegex(ValueError, "quality_git_conditional_include_not_admitted"):
                    inputs._quality_git_condition(f"includeif.{condition}.path", roots, environment)

    def test_quality_git_files_bind_external_ignore_and_attributes(self) -> None:
        ignore = self.home / "external-ignore"
        attributes = self.home / "external-attributes"
        environment = {"HOME": str(self.home)}
        no_value = subprocess.CompletedProcess([], 1, stdout=b"", stderr=b"")
        values = [subprocess.CompletedProcess([], 0, stdout=bytes(path) + b"\0", stderr=b"")
                  for path in (ignore, attributes)]
        with patch.object(subprocess, "run", side_effect=values * 4 + [no_value, no_value] * 2):
            before = inputs._quality_git_files(self.root / "git", self.root, environment)
            ignore.write_text("scripts/helper.mts\n", encoding="utf8")
            added = inputs._quality_git_files(self.root / "git", self.root, environment)
            ignore.write_text("changed\n", encoding="utf8")
            changed = inputs._quality_git_files(self.root / "git", self.root, environment)
            attributes.write_text("*.mts text\n", encoding="utf8")
            changed_attributes = inputs._quality_git_files(self.root / "git", self.root, environment)
            default = inputs._quality_git_files(self.root / "git", self.root, environment)
            default_ignore = self.home / ".config/git/ignore"
            default_ignore.parent.mkdir(parents=True)
            default_ignore.write_text("scripts/helper.mts\n", encoding="utf8")
            changed_default = inputs._quality_git_files(self.root / "git", self.root, environment)
        self.assertNotEqual(before, added)
        self.assertNotEqual(added, changed)
        self.assertNotEqual(changed, changed_attributes)
        self.assertNotEqual(changed_attributes, default)
        self.assertNotEqual(default, changed_default)
        self.assertNotIn("scripts/helper.mts", str(added))

    def test_quality_git_files_reject_relative_config_path(self) -> None:
        relative = subprocess.CompletedProcess([], 0, stdout=b"relative-ignore\0", stderr=b"")
        with patch.object(subprocess, "run", return_value=relative):
            with self.assertRaisesRegex(ValueError, "quality_git_path_not_absolute"):
                inputs._quality_git_files(self.root / "git", self.root, {"HOME": str(self.home)})

    def test_quality_ruff_rejects_external_config_in_temp_and_xdg(self) -> None:
        environment = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.home)}
        with patch.object(tempfile, "gettempdir", return_value=str(self.home)):
            inputs._quality_ruff_absence(self.root, self.home, environment)
            for relative in ("ruff.toml", "ruff/pyproject.toml"):
                config = self.home / relative
                config.parent.mkdir(parents=True, exist_ok=True)
                config.write_text("secret fixture", encoding="utf8")
                with self.assertRaisesRegex(ValueError, "quality_external_ruff_config_not_admitted"):
                    inputs._quality_ruff_absence(self.root, self.home, environment)
                config.unlink()

    def test_quality_python_binds_path_selected_executable(self) -> None:
        binary = self.write("python3", b"first")
        with patch.object(shutil, "which", return_value=str(binary)):
            before = inputs._quality_python({"PATH": str(self.root)})
            binary.write_bytes(b"second")
            self.assertNotEqual(before, inputs._quality_python({"PATH": str(self.root)}))

    def test_configuration_and_unknown_runtime_override_are_rejected(self) -> None:
        config = self.write("bunfig.toml", b"preload = []")
        self.assertEqual(self.collect()["missReason"], "configuration_or_preload_not_admitted")
        config.unlink()
        self.environment["BUN_UNRECOGNIZED"] = "1"
        self.assertEqual(self.collect()["missReason"], "unsupported_override")

    def test_xdg_config_and_mode_specific_env_are_rejected(self) -> None:
        xdg = self.root / "xdg"
        xdg.mkdir()
        self.environment["XDG_CONFIG_HOME"] = str(xdg)
        config = xdg / ".bunfig.toml"
        config.write_text("preload = []")
        self.assertEqual(self.collect()["missReason"], "configuration_or_preload_not_admitted")
        config.unlink()
        for mode in ("test", "development", "production"):
            self.environment["NODE_ENV"] = mode
            for suffix in ("", ".local"):
                with self.subTest(mode=mode, suffix=suffix):
                    path = self.write(f".env.{mode}{suffix}", b"fixture")
                    self.assertEqual(self.collect()["missReason"], "configuration_or_preload_not_admitted")
                    path.unlink()

    def test_node_repl_metadata_is_bound_without_exposing_its_value(self) -> None:
        name = "NODE_REPL_TRUSTED_BROWSER_CLIENT_SHA256S"
        self.environment[name] = "first-fixture-value"
        first = self.collect()
        self.environment[name] = "second-fixture-value"
        second = self.collect()
        self.assertTrue(first["inputsCollected"])
        self.assertFalse(first["cacheable"])
        self.assertNotEqual(first["inventorySha256"], second["inventorySha256"])
        self.assertNotIn("first-fixture-value", json.dumps(first))
        self.assertNotIn("second-fixture-value", json.dumps(second))

    def test_diagnostics_expose_only_aggregate_digests_and_key_names(self) -> None:
        self.environment["CUSTOM_SECRET"] = "private-fixture"
        report = self.collect()
        self.assertTrue(report["inputsCollected"])
        self.assertNotIn("private-fixture", json.dumps(report))
        components = report["componentDigests"]
        self.assertIsInstance(components, dict)
        if not isinstance(components, dict):
            self.fail("component digest record missing")
        self.assertEqual(set(components), {"sources", "packages", "resolver", "bun",
                                           "configurationAbsence", "adapter", "semanticEnvironment"})
        names = report["semanticEnvironmentNames"]
        self.assertIsInstance(names, list)
        if not isinstance(names, list):
            self.fail("environment key names missing")
        self.assertIn("CUSTOM_SECRET", names)

    def test_orchestration_metadata_is_not_semantic_input(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        for name in (
            "MOON_TASK_HASH", "MOON_CACHE", "MOON_CACHE_CAS_VERIFY_INTEGRITY",
            "MOON_EXPERIMENT_CAS_OUTPUTS_CACHE", "CARGO_CAPACITY_PROTECTED_REFS",
        ):
            with self.subTest(name=name):
                self.environment[name] = "orchestration-fixture"
                second = self.collect()
                self.assertEqual(first["inventorySha256"], second["inventorySha256"])
                del self.environment[name]

    def test_cleanup_scope_is_not_a_tool_fingerprint_input(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        for scope in ("run-one", "run-two"):
            with self.subTest(cleanup_scope=scope):
                self.environment["WORKSPACE_RUST_CLEANUP_SCOPE"] = scope
                self.assertEqual(first["inventorySha256"], self.collect()["inventorySha256"])
        del self.environment["WORKSPACE_RUST_CLEANUP_SCOPE"]
        self.environment["WORKSPACE_RUST_CLEANUP_SCOPE_OTHER"] = "run-one"
        self.assertNotEqual(first["inventorySha256"], self.collect()["inventorySha256"])

    def test_moon_color_zero_matches_absent_semantic_environment(self) -> None:
        without_color = self.collect()
        self.environment["MOON_COLOR"] = "0"
        with_color = self.collect()

        self.assertTrue(without_color["inputsCollected"])
        self.assertTrue(with_color["inputsCollected"])
        without_components = without_color["componentDigests"]
        with_components = with_color["componentDigests"]
        self.assertIsInstance(without_components, dict)
        self.assertIsInstance(with_components, dict)
        if not isinstance(without_components, dict) or not isinstance(with_components, dict):
            self.fail("component digest record missing")
        self.assertEqual(without_components["semanticEnvironment"],
                         with_components["semanticEnvironment"])
        self.assertEqual(without_color["semanticEnvironmentNames"],
                         with_color["semanticEnvironmentNames"])
        self.assertEqual(without_color["inventorySha256"], with_color["inventorySha256"])

    def test_dispatcher_roles_are_not_semantic_inputs(self) -> None:
        name = "KRR_RUNTIME_CAS_OPERATION"
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        for value in ("identity:runtime-unused-tool-fingerprint", "validate-all", "ensure-all"):
            with self.subTest(role=value):
                self.environment[name] = value
                report = self.collect()
                self.assertEqual(first["inventorySha256"], report["inventorySha256"])
        del self.environment[name]

    def test_retry_bookkeeping_is_not_semantic_input(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        for attempt, total in (("0", "1"), ("1", "3")):
            with self.subTest(attempt=attempt, total=total):
                self.environment["MOON_TASK_RETRY_ATTEMPT"] = attempt
                self.environment["MOON_TASK_RETRY_TOTAL"] = total
                report = self.collect()
                self.assertEqual(first["inventorySha256"], report["inventorySha256"])
        del self.environment["MOON_TASK_RETRY_ATTEMPT"]
        del self.environment["MOON_TASK_RETRY_TOTAL"]

    def test_semantic_and_unknown_environment_changes_are_bound(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        for name, value in (("PATH", "/other/bin"), ("TZ", "Asia/Tokyo"),
                            ("MOON_UNDECLARED", "fixture"), ("CARGO_UNDECLARED", "fixture"),
                            ("MOON_TASK_RETRY_UNKNOWN", "fixture"),
                            ("KRR_UNDECLARED", "fixture")):
            with self.subTest(name=name):
                old = self.environment.get(name)
                self.environment[name] = value
                changed = self.collect()
                self.assertNotEqual(first["inventorySha256"], changed["inventorySha256"])
                if old is None:
                    del self.environment[name]
                else:
                    self.environment[name] = old

    def test_bun_executable_change_changes_inventory(self) -> None:
        first = self.collect()
        self.assertTrue(first["inputsCollected"])
        self.write("tools/bun", b"different executable")
        second = self.collect()
        self.assertNotEqual(first["inventorySha256"], second["inventorySha256"])

    def test_unknown_node_mode_is_rejected(self) -> None:
        self.environment["NODE_ENV"] = "custom"
        self.assertEqual(self.collect()["missReason"], "unsupported_node_env_mode")

    def test_dynamic_loader_overrides_fail_closed_without_exposing_values(self) -> None:
        for name in ("LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "DYLD_CUSTOM"):
            with self.subTest(name=name):
                self.environment[name] = "private-fixture"
                report = self.collect()
                self.assertEqual(report["missReason"], "unsupported_loader_override")
                self.assertFalse(report["cacheable"])
                self.assertNotIn("private-fixture", json.dumps(report))
                del self.environment[name]

    def test_symlink_and_outside_resolver_are_rejected(self) -> None:
        link = self.root / "node_modules/zod/link.js"
        link.symlink_to(self.root / "package.json")
        self.assertEqual(self.collect()["missReason"], "package_symlink_not_admitted")
        link.unlink()
        self.resolved["zod"] = str(self.root / "package.json")
        self.assertEqual(self.collect()["missReason"], "resolver_outside_installed_packages")

    def test_resolver_drift_is_rejected(self) -> None:
        first = subprocess.CompletedProcess([], 0, json.dumps(self.resolved).encode(), b"")
        changed = {**self.resolved, "moon": "different"}
        second = subprocess.CompletedProcess([], 0, json.dumps(changed).encode(), b"")
        with patch.object(subprocess, "run", side_effect=[first, second]):
            result = inputs.collect(self.root, self.environment)
        self.assertEqual(result["missReason"], "resolver_changed_during_inventory")


if __name__ == "__main__":
    unittest.main()
