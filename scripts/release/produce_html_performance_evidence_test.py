"""性能producerのruntime同期preflightと失敗伝播を検証する単体契約。"""
from __future__ import annotations

import importlib.util
import copy
import json
import errno
import io
import os
import platform
import shutil
import sys
import subprocess
import signal
import socket
import time
import threading
from typing import BinaryIO
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from collections.abc import Callable, Mapping

import produce_html_performance_evidence as typed_producer
import verify_html_performance_evidence as gate

spec = importlib.util.spec_from_file_location(
    "performance_producer", Path(__file__).with_name("produce_html_performance_evidence.py")
)
if spec is None or spec.loader is None:
    raise RuntimeError("performance producer module loader is unavailable")
producer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(producer)


class CatalogProducerOperationTest(unittest.TestCase):
    def test_formal_cli_requires_catalog_before_calling_producer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(producer.sys, "argv", [
                 "produce_html_performance_evidence.py",
                 "--repo", temporary,
                 "--output", str(Path(temporary) / "tmp/evidence"),
                 "--capacity-binding", str(Path(temporary) / "capacity.json"),
                 "--capacity-binding-sha256", "a" * 64,
             ]), \
             patch.object(producer.sys, "stderr", io.StringIO()), \
             patch.object(producer, "produce") as run:
            with self.assertRaises(SystemExit) as caught:
                producer.main()
        self.assertEqual(caught.exception.code, 2)
        run.assert_not_called()

    def test_producer_rejects_missing_catalog_before_preflight_or_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "tmp/evidence"
            with patch.object(producer, "_measurement_command") as run:
                with self.assertRaisesRegex(ValueError, "catalog capture manifest required"):
                    producer.produce(root, output)
            run.assert_not_called()
            self.assertFalse(output.exists())

    def test_formal_cli_forwards_catalog_manifest_to_producer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "tmp/evidence"
            capacity = root / "capacity.json"
            manifest = root / "catalog-stage90-captures.json"
            with patch.object(producer.sys, "argv", [
                "produce_html_performance_evidence.py",
                "--repo", str(root),
                "--output", str(output),
                "--capacity-binding", str(capacity),
                "--capacity-binding-sha256", "b" * 64,
                "--catalog-plan", str(manifest),
            ]), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(root.resolve(), output.resolve(), capacity,
                                        "b" * 64, manifest.resolve())

    def test_producer_reuses_strict_observed_pointer_mapping(self) -> None:
        evidence: gate.JsonObject = {"steps": [
            {"kind": kind, "targetNodeId": 4, "x": 14.0, "y": 22.0}
            for kind in ("move", "down", "up")
        ], "hashBefore": "", "hashAfter": "#a", "destination": "#a"}
        stream = producer._catalog_operation_stream("navigation-pointer-click", evidence)
        self.assertEqual([item["kind"] for item in stream],
                         ["pointer_move", "pointer_down", "pointer_up"])
        self.assertEqual(stream[0]["x"], 14.0)

    def test_unknown_catalog_kind_fails_before_any_build(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported finite catalog operation"):
            producer._catalog_operation_stream("future-operation", {"steps": []})


class RuntimeSyncPreflightTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "repo"
        self.root.mkdir()
        self.output = self.root / "tmp" / "evidence"

    def test_public_producer_rejects_unaccepted_html84_before_build(self) -> None:
        manifest = self.root / "catalog-stage90-captures.json"
        pins = ({"path": str(manifest), "sha256": "a" * 64},
                {"path": str(self.root / "plan.json"), "sha256": "b" * 64})
        validation = {"acceptance": False, "fontIdentityEquivalence": "not-established"}
        validator = ["bun", "run", "scripts/html/fidelity-acceptance-cli.ts",
                     "catalog-stage90-validate-captures", pins[0]["path"], pins[1]["path"]]
        completed = subprocess.CompletedProcess(validator, 0, stdout=b"{}", stderr=b"")
        with patch.object(producer.platform, "system", return_value="Darwin"), \
             patch.object(producer.platform, "machine", return_value="arm64"), \
             patch.object(producer, "load_catalog_conditions", return_value=([], *pins)), \
             patch.object(producer, "source", return_value={"head": "initial"}), \
             patch.object(producer, "_measurement_command", return_value=completed) as run, \
             patch.object(producer, "_validate_catalog_capture_envelope", return_value=validation), \
             patch.object(producer, "command") as build:
            with self.assertRaisesRegex(ValueError, "HTML84 visual acceptance is not established"):
                producer.produce(self.root, self.output, catalog_manifest_path=manifest)
        run.assert_called_once_with(
            validator, cwd=self.root, role="preflight",
            log_directory=self.output.parent / f"{self.output.name}.lifecycle",
        )
        build.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_private_catalog_diagnostic_keeps_unaccepted_state_explicit(self) -> None:
        diagnostic = {"acceptance": False, "fontIdentityEquivalence": "not-established"}
        gate._require_html84_acceptance(diagnostic, diagnostic=True)
        with self.assertRaisesRegex(ValueError, "HTML84 visual acceptance is not established"):
            gate._require_html84_acceptance(diagnostic)

    def test_stale_runtime_check_propagates_before_output_build_or_measurement(self) -> None:
        check = ["just", "runtime-bundle-check"]
        failed = subprocess.CalledProcessError(31, check)
        with patch.object(producer.platform, "system", return_value="Darwin"), \
             patch.object(producer.platform, "machine", return_value="arm64"), \
             patch.object(producer, "source", return_value={"head": "initial"}) as source, \
             patch.object(producer, "_measurement_command", side_effect=failed) as run, \
             patch.object(producer, "write") as write, \
             patch.object(producer, "command") as command:
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                producer.produce(self.root, self.output, _allow_legacy_diagnostic=True)
            self.assertIs(caught.exception, failed)
            self.assertEqual(caught.exception.returncode, 31)
            run.assert_called_once_with(check, cwd=self.root, role="preflight",
                                        log_directory=self.output.parent / f"{self.output.name}.lifecycle")
            source.assert_called_once()
            self.assertEqual(source.call_args.args, (self.root,))
            self.assertTrue(callable(source.call_args.kwargs["git_query"]))
            write.assert_not_called()
            command.assert_not_called()
            self.assertFalse(self.output.exists())


    def test_source_change_during_successful_sync_check_rejects_inventory(self) -> None:
        check = ["just", "runtime-bundle-check"]
        with patch.object(producer.platform, "system", return_value="Darwin"), \
             patch.object(producer.platform, "machine", return_value="arm64"), \
             patch.object(producer, "source", side_effect=[{"head": "initial"}, {"head": "changed"}]) as source, \
             patch.object(producer, "_measurement_command", return_value=subprocess.CompletedProcess(check, 0)) as run, \
             patch.object(producer, "write") as write, \
             patch.object(producer, "command") as command:
            with self.assertRaisesRegex(ValueError, "source changed during runtime bundle check"):
                producer.produce(self.root, self.output, _allow_legacy_diagnostic=True)
            run.assert_called_once_with(check, cwd=self.root, role="preflight",
                                        log_directory=self.output.parent / f"{self.output.name}.lifecycle")
            self.assertEqual(source.call_count, 2)
            write.assert_not_called()
            command.assert_not_called()
            self.assertFalse(self.output.exists())

    def test_lifecycle_failures_stop_before_capacity_or_endpoint(self) -> None:
        failures: tuple[BaseException, ...] = (
            subprocess.TimeoutExpired(["just", "runtime-bundle-check"], 120),
            InterruptedError("measurement interrupted"), RuntimeError("cleanup deadline"),
        )
        for failure in failures:
            with self.subTest(failure=failure), \
                    patch.object(producer.platform, "system", return_value="Darwin"), \
                    patch.object(producer.platform, "machine", return_value="arm64"), \
                    patch.object(producer, "source", return_value={"head": "unit source"}), \
                    patch.object(producer, "_measurement_command", side_effect=failure), \
                    patch.object(producer, "load_capacity_binding") as capacity, \
                    patch.object(producer, "context") as endpoint:
                with self.assertRaises(type(failure)) as caught:
                    producer.produce(self.root, self.output, _allow_legacy_diagnostic=True)
                self.assertIs(caught.exception, failure)
                capacity.assert_not_called()
                endpoint.assert_not_called()
                self.assertFalse(self.output.exists())


class ContextEnvironmentPrivacyTest(unittest.TestCase):
    def test_context_hashes_environment_without_changing_worker_environment(self) -> None:
        environment = {"PATH": os.environ.get("PATH", ""), "KRR_SYNTHETIC_SECRET": "credential-value"}
        root = Path.cwd()
        with patch.dict(os.environ, environment, clear=True), \
             patch.object(producer, "source", return_value={"head": "unit"}), \
             patch.object(producer, "inventory", return_value={"exists": True, "files": []}), \
             patch.object(producer, "command", return_value="tool version"):
            worker_environment_before = dict(os.environ)
            snapshot = producer.context(root, root / "target", [], root / "logs")
            context_json = json.dumps(snapshot, sort_keys=True)
            environment_sha = producer._environment_sha256(environment)
            self.assertNotIn("credential-value", context_json)
            self.assertEqual(snapshot["toolContext"]["environmentSha256"], environment_sha)
            self.assertEqual(producer._environment_sha256(environment), environment_sha)
            changed = {**environment, "KRR_SYNTHETIC_SECRET": "changed"}
            self.assertNotEqual(producer._environment_sha256(changed), environment_sha)
            self.assertEqual(dict(os.environ), worker_environment_before)
            worker = producer._performance_worker_environment(
                "rustc", "cargo", "context-pin", root / "Cargo.lock", {"path": "snapshot.json"})
            self.assertEqual(worker, {**worker_environment_before, "KRR_PERF_RUSTC": "rustc",
                "KRR_PERF_CARGO": "cargo", "KRR_PERF_ENVIRONMENT_SHA256": "context-pin",
                "KRR_PERF_LOCK_PATH": str(root / "Cargo.lock"),
                "KRR_PERF_SOURCE_SNAPSHOT_PATH": "snapshot.json"})


class CapacityBindingUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.harness = self.root / gate.HARNESS_ROOT
        self.harness.mkdir(parents=True)
        self.target = self.root / "target"
        (self.target / "release").mkdir(parents=True)
        self.home = self.root / "home"
        self.home.mkdir()
        self.cargo = self.root / "cargo"
        self.cargo.write_bytes(b"unit executable")
        self.cargo.chmod(0o555)
        self.environment = {"PATH": str(self.root), "CARGO_HOME": str(self.home), "CARGO_CAPACITY_FACTOR": "2"}
        self.state = self.root / "state.json"
        self.saved: dict[str, object] = {
            "target": str(self.target), "baseline_bytes": typed_producer.LOCAL_CAPACITY_BASELINE,
            "factor": 2, "metric": "unique-inode-allocated-bytes", "metric_version": 1,
        }
        self.registry = self.root / "refs.json"
        self.references = []
        for index in range(40):
            path = self.root / f"ref-{index}"
            path.write_bytes(str(index).encode())
            info = path.stat()
            self.references.append({"path": str(path), "sha256": gate.sha256(path), "size": info.st_size,
                                    "dev": info.st_dev, "ino": info.st_ino, "mode": info.st_mode})
        extra = self.root / "ref-40"
        extra.write_bytes(b"extra")
        extra_info = extra.stat()
        self.extra_reference = {"path": str(extra), "sha256": gate.sha256(extra), "size": extra_info.st_size,
                                "dev": extra_info.st_dev, "ino": extra_info.st_ino, "mode": extra_info.st_mode}
        self.write_registry(self.references)
        self.document = self.root / "binding.json"
        self.source_records: gate.SourceRecords = {}
        for path in (self.harness / "Cargo.toml", self.harness / "Cargo.lock",
                     self.root / "scripts/build/cargo_capacity_guard.py", self.root / "scripts/build/cargo_capacity_guard_lock.py"):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"unit committed source")
            self.source_records[str(path.relative_to(self.root))] = (gate.sha256(path), path.stat().st_size)
        files = {
            self.harness / "Cargo.toml": '[package]\nname="krr-html-performance-harness"\nversion="0.1.0"\n',
            self.harness / "Cargo.lock": '[[package]]\nname="katana-render-runtime"\nversion="0.4.23"\nsource="registry+https://github.com/rust-lang/crates.io-index"\n[[package]]\nname="katana-render-runtime"\nversion="0.4.24"\n',
            self.harness / "src/main.rs": "fn main() {}",
            self.root / "Cargo.toml": '[workspace.package]\nversion="0.4.24"\n',
            self.root / "crates/katana-render-runtime/Cargo.toml": '[package]\nversion.workspace=true\n',
            self.root / "crates/katana-render-runtime/src/lib.rs": "pub fn unit() {}",
            self.home / "registry/src/unit/katana-render-runtime-0.4.23/Cargo.toml": "unit registry manifest",
            self.home / "registry/src/unit/katana-render-runtime-0.4.23/src/lib.rs": "pub fn baseline_unit() {}",
        }
        for path, content in files.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
            if path.is_relative_to(self.root):
                self.source_records[str(path.relative_to(self.root))] = (gate.sha256(path), path.stat().st_size)
        (self.target / "release/krr-html-performance-harness").write_bytes(b"unit binary")
        self.lock_sha = gate.sha256(self.harness / "Cargo.lock")
        self.metadata: dict[str, object] = self.arm_metadata("baseline")
        self.commands: list[list[str]] = []
        self.environments: list[dict[str, str]] = []
        self.rebind()

    def arm_metadata(self, arm: str) -> gate.JsonObject:
        runtime_manifest = (self.home / "registry/src/unit/katana-render-runtime-0.4.23/Cargo.toml"
                            if arm == "baseline" else self.root / "crates/katana-render-runtime/Cargo.toml")
        harness_id = "path+" + self.harness.as_uri() + "#0.1.0"
        runtime_id = ("registry+https://github.com/rust-lang/crates.io-index#katana-render-runtime@0.4.23"
                      if arm == "baseline" else "path+" + runtime_manifest.parent.as_uri() + "#0.4.24")
        harness: gate.JsonObject = {"id": harness_id, "manifest_path": str(self.harness / "Cargo.toml"),
            "name": "krr-html-performance-harness", "version": "0.1.0", "source": None,
            "targets": [{"name": "krr-html-performance-harness", "kind": ["bin"], "crate_types": ["bin"],
                         "src_path": str(self.harness / "src/main.rs")}]}
        runtime: gate.JsonObject = {"id": runtime_id, "manifest_path": str(runtime_manifest),
            "name": "katana-render-runtime", "version": "0.4.23" if arm == "baseline" else "0.4.24",
            "source": "registry+https://github.com/rust-lang/crates.io-index" if arm == "baseline" else None,
            "targets": [{"name": "katana_render_runtime", "kind": ["lib"], "crate_types": ["lib"],
                         "src_path": str(runtime_manifest.parent / "src/lib.rs")}]}
        return {"version": 1, "workspace_members": [harness_id],
                "workspace_root": str(self.harness), "target_directory": str(self.target),
                "packages": [harness, runtime], "resolve": {"root": harness_id,
                    "nodes": [{"id": harness_id, "features": [arm], "deps": [{"name": arm, "pkg": runtime_id}]},
                              {"id": runtime_id, "features": ["default"]}]}}

    def arm_events(self, arm: str) -> list[gate.JsonObject]:
        metadata = self.arm_metadata(arm)
        packages = gate._objects(metadata["packages"], "unit packages")
        events: list[gate.JsonObject] = []
        for package in packages:
            target = gate._objects(package["targets"], "unit targets")[0]
            executable = str(self.target / "release/krr-html-performance-harness") if target["kind"] == ["bin"] else None
            events.append({"reason": "compiler-artifact", "package_id": package["id"],
                           "manifest_path": package["manifest_path"], "target": target,
                           "profile": {"opt_level": "3", "test": False}, "fresh": True,
                           "features": [arm] if executable else ["default"],
                           "filenames": [executable] if executable else [], "executable": executable})
        return [*events, {"reason": "build-finished", "success": True}]

    def rebind(self) -> None:
        self.state.write_text(json.dumps(self.saved))
        self.document.write_text(json.dumps({
            "schema": "krr-html-performance-capacity-binding-v1",
            "state": {"path": str(self.state), "sha256": gate.sha256(self.state)},
            "protectedRegistry": {"path": str(self.registry), "sha256": gate.sha256(self.registry)},
        }))

    def write_registry(self, references: list[dict[str, object]], *, physical_count: int | None = None,
                       schema: str = "krr-protected-refs-successor-v1") -> None:
        self.registry.write_text(json.dumps({"schema": schema,
            "physicalRefCount": len(references) if physical_count is None else physical_count,
            "refs": references}))

    def binding(self) -> typed_producer.CapacityBinding:
        return typed_producer.load_capacity_binding(self.root, self.document, gate.sha256(self.document))

    def invoke(self, arm: str = "baseline", mutation: Callable[[], object] | None = None,
               guard_exit: int = 0) -> None:
        binding = self.binding()
        def execute(argv: list[str], *, cwd: Path, env: dict[str, str], role: typed_producer.MeasurementRole, log_directory: Path,
                    check: bool = True) -> subprocess.CompletedProcess[str] | subprocess.CompletedProcess[bytes]:
            self.commands.append(argv)
            self.environments.append(env)
            if argv[1] == "metadata":
                if mutation is not None:
                    mutation()
                return subprocess.CompletedProcess(argv, 0, stdout=json.dumps(self.metadata).encode(), stderr=b"metadata stderr\n")
            if argv[-1] == "-vV":
                tool = "cargo" if Path(argv[0]) == self.cargo else "rustc"
                return subprocess.CompletedProcess(argv, 0, stdout=f"{tool} 1.90.0\n".encode(), stderr=b"")
            stdout = b"macro output preserved\n" + b"\n".join(json.dumps(event).encode() for event in self.arm_events(arm))
            return subprocess.CompletedProcess(argv, guard_exit, stdout=stdout, stderr=b'{"guard":"report"}\nCargo stderr\n')
        # fake Cargo単体では出力fixtureだけを置き、実FD安全性は別classで検証する。
        def frozen_fixture(source: Path, destination: Path) -> str:
            self.assertEqual(source, self.target / "release/krr-html-performance-harness")
            destination.parent.mkdir(parents=True)
            destination.write_bytes(b"unit binary")
            return gate.sha256(destination)
        with patch.dict(os.environ, self.environment, clear=True), \
                patch.object(typed_producer, "freeze_binary", side_effect=frozen_fixture), \
                patch.object(shutil, "which", return_value=str(self.cargo)), \
                patch.object(gate, "LOCK_SHA", self.lock_sha), \
                patch.object(typed_producer, "_measurement_command", side_effect=execute):
            typed_producer.guarded_arm_build(self.root, self.harness, arm, binding, self.source_records,
                                           self.root / "tmp/builds" / arm)

    def test_missing_binding_rejects_even_github_actions_before_output_or_cargo(self) -> None:
        output = self.root / "tmp/evidence"
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}), \
                patch.object(platform, "system", return_value="Darwin"), \
                patch.object(platform, "machine", return_value="arm64"), \
                patch.object(typed_producer, "source", return_value={"head": "unit source"}), \
                patch.object(typed_producer, "_measurement_command") as run:
            with self.assertRaisesRegex(ValueError, "capacity binding input required"):
                typed_producer.produce(self.root, output, _allow_legacy_diagnostic=True)
            run.assert_called_once_with(["just", "runtime-bundle-check"], cwd=self.root,
                                        role="preflight", log_directory=output.parent / f"{output.name}.lifecycle")
            self.assertFalse(output.exists())

    def test_fixed_state_and_reference_shapes_reject_without_recalibration(self) -> None:
        cases: list[tuple[str, object]] = [("baseline_bytes", True), ("baseline_bytes", "2363047936"),
                                         ("baseline_bytes", 1), ("factor", False), ("factor", 3),
                                         ("metric_version", True), ("target", str(self.root / "other"))]
        original = dict(self.saved)
        for field, value in cases:
            self.saved = {**original, field: value}
            self.rebind()
            before = self.state.read_bytes()
            with self.subTest(field=field, value=value), self.assertRaises((ValueError, RuntimeError)):
                self.binding()
            self.assertEqual(self.state.read_bytes(), before)
        self.saved = original
        self.rebind()
        for count in (0, 39, 41):
            self.write_registry(self.references, physical_count=count)
            self.rebind()
            with self.assertRaises(ValueError):
                self.binding()
        self.write_registry(self.references)
        self.rebind()
        self.registry.write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            self.binding()

    def test_both_arms_use_guard_state_target_profile_and_reference_union(self) -> None:
        additional = self.root / "additional-ref"
        additional.write_bytes(b"additional")
        self.environment["CARGO_CAPACITY_PROTECTED_REFS"] = json.dumps([str(additional), self.references[0]["path"]])
        for arm in ("baseline", "candidate"):
            self.metadata = self.arm_metadata(arm)
            self.invoke(arm)
        self.assertEqual(len(self.commands), 12)
        for index, arm in enumerate(("baseline", "candidate")):
            argv = self.commands[index * 6 + 3]
            self.assertEqual(argv[:2], [sys.executable, str(self.root / "scripts/build/cargo_capacity_guard.py")])
            self.assertEqual(argv[argv.index("--expected-state-sha256") + 1], gate.sha256(self.state))
            self.assertEqual(argv[argv.index("--state") + 1], str(self.state))
            self.assertEqual(argv[argv.index("--retain-profile") + 1], str(self.target / "release"))
            self.assertEqual(argv[-2:], ["--features", arm])
            refs = [argv[index + 1] for index, value in enumerate(argv) if value == "--protected-ref"]
            self.assertEqual(len(refs), 41)
            self.assertIn(str(additional), refs)
            self.assertEqual(argv[argv.index("--target-dir") + 1], str(self.target))
        self.assertTrue(all(env["CARGO_TARGET_DIR"] == str(self.target) for env in self.environments))

    def test_invalid_reference_rows_and_environment_union_reject_before_cargo(self) -> None:
        cases: list[tuple[str, object]] = [("path", "relative"), ("path", str(self.root / "missing")),
                                         ("sha256", "0" * 64), ("sha256", True), ("size", True)]
        for field, value in cases:
            records = [{**self.references[0], field: value}, *self.references[1:]]
            self.write_registry(records)
            self.rebind()
            with self.subTest(field=field), self.assertRaises((ValueError, RuntimeError, OSError)):
                self.binding()
        self.write_registry(self.references)
        self.rebind()
        for value in ("{", "[]", json.dumps([17]), json.dumps([str(self.root / "missing")])):
            self.environment["CARGO_CAPACITY_PROTECTED_REFS"] = value
            with self.subTest(environment=value), self.assertRaises((ValueError, RuntimeError)):
                self.invoke()
            self.assertFalse(self.commands)

    def test_current_registry_accepts_pinned_positive_counts_and_rejects_invalid_identities(self) -> None:
        additional_references = [self.extra_reference]
        for index in range(41, 44):
            path = self.root / f"ref-{index}"
            path.write_bytes(str(index).encode())
            info = path.stat()
            additional_references.append({"path": str(path), "sha256": gate.sha256(path),
                                          "size": info.st_size, "dev": info.st_dev,
                                          "ino": info.st_ino, "mode": info.st_mode})
        for label, references in (
            ("40 references", self.references),
            ("41 references", [*self.references, additional_references[0]]),
            ("44 references", [*self.references, *additional_references]),
        ):
            with self.subTest(case=label):
                self.write_registry(references)
                self.rebind()
                binding = self.binding()
                self.assertEqual(len(binding.references), len(references))
        self.write_registry(self.references)
        self.rebind()
        cases: list[tuple[str, list[dict[str, object]], int | None, str]] = [
            ("missing references", [], None, "krr-protected-refs-successor-v1"),
            ("wrong count", self.references, 39, "krr-protected-refs-successor-v1"),
            ("wrong schema", self.references, 40, "krr-protected-refs-other-v1"),
            ("duplicate path and inode", [*self.references[:-1], self.references[0]], 40,
             "krr-protected-refs-successor-v1"),
            ("extra row field", [{**self.references[0], "unexpected": True}, *self.references[1:]], 40,
             "krr-protected-refs-successor-v1"),
            ("replaced inode", [{**self.references[0], "ino": gate._integer(self.references[0]["ino"], "ino") + 1},
                                *self.references[1:]], 40, "krr-protected-refs-successor-v1"),
            ("wrong device", [{**self.references[0], "dev": gate._integer(self.references[0]["dev"], "dev") + 1},
                              *self.references[1:]], 40, "krr-protected-refs-successor-v1"),
            ("wrong mode", [{**self.references[0], "mode": gate._integer(self.references[0]["mode"], "mode") ^ 0o100},
                            *self.references[1:]], 40, "krr-protected-refs-successor-v1"),
            ("noncanonical path", [{**self.references[0], "path": str(self.root) + "/./ref-0"},
                                   *self.references[1:]], 40, "krr-protected-refs-successor-v1"),
        ]
        for label, references, count, schema in cases:
            self.write_registry(references, physical_count=count, schema=schema)
            self.rebind()
            with self.subTest(case=label), self.assertRaises((ValueError, RuntimeError)):
                self.binding()

        symlink = self.root / "ref-symlink"
        symlink.symlink_to(self.root / "ref-0")
        symlink_record = {**self.references[0], "path": str(symlink)}
        self.write_registry([symlink_record, *self.references[1:]])
        self.rebind()
        with self.assertRaises((ValueError, RuntimeError)):
            self.binding()

        hardlink = self.root / "ref-hardlink"
        os.link(self.root / "ref-0", hardlink)
        hardlink_info = hardlink.stat()
        hardlink_record = {**self.references[0], "path": str(hardlink),
                           "dev": hardlink_info.st_dev, "ino": hardlink_info.st_ino,
                           "mode": hardlink_info.st_mode}
        self.write_registry([*self.references[:-1], hardlink_record])
        self.rebind()
        with self.assertRaises((ValueError, RuntimeError)):
            self.binding()

        self.registry.write_bytes(b"{")
        self.rebind()
        with self.assertRaises((ValueError, RuntimeError)):
            self.binding()

        self.write_registry(self.references)
        self.rebind()
        self.assertEqual(len(self.binding().references), len(self.references))

    def test_standalone_metadata_and_source_mutation_reject_before_guard(self) -> None:
        cases: list[tuple[str, object]] = [("target_directory", str(self.root / "other")),
                                         ("workspace_root", str(self.root)), ("packages", []),
                                         ("packages", [{"manifest_path": str(self.root / "Cargo.toml"), "name": "krr-html-performance-harness"}]),
                                         ("build_directory", str(self.root / "other"))]
        original = dict(self.metadata)
        for field, value in cases:
            self.commands.clear()
            self.metadata = {**original, field: value}
            with self.subTest(field=field), self.assertRaises((ValueError, RuntimeError)):
                self.invoke()
            self.assertEqual(len(self.commands), 1)
        self.metadata = original
        manifest = self.harness / "Cargo.toml"
        with self.assertRaisesRegex(ValueError, "source changed during metadata"):
            self.invoke(mutation=lambda: manifest.write_bytes(b"changed source"))
        self.assertEqual(len(self.commands), 2)

    def test_selected_configuration_and_environment_drift_reject_before_guard(self) -> None:
        selected = self.harness / ".cargo/config.toml"
        selected.parent.mkdir()
        for content in ('[build]\ntarget="other"', 'include="other.toml"', '[unstable]\nx=true'):
            selected.write_text(content)
            with self.subTest(content=content), self.assertRaises(RuntimeError):
                self.invoke()
            self.assertFalse(self.commands)
        selected.write_text("[build]\n")
        with self.assertRaises((ValueError, RuntimeError)):
            self.invoke(mutation=lambda: selected.write_text('[build]\ntarget="changed"'))
        self.assertEqual(len(self.commands), 1)
        self.commands.clear()
        selected.write_text("[build]\n")
        for name, value in (("CARGO_BUILD_TARGET", "other"), ("CARGO_CAPACITY_FACTOR", "3")):
            self.environment[name] = value
            with self.subTest(environment=name), self.assertRaises((ValueError, RuntimeError)):
                self.invoke()
            self.assertFalse(self.commands)
            self.environment.pop(name)

    def test_guard_failure_and_registry_change_propagate_before_next_stage(self) -> None:
        with self.assertRaises(subprocess.CalledProcessError) as rejected:
            self.invoke(guard_exit=2)
        self.assertEqual(rejected.exception.returncode, 2)
        self.assertEqual(len(self.commands), 4)
        self.commands.clear()
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            self.invoke(mutation=lambda: self.registry.write_bytes(b"changed registry"))
        self.assertEqual(len(self.commands), 3)


    def expected(self, arm: str = "baseline") -> typed_producer.ExpectedBuild:
        with patch.object(gate, "LOCK_SHA", self.lock_sha):
            return typed_producer.expected_build(self.root, self.harness, arm, self.arm_metadata(arm), self.environment)

    def test_resolved_metadata_rejects_wrong_arm_origin_version_targets_and_shapes(self) -> None:
        for arm in ("baseline", "candidate"):
            good = self.arm_metadata(arm)
            self.expected(arm)
            cases: list[tuple[tuple[str | int, ...], object]] = [
                (("version",), True), (("version",), "1"), (("workspace_members",), []),
                (("resolve", "root"), "other"), (("resolve", "nodes", 0, "features"), ["other"]),
                (("resolve", "nodes", 0, "deps", 0, "name"), "candidate" if arm == "baseline" else "baseline"),
                (("resolve", "nodes", 0, "deps", 0, "pkg"), "unknown"),
                (("packages", 0, "version"), "0.2.0"), (("packages", 0, "source"), "git+other"),
                (("packages", 1, "version"), "9.0.0"), (("packages", 1, "source"), "git+other"),
                (("packages", 1, "manifest_path"), str(self.root / "Cargo.toml")),
                (("packages", 1, "targets", 0, "kind"), ["bin"]),
                (("packages", 1, "targets", 0, "src_path"), str(self.harness / "src/main.rs")),
                (("resolve", "nodes", 1, "features"), [False]),
            ]
            for keys, replacement in cases:
                changed = copy.deepcopy(good)
                current: object = changed
                for key in keys[:-1]:
                    current = (gate._array(current, "unit path")[key] if isinstance(key, int)
                               else gate._object(current, "unit path")[key])
                key = keys[-1]
                if isinstance(key, int):
                    gate._array(current, "unit path")[key] = replacement
                else:
                    gate._object(current, "unit path")[key] = replacement
                with self.subTest(arm=arm, keys=keys), patch.object(gate, "LOCK_SHA", self.lock_sha), self.assertRaises((ValueError, KeyError)):
                    typed_producer.expected_build(self.root, self.harness, arm, changed, self.environment)

    def test_actual_artifacts_reject_identity_profile_features_paths_and_cardinality(self) -> None:
        for arm in ("baseline", "candidate"):
            expected = self.expected(arm)
            events = self.arm_events(arm)
            selected, proof = typed_producer.cargo_build_artifacts(b"\n".join(json.dumps(e).encode() for e in events), expected, self.target)
            self.assertEqual(selected, self.target / "release/krr-html-performance-harness")
            self.assertEqual(gate._object(proof["buildFinished"], "finished")["success"], True)
            cases: list[tuple[int, str, object]] = []
            for index in (0, 1):
                cases.extend([(index, "package_id", "other"), (index, "manifest_path", str(self.root / "Cargo.toml")),
                              (index, "profile", {"opt_level": "0", "test": False}),
                              (index, "profile", {"opt_level": "3", "test": True}),
                              (index, "profile", {"opt_level": "3", "test": 0}), (index, "fresh", 1),
                              (index, "features", ["wrong"]), (index, "target", []),
                              (index, "target", {"name": "wrong"})])
            cases.extend([(0, "executable", str(self.cargo)), (0, "filenames", []),
                          (0, "features", ["baseline", "candidate"]), (1, "executable", str(self.cargo)),
                          (2, "success", 1), (2, "success", False)])
            for index, field, value in cases:
                changed = copy.deepcopy(events)
                changed[index][field] = value
                with self.subTest(arm=arm, index=index, field=field), self.assertRaises((ValueError, KeyError)):
                    typed_producer.cargo_build_artifacts(b"\n".join(json.dumps(e).encode() for e in changed), expected, self.target)
            for changed in ([events[0], events[2]], [events[1], events[2]], events[:-1],
                            [*events[:-1], events[0], events[-1]], [*events[:-1], events[1], events[-1]],
                            [*events, events[-1]], [events[-1], *events[:-1]]):
                with self.subTest(arm=arm, cardinality=changed), self.assertRaises(ValueError):
                    typed_producer.cargo_build_artifacts(b"\n".join(json.dumps(e).encode() for e in changed), expected, self.target)

    def test_observed_candidate_version_cannot_replace_fixed_lock_binding(self) -> None:
        metadata = self.arm_metadata("candidate")
        gate._objects(metadata["packages"], "unit packages")[1]["version"] = "9.0.0"
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion="9.0.0"\n')
        self.assertEqual(gate.sha256(self.harness / "Cargo.lock"), self.lock_sha)
        with patch.object(gate, "LOCK_SHA", self.lock_sha), self.assertRaisesRegex(ValueError, "fixed lock"):
            typed_producer.expected_build(self.root, self.harness, "candidate", metadata, self.environment)
        self.assertFalse(self.commands)

    def test_boolean_guard_success_rejects_before_binary_freeze(self) -> None:
        with self.assertRaisesRegex(ValueError, "integer"):
            self.invoke(guard_exit=False)
        self.assertTrue((self.root / "tmp/builds/baseline/cargo.jsonl").is_file())
        self.assertFalse((self.root / "tmp/builds/baseline/binary").exists())

    def test_raw_json_shape_duplicate_keys_and_nonterminal_output_are_rejected(self) -> None:
        expected = self.expected()
        events = b"\n".join(json.dumps(e).encode() for e in self.arm_events("baseline"))
        for prefix in (b"[proc-macro] emitted helper text", b"[warning: generated shim]", b"[1, raw expansion]"):
            with self.subTest(bracket_output=prefix):
                selected, proof = typed_producer.cargo_build_artifacts(prefix + b"\n" + events, expected, self.target)
                self.assertEqual(selected, self.target / "release/krr-html-performance-harness")
                self.assertEqual(gate._object(proof["buildFinished"], "finished")["success"], True)
        invalid = [b"", b"{", b"[]\n" + events, b"[{}]\n" + events,
                   b"true\n" + events, b'"scalar"\n' + events, b"0\n" + events,
                   b'{"reason":true}\n' + events,
                   b'{"reason":"compiler-message","reason":"compiler-message"}\n' + events,
                   events + b"\nmacro output after completion", events + b'\n{"reason":"compiler-message"}']
        for raw in invalid:
            with self.subTest(raw=raw), self.assertRaises((ValueError, KeyError)):
                typed_producer.cargo_build_artifacts(raw, expected, self.target)
        self.invoke()
        directory = self.root / "tmp/builds/baseline"
        self.assertTrue((directory / "cargo.jsonl").read_bytes().startswith(b"macro output preserved\n"))
        self.assertEqual((directory / "guard-cargo.stderr").read_bytes(), b'{"guard":"report"}\nCargo stderr\n')
        self.assertEqual((directory / "binary/krr-html-performance-harness").read_bytes(), b"unit binary")
        self.assertNotIn("--no-deps", self.commands[0])
        self.assertIn("--message-format=json", self.commands[3])
        self.assertNotIn("--print-json", self.commands[3])


