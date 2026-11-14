from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from contextlib import nullcontext
from pathlib import Path
from unittest.mock import patch

import html_performance_retained_runs as retention
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate


class RetainedRunsTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.history = self.root / "docs/performance/html-performance-history.json"
        self.history.parent.mkdir(parents=True)
        self.mark("a" * 64)
        self.namespace = self.root / "tmp/html-performance-retained"

    def mark(self, checksum: str) -> None:
        self.history.write_text(json.dumps({
            "activeReferenceRecordId": "reviewed",
            "records": [{"id": "reviewed", "bundleManifestSha256": checksum}],
        }))

    def seal(self, output: Path, head: str) -> str:
        manifest = output / "bundle/bundle.json"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(json.dumps({"head": head, "schema": "krr-html-performance-release-bundle-v1",
                                        "attempts": [{"condition": "condition"}], "files": []}))
        checksum = gate.sha256(manifest)
        (output / "batch-receipt.json").write_text(json.dumps({
            "schema": "krr-html-performance-batch-receipt-v2",
            "status": "closed",
            "completeConditionIds": ["condition"],
            "bundle": {"path": str(manifest), "sha256": checksum},
        }))
        return checksum

    def test_repeated_runs_retain_only_marked_reference_and_latest_candidate(self) -> None:
        with retention.managed_output(self.root) as baseline:
            self.mark(self.seal(baseline, "baseline"))
        retained_bytes = []
        for index in range(5):
            with retention.managed_output(self.root) as candidate:
                self.seal(candidate, str(index))
                (candidate / "stdout.raw").write_bytes(b"observation")
            self.assertEqual(set(self.namespace.iterdir()), {baseline.parent, candidate.parent})
            retained_bytes.append(sum(path.stat().st_size for path in self.namespace.rglob("*") if path.is_file()))
        self.assertEqual(len(set(retained_bytes)), 1)
        with self.assertRaises(RuntimeError):
            with retention.managed_output(self.root) as failed:
                failed.mkdir()
                (failed / "stdout.raw").write_bytes(b"failed")
                raise RuntimeError("interrupted")
        self.assertEqual(set(self.namespace.iterdir()), {baseline.parent, candidate.parent})
        self.mark(gate.sha256(candidate / "bundle/bundle.json"))
        with retention.managed_output(self.root):
            pass
        self.assertEqual(set(self.namespace.iterdir()), {candidate.parent})
        with retention.managed_output(self.root) as replacement:
            self.seal(replacement, "replacement")
        self.assertEqual(set(self.namespace.iterdir()), {candidate.parent, replacement.parent})

    def test_resume_keeps_originals_until_formally_complete(self) -> None:
        with retention.managed_output(self.root) as baseline:
            self.mark(self.seal(baseline, "baseline"))

        def seal_with_originals(output: Path, originals: list[Path], *, final: bool = False) -> Path:
            payload = output / "original"
            payload.parent.mkdir(parents=True, exist_ok=True)
            payload.write_bytes(str(output).encode())
            originals = [*originals, payload]
            bundle = output / "bundle"
            bundle.mkdir()
            rows = []
            for index, original in enumerate(originals):
                copy = bundle / str(index)
                copy.write_bytes(original.read_bytes())
                rows.append({"originalPath": str(original), "relativePath": str(index),
                             "sha256": gate.sha256(original)})
            manifest = bundle / "bundle.json"
            manifest.write_text(json.dumps({"schema": "krr-html-performance-release-bundle-v1",
                                            "head": "head", "attempts": [{}] * (84 if final else 1),
                                            "files": rows}))
            ids = [str(index) for index in range(84 if final else len(originals))]
            (output / "batch-receipt.json").write_text(json.dumps({
                "schema": "krr-html-performance-batch-receipt-v2", "status": "closed",
                "head": "head",
                "conditionIds": ids, "completeConditionIds": ids, "formalAccepted": final,
                "bundle": {"path": str(manifest), "sha256": gate.sha256(manifest)},
            }))
            return payload

        originals: list[Path] = []
        identities: list[int] = []
        runs: list[Path] = []
        for _ in range(3):
            with retention.managed_output(self.root) as output:
                original = seal_with_originals(output, originals)
            originals.append(original)
            identities.append(original.stat().st_ino)
            runs.append(output.parent)
            self.assertEqual([path.stat().st_ino for path in originals], identities)
            self.assertEqual(set(self.namespace.iterdir()), {baseline.parent, *runs})
        displaced = self.root / "displaced"
        runs[0].rename(displaced)
        retention.reconcile_reference(self.root)
        self.assertTrue(all(run.is_dir() for run in runs[1:]))
        displaced.rename(runs[0])
        with retention.managed_output(self.root) as final:
            seal_with_originals(final, originals, final=True)
        self.assertEqual(set(self.namespace.iterdir()), {baseline.parent, *runs, final.parent})
        producer.write_formal_verification_receipt(final / "bundle", "head", {
            "head": "head", "conditions": 84, "candidateConditions": 84, "checks": 1680,
            "htmlStage90": {"acceptance": True, "fontIdentityEquivalence": "established"},
            "performanceNonregression": True,
        })
        retention.reconcile_reference(self.root)
        self.assertEqual(set(self.namespace.iterdir()), {baseline.parent, final.parent})

    def test_missing_or_changed_original_refuses_pruning(self) -> None:
        with retention.managed_output(self.root) as first:
            self.seal(first, "first")
            original = first / "original"
            original.write_bytes(b"original")
        with retention.managed_output(self.root) as second:
            self.seal(second, "second")
            bundle = second / "bundle"
            copy = bundle / "copy"
            copy.write_bytes(original.read_bytes())
            manifest = bundle / "bundle.json"
            manifest.write_text(json.dumps({"schema": "krr-html-performance-release-bundle-v1",
                "files": [{"originalPath": str(original), "relativePath": "copy",
                           "sha256": gate.sha256(copy)}]}))
            receipt = gate.read_json(second / "batch-receipt.json")
            receipt["bundle"] = {"path": str(manifest), "sha256": gate.sha256(manifest)}
            (second / "batch-receipt.json").write_text(json.dumps(receipt))
            original.write_bytes(b"changed")
        self.assertTrue(first.is_dir())
        self.assertTrue(second.is_dir())
        with retention.managed_output(self.root) as fresh:
            self.seal(fresh, "fresh")
        self.assertEqual(set(self.namespace.iterdir()), {fresh.parent})

    def test_successive_managed_runs_prune_only_superseded_legacy_stdout(self) -> None:
        legacy = self.root / "tmp/quality/owner"

        def old_bundle(name: str, *, measurement: bool = True) -> tuple[Path, Path, Path]:
            directory = legacy / name / "measurement"
            payload = directory / "bundle/payload/0/stdout.raw"
            payload.parent.mkdir(parents=True)
            payload.write_bytes(b"raw")
            original = directory.with_name("measurement.lifecycle") / "tool-query-1/stdout.raw"
            original.parent.mkdir(parents=True)
            original.write_bytes(b"raw")
            manifest = directory / "bundle/bundle.json"
            manifest.write_text(json.dumps({
                "schema": "krr-html-performance-release-bundle-v1",
                "attempts": [{}] if measurement else [],
                "files": [{"relativePath": "payload/0/stdout.raw", "originalPath": str(original)}],
            }))
            return manifest, payload, original

        active, baseline_copy, baseline_original = old_bundle("active")
        self.mark(gate.sha256(active))
        _, unrelated_copy, unrelated_original = old_bundle("build-only", measurement=False)
        for index in range(3):
            manifest, payload, original = old_bundle(f"superseded-{index}")
            unbundled = original.parent.parent / "unbundled/stdout.raw"
            unbundled.parent.mkdir()
            unbundled.write_bytes(b"raw")
            with retention.managed_output(self.root) as candidate:
                self.seal(candidate, str(index))
            self.assertTrue(manifest.is_file())
            self.assertFalse(any(path.exists() for path in (payload, original, unbundled)))
            self.assertTrue(all(p.is_file() for p in (
                baseline_copy, baseline_original, unrelated_copy, unrelated_original,
            )))
            self.assertEqual(list(self.namespace.iterdir()), [candidate.parent])

    def test_reference_retirement_resumes_only_after_selection_changes(self) -> None:
        old = self.root / "tmp/quality/owner/previous/measurement/bundle"
        old_payload = old / "payload/0/stdout.raw"
        old_payload.parent.mkdir(parents=True)
        old_payload.write_bytes(b"old")
        old_manifest = old / "bundle.json"
        old_manifest.write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1",
            "attempts": [{}] * 84,
            "files": [{"relativePath": "payload/0/stdout.raw", "originalPath": str(old_payload),
                       "sha256": gate.sha256(old_payload)}],
        }))
        old_sha = gate.sha256(old_manifest)
        self.mark(old_sha)
        new = self.root / "tmp/replacement/bundle.json"
        new.parent.mkdir()
        new.write_text(json.dumps({"schema": "krr-html-performance-release-bundle-v1",
                                   "attempts": [{}] * 84, "files": []}))
        new_sha = gate.sha256(new)
        retention.stage_reference_change(self.root, new, new_sha)
        retention.reconcile_reference(self.root)
        self.assertEqual(gate.sha256(old_manifest), old_sha)
        self.assertEqual(old_payload.read_bytes(), b"old")

        retention.stage_reference_change(self.root, new, new_sha)
        self.mark(new_sha)
        missing = new.rename(new.with_suffix(".saved"))
        with self.assertRaisesRegex(ValueError, "replacement reference is missing"):
            retention.reconcile_reference(self.root)
        self.assertEqual(gate.sha256(old_manifest), old_sha)
        missing.rename(new)
        with retention.managed_output(self.root):
            self.assertFalse(old.exists())
        self.assertEqual(retention.reconcile_reference(self.root), new_sha)
        self.assertEqual(gate.sha256(new), new_sha)

    def test_incomplete_previous_run_is_swept_without_touching_foreign_directory(self) -> None:
        self.namespace.mkdir(parents=True)
        stale = self.namespace / ("run-" + "0" * 32)
        foreign = self.namespace / "outside"
        stale.mkdir()
        foreign.mkdir()
        with retention.managed_output(self.root) as candidate:
            self.assertFalse(stale.exists())
            self.seal(candidate, "candidate")
        self.assertEqual(set(self.namespace.iterdir()), {foreign, candidate.parent})

    def test_registers_only_the_marked_baseline_and_copies_its_payload(self) -> None:
        bundle = self.root / "imported"
        bundle.mkdir()
        (bundle / "sample").write_bytes(b"measurement")
        manifest = bundle / "bundle.json"
        manifest.write_text(json.dumps({
            "schema": "krr-html-performance-release-bundle-v1",
            "files": [{"originalPath": "/old-machine/sample", "relativePath": "sample",
                       "sha256": gate.sha256(bundle / "sample")}],
        }))
        with self.assertRaisesRegex(ValueError, "not marked"):
            with retention.managed_output(self.root, baseline=bundle):
                self.fail("an unmarked reference was registered")
        self.mark(gate.sha256(manifest))
        with retention.managed_output(self.root, baseline=bundle) as candidate:
            self.seal(candidate, "candidate")
        (bundle / "sample").unlink()
        retained = set(self.namespace.iterdir()) - {candidate.parent}
        self.assertEqual(len(retained), 1)
        baseline = next(iter(retained)) / "measurement/bundle"
        self.assertEqual((baseline / "sample").read_bytes(), b"measurement")

    def test_default_cli_retains_locally_and_preserves_ci_artifact_path(self) -> None:
        argv = ["produce", "--repo", str(self.root), "--catalog-plan", str(self.root / "catalog")]
        with (patch.object(sys, "argv", argv),
              patch.object(producer, "candidate_build_locks", return_value=nullcontext()),
              patch.object(producer, "produce") as produce):
            with patch.dict(os.environ, {}, clear=True):
                producer.main()
                self.assertTrue(produce.call_args.args[1].is_relative_to(self.namespace))
                self.assertEqual(list(self.namespace.iterdir()), [])
            with patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}, clear=True):
                producer.main()
                self.assertEqual(produce.call_args.args[1], Path("tmp/html-performance-producer").resolve())
