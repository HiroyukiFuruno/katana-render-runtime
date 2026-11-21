from __future__ import annotations

import unittest

import html_performance_same_binary_diagnostic as diagnostic
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


if __name__ == "__main__":
    unittest.main()