class ProducerJsonBoundaryUnitTests(unittest.TestCase):
    def test_missing_or_invalid_no_follow_api_rejects_without_open(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "binary"
            source.write_bytes(b"portable unit input")
            for value in (None, False, True, 0, -1, "64"):
                with self.subTest(value=value), patch.object(os, "O_NOFOLLOW", value, create=True), \
                        patch.object(os, "open") as opened:
                    with self.assertRaisesRegex(ValueError, "no-follow"):
                        typed_producer.freeze_binary(source, root / "frozen/binary")
                    opened.assert_not_called()
                    self.assertFalse((root / "frozen").exists())

    def test_worker_header_keeps_native_float_scale_and_rejects_bad_shapes(self) -> None:
        viewport: typed_producer.Viewport = {"width": 1280, "height": 900, "scale": 1}
        native: dict[str, object] = {"sample": 0, "viewport": {"width": 1280, "height": 900, "scale": 1.0},
                                     "native_extra": ["preserved"]}
        payload = typed_producer.worker_sample(json.dumps(native).encode(), 0, viewport)
        self.assertEqual(payload, native)
        self.assertIsInstance(gate._object(payload["viewport"], "viewport")["scale"], float)
        invalid: list[object] = [None, [], True]
        sample_cases: list[object] = [True, False, "0", 0.0, None, []]
        for sample in sample_cases:
            invalid.append({**native, "sample": sample})
        viewport_cases: list[object] = [None, [], True, {"width": "1280", "height": 900, "scale": 1},
                       {"width": 1280.0, "height": 900, "scale": 1},
                       {"width": 1280, "height": 900, "scale": True},
                       {"width": 1280, "height": 900, "scale": "1"}]
        for actual in viewport_cases:
            invalid.append({**native, "viewport": actual})
        for value in invalid:
            with self.subTest(value=value), self.assertRaises((ValueError, KeyError)):
                typed_producer.worker_sample(json.dumps(value).encode(), 0, viewport)
        duplicate = b'{"sample":0,"sample":0,"viewport":{"width":1280,"height":900,"scale":1}}'
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            typed_producer.worker_sample(duplicate, 0, viewport)

    def test_command_preserves_channels_and_arguments_without_output_coercion(self) -> None:
        cases: list[tuple[bool, object, bool]] = [
            (True, "tool version", True), (False, b"worker JSON", True),
            (True, b"tool version", False), (False, "worker JSON", False),
            (True, None, False), (False, True, False), (False, [b"worker"], False),
        ]
        cwd = Path("/unit/root")
        environment = {"UNIT": "value"}
        for text, output, accepted in cases:
            with self.subTest(text=text, output=output), patch.object(typed_producer, "_measurement_command",
                    return_value=subprocess.CompletedProcess(["unit"], 0, b"raw" if text else output)) as run, \
                    patch.object(io, "TextIOWrapper") as decoder:
                decoder.return_value.__enter__.return_value.read.return_value = output
                def call(text_value: bool = text) -> str | bytes:
                    if text_value:
                        return typed_producer.command(["unit"], text=True, cwd=cwd, env=environment,
                                                      role="tool-query", log_directory=Path("/unit/logs"))
                    return typed_producer.command(["unit"], cwd=cwd, env=environment,
                                                  role="worker", log_directory=Path("/unit/logs"))
                if accepted:
                    self.assertIs(call(), output)
                else:
                    with self.assertRaises(ValueError):
                        call()
                run.assert_called_once_with(["unit"], cwd=cwd, env=environment,
                                            role="tool-query" if text else "worker",
                                            log_directory=Path("/unit/logs"))

    def test_interaction_state_rejects_boolean_integer_and_mapping_coercion(self) -> None:
        checksum = "a" * 64
        accepted = {"inputSha256": checksum, "states": {"js": {"expanded": True, "result": "opened"}}}
        typed_producer.validate_interaction(accepted, checksum)
        invalid: list[object] = [None, [], {"inputSha256": checksum, "states": []},
                                {"inputSha256": checksum, "states": {"js": False}},
                                {"inputSha256": "b" * 64, "states": accepted["states"]}]
        expanded_cases: list[object] = [1, "true", [], None, False]
        for expanded in expanded_cases:
            invalid.append({"inputSha256": checksum, "states": {"js": {"expanded": expanded, "result": "opened"}}})
        for value in invalid:
            with self.subTest(value=value), self.assertRaises((ValueError, KeyError)):
                typed_producer.validate_interaction(value, checksum)


@unittest.skipIf(os.name == "nt", "macOS producerのPOSIX no-follow実FD単体契約")
class BinaryFreezeUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.binary = self.root / "binary"
        self.binary.write_bytes(b"unit binary source")
        self.frozen = self.root / "frozen/binary"

    def test_regular_single_fd_copy_keeps_bytes_sha_and_readonly_mode(self) -> None:
        checksum = typed_producer.freeze_binary(self.binary, self.frozen)
        self.assertEqual(checksum, gate.sha256(self.binary))
        self.assertEqual(checksum, gate.sha256(self.frozen))
        self.assertEqual(self.frozen.read_bytes(), self.binary.read_bytes())
        self.assertEqual(self.frozen.stat().st_mode & 0o777, 0o555)

    def test_symlink_source_or_destination_namespace_is_rejected(self) -> None:
        link = self.root / "link"
        link.symlink_to(self.binary)
        with self.assertRaises(ValueError):
            typed_producer.freeze_binary(link, self.frozen)
        link.unlink()
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            typed_producer.freeze_binary(self.binary, link / "frozen/binary")
        self.assertFalse(self.frozen.exists())

    def test_identity_hash_and_read_failures_close_real_source_fd(self) -> None:
        original_open = os.open
        original_fstat = os.fstat
        descriptors: list[int] = []
        def opened(path: str | os.PathLike[str], flags: int) -> int:
            descriptor = original_open(path, flags)
            descriptors.append(descriptor)
            no_follow: object = getattr(os, "O_NOFOLLOW", None)
            if not isinstance(no_follow, int):
                raise AssertionError("POSIX no-follow flag is missing")
            self.assertNotEqual(flags & no_follow, 0)
            return descriptor
        for failure in ("identity", "path-replace", "hash", "read"):
            destination = self.root / failure / "binary"
            calls = 0
            def status(descriptor: int, failure_value: str = failure) -> os.stat_result:
                nonlocal calls
                calls += 1
                if calls == 2:
                    if failure_value == "identity":
                        self.binary.write_bytes(b"mutated actual source")
                    elif failure_value == "path-replace":
                        replacement = self.root / "replacement"
                        replacement.write_bytes(b"actual replaced path")
                        replacement.replace(self.binary)
                    elif failure_value == "read":
                        raise OSError("unit stat read failed")
                return original_fstat(descriptor)
            with self.subTest(failure=failure), patch.object(os, "open", side_effect=opened), \
                    patch.object(os, "fstat", side_effect=status):
                if failure == "hash":
                    with patch.object(gate, "sha256", return_value="0" * 64), self.assertRaisesRegex(ValueError, "SHA differs"):
                        typed_producer.freeze_binary(self.binary, destination)
                else:
                    with self.assertRaises((ValueError, OSError)):
                        typed_producer.freeze_binary(self.binary, destination)
            with self.assertRaises(OSError):
                original_fstat(descriptors[-1])


class FrozenBinaryCarryUnitTests(unittest.TestCase):
    def test_expected_arm_keeps_the_build_checksum_and_rejects_later_changes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="performance-carry-unit-") as directory:
            binary = Path(directory).resolve() / "binary"
            binary.write_bytes(b"original build")
            checksum = gate.sha256(binary)
            build = typed_producer.ArmBuildResult([], binary, checksum, {}, [])
            build_ref: dict[str, typed_producer.FilePin] = {
                "baseline": {"path": "/build-artifact.json", "sha256": "a" * 64}}
            expected = typed_producer.expected_arm_pins({"baseline": build}, "snapshot", build_ref)
            self.assertEqual(expected["baseline-0.4.23"]["binary_sha256"], checksum)
            self.assertEqual(expected["baseline-0.4.23"]["source_snapshot_sha256"], "snapshot")
            for method in ("write", "replace"):
                with self.subTest(method=method):
                    if method == "write":
                        binary.write_bytes(b"different measurement binary")
                    else:
                        replacement = binary.with_name("replacement")
                        replacement.write_bytes(b"replaced measurement binary")
                        replacement.replace(binary)
                    self.assertNotEqual(gate.sha256(binary), checksum)
                    for validate in (
                        lambda: typed_producer.expected_arm_pins({"baseline": build}, "snapshot", build_ref),
                        lambda: typed_producer.require_frozen_binary(build),
                    ):
                        with self.assertRaisesRegex(ValueError, "frozen build binary changed"):
                            validate()
            self.assertEqual(expected["baseline-0.4.23"]["binary_sha256"], checksum)


