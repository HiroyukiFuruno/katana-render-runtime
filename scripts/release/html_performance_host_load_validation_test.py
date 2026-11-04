"""Host-load method and immutable worker observation contracts."""
from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import cast
from unittest.mock import patch

import html_performance_host_load_validation as host
import html_performance_host_load as observer
import verify_html_performance_evidence as gate
from html_performance_host_load_test_support import cpu_times


class FixtureBundle:
    def __init__(self, order: list[gate.JsonObject], report: gate.JsonObject) -> None:
        self.order = order
        self.report = report

    def json(self, reference: object) -> gate.JsonObject:
        if reference == "manifest":
            return {"order": self.order}
        if reference == "expected":
            return {"reportPins": [{"testReport": True}], "catalogCondition": {"fixture_path": "/fixture"},
                    "operationPlanRef": {"path": "/plan"},
                    "viewport": {"width": 10, "height": 20, "scale": 1}}
        if reference == {"testReport": True}:
            return self.report
        return cast(gate.JsonObject, reference)

    def file(self, reference: object) -> Path:
        return Path(str(cast(gate.JsonObject, reference)["path"]))


def worker(sample: int, raw_root: Path) -> gate.JsonObject:
    folder = raw_root / str(sample)
    folder.mkdir(exist_ok=True)
    stdout = folder / "stdout.raw"
    stderr = folder / "stderr.raw"
    stdout.write_text(json.dumps({"sample": sample}), encoding="utf8")
    stderr.write_bytes(b"")
    record: gate.JsonObject = {
        "sample": sample, "attempt": 1, "accepted": True, "workerStarted": True, "actualExit": 0,
        "loadRatioBefore": 0.1, "loadRatioAfter": 0.2,
        "binarySha256Before": "b" * 64, "binarySha256After": "b" * 64,
        "lifecycle": {"schema": "krr-performance-command-lifecycle-v1", "role": "worker",
                      "exitCode": 0, "groupClosed": True, "rawFdsClosed": True,
                      "failure": None, "cleanupFailure": None, "identity": {"pid": 1},
                      "argv": ["/binary", "--worker", str(sample), "/fixture", "/output", "/plan", "10", "20", "1"],
                      "raw": [{"path": str(stdout)}, {"path": str(stderr)}]},
        "hostLoad": {"schema": "krr-host-cpu-busy-v1", "method": "psutil.cpu_times_percent",
                     "version": "7.2.2", "sampleIntervalSeconds": 1.0,
                     "intervalBeforeSeconds": 1.0, "intervalDuringSeconds": 1.1,
                     "cpuTimesBefore": cpu_times(10.0, 90.0),
                     "cpuTimesDuring": cpu_times(20.0, 80.0)},
    }
    return {**record, "attemptRecord": copy.deepcopy(record)}


class HostLoadValidationTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.raw_root = Path(temporary.name)
        self.rows = [worker(i, self.raw_root) for i in range(5)]
        self.order: list[gate.JsonObject] = [{"workerAttempts": self.rows, "executable_sha256": "b" * 64,
                                              "executable": "/binary", "output": "/output",
                                              "workerLoads": [{"before": 0.1, "after": 0.2}
                                                              for _ in range(5)]}]
        self.bundle = cast(gate.Bundle, FixtureBundle(self.order, {"samples": [{"sample": i} for i in range(5)]}))
        self.records = {host.PRODUCER: ("p" * 64, 1), host.ADAPTER: ("a" * 64, 1)}
        self.pins = patch.multiple(host, CPU_PRODUCER_SHA256="p" * 64, CPU_ADAPTER_SHA256="a" * 64)
        self.pins.start()
        self.addCleanup(self.pins.stop)

    def verify(self) -> None:
        host.verify_catalog_host_load(self.bundle, [{"pairManifest": "manifest", "expected": "expected"}], self.records)

    def _use_retry_attempts(self, first: gate.JsonObject) -> None:
        first["attemptRecord"] = {key: copy.deepcopy(value) for key, value in first.items()
                                  if key != "attemptRecord"}
        second = worker(0, self.raw_root)
        second["attempt"] = 2
        cast(gate.JsonObject, second["attemptRecord"])["attempt"] = 2
        self.order[0]["workerAttempts"] = [first, second, *[worker(i, self.raw_root) for i in range(1, 5)]]

    def test_complete_cpu_observations_are_accepted(self) -> None:
        self.verify()

    def test_method_duration_and_accepted_load_are_not_mutable(self) -> None:
        for field, value in (("method", "unknown"), ("version", "7.2.1"),
                             ("sampleIntervalSeconds", 0.1), ("intervalBeforeSeconds", 0.998),
                             ("intervalDuringSeconds", 0), ("intervalBeforeSeconds", float("nan"))):
            with self.subTest(field=field):
                self.setUp()
                for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
                    cast(gate.JsonObject, record["hostLoad"])[field] = value
                with self.assertRaises(ValueError):
                    self.verify()

    def test_mixed_or_forged_record_and_unknown_source_are_rejected(self) -> None:
        del self.rows[0]["hostLoad"]
        with self.assertRaises(ValueError):
            self.verify()
        self.records[host.PRODUCER] = ("unknown", 1)
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            self.verify()

    def test_accepted_worker_above_quiet_limit_is_rejected(self) -> None:
        for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
            record["loadRatioAfter"] = 0.5
            cast(gate.JsonObject, record["hostLoad"])["cpuTimesDuring"] = cpu_times(50.0, 50.0)
        with self.assertRaisesRegex(ValueError, "exceeds limit"):
            self.verify()

    def test_short_worker_interval_is_rejected(self) -> None:
        for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
            cast(gate.JsonObject, record["hostLoad"])["intervalDuringSeconds"] = 0.999
        with self.assertRaisesRegex(ValueError, "interval"):
            self.verify()

    def test_cpu_times_must_be_observed_valid_and_match_ratio(self) -> None:
        invalid = (cpu_times(0.0, 0.0), {"user": 10.0, "system": 0.0},
                   {"user": -1.0, "system": 0.0, "idle": 90.0},
                   {"user": float("nan"), "system": 0.0, "idle": 90.0},
                   {"user": 10.0, "system": 0.0, "idle": 90.0, "unknown": 1.0},
                   cpu_times(30.0, 70.0))
        for times in invalid:
            with self.subTest(times=times):
                self.setUp()
                for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
                    cast(gate.JsonObject, record["hostLoad"])["cpuTimesBefore"] = times
                with self.assertRaises(ValueError):
                    self.verify()

    def test_accepted_worker_rejects_changed_argv_or_stdout(self) -> None:
        for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
            cast(list[object], cast(gate.JsonObject, record["lifecycle"])["argv"])[3] = "/other-fixture"
        with self.assertRaisesRegex(ValueError, "lifecycle/argv"):
            self.verify()
        self.setUp()
        (self.raw_root / "0/stdout.raw").write_text('{"sample": 1}', encoding="utf8")
        with self.assertRaisesRegex(ValueError, "stdout differs"):
            self.verify()

    def test_load_rejected_retry_can_be_followed_by_accepted_sample(self) -> None:
        first = worker(0, self.raw_root)
        first.update({"accepted": False, "reason": "host load exceeded limit", "loadRatioAfter": 0.3})
        cast(gate.JsonObject, first["hostLoad"])["cpuTimesDuring"] = cpu_times(30.0, 70.0)
        self._use_retry_attempts(first)
        self.verify()

    def test_unobserved_worker_retries_only_with_actual_zero_counters(self) -> None:
        first = worker(0, self.raw_root)
        first.update({"accepted": False, "reason": "host load observation unavailable",
                      "hostLoadErrorType": "NoObservation", "loadRatioAfter": None})
        cast(gate.JsonObject, first["hostLoad"])["cpuTimesDuring"] = cpu_times(0.0, 0.0)
        self._use_retry_attempts(first)
        self.verify()
        for record in (first, cast(gate.JsonObject, first["attemptRecord"])):
            cast(gate.JsonObject, record["hostLoad"])["cpuTimesDuring"] = cpu_times(1.0, 0.0)
        with self.assertRaisesRegex(ValueError, "unobserved"):
            self.verify()

    def test_non_load_failed_attempts_cannot_be_retried_as_success(self) -> None:
        for reason in ("worker command raised", "host load observation failed"):
            with self.subTest(reason=reason):
                first = worker(0, self.raw_root)
                first.update({"accepted": False, "reason": reason, "errorType": "TimeoutError",
                              "actualExit": None, "workerStarted": None, "lifecycle": None,
                              "loadRatioAfter": None})
                if reason == "host load observation failed":
                    first["workerStarted"] = True
                cast(gate.JsonObject, first["hostLoad"])["intervalDuringSeconds"] = None
                self._use_retry_attempts(first)
                with self.assertRaises(ValueError):
                    self.verify()

    def test_missing_duration_cannot_be_accepted(self) -> None:
        for record in (self.rows[0], cast(gate.JsonObject, self.rows[0]["attemptRecord"])):
            record["loadRatioAfter"] = None
            cast(gate.JsonObject, record["hostLoad"])["intervalDuringSeconds"] = None
        with self.assertRaises(ValueError):
            self.verify()

    def test_old_producer_cannot_claim_new_method(self) -> None:
        self.records[host.PRODUCER] = (next(iter(host.LEGACY_PRODUCER_SHA256)), 1)
        with self.assertRaisesRegex(ValueError, "mixed"):
            self.verify()


