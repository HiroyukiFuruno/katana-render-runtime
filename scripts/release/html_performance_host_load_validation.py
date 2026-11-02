"""Bind host-load observations to reviewed producer source and attempt records."""
from __future__ import annotations

import math
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from verify_html_performance_evidence import Bundle, JsonObject, SourceRecords

# Historical producer bytes are fixed from the accepted reference source.
LEGACY_PRODUCER_SHA256 = frozenset({
    "fd6e1083df03cc7dc8f016e847337aa7c7d3c50d16ac57bfa016d6641468b6f5",
    "721ec42d4ba7ce2e06beba9cda6458e68f7b0e27222b3f9cd1fb16e6eec63736",
})
# The producer and its adapter must be independently reviewed and fixed together.
CPU_PRODUCER_SHA256 = "710f6ed046aee1eac06604f01e1733086a0bcb1d3f309b75483be6f5febb87e1"
HISTORICAL_CPU_PRODUCER_SHA256 = frozenset({
    "b51d9b4db9459202da0828fcab217ca5760aa7deeb3ee55d2e01448c4df33fe5",
})
CPU_ADAPTER_SHA256 = "bbfc04542a16b94527431f3288f2f303c34b7940b6ef675f1fab44b94edbd5e2"
PRODUCER = "scripts/release/produce_html_performance_evidence.py"
ADAPTER = "scripts/release/html_performance_host_load.py"


def _method(records: Mapping[str, tuple[str, int]]) -> str:
    producer = records.get(PRODUCER, ("", 0))[0]
    if producer in LEGACY_PRODUCER_SHA256:
        return "legacy-load-average"
    if ((producer == CPU_PRODUCER_SHA256 or producer in HISTORICAL_CPU_PRODUCER_SHA256)
            and records.get(ADAPTER, ("", 0))[0] == CPU_ADAPTER_SHA256):
        return "cpu-busy"
    raise ValueError("unreviewed host-load producer source")


def require_current_cpu_observer_sources(root: Path) -> None:
    """Fail before measurement unless both current observer sources are reviewed."""
    from verify_html_performance_evidence import sha256

    if not root.is_absolute() or root.resolve(strict=True) != root:
        raise ValueError("host-load source root differs")
    records: dict[str, tuple[str, int]] = {}
    for name in (PRODUCER, ADAPTER):
        path = root / name
        if not path.is_file() or path.is_symlink() or path.resolve(strict=True) != path:
            raise ValueError("host-load source is not a canonical regular file")
        records[name] = (sha256(path), path.stat().st_size)
    if _method(records) != "cpu-busy" or records[PRODUCER][0] != CPU_PRODUCER_SHA256:
        raise ValueError("current host-load producer is not the CPU observer")


