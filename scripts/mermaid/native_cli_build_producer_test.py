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
import cargo_capacity_guard as capacity_guard
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

    def test_control_is_loaded_from_the_repository_inventory(self) -> None:
        manifest_path = Path(bindings.__file__).with_name("native-cli-managed-control-inputs.json")
        manifest = json.loads(manifest_path.read_text())
        self.assertEqual(manifest["schema"], "krr-native-cli-managed-control-inputs-v1")
        self.assertEqual(bindings.CONTROL, tuple(manifest["control"]))
        self.assertEqual(len(bindings.CONTROL), 27)
        self.assertIn(str(manifest_path.relative_to(Path(__file__).parents[2])), bindings.CONTROL)

    def test_control_manifest_rejects_noncanonical_inventory(self) -> None:
        manifest_path = Path(bindings.__file__).with_name("native-cli-managed-control-inputs.json")
        valid = json.loads(manifest_path.read_text())
        cases = {
            "unknown": {**valid, "unexpected": True},
            "schema": {**valid, "schema": "other"},
            "empty": {**valid, "control": []},
            "duplicate": {**valid, "control": [*valid["control"][:-1], valid["control"][0]]},
            "absolute": {**valid, "control": ["/tmp/escape", *valid["control"][1:]]},
            "traversal": {**valid, "control": ["scripts/../escape", *valid["control"][1:]]},
            "windows drive": {**valid, "control": ["C:/escape", *valid["control"][1:]]},
            "backslash": {**valid, "control": ["scripts\\escape", *valid["control"][1:]]},
            "missing self": {**valid, "control": [
                *(name for name in valid["control"] if name != bindings.CONTROL_MANIFEST_NAME),
                "scripts/mermaid/other.json",
            ]},
        }
        for name, value in cases.items():
            with self.subTest(case=name), self.assertRaises(RuntimeError):
                bindings.parse_control_manifest(value)

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
        report = self._fresh_capacity_report(target, 1024)
        attempt, _lifecycle, binding = self._preflight_fixture("d", report)
        measured, observed = self._measured_sequence(attempt, [(report, 0, "")])
        original = measured

        def checked(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(kwargs["check"], False)
            self.assertNotIn(guard.REQUEST_ENV, cast(dict[str, str], kwargs["env"]))
            return original(argv, **kwargs)

        with patch.object(owner, "_measurement_command", side_effect=checked):
                evidence = producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(len(observed), 1)
        self.assertIn("--require-capacity", observed[0])
        self.assertNotIn("build", observed[0])
        self.assertEqual(evidence["actualExit"], 0)
        self.assertEqual(cast(JsonObject, evidence["report"])["cleanup_status"], "not-needed")
        self.assertIsNone(evidence["cleanupObservation"])
        self.assertIsNone(evidence["refreshObservation"])
        self.assertTrue((attempt / "capacity-preflight.json").is_file())

    def test_capacity_preflight_reobserves_once_after_cleanup_and_preserves_both_runs(self) -> None:
        target = self.root / "target"
        policy = guard.policy(target)
        threshold = cast(int, policy["baseline_bytes"]) * cast(int, policy["factor"])
        initial = self._cleanup_capacity_report(target, threshold + 1000, threshold - 1000)
        fresh = self._fresh_capacity_report(target, threshold - 1000)
        attempt, _lifecycle, binding = self._preflight_fixture("h", initial)
        measured, calls = self._measured_sequence(attempt, [(initial, 0, "stderr-0"), (fresh, 0, "stderr-1")])

        with patch.object(owner, "_measurement_command", side_effect=measured):
            evidence = producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[0], calls[1])
        self.assertEqual(cast(JsonObject, evidence["report"])["current_bytes"], threshold - 1000)
        cleanup = json.loads((attempt / "capacity-preflight-cleanup-observation.json").read_text())
        refresh = json.loads((attempt / "capacity-preflight-refresh-observation.json").read_text())
        self.assertEqual(cleanup["actualExit"], 0)
        self.assertEqual(cleanup["report"]["current_bytes"], threshold + 1000)
        self.assertEqual(cleanup["report"]["post_clean_bytes"], threshold - 1000)
        self.assertEqual(guard.file_record(self._sequence_stdout(attempt, 0)), cleanup["stdout"])
        self.assertEqual(guard.file_record(self._sequence_stdout(attempt, 1)), evidence["stdout"])
        self.assertEqual(evidence["cleanupObservation"], guard.file_record(
            attempt / "capacity-preflight-cleanup-observation.json"))
        self.assertEqual(evidence["refreshObservation"], guard.file_record(
            attempt / "capacity-preflight-refresh-observation.json"))
        self.assertEqual(guard.file_record(self._sequence_stdout(attempt, 1)), refresh["stdout"])
        self.assertEqual(cleanup["actualExit"], 0)
        self.assertEqual(evidence["actualExit"], 0)

    def test_capacity_preflight_rejects_second_cleanup_or_invalid_fresh_report(self) -> None:
        target = self.root / "target"
        policy = guard.policy(target)
        threshold = cast(int, policy["baseline_bytes"]) * cast(int, policy["factor"])
        initial = self._cleanup_capacity_report(target, threshold + 1000, threshold - 1000)
        missing_clean_status = self._fresh_capacity_report(target, threshold - 1000)
        del missing_clean_status["cleanup_status"]
        negative_cases = [
            (initial, {**self._fresh_capacity_report(target, threshold - 1000),
                       "removed": ["second cleanup"]}, 2),
            (initial, {**self._fresh_capacity_report(target, threshold - 1000),
                       "cleanup_attempts": [{"attempt": 1}]}, 2),
            (initial, self._fresh_capacity_report(target, threshold + 1000), 2),
            (initial, {**self._fresh_capacity_report(target, threshold - 1000),
                       "metric_available": False}, 2),
            (initial, missing_clean_status, 2),
            ({key: value for key, value in self._fresh_capacity_report(target, threshold - 1000).items()
              if key != "cleanup_status"}, None, 1),
            ({**initial, "cleanup_status": "failed"}, None, 1),
            ({**initial, "cleanup_status": "dry-run"}, None, 1),
            ({**initial, "cleanup_status": "unexpected"}, None, 1),
        ]
        for index, (first, fresh, expected_calls) in enumerate(negative_cases):
            with self.subTest(case=index):
                attempt, _lifecycle, binding = self._preflight_fixture(str(index + 1), first)
                sequence = [(first, 0, "")]
                if fresh is not None:
                    sequence.append((fresh, 0, ""))
                measured, calls = self._measured_sequence(attempt, sequence)

                with patch.object(owner, "_measurement_command", side_effect=measured):
                    with self.assertRaises(RuntimeError):
                        producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
                self.assertEqual(len(calls), expected_calls)
                self.assertTrue((attempt / "capacity-preflight-cleanup-observation.json").is_file())
                first_sidecar = json.loads(
                    (attempt / "capacity-preflight-cleanup-observation.json").read_text()
                )
                self.assertEqual(first_sidecar["actualExit"], 0)
                self.assertEqual(guard.file_record(
                    Path(first_sidecar["stdout"]["path"])), first_sidecar["stdout"])
                if fresh is not None:
                    self.assertTrue((attempt / "capacity-preflight-refresh-observation.json").is_file())
                    fresh_sidecar = json.loads(
                        (attempt / "capacity-preflight-refresh-observation.json").read_text()
                    )
                    self.assertEqual(fresh_sidecar["actualExit"], 0)
                    self.assertEqual(guard.file_record(
                        Path(fresh_sidecar["stdout"]["path"])), fresh_sidecar["stdout"])

    def test_capacity_preflight_does_not_retry_nonzero_cleanup_run(self) -> None:
        target = self.root / "target"
        policy = guard.policy(target)
        threshold = cast(int, policy["baseline_bytes"]) * cast(int, policy["factor"])
        report = self._cleanup_capacity_report(self.root / "target", threshold + 1000, threshold - 1000)
        attempt, _lifecycle, binding = self._preflight_fixture("j", report)
        measured, calls = self._measured_sequence(attempt, [(report, 1, "failed")])

        with patch.object(owner, "_measurement_command", side_effect=measured):
            with self.assertRaisesRegex(RuntimeError, "capacity preflight did not admit"):
                producer._capacity_preflight(cast(bindings.Bindings, binding), self.root, attempt)
        self.assertEqual(len(calls), 1)
        cleanup = json.loads((attempt / "capacity-preflight-cleanup-observation.json").read_text())
        self.assertEqual(cleanup["actualExit"], 1)

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
        target.mkdir(exist_ok=True)
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

    def _fresh_capacity_report(self, target: Path, current: int) -> JsonObject:
        return {**guard.policy(target), "metric_available": True, "current_bytes": current,
                "over_capacity": current > cast(int, guard.policy(target)["baseline_bytes"])
                * cast(int, guard.policy(target)["factor"]), "removed": [],
                "cleanup_status": "not-needed"}

    def _cleanup_capacity_report(self, target: Path, current: int, post_clean: int) -> JsonObject:
        policy = guard.policy(target)
        threshold = cast(int, policy["baseline_bytes"]) * cast(int, policy["factor"])
        soft_goal = cast(int, policy["baseline_bytes"])
        shortfall: JsonObject = {"attempt": 1, "maxsize_bytes": soft_goal, "exit_code": 1,
            "output": "insufficient closed inactive artifact capacity; active profiles retained",
            "classification": "closed-inactive-shortfall", "no_mutation": True,
            "staged_paths": [], "invalidated_profiles": [], "pre_clean_bytes": current,
            "post_clean_bytes": current, "actual_removed_bytes": 0, "eviction_details": {},
            "dry_run": False}
        cleanup: JsonObject = {"attempt": 2, "maxsize_bytes": threshold, "exit_code": 0,
            "output": "finite artifact eviction after soft-goal shortfall", "pre_clean_bytes": current,
            "post_clean_bytes": post_clean, "actual_removed_bytes": max(current - post_clean, 0),
            "dry_run": False}
        report: JsonObject = {**policy, "metric_available": True, "current_bytes": current,
            "over_capacity": current > threshold, "removed": ["finite artifact eviction"],
            "cleanup_status": "incomplete", "clean_exitcode": 0,
            "post_clean_bytes": post_clean, "actual_removed_bytes": max(current - post_clean, 0),
            "cleanup_goal_bytes": soft_goal, "cleanup_goal_reached": post_clean <= soft_goal,
            "remaining_over_capacity": post_clean > threshold,
            "cleanup_attempts": [shortfall, cleanup], "finite_cleanup": {},
            "clean_output": "attempt 1: soft-goal shortfall; attempt 2: hard-cap cleanup"}
        self.assertTrue(capacity_guard._safe_incomplete_cleanup_below_hardcap(report))
        return report

    def _sequence_stdout(self, attempt: Path, index: int) -> Path:
        return attempt / f"capacity-sequence-{index}.raw"

    def _measured_sequence(
        self, attempt: Path, sequence: list[tuple[JsonObject, int, str]]
    ) -> tuple[Callable[..., subprocess.CompletedProcess[bytes]], list[list[str]]]:
        calls: list[list[str]] = []

        def measured(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            index = len(calls)
            calls.append(argv.copy())
            report, exit_code, error = sequence[index]
            stdout = self._sequence_stdout(attempt, index)
            stderr = attempt / f"capacity-sequence-{index}.err"
            stdout.write_text(json.dumps(report) + "\n")
            stderr.write_text(error)
            lifecycle = attempt / f"capacity-sequence-{index}.json"
            lifecycle.write_text(json.dumps({"exitCode": exit_code, "groupClosed": True,
                "rawFdsClosed": True, "raw": [{"path": str(stdout)}, {"path": str(stderr)}]}))
            cast(Callable[[Path], None], kwargs["lifecycle_sink"])(lifecycle)
            return subprocess.CompletedProcess(argv, exit_code, stdout.read_bytes(), stderr.read_bytes())

        return measured, calls

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

    def test_rust_facts_host_bootstrap_precedes_request_tokens(self) -> None:
        calls: list[tuple[list[str], str, str]] = []
        marker = cast(owner.NativeCliCommandEvidence, object())

        def run(argv: list[str], role: owner.MeasurementRole, label: str,
                *, environment: dict[str, str] | None = None) -> owner.NativeCliCommandEvidence:
            self.assertIsNone(environment)
            calls.append((argv, role, label))
            return marker

        binding = SimpleNamespace(
            child_environment={"PATH": "/bin"},
            tools={"rtk": {"path": "/bin/rtk"}, "just": {"path": "/bin/just"}},
            run=run,
        )
        result = producer._prebuild_rust_facts_host(cast(bindings.Bindings, binding))
        self.assertIs(result, marker)
        self.assertEqual(calls, [(["/bin/rtk", "proxy", "/bin/just", "rust-facts-host-prebuild"],
                                  "build", "rust-facts-host-prebuild")])
        binding.child_environment[guard.REQUEST_ENV] = "/tmp/request.json"
        with self.assertRaisesRegex(RuntimeError, "must precede the CLI request"):
            producer._prebuild_rust_facts_host(cast(bindings.Bindings, binding))
        self.assertEqual(len(calls), 1)


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
            host_route = (
                "rust-facts-host-prebuild:\n"
                "    bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare "
                "{{quote(REPO_ROOT)}} "
                '{{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}\n'
            )
            (root / "Justfile").write_text(
                text.replace(host_route, host_route + "    echo unexpected\n")
            )
            with self.assertRaisesRegex(RuntimeError, "fixed Rust facts host prebuild route differs"):
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
