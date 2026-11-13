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
CPU_PRODUCER_SHA256 = "85e747448ab4620fb4b652bbbdb199a9eba16842d5a9783dbdd8bb61160dc40b"
HISTORICAL_CPU_PRODUCER_SHA256 = frozenset({
    "6d2816c8a0d0007c027245db79c21e976ca0156889d1f953f9b5fcc64ea8ec3a",
    "29ffbeefb0ffaab61de95c6aa738a4403c76e5809843a6ad3169f6b7fc34467d",
    "d0a4e666e253ecda8ce70f0adc830c31bcc3fd8aaa9c613a4399b41c9a968380",
    "51d703d9061e7753af1e981cb803da0395e312f2d8d412afe1ddafca98872840",
    "69eadcced17e1b451b7667e0154196735cb626ff5deb6eefce5d23bfa7f5877d",
    "213afcfcf895ee1f048564e70d10668187e0829eee49cbb3fbdc384648777e1b",
    "43d967863089f9c2267326f2dde177db898861d1c0634f2e951eda7b212cbbff",
    "2803c2a0a5f212eaeff21334ca2b99e6c80e1fe46f6df232a2a6c20de581d555",
    "43cad2e67fc7aac594e3d120643fcdcb03e97462f40e04c0fdee8a8c9599c323",
    "2082511a77c50406116fbee1158aad474a5f038d18ea46bbce6bfbd68196799c",
    "b8e3fdbc94a9f397051a1268060efb1f0ffe1604e2f0adda0f89afa779bdb359",
    "a2b967f5b5ce92ec9ed3c6fcc066f68f3e4ae25db1d43de3948a3b9f7786e816",
    "5b5c62c6ec147f0ed9a541c6eb9d07a883cb3ad90d75eb71bc5b6d8f7750e08c",
    "80ded11e1b2984635cd455aa55dd926eefbbd03759d8c9eccd719dc16bfd7d45",
    "bffd5b7781ea1650185d2756a90c4c7350d7708726d7482cf6fff397c4b14aa8",
    "490233a4797d9690f7393000cfeda1ceb4d806e1a7df2c588bbb300d8cc1dae5",
    "d4d85ae7e2120e3720ef31afe6a7228011385d0267ec770060e68f606e21388e",
    "d64a50f10dc9aa73617483e24ef20de218abca396f240f6466a635cf8135daaa",
    "6c4f667253690df1e7c703287eec97ed750bfe69fdaeeb65483e33445a1f9524",
    "837c0718d7e264af32ecea8ef0e4b9378a4def6d0fb40599ea25bc9ffc9ff4f2",
    "25b67939da058c13c86288dc0c5edf2a0f51b2018fef7256b104b6f8615898ec",
    "359a0241a1e9774838be30b30bd1d32b6b787375e4b9715fc09e12c7c88aa86a",
    "2bfb0312ed77e0f58350d909059d017196575cc18bde71a8cd85c430667c68cf",
    "12a6eab7b7da414e1fd88ea2071023cd01bdcd24bed85ab107154c420b36e921",
    "b51d9b4db9459202da0828fcab217ca5760aa7deeb3ee55d2e01448c4df33fe5",
    "710f6ed046aee1eac06604f01e1733086a0bcb1d3f309b75483be6f5febb87e1",
    "f402cf7718a323a536a67f8b1311bb315ac36b9ddcb79488bc28646778784809",
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
