"""Capture the dedicated retention probe without replacing measurement ownership."""
from __future__ import annotations

import json
import secrets
import subprocess
import time
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import NamedTuple

import psutil

import html_performance_host_load as host_load
import produce_html_performance_evidence as producer
import verify_html_performance_evidence as gate

WARMUP_CYCLES = 5
MEASUREMENT_CYCLES = 25
TOTAL_CYCLES = WARMUP_CYCLES + MEASUREMENT_CYCLES
MAX_LINE_BYTES = 8 * 1024 * 1024


class RetentionCapture(NamedTuple):
    directory: Path
    lifecycle: Path
    runtime_execution: gate.JsonObject


class _Interaction:
    def __init__(self, metrics: Path, validate_sample: Callable[[int, bytes], None]) -> None:
        self.metrics = metrics
        self.validate_sample = validate_sample
        self.offset = 0
        self.cycle = 0
        self.waiting_ready = False
        self.process_start: float | None = None

    def __call__(self, process: subprocess.Popen[bytes], path: Path, deadline: float) -> None:
        with path.open("rb") as stream:
            stream.seek(self.offset)
            while True:
                start = stream.tell()
                line = stream.readline(MAX_LINE_BYTES + 1)
                if len(line) > MAX_LINE_BYTES:
                    raise ValueError("retention protocol line exceeds limit")
                if not line:
                    break
                if not line.endswith(b"\n"):
                    stream.seek(start)
                    break
                self._line(process, line, deadline)
                self.offset = stream.tell()
        if process.poll() is not None:
            gate.require(self.cycle == TOTAL_CYCLES and not self.waiting_ready
                         and self.offset == path.stat().st_size,
                         "retention protocol incomplete")

    def _line(self, process: subprocess.Popen[bytes], line: bytes, deadline: float) -> None:
        gate.require(self.cycle < TOTAL_CYCLES, "extra retention protocol record")
        sample_prefix = f"SAMPLE {self.cycle} ".encode("ascii")
        if not self.waiting_ready:
            gate.require(line.startswith(sample_prefix), "retention sample order differs")
            self.validate_sample(self.cycle, line[len(sample_prefix):-1])
            self.waiting_ready = True
            return
        gate.require(line == f"READY {self.cycle}\n".encode("ascii"),
                     "retention barrier order differs")
        self._sample_and_ack(process, deadline)
        self.cycle += 1
        self.waiting_ready = False

    def _sample_and_ack(self, process: subprocess.Popen[bytes], deadline: float) -> None:
        gate.require(time.monotonic() < deadline and process.poll() is None,
                     "retention worker unavailable at barrier")
        observed = psutil.Process(process.pid)
        started = observed.create_time()
        if self.process_start is None:
            self.process_start = started
        gate.require(started == self.process_start and observed.is_running(),
                     "retention worker identity differs")
        memory = observed.memory_info()
        gate.require(time.monotonic() < deadline and process.poll() is None
                     and psutil.Process(process.pid).create_time() == started,
                     "retention worker changed during sample")
        record = {"schema": "krr-html-retention-memory-v1", "method": "psutil.Process.memory_info",
                  "psutilVersion": psutil.__version__,
                  "cycle": self.cycle, "warmup": self.cycle < WARMUP_CYCLES,
                  "pid": process.pid, "createTime": started, "rss": memory.rss,
                  "vms": memory.vms, "monotonicNs": time.monotonic_ns()}
        with self.metrics.open("ab") as output:
            output.write(json.dumps(record, sort_keys=True, separators=(",", ":")).encode() + b"\n")
            output.flush()
        gate.require(process.stdin is not None, "retention worker stdin missing")
        if process.stdin is None:
            raise ValueError("retention worker stdin missing")
        process.stdin.write(f"ACK {self.cycle}\n".encode("ascii"))
        process.stdin.flush()


def capture(argv: Sequence[str], *, cwd: Path, env: Mapping[str, str], output: Path,
            validate_sample: Callable[[int, bytes], None]) -> RetentionCapture:
    """Collect candidate retention observations under the established worker owner."""
    gate.require(psutil.__version__ == host_load.VERSION == "7.2.2", "retention psutil version differs")
    gate.require(bool(argv) and all(isinstance(item, str) and item for item in argv),
                 "retention argv differs")
    args = list(argv)
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    with (output / "command.json").open("xb") as stream:
        stream.write(json.dumps({"argv": args, "kind": "retention-observation"},
                                sort_keys=True, separators=(",", ":")).encode() + b"\n")
    metrics = output / "memory.jsonl"
    with metrics.open("xb"):
        pass
    interaction = _Interaction(metrics, validate_sample)
    lifecycle: list[Path] = []
    runtime: list[gate.JsonObject] = []
    result = producer._measurement_command(args, role="worker", log_directory=output,
        cwd=cwd, env=env, lifecycle_sink=lifecycle.append,
        runtime_execution_sink=runtime.append, interaction=interaction,
        native_attempt_token=secrets.token_hex(32))
    gate.require(result.returncode == 0 and interaction.cycle == TOTAL_CYCLES
                 and len(lifecycle) == 1 and len(runtime) == 1
                 and "observationErrorType" not in runtime[0],
                 "retention capture incomplete")
    return RetentionCapture(output, lifecycle[0], runtime[0])
