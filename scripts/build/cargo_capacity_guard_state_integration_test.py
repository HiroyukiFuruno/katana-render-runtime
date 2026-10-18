from __future__ import annotations

import json
import importlib
import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import TYPE_CHECKING

import cargo_capacity_guard as guard
import cargo_capacity_guard_native_cli_observation as observation
import cargo_capacity_guard_state as capacity_state

if TYPE_CHECKING:
    import produce_html_performance_evidence as typed_producer
    import verify_html_performance_evidence as gate
else:
    release_directory = str(Path(__file__).resolve().parents[1] / "release")
    sys.path.insert(0, release_directory)
    typed_producer = importlib.import_module("produce_html_performance_evidence")
    gate = importlib.import_module("verify_html_performance_evidence")


class CapacityStateIntegrationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve() / "repository"
        self.root.mkdir()
        self.target = self.root / "target"
        self.target.mkdir()
        (self.target / "fixture.bin").write_bytes(b"measured target data")
        self.state_path = self.root / "tmp" / "capacity-state.json"
        self.state_path.parent.mkdir()
        self.bootstrap_state_path = self.root / "tmp" / ".cargo-capacity-guard.json"
        self.policy_path = self.root / capacity_state.REFERENCE_POLICY
        self.policy_path.parent.mkdir(parents=True)
        self.write_policy()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_state(self, state: object) -> None:
        self.state_path.write_text(json.dumps(state), encoding="utf-8")

    def write_policy(self, **overrides: object) -> None:
        policy: dict[str, object] = {
            "schema": capacity_state.REFERENCE_SCHEMA,
            "id": capacity_state.REFERENCE_ID,
            "baseline_bytes": capacity_state.REFERENCE_BASELINE_BYTES,
            "factor": capacity_state.REFERENCE_FACTOR,
            "metric": capacity_state.METRIC,
            "metric_version": capacity_state.METRIC_VERSION,
            "baseline_observed_at": capacity_state.REFERENCE_OBSERVED_AT,
            "source_state_sha256": capacity_state.REFERENCE_SOURCE_STATE_SHA256,
        }
        policy.update(overrides)
        self.policy_path.write_text(json.dumps(policy), encoding="utf-8")

    def run_initializer(
        self, *arguments: str, **environment: str
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.pop("CI", None)
        env.pop("GITHUB_ACTIONS", None)
        env.update(environment)
        script = Path(__file__).with_name("initialize_ci_capacity_state.py")
        return subprocess.run(
            [sys.executable, str(script), "--root", str(self.root), *arguments],
            cwd=self.root,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )

    def inspect(self) -> guard.JsonObject:
        return guard.inspect(self.root, self.target, self.state_path, 2.0)

    def assert_physical_interaction_record(self, report: Path, registry_path: Path) -> None:
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
        self.assertEqual(registry["schema"], "krr-protected-refs-successor-v1")
        self.assertEqual(registry["physicalRefCount"], 1)
        self.assertEqual(len(registry["refs"]), 1)
        report_stat = report.stat()
        self.assertEqual(registry["refs"][0], {
            **observation.file_record(report),
            "dev": report_stat.st_dev,
            "ino": report_stat.st_ino,
            "mode": report_stat.st_mode,
        })

    def assert_binding_roundtrip(
        self, report: Path, binding_path: Path, registry_path: Path, binding_sha: str
    ) -> None:
        loaded = typed_producer.load_capacity_binding(self.root, binding_path, binding_sha)
        self.assertEqual(loaded.document, binding_path)
        self.assertEqual(loaded.references, (str(report),))
        self.assertEqual(loaded.state, self.bootstrap_state_path)
        self.assertEqual(loaded.registry, registry_path)
        self.assertEqual(loaded.state_sha256, gate.sha256(self.bootstrap_state_path))
        self.assertEqual(loaded.registry_sha256, gate.sha256(registry_path))

    def test_inspect_accepts_valid_runner_measurement(self) -> None:
        state = capacity_state.create_runner_measurement(
            self.root, self.target, 100, 123.5, 2.0
        )
        self.write_state(state)

        report = self.inspect()

        self.assertEqual(report["baseline_bytes"], 100)
        self.assertEqual(report["threshold_bytes"], 200.0)

    def test_inspect_accepts_runner_measurement_for_contained_custom_target(self) -> None:
        custom_target = self.root / "build-output"
        custom_target.mkdir()
        (custom_target / "fixture.bin").write_bytes(b"custom target data")
        state = capacity_state.create_runner_measurement(
            self.root, custom_target, 321, 123.5, 2.0
        )
        self.write_state(state)

        report = guard.inspect(self.root, custom_target, self.state_path, 2.0)

        self.assertEqual(report["baseline_bytes"], 321)
        self.assertEqual(report["threshold_bytes"], 642.0)

    def test_inspect_keeps_runner_baseline_after_target_tree_is_removed(self) -> None:
        state = capacity_state.create_runner_measurement(
            self.root, self.target, 100, 123.5, 2.0
        )
        self.write_state(state)
        saved_state = self.state_path.read_text(encoding="utf-8")
        (self.target / "fixture.bin").unlink()
        self.target.rmdir()

        report = self.inspect()

        self.assertEqual(report["baseline_bytes"], 100)
        self.assertEqual(report["current_bytes"], 0)
        self.assertEqual(self.state_path.read_text(encoding="utf-8"), saved_state)

    def test_inspect_rejects_unknown_schema_and_downgrade_marker(self) -> None:
        valid = capacity_state.create_runner_measurement(
            self.root, self.target, 100, 123.5, 2.0
        )
        for variation in ("unknown-schema", "missing-schema"):
            with self.subTest(variation=variation):
                altered = dict(valid)
                if variation == "unknown-schema":
                    altered["schema"] = "unknown"
                else:
                    del altered["schema"]
                self.write_state(altered)
                with self.assertRaisesRegex(RuntimeError, "invalid capacity state"):
                    self.inspect()

    def test_inspect_rejects_versioned_target_mismatch(self) -> None:
        state = capacity_state.create_runner_measurement(
            self.root, self.target, 100, 123.5, 2.0
        )
        state["target"] = str(self.root / "another-target")
        self.write_state(state)

        with self.assertRaisesRegex(RuntimeError, "invalid capacity state"):
            self.inspect()

    def test_inspect_accepts_ordinary_legacy_state(self) -> None:
        self.write_state({
            "baseline_bytes": 100,
            "baseline_observed_at": 123.5,
            "target": str(self.target),
            "factor": 2.0,
            "metric": guard.CAPACITY_METRIC,
            "metric_version": guard.CAPACITY_METRIC_VERSION,
        })

        report = self.inspect()

        self.assertEqual(report["baseline_bytes"], 100)
        self.assertEqual(report["threshold_bytes"], 200.0)

    def test_inspect_rejects_reference_without_trusted_policy(self) -> None:
        self.write_state({
            "schema": capacity_state.SCHEMA,
            "kind": "repository_reference",
            "target": str(self.target),
            "baseline_bytes": capacity_state.REFERENCE_BASELINE_BYTES,
            "factor": capacity_state.REFERENCE_FACTOR,
            "metric": capacity_state.METRIC,
            "metric_version": capacity_state.METRIC_VERSION,
            "baseline_observed_at": capacity_state.REFERENCE_OBSERVED_AT,
            "reference": {
                "policy": capacity_state.REFERENCE_POLICY,
                "sha256": "0" * 64,
                "id": capacity_state.REFERENCE_ID,
            },
        })

        with self.assertRaisesRegex(RuntimeError, "invalid capacity state"):
            self.inspect()

    def test_ci_initializer_creates_reference_from_policy(self) -> None:
        committed_policy = Path(__file__).with_name("capacity-reference.json")
        self.policy_path.write_bytes(committed_policy.read_bytes())
        (self.target / "fixture.bin").unlink()
        self.target.rmdir()

        result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.target.exists())
        state = json.loads(self.bootstrap_state_path.read_text(encoding="utf-8"))
        self.assertEqual(state["kind"], "repository_reference")
        self.assertEqual(state["baseline_bytes"], capacity_state.REFERENCE_BASELINE_BYTES)
        self.assertEqual(state["baseline_observed_at"], capacity_state.REFERENCE_OBSERVED_AT)
        self.assertEqual(state["metric"], capacity_state.METRIC)
        self.assertEqual(state["reference"]["id"], capacity_state.REFERENCE_ID)
        self.assertNotIn("measured_bytes", state)

    def test_ci_initializer_binds_only_the_physical_interaction_report(self) -> None:
        initialized = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
        self.assertEqual(initialized.returncode, 0, initialized.stderr)

        report = self.root / "tmp" / "html-interaction-suite" / "krr.json"
        report.parent.mkdir(parents=True)
        report.write_bytes(b"same-run interaction report")

        result = self.run_initializer(
            "--interaction-report", str(report), CI="true", GITHUB_ACTIONS="true"
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        binding_directory = self.root / "tmp" / "html-performance-capacity"
        registry_path = binding_directory / "protected-refs.json"
        binding_path = binding_directory / "binding.json"
        self.assert_physical_interaction_record(report, registry_path)
        binding = json.loads(binding_path.read_text(encoding="utf-8"))
        self.assertEqual(binding["schema"], "krr-html-performance-capacity-binding-v1")
        self.assertEqual(binding["state"]["path"], str(self.bootstrap_state_path))
        self.assertEqual(binding["protectedRegistry"]["path"], str(registry_path))
        binding_sha = observation.file_record(binding_path)["sha256"]
        if not isinstance(binding_sha, str):
            self.fail("binding digest was not recorded")
        self.assertEqual(result.stdout.strip(), binding_sha)
        self.assert_binding_roundtrip(report, binding_path, registry_path, binding_sha)

    def test_ci_initializer_rejects_noncanonical_or_existing_binding_output(self) -> None:
        initialized = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
        self.assertEqual(initialized.returncode, 0, initialized.stderr)

        report = self.root / "tmp" / "html-interaction-suite" / "krr.json"
        report.parent.mkdir(parents=True)
        report.write_bytes(b"same-run interaction report")
        other = self.root / "tmp" / "other-report.json"
        other.write_bytes(report.read_bytes())

        wrong_path = self.run_initializer(
            "--interaction-report", str(other), CI="true", GITHUB_ACTIONS="true"
        )
        self.assertNotEqual(wrong_path.returncode, 0)
        binding_directory = self.root / "tmp" / "html-performance-capacity"
        self.assertFalse(binding_directory.exists())

        binding_directory.mkdir()
        sentinel = binding_directory / "sentinel.json"
        sentinel.write_bytes(b"preserve")
        collision = self.run_initializer(
            "--interaction-report", str(report), CI="true", GITHUB_ACTIONS="true"
        )
        self.assertNotEqual(collision.returncode, 0)
        self.assertEqual(sentinel.read_bytes(), b"preserve")

    def write_recipe_python_shim(
        self, launcher: Path, script: Path, capture: Path, initializer_exit: int = 0
    ) -> None:
        script.write_text(
            "import json, os, sys\n"
            "from pathlib import Path\n"
            "args = sys.argv[1:]\n"
            "if args[0] == 'scripts/build/initialize_ci_capacity_state.py':\n"
            "    expected = ['--root', os.environ['KRR_TEST_ROOT'], '--interaction-report', "
            "os.environ['KRR_TEST_REPORT']]\n"
            "    if args[1:] != expected: raise SystemExit('initializer argv differs')\n"
            "    print('a' * 64)\n"
            f"    raise SystemExit({initializer_exit})\n"
            "elif args[0] == 'scripts/release/produce_html_performance_evidence.py':\n"
            "    Path(os.environ['KRR_TEST_CAPTURE']).write_text(json.dumps(args[1:]))\n"
            "else: raise SystemExit('unexpected python command')\n",
            encoding="utf-8",
        )
        python_path = sys.executable.replace("\\", "/")
        launcher.write_text(
            "#!/usr/bin/env bash\n"
            f"exec {shlex.quote(python_path)} {shlex.quote(script.as_posix())} \"$@\"\n",
            encoding="utf-8",
        )
        launcher.chmod(0o755)

    def run_performance_recipe(
        self, initializer_exit: int = 0, *, manifest_exists: bool = True
    ) -> tuple[subprocess.CompletedProcess[str], list[str] | None]:
        justfile = Path(__file__).resolve().parents[2] / "Justfile"
        fake_bin = self.root / "fake bin with spaces"
        fake_bin.mkdir()
        capture = fake_bin / "producer-argv.json"
        self.write_recipe_python_shim(
            fake_bin / "python3", fake_bin / "python shim.py", capture, initializer_exit
        )
        manifest = self.root / "catalog-stage90-captures.json"
        if manifest_exists:
            manifest.write_text(
                json.dumps({"profile": "catalog-stage90-captures", "acceptance": False}),
                encoding="utf-8",
            )
        report = justfile.parent / "tmp" / "html-interaction-suite" / "krr.json"
        environment = os.environ.copy()
        environment["PATH"] = str(fake_bin) + os.pathsep + environment["PATH"]
        environment["KRR_TEST_ROOT"] = str(justfile.parent)
        environment["KRR_TEST_REPORT"] = str(report)
        environment["KRR_TEST_CAPTURE"] = str(capture)
        result = subprocess.run(
            [
                "just",
                "--justfile",
                str(justfile),
                "html-performance-evidence-produce",
                str(manifest),
            ],
            cwd=justfile.parent,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        forwarded = (
            json.loads(capture.read_text(encoding="utf-8")) if capture.exists() else None
        )
        return result, forwarded

    def assert_performance_workflow_order(self, justfile: Path) -> None:
        workflow = justfile.with_name(".github").joinpath("workflows/test-and-build.yml")
        workflow_source = workflow.read_text(encoding="utf-8")
        paired = workflow_source.split("  paired-performance:\n", 1)[1]
        setup_index = paired.index("      - name: Set up Python 3.13\n")
        initializer_index = paired.index(
            "      - name: Initialize fixed capacity reference before interaction Cargo\n"
        )
        interaction_index = paired.index(
            "      - name: Verify real interaction state for measurement fixture\n"
        )
        self.assertLess(setup_index, initializer_index)
        self.assertLess(initializer_index, interaction_index)
        self.assertIn(
            "uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7.0.0",
            paired,
        )
        self.assertIn("python-version: '3.13'", paired)

    def test_performance_recipe_forwards_binding_path_and_digest(self) -> None:
        result, forwarded = self.run_performance_recipe()
        self.assertEqual(result.returncode, 0, result.stderr)
        justfile = Path(__file__).resolve().parents[2] / "Justfile"
        expected_binding = str(justfile.parent / "tmp/html-performance-capacity/binding.json")
        manifest = str(self.root / "catalog-stage90-captures.json")
        self.assertEqual(forwarded, [
            "--repo", str(justfile.parent),
            "--capacity-binding", expected_binding,
            "--capacity-binding-sha256", "a" * 64,
            "--catalog-plan", manifest,
        ])
        self.assert_performance_workflow_order(justfile)

    def test_performance_recipe_rejects_missing_catalog_manifest(self) -> None:
        result, forwarded = self.run_performance_recipe(manifest_exists=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("catalog stage90 capture manifest is missing", result.stderr)
        self.assertIsNone(forwarded)

    def test_performance_recipe_stops_if_initializer_fails(self) -> None:
        result, forwarded = self.run_performance_recipe(initializer_exit=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(forwarded)

    def test_ci_initializer_preserves_valid_existing_states_byte_for_byte(self) -> None:
        legacy = {
            "baseline_bytes": 100,
            "baseline_observed_at": 123.5,
            "target": str(self.target),
            "factor": 2.0,
            "metric": guard.CAPACITY_METRIC,
            "metric_version": guard.CAPACITY_METRIC_VERSION,
        }
        versioned = capacity_state.create_runner_measurement(
            self.root, self.target, 321, 456.5, 2.0
        )
        for name, state in (("legacy", legacy), ("versioned", versioned)):
            with self.subTest(name=name):
                encoded = (json.dumps(state, indent=1) + "\n").encode()
                self.bootstrap_state_path.write_bytes(encoded)
                result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.bootstrap_state_path.read_bytes(), encoded)

    def test_ci_initializer_fails_closed_for_invalid_existing_state(self) -> None:
        invalid_states = (
            b'{"schema":"unknown","sentinel":"preserve"}\n',
            b'{"baseline_bytes":100,"target":"wrong-target"}\n',
        )
        for encoded in invalid_states:
            with self.subTest(encoded=encoded):
                self.bootstrap_state_path.write_bytes(encoded)
                result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.bootstrap_state_path.read_bytes(), encoded)
                self.bootstrap_state_path.unlink()

    def test_ci_initializer_rejects_uncalibrated_existing_states_and_preserves_bytes(self) -> None:
        states = (b"{}\n", b'{"target":"' + str(self.target).encode() + b'"}\n',
                  b'{"metric":"unique-inode-allocated-bytes"}\n')
        for encoded in states:
            with self.subTest(encoded=encoded):
                self.bootstrap_state_path.write_bytes(encoded)
                result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.bootstrap_state_path.read_bytes(), encoded)
                self.bootstrap_state_path.unlink()

    def test_ci_initializer_rejects_non_ci_before_creating_tmp(self) -> None:
        (self.root / "tmp").rmdir()

        for environment in ({}, {"CI": "true"}, {"GITHUB_ACTIONS": "true"},
                            {"CI": "false", "GITHUB_ACTIONS": "true"}):
            with self.subTest(environment=environment):
                result = self.run_initializer(**environment)
                self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "tmp").exists())

    def test_ci_initializer_rejects_missing_or_changed_policy_without_state_write(self) -> None:
        cases = ("missing", "changed")
        for case in cases:
            with self.subTest(case=case):
                if case == "missing":
                    self.policy_path.unlink()
                else:
                    self.write_policy(factor=3.0)
                result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.bootstrap_state_path.exists())
                if case == "missing":
                    self.write_policy()

    def test_ci_initializer_rejects_symlink_tmp_and_state(self) -> None:
        temporary = self.root / "tmp"
        temporary.rmdir()
        target_tmp = self.root / "other-tmp"
        target_tmp.mkdir()
        temporary.symlink_to(target_tmp, target_is_directory=True)
        result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((target_tmp / ".cargo-capacity-guard.json").exists())

        temporary.unlink()
        temporary.mkdir()
        state_target = self.root / "state-target.json"
        state_target.write_text("{}", encoding="utf-8")
        self.bootstrap_state_path.symlink_to(state_target)
        result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state_target.read_text(encoding="utf-8"), "{}")

    def test_ci_initializer_rejects_non_directory_tmp(self) -> None:
        temporary = self.root / "tmp"
        temporary.rmdir()
        temporary.write_text("not a directory", encoding="utf-8")

        result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(temporary.read_text(encoding="utf-8"), "not a directory")

    def test_ci_initializer_rejects_stale_temporary_write_path(self) -> None:
        write_path = self.bootstrap_state_path.with_suffix(".json.tmp")
        encoded = b"retain this temporary file"
        write_path.write_bytes(encoded)

        result = self.run_initializer(CI="true", GITHUB_ACTIONS="true")

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(write_path.read_bytes(), encoded)
        self.assertFalse(self.bootstrap_state_path.exists())


if __name__ == "__main__":
    unittest.main()
