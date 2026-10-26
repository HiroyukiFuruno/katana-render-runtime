"""Keep execution and receipts bound to the same fixed root coverage plan."""

from __future__ import annotations

from pathlib import Path
import uuid

REPORT_DIRECTORY = Path("tmp/quality/standalone-performance-coverage")


def clean_arguments() -> list[str]:
    return ["llvm-cov", "clean", "--workspace", "--profraw-only"]


def nextest_arguments(repo: Path, threads: str, parent: str | None) -> list[str]:
    arguments = [
        "llvm-cov", "nextest", "--workspace", "--all-targets", "--all-features",
        "--locked", "--cargo-profile", "krr-coverage", "--no-report",
        "--no-fail-fast", "--retries", "0", "--user-config-file",
        str(repo / "scripts/release/nextest-local-user.toml"),
    ]
    if threads:
        if not threads.isdecimal() or int(threads) < 1:
            raise ValueError("TEST_THREADS must be positive or empty")
        arguments.append(f"--test-threads={threads}")
    if parent is not None:
        if str(uuid.UUID(parent)) != parent:
            raise ValueError("retry requires a complete canonical run UUID")
        arguments.extend(["-R", parent])
    return arguments


def report_arguments(repo: Path) -> list[str]:
    return [
        "llvm-cov", "report", "--workspace", "--profile", "krr-coverage", "--locked", "--json",
        "--show-missing-lines", "--output-path", str(repo / REPORT_DIRECTORY / "root.json"),
    ]
