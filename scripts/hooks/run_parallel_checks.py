from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import subprocess
import sys
import threading
from dataclasses import dataclass
from typing import TextIO


@dataclass(frozen=True)
class Lane:
    name: str
    recipe: str


LANES = (
    Lane("cargo", "check-rust"),
    Lane("assets", "check-assets"),
    Lane("contracts", "check-contracts"),
)


def stream_output(name: str, source: TextIO) -> None:
    for line in source:
        sys.stdout.write(f"[{name}] {line}")
        sys.stdout.flush()


def run_lane(lane: Lane) -> tuple[str, int]:
    process = subprocess.Popen(
        ["just", lane.recipe],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    assert process.stdout is not None
    output_thread = threading.Thread(
        target=stream_output,
        args=(lane.name, process.stdout),
        daemon=True,
    )
    output_thread.start()
    return_code = process.wait()
    output_thread.join()
    return lane.name, return_code


def run_lanes(lanes: tuple[Lane, ...], jobs: int) -> int:
    if jobs < 1:
        raise ValueError("jobs must be at least 1")

    # The fixed lane list is intentionally bounded: each lane owns distinct
    # mutable resources, while all Cargo and PlantUML commands stay together.
    results: dict[str, int] = {}
    with ThreadPoolExecutor(max_workers=min(jobs, len(lanes))) as executor:
        futures = [executor.submit(run_lane, lane) for lane in lanes]
        for future in as_completed(futures):
            name, return_code = future.result()
            results[name] = return_code

    failed = [(lane.name, results.get(lane.name, 1)) for lane in lanes if results.get(lane.name, 0) != 0]
    for name, return_code in failed:
        print(f"quality-check lane failed: {name} (exit {return_code})", file=sys.stderr)
    return failed[0][1] if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Run independent KRR quality-check lanes.")
    parser.add_argument("--jobs", type=int, default=3)
    arguments = parser.parse_args()
    if arguments.jobs < 1:
        parser.error("--jobs must be at least 1")
    return run_lanes(LANES, arguments.jobs)


if __name__ == "__main__":
    raise SystemExit(main())
