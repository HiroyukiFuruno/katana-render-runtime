from collections.abc import Sequence
from typing import Literal


class WilcoxonResult:
    statistic: float
    pvalue: float


def wilcoxon(
    x: Sequence[float],
    y: Sequence[float],
    *,
    zero_method: Literal["wilcox", "pratt", "zsplit"] = "wilcox",
    alternative: Literal["two-sided", "less", "greater"] = "two-sided",
    method: Literal["auto", "exact", "approx"] = "auto",
) -> WilcoxonResult: ...
