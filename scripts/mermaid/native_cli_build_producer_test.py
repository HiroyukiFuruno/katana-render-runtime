"""CLI producerの入力境界と既存owner接続を検査する有限単体契約。"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path
from types import SimpleNamespace
from typing import cast
from unittest.mock import patch

import native_cli_build_bindings as bindings
import cargo_capacity_guard_native_cli_observation as guard
from cargo_capacity_guard import JsonObject, validate_build_executable
import produce_native_cli_build_receipt as producer
import produce_html_performance_evidence as owner


class InputAdmissionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "source.rs"
        self.source.write_text("fn main() {}\n")
        self.records = [guard.file_record(self.source)]
        self.expected: JsonObject = {"schema": "krr-native-build-source-manifest-v1",
            "files": self.records, "digest": guard.source_digest(self.records)}

    def test_independently_admitted_current_bytes(self) -> None:
        bindings.admit_source(self.expected, self.records)
        self.source.write_text("fn main() { panic!(); }\n")
        with self.assertRaisesRegex(RuntimeError, "independent expected source differs"):
            bindings.admit_source(self.expected, [guard.file_record(self.source)])

    def test_missing_and_additional_current_source_are_rejected(self) -> None:
        added = self.root / "added.rs"
        added.write_text("new source\n")
        for records in ([], [*self.records, guard.file_record(added)]):
            with self.subTest(records=len(records)), self.assertRaises(RuntimeError):
                bindings.admit_source(self.expected, records)

    def test_manifest_extra_key_and_forged_digest_are_rejected(self) -> None:
        for key, value in (("extra", True), ("digest", "0" * 64)):
            expected = {**self.expected, key: value}
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                bindings.admit_source(expected, self.records)

    def test_fixed_capacity_registry_preserves_role_order_and_identity(self) -> None:
        registry, refs, other = self._capacity_registry()
        result = bindings.capacity_admission(self.root, registry)
        self.assertEqual(result["refs"], refs)
        self.assertEqual(result["environmentValue"], json.dumps([str(self.source), str(other)], separators=(",", ":")))
        other.write_text("changed protected bytes")
        with self.assertRaisesRegex(RuntimeError, "pinned file changed"):
            bindings.capacity_admission(self.root, registry)

    def test_capacity_admission_cli_reuses_full_physical_identity_checks(self) -> None:
        registry, _refs, _other = self._capacity_registry()
        success = self._run_capacity_cli(registry)
        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertEqual(json.loads(success.stdout), bindings.capacity_admission(self.root, registry))
        invalid_rows = []
        masked = json.loads(registry.read_text())
        masked["refs"][0]["mode"] &= 0o7777
        invalid_rows.append(("masked-mode", masked))
        wrong_size = json.loads(registry.read_text())
        wrong_size["refs"][0]["size"] += 1
        invalid_rows.append(("size-mismatch", wrong_size))
        renamed_size = json.loads(registry.read_text())
        renamed_size["refs"][0]["bytes"] = renamed_size["refs"][0].pop("size")
        invalid_rows.append(("renamed-size", renamed_size))
        for name, value in invalid_rows:
            path = self.root / "tmp" / f"{name}.json"
            path.write_text(json.dumps(value))
            with self.subTest(case=name):
                self.assertNotEqual(self._run_capacity_cli(path).returncode, 0)

    def test_source_manifest_symlink_is_not_admitted(self) -> None:
        alias = self.root / "alias"
        alias.symlink_to(self.source)
        with self.assertRaises(RuntimeError):
            guard.file_record(alias)

    def test_capacity_preflight_is_no_child_guard_before_request_environment(self) -> None:
        target = self.root / "target"
        report = {**guard.policy(target), "metric_available": True, "current_bytes": 1024,
                  "over_capacity": False, "removed": ["finite artifact eviction"],
                  "cleanup_status": "completed"}
        attempt, lifecycle, binding = self._preflight_fixture("d", report)
        observed: list[list[str]] = []

        def measured(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            observed.append(argv)
            self.assertEqual(kwargs["check"], False)
            self.assertNotIn(guard.REQUEST_ENV, cast(dict[str, str], kwargs["env"]))
            callback = cast(Callable[[Path], None], kwargs["lifecycle_sink"])
            callback(lifecycle)
            return subprocess.CompletedProcess(argv, 0, (json.dumps(report) + "\n").encode(), b"")

        with patch.object(owner, "_measurement_command", side_effect=measured):
                evidence = producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(len(observed), 1)
        self.assertIn("--require-capacity", observed[0])
        self.assertNotIn("build", observed[0])
        self.assertEqual(evidence["actualExit"], 0)
        self.assertEqual(cast(JsonObject, evidence["report"])["cleanup_status"], "completed")
        self.assertTrue((attempt / "capacity-preflight.json").is_file())

    def test_capacity_preflight_failure_stops_before_cargo(self) -> None:
        attempt = self.root / "tmp" / ("e" * 64)
        attempt.mkdir(parents=True)
        stdout = attempt / "stdout.raw"
        stderr = attempt / "stderr.raw"
        stdout.write_text("{}\n")
        stderr.write_text("capacity unavailable\n")
        lifecycle = attempt / "lifecycle.json"
        lifecycle.write_text(json.dumps({"exitCode": 1, "groupClosed": True,
            "rawFdsClosed": True, "raw": [{"path": str(stdout)}, {"path": str(stderr)}]}))
        binding = SimpleNamespace(tools={"python3": {"path": sys.executable}}, child_environment={})
        cargo_calls: list[object] = []

        def measured(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            cast(Callable[[Path], None], kwargs["lifecycle_sink"])(lifecycle)
            cargo_calls.append(argv)
            return subprocess.CompletedProcess(argv, 1, b"{}\n", b"capacity unavailable\n")

        with patch.object(owner, "_measurement_command", side_effect=measured):
            with self.assertRaisesRegex(RuntimeError, "capacity preflight did not admit"):
                producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(len(cargo_calls), 1)
        self.assertNotIn("krr-build", cast(list[str], cargo_calls[0]))

    def test_capacity_preflight_retains_existing_debug_profile(self) -> None:
        target = self.root / "target"
        report = {**guard.policy(target), "metric_available": True, "current_bytes": 1024,
                  "over_capacity": False, "removed": [], "cleanup_status": "not-needed"}
        attempt, lifecycle, binding = self._preflight_fixture("f", report, retain_debug=True)
        debug_profile = target / "debug"
        observed: list[list[str]] = []

        def measured(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            observed.append(argv)
            cast(Callable[[Path], None], kwargs["lifecycle_sink"])(lifecycle)
            return subprocess.CompletedProcess(argv, 0, (json.dumps(report) + "\n").encode(), b"")

        with patch.object(owner, "_measurement_command", side_effect=measured):
            producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(
            observed[0][observed[0].index("--retain-profile") + 1], str(debug_profile)
        )

    def _preflight_fixture(
        self, token: str, report: JsonObject, *, retain_debug: bool = False
    ) -> tuple[Path, Path, SimpleNamespace]:
        attempt = self.root / "tmp" / (token * 64)
        attempt.mkdir(parents=True)
        target = self.root / "target"
        target.mkdir()
        if retain_debug:
            (target / "debug").mkdir(parents=True)
        stdout = attempt / "stdout.raw"
        stderr = attempt / "stderr.raw"
        stdout.write_text(json.dumps(report) + "\n")
        stderr.write_text("")
        lifecycle = attempt / "lifecycle.json"
        lifecycle.write_text(json.dumps({"exitCode": 0, "groupClosed": True,
            "rawFdsClosed": True, "raw": [{"path": str(stdout)}, {"path": str(stderr)}]}))
        binding = SimpleNamespace(
            tools={"python3": {"path": sys.executable}},
            child_environment={"PATH": os.environ["PATH"], "CARGO_CAPACITY_PROTECTED_REFS": "[]"},
        )
        return attempt, lifecycle, binding

    def test_capacity_preflight_rejects_debug_profile_symlink(self) -> None:
        attempt = self.root / "tmp" / ("g" * 64)
        attempt.mkdir(parents=True)
        target = self.root / "target"
        target.mkdir()
        (target / "debug").symlink_to(self.root / "missing-debug", target_is_directory=True)
        binding = SimpleNamespace(
            tools={"python3": {"path": sys.executable}},
            child_environment={"PATH": os.environ["PATH"], "CARGO_CAPACITY_PROTECTED_REFS": "[]"},
        )
        with self.assertRaisesRegex(RuntimeError, "retained profile"):
            producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)

    def _capacity_registry(self) -> tuple[Path, list[JsonObject], Path]:
        (self.root / "tmp").mkdir(exist_ok=True)
        state = self.root / "tmp/.cargo-capacity-guard.json"
        state.write_text(json.dumps(guard.policy(self.root / "target")))
        other = self.root / "other"
        other.write_text("other protected bytes")
        refs: list[JsonObject] = []
        for path in (self.source, other):
            stat = path.stat()
            refs.append({**guard.file_record(path), "dev": stat.st_dev,
                         "ino": stat.st_ino, "mode": stat.st_mode})
        registry = self.root / "tmp/registry.json"
        registry.write_text(json.dumps({"schema": "krr-protected-refs-successor-v1",
                                       "physicalRefCount": 2, "refs": refs}))
        return registry, refs, other

    @staticmethod
    def _run_capacity_cli(registry: Path) -> subprocess.CompletedProcess[str]:
        cli = Path(bindings.__file__).resolve()
        return subprocess.run([sys.executable, str(cli), "--repo", str(registry.parents[1]),
            "--protected-registry", str(registry)], check=False, capture_output=True, text=True)

    def test_shell_metacharacter_cargo_path_is_rejected_before_any_query(self) -> None:
        attempt = self.root / "tmp" / ("b" * 64)
        attempt.mkdir(parents=True)
        source = self.root / "tmp/expected.json"
        registry = self.root / "tmp/registry.json"
        source.write_text(json.dumps(self.expected))
        registry.write_text("{}")
        with self.assertRaisesRegex(RuntimeError, "unsupported Just shell quoting"):
            bindings.construct_bindings(self.root, attempt, source, "0" * 64, registry,
                "0" * 64, self.root / "tool;unexpected/cargo", self.root / "rustc")

    def test_independent_source_and_registry_hash_must_match_before_tool_selection(self) -> None:
        attempt = self.root / "tmp" / ("a" * 64)
        attempt.mkdir(parents=True)
        source = self.root / "tmp/expected.json"
        source.write_text(json.dumps(self.expected))
        registry = self.root / "tmp/registry.json"
        registry.write_text("{}")
        source_sha = guard.file_record(source)["sha256"]
        self.assertIsInstance(source_sha, str)
        with self.assertRaisesRegex(RuntimeError, "independent source pin differs"):
            bindings.construct_bindings(self.root, attempt, source, "0" * 64, registry,
                                        "0" * 64, self.root / "cargo", self.root / "rustc")
        with self.assertRaisesRegex(RuntimeError, "protected registry SHA differs"):
            bindings.construct_bindings(self.root, attempt, source, str(source_sha), registry,
                                        "0" * 64, self.root / "cargo", self.root / "rustc")

    def test_constructor_binds_direct_toolchain_path_without_mutating_proxy_parent(self) -> None:
        attempt = self.root / "tmp" / ("c" * 64)
        attempt.mkdir(parents=True)
        toolchain = self.root / "toolchain"
        proxy = self.root / "proxy"
        toolchain.mkdir()
        proxy.mkdir()
        for name in ("cargo", "rustc"):
            path = toolchain / name
            path.write_text("controlled unit data; never executed\n")
            path.chmod(0o700)
        rustup = proxy / "rustup"
        rustup.write_text("controlled proxy unit data; never executed\n")
        rustup.chmod(0o700)
        (proxy / "cargo").symlink_to(rustup)
        (self.root / "Justfile").write_bytes((Path(__file__).resolve().parents[2] / "Justfile").read_bytes())
        expected = self.root / "tmp/expected.json"
        expected.write_text(json.dumps(self.expected))
        (self.root / "tmp/.cargo-capacity-guard.json").write_text(json.dumps(guard.policy(self.root / "target")))
        stat = self.source.stat()
        registry = self.root / "tmp/registry.json"
        registry.write_text(json.dumps({"schema": "krr-protected-refs-successor-v1", "physicalRefCount": 1,
            "refs": [{**guard.file_record(self.source), "dev": stat.st_dev, "ino": stat.st_ino, "mode": stat.st_mode}]}))
        parent_path = str(proxy) + os.pathsep + os.environ["PATH"]
        with patch.dict(os.environ, {"PATH": parent_path}):
            result = bindings.construct_bindings(self.root, attempt, expected, str(guard.file_record(expected)["sha256"]),
                registry, str(guard.file_record(registry)["sha256"]), toolchain / "cargo", toolchain / "rustc")
            self.assertEqual(os.environ["PATH"], parent_path)
            self.assertEqual(result.child_environment["PATH"], str(toolchain) + os.pathsep + parent_path)
            self.assertEqual(result.tools["cargo"], guard.file_record(toolchain / "cargo"))
            self.assertEqual(result.tools["rustc"], guard.file_record(toolchain / "rustc"))
            command = [str(toolchain / "cargo"), *guard.BUILD_FLAGS]
            with self.assertRaisesRegex(RuntimeError, "executable differs"):
                validate_build_executable(self.root, command)
            validate_build_executable(self.root, command, result.child_environment)
            with self.assertRaisesRegex(RuntimeError, "executable differs"):
                validate_build_executable(self.root, command, {**result.child_environment, "PATH": parent_path})
            state_path = self.root / "tmp/.cargo-capacity-guard.json"
            for key, invalid in (("metric", "wrong"), ("metric_version", True),
                                 ("baseline_bytes", 2363047937), ("factor", 3), ("target", str(self.root))):
                state = {**guard.policy(self.root / "target"), key: invalid}
                state_path.write_text(json.dumps(state))
                with self.subTest(saved_state=key), patch.object(owner, "observe_native_cli_command") as run:
                    with self.assertRaisesRegex(RuntimeError, "fixed saved capacity state differs"):
                        producer.produce(self.root, expected, str(guard.file_record(expected)["sha256"]),
                            registry, str(guard.file_record(registry)["sha256"]), toolchain / "cargo", toolchain / "rustc", self.source)
                    run.assert_not_called()


class JustEnvironmentTests(unittest.TestCase):
    def test_exact_environment_overlay_and_no_parent_mutation(self) -> None:
        base = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "SHLVL": "0"}
        original = dict(base)
        root = Path.cwd().resolve()
        child = bindings.guard_environment(base, root)
        self.assertEqual(base, original)
        self.assertEqual(child["PWD"], str(root))
        self.assertEqual(child["SHLVL"], "0")
        self.assertEqual(set(child) - set(base), {"PWD", "_"})

    def test_nonempty_just_selectors_and_different_rustflags_reject(self) -> None:
        for key in (*guard.EMPTY_JUST_ENVIRONMENT_KEYS, "RUSTFLAGS"):
            with self.subTest(key=key), patch.dict(os.environ, {key: "not-empty"}), \
                    self.assertRaises(RuntimeError):
                bindings.base_environment([])

    def test_environment_pins_only_passed_values_and_fixed_capacity_paths(self) -> None:
        environment = bindings.base_environment(["/unit/protected"])
        self.assertNotIn("CARGO", environment)
        self.assertEqual(environment["RUSTFLAGS"], "-D warnings")
        self.assertEqual(json.loads(environment["CARGO_CAPACITY_PROTECTED_REFS"]), ["/unit/protected"])
        self.assertTrue(all(environment[key] == "" for key in guard.EMPTY_JUST_ENVIRONMENT_KEYS))

    def test_extra_build_recipe_command_rejects_before_execution(self) -> None:
        source = Path(__file__).resolve().parents[2] / "Justfile"
        text = source.read_text()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "Justfile").write_text(text)
            bindings.validate_just(root)
            proxy_route = 'CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")\n'
            (root / "Justfile").write_text(text.replace(
                proxy_route,
                'CARGO := env_var_or_default("CARGO", RTK_CMD + "cargo")\n',
            ))
            with self.assertRaisesRegex(RuntimeError, "fixed Just route differs"):
                bindings.validate_just(root)
            (root / "Justfile").write_text(text)
            (root / "Justfile").write_text(text.replace("{{CARGO_BUILD}} build -p katana-render-runtime-cli\n",
                "{{CARGO_BUILD}} build -p katana-render-runtime-cli\n    echo unexpected\n"))
            with self.assertRaisesRegex(RuntimeError, "extra Just build command"):
                bindings.validate_just(root)


class CompletionBoundaryTests(unittest.TestCase):
    def test_missing_observation_fields_and_completed_self_assertion_reject(self) -> None:
        invalid: JsonObject = {"schema": guard.OBSERVATION_SCHEMA,
                                       "accepted": True, "status": "cargo-completed"}
        admission = guard.Admission({}, {}, Path("/unit"), {}, [], [],
                                             Path("/unit/state"), "0" * 64, ())
        with self.assertRaisesRegex(RuntimeError, "field set differs"):
            producer.completed_observation(invalid, {}, {}, {}, admission)

    def test_exclusive_receipt_cannot_replace_existing_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "receipt.json"
            original = guard.exclusive_json(path, {"status": "failed"})
            with self.assertRaises(FileExistsError):
                guard.exclusive_json(path, {"status": "completed"})
            self.assertEqual(guard.file_record(path), original)


if __name__ == "__main__":
    unittest.main()
