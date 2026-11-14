from collections.abc import Sequence
from typing import Literal


class WilcoxonResult:
    statistic: float
    pvalue: float


class SignificanceResult:
    statistic: float
    pvalue: float


def kendalltau(
    x: Sequence[float],
    y: Sequence[float],
    *,
    nan_policy: Literal["propagate", "raise", "omit"] = "propagate",
    method: Literal["auto", "asymptotic", "exact"] = "auto",
    variant: Literal["b", "c"] = "b",
    alternative: Literal["two-sided", "less", "greater"] = "two-sided",
    axis: int | None = None,
    keepdims: bool = False,
) -> SignificanceResult: ...


def wilcoxon(
    x: Sequence[float],
    y: Sequence[float],
    *,
    zero_method: Literal["wilcox", "pratt", "zsplit"] = "wilcox",
    alternative: Literal["two-sided", "less", "greater"] = "two-sided",
    method: Literal["auto", "exact", "approx"] = "auto",
) -> WilcoxonResult: ...
