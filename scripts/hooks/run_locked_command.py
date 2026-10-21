from __future__ import annotations

from contextlib import ExitStack
from pathlib import Path
import subprocess
import sys

from local_review_lock import review_lock


ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / ".moon/cache/krr-terminal-report.lock"
ROOT_PRODUCER_LOCK_PATH = ROOT / "tmp/quality/standalone-performance-coverage/root-nextest-producer.lock"


def run_locked_command(
    arguments: list[str],
    lock_path: Path = LOCK_PATH,
    root_producer_lock_path: Path = ROOT_PRODUCER_LOCK_PATH,
) -> int:
    protect_root_producer = bool(arguments) and arguments[0] == "--protect-root-producer"
    command_arguments = arguments[1:] if protect_root_producer else arguments
    if not command_arguments or command_arguments[0] != "--" or len(command_arguments) == 1:
        print(
            "usage: run_locked_command.py [--protect-root-producer] -- COMMAND [ARG ...]",
            file=sys.stderr,
        )
        return 2
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with ExitStack() as locks:
        locks.enter_context(review_lock(lock_path))
        if protect_root_producer:
            # root coverageと同じ非待機ロックで、生成中の成果物をcleanから保護する。
            root_producer_lock_path.parent.mkdir(parents=True, exist_ok=True)
            locks.enter_context(review_lock(root_producer_lock_path))
        try:
            result = subprocess.run(command_arguments[1:], cwd=ROOT, check=False, shell=False)
        except OSError as error:
            print(f"unable to run locked command: {error}", file=sys.stderr)
            return 1
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(run_locked_command(sys.argv[1:]))