class CurrentHostLoadPreflightTest(unittest.TestCase):
    def test_previous_current_cpu_producer_is_historical_and_unknown_is_rejected(self) -> None:
        previous = "6c4f667253690df1e7c703287eec97ed750bfe69fdaeeb65483e33445a1f9524"
        self.assertIn(previous, host.HISTORICAL_CPU_PRODUCER_SHA256)
        self.assertNotEqual(previous, host.CPU_PRODUCER_SHA256)
        records = {host.PRODUCER: (previous, 1), host.ADAPTER: (host.CPU_ADAPTER_SHA256, 1)}
        self.assertEqual(host._method(records), "cpu-busy")
        records[host.PRODUCER] = ("0" * 64, 1)
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            host._method(records)

    def test_historical_cpu_producer_requires_reviewed_adapter(self) -> None:
        historical = "b51d9b4db9459202da0828fcab217ca5760aa7deeb3ee55d2e01448c4df33fe5"
        records = {host.PRODUCER: (historical, 1), host.ADAPTER: (host.CPU_ADAPTER_SHA256, 1)}
        self.assertEqual(host._method(records), "cpu-busy")
        records[host.ADAPTER] = ("0" * 64, 1)
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            host._method(records)

    def test_historical_cpu_producer_is_not_a_current_observer(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            producer = root / host.PRODUCER
            adapter = root / host.ADAPTER
            producer.parent.mkdir(parents=True)
            producer.write_bytes(b"historical-cpu-producer")
            adapter.write_bytes(b"reviewed-adapter")
            with patch.multiple(host, HISTORICAL_CPU_PRODUCER_SHA256=frozenset({gate.sha256(producer)}),
                                CPU_ADAPTER_SHA256=gate.sha256(adapter)):
                with self.assertRaisesRegex(ValueError, "not the CPU observer"):
                    host.require_current_cpu_observer_sources(root)

    def test_cpu_ratio_excludes_idle_iowait_and_duplicate_guest(self) -> None:
        self.assertAlmostEqual(observer.ratio_from_times({"user": 30.0, "system": 10.0,
                                                        "idle": 50.0, "iowait": 10.0,
                                                        "guest": 4.0, "guest_nice": 1.0}), 0.4)

    def test_cpu_ratio_excludes_windows_system_subtotals(self) -> None:
        self.assertAlmostEqual(observer.ratio_from_times({"user": 30.0, "system": 20.0,
                                                        "idle": 50.0, "interrupt": 10.0,
                                                        "dpc": 5.0}), 0.5)

    def test_current_reviewed_sources_pass(self) -> None:
        host.require_current_cpu_observer_sources(Path(__file__).resolve().parents[2])

    def test_modified_or_legacy_producer_is_rejected(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            producer = root / host.PRODUCER
            adapter = root / host.ADAPTER
            producer.parent.mkdir(parents=True)
            producer.write_bytes(b"reviewed-producer")
            adapter.write_bytes(b"reviewed-adapter")
            with patch.multiple(host, CPU_PRODUCER_SHA256=gate.sha256(producer),
                                CPU_ADAPTER_SHA256=gate.sha256(adapter)):
                host.require_current_cpu_observer_sources(root)
                adapter.write_bytes(b"changed")
                with self.assertRaisesRegex(ValueError, "unreviewed"):
                    host.require_current_cpu_observer_sources(root)
                adapter.write_bytes(b"reviewed-adapter")
                producer.write_bytes(b"changed")
                with patch.object(host, "LEGACY_PRODUCER_SHA256", frozenset({gate.sha256(producer)})):
                    with self.assertRaisesRegex(ValueError, "not the CPU observer"):
                        host.require_current_cpu_observer_sources(root)
