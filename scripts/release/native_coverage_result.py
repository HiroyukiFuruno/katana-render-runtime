#!/usr/bin/env python3
"""Create and validate a privacy-safe KRR coverage-result summary."""

from __future__ import annotations

import hashlib
import importlib
import json
import math
import re
import stat
import subprocess
import sys
from collections.abc import Callable, Mapping
from pathlib import Path
from types import ModuleType
from typing import cast

from native_nextest_coverage_commands import (
    ROOT_TEST_ONLY_SHA256, nextest_arguments, report_arguments, root_coverage_ignore_regex,
)


SCHEMA = "krr-native-test-coverage-result-v1"
AUTHENTICITY = "unsigned-local-result"
COVERAGE_CONTEXT_SCHEMA = "krr-native-test-coverage-context-v1"
COVERAGE_CACHE_KEY_SCHEMA = "krr-native-test-coverage-cache-key-v1"
DOMAINS = ("root", "performance-baseline", "performance-candidate")
SOURCE_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "scripts/html/performance-harness/Cargo.toml",
    "scripts/html/performance-harness/Cargo.lock",
    "Justfile",
    "moon.yml",
    ".github/workflows/test-and-build.yml",
    "scripts/build/cargo_capacity_guard.py",
    "scripts/release/nextest-local-user.toml",
    "scripts/release/native_nextest_coverage_commands.py",
    *ROOT_TEST_ONLY_SHA256,
)
REPORT_DIRECTORY = "tmp/quality/standalone-performance-coverage"
COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
LLVM_VERSION_RE = re.compile(r"[0-9]+(?:\.[0-9]+){1,2}(?:[-+][A-Za-z0-9._-]+)?\Z")
SAFE_PROFILE_VALUE_RE = re.compile(r"[A-Za-z0-9_.+-]{1,80}\Z")
SAFE_TOOL_VALUE_RE = re.compile(r"[A-Za-z0-9 .()+_-]{1,120}\Z")
ENVIRONMENT_SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
PROFILE_KEYS = ("runnerOs", "runnerArch", "imageOS", "imageVersion", "environmentSha256", "testThreads")
TOOL_KEYS = ("rustc", "cargo", "cargoLlvmCov", "cargoNextest", "llvm", "java", "bun", "node", "python")

JsonObject = dict[str, object]


class CoverageResultError(ValueError):
    """Raised when a coverage report or its result manifest is invalid."""


class CoveragePolicyError(CoverageResultError):
    """Raised when authenticated coverage is below the fixed acceptance policy."""


def _object(value: object, label: str) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise CoverageResultError(f"{label} must be an object with string keys")
    return cast(JsonObject, value)


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise CoverageResultError(f"{label} must be an array")
    return value


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _read_repo_file(root: Path, relative: str) -> bytes:
    path = root / relative
    cursor = root
    for part in Path(relative).parts:
        cursor /= part
        try:
            mode = cursor.lstat().st_mode
        except OSError as error:
            raise CoverageResultError(f"required input is unavailable: {relative}") from error
        if stat.S_ISLNK(mode):
            raise CoverageResultError(f"input path contains a symlink: {relative}")
    if not stat.S_ISREG(path.lstat().st_mode):
        raise CoverageResultError(f"required input is not a regular file: {relative}")
    try:
        return path.read_bytes()
    except OSError as error:
        raise CoverageResultError(f"required input cannot be read: {relative}") from error


def _verify_tested_source(root: Path, tested_sha: str) -> None:
    try:
        head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, text=True, capture_output=True,
                              check=False)
        status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=all"], cwd=root,
                                text=True, capture_output=True, check=False)
        tracked = subprocess.run(["git", "ls-files", "--error-unmatch", *SOURCE_INPUTS], cwd=root,
                                 text=True, capture_output=True, check=False)
    except OSError as error:
        raise CoverageResultError("tested source Git identity is unavailable") from error
    if (head.returncode != 0 or head.stdout.strip() != tested_sha or status.returncode != 0
            or status.stdout.strip() or tracked.returncode != 0):
        raise CoverageResultError("tested SHA does not identify the clean, pinned source tree")


def _safe_values(value: object, keys: tuple[str, ...], pattern: re.Pattern[str], label: str) -> dict[str, str]:
    fields = _object(value, label)
    if set(fields) != set(keys):
        raise CoverageResultError(f"{label} fields differ from the privacy allowlist")
    result: dict[str, str] = {}
    for key in keys:
        item = fields[key]
        if not isinstance(item, str) or pattern.fullmatch(item) is None:
            raise CoverageResultError(f"{label} contains a value outside the privacy allowlist")
        result[key] = item
    return result


