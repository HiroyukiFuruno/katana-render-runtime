import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_paired_stats as stats
import verify_html_performance_evidence as gate


def _phase_values(
    fixture: gate.JsonObject, arm: str, phase: str, metric: str
) -> list[float]:
    arms = gate._object(fixture["arms"], "fixture arms")
    arm_value = gate._object(arms[arm], f"{arm} arm")
    phase_value = gate._object(arm_value[phase], f"{phase} phase")
    values = gate._array(phase_value[metric], f"{metric} values")
    return [float(gate._numeric(value, f"{phase}.{metric}")) for value in values]


class HtmlPerformancePairedStatsTest(unittest.TestCase):
    def test_r3283_noise_fixture_has_no_directional_regression(self) -> None:
        fixture = gate.read_json(Path(__file__).with_name("html-identical-worker-noise.json"))
        phases = (
            "cold_open",
            "warm_open",
            "full_y",
            "post_operation",
            "re_display",
            "same_session_resize",
        )
        for phase in phases:
            for metric in ("elapsed_ms", "peak_rss_bytes"):
                baseline = _phase_values(fixture, "baseline", phase, metric)
                candidate = _phase_values(fixture, "candidate", phase, metric)
                self.assertFalse(stats.evaluate_paired_regression(baseline, candidate).detected_bad)
                self.assertFalse(stats.evaluate_paired_regression(candidate, baseline).detected_bad)

    def test_equal_vectors_are_explicitly_not_tested(self) -> None:
        values = [float(index) for index in range(25)]
        result = stats.evaluate_paired_regression(values, values.copy())
        self.assertFalse(result.test_performed)
        self.assertIsNone(result.p_value)
        self.assertFalse(result.detected_bad)
        self.assertIn("identical", result.reason or "")

    def test_doubled_real_worker_time_and_memory_are_detected(self) -> None:
        fixture = gate.read_json(Path(__file__).with_name("html-identical-worker-noise.json"))
        for metric in ("elapsed_ms", "peak_rss_bytes"):
            baseline = _phase_values(fixture, "baseline", "re_display", metric)
            result = stats.evaluate_paired_regression(
                baseline, [value * 2 for value in baseline]
            )
            self.assertTrue(result.detected_bad, metric)

    def test_equal_vectors_pass_in_both_arm_directions(self) -> None:
        values = [float(index) for index in range(25)]
        for baseline, candidate in ((values, values.copy()), (values.copy(), values)):
            result = stats.evaluate_paired_regression(baseline, candidate)
            self.assertFalse(result.detected_bad)

    def test_large_positive_shift_is_detected(self) -> None:
        baseline = [float(index + 1) for index in range(25)]
        candidate = [value * 2.0 for value in baseline]
        result = stats.evaluate_paired_regression(baseline, candidate)
        self.assertTrue(result.test_performed)
        self.assertTrue(result.detected_bad)

    def test_negative_shift_is_not_detected(self) -> None:
        baseline = [float(index + 1) for index in range(25)]
        candidate = [value * 0.5 for value in baseline]
        result = stats.evaluate_paired_regression(baseline, candidate)
        self.assertTrue(result.test_performed)
        self.assertFalse(result.detected_bad)

    def test_ties_are_handled_by_scipy(self) -> None:
        baseline = [float(index // 2 + 1) for index in range(25)]
        candidate = [value + 1.0 for value in baseline]
        result = stats.evaluate_paired_regression(baseline, candidate)
        self.assertTrue(result.test_performed)
        self.assertIsNotNone(result.p_value)

    def test_invalid_length_and_nonfinite_values_fail(self) -> None:
        with self.assertRaises(ValueError):
            stats.evaluate_paired_regression([1.0], [1.0])
        with self.assertRaises(ValueError):
            stats.evaluate_paired_regression([float("nan")] * 25, [1.0] * 25)

    def test_invalid_scipy_result_is_rejected(self) -> None:
        values = [float(index + 1) for index in range(25)]
        with patch.object(stats, "_run_wilcoxon", return_value=(float("nan"), 1.0)):
            with self.assertRaises(ValueError):
                stats.evaluate_paired_regression(values, [value * 2.0 for value in values])


if __name__ == "__main__":
    unittest.main()
