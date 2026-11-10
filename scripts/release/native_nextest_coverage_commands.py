"""Keep execution and receipts bound to the same fixed root coverage plan."""

from __future__ import annotations

from pathlib import Path
import re
import uuid

REPORT_DIRECTORY = Path("tmp/quality/standalone-performance-coverage")

ROOT_TEST_ONLY_DIRECTORY = "crates/kdr-linter/src/bin/mathjax_consumer_facts"
ROOT_TEST_ONLY_SHA256 = {
    f"{ROOT_TEST_ONLY_DIRECTORY}/ast_linter_materialization_test_support.rs": "8da256b7b7eeee0856e7241441b59704b1cf11d5b5b00d96b79cfab926ad3909",
    f"{ROOT_TEST_ONLY_DIRECTORY}/bundle_private_ast_tests.rs": "e446403f7e6c5818cb4fb8fd6beb257619e5a1b21087ac0084807136ebff6e83",
    f"{ROOT_TEST_ONLY_DIRECTORY}/default_route_tests.rs": "10ebe13ddd4e1e3b6427a745fab6bcec62eb047ec5fe2123755c9ca3dfeeff46",
    f"{ROOT_TEST_ONLY_DIRECTORY}/default_materialize_private_ast_tests.rs": "dcf25f56d59e5c23f3183269fb44f599201e40435b47b257bcb65e75aae2dd84",
    f"{ROOT_TEST_ONLY_DIRECTORY}/macho_noso_tests_private.rs": "8c1893ff2f1ee0fb16ff5e20f9dfdcc18ba63e10fd27bd06086040be94d092d3",
    f"{ROOT_TEST_ONLY_DIRECTORY}/runtime_dispatch_private_ast_tests.rs": "be1196e1b64af1b7fbdcde58852782bcbb4479443d619cb213fd44689a8285f9",
    f"{ROOT_TEST_ONLY_DIRECTORY}/v8_route_test_support.rs": "df25cc83cb41909bc68d2b6edd5e26b5bff366f7c3a1fe057e9c3cab79aeee49",
}


def root_coverage_ignore_regex(repo: Path) -> str:
    def escaped_path(path: Path) -> str:
        parts = path.parts
        if path.anchor:
            parts = (parts[0].removesuffix(path.anchor[-1:]), *parts[1:])
        return r"[/\\]".join(re.escape(part) for part in parts)

    return "^(" + "|".join(escaped_path(repo / path) for path in ROOT_TEST_ONLY_SHA256) + ")$"


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
        "--show-missing-lines", "--ignore-filename-regex", root_coverage_ignore_regex(repo),
        "--output-path", str(repo / REPORT_DIRECTORY / "root.json"),
    ]
