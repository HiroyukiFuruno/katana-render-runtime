"""性能producerのruntime同期preflightと失敗伝播を検証する単体契約。"""
from __future__ import annotations

import copy
import ast
import inspect
import json
import errno
import io
import os
import stat
import platform
import shutil
import sys
import subprocess
import signal
import socket
import time
import threading
from contextlib import nullcontext
from typing import BinaryIO, NamedTuple, cast
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
from collections.abc import Callable, Mapping, Sequence
from types import FrameType

import psutil
from html_performance_host_load_test_support import cpu_times
import produce_html_performance_evidence as typed_producer
import html_performance_host_load as host_load
import html_performance_host_load_validation as load_validation
import html_performance_reference as reference
import html_performance_source_materialization as source_materialization
import verify_html_performance_evidence as gate

producer = typed_producer


class CpuTimes(NamedTuple):
    user: float
    idle: float
    system: float = 0.0


class CatalogProducerOperationTest(unittest.TestCase):
    def test_catalog_pair_manifest_keeps_capture_binary_separate_from_measurement_arm(self) -> None:
        condition = "async/expected-state/1280x900@1"
        capture_sha = "a" * 64
        candidate_sha = "b" * 64
        evidence: gate.JsonObject = {"path": "/capture/async-krr.json", "sha256": "c" * 64}
        plan: gate.JsonObject = {"conditionId": condition,
                                 "evidenceBinarySha256": capture_sha,
                                 "operationStream": []}
        raw_report: gate.JsonObject = {"binarySha256": capture_sha}
        old_manifest: gate.JsonObject = {"condition_id": condition,
                                         "operation_evidence_ref": evidence}
        self.assertNotEqual(old_manifest.get("binarySha256"), raw_report["binarySha256"])
        manifest = producer._catalog_pair_manifest_fields(plan, condition, evidence)
        self.assertEqual(manifest["binarySha256"], raw_report["binarySha256"])
        self.assertNotEqual(manifest["binarySha256"], candidate_sha)
        self.assertEqual(manifest["operation_evidence_ref"], evidence)
        with self.assertRaisesRegex(ValueError, "condition differs"):
            producer._catalog_pair_manifest_fields(plan, "other", evidence)
        with self.assertRaises(ValueError):
            producer._catalog_pair_manifest_fields({"conditionId": condition}, condition, evidence)
        with self.assertRaises(ValueError):
            producer._catalog_pair_manifest_fields({**plan, "evidenceBinarySha256": "invalid"},
                                                   condition, evidence)

    def test_catalog_loader_rejects_previous_capture_manifest_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = root / "catalog-stage90-captures.json"
            manifest.write_text(json.dumps({
                "schema": "krr-html84-catalog-captures-v1",
                "profile": "catalog-stage90-captures",
                "acceptance": False,
            }))
            with self.assertRaisesRegex(ValueError, "catalog capture manifest schema differs"):
                producer.load_catalog_conditions(root, manifest)

    def _catalog_cli_fixture(
        self, temporary: str,
    ) -> tuple[Path, Path, Path, Path]:
        root = Path(temporary).resolve()
        return (root, root / "tmp/evidence", root / "capacity.json",
                root / "catalog-stage90-captures.json")

    def _catalog_cli_argv(
        self, root: Path, output: Path, capacity: Path, manifest: Path,
        checksum: str, *extra: str,
    ) -> list[str]:
        return [
            "produce_html_performance_evidence.py", "--repo", str(root),
            "--output", str(output), "--capacity-binding", str(capacity),
            "--capacity-binding-sha256", checksum, "--catalog-plan", str(manifest),
            *extra,
        ]

    def test_formal_cli_requires_catalog_before_calling_producer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(sys, "argv", [
                 "produce_html_performance_evidence.py",
                 "--repo", temporary,
                 "--output", str(Path(temporary) / "tmp/evidence"),
                 "--capacity-binding", str(Path(temporary) / "capacity.json"),
                 "--capacity-binding-sha256", "a" * 64,
             ]), \
             patch.object(sys, "stderr", io.StringIO()), \
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

    def test_cpu_observer_source_preflight_precedes_catalog_and_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with (patch.object(platform, "system", return_value="Darwin"),
                  patch.object(platform, "machine", return_value="arm64"),
                  patch.object(load_validation, "require_current_cpu_observer_sources",
                               side_effect=ValueError("CPU observer source differs")),
                  patch.object(producer, "load_catalog_conditions") as catalog,
                  patch.object(producer, "_measurement_command") as command):
                with self.assertRaisesRegex(ValueError, "CPU observer source differs"):
                    producer.produce(root, root / "tmp/evidence", catalog_manifest_path=root / "catalog.json")
            catalog.assert_not_called()
            command.assert_not_called()

    def test_formal_cli_forwards_catalog_manifest_to_producer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output, capacity, manifest = self._catalog_cli_fixture(temporary)
            with patch.object(sys, "argv", self._catalog_cli_argv(
                root, output, capacity, manifest, "b" * 64,
            )), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(root.resolve(), output.resolve(), capacity,
                                        "b" * 64, manifest.resolve())

    def test_diagnostic_cli_forwards_catalog_manifest_without_formal_acceptance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output, capacity, manifest = self._catalog_cli_fixture(temporary)
            with patch.object(sys, "argv", self._catalog_cli_argv(
                root, output, capacity, manifest, "d" * 64, "--diagnostic-only",
            )), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(root, output, capacity, "d" * 64, manifest,
                                        _allow_catalog_diagnostic=True)

    def test_batch_cli_forwards_max_conditions_without_changing_catalog_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root / "tmp/evidence"
            capacity = root / "capacity.json"
            manifest = root / "catalog-stage90-captures.json"
            with patch.object(sys, "argv", [
                "produce_html_performance_evidence.py", "--repo", str(root),
                "--output", str(output), "--capacity-binding", str(capacity),
                "--capacity-binding-sha256", "f" * 64, "--catalog-plan", str(manifest),
                "--max-conditions", "12",
            ]), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(root, output, capacity, "f" * 64, manifest,
                                        max_conditions=12)

    def test_batch_cli_forwards_closed_batch_resume(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root / "tmp/resumed"
            capacity = root / "capacity.json"
            manifest = root / "catalog-stage90-captures.json"
            batch = root / "tmp/first-batch"
            with patch.object(sys, "argv", [
                "produce_html_performance_evidence.py", "--repo", str(root),
                "--output", str(output), "--capacity-binding", str(capacity),
                "--capacity-binding-sha256", "g" * 64, "--catalog-plan", str(manifest),
                "--resume-batch", str(batch),
            ]), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(root, output, capacity, "g" * 64, manifest,
                                        resume_batch=batch)

    def test_reference_parent_cli_forwards_explicit_wrapper_pin(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root / "tmp/evidence"
            capacity = root / "capacity.json"
            manifest = root / "catalog-stage90-captures.json"
            wrapper = root / "reference-parent-wrapper.json"
            with patch.object(sys, "argv", [
                "produce_html_performance_evidence.py", "--repo", str(root),
                "--output", str(output), "--capacity-binding", str(capacity),
                "--capacity-binding-sha256", "h" * 64, "--catalog-plan", str(manifest),
                "--reference-parent-wrapper", str(wrapper),
                "--reference-parent-wrapper-sha256", "i" * 64,
            ]), patch.object(producer, "produce") as run:
                producer.main()
            run.assert_called_once_with(
                root, output, capacity, "h" * 64, manifest,
                reference_parent_wrapper=wrapper,
                reference_parent_wrapper_sha256="i" * 64,
            )

    def test_reference_parent_loader_failure_happens_before_measurements(self) -> None:
        initial: gate.JsonObject = {"head": "a" * 40, "files": [], "inventoryDigest": "b" * 64}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            wrapper = root / "reference-parent-wrapper.json"
            with patch.object(platform, "system", return_value="Darwin"), \
                 patch.object(platform, "machine", return_value="arm64"), \
                 patch.object(load_validation, "require_current_cpu_observer_sources"), \
                 patch.object(producer, "_make_git_query", return_value=lambda *_args: ""), \
                 patch.object(producer, "source", return_value=initial), \
                 patch.object(producer, "load_catalog_conditions",
                              return_value=(None, None, None, None)), \
                 patch.object(producer, "load_reference_parent",
                              side_effect=ValueError("reference parent rejected")), \
                 patch.object(producer, "_measurement_command") as command:
                with self.assertRaisesRegex(ValueError, "reference parent rejected"):
                    producer.produce(
                        root, root / "tmp/evidence", root / "capacity.json", "j" * 64,
                        root / "catalog.json", reference_parent_wrapper=wrapper,
                        reference_parent_wrapper_sha256="k" * 64,
                    )
            command.assert_not_called()

    def test_reference_parent_arm_keeps_original_source_role_and_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            baseline_binary = root / "baseline"
            candidate_binary = root / "candidate"
            baseline_binary.write_bytes(b"baseline")
            candidate_binary.write_bytes(b"candidate")
            parent = reference.ReferenceParentProof(
                reference.REFERENCE_ROLE, "candidate", "3" * 40,
                "a" * 64, gate.sha256(baseline_binary), reference.PARENT_LOCK_SHA256,
                "d" * 64, "e" * 64, "f" * 64, (),
                root / "parent-build.json", baseline_binary,
            )
            contract = reference.reference_parent_contract(parent)
            builds = {
                "baseline": producer.ArmBuildResult([], baseline_binary,
                                                     gate.sha256(baseline_binary), {}, []),
                "candidate": producer.ArmBuildResult([], candidate_binary,
                                                      gate.sha256(candidate_binary), {}, []),
            }
            refs: dict[str, typed_producer.FilePin] = {
                arm: {"path": str(root / (arm + ".json")), "sha256": "b" * 64}
                for arm in builds
            }
            arms = producer.expected_arm_pins(
                builds, "c" * 64, refs, reference_parent=parent,
                reference_contract=contract,
            )
            self.assertEqual(set(arms), {reference.REFERENCE_ROLE, "candidate"})
            self.assertEqual(arms[reference.REFERENCE_ROLE]["source_snapshot_sha256"], "a" * 64)
            self.assertEqual(arms[reference.REFERENCE_ROLE]["lock_sha256"], reference.PARENT_LOCK_SHA256)
            self.assertEqual(arms["candidate"]["source_snapshot_sha256"], "c" * 64)
            self.assertEqual(arms["candidate"]["lock_sha256"], gate.LOCK_SHA)
            fresh_contract: gate.JsonObject = {"role": "fresh-history-source-record",
                "armId": "fresh-history-source-record", "recordId": "record",
                "referenceKind": "fresh-history-source", "originalArm": "candidate", "publishedSdk": False}
            fresh_arms = producer.expected_arm_pins(
                builds, "c" * 64, refs, reference_contract=fresh_contract, fresh_snapshot_sha256="e" * 64)
            self.assertEqual(fresh_arms["fresh-history-source-record"]["source_snapshot_sha256"], "e" * 64)
            self.assertEqual(fresh_arms["candidate"]["source_snapshot_sha256"], "c" * 64)
            self.assertEqual(fresh_arms["fresh-history-source-record"]["binary_sha256"], gate.sha256(baseline_binary))

    def test_reference_parent_loader_forwards_query_and_all_dependency_refs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            wrapper = root / "wrapper.json"
            build = root / "build.json"
            binary = root / "binary"
            dependency = root / "dependency.raw"
            wrapper.write_text("{}")
            build.write_text("{}")
            binary.write_bytes(b"binary")
            dependency.write_bytes(b"dependency")
            proof = reference.ReferenceParentProof(
                reference.REFERENCE_ROLE, "candidate", "3" * 40,
                "a" * 64, gate.sha256(binary), gate.LOCK_SHA,
                "d" * 64, gate.sha256(build), gate.sha256(wrapper),
                (reference.FilePin(str(dependency), gate.sha256(dependency)),),
                build, binary,
            )

            def query(root: Path, args: Sequence[str], input_bytes: bytes | None,
                      text: bool) -> str:
                if root != Path(temporary).resolve() or input_bytes is not None or not text:
                    raise ValueError("unexpected Git query context")
                if tuple(args) == ("rev-parse", "HEAD"):
                    return "3" * 40
                if tuple(args) == ("ls-tree", "--name-only", "3" * 40, "--", reference.HISTORY_PATH):
                    return ""
                raise ValueError("unexpected Git query")

            with patch.object(reference, "load_reference_parent_proof",
                              return_value=proof) as loader:
                _, contract, _, refs = producer.load_reference_parent(
                    root, wrapper, gate.sha256(wrapper), git_query=query,
                )
            self.assertIsNotNone(loader.call_args.kwargs["git_query"])
            self.assertEqual(contract["dependencyRefs"],
                             [{"path": str(dependency), "sha256": gate.sha256(dependency)}])
            self.assertEqual({pin["path"] for pin in refs},
                             {str(wrapper), str(dependency)})

    def test_full_cli_holds_canonical_locks_during_production(self) -> None:
        for diagnostic in (False, True):
            with self.subTest(diagnostic_only=diagnostic), \
                 tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                output = root / "tmp/evidence"
                capacity = root / "capacity.json"
                manifest = root / "catalog-stage90-captures.json"

                def assert_lock_is_held(*_args: object, _root: Path = root,
                                         **_kwargs: object) -> None:
                    with self.assertRaisesRegex(RuntimeError, "another local review is active"):
                        with producer.candidate_build_locks(_root):
                            self.fail("full producer entered without the canonical lock")

                argv = [
                    "produce_html_performance_evidence.py", "--repo", str(root),
                    "--output", str(output), "--capacity-binding", str(capacity),
                    "--capacity-binding-sha256", "e" * 64, "--catalog-plan", str(manifest),
                ]
                if diagnostic:
                    argv.append("--diagnostic-only")
                with patch.object(sys, "argv", argv), \
                     patch.object(producer, "produce", side_effect=assert_lock_is_held):
                    producer.main()

    def test_candidate_build_cli_forwards_pinned_capacity_without_catalog(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root / "tmp/candidate-build"
            capacity = root / "capacity.json"
            with patch.object(sys, "argv", [
                "produce_html_performance_evidence.py", "--repo", str(root),
                "--output", str(output), "--capacity-binding", str(capacity),
                "--capacity-binding-sha256", "c" * 64, "--candidate-build-only",
            ]), patch.object(sys, "stdout", io.StringIO()), \
                 patch.object(producer, "produce_candidate_build_proof",
                              return_value={"buildProofValidated": True}) as run:
                producer.main()
            run.assert_called_once_with(root, output, capacity, "c" * 64)

    def test_candidate_build_cli_rejects_existing_review_lock(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with producer.candidate_build_locks(root):
                with self.assertRaisesRegex(RuntimeError, "another local review is active"):
                    with producer.candidate_build_locks(root):
                        self.fail("competing candidate producer entered its critical section")

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


class TransportBundleUnitTests(unittest.TestCase):
    def test_formal_receipt_is_exclusive_and_binds_verified_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle = Path(temporary).resolve()
            manifest = bundle / "bundle.json"
            manifest.write_text("{}")
            head = "a" * 40
            result: gate.JsonObject = {"head": head, "conditions": 84, "candidateConditions": 84,
                                       "performanceNonregression": True, "htmlStage90": {
                                           "acceptance": True, "fontIdentityEquivalence": "established"}}
            producer.write_formal_verification_receipt(bundle, head, result)
            receipt = gate.read_json(bundle / "producer-verification.json")
            self.assertEqual(receipt["bundleManifestSha256"], gate.sha256(manifest))
            self.assertEqual(receipt["result"], result)
            with self.assertRaises(FileExistsError):
                producer.write_formal_verification_receipt(bundle, head, result)

    def capacity_binding(self, root: Path) -> typed_producer.CapacityBinding:
        document, state, registry = (root / name for name in
                                     ("binding.json", "state.json", "registry.json"))
        for path in (document, state, registry):
            path.write_text(path.name)
        return producer.CapacityBinding(
            document, gate.sha256(document), state, gate.sha256(state),
            registry, gate.sha256(registry), (),
        )

    def test_capacity_inputs_are_transported_with_recorded_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binding = self.capacity_binding(root)
            capacity_input = producer.write(root / "capacity-inputs.json", {
                "stateRef": {"path": str(binding.state), "sha256": binding.state_sha256},
                "protectedRegistryRef": {"path": str(binding.registry), "sha256": binding.registry_sha256},
            })
            incomplete = root / "incomplete"
            producer.write_transport_bundle(incomplete, "a" * 40, [capacity_input], [])
            missing = gate.Bundle(incomplete)
            with self.assertRaises(KeyError):
                gate._recorded_capacity_state(missing, missing.json(capacity_input))
            refs = [capacity_input, *producer.capacity_transport_refs(binding)]
            directory = root / "complete"
            producer.write_transport_bundle(directory, "a" * 40, refs, [])
            bundle = gate.Bundle(directory)
            value = bundle.json(capacity_input)
            self.assertEqual(gate._recorded_capacity_state(bundle, value),
                             (binding.state, binding.state_sha256))
            self.assertEqual(bundle.file(value["protectedRegistryRef"]).read_text(), binding.registry.name)
            self.assertEqual(bundle.file(refs[1]).read_text(), binding.document.name)
            bundle.file(value["stateRef"]).write_text("changed")
            with self.assertRaisesRegex(ValueError, "original pin mismatch"):
                gate._recorded_capacity_state(bundle, value)

    def test_shared_bundle_writer_keeps_release_bundle_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source.json"
            source.write_text("{}")
            pin: typed_producer.FilePin = {"path": str(source), "sha256": gate.sha256(source)}
            attempts: list[dict[str, typed_producer.FilePin]] = [
                {"expected": pin, "result": pin, "pairManifest": pin}]
            bundle = root / "bundle"
            relative_paths = producer.write_transport_bundle(bundle, "a" * 40, [pin, pin], attempts)
            manifest = gate.Bundle(bundle)
            self.assertEqual(manifest.manifest["schema"], "krr-html-performance-release-bundle-v1")
            self.assertEqual(manifest.manifest["head"], "a" * 40)
            self.assertEqual(manifest.manifest["attempts"], attempts)
            self.assertEqual(len(gate._array(manifest.manifest["files"], "bundle files")), 1)
            self.assertEqual(relative_paths[str(source)], "payload/0/source.json")
            self.assertEqual(manifest.file(pin).read_text(), "{}")

    def test_shared_bundle_writer_preserves_executable_and_private_modes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            executable = root / "candidate"
            private = root / "private.raw"
            executable.write_bytes(b"candidate bytes")
            private.write_bytes(b"private bytes")
            executable.chmod(0o555)
            private.chmod(0o600)
            pins: list[typed_producer.FilePin] = [
                {"path": str(path), "sha256": gate.sha256(path)}
                for path in (executable, private)
            ]
            bundle = root / "bundle"
            relative_paths = producer.write_transport_bundle(bundle, "a" * 40, pins, [])
            transported_executable = bundle / relative_paths[str(executable)]
            transported_private = bundle / relative_paths[str(private)]
            self.assertEqual(stat.S_IMODE(transported_executable.stat().st_mode), 0o555)
            self.assertEqual(stat.S_IMODE(transported_private.stat().st_mode), 0o600)
            if os.name != "nt":
                self.assertTrue(os.access(transported_executable, os.X_OK))
                self.assertFalse(os.access(transported_private, os.X_OK))
            self.assertEqual(gate.sha256(transported_executable), pins[0]["sha256"])
            self.assertEqual(gate.sha256(transported_private), pins[1]["sha256"])

    def test_historical_transport_preserves_logical_pin_and_rejects_changed_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            payload = root / "old-payload"
            payload.write_bytes(b"original")
            logical = str(root / "original-path")
            pin: typed_producer.FilePin = {"path": logical, "sha256": gate.sha256(payload)}
            producer.write_transport_bundle(root / "bundle", "a" * 40, [pin], [],
                                            source_paths={logical: payload})
            self.assertEqual(gate.Bundle(root / "bundle").file(pin).read_bytes(), b"original")
            payload.write_bytes(b"modified")
            with self.assertRaises(ValueError):
                producer.write_transport_bundle(root / "changed", "a" * 40, [pin], [],
                                                source_paths={logical: payload})

    @unittest.skipUnless(platform.system() == "Darwin", "macOS clone contract")
    def test_transport_clone_is_independent_and_keeps_source_stamp(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            target = root / "target"
            source.write_bytes(b"original")
            before = source.lstat()
            producer._clone_transport_file(source, target, gate.sha256(source))
            after = source.lstat()
            self.assertEqual(producer._file_identity(before), producer._file_identity(after))
            self.assertEqual(before.st_mode, after.st_mode)
            self.assertNotEqual((before.st_dev, before.st_ino), (target.stat().st_dev, target.stat().st_ino))
            target.write_bytes(b"clone changed")
            self.assertEqual(source.read_bytes(), b"original")
            source.write_bytes(b"source changed")
            self.assertEqual(target.read_bytes(), b"clone changed")
            with self.assertRaises(ValueError):
                producer._clone_transport_file(source, target, gate.sha256(source))
            with self.assertRaises(ValueError):
                producer._clone_transport_file(source, source, gate.sha256(source))

    @unittest.skipUnless(platform.system() == "Darwin", "macOS clone contract")
    def test_transport_copy_failure_is_not_suppressed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            target = root / "target"
            source.write_bytes(b"original")
            with patch("reflink_copy.reflink_or_copy",
                       side_effect=OSError(errno.ENOTSUP, "copy unsupported")):
                with self.assertRaises(OSError):
                    producer._clone_transport_file(source, target, gate.sha256(source))
            self.assertFalse(target.exists())

    @unittest.skipUnless(platform.system() == "Darwin", "macOS system-volume transport")
    def test_transport_copies_readonly_system_volume_file_without_changing_source(self) -> None:
        source = Path("/System/Library/CoreServices/SystemVersion.plist").resolve()
        before = source.lstat()
        checksum = gate.sha256(source)
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary).resolve() / "system-version-copy.plist"
            producer._clone_transport_file(source, target, checksum)
            after = source.lstat()
            self.assertEqual(producer._file_identity(before), producer._file_identity(after))
            self.assertEqual(before.st_mode, after.st_mode)
            self.assertNotEqual((before.st_dev, before.st_ino),
                                (target.stat().st_dev, target.stat().st_ino))
            self.assertEqual(gate.sha256(target), checksum)
            self.assertEqual(gate.sha256(source), checksum)
            with self.assertRaises(ValueError):
                producer._clone_transport_file(source, target, checksum)


class HtmlPerformanceBatchUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self._batch_counter = 0

    def test_catalog_default_batch_has_the_existing_budget_and_legacy_does_not(self) -> None:
        catalog_mode = producer._batch_mode(self.root / "catalog.json", None, None)
        legacy_mode = producer._batch_mode(None, None, None)
        self.assertTrue(catalog_mode)
        self.assertFalse(legacy_mode)
        self.assertEqual(
            producer._batch_deadline(100.0, catalog_mode),
            100.0 + producer.BATCH_WORK_BUDGET_SECONDS,
        )
        self.assertIsNone(producer._batch_deadline(100.0, legacy_mode))
        self.assertTrue(producer._batch_mode(None, 1, None))
        self.assertTrue(producer._batch_mode(None, None, self.root / "resume"))

    def test_completed_condition_ids_use_only_the_measured_planned_prefix(self) -> None:
        planned: list[gate.JsonObject] = [
            {"condition_id": "case-a"}, {"condition_id": "case-b"},
        ]
        measured: list[tuple[Path, typed_producer.ExpectedPerformance,
                             gate.JsonObject, typed_producer.FilePin]] = [
            (self.root, cast(typed_producer.ExpectedPerformance, {}),
             {"policy": {"status": "complete"}}, cast(typed_producer.FilePin, {})),
        ]
        self.assertEqual(producer._completed_batch_ids([], planned, planned[:1], measured),
                         ["case-a"])
        with self.assertRaisesRegex(ValueError, "planned prefix"):
            producer._completed_batch_ids([], planned, planned[1:], measured)

    def test_summary_counts_actual_phase_records_and_rejects_invalid_values(self) -> None:
        def result(records: object) -> tuple[Path, typed_producer.ExpectedPerformance,
                                             gate.JsonObject, typed_producer.FilePin]:
            return (self.root, cast(typed_producer.ExpectedPerformance, {}),
                    {"policy": {"phaseRecords": records}},
                    cast(typed_producer.FilePin, {}))

        self.assertEqual(producer._measured_phase_record_count([
            result(300), result(150),
        ]), 450)
        saved: list[gate.JsonObject] = [
            {"policy": {"phaseRecords": 300}} for _ in range(72)
        ]
        saved.extend({"policy": {"phaseRecords": 150}} for _ in range(12))
        self.assertEqual(producer._saved_phase_record_count(saved), 23_400)
        for invalid in (None, -1, True):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    producer._measured_phase_record_count([result(invalid)])

    def create_batch(
        self,
        condition_ids: Sequence[str] = ("case-a", "case-b"),
        complete_ids: Sequence[str] | None = None,
        policy_accepted: bool = True,
        seal_receipt: bool = True,
    ) -> tuple[Path, Path]:
        self._batch_counter += 1
        batch = self.root / f"batch-{self._batch_counter}"
        files: list[typed_producer.FilePin] = []
        source_before = producer.write(batch / "source-before.json", {"head": "a"})
        source_end = producer.write(batch / "source-end.json", {"head": "a"})
        context_before = producer.write(batch / "context-before.json", {"stable": True})
        context_end = producer.write(batch / "context-end.json", {"stable": True})
        files.extend([source_before, source_end, context_before, context_end])
        attempts: list[dict[str, typed_producer.FilePin]] = []
        for condition_id in condition_ids:
            expected = producer.write(batch / f"{condition_id}-expected.json", {
                "catalogCondition": {"condition_id": condition_id},
            })
            result = producer.write(batch / f"{condition_id}-result.json", {})
            manifest = producer.write(batch / f"{condition_id}-manifest.json", {})
            attempts.append({"expected": expected, "result": result, "pairManifest": manifest})
            files.extend([expected, result, manifest])
        bundle = batch / "bundle"
        producer.write_transport_bundle(bundle, "a" * 40, files, attempts)
        if seal_receipt:
            producer.write_batch_receipt(
                batch, head="a" * 40, source_before=source_before, source_end=source_end,
                context_before=context_before, context_end=context_end, build_artifacts={},
                build_pins=[], condition_ids=condition_ids,
                complete_condition_ids=condition_ids if complete_ids is None else complete_ids,
                policy_accepted=policy_accepted, attempts=attempts, bundle=bundle,
            )
        return batch, bundle

    def test_resume_requires_original_context_at_both_boundaries(self) -> None:
        batch, bundle_path = self.create_batch(condition_ids=())
        receipt = gate.read_json(batch / "batch-receipt.json")
        stable = cast(typed_producer.ContextSnapshot, {"stable": True})
        changed = cast(typed_producer.ContextSnapshot, {"stable": False})
        for before, after, error in (
            (stable, stable, None),
            (changed, stable, "resume context-before differs"),
            (stable, changed, "resume context-end differs"),
        ):
            with self.subTest(error=error):
                expectation = self.assertRaisesRegex(ValueError, error) if error else nullcontext()
                with expectation:
                    producer._validate_resume_batch(
                        receipt, gate.Bundle(bundle_path), [], head="a" * 40,
                        source={"head": "a"}, before_context=before,
                        after_context=after, arms={},
                    )

    def test_resume_rejects_old_or_modified_worker_load_before_measurement(self) -> None:
        valid = {"schema": "krr-host-cpu-busy-v1", "method": "psutil.cpu_times_percent",
                 "version": "7.2.2", "sampleIntervalSeconds": host_load.INTERVAL_SECONDS,
                 "intervalBeforeSeconds": host_load.INTERVAL_SECONDS, "intervalDuringSeconds": 1.0,
                 "cpuTimesBefore": cpu_times(10.0, 90.0),
                 "cpuTimesDuring": cpu_times(10.0, 90.0)}
        for index, tag in enumerate((valid, None, {**valid, "method": "loadavg"},
                                     {**valid, "intervalDuringSeconds": 0.0})):
            with self.subTest(index=index):
                root = self.root / f"cpu-resume-{index}"
                worker: gate.JsonObject = {"accepted": True, "loadRatioBefore": 0.1, "loadRatioAfter": 0.1}
                if tag is not None:
                    worker["hostLoad"] = tag
                pin = producer.write(root / "attempt.json", worker)
                manifest = producer.write(root / "manifest.json", {
                    "order": [{"workerAttempts": [{**worker, "attemptRecord": pin}]}],
                })
                bundle_path = root / "bundle"
                producer.write_transport_bundle(bundle_path, "a" * 40, [pin, manifest], [])
                bundle = gate.Bundle(bundle_path)
                attempt = {"pairManifest": manifest}
                expectation = nullcontext() if index == 0 else self.assertRaises(ValueError)
                with expectation:
                    producer._require_cpu_busy_resume(bundle, [attempt])

    def test_resume_retains_unobserved_retry_and_later_valid_worker(self) -> None:
        root = self.root / "cpu-resume-retry"
        tag: gate.JsonObject = {"schema": "krr-host-cpu-busy-v1", "method": "psutil.cpu_times_percent",
            "version": "7.2.2", "sampleIntervalSeconds": host_load.INTERVAL_SECONDS,
            "intervalBeforeSeconds": host_load.INTERVAL_SECONDS,
            "cpuTimesBefore": cpu_times(10.0, 90.0)}
        rows: list[gate.JsonObject] = [
            {"accepted": False, "reason": "host load observation unavailable", "loadRatioBefore": 0.1,
             "loadRatioAfter": None, "hostLoad": {**tag, "intervalDuringSeconds": 1.0,
                                                     "cpuTimesDuring": cpu_times(0.0, 0.0)}},
            {"accepted": True, "loadRatioBefore": 0.1, "loadRatioAfter": 0.1,
             "hostLoad": {**tag, "intervalDuringSeconds": 1.0,
                          "cpuTimesDuring": cpu_times(10.0, 90.0)}},
        ]
        pins = [producer.write(root / f"attempt-{index}.json", row) for index, row in enumerate(rows)]
        manifest = producer.write(root / "manifest.json", {
            "order": [{"workerAttempts": [{**row, "attemptRecord": pin}
                                          for row, pin in zip(rows, pins, strict=True)]}],
        })
        bundle_path = root / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40, [*pins, manifest], [])
        producer._require_cpu_busy_resume(gate.Bundle(bundle_path), [{"pairManifest": manifest}])

    def resume_context_fixture(
        self, directory: Path, rows: list[gate.JsonObject],
        pins: Sequence[typed_producer.FilePin],
    ) -> tuple[gate.Bundle, gate.JsonObject]:
        context = producer.write(directory / "context-before.json", {"inputs": rows})
        bundle_path = directory / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40, [*pins, context], [])
        return gate.Bundle(bundle_path), {"contexts": {"before": context}}

    def closed_plan_batch(
        self, directory: Path, rows: list[gate.JsonObject],
        pins: Sequence[typed_producer.FilePin],
        catalog_validation: typed_producer.FilePin | None = None,
    ) -> tuple[gate.Bundle, gate.JsonObject]:
        source = producer.write(directory / "source.json", {"head": "a" * 40})
        context = producer.write(directory / "context.json", {"inputs": rows})
        bundle_path = directory / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40, [source, context, *pins], [])
        producer.write_batch_receipt(
            directory, head="a" * 40, source_before=source, source_end=source,
            context_before=context, context_end=context, build_artifacts={}, build_pins=[],
            condition_ids=[], complete_condition_ids=[], policy_accepted=True,
            attempts=[], bundle=bundle_path, catalog_validation=catalog_validation,
        )
        receipt, bundle, _ = producer.load_closed_batch(directory, self.root)
        return bundle, receipt

    def test_resume_plan_survives_multiple_closed_batches_without_repinning(self) -> None:
        condition = "async/expected-state/1280x900@1"
        label = "async_expected-state_1280x900_1"
        plan: gate.JsonObject = {"conditionId": condition, "operationStream": []}
        original = producer.write(self.root / "fresh/conditions" / label / "operation-plan.json", plan)
        rows: list[gate.JsonObject] = [{**original, "exists": True}]
        bundle, receipt = self.closed_plan_batch(self.root / "fresh", rows, [original])
        for batch in ("resume-one", "resume-two"):
            old_path, old_plan, pin = producer._resume_condition_plan(bundle, receipt, condition, plan)
            self.assertEqual(pin, original)
            self.assertEqual(old_path, Path(original["path"]))
            self.assertEqual(old_plan, plan)
            bundle, receipt = self.closed_plan_batch(self.root / batch, rows, [original])
        old_path, _, pin = producer._resume_condition_plan(bundle, receipt, condition, plan)
        self.assertEqual((old_path, pin), (Path(original["path"]), original))

    def test_resume_plan_rejects_missing_duplicate_and_changed_inputs(self) -> None:
        condition = "case/state/1280x900@1"
        label = "case_state_1280x900_1"
        plan: gate.JsonObject = {"conditionId": condition, "operationStream": []}
        for case in ("missing", "duplicate", "unavailable", "content", "tampered",
                     "sha", "bundle-missing", "wrong-condition"):
            with self.subTest(case=case):
                directory = self.root / case
                stored: gate.JsonObject = ({**plan, "conditionId": "another"}
                                           if case == "wrong-condition" else plan)
                original = producer.write(directory / "origin/conditions" / label / "operation-plan.json", stored)
                row: gate.JsonObject = {**original, "exists": case != "unavailable"}
                if case == "sha":
                    row["sha256"] = "0" * 64
                rows = [] if case == "missing" else [row, row] if case == "duplicate" else [row]
                pins = [] if case == "bundle-missing" else [original]
                bundle, receipt = self.closed_plan_batch(directory / "batch", rows, pins)
                if case == "tampered":
                    Path(original["path"]).write_text("changed", encoding="utf-8")
                current: gate.JsonObject = ({**plan, "operationStream": ["changed"]}
                                            if case == "content" else plan)
                with self.assertRaises(ValueError):
                    producer._resume_condition_plan(bundle, receipt, condition, current)

    def test_resume_reuses_context_snapshot_across_output_directories(self) -> None:
        old = producer.write(self.root / "old/shared-source-snapshot.json", {"records": []})
        new = producer.write(self.root / "new/shared-source-snapshot.json", {"records": []})
        artifact = producer.write(self.root / "old/build-artifact.json", {"sourceSnapshotRef": old})
        bundle, receipt = self.resume_context_fixture(
            self.root / "old", [{**old, "exists": True}], [old, artifact],
        )
        receipt["buildArtifacts"] = {"candidate": artifact}
        role = producer._resume_candidate_snapshot_pin(bundle, receipt)
        selected = producer._resume_context_pin(bundle, receipt, new, role)
        self.assertEqual(selected, old)
        self.assertNotEqual(selected["path"], new["path"])
        current = producer.write(self.root / "new/shared-source-snapshot.json", {"records": [1]})
        with self.assertRaisesRegex(ValueError, "resume context input differs"):
            producer._resume_context_pin(bundle, receipt, current, role)
        producer.write(self.root / "new/shared-source-snapshot.json", {"records": []})
        Path(old["path"]).write_text("tampered")
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            producer._resume_context_pin(bundle, receipt, new, role)

    def test_resume_reuses_context_capture_validation_across_output_directories(self) -> None:
        name = "catalog-chrome-capture-validation.json"
        old = producer.write(self.root / "old" / name, {"status": "passed"})
        new = producer.write(self.root / "new" / name, {"status": "passed"})
        expected = producer.write(self.root / "old/expected.json", {
            "catalogCaptureValidationRef": old,
        })
        bundle, receipt = self.resume_context_fixture(
            self.root / "old", [{**old, "exists": True}], [old, expected],
        )
        receipt["attempts"] = [{"expected": expected}]
        role = producer._resume_catalog_validation_pin(bundle, receipt)
        self.assertEqual(producer._resume_context_pin(bundle, receipt, new, role), old)

    def test_empty_resume_finds_capture_validation_in_original_context(self) -> None:
        name = "catalog-chrome-capture-validation.json"
        old = producer.write(self.root / "old" / name, {"status": "passed"})
        new = producer.write(self.root / "new" / name, {"status": "passed"})
        bundle, receipt = self.resume_context_fixture(
            self.root / "old", [{**old, "exists": True}], [old],
        )
        receipt["attempts"] = []
        role = producer._resume_catalog_validation_pin(bundle, receipt, Path(old["path"]))
        self.assertEqual(producer._resume_context_pin(bundle, receipt, new, role), old)
        changed = producer.write(self.root / "new" / name, {"status": "changed"})
        with self.assertRaisesRegex(ValueError, "resume context input differs"):
            producer._resume_context_pin(bundle, receipt, changed, role)

    def test_empty_resume_preserves_catalog_role_across_two_closed_batches(self) -> None:
        name = "catalog-chrome-capture-validation.json"
        original = producer.write(self.root / "fresh" / name, {"status": "passed"})
        rows: list[gate.JsonObject] = [{**original, "exists": True}]
        bundle, receipt = self.closed_plan_batch(self.root / "fresh", rows, [original])
        for batch in ("resume-one", "resume-two"):
            previous = Path(gate._text(gate._object(receipt["bundle"], "bundle")["path"], "path")).parent.parent
            role = producer._resume_catalog_validation_pin(bundle, receipt, previous / name)
            current = producer.write(self.root / batch / name, {"status": "passed"})
            selected = producer._resume_context_pin(bundle, receipt, current, role)
            self.assertEqual(selected, original)
            bundle, receipt = self.closed_plan_batch(self.root / batch, rows, [original], selected)
        self.assertEqual(producer._resume_catalog_validation_pin(bundle, receipt), original)
        receipt["catalogCaptureValidationRef"] = {**original, "sha256": "0" * 64}
        with self.assertRaises(ValueError):
            producer._resume_catalog_validation_pin(bundle, receipt)

    def test_resume_snapshot_uses_declared_role_with_same_basename(self) -> None:
        for same_digest in (False, True):
            with self.subTest(same_digest=same_digest):
                directory = self.root / str(same_digest)
                old = producer.write(directory / "candidate/shared-source-snapshot.json", {"records": []})
                other = producer.write(directory / "parent/shared-source-snapshot.json",
                                       {"records": [] if same_digest else [1]})
                current = producer.write(directory / "new/shared-source-snapshot.json", {"records": []})
                artifact = producer.write(directory / "candidate/build-artifact.json",
                                          {"sourceSnapshotRef": old})
                bundle, receipt = self.resume_context_fixture(
                    directory, [{**other, "exists": True}, {**old, "exists": True}],
                    [old, other, artifact],
                )
                receipt["buildArtifacts"] = {"candidate": artifact}
                role = producer._resume_candidate_snapshot_pin(bundle, receipt)
                self.assertEqual(producer._resume_context_pin(bundle, receipt, current, role), old)

    def test_resume_context_rejects_missing_or_duplicate_declared_row(self) -> None:
        for rows in (0, 2):
            with self.subTest(rows=rows):
                directory = self.root / str(rows)
                old = producer.write(directory / "old/shared-source-snapshot.json", {"records": []})
                current = producer.write(directory / "new/shared-source-snapshot.json", {"records": []})
                bundle, receipt = self.resume_context_fixture(
                    directory, [{**old, "exists": True}] * rows, [old],
                )
                with self.assertRaisesRegex(ValueError, "ambiguous or missing"):
                    producer._resume_context_pin(bundle, receipt, current, old)

    def test_resume_context_rejects_missing_or_mismatched_row_identity(self) -> None:
        for case in ("missing", "sha"):
            with self.subTest(case=case):
                directory = self.root / case
                old = producer.write(directory / "old/shared-source-snapshot.json", {"records": []})
                current = producer.write(directory / "new/shared-source-snapshot.json", {"records": []})
                row = {**old, "exists": True}
                if case == "missing":
                    row["exists"] = False
                else:
                    row["sha256"] = "0" * 64
                bundle, receipt = self.resume_context_fixture(directory, [row], [old])
                with self.assertRaisesRegex(ValueError, "resume context input differs"):
                    producer._resume_context_pin(bundle, receipt, current, old)

    def test_resume_rejects_missing_role_declarations_and_conflicting_catalog_pins(self) -> None:
        directory = self.root / "roles"
        first = producer.write(directory / "first.json", {"value": 1})
        second = producer.write(directory / "second.json", {"value": 2})
        missing = producer.write(directory / "missing.json", {})
        expected_a = producer.write(directory / "expected-a.json", {"catalogCaptureValidationRef": first})
        expected_b = producer.write(directory / "expected-b.json", {"catalogCaptureValidationRef": second})
        bundle_path = directory / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40,
                                        [first, second, missing, expected_a, expected_b], [])
        bundle = gate.Bundle(bundle_path)
        with self.assertRaises((ValueError, TypeError)):
            producer._resume_candidate_snapshot_pin(bundle, {"buildArtifacts": {"candidate": missing}})
        for attempts in ([], [{"expected": missing}],
                         [{"expected": expected_a}, {"expected": expected_b}]):
            with self.subTest(attempts=len(attempts)), self.assertRaises((ValueError, TypeError)):
                producer._resume_catalog_validation_pin(bundle, {"attempts": attempts})

    def test_resume_builds_reuse_only_the_declared_frozen_binary(self) -> None:
        directory = self.root / "resume-builds"
        pins: dict[str, typed_producer.FilePin] = {}
        artifacts: dict[str, typed_producer.FilePin] = {}
        for role in ("baseline", "candidate"):
            binary = directory / role / "binary"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"same executable")
            binary.chmod(0o755)
            pins[role] = {"path": str(binary), "sha256": gate.sha256(binary)}
            artifacts[role] = producer.write(directory / role / "artifact.json",
                                             {"frozenBinary": pins[role]})
        arms: gate.JsonObject = {
            gate.DEFAULT_BASELINE_ARM_ID: {"build_artifact_ref": artifacts["baseline"],
                                          "binary_sha256": pins["baseline"]["sha256"]},
            "candidate": {"build_artifact_ref": artifacts["candidate"],
                          "binary_sha256": pins["candidate"]["sha256"]},
        }
        expected = producer.write(directory / "expected.json", {"arms": arms})
        bundle_path = directory / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40,
                                        [*pins.values(), *artifacts.values(), expected], [])
        bundle = gate.Bundle(bundle_path)
        receipt: gate.JsonObject = {"buildArtifacts": artifacts, "buildPins": []}
        attempts = [{"expected": expected}]
        builds, _, _ = producer._resume_builds(bundle, receipt, attempts)
        self.assertEqual(str(builds["candidate"].binary), pins["candidate"]["path"])
        empty_builds, _, _ = producer._resume_builds(bundle, receipt, [])
        self.assertEqual(str(empty_builds["candidate"].binary), pins["candidate"]["path"])
        bad_receipt: gate.JsonObject = {"buildArtifacts": {"baseline": artifacts["baseline"],
                                                          "candidate": artifacts["baseline"]},
                                       "buildPins": []}
        with self.assertRaisesRegex(ValueError, "role proof differs"):
            producer._resume_builds(bundle, bad_receipt, attempts)
        candidate = Path(pins["candidate"]["path"])
        candidate.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            producer._resume_builds(bundle, receipt, attempts)
        candidate.unlink()
        with self.assertRaises(ValueError):
            producer._resume_builds(bundle, receipt, attempts)
        candidate.symlink_to(Path(pins["baseline"]["path"]))
        with self.assertRaisesRegex(ValueError, "canonical regular file"):
            producer._resume_builds(bundle, receipt, attempts)

    def test_empty_resume_build_validation_checks_arm_roles_and_identity(self) -> None:
        directory = self.root / "empty-build-validation"
        source = producer.write(directory / "source.json", {"head": "a" * 40})
        context = producer.write(directory / "context.json", {"inputs": []})
        artifacts = {role: producer.write(directory / f"{role}.json", {"role": role})
                     for role in ("baseline", "candidate")}
        bundle_path = directory / "bundle"
        producer.write_transport_bundle(bundle_path, "a" * 40, [source, context, *artifacts.values()], [])
        bundle = gate.Bundle(bundle_path)
        receipt: gate.JsonObject = {"source": {"before": source}, "contexts": {"before": context}}
        arms = cast(dict[str, typed_producer.ArmPin], {
            gate.DEFAULT_BASELINE_ARM_ID if role == "baseline" else role: {
                "binary_sha256": role, "source_snapshot_sha256": "snapshot",
                "lock_sha256": "lock", "build_artifact_ref": pin,
            } for role, pin in artifacts.items()
        })

        def validate_proof(_bundle: gate.Bundle, _root: Path, ref: object, role: str,
                           **_kwargs: object) -> gate.JsonObject:
            gate.require(ref == artifacts[role], "build role differs")
            return {"binarySha256": role, "sourceSnapshotSha256": "snapshot", "lockSha256": "lock"}

        with patch.object(reference, "active_reference_at_commit", return_value=None), \
             patch.object(gate, "_validate_build_proof", side_effect=validate_proof):
            producer._validate_empty_resume_builds(bundle, self.root, receipt, arms, None, git_query=None)
            baseline = arms[gate.DEFAULT_BASELINE_ARM_ID]
            baseline["build_artifact_ref"] = artifacts["candidate"]
            with self.assertRaisesRegex(ValueError, "build role differs"):
                producer._validate_empty_resume_builds(bundle, self.root, receipt, arms, None, git_query=None)
            baseline["build_artifact_ref"] = artifacts["baseline"]
            baseline["binary_sha256"] = "tampered"
            with self.assertRaisesRegex(ValueError, "resume verified build identity differs"):
                producer._validate_empty_resume_builds(bundle, self.root, receipt, arms, None, git_query=None)

    def test_context_preflight_checks_every_catalog_input_reference(self) -> None:
        keys = ("operationPlanRef", "domStateRef", "operationEvidenceRef", "catalogManifestRef",
                "catalogPlanRef", "catalogCaptureValidationRef", "catalogBinaryProofRef",
                "catalogBuildReportRef", "catalogCaptureBinaryRef", "catalogCurrentBuildReportRef",
                "chromeCaptureRef", "chromeReceiptRef", "fontIdentityRequestRef",
                "fontIdentityResponseRef", "fontIdentityProbeBinary")
        expected: gate.JsonObject = {key: {"path": key, "sha256": key} for key in keys}
        extra: list[gate.JsonObject] = [
            {"path": key, "sha256": key} for key in ("snapshot", "capture", "link", "font")
        ]
        refs = [cast(gate.JsonObject, expected[key]) for key in keys] + extra
        execution: list[gate.JsonObject] = [{"path": name, "sha256": name}
                                            for name in ("binary", "fixture", "lock")]
        refs.extend(execution)
        context: gate.JsonObject = {"inputs": [{**ref, "exists": True} for ref in refs]}
        def validate(value: gate.JsonObject) -> None:
            gate.validate_context_input_refs(value, expected, extra[0], extra[1], [extra[2]], [extra[3]], execution)
        validate(context)
        checked = gate.validate_context_input_refs(context, expected, extra[0], extra[1], [extra[2]], [extra[3]], execution)
        self.assertEqual(checked["binary"]["sha256"], "binary")
        for ref in refs:
            rows = [{**item, "exists": True} for item in refs if item != ref]
            with self.subTest(path=ref["path"]), self.assertRaisesRegex(ValueError, "context is missing"):
                validate({"inputs": rows})
        for change in ({"exists": False}, {"sha256": "different"}):
            rows = [{**ref, "exists": True} for ref in refs]
            rows[0].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "context differs"):
                validate({"inputs": rows})

    def test_planned_input_validation_rejects_later_missing_references(self) -> None:
        snapshot: gate.JsonObject = {"path": "snapshot", "sha256": "snapshot"}
        first: gate.JsonObject = {"operationPlanRef": {"path": "first", "sha256": "first"},
                                  "domStateRef": {"path": "dom", "sha256": "dom"}}
        later: gate.JsonObject = {"operationPlanRef": {"path": "later", "sha256": "later"},
                                  "domStateRef": {"path": "dom", "sha256": "dom"}}
        execution: list[gate.JsonObject] = [{"path": name, "sha256": name}
                                            for name in ("binary", "fixture", "lock")]
        refs = [snapshot, cast(gate.JsonObject, first["operationPlanRef"]),
                cast(gate.JsonObject, first["domStateRef"]),
                cast(gate.JsonObject, later["operationPlanRef"]), *execution]
        plans: list[tuple[gate.JsonObject, gate.JsonObject, gate.JsonObject | None,
                          list[gate.JsonObject], list[gate.JsonObject], list[gate.JsonObject]]] = [
            (expected, snapshot, None, [], [], execution) for expected in (first, later)
        ]
        producer._validate_planned_inputs({"inputs": [{**ref, "exists": True} for ref in refs]}, plans)
        for absent in ("later", "binary", "fixture", "lock"):
            context: gate.JsonObject = {"inputs": [{**ref, "exists": True} for ref in refs
                                                   if ref["path"] != absent]}
            with self.subTest(absent=absent):
                with self.assertRaisesRegex(ValueError, "context is missing"):
                    producer._validate_planned_inputs(context, plans)

    def test_producer_unconditionally_validates_all_planned_rows_before_worker_loop(self) -> None:
        parsed = ast.parse(inspect.getsource(producer.produce))
        function = parsed.body[0]
        self.assertIsInstance(function, ast.FunctionDef)
        if not isinstance(function, ast.FunctionDef):
            raise AssertionError("expected producer function")

        def calls(node: ast.AST, name: str) -> bool:
            return any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                       and item.func.id == name for item in ast.walk(node))

        assembly = [index for index, node in enumerate(function.body)
                    if isinstance(node, ast.For) and isinstance(node.iter, ast.Name)
                    and node.iter.id == "all_measurement_rows"
                    and calls(node, "planned_expected")]
        validation = [index for index, node in enumerate(function.body)
                      if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
                      and isinstance(node.value.func, ast.Name)
                      and node.value.func.id == "_validate_planned_inputs"
                      and any(isinstance(arg, ast.Name) and arg.id == "planned_inputs"
                              for arg in node.value.args)]
        worker = [index for index, node in enumerate(function.body)
                  if isinstance(node, ast.For) and isinstance(node.iter, ast.Name)
                  and node.iter.id == "measurement_rows"
                  and calls(node, "_collect_condition_worker_pairs")]
        self.assertEqual(len(assembly), 1)
        self.assertEqual(len(validation), 1)
        self.assertEqual(len(worker), 1)
        self.assertLess(assembly[0], validation[0])
        self.assertLess(validation[0], worker[0])
        self.assertTrue(all(not calls(node, "_collect_condition_worker_pairs")
                            for node in function.body[:validation[0]]))
        row_loop = function.body[assembly[0]]
        self.assertIsInstance(row_loop, ast.For)
        if not isinstance(row_loop, ast.For):
            raise AssertionError("expected planned row loop")
        self.assertFalse(any(isinstance(node, (ast.Break, ast.Continue, ast.Return))
                             for node in ast.walk(row_loop)))
        self.assertTrue(any(isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
                            and isinstance(node.value.func, ast.Attribute)
                            and isinstance(node.value.func.value, ast.Name)
                            and node.value.func.value.id == "planned_inputs"
                            and node.value.func.attr == "append" for node in row_loop.body))

    def failed_batch_fixture(
        self,
    ) -> tuple[Path, Callable[[], typed_producer.FilePin], typed_producer.FilePin]:
        batch, bundle_path = self.create_batch(
            condition_ids=("case-a",), complete_ids=(), policy_accepted=False,
            seal_receipt=False,
        )
        bundle = gate.Bundle(bundle_path)
        attempts = cast(list[dict[str, typed_producer.FilePin]],
                        gate._objects(bundle.manifest.get("attempts"), "fixture attempts"))
        pins: dict[str, typed_producer.FilePin] = {name: {
            "path": str((batch / name).resolve()), "sha256": gate.sha256(batch / name),
        } for name in (
                           "source-before.json", "source-end.json",
                           "context-before.json", "context-end.json",
                       )}

        def seal() -> typed_producer.FilePin:
            return producer.write_batch_receipt(
                batch, head="a" * 40, source_before=pins["source-before.json"],
                source_end=pins["source-end.json"],
                context_before=pins["context-before.json"],
                context_end=pins["context-end.json"], build_artifacts={}, build_pins=[],
                condition_ids=("case-a",), complete_condition_ids=(),
                policy_accepted=False, attempts=attempts, bundle=bundle_path,
            )
        return batch, seal, pins["context-end.json"]

    def test_failed_batch_seals_attempt_prefix_and_context_before_raising(self) -> None:
        batch, seal, context_end = self.failed_batch_fixture()
        failure = RuntimeError("worker failed after one completed attempt")
        summary: gate.JsonObject = {"conditions": 1, "completeConditions": 0,
                                    "formalAccepted": False}
        with patch("builtins.print") as report:
            with self.assertRaises(RuntimeError) as raised:
                producer._seal_failed_batch(failure, seal, summary)
        self.assertIs(raised.exception, failure)
        report.assert_called_once_with(json.dumps(summary))
        receipt = gate.read_json(batch / "batch-receipt.json")
        self.assertEqual(receipt["status"], "closed")
        self.assertIs(receipt["formalAccepted"], False)
        self.assertEqual(receipt["completeConditionIds"], [])
        contexts = gate._object(receipt.get("contexts"), "receipt contexts")
        self.assertEqual(gate._object(contexts.get("end"), "receipt context end"), context_end)
        with patch.object(gate, "validate_attempt"):
            loaded, _, completed = producer.load_closed_batch(batch, self.root)
        self.assertEqual(loaded["conditionIds"], ["case-a"])
        self.assertEqual(completed, [])

    def test_full_catalog_verifies_once_before_sealing(self) -> None:
        events: list[str] = []

        def seal() -> typed_producer.FilePin:
            events.append("seal")
            return producer.write_exclusive(
                self.root / "formal-seal-fixture-marker.json", {"fixtureOnly": True},
            )

        with patch.object(gate, "verify", side_effect=lambda *_args, **_kwargs: events.append("verify")) as verify, \
             patch.object(gate, "validate_attempt") as validate:
            producer._verify_and_seal_catalog_batch(
                self.root / "bundle", self.root, (), ("case-a",),
                formal_ready=True, diagnostic=False, batch_failure=None,
                seal=seal, summary={}, git_query=None,
            )

        self.assertEqual(events, ["verify", "seal"])
        verify.assert_called_once_with(self.root / "bundle", self.root, git_query=None)
        validate.assert_not_called()

    def test_formal_verifier_failure_does_not_seal(self) -> None:
        failure = ValueError("formal verification failed")
        seal = Mock()
        with patch.object(gate, "verify", side_effect=failure) as verify, \
             patch.object(gate, "validate_attempt") as validate:
            with self.assertRaises(ValueError) as raised:
                producer._verify_and_seal_catalog_batch(
                    self.root / "bundle", self.root, (), ("case-a",),
                    formal_ready=True, diagnostic=False, batch_failure=None,
                    seal=seal, summary={}, git_query=None,
                )
        self.assertIs(raised.exception, failure)
        verify.assert_called_once()
        validate.assert_not_called()
        seal.assert_not_called()

    def test_catalog_diagnostic_verifies_unaccepted_result_before_sealing(self) -> None:
        events: list[str] = []
        unaccepted: gate.JsonObject = {"htmlStage90": {"acceptance": False}}

        def verify(*_args: object, **_kwargs: object) -> gate.JsonObject:
            events.append("diagnostic")
            return unaccepted

        def seal() -> typed_producer.FilePin:
            events.append("seal")
            html_stage90 = gate._object(unaccepted["htmlStage90"], "HTML84 diagnostic result")
            self.assertIs(html_stage90["acceptance"], False)
            return producer.write_exclusive(
                self.root / "diagnostic-seal-fixture-marker.json", {"fixtureOnly": True},
            )

        with patch.object(gate, "_verify_catalog_diagnostic", side_effect=verify) as diagnostic_verify, \
             patch.object(gate, "verify") as formal_verify, \
             patch.object(gate, "validate_attempt") as validate:
            producer._verify_and_seal_catalog_batch(
                self.root / "bundle", self.root, (), ("case-a",),
                formal_ready=True, diagnostic=True, batch_failure=None,
                seal=seal, summary={"formalAccepted": False}, git_query=None,
            )

        self.assertEqual(events, ["diagnostic", "seal"])
        diagnostic_verify.assert_called_once_with(
            self.root / "bundle", self.root, git_query=None,
        )
        formal_verify.assert_not_called()
        validate.assert_not_called()

    def test_partial_catalog_failure_validates_complete_prefix_then_seals_and_raises(self) -> None:
        batch, bundle_path = self.create_batch(
            condition_ids=("case-a", "case-b"), complete_ids=(),
            policy_accepted=False, seal_receipt=False,
        )
        bundle = gate.Bundle(bundle_path)
        attempts = cast(list[dict[str, typed_producer.FilePin]],
                        gate._objects(bundle.manifest.get("attempts"), "fixture attempts"))
        pins: dict[str, typed_producer.FilePin] = {
            name: {"path": str((batch / name).resolve()), "sha256": gate.sha256(batch / name)}
            for name in ("source-before.json", "source-end.json",
                         "context-before.json", "context-end.json")
        }
        events: list[str] = []

        def seal() -> typed_producer.FilePin:
            events.append("seal")
            return producer.write_batch_receipt(
                batch, head="a" * 40, source_before=pins["source-before.json"],
                source_end=pins["source-end.json"], context_before=pins["context-before.json"],
                context_end=pins["context-end.json"], build_artifacts={}, build_pins=[],
                condition_ids=("case-a", "case-b"), complete_condition_ids=("case-a",),
                policy_accepted=False, attempts=attempts, bundle=bundle_path,
            )

        failure = RuntimeError("worker failed after a completed prefix")

        def validate_builds(*_args: object, **_kwargs: object) -> Mapping[str, gate.JsonObject]:
            events.append("build")
            return {}

        def validate_attempt(*_args: object, **_kwargs: object) -> None:
            events.append("attempt")

        with patch.object(gate, "validate_expected_builds",
                          side_effect=validate_builds) as builds, \
             patch.object(gate, "validate_attempt",
                          side_effect=validate_attempt) as validate, \
             patch.object(gate, "committed_records", return_value={}) as records, \
             patch.object(load_validation, "verify_catalog_host_load",
                          side_effect=lambda *_args: events.append("host-load")) as host_load_check, \
             patch("builtins.print"):
            with self.assertRaises(RuntimeError) as raised:
                producer._verify_and_seal_catalog_batch(
                    bundle_path, self.root, attempts, ("case-a",),
                    formal_ready=False, diagnostic=False, batch_failure=failure,
                    seal=seal, summary={"formalAccepted": False}, git_query=None,
                )

        self.assertIs(raised.exception, failure)
        self.assertEqual(events, ["build", "attempt", "host-load", "seal"])
        builds.assert_called_once()
        validate.assert_called_once()
        records.assert_called_once_with(self.root, "a" * 40, git_query=None)
        self.assertEqual(host_load_check.call_args.args[1], [attempts[0]])
        receipt = gate.read_json(batch / "batch-receipt.json")
        self.assertEqual(receipt["conditionIds"], ["case-a", "case-b"])
        self.assertEqual(receipt["completeConditionIds"], ["case-a"])
        self.assertIs(receipt["formalAccepted"], False)

    def test_closed_batch_reader_reuses_only_complete_attempts(self) -> None:
        batch, _ = self.create_batch(complete_ids=("case-a",))
        build_facts = {"baseline": {"proof": "validated"}, "candidate": {"proof": "validated"}}
        with patch.object(gate, "validate_expected_builds", return_value=build_facts) as builds, \
             patch.object(gate, "validate_attempt") as validate, \
             patch.object(gate, "committed_records", return_value={}) as records, \
             patch.object(load_validation, "verify_catalog_host_load") as host_load_check:
            receipt, _, attempts = producer.load_closed_batch(batch, self.root)
        self.assertEqual(receipt["completeConditionIds"], ["case-a"])
        self.assertEqual(len(attempts), 1)
        builds.assert_called_once()
        validate.assert_called_once()
        self.assertIs(validate.call_args.kwargs["validated_builds"], build_facts)
        records.assert_called_once_with(self.root, "a" * 40, git_query=None)
        self.assertEqual(host_load_check.call_args.args[1], attempts)

    def test_invalid_host_load_rejects_closed_and_partial_batch(self) -> None:
        batch, bundle_path = self.create_batch(complete_ids=("case-a",))
        bundle = gate.Bundle(bundle_path)
        attempts = cast(list[dict[str, typed_producer.FilePin]],
                        gate._objects(bundle.manifest.get("attempts"), "fixture attempts"))
        seal = Mock()
        with patch.object(gate, "validate_expected_builds", return_value={}), \
             patch.object(gate, "validate_attempt"), \
             patch.object(gate, "committed_records", return_value={}), \
             patch.object(load_validation, "verify_catalog_host_load",
                          side_effect=ValueError("invalid host load")):
            with self.assertRaisesRegex(ValueError, "invalid host load"):
                producer.load_closed_batch(batch, self.root)
            with self.assertRaisesRegex(ValueError, "invalid host load"):
                producer._verify_and_seal_catalog_batch(
                    bundle_path, self.root, attempts, ("case-a",), formal_ready=False,
                    diagnostic=False, batch_failure=None, seal=seal, summary={}, git_query=None,
                )
        seal.assert_not_called()

    def test_batch_reader_rejects_source_drift_and_edited_pin(self) -> None:
        batch, _ = self.create_batch()
        (batch / "source-before.json").write_bytes(b'{"changed":true}')
        with patch.object(gate, "validate_attempt"):
            with self.assertRaises((ValueError, KeyError)):
                producer.load_closed_batch(batch, self.root)

    def test_batch_reader_rejects_duplicate_missing_or_reordered_ids(self) -> None:
        for ids in (("case-a", "case-a"), ("case-a",), ("case-b", "case-a")):
            with self.subTest(ids=ids):
                batch, _ = self.create_batch()
                receipt_path = batch / "batch-receipt.json"
                receipt = json.loads(receipt_path.read_text())
                receipt["conditionIds"] = list(ids)
                receipt_path.write_bytes(producer.encoded(receipt))
                with patch.object(gate, "validate_attempt"):
                    with self.assertRaises((ValueError, KeyError)):
                        producer.load_closed_batch(batch, self.root)

    def test_partial_zero_sample_is_rejected_by_existing_attempt_validator(self) -> None:
        batch, _ = self.create_batch(complete_ids=("case-a",))
        with patch.object(gate, "validate_expected_builds", return_value={"baseline": {}, "candidate": {}}), \
             patch.object(gate, "validate_attempt", side_effect=ValueError("zero samples")):
            with self.assertRaisesRegex(ValueError, "zero samples"):
                producer.load_closed_batch(batch, self.root)

    def test_policy_false_never_promotes_batch_or_fishes_missing_conditions(self) -> None:
        batch, _ = self.create_batch(policy_accepted=False)
        receipt = gate.read_json(batch / "batch-receipt.json")
        self.assertIs(receipt["policyAccepted"], False)
        self.assertIs(receipt["formalAccepted"], False)
        rows: list[gate.JsonObject] = [
            {"condition_id": value} for value in ("a", "b", "c")
        ]
        selected = producer._select_batch_rows(rows, {"a"}, 1)
        self.assertEqual([row["condition_id"] for row in selected], ["b"])
        self.assertFalse(producer._batch_formal_acceptance(["a"] * 84, ["a"] * 84, False))

    def test_complete_condition_policy_uses_canonical_measurement_helper(self) -> None:
        expected = cast(typed_producer.ExpectedPerformance, {})
        canonical: gate.JsonObject = {
            "version": "paired-wilcoxon-family-v1", "policyPass": True,
        }
        with patch.object(gate, "evaluate_measurement_policy",
                          return_value=canonical) as evaluate:
            result = producer._condition_measurement_policy(
                [self.root / "baseline-0.json"], expected, [], 300,
            )
        self.assertIs(result, canonical)
        evaluate.assert_called_once_with(
            [self.root / "baseline-0.json"], expected,
        )

    def test_incomplete_condition_policy_does_not_fabricate_canonical_statistics(self) -> None:
        expected = cast(typed_producer.ExpectedPerformance, {})
        failure: gate.JsonObject = {"status": "incomplete"}
        with patch.object(gate, "evaluate_measurement_policy") as evaluate:
            result = producer._condition_measurement_policy(
                [], expected, [failure], 12,
            )
        self.assertIs(result["policyPass"], False)
        self.assertEqual(result["phaseRecords"], 12)
        self.assertEqual(result["nullDifference"], 0)
        self.assertNotIn("deltaThreshold", result)
        self.assertNotIn("statistics", result)
        evaluate.assert_not_called()

    def test_known_fragment_failure_requires_exact_skipped_baseline_sequence(self) -> None:
        condition_id, fragment = next(iter(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES.items()))
        first: gate.JsonObject = {
            "status": "nonComparable",
            "reason": "known baseline fragment navigation refusal",
            "conditionId": condition_id, "fragment": fragment,
            "sample": 0, "attempt": 1, "actualExit": 1,
            "errorType": "CalledProcessError",
        }
        failures: list[gate.JsonObject] = [first]
        failures.extend({
            "status": "missing",
            "reason": "baseline skipped after known fragment refusal",
            "afterFailure": first,
        } for _ in range(4))
        self.assertTrue(producer._known_fragment_failure_sequence(
            failures, condition_id, fragment,
        ))
        self.assertFalse(producer._known_fragment_failure_sequence(
            failures[:-1], condition_id, fragment,
        ))
        self.assertFalse(producer._known_fragment_failure_sequence(
            failures, condition_id, "#other",
        ))

    def test_budgeted_batch_can_resume_full_acceptance_but_policy_failure_cannot_clear(self) -> None:
        accepted: gate.JsonObject = {"comparisonStatus": "paired",
                                     "policy": {"policyPass": True}}
        failed: gate.JsonObject = {"comparisonStatus": "paired",
                                   "policy": {"policyPass": False}}
        self.assertTrue(producer._batch_policy_accepted(
            producer.BatchBudgetExhausted("bounded"), [accepted], True))
        self.assertFalse(producer._batch_formal_acceptance(["case-a"], ["case-a"], True))
        self.assertTrue(producer._batch_policy_accepted(None, [accepted] * 84, True))
        self.assertTrue(producer._batch_policy_accepted(
            None, [accepted] * 72 + self._non_comparable_results(), True))
        self.assertFalse(producer._batch_formal_acceptance(["case-a"] * 84,
                                                           ["case-a"] * 84, False))
        self.assertFalse(producer._batch_policy_accepted(None, [failed], True))
        self.assertFalse(producer._batch_policy_accepted(
            producer.BatchBudgetExhausted("bounded"), [accepted], False))

    @staticmethod
    def _non_comparable_results() -> list[gate.JsonObject]:
        return [
            {"comparisonStatus": "nonComparable", "policy": {
                "status": "nonComparable", "policyPass": False,
                "reason": "known baseline fragment navigation refusal",
                "conditionId": condition_id, "fragment": fragment,
                "candidateOperationalPass": True, "candidateSamples": 25,
                "phaseRecords": len(gate.PHASES) * 25,
            }}
            for condition_id, fragment in gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES.items()
        ]

    def test_exact_known_noncomparable_set_keeps_candidate_operations_accepted(self) -> None:
        paired: list[gate.JsonObject] = [
            {"comparisonStatus": "paired", "policy": {"policyPass": True}}
            for _ in range(72)
        ]
        non_comparable = self._non_comparable_results()
        self.assertTrue(producer._batch_policy_accepted(None, paired + non_comparable, True))
        self.assertTrue(producer._batch_policy_accepted(None, paired + non_comparable[:-1], True))
        complete_ids = [f"condition-{index}" for index in range(84)]
        self.assertFalse(producer._batch_formal_acceptance(
            complete_ids[:-1], complete_ids, True))

    def test_old_run_without_new_receipt_is_not_a_resumable_batch(self) -> None:
        with patch.object(gate, "validate_attempt"):
            with self.assertRaises(FileNotFoundError):
                producer.load_closed_batch(self.root / "r3242", self.root)

    def test_formal_verification_is_reserved_for_the_complete_84_union(self) -> None:
        ids = [f"condition-{index}" for index in range(84)]
        self.assertTrue(producer._batch_formal_acceptance(ids, ids, True))
        self.assertFalse(producer._batch_formal_acceptance(ids[:-1], ids[:-1], True))

    def test_expired_batch_deadline_stops_before_starting_a_worker(self) -> None:
        binary = self.root / "worker"
        binary.write_bytes(b"worker")
        build = producer.ArmBuildResult([], binary, gate.sha256(binary), {}, [])
        with patch.object(time, "monotonic", return_value=2.0), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command") as command:
            with self.assertRaises(producer.BatchBudgetExhausted):
                producer.observe_worker_attempt(
                    build, 0, 1, self.root / "fixture.html", self.root / "report.json",
                    self.root / "plan.json", 800, 600, 1, self.root, {}, self.root,
                    batch_deadline=1.0,
                )
        command.assert_not_called()


