from __future__ import annotations

import unittest
from typing import cast

import html_performance_same_binary_diagnostic as diagnostic
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate


class EnvironmentComparisonTests(unittest.TestCase):
    def test_reports_key_and_digest_differences_without_environment_values(self) -> None:
        old: gate.JsonObject = {
            "schema": "krr-html-worker-runtime-execution-v1",
            "keyNames": ["A", "B"], "keyNamesSha256": "a" * 64,
            "nonMetadataEnvironmentSha256": "b" * 64,
        }
        actual: gate.JsonObject = {"keyNames": ["A", "C"], "keyNamesSha256": "c" * 64,
                                   "nonMetadataEnvironmentSha256": "d" * 64}
        result = diagnostic.environment_comparison([dict(old) for _ in range(50)], actual)
        self.assertEqual(result["environmentKeyNamesAdded"], ["C"])
        self.assertEqual(result["environmentKeyNamesRemoved"], ["B"])
        self.assertIs(result["originalEnvironmentValuesAvailable"], False)
        self.assertNotIn("environment", result)

    def test_missing_or_inconsistent_observations_reject(self) -> None:
        old: gate.JsonObject = {
            "schema": "krr-html-worker-runtime-execution-v1",
            "keyNames": ["A"], "keyNamesSha256": "a" * 64,
            "nonMetadataEnvironmentSha256": "b" * 64,
        }
        with self.assertRaisesRegex(ValueError, "50 worker"):
            diagnostic.environment_comparison([], old)
        different = {**old, "nonMetadataEnvironmentSha256": "c" * 64}
        with self.assertRaisesRegex(ValueError, "environments differ"):
            diagnostic.environment_comparison([old] * 49 + [different], old)

    def test_rejects_invalid_digest_even_if_all_observations_match(self) -> None:
        valid: gate.JsonObject = {
            "schema": "krr-html-worker-runtime-execution-v1", "keyNames": ["A"],
            "keyNamesSha256": "a" * 64, "nonMetadataEnvironmentSha256": "b" * 64,
        }
        for key in ("keyNamesSha256", "nonMetadataEnvironmentSha256"):
            invalid = {**valid, key: "g" * 64}
            with self.subTest(key=key), self.assertRaises(ValueError):
                diagnostic.environment_comparison([invalid] * 50, valid)
            with self.subTest(actual=key), self.assertRaises(ValueError):
                diagnostic.environment_comparison([valid] * 50, invalid)


class LegacyEnvironmentComparisonTests(unittest.TestCase):
    def test_marks_a_v2_observation_without_runtime_execution_as_not_established(self) -> None:
        result = diagnostic.legacy_environment_comparison(
            producer.BATCH_RECEIPT_SCHEMA_V2, [{} for _ in range(50)],
        )

        self.assertEqual(result["originalRuntimeExecution"], "unavailable-legacy-v2")
        self.assertEqual(result["environmentComparability"], "not-established")
        self.assertIs(result["originalEnvironmentValuesAvailable"], False)

    def test_rejects_mixed_or_incomplete_legacy_observations(self) -> None:
        with self.assertRaisesRegex(ValueError, "50 worker"):
            diagnostic.legacy_environment_comparison(
                producer.BATCH_RECEIPT_SCHEMA_V2, [{} for _ in range(49)],
            )
        for runtime in (None, cast(gate.JsonObject, {})):
            mixed: list[gate.JsonObject] = [{} for _ in range(49)] + [{"runtimeExecution": runtime}]
            with self.subTest(runtime=runtime), self.assertRaisesRegex(ValueError, "must omit runtime execution"):
                diagnostic.legacy_environment_comparison(producer.BATCH_RECEIPT_SCHEMA_V2, mixed)
        with self.assertRaisesRegex(ValueError, "requires a v2 receipt"):
            diagnostic.legacy_environment_comparison(
                producer.BATCH_RECEIPT_SCHEMA, [{} for _ in range(50)],
            )


if __name__ == "__main__":
    unittest.main()