def _safe_profile_values(value: object, label: str) -> dict[str, str]:
    profile = _safe_values(value, PROFILE_KEYS, SAFE_PROFILE_VALUE_RE, label)
    if ENVIRONMENT_SHA256_RE.fullmatch(profile["environmentSha256"]) is None:
        raise CoverageResultError(f"{label} environment digest must be lowercase SHA-256")
    return profile


def _validate_coverage_policy(value: object) -> None:
    policy = _object(value, "coverage policy")
    if (set(policy) != {"linePercent", "unmeasuredLines"}
            or type(policy["linePercent"]) is not int or policy["linePercent"] != 100
            or type(policy["unmeasuredLines"]) is not int or policy["unmeasuredLines"] != 0):
        raise CoverageResultError("coverage policy differs from the fixed 100/0 requirement")


def parse_coverage_context(value: object) -> JsonObject:
    context = _object(value, "coverage context")
    if set(context) != {"schema", "testedSha", "workflowSha", "profile", "tools"}:
        raise CoverageResultError("coverage context fields differ from the fixed schema")
    tested_sha = context.get("testedSha")
    workflow_sha = context.get("workflowSha")
    if (context.get("schema") != COVERAGE_CONTEXT_SCHEMA
            or not isinstance(tested_sha, str) or COMMIT_RE.fullmatch(tested_sha) is None
            or not isinstance(workflow_sha, str) or COMMIT_RE.fullmatch(workflow_sha) is None):
        raise CoverageResultError("coverage context schema or commit identity is invalid")
    profile = _safe_profile_values(context.get("profile"), "profile")
    threads = profile["testThreads"]
    if threads != "default" and (not threads.isdecimal() or int(threads) < 1):
        raise CoverageResultError("test thread count must be positive or default")
    tools = _safe_values(context.get("tools"), TOOL_KEYS, SAFE_TOOL_VALUE_RE, "tools")
    return {"schema": COVERAGE_CONTEXT_SCHEMA, "testedSha": tested_sha, "workflowSha": workflow_sha,
            "profile": profile, "tools": tools}


def coverage_cache_key(root: Path, context_value: object) -> JsonObject:
    """Derive a deterministic key from exact context, commands, policy, and current source pins."""
    repo = root.resolve(strict=True)
    context = parse_coverage_context(context_value)
    tested_sha = cast(str, context["testedSha"])
    _verify_tested_source(repo, tested_sha)
    source_pins = _source_pins(repo)
    profile = cast(dict[str, str], context["profile"])
    commands = []
    for domain in DOMAINS:
        report_name = "root.json" if domain == "root" else f"{domain.removeprefix('performance-')}.json"
        commands.append({"domain": domain, **_domain_command(repo, domain, report_name, profile["testThreads"])})
    payload = {"schema": COVERAGE_CACHE_KEY_SCHEMA, "context": context,
               "coveragePolicy": {"linePercent": 100, "unmeasuredLines": 0},
               "sourceInputs": source_pins, "commands": commands}
    _verify_tested_source(repo, tested_sha)
    if source_pins != _source_pins(repo):
        raise CoverageResultError("pinned source inputs changed while deriving the cache key")
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return {"schema": COVERAGE_CACHE_KEY_SCHEMA, "key": _sha256(encoded)}


def _collected_line_totals(report: object, domain: str) -> JsonObject:
    root = _object(report, f"{domain} llvm-cov report")
    version = root.get("version")
    if (root.get("type") != "llvm.coverage.json.export"
            or not isinstance(version, str)
            or LLVM_VERSION_RE.fullmatch(version) is None):
        raise CoverageResultError(f"{domain} report does not use the llvm-cov export schema")
    data = _array(root.get("data"), f"{domain} report data")
    if not data:
        raise CoverageResultError(f"{domain} report has no coverage mappings")
    total_count = 0
    total_covered = 0
    for index, raw_item in enumerate(data):
        item = _object(raw_item, f"{domain} data[{index}]")
        totals = _object(item.get("totals"), f"{domain} data[{index}] totals")
        lines = _object(totals.get("lines"), f"{domain} data[{index}] lines")
        if set(lines) != {"count", "covered", "percent"}:
            raise CoverageResultError(f"{domain} line summary fields differ from llvm-cov schema")
        count = lines["count"]
        covered = lines["covered"]
        percent = lines["percent"]
        if (type(count) is not int or count < 0 or type(covered) is not int
                or covered < 0 or covered > count or isinstance(percent, bool)
                or not isinstance(percent, (int, float)) or not math.isfinite(percent)):
            raise CoverageResultError(f"{domain} line summary contains invalid values")
        expected_percent = 100.0 if count == covered and count > 0 else 0.0 if count == 0 else covered * 100.0 / count
        if not math.isclose(float(percent), expected_percent, rel_tol=0.0, abs_tol=0.005):
            raise CoverageResultError(f"{domain} line percentage disagrees with its counts")
        total_count += count
        total_covered += covered
    if total_count <= 0:
        raise CoverageResultError(f"{domain} coverage does not satisfy 100/0: no measured lines")
    unmeasured = total_count - total_covered
    percent = total_covered * 100.0 / total_count
    return {"count": total_count, "covered": total_covered, "unmeasured": unmeasured,
            "percent": percent}


