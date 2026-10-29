"""性能producerのruntime同期preflightと失敗伝播を検証する単体契約。"""
from __future__ import annotations

import copy
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
from typing import BinaryIO, cast
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from collections.abc import Callable, Mapping, Sequence
from types import FrameType

import produce_html_performance_evidence as typed_producer
import html_performance_reference as reference
import verify_html_performance_evidence as gate

producer = typed_producer


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
                del root, args, input_bytes, text
                return "3" * 40

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
    def test_transport_clone_failure_does_not_fall_back_to_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            target = root / "target"
            source.write_bytes(b"original")
            with patch("reflink_copy.reflink", side_effect=OSError(errno.ENOTSUP, "clone unsupported")):
                with self.assertRaises(OSError):
                    producer._clone_transport_file(source, target, gate.sha256(source))
            self.assertFalse(target.exists())


class HtmlPerformanceBatchUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self._batch_counter = 0

    def create_batch(
        self,
        condition_ids: Sequence[str] = ("case-a", "case-b"),
        complete_ids: Sequence[str] | None = None,
        policy_accepted: bool = True,
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
        producer.write_batch_receipt(
            batch, head="a" * 40, source_before=source_before, source_end=source_end,
            context_before=context_before, context_end=context_end, build_artifacts={},
            build_pins=[], condition_ids=condition_ids,
            complete_condition_ids=condition_ids if complete_ids is None else complete_ids,
            policy_accepted=policy_accepted, attempts=attempts, bundle=bundle,
        )
        return batch, bundle

    def test_closed_batch_reader_reuses_only_complete_attempts(self) -> None:
        batch, _ = self.create_batch(complete_ids=("case-a",))
        build_facts = {"baseline": {"proof": "validated"}, "candidate": {"proof": "validated"}}
        with patch.object(gate, "validate_expected_builds", return_value=build_facts) as builds, \
             patch.object(gate, "validate_attempt") as validate:
            receipt, _, attempts = producer.load_closed_batch(batch, self.root)
        self.assertEqual(receipt["completeConditionIds"], ["case-a"])
        self.assertEqual(len(attempts), 1)
        builds.assert_called_once()
        validate.assert_called_once()
        self.assertIs(validate.call_args.kwargs["validated_builds"], build_facts)

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

    def test_budgeted_batch_can_resume_full_acceptance_but_policy_failure_cannot_clear(self) -> None:
        accepted: gate.JsonObject = {"policy": {"policyPass": True}}
        failed: gate.JsonObject = {"policy": {"policyPass": False}}
        self.assertTrue(producer._batch_policy_accepted(
            producer.BatchBudgetExhausted("bounded"), [accepted], True))
        self.assertFalse(producer._batch_formal_acceptance(["case-a"], ["case-a"], True))
        self.assertTrue(producer._batch_policy_accepted(None, [accepted] * 84, True))
        self.assertFalse(producer._batch_formal_acceptance(["case-a"] * 84,
                                                           ["case-a"] * 84, False))
        self.assertFalse(producer._batch_policy_accepted(None, [failed], True))
        self.assertFalse(producer._batch_policy_accepted(
            producer.BatchBudgetExhausted("bounded"), [accepted], False))

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
             patch.object(producer, "quiet", return_value=0.1), \
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
        with patch.object(platform, "system", return_value="Darwin"), \
             patch.object(platform, "machine", return_value="arm64"), \
             patch.object(producer, "load_catalog_conditions",
                          return_value=([], *pins, binary_refs)), \
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
            lifecycle_value = {"argv": list(args), "exitCode": returncode, "raw": []}
            if closed:
                lifecycle_value.update({"identity": {"pid": 4242}, "groupClosed": True,
                    "rawFdsClosed": True, "failure": None, "cleanupFailure": None})
            lifecycle.write_text(json.dumps(lifecycle_value))
            if lifecycle_sink is not None:
                lifecycle_sink(lifecycle)
            payload = json.dumps({"sample": int(args[2]), "viewport": self.viewport,
                                  "elapsed_ms": elapsed_ms}).encode()
            return subprocess.CompletedProcess(args, returncode, payload, b"worker stderr")
        return execute

    def test_retries_only_host_load_and_records_both_attempts(self) -> None:
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", side_effect=[0.31, 0.1]), \
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

    def test_persistent_host_load_stops_after_three_recorded_attempts(self) -> None:
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.3), \
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
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
    ) -> tuple[list[gate.JsonObject], subprocess.CalledProcessError]:
        failures: list[gate.JsonObject] = []
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
             patch.object(producer, "_measurement_command",
                          side_effect=self.command_result(returncode=returncode, closed=closed)):
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                producer.collect_worker_sample(
                    self.build, 0, self.fixture, self.root / "reports.json", self.plan, 800, 600, 1,
                    self.root, {"PATH": os.environ.get("PATH", "")}, self.logs, self.attempts,
                    failures.extend,
                )
        return failures, caught.exception

    def test_closed_baseline_failure_can_continue_without_a_sample(self) -> None:
        failures, error = self._collect_failed_worker_sample(returncode=7, closed=True)
        self.assertTrue(producer._worker_failure_can_continue("baseline", failures, error))
        self.assertEqual(len(failures), 1)
        self.assertIs(failures[0]["accepted"], False)
        self.assertEqual(failures[0]["actualExit"], 7)

    def test_incomplete_baseline_failure_is_not_treated_as_candidate_success(self) -> None:
        failures, error = self._collect_failed_worker_sample(returncode=7, closed=True)
        self.assertFalse(producer._worker_failure_can_continue("candidate", failures, error))

    def test_closed_baseline_failure_keeps_attempt_pin_for_transport(self) -> None:
        failures: list[gate.JsonObject] = []
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        failures, error = self._collect_failed_worker_sample(returncode=7, closed=False)
        self.assertFalse(producer._worker_failure_can_continue("baseline", failures, error))

    def test_signal_terminated_baseline_failure_fails_closed(self) -> None:
        failures, error = self._collect_failed_worker_sample(returncode=-15, closed=True)
        self.assertFalse(producer._worker_failure_can_continue("baseline", failures, error))

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

    def test_condition_collection_keeps_candidates_after_baseline_failure(self) -> None:
        candidate_build, candidate_binary, snapshot, arms, expected, row = self._condition_case()
        baseline_calls = 0
        candidate_calls = 0

        def collect_sample(
            build: typed_producer.ArmBuildResult, sample: int, _fixture: Path,
            _report_path: Path, _plan_path: Path, _width: int, _height: int,
            _scale: int, _root: Path, _environment: Mapping[str, str],
            _lifecycle: Path, attempts_directory: Path,
            failure_sink: Callable[[list[gate.JsonObject]], None] | None = None,
            *, batch_deadline: float | None = None,
        ) -> tuple[gate.JsonObject, dict[str, float], list[gate.JsonObject]]:
            nonlocal baseline_calls, candidate_calls
            if build is self.build:
                baseline_calls += 1
            else:
                candidate_calls += 1
            attempts_directory.mkdir(parents=True, exist_ok=True)
            if build is self.build and baseline_calls == 1:
                lifecycle_path = self.logs / "baseline-failure.json"
                lifecycle_path.parent.mkdir(parents=True, exist_ok=True)
                lifecycle_path.write_text(json.dumps({"identity": {"pid": 4242},
                    "groupClosed": True, "rawFdsClosed": True, "failure": None,
                    "cleanupFailure": None, "exitCode": 7}))
                lifecycle_pin: typed_producer.FilePin = {
                    "path": str(lifecycle_path), "sha256": gate.sha256(lifecycle_path)}
                record: gate.JsonObject = {"sample": sample, "attempt": 1, "actualExit": 7,
                    "workerStarted": True, "accepted": False,
                    "loadRatioBefore": 0.1, "loadRatioAfter": 0.1,
                    "lifecycle": lifecycle_pin,
                    "attemptRecord": producer.write(attempts_directory / "attempt.json", {
                        "sample": sample, "actualExit": 7})}
                if failure_sink is not None:
                    failure_sink([record])
                raise subprocess.CalledProcessError(7, ["worker"])
            success_record: gate.JsonObject = {"sample": sample, "attempt": 1, "actualExit": 0,
                "workerStarted": True, "accepted": True,
                "loadRatioBefore": 0.1, "loadRatioAfter": 0.1,
                "attemptRecord": producer.write(attempts_directory / f"attempt-{sample}.json", {
                    "sample": sample, "actualExit": 0})}
            return ({"sample": sample, "viewport": self.viewport, "elapsed_ms": 1.0},
                    {"before": 0.1, "after": 0.1}, [success_record])

        with patch.object(producer, "collect_worker_sample", side_effect=collect_sample):
            collection = producer._collect_condition_worker_pairs(
                folder=self.root / "condition", row=row, expected=expected,
                fixture=self.fixture, plan_path=self.plan, width=800, height=600,
                viewport=self.viewport, builds={"baseline": self.build, "candidate": candidate_build},
                binaries={"baseline": self.binary, "candidate": candidate_binary}, arms=arms,
                root=self.root, lifecycle=self.logs, lock=self.plan, snapshot=snapshot,
                environment_sha="e" * 64,
            )
        self.assertEqual(baseline_calls, 1)
        self.assertEqual(candidate_calls, 25)
        self.assertEqual(sum(item.get("status") == "missing" for item in collection.order), 4)
        self.assertEqual(sum(item.get("status") == "incomplete" for item in collection.order), 1)
        incomplete = next(item for item in collection.order
                          if item.get("status") == "incomplete")
        self.assertEqual(incomplete["workerLoads"], [])
        failed_loads = gate._array(incomplete["failedWorkerLoads"], "failed worker loads")
        self.assertEqual(failed_loads, [{"before": 0.1, "after": 0.1}])
        self.assertNotIn("status", gate._object(failed_loads[0], "failed worker load"))
        self.assertEqual(len(collection.reports), 10)
        self.assertEqual(collection.actual_phase_records, len(gate.PHASES) * 25)

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
            with self.assertRaises(producer.BatchBudgetExhausted):
                producer._collect_condition_worker_pairs(
                    folder=self.root / "condition", row=row, expected=expected,
                    fixture=self.fixture, plan_path=self.plan, width=800, height=600,
                    viewport=self.viewport,
                    builds={"baseline": self.build, "candidate": candidate_build},
                    binaries={"baseline": self.binary, "candidate": candidate_binary},
                    arms=arms, root=self.root, lifecycle=self.logs, lock=self.plan,
                    snapshot=snapshot, environment_sha="e" * 64, batch_deadline=1.0,
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

        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
             patch.object(producer, "_measurement_command", side_effect=run_command):
            for sample in range(3):
                try:
                    payload, sample_load, sample_attempts = producer.collect_worker_sample(
                        self.build, sample, self.fixture, self.root / "reports.json", self.plan,
                        800, 600, 1, self.root, {"PATH": os.environ.get("PATH", "")},
                        self.logs, self.attempts, failures.extend,
                    )
                except subprocess.CalledProcessError as error:
                    self.assertTrue(producer._worker_failure_can_continue("baseline", failures, error))
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
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        with patch.object(producer, "await_quiet", return_value=0.1), \
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

    def test_worker_start_is_unknown_when_launch_has_no_pid_record(self) -> None:
        def fail_before_spawn(*_args: object, **_kwargs: object) -> None:
            raise FileNotFoundError("worker executable unavailable")

        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        self.assertEqual(attempt["loadRatioAfter"], 0.1)

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

        with patch.object(producer, "await_quiet", return_value=0.1), \
             patch.object(producer, "quiet", return_value=0.1), \
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
        with patch.object(producer, "quiet", side_effect=[0.3, 0.25, 0.25]), \
             patch.object(time, "sleep") as sleep:
            self.assertEqual(producer.await_quiet(), producer.MAX_WORKER_LOAD)
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
