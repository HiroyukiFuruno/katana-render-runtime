"""Retention collection adapters preserve capture and policy boundaries."""
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_memory_evidence as memory
import html_performance_memory_policy as memory_policy
import html_performance_retention_capture as capture
import html_performance_retention_collection as collection
import html_performance_retention_evidence as evidence
import verify_html_performance_evidence as gate
from produce_html_performance_evidence import FilePin


class RetentionCollectionAdapterTests(unittest.TestCase):
    def test_collect_uses_constructed_argv_and_attaches_capture_and_transport(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary).resolve()
            expected: gate.JsonObject = {"viewport": {"width": 800, "height": 600, "scale": 1}}
            observed = capture.RetentionCapture(folder / "capture", folder / "lifecycle.json", {})
            transport_path = folder / "raw.json"
            reference_path = folder / "retention.json"
            transport_path.write_text("{}")
            reference_path.write_text("{}")
            transport = [FilePin(path=str(transport_path), sha256=gate.sha256(transport_path))]
            reference = FilePin(path=str(reference_path), sha256=gate.sha256(reference_path))
            with patch.object(capture, "capture", return_value=observed) as capture_call, \
                 patch.object(evidence, "write_retention_evidence", return_value=(reference, transport)) as writer:
                result = collection.collect(
                    folder, expected, binary=folder / "renderer", fixture=folder / "case.html",
                    plan=folder / "plan.json", root=folder, environment={"MODE": "test"},
                )

            argv = [str(folder / "renderer"), "--retention", str(folder / "case.html"),
                    str(folder / "unused-retention-output.json"), str(folder / "plan.json"),
                    "800", "600", "1"]
            capture_call.assert_called_once()
            self.assertEqual(capture_call.call_args.args[0], argv)
            self.assertEqual(capture_call.call_args.kwargs["cwd"], folder)
            self.assertEqual(capture_call.call_args.kwargs["env"], {"MODE": "test"})
            self.assertEqual(capture_call.call_args.kwargs["output"], folder / "retention")
            self.assertTrue(callable(capture_call.call_args.kwargs["validate_sample"]))
            writer.assert_called_once_with(folder / "memory-retention.json", capture=observed,
                                           expected=expected)
            self.assertEqual(result, (reference, transport))

    def test_policy_rejects_a_changed_retention_pin_before_evaluation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            record_path = Path(temporary) / "retention.json"
            record_path.write_text(json.dumps({"retainedRssBytes": [100, 110]}))
            expected: gate.JsonObject = {"memoryRetentionRef": {
                "path": str(record_path), "sha256": "0" * 64,
            }}
            with patch.object(memory, "evaluate_paired") as paired, \
                 patch.object(memory, "evaluate_candidate_only") as candidate_only:
                with self.assertRaisesRegex(ValueError, "retention observation changed"):
                    memory.evaluate_retention_record([], expected, {"comparisonStatus": "paired"})
            paired.assert_not_called()
            candidate_only.assert_not_called()

    def test_policy_routes_paired_and_noncomparable_results_to_matching_evaluator(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            record_path = Path(temporary) / "retention.json"
            record_path.write_text(json.dumps({"retainedRssBytes": [100, 110]}))
            expected: gate.JsonObject = {"memoryRetentionRef": {
                "path": str(record_path), "sha256": gate.sha256(record_path),
            }}
            reports = [Path(temporary) / name for name in ("candidate-0.json", "baseline-0.json")]
            paired_result: gate.JsonObject = {"comparisonStatus": "paired"}
            candidate_result: gate.JsonObject = {"comparisonStatus": "nonComparable"}
            paired_policy: gate.JsonObject = {"policyPass": True}
            candidate_policy: gate.JsonObject = {"policyPass": True}
            with patch.object(memory, "evaluate_paired", return_value=paired_policy) as paired, \
                 patch.object(memory, "evaluate_candidate_only", return_value=candidate_policy) as candidate_only:
                self.assertIs(memory.evaluate_retention_record(reports, expected, paired_result), paired_policy)
                self.assertIs(memory.evaluate_retention_record(reports, expected, candidate_result), candidate_policy)

            retained = [100.0, 110.0]
            paired.assert_called_once_with(reports, expected, retained, policy_version=memory_policy.POLICY_VERSION)
            candidate_only.assert_called_once_with(reports[:1], expected, retained, policy_version=memory_policy.POLICY_VERSION)


if __name__ == "__main__":
    unittest.main()