def _cpu_attempt(bundle: Bundle, row: JsonObject, expected: JsonObject, order: JsonObject,
                 report: JsonObject) -> tuple[int, bool, float, float | None]:
    from html_performance_host_load import NoObservation, ratio_from_times
    from verify_html_performance_evidence import (
        _integer, _number, _numeric, _object, _objects, _validate_worker_lifecycle_identity,
        require, unique_object,
    )
    import json

    reference = _object(row.get("attemptRecord"), "host-load attempt record")
    saved = bundle.json(reference)
    require(saved == {key: value for key, value in row.items() if key != "attemptRecord"},
            "host-load attempt record differs")
    tag = _object(row.get("hostLoad"), "host-load method")
    require(set(tag) == {"schema", "method", "version", "sampleIntervalSeconds",
                         "intervalBeforeSeconds", "intervalDuringSeconds",
                         "cpuTimesBefore", "cpuTimesDuring"}
            and tag.get("schema") == "krr-host-cpu-busy-v1"
            and tag.get("method") == "psutil.cpu_times_percent" and tag.get("version") == "7.2.2"
            and _number(tag.get("sampleIntervalSeconds"), "sample interval") == 1.0
            and 0.999 <= _number(tag.get("intervalBeforeSeconds"), "before interval") <= 900,
            "host-load method or interval differs")
    before = float(_numeric(row.get("loadRatioBefore"), "host-load before"))
    accepted = row.get("accepted")
    if not isinstance(accepted, bool):
        raise ValueError("host-load worker acceptance differs")
    require(math.isfinite(before) and 0 <= before <= 0.25, "host-load worker value differs")
    require(1.0 <= _number(tag.get("intervalDuringSeconds"), "during interval") <= 900
            and row.get("workerStarted") is True, "host-load during interval differs")
    require(before == ratio_from_times(tag.get("cpuTimesBefore")),
            "host-load before percentage differs from recorded CPU times")
    during: float | None = None
    if row.get("reason") == "host load observation unavailable":
        values = _object(tag.get("cpuTimesDuring"), "unobserved CPU times")
        require(not accepted and row.get("hostLoadErrorType") == "NoObservation"
                and row.get("loadRatioAfter") is None and bool(values)
                and all(type(value) in (int, float) and value == 0 for value in values.values()),
                "unobserved host-load record differs")
        try:
            ratio_from_times(values)
        except NoObservation:
            pass
        else:
            raise ValueError("unobserved host-load counters are not empty")
    else:
        during = float(_numeric(row.get("loadRatioAfter"), "host-load during"))
        require(math.isfinite(during) and 0 <= during <= 1
                and during == ratio_from_times(tag.get("cpuTimesDuring")),
                "host-load during percentage differs from recorded CPU times")
    binary_sha = order.get("executable_sha256")
    require(isinstance(binary_sha, str) and row.get("binarySha256Before") == binary_sha
            and row.get("binarySha256After") == binary_sha and row.get("actualExit") == 0,
            "host-load worker binary or exit differs")
    lifecycle = bundle.json(row.get("lifecycle"))
    require(lifecycle.get("schema") == "krr-performance-command-lifecycle-v1",
            "host-load worker lifecycle differs")
    sample = _integer(row.get("sample"), "host-load sample")
    _validate_worker_lifecycle_identity(expected, order, lifecycle, sample=sample, exit_code=0)
    if accepted:
        require(during is not None and during <= 0.25, "accepted host-load worker exceeds limit")
        raw = _objects(lifecycle.get("raw"), "worker lifecycle raw streams")
        stdout = [item for item in raw if Path(str(item.get("path"))).name == "stdout.raw"]
        stderr = [item for item in raw if Path(str(item.get("path"))).name == "stderr.raw"]
        require(len(raw) == 2 and len(stdout) == len(stderr) == 1,
                "worker lifecycle raw stream count differs")
        payload = json.loads(bundle.file(stdout[0]).read_bytes(), object_pairs_hook=unique_object)
        bundle.file(stderr[0])
        samples = _objects(report.get("samples"), "worker report samples")
        require(sample in range(len(samples)) and payload == samples[sample],
                "worker stdout differs from report sample")
    else:
        require((row.get("reason") == "host load exceeded limit" and during is not None and during > 0.25)
                or (row.get("reason") == "host load observation unavailable" and during is None),
                "unaccepted host-load worker differs")
    return sample, accepted, before, during


def verify_catalog_host_load(bundle: Bundle, attempts: list[JsonObject], records: SourceRecords) -> None:
    """Reject mixed methods and require the reviewed producer for CPU observations."""
    from verify_html_performance_evidence import _objects, require

    method = _method(records)
    for attempt in attempts:
        manifest = bundle.json(attempt["pairManifest"])
        expected = bundle.json(attempt["expected"])
        pins = _objects(expected.get("reportPins"), "host-load report pins")
        orders = _objects(manifest.get("order"), "host-load pair order")
        require(len(pins) == len(orders), "host-load report count differs")
        for index, order in enumerate(orders):
            workers = _objects(order.get("workerAttempts", []), "host-load worker attempts")
            if method == "legacy-load-average":
                require(all("hostLoad" not in worker for worker in workers), "mixed host-load methods")
                continue
            loads = _objects(order.get("workerLoads"), "host-load accepted loads")
            require(len(loads) == 5 and bool(workers), "CPU host-load attempts missing")
            report = bundle.json(pins[index])
            accepted: dict[int, tuple[float, float]] = {}
            next_attempt = [1] * 5
            for worker in workers:
                sample, ok, before, during = _cpu_attempt(bundle, worker, expected, order, report)
                require(sample in range(5), "host-load sample differs")
                require(sample not in accepted and worker.get("attempt") == next_attempt[sample]
                        and next_attempt[sample] <= 3, "host-load retry order differs")
                next_attempt[sample] += 1
                if ok:
                    if during is None:
                        raise ValueError("accepted host-load observation missing")
                    accepted[sample] = (before, during)
            require(set(accepted) == set(range(5))
                    and loads == [{"before": accepted[i][0], "after": accepted[i][1]}
                                  for i in range(5)], "host-load accepted observations differ")
