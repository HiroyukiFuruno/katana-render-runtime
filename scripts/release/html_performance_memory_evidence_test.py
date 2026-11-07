import json
import unittest
from pathlib import Path

import html_performance_memory_evidence as evidence
import html_performance_memory_policy as policy
import verify_html_performance_evidence as gate
import verify_html_performance_evidence_test as fixture_module


class MemoryEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = fixture_module.PerformanceEvidenceTest("runTest")
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.setUp()
        self.paths = self.fixture.report_paths[0]
        self.expected = gate.read_json(self.fixture.expected_paths[0])
        self.retained = [100.0] * 25

    def test_paired_recomputes_v1_without_changing_raw_reports(self) -> None:
        before = {path: path.read_bytes() for path in self.paths}
        result = evidence.evaluate_paired(self.paths, self.expected, self.retained)
        self.assertEqual(result["v1Policy"], gate.evaluate_measurement_policy(self.paths, self.expected))
        self.assertEqual(result["policyPass"], True)
        self.assertEqual(result["formalAccepted"], False)
        self.assertEqual(set(gate._object(result["phases"], "phases")), set(gate.PHASES))
        self.assertEqual(before, {path: path.read_bytes() for path in self.paths})

    def test_explicit_v2_and_v3_keep_v1_and_apply_own_retention_budget(self) -> None:
        retained = [40 * policy.MIB] * 15 + [44 * policy.MIB] * 10
        legacy = evidence.evaluate_paired(self.paths, self.expected, retained,
                                          policy_version=policy.LEGACY_POLICY_VERSION)
        current = evidence.evaluate_paired(self.paths, self.expected, retained)
        self.assertEqual(legacy["version"], policy.LEGACY_POLICY_VERSION)
        self.assertEqual(current["version"], policy.POLICY_VERSION)
        self.assertEqual(legacy["v1Policy"], current["v1Policy"])
        self.assertFalse(legacy["policyPass"])
        self.assertTrue(current["policyPass"])
        with self.assertRaisesRegex(ValueError, "unsupported memory policy version"):
            evidence.evaluate_paired(self.paths, self.expected, retained, policy_version="unknown")

    def test_paired_rejects_missing_or_unexecuted_reports(self) -> None:
        with self.assertRaises(ValueError):
            evidence.evaluate_paired(self.paths[:-1], self.expected, self.retained)
        candidate = next(path for path in self.paths if path.name == "candidate-0.json")
        report = gate.read_json(candidate)
        report["measurement_executed"] = False
        candidate.write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError, "actual measurements absent"):
            evidence.evaluate_paired(self.paths, self.expected, self.retained)

    def test_v1_rss_failure_can_have_separate_v2_small_peak_result(self) -> None:
        for path in self.paths:
            if path.name.startswith("candidate-"):
                report = gate.read_json(path)
                for sample in gate._objects(report["samples"], "samples"):
                    phases = gate._object(sample["phases"], "phases")
                    gate._object(phases["cold_open"], "cold")['peak_rss_bytes'] = 101
                path.write_text(json.dumps(report))
        result = evidence.evaluate_paired(self.paths, self.expected, self.retained)
        self.assertFalse(gate._object(result["v1Policy"], "v1")["policyPass"])
        self.assertTrue(result["policyPass"])
        self.assertFalse(result["formalAccepted"])

    def candidate_fixture(self) -> list[Path]:
        _, self.expected, _, _, _ = self.fixture._noncomparable_baseline_fixture("memory")
        identity = gate._object(self.expected["catalogCondition"], "catalog")
        reference = gate._object(self.expected["operationEvidenceRef"], "operation evidence")
        candidates = [path for path in self.paths if path.name.startswith("candidate-")]
        for path in candidates:
            report = gate.read_json(path)
            report.update(
                condition_id=identity["condition_id"], case_id=identity["case_id"],
                state=identity["state"], operation_kind=identity["operation_kind"],
                operation_evidence_ref=reference, catalog_condition=identity,
            )
            for sample in gate._objects(report["samples"], "samples"):
                sample["catalog_condition"] = {
                    "profile": "catalog-plan84", "condition_id": identity["condition_id"],
                    "case_id": identity["case_id"], "state": identity["state"],
                    "operation_kind": identity["operation_kind"],
                    "operation_evidence_sha256": reference["sha256"],
                }
            path.write_text(json.dumps(report))
        return candidates

    def test_candidate_only_never_accepts_an_absent_baseline(self) -> None:
        candidates = self.candidate_fixture()
        result = evidence.evaluate_candidate_only(candidates, self.expected, self.retained)
        self.assertTrue(result["candidateRetentionPass"])
        self.assertFalse(result["policyPass"])
        self.assertFalse(result["formalAccepted"])
        self.assertEqual(result["candidateSamples"], 25)
        legacy = evidence.evaluate_candidate_only(candidates, self.expected, self.retained,
                                                   policy_version=policy.LEGACY_POLICY_VERSION)
        self.assertEqual(legacy["version"], policy.LEGACY_POLICY_VERSION)
        self.assertFalse(legacy["formalAccepted"])
        with self.assertRaisesRegex(ValueError, "unsupported memory policy version"):
            evidence.evaluate_candidate_only(candidates, self.expected, self.retained,
                                             policy_version="unknown")
        with self.assertRaises(ValueError):
            evidence.evaluate_candidate_only(candidates[:-1], self.expected, self.retained)
        with self.assertRaises(ValueError):
            evidence.evaluate_candidate_only(candidates[:4] + candidates[:1], self.expected, self.retained)

    def test_candidate_only_checks_known_condition_and_real_report_metadata(self) -> None:
        candidates = self.candidate_fixture()
        catalog = gate._object(self.expected["catalogCondition"], "catalog")
        catalog["condition_id"] = "unknown"
        with self.assertRaises(ValueError):
            evidence.evaluate_candidate_only(candidates, self.expected, self.retained)
        catalog["condition_id"] = "navigation/top/1280x900@1"
        report = gate.read_json(candidates[0])
        report["measurement_executed"] = False
        candidates[0].write_text(json.dumps(report))
        with self.assertRaises(ValueError):
            evidence.evaluate_candidate_only(candidates, self.expected, self.retained)