def _line_totals(report: object, domain: str) -> JsonObject:
    totals = _collected_line_totals(report, domain)
    _require_complete_line_totals(totals, domain)
    totals["percent"] = 100.0
    return totals


def _require_complete_line_totals(totals: JsonObject, domain: str) -> None:
    if totals["covered"] != totals["count"] or totals["unmeasured"] != 0:
        raise CoveragePolicyError(f"{domain} coverage does not satisfy 100/0")


def validate_collection_policy(value: object, domains: tuple[str, ...]) -> None:
    """Apply strict 100/0 policy to already authenticated collection rows."""
    rows = _array(value, "collection domains")
    if len(rows) != len(domains):
        raise CoverageResultError("collection domains differ from the fixed producer")
    for domain, raw_row in zip(domains, rows, strict=True):
        row = _object(raw_row, f"{domain} collection result")
        lines = _object(row.get("lines"), f"{domain} collection line totals")
        if set(lines) != {"count", "covered", "unmeasured", "percent"}:
            raise CoverageResultError(f"{domain} collection line totals are malformed")
        _require_complete_line_totals(lines, domain)


def _capacity_guard_module() -> ModuleType:
    build_path = str(Path(__file__).resolve().parents[1] / "build")
    imported = sys.modules.get("cargo_capacity_guard")
    if imported is not None:
        module_path = getattr(imported, "__file__", None)
        if module_path is None or Path(module_path).resolve() != Path(build_path, "cargo_capacity_guard.py"):
            raise CoverageResultError("existing coverage command parser has an unexpected origin")
    else:
        sys.path.insert(0, build_path)
        try:
            imported = importlib.import_module("cargo_capacity_guard")
        except ImportError as error:
            raise CoverageResultError("existing coverage command parser is unavailable") from error
        finally:
            sys.path.remove(build_path)
    return imported


def _coverage_test_only_ignore_regex() -> str:
    value = getattr(_capacity_guard_module(), "COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX", None)
    if not isinstance(value, str):
        raise CoverageResultError("existing coverage source exclusion is unavailable")
    return value


def _coverage_selection() -> Callable[..., tuple[str, Path]]:
    imported = _capacity_guard_module()
    selector = getattr(imported, "coverage_selection", None)
    if not callable(selector):
        raise CoverageResultError("existing coverage command parser is unavailable")
    return cast(Callable[..., tuple[str, Path]], selector)


def _domain_command(root: Path, domain: str, report_name: str, threads: str) -> JsonObject:
    argv = ["cargo", "llvm-cov"]
    if domain == "root":
        argv = ["cargo", *report_arguments(root)]
        just_target = "root-test-and-coverage"
    else:
        feature = domain.removeprefix("performance-")
        argv.extend(["--manifest-path", "scripts/html/performance-harness/Cargo.toml", "--workspace",
                     "--all-targets", "--no-default-features", "--features", feature, "--locked",
                     "--profile", "krr-coverage", "--ignore-filename-regex",
                     _coverage_test_only_ignore_regex(), "--json"])
        just_target = "performance-harness-coverage"
        argv.extend(["--output-path", f"{REPORT_DIRECTORY}/{report_name}"])
        if threads != "default":
            argv.extend(["--", f"--test-threads={threads}"])
    selected = root / "target" / ("llvm-cov-target" if domain == "root" else f"llvm-cov-{domain}")
    environment = {"CARGO_TARGET_DIR": str(root / "target"), "CARGO_LLVM_COV_TARGET_DIR": str(selected),
                   "CARGO_HOME": str(root / "tmp/cargo-home")}
    try:
        # collection と strict の記録は同じcollector argvに束縛し、strict性は行数検証で判定する。
        action, actual_target = _coverage_selection()(root, root / "target", domain, argv, environment,
                                                      collection=True)
    except (OSError, RuntimeError) as error:
        raise CoverageResultError(f"{domain} command fails the existing coverage contract") from error
    if action != ("report" if domain == "root" else "test") or actual_target != selected:
        raise CoverageResultError(f"{domain} command does not select its fixed coverage target")
    manifest_argv = list(argv)
    if domain == "root":
        manifest_argv[manifest_argv.index("--output-path") + 1] = f"{REPORT_DIRECTORY}/{report_name}"
        manifest_argv[manifest_argv.index("--ignore-filename-regex") + 1] = root_coverage_ignore_regex(Path("<REPO>"))
    record: JsonObject = {
        "guardDomain": domain, "justTarget": just_target, "argv": manifest_argv,
        "selectedTarget": str(selected.relative_to(root)),
    }
    if domain == "root":
        test_argv = ["cargo", *nextest_arguments(root, "" if threads == "default" else threads, None)]
        test_argv[test_argv.index("--user-config-file") + 1] = "scripts/release/nextest-local-user.toml"
        record["testScope"] = {
            "initialArgv": test_argv,
            "retry": "authenticated-full-run-uuid",
            "artifacts": "complete-nextest-state-and-llvm-cov-target-trees",
        }
    return record


