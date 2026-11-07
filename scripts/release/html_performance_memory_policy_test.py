import importlib.metadata
import math
import unittest
from collections.abc import Sequence
from types import SimpleNamespace
from typing import cast
from unittest.mock import patch

import html_performance_memory_policy as policy


def _series(value: float) -> list[float]:
    return [value] * policy.SAMPLE_COUNT


class MemoryPolicyTest(unittest.TestCase):
    def test_small_peak_requires_both_bounds_and_no_time_regression(self) -> None:
        cases = [
            (20 * policy.MIB, policy.MIB, True),
            (10 * policy.MIB, policy.MIB, False),
            (40 * policy.MIB, policy.MIB + 1, False),
            (20 * policy.MIB, -policy.MIB, True),
        ]
        for baseline, increase, expected in cases:
            with self.subTest(baseline=baseline, increase=increase):
                result = policy.evaluate_peak_phase(
                    _series(baseline), _series(baseline + increase), _series(20), _series(20),
                )
                self.assertEqual(result.peak_allowed, expected)
                self.assertEqual(result.category, "small" if expected else "rejected")
        regression = policy.evaluate_peak_phase(
            _series(100 * policy.MIB), _series(100 * policy.MIB), _series(20), _series(25),
        )
        self.assertTrue(regression.time_regression.detected_bad)
        self.assertFalse(regression.peak_allowed)

    def test_large_peak_requires_both_limits_and_significant_speedup(self) -> None:
        cases = [
            (40 * policy.MIB, 8 * policy.MIB, 20, 18, True),
            (32 * policy.MIB, 8 * policy.MIB, 20, 18, True),
            (20 * policy.MIB, 6 * policy.MIB, 20, 18, False),
            (100 * policy.MIB, 8 * policy.MIB + 1, 20, 18, False),
            (40 * policy.MIB, 2 * policy.MIB, 20, 19, False),
            (40 * policy.MIB, 2 * policy.MIB, 5, 4.5, False),
        ]
        for base, increase, original, candidate, expected in cases:
            with self.subTest(base=base, increase=increase, candidate=candidate):
                result = policy.evaluate_peak_phase(
                    _series(base), _series(base + increase), _series(original), _series(candidate),
                )
                self.assertEqual(result.peak_allowed, expected)
                self.assertEqual(result.category, "tradeoff" if expected else "rejected")

    def test_maximum_and_p95_are_not_hidden_by_the_median(self) -> None:
        baseline = _series(100 * policy.MIB)
        for changed in (1, 5):
            with self.subTest(changed=changed):
                candidate = baseline.copy()
                candidate[-changed:] = [110 * policy.MIB] * changed
                result = policy.evaluate_peak_phase(baseline, candidate, _series(20), _series(20))
                self.assertEqual(result.median.delta_bytes, 0)
                self.assertGreater(result.maximum.delta_bytes, policy.MAX_PEAK_BYTES)
                self.assertFalse(result.peak_allowed)
                if changed == 5:
                    self.assertGreater(result.p95.delta_bytes, policy.MAX_PEAK_BYTES)

    def test_zero_duration_is_accepted_only_after_external_phase_validation(self) -> None:
        rss = _series(100 * policy.MIB)
        equal = policy.evaluate_peak_phase(rss, rss, _series(0), _series(0))
        self.assertEqual(equal.median_speedup_ms, 0)
        self.assertEqual(equal.median_speedup_ratio, 0)
        self.assertTrue(equal.peak_allowed)
        slower = policy.evaluate_peak_phase(rss, rss, _series(0), _series(1))
        self.assertEqual(slower.median_speedup_ratio, 0)
        self.assertTrue(slower.time_regression.detected_bad)
        self.assertFalse(slower.peak_allowed)

    def test_finite_inputs_cannot_produce_nonfinite_comparison_results(self) -> None:
        with self.assertRaisesRegex(ValueError, "peak RSS comparison is not finite"):
            policy.evaluate_peak_phase(
                _series(1e-300), _series(1e300), _series(1), _series(1),
            )
        with self.assertRaisesRegex(ValueError, "elapsed comparison is not finite"):
            policy.evaluate_peak_phase(
                _series(1), _series(1), _series(1e-300), _series(1e300),
            )

    def test_retention_distinguishes_bounded_warmup_and_sustained_growth(self) -> None:
        base = 100 * policy.MIB
        bounded = [base + min(index, 5) * 50_000 for index in range(25)]
        trend = [base + index * 10_000 for index in range(25)]
        oversize = [base] * 20 + [base + 2 * policy.MIB] * 5
        cases: list[tuple[Sequence[float], bool]] = [
            (_series(base), True), (bounded, True), (trend, False), (oversize, False),
        ]
        for values, expected in cases:
            with self.subTest(values=values):
                self.assertEqual(policy.evaluate_retention(values).retention_allowed, expected)
        sustained = policy.evaluate_retention(trend)
        self.assertEqual(sustained.version, policy.POLICY_VERSION)
        self.assertEqual(sustained.trend_family_comparisons, 168)
        self.assertEqual(sustained.trend_family_alpha, 0.05)
        self.assertEqual(sustained.trend_alpha, 0.05 / 168)
        self.assertTrue(sustained.overall_trend.significant_upward)
        self.assertTrue(sustained.final_trend.significant_upward)
        self.assertTrue(sustained.final_window_increase)

    def test_retention_applies_smaller_of_absolute_and_relative_limits(self) -> None:
        for base, increase in [(10 * policy.MIB, 0.5 * policy.MIB),
                               (100 * policy.MIB, policy.MIB)]:
            with self.subTest(base=base):
                boundary = [base] * 15 + [base + increase] * 10
                exceeded = [base] * 15 + [base + increase + 1] * 10
                self.assertTrue(policy.evaluate_retention(boundary).retention_allowed)
                self.assertFalse(policy.evaluate_retention(exceeded).retention_allowed)

    def test_invalid_samples_are_rejected(self) -> None:
        cases: list[list[float]] = [
            [1.0], [math.nan] * 25, [math.inf] * 25, [0.0] * 25,
            [-1.0] * 25, [True] * 25, [10**400] * 25,
        ]
        for values in cases:
            with self.subTest(values=values[:1]), self.assertRaises(ValueError):
                policy.evaluate_retention(values)
        with self.assertRaises(ValueError):
            policy.evaluate_peak_phase(_series(1), _series(1), _series(1), [1])
        with self.assertRaises(ValueError):
            policy.evaluate_retention(cast(Sequence[float], ["invalid"] * 25))

    def test_trend_requires_supported_scipy_and_finite_result(self) -> None:
        with patch.object(importlib.metadata, "version", return_value="0"):
            with self.assertRaises(RuntimeError):
                policy.evaluate_retention([float(i + 1) for i in range(25)])
        invalid = SimpleNamespace(statistic=math.nan, pvalue=math.nan)
        with patch("scipy.stats.kendalltau", return_value=invalid):
            with self.assertRaises(ValueError):
                policy.evaluate_retention([float(i + 1) for i in range(25)])


if __name__ == "__main__":
    unittest.main()
