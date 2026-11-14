"""Contracts for the real host CPU time observer."""
from __future__ import annotations

import math
import threading
import time
import unittest
from typing import NamedTuple
from unittest.mock import call, patch

import psutil
import html_performance_host_load as host_load


class CpuTimes(NamedTuple):
    user: float
    idle: float
    system: float = 0.0


class HostLoadTests(unittest.TestCase):
    def test_ratios_exclude_guest_and_iowait_without_rounding_to_zero(self) -> None:
        self.assertAlmostEqual(host_load.ratio_from_times({"user": 30.0, "system": 10.0,
            "idle": 50.0, "iowait": 10.0, "guest": 5.0, "guest_nice": 2.0}), 0.4)
        self.assertEqual(host_load.ratio_from_times({"user": 0.0, "system": 0.0, "idle": 100.0}), 0.0)
        self.assertEqual(host_load.ratio_from_times({"user": 30.0, "system": 20.0,
            "idle": 50.0, "interrupt": 10.0, "dpc": 5.0}), 0.5)

    def test_unobserved_and_invalid_time_fields_are_rejected(self) -> None:
        with self.assertRaises(host_load.NoObservation):
            host_load.ratio_from_times({"user": 0.0, "system": 0.0, "idle": 0.0})
        for value in ({"user": 0.0, "idle": 100.0},
                      {"user": 101.0, "system": 0.0, "idle": 0.0},
                      {"user": 0.0, "system": 0.0, "idle": 0.0, "guest": 1.0}):
            with self.subTest(value=value), self.assertRaises(ValueError) as caught:
                host_load.ratio_from_times(value)
            self.assertNotIsInstance(caught.exception, host_load.NoObservation)
        for value in ({"user": 1.0}, *({"user": 0.0, "system": 0.0, "idle": item}
                       for item in (-1.0, math.nan, math.inf, True)),
                      {"user": 0.0, "system": 0.0, "idle": 1.0, "unknown": 1.0}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                host_load.ratio_from_times(value)

    def test_blocking_prime_then_worker_interval_on_same_thread(self) -> None:
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", side_effect=[CpuTimes(20, 80), CpuTimes(30, 70)]) as cpu,
              patch.object(time, "monotonic", side_effect=[1.0, 1.0, 2.0, 3.0, 3.0])):
            before = host_load.start()
            after, duration, values = host_load.finish(before)
        self.assertAlmostEqual(before.before, 0.2)
        self.assertAlmostEqual(before.before_seconds, host_load.INTERVAL_SECONDS)
        self.assertAlmostEqual(after, 0.3)
        self.assertAlmostEqual(duration, 1.0)
        self.assertEqual(values, {"user": 30, "idle": 70, "system": 0})
        self.assertEqual(cpu.call_args_list, [call(interval=host_load.INTERVAL_SECONDS, percpu=False),
                                               call(interval=None, percpu=False)])

    def test_zero_prime_is_retried_and_never_interpreted_as_idle(self) -> None:
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", side_effect=[CpuTimes(0, 0), CpuTimes(0, 100)]) as cpu,
              patch.object(time, "monotonic", side_effect=[0.0, 0.0, 1.0, 1.0, 2.0])):
            before = host_load.start()
        self.assertEqual(before.before, 0.0)
        self.assertEqual(before.before_times, {"user": 0, "idle": 100, "system": 0})
        self.assertEqual(cpu.call_count, 2)

    def test_short_worker_waits_then_zero_interval_rejects_without_resampling(self) -> None:
        probe = host_load.Probe(0.1, 0.1, 10.0, threading.get_ident(), {"user": 10, "idle": 90})
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", return_value=CpuTimes(0, 0)) as cpu,
              patch.object(time, "monotonic", side_effect=[10.04, 11.01, 11.01]),
              patch.object(time, "sleep") as sleep):
            with self.assertRaises(host_load.NoObservation) as caught:
                host_load.finish(probe)
        self.assertEqual(caught.exception.times, {"user": 0, "idle": 0, "system": 0})
        self.assertAlmostEqual(caught.exception.interval_seconds or 0, 1.01)
        self.assertEqual(sleep.call_count, 1)
        self.assertAlmostEqual(sleep.call_args_list[0].args[0], host_load.INTERVAL_SECONDS - 0.04)
        self.assertEqual(cpu.call_count, 1)

    def test_long_worker_does_not_wait_when_observation_is_valid(self) -> None:
        probe = host_load.Probe(0.1, 0.1, 10.0, threading.get_ident(), {"user": 10, "idle": 90})
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", return_value=CpuTimes(10, 90)),
              patch.object(time, "monotonic", return_value=11.5), patch.object(time, "sleep") as sleep):
            _, duration, _ = host_load.finish(probe)
        self.assertAlmostEqual(duration, 1.5)
        sleep.assert_not_called()

    def test_zero_counters_time_out_and_clock_errors_fail_closed(self) -> None:
        probe = host_load.Probe(0.1, 0.1, 1.0, threading.get_ident(), {"user": 10, "idle": 90})
        clock = [1.0]
        def sample(*, interval: float | None, percpu: bool) -> CpuTimes:
            self.assertEqual(interval, host_load.INTERVAL_SECONDS)
            self.assertFalse(percpu)
            clock[0] += host_load.INTERVAL_SECONDS
            return CpuTimes(0, 0)
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", side_effect=sample),
              patch.object(time, "monotonic", side_effect=lambda: clock[0])):
            with self.assertRaises(host_load.NoObservation):
                host_load.start()
        for times in ([0.9], [1.01, 1.01], [math.nan]):
            with (self.subTest(times=times), patch.object(psutil, "__version__", host_load.VERSION),
                  patch.object(psutil, "cpu_times_percent") as cpu,
                  patch.object(time, "monotonic", side_effect=times), patch.object(time, "sleep")):
                with self.assertRaises(ValueError):
                    host_load.finish(probe)
                cpu.assert_not_called()

    def test_wrong_thread_version_and_sampler_failure_are_rejected(self) -> None:
        probe = host_load.Probe(0.1, 0.1, 1.0, threading.get_ident() + 1, {"user": 10, "idle": 90})
        with patch.object(psutil, "__version__", host_load.VERSION), self.assertRaises(ValueError):
            host_load.finish(probe)
        with patch.object(psutil, "__version__", "unexpected"), self.assertRaises(ValueError):
            host_load.start()
        with (patch.object(psutil, "__version__", host_load.VERSION),
              patch.object(psutil, "cpu_times_percent", side_effect=OSError("unavailable")),
              patch.object(time, "monotonic", side_effect=[1.0, 1.0]), self.assertRaises(OSError)):
            host_load.start()