class RuntimeSyncPreflightTest(unittest.TestCase):
    def setUp(self) -> None:
        guard = patch.object(load_validation, "require_current_cpu_observer_sources")
        guard.start()
        self.addCleanup(guard.stop)
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "repo"
        self.root.mkdir()
        self.output = self.root / "tmp" / "evidence"

    def test_public_producer_rejects_unaccepted_html84_before_build(self) -> None:
        manifest = self.root / "catalog-stage90-captures.json"
        pins: tuple[producer.FilePin, producer.FilePin] = (
            {"path": str(manifest), "sha256": "a" * 64},
            {"path": str(self.root / "plan.json"), "sha256": "b" * 64},
        )
        binary_refs = gate.CatalogBinaryRefs(pins[0], pins[1], pins[0], pins[1])
        validation = {"acceptance": False, "fontIdentityEquivalence": "not-established"}
        validator = ["bun", "run", "scripts/html/fidelity-acceptance-cli.ts",
                     "catalog-stage90-validate-captures", pins[0]["path"], pins[1]["path"]]
        completed = subprocess.CompletedProcess(validator, 0, stdout=b"{}", stderr=b"")
        for with_wrapper in (False, True):
            with self.subTest(with_wrapper=with_wrapper), \
                 patch.object(platform, "system", return_value="Darwin"), \
                 patch.object(platform, "machine", return_value="arm64"), \
                 patch.object(producer, "load_catalog_conditions",
                              return_value=([], *pins, binary_refs)), \
                 patch.object(producer, "source", return_value={"head": "a" * 40}), \
                 patch.object(producer, "_measurement_command", return_value=completed) as run, \
                 patch.object(producer, "_validate_catalog_capture_envelope", return_value=validation), \
                 patch.object(producer, "load_reference_parent") as load_reference, \
                 patch.object(reference, "active_reference_at_commit") as select_reference, \
                 patch.object(producer, "command") as build:
                with self.assertRaisesRegex(ValueError, "HTML84 visual acceptance is not established"):
                    producer.produce(
                        self.root, self.output, catalog_manifest_path=manifest,
                        reference_parent_wrapper=self.root / "wrapper.json" if with_wrapper else None,
                        reference_parent_wrapper_sha256="c" * 64 if with_wrapper else None,
                    )
                run.assert_called_once_with(
                    validator, cwd=self.root, role="preflight",
                    log_directory=self.output.parent / f"{self.output.name}.lifecycle",
                )
                load_reference.assert_not_called()
                select_reference.assert_not_called()
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
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "machine", return_value="arm64"), \
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
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "machine", return_value="arm64"), \
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
                    patch.object(platform, "system", return_value="Darwin"), \
                    patch.object(platform, "machine", return_value="arm64"), \
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
                "rustc", "cargo", "context-pin", root / "Cargo.lock",
                {"path": "snapshot.json", "sha256": "s" * 64})
            self.assertEqual(worker, {**worker_environment_before, "KRR_PERF_RUSTC": "rustc",
                "KRR_PERF_CARGO": "cargo", "KRR_PERF_ENVIRONMENT_SHA256": "context-pin",
                "KRR_PERF_LOCK_PATH": str(root / "Cargo.lock"),
                "KRR_PERF_SOURCE_SNAPSHOT_PATH": "snapshot.json"})


class WorkerAttemptContractTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.binary = self.root / "worker"
        self.binary.write_bytes(b"frozen worker")
        self.build = typed_producer.ArmBuildResult([], self.binary, gate.sha256(self.binary), {}, [])
        self.fixture = self.root / "input.html"
        self.fixture.write_text("<p>fixture</p>")
        self.plan = self.root / "plan.json"
        self.plan.write_text("{}")
        self.logs = self.root / "raw"
        self.attempts = self.root / "attempts"
        self.viewport: typed_producer.Viewport = {"width": 800, "height": 600, "scale": 1}

    def command_result(
        self, *, returncode: int = 0, mutate_binary: bool = False,
        elapsed_ms: float = 1.0, closed: bool = False,
        stderr: bytes = b"worker stderr",
    ) -> Callable[..., subprocess.CompletedProcess[bytes]]:
        def execute(
            args: Sequence[str], *, role: str,
            log_directory: Path, cwd: Path | None = None,
            env: Mapping[str, str] | None = None, check: bool = True,
            lifecycle_sink: Callable[[Path], None] | None = None,
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(role, "worker")
            self.assertIs(check, False)
            if mutate_binary:
                self.binary.write_bytes(b"changed worker")
            log_directory.mkdir(parents=True, exist_ok=True)
            lifecycle = log_directory / f"worker-{len(list(log_directory.glob('worker-*.json')))}.json"
            payload = json.dumps({"sample": int(args[2]), "viewport": self.viewport,
                                  "elapsed_ms": elapsed_ms}).encode()
            stdout_path, stderr_path = log_directory / "stdout.raw", log_directory / "stderr.raw"
            stdout_path.write_bytes(payload)
            stderr_path.write_bytes(stderr)
            lifecycle_value = {"role": "worker", "argv": list(args), "exitCode": returncode,
                "raw": [{"path": str(stdout_path), "sha256": gate.sha256(stdout_path)},
                        {"path": str(stderr_path), "sha256": gate.sha256(stderr_path)}]}
            if closed:
                lifecycle_value.update({"identity": {"pid": 4242}, "groupClosed": True,
                    "rawFdsClosed": True, "failure": None, "cleanupFailure": None})
            lifecycle.write_text(json.dumps(lifecycle_value))
            if lifecycle_sink is not None:
                lifecycle_sink(lifecycle)
            return subprocess.CompletedProcess(args, returncode, payload, stderr)
        return execute

    def test_retries_only_host_load_and_records_both_attempts(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", side_effect=[(0.31, 1.0, cpu_times(31.0, 69.0)), (0.1, 1.0, cpu_times(10.0, 90.0))]), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result()) as run:
            sample, loads, attempts = producer.collect_worker_sample(
                self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
            )
        self.assertEqual(sample["elapsed_ms"], 1.0)
        self.assertEqual(loads, {"before": 0.1, "after": 0.1})
        self.assertEqual(len(attempts), 2)
        self.assertEqual(attempts[0]["reason"], "host load exceeded limit")
        self.assertIs(attempts[0]["accepted"], False)
        self.assertIs(attempts[1]["accepted"], True)
        self.assertIs(attempts[1]["workerStarted"], True)
        self.assertEqual(attempts[0]["loadRatioBefore"], 0.1)
        self.assertEqual(attempts[0]["loadRatioAfter"], 0.31)
        self.assertEqual(attempts[0]["actualExit"], 0)
        lifecycle = gate._object(attempts[0]["lifecycle"], "worker lifecycle pin")
        lifecycle_path = Path(gate._text(lifecycle["path"], "worker lifecycle path"))
        lifecycle_sha = gate._text(lifecycle["sha256"], "worker lifecycle SHA")
        self.assertTrue(lifecycle_path.is_file())
        self.assertEqual(gate.sha256(lifecycle_path), lifecycle_sha)
        self.assertEqual(attempts[0]["binarySha256Before"], attempts[0]["binarySha256After"])
        self.assertEqual(len(run.call_args_list), 2)
        self.assertEqual(len(list(self.attempts.glob("*.json"))), 2)

    def test_worker_is_between_blocking_prime_and_cpu_interval_sample(self) -> None:
        steps: list[float | None | str] = []

        def cpu(*, interval: float | None, percpu: bool) -> CpuTimes:
            self.assertFalse(percpu)
            steps.append(interval)
            return CpuTimes(10.0, 90.0)

        def worker(*args: object, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            steps.append("worker")
            return self.command_result()(*args, **kwargs)

        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", side_effect=cpu),
              patch.object(time, "monotonic", side_effect=[1.0, 1.0, 1.0, 2.0, 3.0, 3.0]),
              patch.object(producer, "_measurement_command", side_effect=worker)):
            _, _, attempts = producer.collect_worker_sample(
                self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
            )
        self.assertEqual(steps, [host_load.INTERVAL_SECONDS, "worker", None])
        tag = gate._object(attempts[0]["hostLoad"], "host load")
        self.assertAlmostEqual(cast(float, tag["intervalBeforeSeconds"]), host_load.INTERVAL_SECONDS)
        self.assertAlmostEqual(cast(float, tag["intervalDuringSeconds"]), 1.0)
        self.assertEqual(tag["method"], "psutil.cpu_times_percent")

    def test_worker_cpu_sampling_failure_is_saved_and_rejected(self) -> None:
        with (patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))),
              patch.object(host_load, "finish", side_effect=OSError("unavailable")),
              patch.object(producer, "_measurement_command", side_effect=self.command_result())):
            with self.assertRaises(OSError):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {}, self.logs, self.attempts,
                )
        attempt = json.loads(next(self.attempts.glob("attempt-0-1.json")).read_text())
        self.assertFalse(attempt["accepted"])
        self.assertIsNone(attempt["loadRatioAfter"])
        self.assertIsNone(attempt["hostLoad"]["intervalDuringSeconds"])
        self.assertEqual(attempt["hostLoadErrorType"], "OSError")
        self.assertEqual(attempt["reason"], "host load observation failed")

    def test_unobserved_cpu_retries_the_worker_not_only_the_sampler(self) -> None:
        probe = host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))
        with (patch.object(producer, "await_quiet", return_value=probe) as prime,
              patch.object(host_load, "finish", side_effect=[host_load.NoObservation(
                  "zero", cpu_times(0.0, 0.0), 1.0),
                  (0.1, 1.0, cpu_times(10.0, 90.0))]),
              patch.object(producer, "_measurement_command", side_effect=self.command_result()) as worker):
            _, _, attempts = producer.collect_worker_sample(
                self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                self.root, {}, self.logs, self.attempts,
            )
        self.assertEqual(prime.call_count, 2)
        self.assertEqual(worker.call_count, 2)
        self.assertEqual(attempts[0]["reason"], "host load observation unavailable")
        self.assertIsNone(attempts[0]["loadRatioAfter"])
        first_tag = gate._object(attempts[0]["hostLoad"], "host load")
        self.assertEqual(first_tag["cpuTimesDuring"], cpu_times(0.0, 0.0))
        self.assertEqual(first_tag["intervalDuringSeconds"], host_load.INTERVAL_SECONDS)
        self.assertTrue(attempts[1]["accepted"])

    def test_persistent_host_load_stops_after_three_recorded_attempts(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.3, 1.0, cpu_times(30.0, 70.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result()) as run:
            with self.assertRaisesRegex(ValueError, "host load remained above limit"):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                )
        self.assertEqual(len(run.call_args_list), 3)
        self.assertEqual(len(list(self.attempts.glob("*.json"))), 3)

    def test_worker_binary_change_is_distinct_and_immediate(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(mutate_binary=True)) as run:
            with self.assertRaisesRegex(ValueError, "worker binary changed during measurement"):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                )
        self.assertEqual(len(run.call_args_list), 1)
        attempt = json.loads(next(self.attempts.glob("*.json")).read_text())
        self.assertNotEqual(attempt["binarySha256Before"], attempt["binarySha256After"])

    def test_worker_failure_preserves_exit_and_is_not_retried(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(returncode=7)) as run:
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                )
        self.assertEqual(caught.exception.returncode, 7)
        self.assertEqual(caught.exception.output, json.dumps({
            "sample": 0, "viewport": self.viewport, "elapsed_ms": 1.0,
        }).encode())
        self.assertEqual(caught.exception.stderr, b"worker stderr")
        self.assertEqual(len(run.call_args_list), 1)
        attempt = json.loads(next(self.attempts.glob("*.json")).read_text())
        self.assertEqual(attempt["actualExit"], 7)
        self.assertIs(attempt["accepted"], False)

    def _collect_failed_worker_sample(
        self, *, returncode: int, closed: bool,
        stderr: bytes = b"worker stderr",
    ) -> tuple[list[gate.JsonObject], subprocess.CalledProcessError]:
        failures: list[gate.JsonObject] = []
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(returncode=returncode, closed=closed,
                                                          stderr=stderr)):
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                    failures.extend,
                )
        return failures, caught.exception

    def _known_navigation_expected(self) -> typed_producer.ExpectedPerformance:
        baseline = "reference-parent-371274ed"
        return cast(typed_producer.ExpectedPerformance, {
            "referenceParent": {"role": baseline, "referenceKind": "correct-parent-source",
                                "publishedSdk": False, "armId": baseline},
            "catalogCondition": {"condition_id": "navigation/top/1280x900@1",
                                 "operation_kind": "navigation-pointer-click"},
            "arms": {baseline: {"binary_sha256": self.build.binary_sha256}},
        })

    def _known_navigation_failure(self, *, closed: bool = True) -> tuple[
        list[gate.JsonObject], subprocess.CalledProcessError,
        typed_producer.ExpectedPerformance,
    ]:
        expected = self._known_navigation_expected()
        fragment = "#a"
        stderr = (f'Error: RuntimeFailure {{ error: "navigation is not allowed: {fragment}" }}'
                  ).encode()
        failures, error = self._collect_failed_worker_sample(
            returncode=1, closed=closed, stderr=stderr,
        )
        return failures, error, expected

    def test_closed_baseline_failure_can_continue_without_a_sample(self) -> None:
        failures, error, expected = self._known_navigation_failure()
        self.assertTrue(producer._worker_failure_can_continue("baseline", failures, error,
                                                              expected))
        self.assertEqual(len(failures), 1)
        self.assertIs(failures[0]["accepted"], False)
        self.assertEqual(failures[0]["actualExit"], 1)

    def test_incomplete_baseline_failure_is_not_treated_as_candidate_success(self) -> None:
        failures, error, expected = self._known_navigation_failure()
        self.assertFalse(producer._worker_failure_can_continue("candidate", failures, error,
                                                               expected))

    def test_only_exact_fragment_refusal_is_noncomparable(self) -> None:
        failures, error, expected = self._known_navigation_failure()
        wrong_fragment = copy.copy(error)
        wrong_fragment.stderr = b'navigation is not allowed: #other\n'
        self.assertFalse(producer._worker_failure_can_continue(
            "baseline", failures, wrong_fragment, expected,
        ))
        wrong_condition = copy.deepcopy(expected)
        wrong_condition["catalogCondition"]["condition_id"] = (
            "navigation/top/other@1"
        )
        self.assertFalse(producer._worker_failure_can_continue(
            "baseline", failures, error, wrong_condition,
        ))

    def test_closed_baseline_failure_keeps_attempt_pin_for_transport(self) -> None:
        failures: list[gate.JsonObject] = []
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(returncode=7, closed=True)):
            with self.assertRaises(subprocess.CalledProcessError):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                    failures.extend,
                )
        pins: list[typed_producer.FilePin] = []
        producer._append_worker_attempt_pins(pins, failures)
        self.assertEqual(len(pins), 1)
        self.assertTrue(Path(pins[0]["path"]).is_file())

    def test_baseline_failure_with_unknown_closure_fails_closed(self) -> None:
        failures, error, expected = self._known_navigation_failure(closed=False)
        self.assertFalse(producer._worker_failure_can_continue("baseline", failures, error,
                                                               expected))

    def test_signal_terminated_baseline_failure_fails_closed(self) -> None:
        failures, error = self._collect_failed_worker_sample(returncode=-15, closed=True)
        self.assertFalse(producer._worker_failure_can_continue("baseline", failures, error,
                                                               self._known_navigation_expected()))

    def _condition_case(
        self,
    ) -> tuple[typed_producer.ArmBuildResult, Path, typed_producer.FilePin,
               dict[str, typed_producer.ArmPin], typed_producer.ExpectedPerformance,
               gate.JsonObject]:
        candidate_binary = self.root / "candidate-worker"
        candidate_binary.write_bytes(b"candidate worker")
        candidate_build = typed_producer.ArmBuildResult(
            [], candidate_binary, gate.sha256(candidate_binary), {}, [])
        snapshot: typed_producer.FilePin = {
            "path": str(self.plan), "sha256": gate.sha256(self.plan)}
        artifact: typed_producer.FilePin = {
            "path": str(self.plan), "sha256": gate.sha256(self.plan)}
        arms: dict[str, typed_producer.ArmPin] = {
            "baseline-0.4.23": {
                "binary_sha256": self.build.binary_sha256,
                "source_snapshot_sha256": snapshot["sha256"],
                "lock_sha256": gate.LOCK_SHA, "build_artifact_ref": artifact,
            },
            "candidate": {
                "binary_sha256": candidate_build.binary_sha256,
                "source_snapshot_sha256": snapshot["sha256"],
                "lock_sha256": gate.LOCK_SHA, "build_artifact_ref": artifact,
            },
        }
        expected: typed_producer.ExpectedPerformance = {
            "fixture_sha256": gate.sha256(self.fixture),
            "operation_plan_sha256": gate.sha256(self.plan),
            "operation": "test", "expected_dom_state_sha256": gate.sha256(self.plan),
            "viewport": self.viewport,
            "provenance": {"rustc": "rustc", "cargo": "cargo",
                            "environment_sha256": "e" * 64},
            "arms": arms, "reportPins": [], "operationPlanRef": artifact,
            "domStateRef": artifact,
        }
        return (candidate_build, candidate_binary, snapshot, arms, expected,
                {"catalog": False})

    def _worker_report_sample(
        self, expected: typed_producer.ExpectedPerformance,
    ) -> gate.JsonObject:
        phases: gate.JsonObject = {}
        frame = {"viewport": self.viewport, "pixel_bytes": 800 * 600 * 4,
                 "scroll_y": 100.0}
        for phase in gate.PHASES:
            measurement = {"elapsed_ms": 1.0, "peak_rss_bytes": 100, "frame": frame.copy()}
            phases[phase] = ({"measurement": measurement}
                             if phase in ("post_operation", "same_session_resize")
                             else measurement)
        full_y = gate._object(phases["full_y"], "full-Y sample")
        full_y.update({"max_frame_elapsed_ms": 1.0, "scroll_steps": 1,
                       "max_scroll": 100.0, "final_scroll_y": 100.0,
                       "actual_bottom": True})
        post = gate._object(phases["post_operation"], "post-operation sample")
        post.update({"operation": expected["operation"],
                     "expected_dom_state_sha256": expected["expected_dom_state_sha256"]})
        resize = gate._object(phases["same_session_resize"], "resize sample")
        resize["resize_evidence"] = {
            "from_viewport": self.viewport,
            "to_viewport": {"width": 600, "height": 600, "scale": 1},
            "generation_before": 0, "generation_after": 1, "real_raster_frame": True,
        }
        return {"phases": phases}

    def _known_navigation_collection_case(self) -> tuple[
        typed_producer.ArmBuildResult, Path, typed_producer.FilePin,
        dict[str, typed_producer.ArmPin], typed_producer.ExpectedPerformance,
        gate.JsonObject, list[gate.JsonObject], subprocess.CalledProcessError,
    ]:
        candidate, binary, snapshot, arms, expected, row = self._condition_case()
        failures, error, known = self._known_navigation_failure()
        expected["referenceParent"] = known["referenceParent"]
        expected["catalogCondition"] = {
            **gate._object(known["catalogCondition"], "known catalog condition"),
            "case_id": "navigation", "state": "top",
        }
        expected["operationEvidenceRef"] = snapshot
        row.update({"catalog": True, "condition_id": "navigation/top/1280x900@1",
                    "case_id": "navigation", "state": "top",
                    "operation_kind": "navigation-pointer-click",
                    "operation_evidence": snapshot})
        baseline_id = gate.baseline_arm_id(expected)
        arms[baseline_id] = arms.pop("baseline-0.4.23")
        expected["arms"] = arms
        return candidate, binary, snapshot, arms, expected, row, failures, error

    def _collect_condition(
        self, *, folder: Path, candidate: typed_producer.ArmBuildResult,
        candidate_binary: Path, snapshot: typed_producer.FilePin,
        arms: dict[str, typed_producer.ArmPin],
        expected: typed_producer.ExpectedPerformance, row: gate.JsonObject,
        batch_deadline: float | None = None,
    ) -> typed_producer.ConditionWorkerCollection:
        return producer._collect_condition_worker_pairs(
            folder=folder, row=row, expected=expected, fixture=self.fixture,
            plan_path=self.plan, width=800, height=600, viewport=self.viewport,
            builds={"baseline": self.build, "candidate": candidate},
            binaries={"baseline": self.binary, "candidate": candidate_binary},
            arms=arms, root=self.root, lifecycle=self.logs, lock=self.plan,
            snapshot=snapshot, environment_sha="e" * 64, batch_deadline=batch_deadline,
        )

    def _assert_collection_raises(
        self, error_type: type[BaseException], *, folder: Path,
        candidate: typed_producer.ArmBuildResult, candidate_binary: Path,
        snapshot: typed_producer.FilePin, arms: dict[str, typed_producer.ArmPin],
        expected: typed_producer.ExpectedPerformance, row: gate.JsonObject,
        batch_deadline: float | None = None,
    ) -> None:
        with self.assertRaises(error_type):
            self._collect_condition(
                folder=folder, candidate=candidate, candidate_binary=candidate_binary,
                snapshot=snapshot, arms=arms, expected=expected, row=row,
                batch_deadline=batch_deadline,
            )

    def test_worker_report_uses_each_arm_source_and_lock_pins(self) -> None:
        candidate_build, _, _, _, expected, row = self._condition_case()
        parent = reference.ReferenceParentProof(
            reference.REFERENCE_ROLE, "candidate", "3" * 40, "a" * 64,
            self.build.binary_sha256, reference.PARENT_LOCK_SHA256,
            "d" * 64, "e" * 64, "f" * 64, (), self.root / "parent-build.json", self.binary,
        )
        artifact: typed_producer.FilePin = {
            "path": str(self.plan), "sha256": gate.sha256(self.plan)}
        arms = producer.expected_arm_pins(
            {"baseline": self.build, "candidate": candidate_build}, "c" * 64,
            {"baseline": artifact, "candidate": artifact}, reference_parent=parent,
            reference_contract=reference.reference_parent_contract(parent),
        )
        expected["arms"] = arms
        samples = [self._worker_report_sample(expected)]
        for arm, pins in arms.items():
            report = producer._build_worker_report(
                expected, row, arms, arm, 0, self.viewport, samples, None,
            )
            provenance = gate._object(report["provenance"], "worker provenance")
            self.assertEqual(provenance["source_snapshot_sha256"], pins["source_snapshot_sha256"])
            self.assertEqual(provenance["deps_lock_sha256"], pins["lock_sha256"])
            gate._validate_report(report, dict(expected))
            for key in ("source_snapshot_sha256", "deps_lock_sha256"):
                mutated = copy.deepcopy(report)
                gate._object(mutated["provenance"], "worker provenance")[key] = "0" * 64
                with self.subTest(arm=arm, key=key), self.assertRaisesRegex(
                    ValueError, f"{arm} provenance mismatch for {key}"
                ):
                    gate._validate_report(mutated, dict(expected))

    def test_condition_collection_rejects_unknown_baseline_failure_before_candidates(self) -> None:
        candidate_build, candidate_binary, snapshot, arms, expected, row = self._condition_case()
        calls = {"baseline": 0, "candidate": 0}

        def collect_sample(
            build: typed_producer.ArmBuildResult, *_args: object,
            **_kwargs: object,
        ) -> tuple[gate.JsonObject, dict[str, float], list[gate.JsonObject]]:
            if build is self.build:
                calls["baseline"] += 1
            else:
                calls["candidate"] += 1
            raise subprocess.CalledProcessError(7, ["worker"])

        with patch.object(producer, "collect_worker_sample", side_effect=collect_sample):
            self._assert_collection_raises(
                subprocess.CalledProcessError, folder=self.root / "condition",
                candidate=candidate_build, candidate_binary=candidate_binary,
                snapshot=snapshot, arms=arms, expected=expected, row=row,
            )
        self.assertEqual(calls, {"baseline": 1, "candidate": 0})

    def test_known_baseline_refusal_collects_and_requires_all_candidate_samples(self) -> None:
        (candidate_build, candidate_binary, snapshot, arms, expected, row,
         failed_attempts, failure) = self._known_navigation_collection_case()
        calls = {"baseline": 0, "candidate": 0}

        def collect_sample(
            build: typed_producer.ArmBuildResult, sample: int, _fixture: Path,
            _report_path: Path, _plan_path: Path, _width: int, _height: int,
            _scale: int, _root: Path, _environment: Mapping[str, str],
            _lifecycle: Path, attempts_directory: Path,
            failure_sink: Callable[[list[gate.JsonObject]], None] | None = None,
            *, batch_deadline: float | None = None,
        ) -> tuple[gate.JsonObject, dict[str, float], list[gate.JsonObject]]:
            if build is self.build:
                calls["baseline"] += 1
                if failure_sink is None:
                    raise AssertionError("baseline failure sink missing")
                failure_sink(failed_attempts)
                raise failure
            calls["candidate"] += 1
            attempts_directory.mkdir(parents=True, exist_ok=True)
            pin = producer.write(attempts_directory / f"attempt-{sample}.json", {
                "sample": sample, "actualExit": 0,
            })
            payload = self._worker_report_sample(expected)
            payload["catalog_condition"] = {
                "profile": "catalog-plan84", "condition_id": row["condition_id"],
                "case_id": row["case_id"], "state": row["state"],
                "operation_kind": row["operation_kind"],
                "operation_evidence_sha256": snapshot["sha256"],
            }
            return payload, {"before": 0.1, "after": 0.1}, [
                {"sample": sample, "attemptRecord": pin},
            ]

        with patch.object(producer, "collect_worker_sample", side_effect=collect_sample):
            collection = self._collect_condition(
                folder=self.root / "known-condition", candidate=candidate_build,
                candidate_binary=candidate_binary, snapshot=snapshot, arms=arms,
                expected=expected, row=row,
            )
        self.assertEqual(calls, {"baseline": 1, "candidate": 25})
        self.assertEqual([failure["status"] for failure in collection.failures],
                         ["nonComparable", "missing", "missing", "missing", "missing"])
        self.assertEqual(collection.actual_phase_records, len(gate.PHASES) * 25)
        self.assertEqual(len(collection.reports), 10)
        policy = producer._condition_measurement_policy(
            collection.reports, expected, collection.failures, collection.actual_phase_records)
        self.assertIs(policy["candidateOperationalPass"], True)
        self.assertEqual(policy["candidateSamples"], 25)
        candidate_missing = [p for p in collection.reports if p.name != "candidate-0.json"]
        with self.assertRaisesRegex(ValueError, "five candidate reports"):
            producer._condition_measurement_policy(
                candidate_missing, expected, collection.failures, collection.actual_phase_records)

    def test_condition_collection_budget_stops_before_next_worker(self) -> None:
        candidate_build, candidate_binary, snapshot, arms, expected, row = self._condition_case()
        calls = 0

        def collect_sample(*_args: object, **_kwargs: object) -> tuple[
            gate.JsonObject, dict[str, float], list[gate.JsonObject]
        ]:
            nonlocal calls
            calls += 1
            raise producer.BatchBudgetExhausted("expired")

        with patch.object(producer, "_require_batch_budget", return_value=None), \
             patch.object(producer, "collect_worker_sample", side_effect=collect_sample):
            self._assert_collection_raises(
                producer.BatchBudgetExhausted, folder=self.root / "condition",
                candidate=candidate_build, candidate_binary=candidate_binary,
                snapshot=snapshot, arms=arms, expected=expected, row=row,
                batch_deadline=1.0,
            )
        self.assertEqual(calls, 1)

    def test_partial_baseline_failure_keeps_successful_samples_and_attempts(self) -> None:
        failures: list[gate.JsonObject] = []
        samples: list[gate.JsonObject] = []
        loads: list[dict[str, float]] = []
        attempts: list[gate.JsonObject] = []
        commands = iter([self.command_result(), self.command_result(),
                         self.command_result(returncode=7, closed=True)])

        def run_command(*args: object, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            return next(commands)(*args, **kwargs)

        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command", side_effect=run_command):
            for sample in range(3):
                try:
                    payload, sample_load, sample_attempts = producer.collect_worker_sample(
                        self.build, sample, self.fixture, self.root / "reports.json", self.plan,
                        800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                        self.logs, self.attempts, failures.extend,
                    )
                except subprocess.CalledProcessError as error:
                    self.assertFalse(producer._worker_failure_can_continue(
                        "baseline", failures, error, self._known_navigation_expected()))
                    attempts.extend(failures)
                    break
                samples.append(payload)
                loads.append(sample_load)
                attempts.extend(sample_attempts)
        self.assertEqual([sample["sample"] for sample in samples], [0, 1])
        self.assertEqual(len(loads), 2)
        self.assertEqual(len(attempts), 3)
        self.assertEqual(attempts[-1]["actualExit"], 7)

    def test_elapsed_time_does_not_trigger_host_load_retry(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(elapsed_ms=90_000.0)) as run:
            sample, _loads, attempts = producer.collect_worker_sample(
                self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
            )
        self.assertEqual(sample["elapsed_ms"], 90_000.0)
        self.assertEqual(len(run.call_args_list), 1)
        self.assertIs(attempts[0]["accepted"], True)

    def test_worker_attempt_records_are_unique_across_arms_and_pairs(self) -> None:
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result()) as run:
            for arm in ("baseline", "candidate"):
                for pair in range(2):
                    attempt_directory = producer.worker_attempts_directory(
                        self.attempts, arm, pair,
                    )
                    producer.collect_worker_sample(
                        self.build, 0, self.fixture, self.root / "reports.json", self.plan,
                        800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                        self.logs, attempt_directory,
                    )
        saved = list(self.attempts.rglob("attempt-0-1.json"))
        self.assertEqual(len(saved), 4)
        self.assertEqual(len({path.parent for path in saved}), 4)
        self.assertEqual(len(run.call_args_list), 4)

    def test_pre_worker_binary_mismatch_is_recorded_without_launch(self) -> None:
        self.binary.write_bytes(b"changed before worker")
        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command") as run:
            with self.assertRaisesRegex(ValueError, "binary changed before measurement"):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan,
                    800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                    self.logs, self.attempts,
                )
        run.assert_not_called()
        attempt = json.loads(next(self.attempts.glob("attempt-0-1.json")).read_text())
        actual_sha = gate.sha256(self.binary)
        self.assertEqual(attempt["binarySha256Before"], actual_sha)
        self.assertIsNone(attempt["binarySha256After"])
        self.assertEqual(attempt["reason"], "worker binary changed before measurement")
        self.assertIs(attempt["workerStarted"], False)
        self.assertIsNone(attempt["actualExit"])
        self.assertIsNone(attempt["hostLoad"]["intervalDuringSeconds"])

    def test_worker_start_is_unknown_when_launch_has_no_pid_record(self) -> None:
        def fail_before_spawn(*_args: object, **_kwargs: object) -> None:
            raise FileNotFoundError("worker executable unavailable")

        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command", side_effect=fail_before_spawn):
            with self.assertRaisesRegex(FileNotFoundError, "worker executable unavailable"):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan,
                    800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                    self.logs, self.attempts,
                )
        attempt = json.loads(next(self.attempts.glob("attempt-0-1.json")).read_text())
        self.assertIsNone(attempt["workerStarted"])
        self.assertIsNone(attempt["actualExit"])
        self.assertIsNone(attempt["lifecycle"])
        self.assertIsNone(attempt["loadRatioAfter"])
        self.assertIsNone(attempt["hostLoad"]["intervalDuringSeconds"])

    def test_worker_start_is_true_when_timeout_lifecycle_records_pid(self) -> None:
        def timeout_after_spawn(
            args: Sequence[str], *, log_directory: Path,
            lifecycle_sink: Callable[[Path], None] | None = None, **_kwargs: object,
        ) -> None:
            lifecycle = log_directory / "worker-timeout.json"
            lifecycle.parent.mkdir(parents=True, exist_ok=True)
            lifecycle.write_text(json.dumps({"identity": {"pid": 4242}, "exitCode": -15}))
            if lifecycle_sink is not None:
                lifecycle_sink(lifecycle)
            raise subprocess.TimeoutExpired(args, 60)

        with patch.object(producer, "await_quiet", return_value=host_load.Probe(0.1, 1.0, 1.0, 0, cpu_times(10.0, 90.0))), \
             patch.object(host_load, "finish", return_value=(0.1, 1.0, cpu_times(10.0, 90.0))), \
             patch.object(producer, "_measurement_command", side_effect=timeout_after_spawn):
            with self.assertRaises(subprocess.TimeoutExpired):
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan,
                    800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                    self.logs, self.attempts,
                )
        attempt = json.loads(next(self.attempts.glob("attempt-0-1.json")).read_text())
        self.assertIs(attempt["workerStarted"], True)
        self.assertIsNone(attempt["actualExit"])
        self.assertEqual(attempt["loadRatioAfter"], 0.1)

    def test_quiet_wait_uses_the_worker_load_limit(self) -> None:
        with patch.object(host_load, "start", side_effect=[host_load.Probe(0.3, 1.0, 1.0, 0, cpu_times(30.0, 70.0)),
                                                            host_load.Probe(0.25, 1.0, 2.0, 0, cpu_times(25.0, 75.0))]), \
             patch.object(time, "sleep") as sleep:
            self.assertEqual(producer.await_quiet().before, producer.MAX_WORKER_LOAD)
        sleep.assert_called_once_with(5)
        self.assertEqual(producer.MAX_WORKER_LOAD, 0.25)

    def test_condition_checkpoint_is_nonfinal_and_exclusive(self) -> None:
        folder = self.root / "condition"
        checkpoint = producer.write_condition_checkpoint(
            folder, {"fixture_sha256": "a" * 64}, {"formalAccepted": False},
            {"measurement_executed": True},
        )
        value = json.loads(Path(checkpoint["path"]).read_text())
        self.assertIs(value["formalAccepted"], False)
        self.assertIs(value["endpointValidated"], False)
        self.assertEqual(value["expected"]["fixture_sha256"], "a" * 64)
        self.assertEqual(value["result"]["formalAccepted"], False)
        self.assertTrue(value["manifest"]["measurement_executed"])
        with self.assertRaises(FileExistsError):
            producer.write_condition_checkpoint(
                folder, {"fixture_sha256": "b" * 64}, {}, {},
            )


class CapacityBindingUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        guard = patch.object(load_validation, "require_current_cpu_observer_sources")
        guard.start()
        self.addCleanup(guard.stop)
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
                    check: bool = True, lifecycle_sink: Callable[[Path], None] | None = None) -> subprocess.CompletedProcess[str] | subprocess.CompletedProcess[bytes]:
            self.commands.append(argv)
            self.environments.append(env)
            if argv[1] == "metadata":
                if mutation is not None:
                    mutation()
                return subprocess.CompletedProcess(argv, 0, stdout=json.dumps(self.metadata).encode(), stderr=b"metadata stderr\n")
            if argv[-1] == "-vV":
                tool = "cargo" if Path(argv[0]) == self.cargo else "rustc"
                return subprocess.CompletedProcess(argv, 0, stdout=f"{tool} 1.90.0\n".encode(), stderr=b"")
            if lifecycle_sink is not None:
                log_directory.mkdir(parents=True, exist_ok=True)
                lifecycle = log_directory / "build-lifecycle.json"
                lifecycle.write_text("{}")
                lifecycle_sink(lifecycle)
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
            ("one bound reference", self.references[:1]),
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

    def test_unapproved_historical_source_is_rejected_before_metadata_and_build(self) -> None:
        selected = reference.ActiveReference("selected", "a" * 40, "b" * 64, "c" * 64, {})
        history = self.root / "tmp/reference"
        binding = self.binding()
        def query(root: Path, args: Sequence[str], input_bytes: bytes | None, text: bool) -> str:
            del root, args, input_bytes, text
            return ""
        with patch.dict(os.environ, self.environment, clear=True), \
             patch.object(shutil, "which", return_value=str(self.cargo)), \
             patch.object(source_materialization, "verify_selected_source",
                          side_effect=ValueError("historical selection rejected")), \
             patch.object(typed_producer, "_measurement_command") as command:
            with self.assertRaisesRegex(ValueError, "historical selection rejected"):
                typed_producer.guarded_arm_build(
                    self.root, history / gate.HARNESS_ROOT, "candidate", binding, self.source_records,
                    self.root / "tmp/builds/baseline", historical_source=(history, selected), git_query=query)
        command.assert_not_called()

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

    def test_artifact_reader_allows_removed_target_but_live_freeze_rejects_it(self) -> None:
        expected = self.expected("candidate")
        events = self.arm_events("candidate")
        executable = self.target / "release/krr-html-performance-harness"
        executable.unlink()
        selected, _ = typed_producer.cargo_build_artifacts(
            b"\n".join(json.dumps(event).encode() for event in events), expected, self.target,
        )
        self.assertEqual(selected, executable)
        self.assertFalse(executable.exists())
        with self.assertRaisesRegex(ValueError, "selected binary must be a canonical regular file"):
            typed_producer.freeze_binary(selected, self.root / "tmp/missing-target/frozen")

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


    def test_candidate_build_only_validates_bundle_without_html_acceptance(self) -> None:
        initial: gate.JsonObject = {"head": "a" * 40, "files": [], "inventoryDigest": "b" * 64}
        facts: gate.JsonObject = {"binarySha256": "c" * 64, "buildArtifactSha256": "d" * 64}
        output = self.root / "tmp/candidate-proof"
        binding = self.binding()

        def build(_root: Path, _harness: Path, arm: str, _binding: object,
                  _records: gate.SourceRecords, build_output: Path) -> typed_producer.ArmBuildResult:
            self.assertEqual(arm, "candidate")
            binary = build_output / "binary" / "krr-html-performance-harness"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"candidate binary")
            raw = build_output / "cargo.jsonl"
            raw.write_bytes(b"build raw")
            return typed_producer.ArmBuildResult([], binary, gate.sha256(binary),
                {"schema": "krr-html-performance-build-artifact-v2", "arm": arm},
                [{"path": str(raw), "sha256": gate.sha256(raw)}])

        def git_query(_root: Path, _args: Sequence[str],
                      _input_bytes: bytes | None, _text: bool) -> str:
            return ""

        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "machine", return_value="arm64"), \
             patch.object(producer, "_make_git_query",
                          return_value=git_query), \
             patch.object(producer, "source", side_effect=[initial, initial, initial, initial]), \
             patch.object(producer, "_measurement_command",
                          return_value=subprocess.CompletedProcess(
                              ["just", "runtime-bundle-check"], 0, b"", b"",
                          )) as command, \
             patch.object(producer, "load_capacity_binding", return_value=binding), \
             patch.object(producer, "guarded_arm_build", side_effect=build) as build_call, \
             patch.object(gate, "current_source", return_value={}), \
             patch.object(gate, "LOCK_SHA", self.lock_sha), \
             patch.object(gate, "validate_candidate_build_proof",
                          return_value=facts) as validate, \
             patch.object(producer, "_validate_catalog_capture_envelope",
                          side_effect=AssertionError), \
             patch.object(gate, "verify", side_effect=AssertionError), \
             patch.object(producer, "await_quiet", side_effect=AssertionError):
            result = producer.produce_candidate_build_proof(
                self.root, output, self.document, gate.sha256(self.document),
            )

        self.assertEqual(build_call.call_args.args[2], "candidate")
        self.assertEqual(command.call_args.args[0], ["just", "runtime-bundle-check"])
        self.assertEqual(validate.call_count, 1)
        self.assertIs(validate.call_args.kwargs["git_query"], git_query)
        self.assertEqual(result["htmlPerformanceAcceptance"], "not-evaluated")
        self.assertNotIn("performanceAccepted", result)
        self.assertNotIn("acceptance", result)
        self.assertEqual(result["candidateBuildFacts"], facts)
        binary_pin = gate._object(result["candidateProbeBinary"], "candidate binary pin")
        binary_path = Path(gate._text(binary_pin["path"], "candidate binary path"))
        self.assertEqual(binary_path.read_bytes(), b"candidate binary")

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
            self.assertEqual(expected["baseline-0.4.23"]["lock_sha256"], gate.LOCK_SHA)
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
    if mode not in {"exit", "nonzero", "wait", "ignore", "parent", "interrupt-parent-on-term"}:
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
    elif mode == "interrupt-parent-on-term":
        def interrupt_parent(signum: int, frame: FrameType | None) -> None:
            del signum, frame
            os.kill(os.getppid(), signal.SIGINT)
            raise SystemExit(0)

        signal.signal(signal.SIGTERM, interrupt_parent)
        os.kill(os.getppid(), signal.SIGINT)
        time.sleep(30)
    return 7 if mode == "nonzero" else 0


