"""Shared fixtures for host CPU load tests."""
from __future__ import annotations


def cpu_times(user: float, idle: float) -> dict[str, float]:
    return {"user": user, "system": 0.0, "idle": idle}
