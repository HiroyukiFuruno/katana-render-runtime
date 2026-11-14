"""current-parent性能基準wrapperのidentity契約を検証する。"""
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_reference as reference


def _write(path: Path, value: object) -> dict[str, str]:
    path.write_text(json.dumps(value, sort_keys=True), encoding="utf8")
    return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


class ActiveReferenceTest(unittest.TestCase):
    def history(self, active: str = "first") -> dict[str, object]:
        record = {
            "id": "first", "sourceCommit": "1" * 40, "bundleManifestSha256": "2" * 64,
            "evidence": {"verifierStdoutSha256": "3" * 64},
            "scope": {"candidateConditions": 84},
            "measuredAcceptance": {"verifierExitCode": 0, "performanceNonregression": True,
                                   "outputFontIdentityEquivalence": "established",
                                   "htmlMinimumScore": 92, "htmlScoreThreshold": 90},
            "candidatePhaseStatistics": {name: {"maxSampleSeconds": 1.0} for name in (
                "cold_open", "warm_open", "re_display", "full_y", "post_operation", "same_session_resize")},
            "referenceBuild": {"schema": "krr-html-performance-reference-build-pins-v1",
                               "buildArtifactSchema": "krr-html-performance-build-artifact-v3",
                               **{field: "4" * 64 for field in reference.REFERENCE_BUILD_FIELDS}},
        }
        return {"schema": "krr-html-performance-history-v1", "activeReferenceRecordId": active,
                "records": [record]}

    @staticmethod
    def memory_policy(**overrides: object) -> dict[str, object]:
        return {
            "version": "html-memory-v2", "policySha256": "a" * 64,
            "retentionEvidenceSha256": "b" * 64, "candidateConditions": 84,
            "acceptedConditions": 84, "warmupCycles": 5, "measuredCycles": 25,
            **overrides,
        }

    @staticmethod
    def memory_policy_v3(**overrides: object) -> dict[str, object]:
        return {
            "version": "html-memory-v3", "policySha256": "a" * 64,
            "retentionEvidenceSha256": "b" * 64, "candidateConditions": 84,
            "acceptedConditions": 84, "warmupCycles": 5, "measuredCycles": 25,
            **overrides,
        }

    @staticmethod
    def memory_policy_v4(**overrides: object) -> dict[str, object]:
        return {
            "version": "html-memory-v4", "policySha256": "c" * 64,
            "retentionEvidenceSha256": "d" * 64, "candidateConditions": 84,
            "acceptedConditions": 84, "warmupCycles": 5, "measuredCycles": 25,
            **overrides,
        }

    def history_query(self, value: dict[str, object]) -> reference.GitQuery:
        def query(_root: Path, args: tuple[str, ...]) -> str:
            return reference.HISTORY_PATH if args[0] == "ls-tree" else json.dumps(value)
        return query

    def test_selection_reads_the_requested_commit_and_rejects_missing_current_history(self) -> None:
        old = "a" * 40
        current = "b" * 40
        calls: list[tuple[str, ...]] = []
        def query(_root: Path, args: tuple[str, ...]) -> str:
            calls.append(args)
            if args[0] == "ls-tree":
                return reference.HISTORY_PATH if args[2] == current else ""
            return json.dumps(self.history())
        selected = reference.active_reference_at_commit(Path("/fixture"), current, query)
        self.assertIsNotNone(selected)
        self.assertEqual(selected.record_id if selected else None, "first")
        self.assertIsNone(reference.active_reference_at_commit(Path("/fixture"), old, query,
                                                               allow_legacy_missing=True))
        with self.assertRaisesRegex(ValueError, "absent"):
            reference.active_reference_at_commit(Path("/fixture"), old, query)
        self.assertIn(("show", f"{current}:{reference.HISTORY_PATH}"), calls)

    def test_selection_uses_each_measured_commits_active_record(self) -> None:
        old, current = "a" * 40, "b" * 40
        first = self.history()
        later = self.history()
        rows = later["records"]
        assert isinstance(rows, list) and isinstance(rows[0], dict)
        second = dict(rows[0])
        second["id"] = "second"
        second["sourceCommit"] = "5" * 40
        rows.append(second)
        later["activeReferenceRecordId"] = "second"
        def query(_root: Path, args: tuple[str, ...]) -> str:
            if args[0] == "ls-tree":
                return reference.HISTORY_PATH
            return json.dumps(first if args[1].startswith(old + ":") else later)
        previous = reference.active_reference_at_commit(Path("/fixture"), old, query)
        selected = reference.active_reference_at_commit(Path("/fixture"), current, query)
        self.assertIsNotNone(previous)
        self.assertIsNotNone(selected)
        self.assertEqual(previous.record_id if previous else None, "first")
        self.assertEqual(selected.record_id if selected else None, "second")

    def test_selection_rejects_unaccepted_and_invalid_pins(self) -> None:
        cases = ("missing-active", "duplicate", "failed", "missing-pin", "slow")
        for case in cases:
            with self.subTest(case=case):
                history = self.history("missing" if case == "missing-active" else "first")
                records = history["records"]
                assert isinstance(records, list) and isinstance(records[0], dict)
                record = records[0]
                if case == "duplicate":
                    records.append(dict(record))
                elif case == "failed":
                    acceptance = record["measuredAcceptance"]
                    assert isinstance(acceptance, dict)
                    acceptance["performanceNonregression"] = False
                elif case == "missing-pin":
                    pins = record["referenceBuild"]
                    assert isinstance(pins, dict)
                    pins.pop("binarySha256")
                elif case == "slow":
                    phases = record["candidatePhaseStatistics"]
                    assert isinstance(phases, dict) and isinstance(phases["cold_open"], dict)
                    phases["cold_open"]["maxSampleSeconds"] = 3.0
                with self.assertRaises(ValueError):
                    reference.active_reference_at_commit(Path("/fixture"), "b" * 40,
                                                         self.history_query(history))

    def test_v2_active_reference_accepts_memory_policy_and_rejects_invalid_boundaries(self) -> None:
        history = self.history()
        history["schema"] = "krr-html-performance-history-v2"
        history["activeReferenceSelectionNote"] = "selected by the recorded policy"
        records = history["records"]
        assert isinstance(records, list) and isinstance(records[0], dict)
        records[0]["memoryPolicy"] = self.memory_policy()
        legacy_record = dict(records[0])
        legacy_record["id"] = "legacy"
        legacy_record.pop("memoryPolicy")
        records.append(legacy_record)
        selected = reference.active_reference_at_commit(
            Path("/fixture"), "b" * 40, self.history_query(history),
        )
        self.assertIsNotNone(selected)
        self.assertEqual(selected.record_id if selected else None, "first")

        v1_history = self.history()
        v1_records = v1_history["records"]
        assert isinstance(v1_records, list) and isinstance(v1_records[0], dict)
        v1_records[0]["memoryPolicy"] = self.memory_policy()
        with self.assertRaisesRegex(ValueError, "v1 history cannot contain"):
            reference.active_reference_at_commit(
                Path("/fixture"), "b" * 40, self.history_query(v1_history),
            )

        invalid_policy_values: tuple[tuple[str, object], ...] = (
            ("version", "html-memory-v6"),
            ("policySha256", "A" * 64),
            ("retentionEvidenceSha256", "bad"),
            ("candidateConditions", 83),
            ("acceptedConditions", 83),
            ("warmupCycles", 4),
            ("measuredCycles", 24),
        )
        for field, value in invalid_policy_values:
            with self.subTest(field=field):
                malformed = json.loads(json.dumps(history))
                malformed_records = malformed["records"]
                assert isinstance(malformed_records, list) and isinstance(malformed_records[0], dict)
                policy = malformed_records[0]["memoryPolicy"]
                assert isinstance(policy, dict)
                policy[field] = value
                with self.assertRaises(ValueError):
                    reference.active_reference_at_commit(
                        Path("/fixture"), "b" * 40, self.history_query(malformed),
                    )

        malformed = json.loads(json.dumps(history))
        malformed_records = malformed["records"]
        assert isinstance(malformed_records, list) and isinstance(malformed_records[0], dict)
        acceptance = malformed_records[0]["measuredAcceptance"]
        assert isinstance(acceptance, dict)
        acceptance["performanceNonregression"] = False
        with self.assertRaises(ValueError):
            reference.active_reference_at_commit(
                Path("/fixture"), "b" * 40, self.history_query(malformed),
            )

    def test_v3_active_reference_accepts_only_complete_memory_assessments(self) -> None:
        history = self.history()
        history["schema"] = "krr-html-performance-history-v2"
        history["activeReferenceSelectionNote"] = "existing active reference retained"
        rows = history["records"]
        assert isinstance(rows, list) and isinstance(rows[0], dict)
        rows[0]["memoryPolicy"] = self.memory_policy_v3()
        legacy = dict(rows[0])
        legacy["id"] = "legacy-v1"
        legacy.pop("memoryPolicy")
        rows.append(legacy)
        selected = reference.active_reference_at_commit(
            Path("/fixture"), "b" * 40, self.history_query(history),
        )
        self.assertEqual(selected.record_id if selected else None, "first")

        for field, value in (("candidateConditions", 83), ("acceptedConditions", 83),
                             ("warmupCycles", 4), ("measuredCycles", 24),
                             ("retentionEvidenceSha256", "bad")):
            with self.subTest(field=field):
                invalid = json.loads(json.dumps(history))
                records = invalid["records"]
                assert isinstance(records, list) and isinstance(records[0], dict)
                policy = records[0]["memoryPolicy"]
                assert isinstance(policy, dict)
                policy[field] = value
                with self.assertRaises(ValueError):
                    reference.active_reference_at_commit(
                        Path("/fixture"), "b" * 40, self.history_query(invalid),
                    )

        invalid_v1 = self.history()
        records = invalid_v1["records"]
        assert isinstance(records, list) and isinstance(records[0], dict)
        records[0]["memoryPolicy"] = self.memory_policy_v3()
        with self.assertRaisesRegex(ValueError, "v1 history cannot contain"):
            reference.active_reference_at_commit(
                Path("/fixture"), "b" * 40, self.history_query(invalid_v1),
            )

        unknown = json.loads(json.dumps(history))
        unknown["schema"] = "krr-html-performance-history-v3"
        with self.assertRaisesRegex(ValueError, "unsupported reference history"):
            reference.active_reference_at_commit(
                Path("/fixture"), "b" * 40, self.history_query(unknown),
            )

    def test_v4_reference_accepts_mixed_legacy_history_and_rejects_bad_counts(self) -> None:
        history = self.history()
        history["schema"] = "krr-html-performance-history-v2"
        history["activeReferenceSelectionNote"] = "V4 assessment with legacy records retained"
        records = history["records"]
        assert isinstance(records, list) and isinstance(records[0], dict)
        records[0]["memoryPolicy"] = self.memory_policy_v4()
        legacy = dict(records[0])
        legacy["id"] = "legacy"
        legacy.pop("memoryPolicy")
        records.append(legacy)
        selected = reference.active_reference_at_commit(
            Path("/fixture"), "b" * 40, self.history_query(history),
        )
        self.assertEqual(selected.record_id if selected else None, "first")
        for field in ("candidateConditions", "acceptedConditions"):
            malformed = json.loads(json.dumps(history))
            malformed["records"][0]["memoryPolicy"][field] = 83
            with self.subTest(field=field), self.assertRaises(ValueError):
                reference.active_reference_at_commit(
                    Path("/fixture"), "b" * 40, self.history_query(malformed),
                )

    def test_v5_reference_accepts_mixed_legacy_history_and_rejects_bad_counts(self) -> None:
        history = self.history()
        history["schema"] = "krr-html-performance-history-v2"
        history["activeReferenceSelectionNote"] = "V4 assessment with legacy records retained"
        records = history["records"]
        assert isinstance(records, list) and isinstance(records[0], dict)
        records[0]["memoryPolicy"] = {**self.memory_policy_v4(), "version": "html-memory-v5"}
        legacy = dict(records[0])
        legacy["id"] = "legacy"
        legacy.pop("memoryPolicy")
        records.append(legacy)
        selected = reference.active_reference_at_commit(
            Path("/fixture"), "b" * 40, self.history_query(history),
        )
        self.assertEqual(selected.record_id if selected else None, "first")
        for field in ("candidateConditions", "acceptedConditions"):
            malformed = json.loads(json.dumps(history))
            malformed["records"][0]["memoryPolicy"][field] = 83
            with self.subTest(field=field), self.assertRaises(ValueError):
                reference.active_reference_at_commit(
                    Path("/fixture"), "b" * 40, self.history_query(malformed),
                )

    def test_history_wrapper_binds_selected_candidate_and_rejects_role_or_pin_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary = root / "binary"
            binary.write_bytes(b"binary")
            binary_pin = {"path": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
            snapshot_pin = _write(root / "snapshot.json", {"records": []})
            build_pin = _write(root / "build.json", {"arm": "candidate"})
            context_pin = _write(root / "context.json", {"inputs": [binary_pin]})
            manifest_pin = _write(root / "bundle.json", {
                "schema": "krr-html-performance-release-bundle-v1", "head": "1" * 40})
            stdout_pin = _write(root / "stdout.json", {"acceptance": True})
            pins = {field: "4" * 64 for field in reference.REFERENCE_BUILD_FIELDS}
            pins.update({"binarySha256": binary_pin["sha256"], "sourceSnapshotSha256": snapshot_pin["sha256"],
                         "buildArtifactSha256": build_pin["sha256"], "contextBeforeSha256": context_pin["sha256"]})
            selection = reference.ActiveReference("first", "1" * 40, manifest_pin["sha256"],
                                                  stdout_pin["sha256"], pins)
            wrapper: dict[str, object] = {
                "schema": "krr-html-performance-reference-history-v1", "recordId": "first",
                "role": "reference-history-first", "referenceKind": "accepted-history-candidate",
                "originalArm": "candidate", "publishedSdk": False, "parentHead": "1" * 40,
                "sourceBundleManifest": manifest_pin, "verifierStdout": stdout_pin,
                "originalBuildArtifact": build_pin, "sourceSnapshot": snapshot_pin,
                "frozenBinary": binary_pin, "parentContextBefore": context_pin,
            }
            wrapper_path = root / "wrapper.json"
            validated: dict[str, object] = {"head": "1" * 40, **pins}
            def resolve(value: object, _label: str) -> Path:
                assert isinstance(value, dict)
                path = value.get("path")
                assert isinstance(path, str)
                return Path(path)
            def validate(_value: object, _selection: reference.ActiveReference) -> dict[str, object]:
                return validated
            for field, replacement in ((None, None), ("role", "reference-parent-371274ed"),
                                       ("originalArm", "baseline"), ("recordId", "other")):
                with self.subTest(field=field):
                    current = dict(wrapper)
                    if field is not None:
                        current[field] = replacement
                    _write(wrapper_path, current)
                    if field is None:
                        result = reference.load_history_reference_proof(
                            wrapper_path, selection, file_resolver=resolve, validate_build=validate)
                        self.assertEqual(result.role, "reference-history-first")
                        self.assertEqual(result.binary_sha256, binary_pin["sha256"])
                    else:
                        with self.assertRaises(ValueError):
                            reference.load_history_reference_proof(
                                wrapper_path, selection, file_resolver=resolve, validate_build=validate)
            _write(wrapper_path, wrapper)
            validated["binarySha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "validated build differs"):
                reference.load_history_reference_proof(
                    wrapper_path, selection, file_resolver=resolve, validate_build=validate)


class ReferenceParentProofTest(unittest.TestCase):
    def test_loader_preserves_candidate_origin_and_parent_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "scripts/html/performance-harness"
            (harness / "src").mkdir(parents=True)
            snapshot = root / "snapshot.json"
            source_before = root / "source-before.json"
            binary = root / "worker"
            binary.write_bytes(b"worker")
            cargo = root / "cargo"
            cargo.write_bytes(b"cargo")
            rustc = root / "rustc"
            rustc.write_bytes(b"rustc")
            raw_pin = root / "cargo.jsonl"
            raw_pin.write_bytes(b"{}\n")
            context = root / "context.json"
            parent_lock = (b'version = 3\n\n[[package]]\nname = "krr-html-performance-harness"\n'
                           b'version = "0.1.0"\ndependencies = ["serde 1.0.0", "katana-render-runtime 0.4.23"]\n\n'
                           b'[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+https://example.invalid/index"\n'
                           b'checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"\n\n'
                           b'[[package]]\nname = "katana-render-runtime"\nversion = "0.4.23"\ndependencies = ["sdk-child 1.0.0"]\n\n'
                           b'[[package]]\nname = "sdk-child"\nversion = "1.0.0"\nsource = "registry+https://example.invalid/index"\n'
                           b'checksum = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"\n')
            current_lock = parent_lock.replace(b'katana-render-runtime 0.4.23', b'katana-render-runtime 0.4.24')
            current_lock = current_lock.replace(b'version = "0.4.23"', b'version = "0.4.24"')
            current_lock = current_lock.replace(b'sdk-child 1.0.0', b'sdk-child 2.0.0')
            current_lock = current_lock.replace(b'version = "1.0.0"\nsource = "registry+https://example.invalid/index"\n'
                                                b'checksum = "bbbb',
                                                b'version = "2.0.0"\nsource = "registry+https://example.invalid/index"\n'
                                                b'checksum = "bbbb')
            (harness / "Cargo.lock").write_bytes(current_lock)
            context_ref = _write(context, {"inputs": [{
                "path": str(harness / "Cargo.lock"), "sha256": hashlib.sha256(parent_lock).hexdigest(),
            }]})
            snapshot_records: list[dict[str, object]] = []
            for index in range(33):
                relative = f"scripts/html/performance-harness/src/source-{index:02d}.rs"
                source = f"// retained parent source {index}\n".encode()
                path = root / relative
                path.write_bytes(source)
                snapshot_records.append({"path": relative, "sha256": hashlib.sha256(source).hexdigest(),
                                         "size": len(source)})
            snapshot_records.append({"path": "scripts/html/performance-harness/Cargo.lock",
                                    "sha256": hashlib.sha256(parent_lock).hexdigest(),
                                    "size": len(parent_lock)})
            snapshot_ref = _write(snapshot, {"records": snapshot_records})
            source_before_ref = _write(source_before, {"head": reference.PARENT_HEAD})
            build = root / "build.json"
            build_ref = _write(build, {
                "schema": "krr-html-performance-build-artifact-v2",
                "arm": "candidate",
                "buildExit": 0,
                "sourceSnapshotRef": snapshot_ref,
                "sourceBeforeRef": source_before_ref,
                "selectedBinary": {"sha256": reference.PARENT_BINARY_SHA256},
                "frozenBinary": {"sha256": reference.PARENT_BINARY_SHA256},
                "lockSha256": hashlib.sha256(parent_lock).hexdigest(),
                "cargoConfigPins": [],
                "cargoExecutable": {"path": str(cargo), "sha256": hashlib.sha256(cargo.read_bytes()).hexdigest()},
                "rustcExecutable": {"path": str(rustc), "sha256": hashlib.sha256(rustc.read_bytes()).hexdigest()},
                "rawPins": [{"path": str(raw_pin), "sha256": hashlib.sha256(raw_pin.read_bytes()).hexdigest()}],
                "argv": ["cargo", "build", "--locked"],
                "expectedRuntime": {"name": "katana-render-runtime", "version": "0.4.24", "source": None,
                                    "manifest_path": "/root/crates/katana-render-runtime/Cargo.toml"},
                "expectedHarness": {"name": "krr-html-performance-harness",
                                    "manifest_path": "/root/scripts/html/performance-harness/Cargo.toml"},
            })
            binary.write_bytes(b"x" * 32)
            binary_ref = {"path": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
            build_value = json.loads(build.read_text(encoding="utf8"))
            build_value["selectedBinary"]["sha256"] = binary_ref["sha256"]
            build_value["frozenBinary"]["sha256"] = binary_ref["sha256"]
            build_ref = _write(build, build_value)
            wrapper = root / "wrapper.json"
            wrapper.write_text(json.dumps({
                "schema": "krr-html-performance-reference-parent-v1",
                "role": reference.REFERENCE_ROLE,
                "referenceKind": reference.REFERENCE_KIND,
                "publishedSdk": False,
                "originalArm": "candidate",
                "parentHead": reference.PARENT_HEAD,
                "sourceSnapshot": snapshot_ref,
                "frozenBinary": {"path": binary_ref["path"], "sha256": binary_ref["sha256"]},
                "lockSha256": hashlib.sha256(parent_lock).hexdigest(),
                "originalBuildArtifact": build_ref,
                "originalBuildArtifactSha256": build_ref["sha256"],
                "parentContextBefore": context_ref,
            }), encoding="utf8")
            with patch.object(reference, "PARENT_SOURCE_SNAPSHOT_SHA256", snapshot_ref["sha256"]), \
                    patch.object(reference, "PARENT_BINARY_SHA256", binary_ref["sha256"]), \
                    patch.object(reference, "PARENT_LOCK_SHA256", hashlib.sha256(parent_lock).hexdigest()), \
                    patch.object(reference, "PARENT_BUILD_ARTIFACT_SHA256", build_ref["sha256"]), \
                    patch.object(reference, "PARENT_CARGO_SHA256", hashlib.sha256(cargo.read_bytes()).hexdigest()), \
                    patch.object(reference, "PARENT_RUSTC_SHA256", hashlib.sha256(rustc.read_bytes()).hexdigest()), \
                    patch.object(reference, "PARENT_CONTEXT_BEFORE_SHA256", context_ref["sha256"]):
                def git_query(query_root: Path, args: tuple[str, ...]) -> str:
                    if args[0] == "rev-parse":
                        return reference.PARENT_HEAD
                    if args[0] == "show":
                        return parent_lock.decode("utf8")
                    if args[0] == "ls-tree":
                        return "\n".join(str(record["path"]) for record in snapshot_records) + "\n"
                    path = args[-1]
                    return "100644 blob " + hashlib.sha256((query_root / path).read_bytes()).hexdigest() + " " + path

                proof = reference.load_reference_parent_proof(
                    wrapper, root, git_query=git_query,
                )
                resolved_labels: list[str] = []
                def resolve_bundle_file(value: object, label: str) -> Path:
                    path_value = value.get("path") if isinstance(value, dict) else None
                    if not isinstance(path_value, str):
                        raise ValueError("bundle file reference is invalid")
                    resolved_labels.append(label)
                    return Path(path_value)
                bundle_proof = reference.load_reference_parent_proof(
                    wrapper, root, git_query=git_query, file_resolver=resolve_bundle_file,
                )
                self.assertEqual(bundle_proof.lock_sha256, proof.lock_sha256)
                self.assertNotIn("parent runtime input", resolved_labels)
            self.assertEqual(proof.original_arm, "candidate")
            self.assertEqual(reference.reference_parent_contract(proof)["publishedSdk"], False)
            def load_with_pins(query: reference.GitQuery | None = git_query) -> reference.ReferenceParentProof:
                with patch.object(reference, "PARENT_SOURCE_SNAPSHOT_SHA256", snapshot_ref["sha256"]), \
                        patch.object(reference, "PARENT_BINARY_SHA256", binary_ref["sha256"]), \
                        patch.object(reference, "PARENT_LOCK_SHA256", hashlib.sha256(parent_lock).hexdigest()), \
                        patch.object(reference, "PARENT_BUILD_ARTIFACT_SHA256", build_ref["sha256"]), \
                        patch.object(reference, "PARENT_CARGO_SHA256", hashlib.sha256(cargo.read_bytes()).hexdigest()), \
                        patch.object(reference, "PARENT_RUSTC_SHA256", hashlib.sha256(rustc.read_bytes()).hexdigest()), \
                        patch.object(reference, "PARENT_CONTEXT_BEFORE_SHA256", context_ref["sha256"]):
                    return reference.load_reference_parent_proof(wrapper, root, git_query=query)
            valid_wrapper = json.loads(wrapper.read_text(encoding="utf8"))
            controls = {
                "wrong role": ("role", "wrong-role"),
                "wrong parent": ("parentHead", "0" * 40),
                "wrong original proof pin": ("originalBuildArtifactSha256", "0" * 64),
                "wrong worker pin": ("frozenBinary", {"path": binary_ref["path"], "sha256": "0" * 64}),
                "wrong source pin": ("sourceSnapshot", {"path": snapshot_ref["path"], "sha256": "0" * 64}),
                "wrong context pin": ("parentContextBefore", {"path": context_ref["path"], "sha256": "0" * 64}),
            }
            for label, (field, value) in controls.items():
                mutated = dict(valid_wrapper)
                mutated[field] = value
                wrapper.write_text(json.dumps(mutated), encoding="utf8")
                with self.subTest(control=label), self.assertRaises(ValueError):
                    load_with_pins()
            wrapper.write_text(json.dumps(valid_wrapper), encoding="utf8")
            def changed_parent_lock(query_root: Path, args: tuple[str, ...]) -> str:
                if args[0] == "show":
                    return parent_lock.decode("utf8").replace("serde 1.0.0", "serde 1.1.0")
                return git_query(query_root, args)
            with self.assertRaisesRegex(ValueError, "immutable pin"):
                load_with_pins(changed_parent_lock)
            with self.assertRaisesRegex(ValueError, "commit query"):
                load_with_pins(None)

    def test_loader_rejects_edited_original_arm(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            wrapper = Path(temporary) / "wrapper.json"
            wrapper.write_text(json.dumps({
                "schema": "krr-html-performance-reference-parent-v1",
                "role": reference.REFERENCE_ROLE,
                "referenceKind": reference.REFERENCE_KIND,
                "publishedSdk": False,
                "originalArm": "baseline",
                "parentHead": reference.PARENT_HEAD,
            }), encoding="utf8")
            with self.assertRaisesRegex(ValueError, "original reference arm"):
                reference.load_reference_parent_proof(wrapper, Path(temporary))

    def test_loader_rejects_duplicate_wrapper_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            wrapper = Path(temporary) / "wrapper.json"
            wrapper.write_text('{"schema":"x","schema":"y"}', encoding="utf8")
            with self.assertRaisesRegex(ValueError, "duplicate JSON keys"):
                reference.load_reference_parent_proof(wrapper, Path(temporary))


if __name__ == "__main__":
    unittest.main()