def _lifecycle_fixture(mode: str) -> int:
    if mode not in {"exit", "nonzero", "wait", "ignore", "parent"}:
        raise ValueError("unknown lifecycle fixture mode")
    os.write(1, b"fixture stdout\n")
    os.write(2, b"fixture stderr\n")
    if mode == "parent":
        child = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--lifecycle-fixture", "wait"])
        os.write(1, f"child={child.pid}\n".encode())
    elif mode in {"wait", "ignore"}:
        if mode == "ignore":
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
        time.sleep(30)
    return 7 if mode == "nonzero" else 0


@unittest.skipUnless(os.name == "posix", "actual POSIX lifecycle only")
class MeasurementLifecycleIntegrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.logs = Path(self.temporary.name) / "raw"

    def invoke(self, mode: str) -> subprocess.CompletedProcess[bytes]:
        return typed_producer._measurement_command(
            [sys.executable, str(Path(__file__).resolve()), "--lifecycle-fixture", mode],
            role="worker", log_directory=self.logs,
        )

    def evidence(self) -> gate.JsonObject:
        paths = list(self.logs.glob("worker-*/lifecycle.json"))
        self.assertEqual(len(paths), 1)
        result = gate.read_json(paths[0])
        self.assertIs(result["groupClosed"], True)
        self.assertIs(result["rawFdsClosed"], True)
        identity = gate._object(result["identity"], "identity")
        pid = gate._integer(identity["pid"], "pid")
        self.assertFalse(typed_producer._measurement_group_exists(pid))
        self.assertEqual(identity["sessionId"], pid)
        self.assertEqual(identity["processGroupId"], pid)
        self.assertEqual(identity["parentPid"], os.getpid())
        for row in gate._objects(result["raw"], "raw"):
            path = Path(gate._text(row["path"], "raw path"))
            self.assertEqual(gate.sha256(path), row["sha256"])
        return result

    def test_actual_exit_keeps_raw_channels_and_group_closure(self) -> None:
        completed = self.invoke("exit")
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, b"fixture stdout\n")
        self.assertEqual(completed.stderr, b"fixture stderr\n")
        self.assertEqual(self.evidence()["exitCode"], 0)

    def test_actual_nonzero_preserves_original_error_and_raw(self) -> None:
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            self.invoke("nonzero")
        self.assertEqual(caught.exception.returncode, 7)
        self.assertEqual(caught.exception.stdout, b"fixture stdout\n")
        self.assertEqual(caught.exception.stderr, b"fixture stderr\n")
        self.assertEqual(self.evidence()["exitCode"], 7)

    def test_actual_successful_parent_cannot_leave_stream_holding_child(self) -> None:
        completed = self.invoke("parent")
        self.assertEqual(completed.returncode, 0)
        self.assertIn(b"child=", completed.stdout)
        self.evidence()

    def test_actual_timeout_terminates_term_ignoring_child_and_preserves_raw(self) -> None:
        limits = dict(typed_producer.MEASUREMENT_LIMITS)
        limits["worker"] = 0.5
        with patch.object(typed_producer, "MEASUREMENT_LIMITS", limits):
            with self.assertRaises(subprocess.TimeoutExpired) as caught:
                self.invoke("ignore")
        self.assertIn(b"fixture stdout", typed_producer._bytes(caught.exception.output, "stdout"))
        self.assertIn(b"fixture stderr", typed_producer._bytes(caught.exception.stderr, "stderr"))
        self.assertIn("TimeoutExpired", gate._text(self.evidence()["failure"], "failure"))

    def test_actual_interrupt_closes_child_before_propagation(self) -> None:
        timer = threading.Timer(0.5, os.kill, args=(os.getpid(), signal.SIGINT))
        timer.start()
        try:
            with self.assertRaises(KeyboardInterrupt):
                self.invoke("wait")
        finally:
            timer.cancel()
            timer.join(timeout=1)
        self.assertIn("KeyboardInterrupt", gate._text(self.evidence()["failure"], "failure"))

    def test_binary_stdin_uses_regular_file_and_reaches_child(self) -> None:
        payload = b"complete object-id batch input\n"
        code = "import os,stat; assert stat.S_ISREG(os.fstat(0).st_mode); os.write(1, os.read(0, 4096))"
        completed = typed_producer._measurement_command(
            [sys.executable, "-c", code], role="tool-query", log_directory=self.logs,
            input_bytes=payload,
        )
        self.assertEqual(completed.stdout, payload)
        self.assertEqual(completed.returncode, 0)
        records = list(self.logs.glob("tool-query-*/lifecycle.json"))
        self.assertEqual(len(records), 1)
        self.assertTrue(gate.read_json(records[0])["groupClosed"])

    def test_producer_git_query_reads_real_temporary_repository(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            root.mkdir()
            source_path = root / "source.py"
            source_path.write_text("value = 1\n")
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "source.py"], check=True)
            subprocess.run(
                ["git", "-C", str(root), "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
                 "commit", "-qm", "fixture"],
                check=True,
            )
            snapshot = typed_producer.source(root, git_query=typed_producer._make_git_query(self.logs))
            self.assertEqual(snapshot["head"], gate.git(root, "rev-parse", "HEAD").strip())
            records = gate.records_map(snapshot["files"])
            self.assertIn(str(source_path), records)
            self.assertEqual(records[str(source_path)], (gate.sha256(source_path), source_path.stat().st_size))
            lifecycle_records = list(self.logs.glob("tool-query-*/lifecycle.json"))
            self.assertGreaterEqual(len(lifecycle_records), 8)
            for path in lifecycle_records:
                row = gate.read_json(path)
                self.assertTrue(row["groupClosed"])
                self.assertTrue(row["rawFdsClosed"])

    def test_lifecycle_record_write_failure_keeps_primary_command_failure(self) -> None:
        original_write = typed_producer.write

        def fail_lifecycle_record(path: Path, value: object) -> typed_producer.FilePin:
            if path.name == "lifecycle.json":
                raise OSError("lifecycle record unavailable")
            return original_write(path, value)

        with patch.object(typed_producer, "write", side_effect=fail_lifecycle_record):
            with self.assertRaisesRegex(RuntimeError, "lifecycle record write failed") as caught:
                self.invoke("nonzero")
        self.assertIn("CalledProcessError", str(caught.exception))
        self.assertIn("lifecycle record unavailable", str(caught.exception))
        self.assertIsInstance(caught.exception.__cause__, subprocess.CalledProcessError)


