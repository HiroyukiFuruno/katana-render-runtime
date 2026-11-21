"""Paired regression statistics for the HTML performance domain."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
import importlib.metadata
import math


EXPECTED_SAMPLE_COUNT = 25
DEFAULT_FAMILY_COMPARISONS = 84 * 6 * 2


@dataclass(frozen=True)
class PairedRegressionResult:
    """The auditable result of one phase/metric regression comparison."""

    library: str
    method: str
    sample_count: int
    comparison_alpha: float
    test_performed: bool
    p_value: float | None
    statistic: float | None
    detected_bad: bool
    reason: str | None

    def to_json(self) -> dict[str, object]:
        return {
            "library": self.library,
            "method": self.method,
            "sampleCount": self.sample_count,
            "comparisonAlpha": self.comparison_alpha,
            "testPerformed": self.test_performed,
            "pValue": self.p_value,
            "statistic": self.statistic,
            "detectedBad": self.detected_bad,
            "reason": self.reason,
        }


def _validated_values(values: Sequence[float], label: str) -> tuple[float, ...]:
    if len(values) != EXPECTED_SAMPLE_COUNT:
        raise ValueError(f"{label} must contain exactly {EXPECTED_SAMPLE_COUNT} values")
    result = tuple(float(value) for value in values)
    if not all(math.isfinite(value) for value in result):
        raise ValueError(f"{label} must contain only finite values")
    return result


def _run_wilcoxon(
    candidate: tuple[float, ...], baseline: tuple[float, ...]
) -> tuple[float, float]:
    version = importlib.metadata.version("scipy")
    if version != "1.18.1":
        raise RuntimeError(f"unsupported SciPy version: {version}")
    from scipy.stats import wilcoxon

    result = wilcoxon(
        candidate,
        baseline,
        zero_method="pratt",
        alternative="greater",
        method="auto",
    )
    return float(result.pvalue), float(result.statistic)


def evaluate_paired_regression(
    baseline: Sequence[float],
    candidate: Sequence[float],
    *,
    family_comparisons: int = DEFAULT_FAMILY_COMPARISONS,
) -> PairedRegressionResult:
    """Test a candidate's whole-phase paired location shift.

    The test is not a separate hypothesis test for the raw median or p95.
    Equal vectors are explicitly reported as untested rather than assigned a
    fabricated p-value.
    """
    baseline_values = _validated_values(baseline, "baseline")
    candidate_values = _validated_values(candidate, "candidate")
    if family_comparisons <= 0:
        raise ValueError("family_comparisons must be positive")
    alpha = 0.05 / family_comparisons
    if baseline_values == candidate_values:
        return PairedRegressionResult(
            library="scipy==1.18.1 scipy.stats.wilcoxon",
            method="wilcoxon(pratt, greater, auto)",
            sample_count=EXPECTED_SAMPLE_COUNT,
            comparison_alpha=alpha,
            test_performed=False,
            p_value=None,
            statistic=None,
            detected_bad=False,
            reason="paired vectors are identical; no test performed",
        )

    p_value, statistic = _run_wilcoxon(candidate_values, baseline_values)
    if not math.isfinite(p_value) or not 0.0 <= p_value <= 1.0:
        raise ValueError(f"SciPy returned an invalid p-value: {p_value!r}")
    if not math.isfinite(statistic):
        raise ValueError(f"SciPy returned an invalid statistic: {statistic!r}")
    return PairedRegressionResult(
        library="scipy==1.18.1 scipy.stats.wilcoxon",
        method="wilcoxon(pratt, greater, auto)",
        sample_count=EXPECTED_SAMPLE_COUNT,
        comparison_alpha=alpha,
        test_performed=True,
        p_value=p_value,
        statistic=statistic,
        detected_bad=p_value <= alpha,
        reason=None,
    )