def _report_record(root: Path, domain: str, threads: str, *, collection: bool = False) -> JsonObject:
    name = "root.json" if domain == "root" else f"{domain.removeprefix('performance-')}.json"
    relative = f"{REPORT_DIRECTORY}/{name}"
    raw = _read_repo_file(root, relative)
    try:
        report = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CoverageResultError(f"{domain} report is not valid JSON") from error
    return {
        "reportName": name,
        "sha256": _sha256(raw),
        "bytes": len(raw),
        "command": _domain_command(root, domain, name, threads),
        "lines": (_collected_line_totals if collection else _line_totals)(report, domain),
    }


def _collection_report_record(root: Path, domain: str, threads: str) -> JsonObject:
    return _report_record(root, domain, threads, collection=True)


def _source_pins(root: Path) -> list[JsonObject]:
    return [{"path": name, "sha256": _sha256(_read_repo_file(root, name))} for name in SOURCE_INPUTS]


def _validate_manifest_domains(root: Path, rows: object, threads: str) -> None:
    domains = _array(rows, "coverage result domains")
    if len(domains) != len(DOMAINS):
        raise CoverageResultError("coverage result must contain exactly three domains")
    for domain, raw_row in zip(DOMAINS, domains, strict=True):
        row = _object(raw_row, f"{domain} result")
        report_name = "root.json" if domain == "root" else f"{domain.removeprefix('performance-')}.json"
        if set(row) != {"domain", "reportName", "sha256", "bytes", "command", "lines"}:
            raise CoverageResultError(f"{domain} result fields differ from the privacy allowlist")
        if row.get("domain") != domain or row.get("reportName") != report_name:
            raise CoverageResultError(f"{domain} result does not use its fixed report identity")
        digest = row.get("sha256")
        size = row.get("bytes")
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            raise CoverageResultError(f"{domain} output digest is invalid")
        if type(size) is not int or size <= 0:
            raise CoverageResultError(f"{domain} output size is invalid")
        lines = _object(row.get("lines"), f"{domain} line totals")
        if set(lines) != {"count", "covered", "unmeasured", "percent"}:
            raise CoverageResultError(f"{domain} line totals differ from the privacy allowlist")
        count = lines["count"]
        covered = lines["covered"]
        unmeasured = lines["unmeasured"]
        percent = lines["percent"]
        if (type(count) is not int or count <= 0 or covered != count or type(covered) is not int
                or unmeasured != 0 or type(unmeasured) is not int or percent != 100.0):
            raise CoverageResultError(f"{domain} coverage does not satisfy 100/0")
        expected_command = _domain_command(root, domain, report_name, threads)
        if row.get("command") != expected_command:
            raise CoverageResultError(f"{domain} command differs from the fixed coverage contract")


