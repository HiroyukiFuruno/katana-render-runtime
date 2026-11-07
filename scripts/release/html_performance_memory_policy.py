"""Evaluate v2 peak and retained RSS observations without accepting evidence."""

from __future__ import annotations

import importlib.metadata
import math
import statistics
from collections.abc import Sequence
from dataclasses import dataclass

from html_performance_paired_stats import PairedRegressionResult, evaluate_paired_regression

POLICY_VERSION = "html-memory-v2"
SAMPLE_COUNT = 25
MIB = 1024 * 1024
SMALL_PEAK_BYTES = MIB
SMALL_PEAK_RATIO = 0.05
MAX_PEAK_BYTES = 8 * MIB
MAX_PEAK_RATIO = 0.25
MIN_SPEEDUP_RATIO = 0.10
MIN_SPEEDUP_MS = 1.0
RETENTION_BYTES = MIB
RETENTION_RATIO = 0.05
TREND_FAMILY_COMPARISONS = 84 * 2
TREND_FAMILY_ALPHA = 0.05
TREND_ALPHA = TREND_FAMILY_ALPHA / TREND_FAMILY_COMPARISONS
SCIPY_VERSION = "1.18.1"


@dataclass(frozen=True)
class PeakDelta:
    baseline: float
    candidate: float
    delta_bytes: float
    delta_ratio: float


@dataclass(frozen=True)
class PeakPhaseResult:
    version: str
    median: PeakDelta
    p95: PeakDelta
    maximum: PeakDelta
    time_regression: PairedRegressionResult
    speed_improvement: PairedRegressionResult
    median_speedup_ms: float
    median_speedup_ratio: float
    category: str
    peak_allowed: bool


@dataclass(frozen=True)
class Trend:
    tau: float | None
    p_value: float | None
    significant_upward: bool


@dataclass(frozen=True)
class RetentionResult:
    version: str
    trend_family_comparisons: int
    trend_family_alpha: float
    trend_alpha: float
    initial_median: float
    final_median: float
    delta_bytes: float
    budget_bytes: float
    overall_trend: Trend
    final_trend: Trend
    final_window_increase: bool
    retention_allowed: bool


def _values(
    values: Sequence[float], label: str, *, allow_zero: bool = False
) -> tuple[float, ...]:
    if len(values) != SAMPLE_COUNT:
        raise ValueError(f"{label} must contain exactly {SAMPLE_COUNT} samples")
    if any(isinstance(value, bool) or not isinstance(value, (int, float)) for value in values):
        raise ValueError(f"{label} must contain numeric samples")
    try:
        result = tuple(float(value) for value in values)
    except OverflowError as error:
        raise ValueError(f"{label} samples are outside the valid range") from error
    if any(not math.isfinite(value) or value < 0 or (not allow_zero and value == 0)
           for value in result):
        raise ValueError(f"{label} samples are outside the valid range")
    return result


def _p95(values: Sequence[float]) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * 0.95
    lower = math.floor(position)
    upper = math.ceil(position)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def _delta(baseline: float, candidate: float) -> PeakDelta:
    difference = candidate - baseline
    ratio = difference / baseline
    if not math.isfinite(difference) or not math.isfinite(ratio):
        raise ValueError("peak RSS comparison is not finite")
    return PeakDelta(baseline, candidate, difference, ratio)


def _within(delta: PeakDelta, byte_limit: float, ratio_limit: float) -> bool:
    return delta.delta_bytes <= byte_limit and delta.delta_ratio <= ratio_limit


def evaluate_peak_phase(
    baseline_rss: Sequence[float],
    candidate_rss: Sequence[float],
    baseline_elapsed: Sequence[float],
    candidate_elapsed: Sequence[float],
) -> PeakPhaseResult:
    """Judge one phase; input provenance and cross-phase checks belong to the caller."""
    base_rss = _values(baseline_rss, "baseline RSS")
    cand_rss = _values(candidate_rss, "candidate RSS")
    # Zero-duration full-Y is admitted only when the outer validator proves zero scroll.
    base_time = _values(baseline_elapsed, "baseline elapsed", allow_zero=True)
    cand_time = _values(candidate_elapsed, "candidate elapsed", allow_zero=True)
    median = _delta(statistics.median(base_rss), statistics.median(cand_rss))
    p95 = _delta(_p95(base_rss), _p95(cand_rss))
    maximum = _delta(max(base_rss), max(cand_rss))
    differences = (median, p95, maximum)
    regression = evaluate_paired_regression(base_time, cand_time)
    improvement = evaluate_paired_regression(cand_time, base_time)
    base_median = statistics.median(base_time)
    speedup_ms = base_median - statistics.median(cand_time)
    speedup_ratio = speedup_ms / base_median if base_median > 0 else 0.0
    if not math.isfinite(speedup_ms) or not math.isfinite(speedup_ratio):
        raise ValueError("elapsed comparison is not finite")
    small = all(_within(item, SMALL_PEAK_BYTES, SMALL_PEAK_RATIO) for item in differences)
    bounded = all(_within(item, MAX_PEAK_BYTES, MAX_PEAK_RATIO) for item in differences)
    faster = (improvement.test_performed and improvement.detected_bad
              and speedup_ms >= MIN_SPEEDUP_MS and speedup_ratio >= MIN_SPEEDUP_RATIO)
    category = "small" if small else "tradeoff" if bounded and faster else "rejected"
    return PeakPhaseResult(
        POLICY_VERSION, median, p95, maximum, regression, improvement, speedup_ms,
        speedup_ratio, category, not regression.detected_bad and category != "rejected",
    )


def _trend(values: Sequence[float]) -> Trend:
    if len(set(values)) == 1:
        return Trend(None, None, False)
    version = importlib.metadata.version("scipy")
    if version != SCIPY_VERSION:
        raise RuntimeError(f"unsupported SciPy version: {version}")
    from scipy.stats import kendalltau

    result = kendalltau(range(len(values)), values, alternative="greater", method="auto")
    tau, p_value = float(result.statistic), float(result.pvalue)
    if not math.isfinite(tau) or not math.isfinite(p_value) or not 0 <= p_value <= 1:
        raise ValueError("invalid Kendall trend result")
    return Trend(tau, p_value, tau > 0 and p_value <= TREND_ALPHA)


def evaluate_retention(candidate_current_rss: Sequence[float]) -> RetentionResult:
    """Assess the finite post-release window; this cannot establish absence of leaks."""
    values = _values(candidate_current_rss, "candidate current RSS")
    initial = statistics.median(values[:5])
    final = statistics.median(values[-5:])
    delta = final - initial
    budget = min(RETENTION_BYTES, initial * RETENTION_RATIO)
    overall = _trend(values)
    tail = _trend(values[-10:])
    tail_increase = final > statistics.median(values[-10:-5])
    sustained = overall.significant_upward and tail.significant_upward and tail_increase
    return RetentionResult(
        POLICY_VERSION, TREND_FAMILY_COMPARISONS, TREND_FAMILY_ALPHA, TREND_ALPHA,
        initial, final, delta, budget, overall, tail, tail_increase,
        delta <= budget and not sustained,
    )