@unittest.skipUnless(os.name == "posix", "actual POSIX lifecycle only")
class MeasurementLifecycleIntegrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.logs = Path(self.temporary.name) / "raw"

    def create_git_fixture(self, parent: Path) -> tuple[Path, Path]:
        root = parent / "repo"
        root.mkdir()
        source_path = root / "source.py"
        source_path.write_text("value = 1\n")
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        self.commit_git_fixture(root, "fixture")
        return root, source_path

    def commit_git_fixture(self, root: Path, message: str) -> None:
        subprocess.run(["git", "-C", str(root), "add", "source.py"], check=True)
        subprocess.run(
            ["git", "-C", str(root), "-c", "user.name=Unit", "-c", "user.email=unit@example.invalid",
             "commit", "-qm", message],
            check=True,
        )

    def invoke(self, mode: str, *, native: bool = False) -> subprocess.CompletedProcess[bytes]:
        return typed_producer._measurement_command(
            [sys.executable, str(Path(__file__).resolve()), "--lifecycle-fixture", mode],
            role="worker", log_directory=self.logs,
            cwd=Path(self.temporary.name).resolve() if native else None,
            env={"PATH": os.environ["PATH"]} if native else None,
            native_attempt_token="f" * 64 if native else None,
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

    def test_native_cleanup_interrupt_is_deferred_until_raw_closure(self) -> None:
        with self.assertRaises(KeyboardInterrupt):
            self.invoke("interrupt-parent-on-term", native=True)
        evidence = self.evidence()
        self.assertIn("KeyboardInterrupt", gate._text(evidence["failure"], "failure"))
        self.assertTrue(evidence["rawFdsClosed"])
        closure = gate._object(evidence["nativeRawFdClosure"], "native closure")
        self.assertEqual(gate._array(closure["holders"], "raw holders"), [])

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
            root, source_path = self.create_git_fixture(Path(temporary))
            git_query = typed_producer._make_git_query(self.logs)
            snapshot = typed_producer.source(root, git_query=git_query)
            self.assertEqual(typed_producer.source(root, git_query=git_query), snapshot)
            self.assertEqual(snapshot["head"], gate.git(root, "rev-parse", "HEAD").strip())
            records = gate.records_map(snapshot["files"])
            self.assertIn(str(source_path), records)
            self.assertEqual(records[str(source_path)], (gate.sha256(source_path), source_path.stat().st_size))
            lifecycle_records = list(self.logs.glob("tool-query-*/lifecycle.json"))
            cat_file_calls = [path for path in lifecycle_records
                              if gate._array(gate.read_json(path)["argv"], "Git argv")[-2:]
                              == ["cat-file", "--batch"]]
            self.assertEqual(len(cat_file_calls), 1)
            for path in lifecycle_records:
                row = gate.read_json(path)
                self.assertTrue(row["groupClosed"])
                self.assertTrue(row["rawFdsClosed"])
            source_path.write_text("value = 2\n")
            self.commit_git_fixture(root, "updated fixture")
            updated = typed_producer.source(root, git_query=git_query)
            self.assertNotEqual(updated["head"], snapshot["head"])
            updated_lifecycles = list(self.logs.glob("tool-query-*/lifecycle.json"))
            updated_cat_file_calls = [path for path in updated_lifecycles
                                      if gate._array(gate.read_json(path)["argv"], "Git argv")[-2:]
                                      == ["cat-file", "--batch"]]
            self.assertEqual(len(updated_cat_file_calls), 2)

    def test_symbolic_cat_file_batch_input_is_never_cached(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, source_path = self.create_git_fixture(Path(temporary))
            query = typed_producer._make_git_query(self.logs)
            first = query(root, ("cat-file", "--batch"), b"HEAD\n", False)
            source_path.write_text("value = 2\n")
            self.commit_git_fixture(root, "updated fixture")
            second = query(root, ("cat-file", "--batch"), b"HEAD\n", False)
            self.assertNotEqual(first, second)
            lifecycle_records = list(self.logs.glob("tool-query-*/lifecycle.json"))
            symbolic_calls = [path for path in lifecycle_records
                              if gate._array(gate.read_json(path)["argv"], "Git argv")[-2:]
                              == ["cat-file", "--batch"]]
            self.assertEqual(len(symbolic_calls), 2)

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
    def test_txt_mapping_without_access_is_read_only_but_writable_controls_remain_holders(self) -> None:
        identity = (0x1000010, 833523274)

        def record(descriptor: str, access: str | None, inode: int = identity[1]) -> bytes:
            fields = ["p40633", "g40633", f"f{descriptor}"]
            if access is not None:
                fields.append(f"a{access}")
            fields.extend(["tREG", f"D{identity[0]:x}", f"i{inode}"])
            return ("\n".join(fields) + "\n").encode("ascii")

        text = record("txt", " ")
        writable = record("3", "w")
        used = record("4", "u")
        unknown = record("5", None)
        for raw in (text, record("txt", ""), record("3", "r")):
            self.assertEqual(typed_producer._writable_raw_holders(raw, [identity]), [])
        for raw in (writable, used, unknown, record("5", " "), record("txt", None)):
            holders = typed_producer._writable_raw_holders(raw, [identity])
            self.assertEqual(len(holders), 1)

        with self.assertRaisesRegex(ValueError, "raw FD observer identity differs"):
            typed_producer._writable_raw_holders(record("txt", "", inode=identity[1] + 1), [identity])

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

    def test_unchecked_nonzero_returns_closed_actual_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            evidence = typed_producer.observe_native_cli_command(
                [sys.executable, "-c", "import os;os.write(1,b'out');os.write(2,b'err');raise SystemExit(7)"],
                role="tool-query", cwd=root, env={"PATH": os.environ["PATH"]},
                log_directory=root / "logs", attempt_token="f" * 64,
                label="probe-nonzero-closed", check=False,
            )
            record = evidence.observation
            lifecycle = gate.read_json(Path(gate._text(evidence.lifecycle["path"], "lifecycle")))
            self.assertEqual(record["actualExit"], 7)
            self.assertIs(record["reaped"], True)
            self.assertEqual(record["errors"], [])
            self.assertEqual(lifecycle["exitCode"], 7)
            self.assertIs(lifecycle["groupClosed"], True)
            self.assertIs(lifecycle["rawFdsClosed"], True)
            for key, expected in (("stdout", b"out"), ("stderr", b"err")):
                pin = gate._object(record[key], key)
                self.assertEqual(Path(gate._text(pin["path"], key)).read_bytes(), expected)


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

    def test_native_spawn_failure_still_observes_raw_fd_closure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            logs = root / "raw"
            with patch.object(typed_producer, "_native_raw_fd_closure", return_value={"holders": []}) as closure:
                with self.assertRaises(FileNotFoundError):
                    typed_producer._measurement_command(
                        [str(root / "missing-worker")], role="worker", log_directory=logs,
                        cwd=root, env=dict(os.environ), native_attempt_token="spawn-failure",
                    )
            self.assertTrue(closure.called)
            record = gate.read_json(next(logs.iterdir()) / "lifecycle.json")
            self.assertIs(record["groupClosed"], False)
            self.assertIs(record["rawFdsClosed"], True)
            self.assertNotIn("native cleanup deadline missing", str(record))


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--lifecycle-fixture":
        raise SystemExit(_lifecycle_fixture(sys.argv[2]))
    unittest.main()
