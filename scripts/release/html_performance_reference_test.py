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