def create_coverage_result_manifest(
    root: Path,
    tested_sha: str,
    workflow_sha: str,
    profile: Mapping[str, str],
    tools: Mapping[str, str],
) -> JsonObject:
    """Summarize the three standard reports without copying report paths or contents."""
    repo = root.resolve(strict=True)
    if COMMIT_RE.fullmatch(tested_sha) is None or COMMIT_RE.fullmatch(workflow_sha) is None:
        raise CoverageResultError("tested and workflow SHA values must remain separate full commit IDs")
    _verify_tested_source(repo, tested_sha)
    if not isinstance(profile, Mapping) or not isinstance(tools, Mapping):
        raise CoverageResultError("profile and tools must be mappings")
    normalized_profile = _safe_profile_values(profile, "profile")
    threads = normalized_profile["testThreads"]
    if threads != "default" and (not threads.isdecimal() or int(threads) < 1):
        raise CoverageResultError("test thread count must be positive or default")
    source_pins = _source_pins(repo)
    manifest: JsonObject = {
        "schema": SCHEMA,
        "authenticity": AUTHENTICITY,
        "testedSha": tested_sha,
        "workflowSha": workflow_sha,
        "coveragePolicy": {"linePercent": 100, "unmeasuredLines": 0},
        "sourceInputs": source_pins,
        "profile": normalized_profile,
        "tools": _safe_values(tools, TOOL_KEYS, SAFE_TOOL_VALUE_RE, "tools"),
        "domains": [{"domain": domain, **_report_record(repo, domain, threads)} for domain in DOMAINS],
    }
    _verify_tested_source(repo, tested_sha)
    if source_pins != _source_pins(repo):
        raise CoverageResultError("pinned source inputs changed while reading coverage reports")
    return manifest


def verify_coverage_result_manifest(
    value: object,
    root: Path,
    expected_tested_sha: str,
    expected_workflow_sha: str,
    expected_profile: Mapping[str, str],
    expected_tools: Mapping[str, str],
) -> None:
    """Validate a sanitized result without requiring the raw coverage reports."""
    repo = root.resolve(strict=True)
    manifest = _object(value, "coverage result manifest")
    if set(manifest) != {"schema", "authenticity", "testedSha", "workflowSha", "coveragePolicy",
                         "sourceInputs", "profile", "tools", "domains"}:
        raise CoverageResultError("manifest fields differ from the privacy allowlist")
    tested_sha = manifest.get("testedSha")
    workflow_sha = manifest.get("workflowSha")
    if (COMMIT_RE.fullmatch(expected_tested_sha) is None
            or COMMIT_RE.fullmatch(expected_workflow_sha) is None):
        raise CoverageResultError("expected tested and workflow SHA values must be full commit IDs")
    if not isinstance(tested_sha, str) or not isinstance(workflow_sha, str):
        raise CoverageResultError("tested and workflow SHA values must remain separate full commit IDs")
    if tested_sha != expected_tested_sha or workflow_sha != expected_workflow_sha:
        raise CoverageResultError("manifest tested or workflow SHA differs from expected identity")
    if manifest.get("schema") != SCHEMA or manifest.get("authenticity") != AUTHENTICITY:
        raise CoverageResultError("coverage result schema or authenticity marker is invalid")
    _validate_coverage_policy(manifest.get("coveragePolicy"))
    expected_profile_values = _safe_profile_values(expected_profile, "expected profile")
    expected_tool_values = _safe_values(expected_tools, TOOL_KEYS, SAFE_TOOL_VALUE_RE, "expected tools")
    received_profile = _safe_profile_values(manifest.get("profile"), "profile")
    received_tools = _safe_values(manifest.get("tools"), TOOL_KEYS, SAFE_TOOL_VALUE_RE, "tools")
    if received_profile != expected_profile_values or received_tools != expected_tool_values:
        raise CoverageResultError("manifest profile or tools differ from independently expected values")
    threads = expected_profile_values["testThreads"]
    if threads != "default" and (not threads.isdecimal() or int(threads) < 1):
        raise CoverageResultError("test thread count must be positive or default")
    _verify_tested_source(repo, expected_tested_sha)
    if manifest.get("sourceInputs") != _source_pins(repo):
        raise CoverageResultError("manifest source inputs differ from current pinned source files")
    _validate_manifest_domains(repo, manifest.get("domains"), threads)
    _verify_tested_source(repo, expected_tested_sha)


def verify_local_coverage_result_manifest(
    value: object,
    root: Path,
    expected_tested_sha: str,
    expected_workflow_sha: str,
    expected_profile: Mapping[str, str],
    expected_tools: Mapping[str, str],
) -> None:
    """Additionally compare the manifest with the three local llvm-cov reports."""
    repo = root.resolve(strict=True)
    verify_coverage_result_manifest(
        value, repo, expected_tested_sha, expected_workflow_sha, expected_profile, expected_tools,
    )
    manifest = _object(value, "coverage result manifest")
    threads = expected_profile["testThreads"]
    expected_domains = [
        {"domain": domain, **_report_record(repo, domain, threads)} for domain in DOMAINS
    ]
    if manifest.get("domains") != expected_domains:
        raise CoverageResultError("coverage result manifest differs from current reports")
    _verify_tested_source(repo, expected_tested_sha)
