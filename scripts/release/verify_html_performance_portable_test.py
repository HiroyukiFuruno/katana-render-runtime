from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import verify_html_performance_evidence as gate
import verify_html_performance_portable as portable
import html_performance_memory_evidence as memory_evidence


class PortableVerificationTest(unittest.TestCase):
    def test_relocated_bundle_and_modified_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source"
            source.mkdir()
            files: list[dict[str, str]] = []

            def add(relative: str, value: object) -> dict[str, str]:
                path = source / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                raw = json.dumps(value).encode()
                path.write_bytes(raw)
                sha = hashlib.sha256(raw).hexdigest()
                original = "/original/" + relative
                files.append({"originalPath": original, "relativePath": relative, "sha256": sha})
                return {"path": original, "sha256": sha}

            viewport = {"width": 100, "height": 100, "scale": 1}
            fixture_sha = "a" * 64
            conditions = [{"conditionId": str(i), "caseId": "case", "state": "ready",
                           "operationKind": "static", "viewport": viewport, "sourceSha256": fixture_sha}
                          for i in range(84)]
            catalog_ref = add("catalog.json", {"profile": "krr-html84-condition-plan-v1",
                                               "acceptance": False, "conditions": conditions})
            plan_ref = add("plan.json", {"operationStream": []})
            evidence_ref = add("operation.json", {})
            attempts = []
            for i in range(84):
                pins: list[dict[str, str]] = []
                order = []
                report_paths = []
                condition = {"condition_id": str(i), "case_id": "case", "state": "ready",
                             "operation_kind": "static", "viewport": viewport, "fixture_sha256": fixture_sha}
                provenance = {"rustc": "rustc", "cargo": "cargo", "environment_sha256": "b" * 64}
                arm_data = {"binary_sha256": "c" * 64, "source_snapshot_sha256": "d" * 64,
                            "lock_sha256": "e" * 64}
                expected = {"catalogCondition": condition, "viewport": viewport, "fixture_sha256": fixture_sha,
                            "catalogPlanRef": catalog_ref, "operationPlanRef": plan_ref,
                            "operationEvidenceRef": evidence_ref, "reportPins": pins,
                            "operation_plan_sha256": plan_ref["sha256"], "operation": "static",
                            "expected_dom_state_sha256": "f" * 64, "provenance": provenance,
                            "arms": {gate.DEFAULT_BASELINE_ARM_ID: arm_data, "candidate": arm_data},
                            "memoryRetentionRef": {"path": "/retention", "sha256": "1" * 64}}
                for pair in range(5):
                    arms = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
                    for arm in arms:
                        samples = []
                        for sample in range(5):
                            frame = {"viewport": viewport, "pixel_bytes": 40000}
                            phases = {phase: {"elapsed_ms": 10, "peak_rss_bytes": 100, "frame": dict(frame)}
                                      for phase in gate.PHASES}
                            phases["full_y"].update({"actual_bottom": True, "scroll_steps": 3,
                                                     "max_scroll": 1000, "final_scroll_y": 1000,
                                                     "max_frame_elapsed_ms": 10})
                            gate._object(phases["full_y"]["frame"], "fixture frame")["scroll_y"] = 1000
                            phases["post_operation"] = {"measurement": phases["post_operation"],
                                "catalog_operation": {"condition_id": str(i), "case_id": "case", "state": "ready",
                                                      "operation_kind": "static", "operation_evidence_sha256": evidence_ref["sha256"],
                                                      "delivery": "measured-close-and-new-session-open"}}
                            resized = {"width": 75, "height": 100, "scale": 1}
                            phases["same_session_resize"] = {"measurement": {"elapsed_ms": 10, "peak_rss_bytes": 100,
                                "frame": {"viewport": resized, "pixel_bytes": 30000}},
                                "resize_evidence": {"from_viewport": viewport, "to_viewport": resized,
                                                    "generation_before": 2, "generation_after": 3,
                                                    "real_raster_frame": True}}
                            samples.append({"sample": sample, "viewport": viewport, "phases": phases,
                                "catalog_condition": {"profile": "catalog-plan84", "condition_id": str(i),
                                    "case_id": "case", "state": "ready", "operation_kind": "static",
                                    "operation_evidence_sha256": evidence_ref["sha256"]}})
                        raw_arm = gate.DEFAULT_BASELINE_ARM_ID if arm == "baseline" else arm
                        report = {"arm": raw_arm, "measurement_executed": True, "pair_index": pair,
                                  "source_sha256": fixture_sha, "viewport": viewport,
                                  "operation_plan_sha256": plan_ref["sha256"], "operation": "static",
                                  "expected_dom_state_sha256": "f" * 64,
                                  "provenance": {**provenance, "binary_sha256": "c" * 64,
                                                 "source_snapshot_sha256": "d" * 64, "deps_lock_sha256": "e" * 64},
                                  "sample_count": 5, "samples": samples, "condition_id": str(i),
                                  "case_id": "case", "state": "ready", "operation_kind": "static",
                                  "operation_evidence_ref": evidence_ref, "catalog_condition": condition}
                        pin = add(f"{i}/{arm}-{pair}.json", report)
                        pins.append(pin)
                        report_paths.append(source / f"{i}/{arm}-{pair}.json")
                        order.append({"arm": arm, "pair_index": pair, "output": pin["path"]})
                policy = gate.evaluate_measurement_policy(report_paths, expected)
                memory_policy = memory_evidence.evaluate_paired(report_paths, expected, [100.0] * 25)
                attempts.append({"expected": add(f"{i}/expected.json", expected),
                                 "result": add(f"{i}/result.json", {"comparisonStatus": "paired",
                                                                        "policy": policy, "memoryPolicy": memory_policy}),
                                 "pairManifest": add(f"{i}/pairs.json", {"measurement_executed": True,
                                                                               "order": order})})
            raw = json.dumps({"schema": "krr-html-performance-release-bundle-v1", "files": files,
                              "attempts": attempts}).encode()
            (source / "bundle.json").write_bytes(raw)
            checksum = hashlib.sha256(raw).hexdigest()
            relocated = Path(temporary) / "relocated"
            shutil.move(source, relocated)
            with patch("html_performance_retention_evidence.read_retained_rss", return_value=[100.0] * 25):
                result = portable.verify_portable(relocated, checksum)
                self.assertEqual(result["pairedConditions"], 84)
                self.assertFalse(result["formalReleaseAcceptance"])
                (relocated / "0/candidate-0.json").write_bytes(b"tampered")
                with self.assertRaisesRegex(ValueError, "transport file hash mismatch"):
                    portable.verify_portable(relocated, checksum)

    def test_missing_conditions_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            payload = b'{"schema":"test"}'
            (directory / "bundle.json").write_bytes(payload)
            checksum = hashlib.sha256(payload).hexdigest()
            with patch.object(gate, "Bundle") as bundle:
                bundle.return_value.manifest = {"attempts": []}
                with self.assertRaisesRegex(ValueError, "84 catalog"):
                    portable.verify_portable(directory, checksum)

    def test_wrong_hash_never_calls_formal_gate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "bundle.json").write_bytes(b"changed")
            with patch.object(gate, "Bundle") as bundle:
                with self.assertRaisesRegex(ValueError, "trusted manifest"):
                    portable.verify_portable(directory, "0" * 64)
                bundle.assert_not_called()


if __name__ == "__main__":
    unittest.main()