@unittest.skipUnless(os.name == "posix", "POSIX ownership boundary unit only")
class NativeCliOwnerHookTests(unittest.TestCase):
    def test_escaped_session_holding_raw_is_failed_without_signalling_unknown_pid(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            marker = root / "owned-test-child.json"
            child = ("import os,json,time;from pathlib import Path;"
                     f"Path({str(marker)!r}).write_text(json.dumps({{'pid':os.getpid(),'group':os.getpgrp()}}));"
                     "time.sleep(30)")
            parent = ("import subprocess,sys,time;from pathlib import Path;"
                      f"subprocess.Popen([sys.executable,'-c',{child!r}],start_new_session=True);"
                      f"p=Path({str(marker)!r});deadline=time.monotonic()+3\n"
                      "while not p.exists():\n"
                      " if time.monotonic()>=deadline:raise RuntimeError('controlled child marker timeout')\n"
                      " time.sleep(0.01)\n")
            pid: int | None = None
            try:
                with self.assertRaisesRegex(RuntimeError, "unknown writable holders"):
                    typed_producer.observe_native_cli_command([sys.executable, "-c", parent],
                        role="tool-query", cwd=root, env={"PATH": os.environ["PATH"]},
                        log_directory=root / "logs", attempt_token="c" * 64, label="probe-1-unit")
                pid = gate._integer(gate.read_json(marker)["pid"], "controlled child pid")
                os.kill(pid, 0)
                lifecycle = gate.read_json(next((root / "logs").glob("tool-query-*/lifecycle.json")))
                self.assertTrue(lifecycle["groupClosed"])
                self.assertFalse(lifecycle["rawFdsClosed"])
                self.assertFalse(lifecycle["descendantClosureProved"])
                self.assertIn("unknown writable holders", str(lifecycle["failure"]))
                self.assertIn("unknown writable holders", str(lifecycle["cleanupFailure"]))
                closure = gate._object(lifecycle["nativeRawFdClosure"], "native closure")
                self.assertIn(pid, [gate._object(row, "holder")["pid"] for row in gate._array(closure["holders"], "holders")])
            finally:
                if pid is None and marker.exists():
                    pid = gate._integer(gate.read_json(marker)["pid"], "controlled child pid")
                if pid is not None:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    def test_native_same_group_descendant_is_closed_before_raw_acceptance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            evidence = typed_producer.observe_native_cli_command(
                [sys.executable, str(Path(__file__).resolve()), "--lifecycle-fixture", "parent"],
                role="tool-query", cwd=root, env={"PATH": os.environ["PATH"]},
                log_directory=root / "logs", attempt_token="d" * 64, label="probe-1-unit")
            lifecycle = gate.read_json(Path(gate._text(evidence.lifecycle["path"], "lifecycle")))
            self.assertTrue(lifecycle["rawFdsClosed"])
            self.assertEqual(gate._object(lifecycle["nativeRawFdClosure"], "closure")["holders"], [])

    def test_actual_hook_binds_closed_raw_pid_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            environment = {"PATH": os.environ["PATH"], "KRR_UNIT_VALUE": "opaque unit value"}
            evidence = typed_producer.observe_native_cli_command(
                [sys.executable, "-c", "import os;os.write(1,b'raw out');os.write(2,b'raw err')"],
                role="tool-query", cwd=root, env=environment, log_directory=root / "logs",
                attempt_token="a" * 64, label="probe-1-unit",
            )
            record = evidence.observation
            lifecycle = gate.read_json(Path(gate._text(evidence.lifecycle["path"], "lifecycle")))
            self.assertEqual(record["environment"], environment)
            self.assertEqual(record["pid"], gate._object(lifecycle["identity"], "identity")["pid"])
            self.assertIs(record["reaped"], True)
            self.assertEqual(record["errors"], [])
            self.assertEqual(record["attemptToken"], "a" * 64)
            self.assertGreaterEqual(int(str(record["endedNs"])), int(str(record["startedNs"])))
            for key, expected in (("stdout", b"raw out"), ("stderr", b"raw err")):
                pin = gate._object(record[key], key)
                self.assertEqual(Path(gate._text(pin["path"], key)).read_bytes(), expected)
                self.assertEqual(pin["size"], len(expected))

    @unittest.skipUnless(hasattr(socket, "AF_UNIX"), "Unix-domain socket synchronization required")
    def test_live_child_raw_stdout_rename_prevents_completed_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            log_directory = root / "logs"
            log_directory.mkdir()
            ready_path = root / "helper-ready.sock"
            coordinator_path = root / "raw-coordinator.sock"
            ready_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            ready_listener.bind(str(ready_path))
            ready_listener.listen(1)
            ready_listener.settimeout(15)
            coordinator_code = (
                "import socket,sys; server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); "
                "server.bind(sys.argv[2]); server.listen(1); ready=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); "
                "ready.connect(sys.argv[1]); ready.sendall(b'ready'); ready.close(); "
                "connection,_=server.accept(); path=connection.recv(4096).decode(); "
                "renamed=path.removesuffix('stdout.raw')+'stdout.moved'; "
                "import os; os.replace(path,renamed); connection.sendall(b'renamed'); "
                "connection.close(); server.close()"
            )
            child_code = (
                "import glob,os,socket,sys; os.write(1,b'owned raw'); "
                "paths=glob.glob(sys.argv[2]+'/tool-query-*/stdout.raw'); "
                "assert len(paths)==1; connection=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); "
                "connection.connect(sys.argv[1]); connection.sendall(paths[0].encode()); "
                "assert connection.recv(16)==b'renamed'; connection.close()"
            )
            coordinator = subprocess.Popen(
                [sys.executable, "-c", coordinator_code, str(ready_path), str(coordinator_path)],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            try:
                ready_connection, _ = ready_listener.accept()
                with ready_connection:
                    self.assertEqual(ready_connection.recv(16), b"ready")
                with self.assertRaisesRegex(RuntimeError, "measurement cleanup failed") as caught:
                    typed_producer.observe_native_cli_command(
                        [sys.executable, "-c", child_code, str(coordinator_path), str(log_directory)],
                        role="tool-query", cwd=root, env={"PATH": os.environ["PATH"]},
                        log_directory=log_directory, attempt_token="e" * 64, label="rename-live-raw",
                    )
                self.assertEqual(coordinator.wait(timeout=15), 0)
            finally:
                ready_listener.close()
                if coordinator.poll() is None:
                    coordinator.kill()
                    coordinator.wait()
            self.assertIn("FileNotFoundError", str(caught.exception))
            self.assertEqual(list(log_directory.glob("tool-query-*/lifecycle.json")), [])
            self.assertEqual(list(log_directory.glob("tool-query-*/stdout.moved"))[0].read_bytes(), b"owned raw")

    def test_failed_actual_hook_never_returns_completed_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with self.assertRaises(subprocess.CalledProcessError):
                typed_producer.observe_native_cli_command([sys.executable, "-c", "raise SystemExit(7)"],
                    role="build", cwd=root, env={"PATH": os.environ["PATH"]}, log_directory=root,
                    attempt_token="b" * 64, label="probe-1-unit")
            lifecycle = gate.read_json(next(root.glob("build-*/lifecycle.json")))
            self.assertEqual(lifecycle["exitCode"], 7)
            self.assertTrue(lifecycle["groupClosed"])
            self.assertTrue(lifecycle["rawFdsClosed"])


@unittest.skipUnless(os.name == "posix", "POSIX ownership boundary unit only")
class MeasurementLifecycleUnitTests(unittest.TestCase):
    def test_polled_exit_code_is_validated(self) -> None:
        with patch.object(subprocess, "Popen") as constructor:
            process = constructor.return_value
            bad_values: tuple[object, ...] = (True, {}, b"0", "0", [0])
            for value in bad_values:
                with self.subTest(value=value):
                    process.poll.return_value = value
                    with self.assertRaises(ValueError):
                        typed_producer._measurement_exit(process)
            for value, exited in ((None, False), (0, True), (-9, True)):
                with self.subTest(value=value):
                    process.poll.reset_mock()
                    process.poll.return_value = value
                    self.assertIs(typed_producer._measurement_exit(process), exited)
                    process.poll.assert_called_once_with()

    def test_observation_failure_still_kills_and_reaps_owned_child(self) -> None:
        with patch.object(subprocess, "Popen") as constructor, \
                patch.object(typed_producer, "_measurement_exit", side_effect=ValueError("identity differs")), \
                patch.object(os, "killpg") as group, \
                patch.object(typed_producer, "_measurement_group_exists", return_value=False):
            process = constructor.return_value
            process.pid = 42
            with self.assertRaisesRegex(RuntimeError, "identity differs"):
                typed_producer._close_measurement_group(process)
            self.assertEqual([call.args for call in group.call_args_list], [(42, signal.SIGTERM), (42, signal.SIGKILL)])
            process.kill.assert_called_once_with()
            process.wait.assert_called_once()
            self.assertGreaterEqual(process.wait.call_args.kwargs["timeout"], 0)
            self.assertLessEqual(process.wait.call_args.kwargs["timeout"], 10)

    def test_permission_or_remaining_group_never_becomes_closed(self) -> None:
        for observed in (True, PermissionError(errno.EPERM, "denied")):
            with self.subTest(observed=observed), patch.object(subprocess, "Popen") as constructor, \
                    patch.object(typed_producer, "_measurement_exit", return_value=True), \
                    patch.object(os, "killpg"), \
                    patch.object(typed_producer, "MEASUREMENT_CLEANUP_SECONDS", 0.02), \
                    patch.object(typed_producer, "_measurement_group_exists",
                                 side_effect=observed if isinstance(observed, Exception) else None,
                                 return_value=observed):
                process = constructor.return_value
                process.pid = 42
                with self.assertRaisesRegex(RuntimeError, "cleanup deadline"):
                    typed_producer._close_measurement_group(process)
                process.kill.assert_called_once_with()
                process.wait.assert_called_once()

    def test_spawn_failure_keeps_raw_files_and_closes_open_fds(self) -> None:
        handles: list[BinaryIO] = []

        def rejected(args: list[str], *, cwd: Path | None, env: Mapping[str, str] | None,
                     stdin: int, stdout: BinaryIO, stderr: BinaryIO,
                     start_new_session: bool) -> subprocess.Popen[bytes]:
            del args, cwd, env
            self.assertEqual(stdin, subprocess.DEVNULL)
            self.assertTrue(start_new_session)
            handles.extend([stdout, stderr])
            self.assertFalse(stdout.closed)
            self.assertFalse(stderr.closed)
            stdout.write(b"opened stdout")
            stderr.write(b"opened stderr")
            raise OSError("spawn rejected")

        with tempfile.TemporaryDirectory() as temporary, patch.object(subprocess, "Popen", side_effect=rejected):
            logs = Path(temporary) / "raw"
            with self.assertRaisesRegex(OSError, "spawn rejected"):
                typed_producer._measurement_command(["unit"], role="worker", log_directory=logs)
            self.assertEqual(len(handles), 2)
            self.assertTrue(all(handle.closed for handle in handles))
            folders = list(logs.iterdir())
            self.assertEqual(len(folders), 1)
            self.assertEqual((folders[0] / "stdout.raw").read_bytes(), b"opened stdout")
            self.assertEqual((folders[0] / "stderr.raw").read_bytes(), b"opened stderr")
            record = gate.read_json(folders[0] / "lifecycle.json")
            self.assertIs(record["groupClosed"], False)
            self.assertIs(record["rawFdsClosed"], True)
            self.assertIn("spawn rejected", gate._text(record["failure"], "failure"))


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--lifecycle-fixture":
        raise SystemExit(_lifecycle_fixture(sys.argv[2]))
    unittest.main()
