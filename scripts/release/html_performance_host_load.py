"""Measure whole-host CPU busy time around one performance worker."""
from __future__ import annotations

import math
import threading
import time
from dataclasses import dataclass

import psutil

VERSION = "7.2.2"
INTERVAL_SECONDS = 1.0
OBSERVATION_LIMIT_SECONDS = 2.0
CPU_FIELDS = frozenset({"user", "nice", "system", "idle", "iowait", "irq", "softirq",
                        "steal", "guest", "guest_nice", "interrupt", "dpc"})
OVERLAPPING_FIELDS = frozenset({"guest", "guest_nice", "interrupt", "dpc"})


class NoObservation(ValueError):
    """CPU counters supplied no usable interval."""

    def __init__(self, message: str, times: dict[str, float] | None = None,
                 interval_seconds: float | None = None) -> None:
        super().__init__(message)
        self.times = times
        self.interval_seconds = interval_seconds


@dataclass(frozen=True)
class Probe:
    before: float
    before_seconds: float
    started_at: float
    thread_id: int
    before_times: dict[str, float]


def _timestamp() -> float:
    value = time.monotonic()
    if not math.isfinite(value):
        raise ValueError("host load monotonic clock is invalid")
    return value


def ratio_from_times(value: object) -> float:
    if (not isinstance(value, dict) or not {"user", "system", "idle"} <= set(value)
            or not set(value) <= CPU_FIELDS):
        raise ValueError("host CPU time fields are invalid")
    try:
        if any(isinstance(item, bool) or not isinstance(item, (int, float))
               or not math.isfinite(item) or not 0 <= item <= 100 for item in value.values()):
            raise ValueError("host CPU time percentages are invalid")
        total = math.fsum(float(item) for name, item in value.items() if name not in OVERLAPPING_FIELDS)
    except OverflowError as error:
        raise ValueError("host CPU time percentages are invalid") from error
    if total == 0:
        if any(item != 0 for item in value.values()):
            raise ValueError("host CPU overlapping percentages are inconsistent")
        raise NoObservation("host CPU counters did not advance", dict(value))
    idle = float(value["idle"]) + float(value.get("iowait", 0.0))
    ratio = (total - idle) / total
    if not math.isfinite(ratio) or not 0.0 <= ratio <= 1.0:
        raise ValueError("host CPU busy sample is invalid")
    return ratio


def _sample(interval: float | None) -> tuple[float, dict[str, float]]:
    raw = psutil.cpu_times_percent(interval=interval, percpu=False)
    values = dict(raw._asdict())
    return ratio_from_times(values), values


def _duration(start: float, end: float) -> float:
    value = end - start
    if not math.isfinite(value) or value <= 0.0:
        raise ValueError("host CPU measurement interval is invalid")
    return value


def start() -> Probe:
    if psutil.__version__ != VERSION:
        raise ValueError("host CPU observer version differs")
    last = _timestamp()
    limit = last + OBSERVATION_LIMIT_SECONDS
    while True:
        before = _timestamp()
        if before < last:
            raise ValueError("host CPU monotonic clock moved backwards")
        if before + INTERVAL_SECONDS > limit:
            raise NoObservation("host CPU pre-worker observation timed out")
        try:
            ratio, values = _sample(INTERVAL_SECONDS)
        except NoObservation:
            last = _timestamp()
            if last <= before:
                raise ValueError("host CPU monotonic clock did not advance") from None
            continue
        after = _timestamp()
        if after > limit:
            raise NoObservation("host CPU pre-worker observation timed out")
        return Probe(ratio, _duration(before, after), after, threading.get_ident(), values)


def finish(probe: Probe) -> tuple[float, float, dict[str, float]]:
    if psutil.__version__ != VERSION or threading.get_ident() != probe.thread_id:
        raise ValueError("host CPU observer identity differs")
    now = _timestamp()
    if now < probe.started_at:
        raise ValueError("host CPU monotonic clock moved backwards")
    while now - probe.started_at < INTERVAL_SECONDS:
        time.sleep(INTERVAL_SECONDS - (now - probe.started_at))
        after_sleep = _timestamp()
        if after_sleep <= now:
            raise ValueError("host CPU monotonic clock did not advance")
        now = after_sleep
    # 無観測でもpsutilの基準点は進むため、再読せずworker全体を再試行する。
    try:
        ratio, values = _sample(None)
    except NoObservation as error:
        observed = _timestamp()
        if observed < now:
            raise ValueError("host CPU monotonic clock moved backwards") from error
        error.interval_seconds = _duration(probe.started_at, observed)
        raise
    observed = _timestamp()
    if observed < now:
        raise ValueError("host CPU monotonic clock moved backwards")
    return ratio, _duration(probe.started_at, observed), values


def metadata(probe: Probe, during_seconds: float | None,
             during_times: dict[str, float] | None) -> dict[str, object]:
    return {"schema": "krr-host-cpu-busy-v1", "method": "psutil.cpu_times_percent",
            "version": VERSION, "sampleIntervalSeconds": INTERVAL_SECONDS,
            "intervalBeforeSeconds": probe.before_seconds, "intervalDuringSeconds": during_seconds,
            "cpuTimesBefore": probe.before_times, "cpuTimesDuring": during_times}
