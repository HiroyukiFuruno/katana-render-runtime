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


def _legacy_peak(baseline: Sequence[float], candidate: Sequence[float],
                 original: Sequence[float], elapsed: Sequence[float]) -> policy.PeakPhaseResult:
    return policy.evaluate_peak_phase(baseline, candidate, original, elapsed,
                                      policy_version=policy.LEGACY_POLICY_VERSION)


def _legacy_retention(values: Sequence[float]) -> policy.RetentionResult:
    return policy.evaluate_retention(values, policy_version=policy.LEGACY_POLICY_VERSION)


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
                result = _legacy_peak(
                    _series(baseline), _series(baseline + increase), _series(20), _series(20),
                )
                self.assertEqual(result.peak_allowed, expected)
                self.assertEqual(result.category, "small" if expected else "rejected")
        regression = _legacy_peak(
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
                result = _legacy_peak(
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
                result = _legacy_peak(baseline, candidate, _series(20), _series(20))
                self.assertEqual(result.median.delta_bytes, 0)
                self.assertGreater(result.maximum.delta_bytes, policy.MAX_PEAK_BYTES)
                self.assertFalse(result.peak_allowed)
                if changed == 5:
                    self.assertGreater(result.p95.delta_bytes, policy.MAX_PEAK_BYTES)

    def test_zero_duration_is_accepted_only_after_external_phase_validation(self) -> None:
        rss = _series(100 * policy.MIB)
        equal = _legacy_peak(rss, rss, _series(0), _series(0))
        self.assertEqual(equal.median_speedup_ms, 0)
        self.assertEqual(equal.median_speedup_ratio, 0)
        self.assertTrue(equal.peak_allowed)
        slower = _legacy_peak(rss, rss, _series(0), _series(1))
        self.assertEqual(slower.median_speedup_ratio, 0)
        self.assertTrue(slower.time_regression.detected_bad)
        self.assertFalse(slower.peak_allowed)

    def test_finite_inputs_cannot_produce_nonfinite_comparison_results(self) -> None:
        with self.assertRaisesRegex(ValueError, "peak RSS comparison is not finite"):
            _legacy_peak(
                _series(1e-300), _series(1e300), _series(1), _series(1),
            )
        with self.assertRaisesRegex(ValueError, "elapsed comparison is not finite"):
            _legacy_peak(
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
                self.assertEqual(_legacy_retention(values).retention_allowed, expected)
        sustained = _legacy_retention(trend)
        self.assertEqual(sustained.version, policy.LEGACY_POLICY_VERSION)
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
                self.assertTrue(_legacy_retention(boundary).retention_allowed)
                self.assertFalse(_legacy_retention(exceeded).retention_allowed)

    def test_invalid_samples_are_rejected(self) -> None:
        cases: list[list[float]] = [
            [1.0], [math.nan] * 25, [math.inf] * 25, [0.0] * 25,
            [-1.0] * 25, [True] * 25, [10**400] * 25,
        ]
        for values in cases:
            with self.subTest(values=values[:1]), self.assertRaises(ValueError):
                _legacy_retention(values)
        with self.assertRaises(ValueError):
            _legacy_peak(_series(1), _series(1), _series(1), [1])
        with self.assertRaises(ValueError):
            _legacy_retention(cast(Sequence[float], ["invalid"] * 25))

    def test_trend_requires_supported_scipy_and_finite_result(self) -> None:
        with patch.object(importlib.metadata, "version", return_value="0"):
            with self.assertRaises(RuntimeError):
                _legacy_retention([float(i + 1) for i in range(25)])
        invalid = SimpleNamespace(statistic=math.nan, pvalue=math.nan)
        with patch("scipy.stats.kendalltau", return_value=invalid):
            with self.assertRaises(ValueError):
                _legacy_retention([float(i + 1) for i in range(25)])


class MemoryPolicyV3Test(unittest.TestCase):
    def test_v3_accepts_bounded_observed_retention_steps(self) -> None:
        cases = [
            [49_905_664] * 6 + [50_823_168] * 19,
            [44_875_776] * 22 + [49_184_768] * 3,
            [49_872_896] * 14 + [57_376_768] * 11,
        ]
        for values in cases:
            with self.subTest(first=values[0]):
                self.assertTrue(policy.evaluate_retention(values, policy_version="html-memory-v3").retention_allowed)
        for values in cases[1:]:
            with self.subTest(first=values[0]):
                self.assertFalse(policy.evaluate_retention(values, policy_version="html-memory-v2").retention_allowed)

    def test_v3_accepts_isolated_bounded_peak_without_speedup(self) -> None:
        baseline = [50_003_968.0] * 24 + [51_920_896.0]
        candidate = [50_085_888.0] * 24 + [53_985_280.0]
        arguments = (baseline, candidate, _series(20), _series(20))
        self.assertFalse(policy.evaluate_peak_phase(*arguments, policy_version="html-memory-v2").peak_allowed)
        self.assertTrue(policy.evaluate_peak_phase(*arguments, policy_version="html-memory-v3").peak_allowed)

    def test_v3_rejects_growth_bad_time_and_peak_bounds(self) -> None:
        sustained = [100 * policy.MIB + index * 10_000 for index in range(25)]
        self.assertFalse(policy.evaluate_retention(sustained, policy_version="html-memory-v3").retention_allowed)
        for base, increase in ((100 * policy.MIB, 8 * policy.MIB + 1),
                               (20 * policy.MIB, 5 * policy.MIB + 1)):
            candidate = _series(base)
            candidate[-1] += increase
            with self.subTest(base=base):
                self.assertFalse(policy.evaluate_peak_phase(
                    _series(base), candidate, _series(20), _series(20),
                    policy_version="html-memory-v3",
                ).peak_allowed)
        self.assertFalse(policy.evaluate_peak_phase(
            _series(100 * policy.MIB), _series(100 * policy.MIB), _series(20), _series(25),
            policy_version="html-memory-v3",
        ).peak_allowed)

    def test_v3_central_increase_requires_speedup_and_retention_limits_both_bounds(self) -> None:
        base = 40 * policy.MIB
        for elapsed, expected in ((20, False), (19, False), (18, True)):
            with self.subTest(elapsed=elapsed):
                result = policy.evaluate_peak_phase(
                    _series(base), _series(base + 2 * policy.MIB), _series(20), _series(elapsed),
                )
                self.assertEqual(result.peak_allowed, expected)
        for start, increase in ((20 * policy.MIB, 5 * policy.MIB),
                                (40 * policy.MIB, 8 * policy.MIB)):
            with self.subTest(start=start):
                self.assertTrue(policy.evaluate_retention([start] * 15 + [start + increase] * 10).retention_allowed)
                self.assertFalse(policy.evaluate_retention(
                    [start] * 15 + [start + increase + 1] * 10,
                ).retention_allowed)

    def test_unknown_policy_version_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            policy.evaluate_retention(_series(100), policy_version="unknown")
        with self.assertRaises(ValueError):
            policy.evaluate_peak_phase(_series(100), _series(100), _series(1), _series(1),
                                       policy_version="unknown")


if __name__ == "__main__":
    unittest.main()
