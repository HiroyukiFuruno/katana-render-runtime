"""Observe a diagnostic probe at its two open barriers; not release evidence."""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import psutil

import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate

MAX_LINE_BYTES = 8 * 1024
MAX_TOOL_OUTPUT_BYTES = 16 * 1024 * 1024
MALLOC_REGION = re.compile(
    r"^(Malloc [A-Za-z]+(?: \(empty\))?)\s+[0-9a-fA-F]+-[0-9a-fA-F]+\s+"
    r"\[\s+\S+\s+(\S+)\s+(\S+)"
)


def _bytes(value: str) -> int:
    match = re.fullmatch(r"(\d+(?:\.\d+)?)([KMG])", value)
    if match is None:
        raise ValueError("unsupported vmmap size")
    return int(float(match.group(1)) * {"K": 1024, "M": 1024**2, "G": 1024**3}[match.group(2)])


def malloc_regions(text: str) -> dict[str, dict[str, int]]:
    regions: dict[str, dict[str, int]] = {}
    for line in text.splitlines():
        match = MALLOC_REGION.match(line)
        if match is None:
            continue
        category = regions.setdefault(match.group(1), {"resident": 0, "dirty": 0})
        category["resident"] += _bytes(match.group(2))
        category["dirty"] += _bytes(match.group(3))
    gate.require(bool(regions), "vmmap contains no malloc regions")
    return regions


def heap_summary(text: str) -> dict[str, int]:
    matches = re.findall(r"^All zones: (\d+) nodes \((\d+) bytes\)$", text, re.MULTILINE)
    gate.require(len(matches) == 1, "heap summary is unavailable or ambiguous")
    return {"allocations": int(matches[0][0]), "bytes": int(matches[0][1])}


def bounded_tool_output(arguments: list[str], deadline: float) -> str:
    remaining = deadline - time.monotonic()
    gate.require(remaining > 0, "open diagnostic deadline expired")
    # POSIX sh applies the file-size limit after exec, without a preexec_fn in this process.
    limit_kib = MAX_TOOL_OUTPUT_BYTES // 1024
    command = ["/bin/sh", "-c", f'ulimit -f {limit_kib} || exit 125; exec "$@"',
               "krr-diagnostic", *arguments]
    with tempfile.TemporaryFile() as output:
        result = subprocess.run(command, stdout=output, stderr=subprocess.DEVNULL,
                                check=False, timeout=min(30.0, remaining))
        gate.require(result.returncode == 0, f"diagnostic tool exited {result.returncode}")
        gate.require(output.tell() <= MAX_TOOL_OUTPUT_BYTES, "diagnostic tool output exceeds limit")
        output.seek(0)
        return output.read(MAX_TOOL_OUTPUT_BYTES + 1).decode("utf-8")


class OpenInteraction:
    def __init__(self) -> None:
        self.offset = 0
        self.samples: list[int] = []
        self.regions: list[dict[str, dict[str, int]]] = []
        self.heap: list[dict[str, int]] = []
        self.started: float | None = None

    def __call__(self, process: subprocess.Popen[bytes], output: Path, deadline: float) -> None:
        with output.open("rb") as stream:
            stream.seek(self.offset)
            while line := stream.readline(MAX_LINE_BYTES + 1):
                gate.require(len(line) <= MAX_LINE_BYTES, "open diagnostic line exceeds limit")
                if not line.endswith(b"\n"):
                    break
                gate.require(line == b"READY\n" and len(self.samples) < 2,
                             "invalid open diagnostic protocol")
                self._sample(process, deadline)
                self.offset = stream.tell()
        if process.poll() is not None:
            gate.require(len(self.samples) == 2 and self.offset == output.stat().st_size,
                         "incomplete open diagnostic protocol")

    def _sample(self, process: subprocess.Popen[bytes], deadline: float) -> None:
        gate.require(time.monotonic() < deadline and process.poll() is None,
                     "open diagnostic worker unavailable")
        observed = psutil.Process(process.pid)
        started = observed.create_time()
        if self.started is None:
            self.started = started
        gate.require(started == self.started and observed.is_running(), "worker identity changed")
        self.samples.append(observed.memory_info().rss)
        self.regions.append(malloc_regions(bounded_tool_output(["vmmap", "-wide", str(process.pid)], deadline)))
        self.heap.append(heap_summary(bounded_tool_output(["heap", "-s", "-q", str(process.pid)], deadline)))
        gate.require(time.monotonic() < deadline and process.poll() is None
                     and psutil.Process(process.pid).create_time() == started,
                     "worker identity changed during vmmap")
        worker_input = process.stdin
        if worker_input is None:
            raise ValueError("worker input unavailable")
        worker_input.write(b"ACK\n")
        worker_input.flush()


def collect(binary: Path, fixture: Path, width: int, height: int, scale: float,
            output: Path) -> None:
    gate.require(sys.platform == "darwin" and binary.is_file() and fixture.is_file(),
                 "diagnostic inputs unavailable")
    binary_hash, fixture_hash = gate.sha256(binary), gate.sha256(fixture)
    interaction = OpenInteraction()
    lifecycle: list[Path] = []
    with tempfile.TemporaryDirectory(prefix="html-open-diagnostic-") as directory:
        result = producer._measurement_command(
            [str(binary), str(fixture), str(width), str(height), str(scale)], role="worker",
            log_directory=Path(directory), cwd=Path.cwd(), env=dict(os.environ),
            lifecycle_sink=lifecycle.append, interaction=interaction,
        )
        gate.require(result.returncode == 0 and len(lifecycle) == 1, "diagnostic worker failed")
        state = gate.read_json(lifecycle[0])
        gate.require(state.get("groupClosed") is True and state.get("rawFdsClosed") is True,
                     "diagnostic worker cleanup failed")
        gate.require(gate.sha256(binary) == binary_hash and gate.sha256(fixture) == fixture_hash,
                     "diagnostic input changed")
        summary = {"diagnosticOnly": True, "binarySha256": binary_hash,
                   "fixtureSha256": fixture_hash, "viewport": [width, height, scale],
                   "beforeRss": interaction.samples[0], "afterRss": interaction.samples[1],
                   "beforeMallocRegions": interaction.regions[0],
                   "afterMallocRegions": interaction.regions[1],
                   "beforeHeap": interaction.heap[0], "afterHeap": interaction.heap[1],
                   "exitCode": result.returncode, "groupClosed": True, "rawFdsClosed": True}
        descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(summary, stream, ensure_ascii=False, indent=2)
            stream.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("width", type=int)
    parser.add_argument("height", type=int)
    parser.add_argument("scale", type=float)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    collect(args.binary.resolve(strict=True), args.fixture.resolve(strict=True), args.width,
            args.height, args.scale, args.output)


if __name__ == "__main__":
    main()
