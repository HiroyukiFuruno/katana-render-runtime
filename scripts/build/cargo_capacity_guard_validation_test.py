from __future__ import annotations

from dataclasses import replace
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from cargo_capacity_guard import CAPACITY_METRIC, CAPACITY_METRIC_VERSION, DEFAULT_FACTOR
from cargo_capacity_guard_build_script_ownership import SourcePin, source_pin
from cargo_capacity_guard_validation import (
    CapacityInputs,
    capture_candidates,
    capture_snapshot,
    compare_candidates,
    compare_snapshots,
    verify_code_sha256,
)


class CapacityValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.target = self.root / "target"
        self.tree = self.target / "current"
        self.tree.mkdir(parents=True)
        self.code = self.root / "guard.py"
        self.output = self.target / "consumer"
        self.state = self.root / "state.json"
        self.lock = self.root / "Cargo.lock"
        for path in (self.code, self.output, self.lock, self.tree / "input"):
            path.write_bytes(b"retained bytes")
        self.state.write_text(json.dumps({
            "target": str(self.target), "baseline_bytes": 1_000_000,
            "metric": CAPACITY_METRIC, "metric_version": CAPACITY_METRIC_VERSION,
        }))
        self.inputs = CapacityInputs(self.root, self.target, (self.code,), (self.output,),
                                     (self.tree,), self.state, self.lock)

    def test_snapshot_reuses_guard_inspection_and_preserves_physical_pins(self) -> None:
        before = capture_snapshot(self.inputs)
        after = capture_snapshot(self.inputs)
        self.assertEqual(before.pins.code[0].source, source_pin(self.code))
        self.assertGreater(before.current_bytes, 0)
        self.assertEqual(before.threshold_bytes, 1_000_000 * DEFAULT_FACTOR)
        self.assertTrue(compare_snapshots(before, after).strict_unchanged)

    def test_expected_sha_is_compared_to_source_pin_digest_not_record(self) -> None:
        observed = capture_snapshot(self.inputs)
        expected = {self.code: source_pin(self.code).sha256}
        self.assertNotEqual(observed.pins.code[0].source, expected[self.code])
        verify_code_sha256(observed, expected)
        with self.assertRaisesRegex(RuntimeError, "SHA"):
            verify_code_sha256(observed, {self.code: "0" * 64})
        with self.assertRaisesRegex(RuntimeError, "set"):
            verify_code_sha256(observed, {})

    def test_source_state_lock_and_tree_drift_remain_strict_failures(self) -> None:
        for path in (self.code, self.state, self.lock, self.tree / "input"):
            with self.subTest(path=path.name):
                before = capture_snapshot(self.inputs)
                path.write_bytes(path.read_bytes() + (b" " if path == self.state else b" changed"))
                result = compare_snapshots(before, capture_snapshot(self.inputs))
                self.assertFalse(result.strict_unchanged)
                self.assertTrue(result.changes)
                self.assertTrue(all(change.classification == "unexpected" for change in result.changes))

    def test_declared_build_output_metadata_does_not_change_strict_failure(self) -> None:
        before = capture_snapshot(self.inputs)
        replacement = self.target / "replacement"
        replacement.write_bytes(self.output.read_bytes())
        os.replace(replacement, self.output)
        result = compare_snapshots(before, capture_snapshot(self.inputs), (self.output,))
        self.assertFalse(result.strict_unchanged)
        self.assertEqual(len(result.changes), 1)
        self.assertEqual(result.changes[0].classification, "declared-build-output-metadata")
        self.assertEqual(result.changes[0].path, self.output)

    def test_declared_output_content_change_is_reported_not_accepted(self) -> None:
        before = capture_snapshot(self.inputs)
        self.output.write_bytes(b"new compiled bytes")
        result = compare_snapshots(before, capture_snapshot(self.inputs), (self.output,))
        self.assertFalse(result.strict_unchanged)
        self.assertEqual(result.changes[0].classification, "declared-build-output-content")

    def test_declarations_cannot_reclassify_source_or_unknown_path(self) -> None:
        observed = capture_snapshot(self.inputs)
        for outputs in ((self.code,), (self.target / "missing",), (self.output, self.output)):
            with self.subTest(outputs=outputs):
                with self.assertRaisesRegex(RuntimeError, "declared|duplicate"):
                    compare_snapshots(observed, observed, outputs)

    def test_inputs_require_canonical_root_paths_and_unique_roles(self) -> None:
        variants = (
            replace(self.inputs, root=Path(".")),
            replace(self.inputs, code=(self.root / "target" / ".." / "guard.py",)),
            replace(self.inputs, code=(self.code, self.code)),
            replace(self.inputs, state=self.output),
        )
        for inputs in variants:
            with self.subTest(inputs=inputs):
                with self.assertRaises(RuntimeError):
                    capture_snapshot(inputs)

    def test_file_directory_symlink_and_unknown_kind_are_rejected(self) -> None:
        link = self.root / "link"
        link.symlink_to(self.code)
        bad_paths = [link, self.tree]
        if hasattr(os, "mkfifo"):
            fifo = self.root / "fifo"
            os.mkfifo(fifo)
            bad_paths.append(fifo)
        for path in bad_paths:
            with self.subTest(path=path.name):
                with self.assertRaises(RuntimeError):
                    capture_snapshot(replace(self.inputs, code=(path,)))
        (self.tree / "linked-dir").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(RuntimeError):
            capture_snapshot(self.inputs)

    def test_drift_during_snapshot_is_rejected(self) -> None:
        original = source_pin
        changed = False

        def changing_pin(path: Path) -> SourcePin:
            nonlocal changed
            pin = original(path)
            if path == self.code and not changed:
                changed = True
                path.write_bytes(b"drift during observation")
            return pin

        with patch("cargo_capacity_guard_validation.source_pin", side_effect=changing_pin):
            with self.assertRaisesRegex(RuntimeError, "drift"):
                capture_snapshot(self.inputs)

    def test_candidate_set_order_is_irrelevant_but_membership_and_pins_are_bound(self) -> None:
        candidates = (self.target / "older-a", self.target / "older-b")
        for path in candidates:
            path.write_bytes(b"optional cache")
        snapshot = capture_snapshot(self.inputs)
        first = capture_candidates(snapshot, candidates)
        self.assertTrue(compare_candidates(first, capture_candidates(snapshot, candidates[::-1])))
        self.assertFalse(compare_candidates(first, capture_candidates(snapshot, candidates[:1])))
        candidates[0].write_bytes(b"changed optional cache")
        self.assertFalse(compare_candidates(first, capture_candidates(snapshot, candidates)))

    def test_candidates_reject_alias_protection_outside_target_and_duplicates(self) -> None:
        alias = self.target / "alias"
        os.link(self.output, alias)
        snapshot = capture_snapshot(self.inputs)
        for paths in ((self.output,), (alias,), (self.code,), (alias, alias)):
            with self.subTest(paths=paths):
                with self.assertRaises(RuntimeError):
                    capture_candidates(snapshot, paths)
        for role in ("retained-tree", "code", "state", "lock"):
            path = self.tree / "input" if role == "retained-tree" else self.target / role
            source = self.state if role == "state" else self.code
            if role != "retained-tree":
                path.write_bytes(source.read_bytes())
            inputs = self.inputs
            if role == "code":
                inputs = replace(inputs, code=(path,))
            elif role == "state":
                inputs = replace(inputs, state=path)
            elif role == "lock":
                inputs = replace(inputs, lock=path)
            hardlink = self.target / (role + "-alias")
            os.link(path, hardlink)
            snapshot = capture_snapshot(inputs)
            for candidate in (path, hardlink):
                with self.subTest(role=role, candidate=candidate.name):
                    with self.assertRaisesRegex(RuntimeError, "retained"):
                        capture_candidates(snapshot, (candidate,))

    def test_candidate_capture_rejects_stale_snapshot_and_in_observation_drift(self) -> None:
        candidate = self.target / "older"
        candidate.write_bytes(b"optional")
        snapshot = capture_snapshot(self.inputs)
        self.lock.write_bytes(b"different lock")
        with self.assertRaisesRegex(RuntimeError, "snapshot drift"):
            capture_candidates(snapshot, (candidate,))
        snapshot = capture_snapshot(self.inputs)
        original = source_pin

        def changing_candidate(path: Path) -> SourcePin:
            pin = original(path)
            if path == candidate:
                self.code.write_bytes(b"source changed during candidate observation")
            return pin

        with patch("cargo_capacity_guard_validation.source_pin", side_effect=changing_candidate):
            with self.assertRaisesRegex(RuntimeError, "drift"):
                capture_candidates(snapshot, (candidate,))

    def test_input_sets_cannot_be_changed_between_comparisons(self) -> None:
        before = capture_snapshot(self.inputs)
        after = capture_snapshot(replace(self.inputs, protected=()))
        with self.assertRaisesRegex(RuntimeError, "inputs"):
            compare_snapshots(before, after)

    def test_invalid_state_fails_existing_guard_validation(self) -> None:
        self.state.write_text("{}")
        with self.assertRaisesRegex(RuntimeError, "threshold"):
            capture_snapshot(self.inputs)


if __name__ == "__main__":
    unittest.main()
