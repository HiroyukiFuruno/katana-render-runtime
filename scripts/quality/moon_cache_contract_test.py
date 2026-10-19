"""既存moonの結果cacheを、KRR専用の小fixtureで契約検証する。"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import time
from pathlib import Path
from typing import TypedDict

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_SOURCE = ROOT / "tests/fixtures/quality-cache"
ASSESSMENT = ROOT / "tmp/html-fidelity/test-cache-library-assessment-20261012-r1"
DIRECT_INPUT_EVIDENCE = ROOT / "tmp/quality/moon-tmp-durable-regression-r1"
DIRECT_INPUTS: list[str] = [
    "**/*",
    "!/tmp/**",
    "!/.moon/cache/**",
    "!/target/**",
    "!/crates/katana-render-runtime/target/**",
    "!**/.ruff_cache/**",
    "!**/.mypy_cache/**",
    "!**/.pytest_cache/**",
    "!/tests/fixtures/quality-cache/.moon/cache/**",
    "!/tests/fixtures/quality-cache/.outputs/**",
]
RUNTIME_UNUSED_LEAF_IDS = (
    "runtime-unused-fragment-loader",
    "runtime-unused-bundle-definitions",
    "runtime-unused-check",
    "runtime-unused-tool-fingerprint",
    "runtime-unused-mathjax",
    "runtime-unused-native-width",
    "runtime-unused-rust-classic",
)
SCHEDULER_DEPENDENCIES: dict[str, list[str]] = {
    "unit-test": ["runtime-asset-script-test"],
    "runtime-asset-script-test": [
        "runtime-asset-script-other",
        "runtime-unused-script-regression",
    ],
    "runtime-asset-script-other": ["runtime-classic-generation-evidence"],
    "runtime-unused-fragment-loader-producer": ["runtime-classic-generation-evidence"],
    "runtime-unused-bundle-definitions-producer": [
        "runtime-unused-fragment-loader-producer"
    ],
    "runtime-unused-check-producer": ["runtime-unused-bundle-definitions-producer"],
    "runtime-unused-tool-fingerprint-producer": ["runtime-unused-check-producer"],
    "runtime-unused-mathjax-producer": ["runtime-unused-tool-fingerprint-producer"],
    "runtime-unused-native-width-producer": ["runtime-unused-mathjax-producer"],
    "runtime-unused-rust-classic-producer": ["runtime-unused-native-width-producer"],
    "runtime-unused-script-regression": ["runtime-unused-rust-classic-producer"],
    "runtime-classic-generation-evidence": ["runtime-asset-script-preflight"],
    "runtime-asset-script-preflight": ["mathjax-owned-evidence"],
    "mathjax-owned-evidence": ["static", "rust-facts-host-prebuild"],
    "rust-facts-host-prebuild": ["capacity-admission"],
    "capacity-admission": ["source-static-foundation"],
    "static": ["source-static", "dependency-leak"],
    "source-static": ["source-static-foundation", "biome"],
    "source-static-foundation": [
        "runtime-bundle-check",
        "runtime-asset-check",
        "fmt-check",
        "typecheck",
        "python-lint",
        "python-typecheck",
        "duplication-check",
        "html-harness-contract-check",
        "owned-validation-contract-check",
        "mathjax-import-pin-preflight",
    ],
    "mathjax-import-pin-preflight": [],
    "runtime-bundle-check": [],
    "runtime-asset-check": [],
    "biome": ["rust-facts-host-prebuild"],
    "typecheck": [],
    "python-lint": [],
    "python-typecheck": [],
    "duplication-check": [],
    "html-harness-contract-check": [],
    "owned-validation-contract-check": [],
    "dependency-leak": ["lint"],
    "ast-lint": ["fmt-check", "source-static"],
    "lint": ["ast-lint"],
    "fmt-check": [],
}
SCHEDULER_NOOP_TASKS = {
    "static",
    "source-static",
    "source-static-foundation",
}
SCHEDULER_TASK_OPTIONS: dict[str, str | bool] = {
    "cache": False,
    "runInCI": "always",
    "shell": False,
    "allowFailure": False,
    "expectFailure": False,
}
SCHEDULER_EVENT_PHASES = (
    "runtime-bundle-check",
    "runtime-asset-check",
    "fmt-check",
    "typecheck",
    "python-lint",
    "python-typecheck",
    "duplication-check",
    "html-harness-contract-check",
    "owned-validation-contract-check",
    "mathjax-import-pin-preflight",
    "capacity-admission",
    "rust-facts-host-prebuild",
    "static",
    "ast-lint",
    "lint",
    "dependency-leak",
    "mathjax-owned-evidence",
    "runtime-asset-script-preflight",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-other",
    "runtime-unused-fragment-loader-producer",
    "runtime-unused-bundle-definitions-producer",
    "runtime-unused-check-producer",
    "runtime-unused-tool-fingerprint-producer",
    "runtime-unused-mathjax-producer",
    "runtime-unused-native-width-producer",
    "runtime-unused-rust-classic-producer",
    "runtime-unused-script-regression",
    "runtime-asset-script-test",
    "unit-test",
)
SCHEDULER_FOUNDATION_PHASES = SCHEDULER_EVENT_PHASES[:10]
SCHEDULER_PRE_LINT_PHASES = SCHEDULER_EVENT_PHASES[:13]
SCHEDULER_BARRIER_PHASES = (
    "lint",
    "ast-lint",
    "dependency-leak",
    "mathjax-owned-evidence",
    "runtime-asset-script-preflight",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-other",
    "runtime-unused-fragment-loader-producer",
    "runtime-unused-bundle-definitions-producer",
    "runtime-unused-check-producer",
    "runtime-unused-tool-fingerprint-producer",
    "runtime-unused-mathjax-producer",
    "runtime-unused-native-width-producer",
    "runtime-unused-rust-classic-producer",
    "runtime-unused-script-regression",
    "runtime-asset-script-test",
    "unit-test",
)
SCHEDULER_NATIVE_PHASES = (
    "runtime-asset-script-preflight",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-other",
    "runtime-unused-fragment-loader-producer",
    "runtime-unused-bundle-definitions-producer",
    "runtime-unused-check-producer",
    "runtime-unused-tool-fingerprint-producer",
    "runtime-unused-mathjax-producer",
    "runtime-unused-native-width-producer",
    "runtime-unused-rust-classic-producer",
    "runtime-unused-script-regression",
    "runtime-asset-script-test",
    "unit-test",
)
MOON_TASK_CONTEXT_VARIABLES = (
    "MOON_CACHE_DIR",
    "MOON_PROJECT_ID",
    "MOON_PROJECT_ROOT",
    "MOON_PROJECT_SOURCE",
    "MOON_PROJECT_SNAPSHOT",
    "MOON_TARGET",
    "MOON_TASK_ID",
    "MOON_TASK_HASH",
    "MOON_WORKSPACE_ROOT",
    "MOON_WORKING_DIR",
)


class OptionalFixtureTask(TypedDict, total=False):
    outputs: list[str]
    options: dict[str, str | bool]
    deps: list[str]
    toolchains: list[str]
    checks: list[dict[str, str]]


class FixtureTask(OptionalFixtureTask):
    command: str | list[str]
    inputs: list[str]
    env: dict[str, str]


class DirectInputRunRecord(TypedDict):
    label: str
    status: str
    actionStatuses: list[str]
    exit_code: int
    hash_prefix: str
    hash: str
    target_state: str
    execution_count: int
    duration_ms: int
    log: str
    log_sha256: str
    report: str
    report_sha256: str
    hash_manifest: str
    hash_manifest_sha256: str
    input_paths: list[str]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def moon_json_object(value: object) -> dict[str, object]:
    assert isinstance(value, dict), "Moon JSON object is required"
    assert all(isinstance(key, str) for key in value), "Moon JSON keys must be strings"
    return {str(key): item for key, item in value.items()}


def validate_root_test_coverage_contract(declared: dict[str, object]) -> None:
    unit = moon_json_object(declared.get("unit-test"))
    coverage = moon_json_object(declared.get("coverage"))
    performance_task = moon_json_object(declared.get("performance-coverage"))
    assert unit.get("command") == ["just", "root-test-and-coverage"]
    assert unit.get("deps") == ["runtime-asset-script-test"]
    assert unit.get("outputs") == [
        "tmp/quality/standalone-performance-coverage/root.json",
        "tmp/quality/standalone-performance-coverage/root-local-result.json",
    ]
    assert unit.get("checks") == [
        {
            "check": "fingerprint",
            "script": "just native-coverage-cache-key root",
            "hash": "stdout",
        }
    ]
    assert coverage.get("command") == ["just", "coverage-result-check"]
    assert coverage.get("deps") == ["performance-coverage"]
    assert performance_task.get("command") == ["just", "performance-harness-coverage"]
    assert performance_task.get("deps") == ["heavy-native", "check"]
    assert performance_task.get("outputs") == [
        "tmp/quality/standalone-performance-coverage/baseline.json",
        "tmp/quality/standalone-performance-coverage/candidate.json",
        "tmp/quality/standalone-performance-coverage/performance-local-result.json",
    ]
    assert performance_task.get("checks") == [
        {
            "check": "fingerprint",
            "script": "just native-coverage-cache-key performance",
            "hash": "stdout",
        }
    ]
    justfile = (ROOT / "Justfile").read_text()
    assert (
        "unit-test: plantuml-install\n    {{CARGO}} test --workspace --all-targets --all-features --locked{{TEST_THREAD_ARGS}}"
        in justfile
    )
    assert "root-test-and-coverage: plantuml-install\n" in justfile
    producer = justfile.split("\nroot-test-and-coverage: plantuml-install\n", 1)[
        1
    ].split("\n\n", 1)[0]
    guard = 'CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-target" python3 scripts/build/cargo_capacity_guard.py --coverage-domain root -- {{CARGO}}'
    collection_guard = 'NEXTEST_STATE_DIR="{{REPO_ROOT}}/target/nextest-root-state" LLVM_PROFILE_FILE_NAME="krr-nextest-%4m.profraw" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-target" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain root -- {{CARGO}}'
    assert producer.splitlines() == [
        '    mkdir -p "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage"',
        '    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-begin --producer root',
        f"    {guard} llvm-cov clean --workspace",
        f'    {collection_guard} llvm-cov nextest --workspace --all-targets --all-features --locked --cargo-profile krr-coverage --no-fail-fast --retries 0 --user-config-file "{{{{REPO_ROOT}}}}/scripts/release/nextest-local-user.toml" --json --summary-only --output-path "{{{{REPO_ROOT}}}}/tmp/quality/standalone-performance-coverage/root.json"{{{{NEXTEST_THREAD_ARGS}}}}',
        '    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collect-finish --producer root --producer-exit 0',
    ]
    coverage_recipe = justfile.split("\ncoverage:", 1)[1].split("\n\n", 1)[0]
    coverage_commands = [line for line in coverage_recipe.splitlines() if line]
    assert coverage_commands == [
        "    just root-test-and-coverage",
        "    just performance-harness-coverage",
        "    just coverage-result-check",
    ]
    performance = justfile.split("\nperformance-harness-coverage:", 1)[1].split(
        "\n\n", 1
    )[0]
    assert performance.count(" llvm-cov --manifest-path ") == 2
    assert (
        "--features baseline" in performance and "--features candidate" in performance
    )


def production_task_input_binding(
    bun: Path, moon_command: list[str], evidence_dir: Path
) -> dict[str, object]:
    config_path = ROOT / "moon.yml"
    source_sha256 = digest(config_path)
    declared_output = subprocess.check_output(
        [
            str(bun),
            "-e",
            (
                "const config = Bun.YAML.parse(await Bun.file(process.argv[1]).text()); "
                "console.log(JSON.stringify(config.tasks));"
            ),
            str(config_path),
        ],
        text=True,
    )
    declared_path = evidence_dir / "production-declared-inputs.json"
    declared_path.write_text(declared_output)
    declared = moon_json_object(json.loads(declared_output))
    assert len(declared) == 44, "Production Moon task count changed"
    validate_root_test_coverage_contract(declared)
    runtime_preflight = moon_json_object(declared.get("runtime-asset-script-preflight"))
    classic_evidence = moon_json_object(
        declared.get("runtime-classic-generation-evidence")
    )
    runtime_other = moon_json_object(declared.get("runtime-asset-script-other"))
    runtime_unused = moon_json_object(declared.get("runtime-unused-script-regression"))
    runtime_aggregate = moon_json_object(declared.get("runtime-asset-script-test"))
    assert runtime_preflight.get("deps") == ["mathjax-owned-evidence"]
    assert classic_evidence.get("deps") == ["runtime-asset-script-preflight"]
    assert runtime_other.get("deps") == ["runtime-classic-generation-evidence"]
    assert runtime_unused.get("deps") == ["runtime-unused-rust-classic-producer"]
    assert runtime_unused.get("command") == [
        "just",
        "runtime-unused-script-final-verify-run",
    ]
    pin_preflight = moon_json_object(declared.get("mathjax-import-pin-preflight"))
    assert pin_preflight.get("command") == ["just", "mathjax-import-pin-preflight"]
    assert pin_preflight.get("deps", []) == []
    foundation = moon_json_object(declared.get("source-static-foundation"))
    foundation_deps = foundation.get("deps")
    assert isinstance(foundation_deps, list)
    assert "mathjax-import-pin-preflight" in foundation_deps
    for index, leaf in enumerate(RUNTIME_UNUSED_LEAF_IDS):
        producer = moon_json_object(declared.get(f"{leaf}-producer"))
        previous = (
            "runtime-classic-generation-evidence"
            if index == 0
            else f"{RUNTIME_UNUSED_LEAF_IDS[index - 1]}-producer"
        )
        assert producer.get("deps") == [previous]
        assert producer.get("command") == [
            "just",
            "runtime-unused-script-producer-run",
            leaf,
        ]
        assert producer.get("outputs") == [
            f"tmp/quality/runtime-script-results/{leaf}.json",
            f"tmp/quality/runtime-script-results/{leaf}-current-group-001.xml",
        ]
        assert moon_json_object(producer.get("options")).get("cache") == "local"
        assert producer.get("checks") == [
            {
                "check": "fingerprint",
                "script": f"just runtime-unused-script-cache-key {leaf}",
                "hash": "stdout",
            }
        ]
    assert runtime_aggregate.get("deps") == [
        "runtime-asset-script-other",
        "runtime-unused-script-regression",
    ]
    for name, raw in declared.items():
        task = moon_json_object(raw)
        assert task.get("inputs") == DIRECT_INPUTS, (
            f"Production task {name} does not use exact generated-cache exclusions"
        )
    query_output = subprocess.check_output(
        [
            *moon_command,
            "query",
            "tasks",
            "--project",
            "krr",
            "--cache",
            "read",
            "--color=false",
        ],
        cwd=ROOT,
        text=True,
    )
    query_path = evidence_dir / "production-expanded-tasks.json"
    query_path.write_text(query_output)
    query = moon_json_object(json.loads(query_output))
    projects = moon_json_object(query.get("tasks"))
    tasks = moon_json_object(projects.get("krr"))
    assert set(tasks) == set(declared), (
        "Actual Moon task names differ from current YAML"
    )
    # Moon自身が追加する設定入力を保持し、宣言した入力だけとの混同を防ぐ。
    config_glob = "/.moon/*.{yml,yaml,jsonc,json,pkl,hcl,toml}"
    expected = [*DIRECT_INPUTS, config_glob]
    for name, raw in tasks.items():
        task = moon_json_object(raw)
        inputs = task.get("inputs")
        assert isinstance(inputs, list), f"Actual Moon inputs missing: {name}"
        globs = [moon_json_object(item).get("glob") for item in inputs]
        assert globs == expected, f"Actual Moon input expansion differs: {name} {globs}"
    source_sha256_after = digest(config_path)
    assert source_sha256_after == source_sha256, (
        "Production Moon YAML changed during query"
    )
    return {
        "accepted": True,
        "task_count": len(tasks),
        "task_names": sorted(tasks),
        "declared_inputs": DIRECT_INPUTS,
        "expanded_inputs": expected,
        "source_sha256": source_sha256,
        "source_sha256_before": source_sha256,
        "source_sha256_after": source_sha256_after,
        "declared": str(declared_path),
        "declared_sha256": digest(declared_path),
        "query": str(query_path),
        "query_sha256": digest(query_path),
    }


def direct_input_run_record(
    workspace_dir: Path,
    evidence_dir: Path,
    label: str,
    completed: subprocess.CompletedProcess[str],
    executions: Path,
    duration_ms: int,
) -> DirectInputRunRecord:
    log = evidence_dir / f"{label}.log"
    log.write_text(completed.stdout + completed.stderr)
    report_path = evidence_dir / f"{label}.runReport.json"
    shutil.copyfile(workspace_dir / ".moon/cache/runReport.json", report_path)
    report = moon_json_object(json.loads(report_path.read_text()))
    assert completed.returncode == 0 and report.get("status") == "completed", (
        f"{label}: Moon did not complete successfully; see {log}"
    )
    context = moon_json_object(report.get("context"))
    states = moon_json_object(context.get("targetStates"))
    target = moon_json_object(states.get("probe:hash-probe"))
    task_hash = target.get("hash")
    assert isinstance(task_hash, str) and re.fullmatch(r"[0-9a-f]{64}", task_hash), (
        f"{label}: genuine Moon full task hash missing"
    )
    assert target.get("state") == "passed", f"{label}: Moon target did not pass"
    actions = report.get("actions")
    assert isinstance(actions, list), f"{label}: Moon actions missing"
    matched = [
        moon_json_object(item)
        for item in actions
        if moon_json_object(item).get("label") == "RunTask(probe:hash-probe)"
    ]
    assert len(matched) == 1, f"{label}: expected exactly one Moon target action"
    action = matched[0]
    action_status = action.get("status")
    assert isinstance(action_status, str) and action_status in {"passed", "cached"}, (
        f"{label}: Moon action did not pass or reuse success"
    )
    operations = action.get("operations")
    assert isinstance(operations, list), f"{label}: Moon operations missing"
    generated_hashes = []
    for raw in operations:
        operation = moon_json_object(raw)
        meta = moon_json_object(operation.get("meta"))
        if meta.get("type") == "hash-generation":
            assert operation.get("status") == "passed", (
                f"{label}: hash generation failed"
            )
            generated_hashes.append(meta.get("hash"))
    assert generated_hashes == [task_hash], (
        f"{label}: generated hash differs from target"
    )
    manifest_path = evidence_dir / f"{label}.hashManifest.json"
    shutil.copyfile(
        workspace_dir / ".moon/cache/blobs" / task_hash[:2] / task_hash[2:],
        manifest_path,
    )
    manifest: object = json.loads(manifest_path.read_text())
    assert isinstance(manifest, list) and len(manifest) == 1, (
        f"{label}: expected one genuine Moon hash manifest"
    )
    inputs = moon_json_object(moon_json_object(manifest[0]).get("inputs"))
    required = {
        ".moon/workspace.yml",
        "moon.yml",
        "source.txt",
        "scripts/tmp/managed.txt",
        "scripts/.moon/cache/managed.txt",
    }
    assert required <= set(inputs), f"{label}: managed inputs escaped Moon hashing"
    excluded = (
        "tmp/",
        ".moon/cache/",
        "target/",
        "crates/katana-render-runtime/target/",
        "tests/fixtures/quality-cache/.moon/cache/",
        "tests/fixtures/quality-cache/.outputs/",
    )
    assert not any(
        name.startswith(excluded)
        or any(
            f"/{cache}/" in f"/{name}"
            for cache in (".ruff_cache", ".mypy_cache", ".pytest_cache")
        )
        for name in inputs
    ), f"{label}: generated inputs entered Moon hashing"
    count = len(executions.read_text().splitlines()) if executions.exists() else 0
    return {
        "label": label,
        "status": "completed",
        "actionStatuses": [action_status],
        "target_state": "passed",
        "exit_code": completed.returncode,
        "hash_prefix": task_hash[:8],
        "hash": task_hash,
        "execution_count": count,
        "duration_ms": duration_ms,
        "log": str(log),
        "log_sha256": digest(log),
        "report": str(report_path),
        "report_sha256": digest(report_path),
        "hash_manifest": str(manifest_path),
        "hash_manifest_sha256": digest(manifest_path),
        "input_paths": sorted(inputs),
    }


def scheduler_success_order_valid(phases: list[str]) -> bool:
    if len(phases) != len(SCHEDULER_EVENT_PHASES) or set(phases) != set(
        SCHEDULER_EVENT_PHASES
    ):
        return False
    positions = {phase: phases.index(phase) for phase in phases}
    return (
        all(
            positions[phase] < positions["capacity-admission"]
            for phase in SCHEDULER_FOUNDATION_PHASES
        )
        and positions["capacity-admission"] < positions["rust-facts-host-prebuild"]
        and positions["rust-facts-host-prebuild"]
        < positions["static"]
        < positions["ast-lint"]
        < positions["lint"]
        < positions["dependency-leak"]
        < positions["mathjax-owned-evidence"]
        < positions["runtime-asset-script-preflight"]
        < positions["runtime-classic-generation-evidence"]
        and positions["runtime-classic-generation-evidence"]
        < positions["runtime-asset-script-other"]
        and positions["runtime-asset-script-other"]
        < positions["runtime-asset-script-test"]
        and positions["runtime-classic-generation-evidence"]
        < positions["runtime-unused-fragment-loader-producer"]
        < positions["runtime-unused-bundle-definitions-producer"]
        < positions["runtime-unused-check-producer"]
        < positions["runtime-unused-tool-fingerprint-producer"]
        < positions["runtime-unused-mathjax-producer"]
        < positions["runtime-unused-native-width-producer"]
        < positions["runtime-unused-rust-classic-producer"]
        < positions["runtime-unused-script-regression"]
        < positions["runtime-asset-script-test"]
        < positions["unit-test"]
    )


def scheduler_failure_barrier_is_closed(phases: list[str]) -> bool:
    return all(phase not in phases for phase in SCHEDULER_BARRIER_PHASES)


def initialize_isolated_git(workspace_dir: Path) -> None:
    subprocess.run(
        ["git", "init", "--quiet", "--initial-branch=master", str(workspace_dir)],
        check=True,
    )
    git = ["git", "-C", str(workspace_dir)]
    subprocess.run([*git, "config", "user.name", "Moon Cache Contract"], check=True)
    subprocess.run(
        [*git, "config", "user.email", "moon-cache-contract@example.invalid"],
        check=True,
    )
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run(
        [*git, "commit", "--quiet", "-m", "Create isolated Moon fixture"], check=True
    )


def main() -> int:
    if not __debug__:
        raise RuntimeError("Moon owner requires enabled Python assertions")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--moon", nargs="+", required=True)
    parser.add_argument("--bun", type=Path, default=Path(shutil.which("bun") or "bun"))
    parser.add_argument("--cas-integrity-only", action="store_true")
    args = parser.parse_args()
    moon_command: list[str] = args.moon
    bun = args.bun.resolve()
    assert bun.is_file(), "Bun executable is required"
    run_dir = ASSESSMENT / f"moon-contract-{time.time_ns()}"
    run_dir.mkdir(parents=True)
    direct_run_dir = DIRECT_INPUT_EVIDENCE / f"run-{time.time_ns()}"
    direct_run_dir.mkdir(parents=True)
    fixture_root = run_dir / "fixture"
    fixture_root.mkdir()
    fixture_sources = sorted(FIXTURE_SOURCE.glob("*.ts"))
    tracked_fixture_sources = [FIXTURE_SOURCE / ".gitignore", *fixture_sources]
    source_bytes = {path: path.read_bytes() for path in tracked_fixture_sources}
    source_pins = {
        str(path): hashlib.sha256(content).hexdigest()
        for path, content in source_bytes.items()
    }
    for source in tracked_fixture_sources:
        (fixture_root / source.name).write_bytes(source_bytes[source])
    originals = {
        fixture_root / source.name: source_bytes[source] for source in fixture_sources
    }
    initialize_isolated_git(fixture_root)

    def assert_source_fixture_unchanged() -> None:
        changed = [
            path
            for path, expected in source_bytes.items()
            if Path(path).read_bytes() != expected
        ]
        assert not changed, f"Tracked Moon fixture sources changed: {changed}"

    probe = fixture_root / ".probe"
    probe.mkdir(exist_ok=True)
    (fixture_root / ".moon").mkdir(exist_ok=True)
    events_path = probe / "events.jsonl"
    events_path.write_text("")
    results: list[dict[str, object]] = []
    config_path, lock_path = probe / "config.json", probe / "lock.json"
    config = {"revision": "baseline", "staticFailure": False}
    config_path.write_text(json.dumps(config))
    lock_path.write_text(json.dumps({"revision": "baseline"}))
    workspace = {
        "projects": {"fixture": "."},
        "vcs": {"client": "git", "defaultBranch": "master", "sync": False},
    }
    (fixture_root / ".moon/workspace.yml").write_text(json.dumps(workspace))
    environment = os.environ.copy()
    # 入れ子のMoonには外側taskのworkspace/project情報を引き継がない。
    for variable in MOON_TASK_CONTEXT_VARIABLES:
        environment.pop(variable, None)
    environment.pop("MOON_CACHE_CAS_VERIFY_INTEGRITY", None)
    environment.pop("MOON_EXPERIMENT_CAS_OUTPUTS_CACHE", None)
    environment.update(
        {
            "MOON_TELEMETRY": "false",
            "MOON_CACHE": "read-write",
            "CACHE_CONTRACT_ENV": "baseline",
            "MOON_AUTO_INSTALL": "false",
            "PWD": str(fixture_root),
        }
    )
    inputs = [
        "moon-cache-subject.ts",
        "moon-cache-subject.test.ts",
        "moon-cache-task.ts",
        ".probe/config.json",
        ".probe/lock.json",
        "$CACHE_CONTRACT_ENV",
    ]
    common_env = {"CONTRACT_ROOT": str(fixture_root)}

    def settings(extra_key: str = "baseline", *, cas_integrity: bool = False) -> None:
        tasks: dict[str, FixtureTask] = {
            "unit": {
                "command": [str(bun), "test", "./moon-cache-subject.test.ts"],
                "inputs": inputs,
                "outputs": [".outputs/unit.json"],
                "env": {**common_env, "CONTRACT_PHASE": "unit"},
            },
            "coverage": {
                "command": [
                    str(bun),
                    "test",
                    "--coverage",
                    "--coverage-reporter=lcov",
                    "--coverage-dir",
                    ".outputs/coverage",
                    "./moon-cache-subject.test.ts",
                ],
                "inputs": inputs,
                "outputs": [".outputs/coverage.json", ".outputs/coverage/lcov.info"],
                "env": {**common_env, "CONTRACT_PHASE": "coverage"},
            },
            "static": {
                "command": [str(bun), "run", "moon-cache-task.ts", "static"],
                "inputs": inputs,
                "env": common_env,
                "options": {"cache": False},
            },
            "heavy": {
                "command": [str(bun), "run", "moon-cache-task.ts", "heavy"],
                "inputs": inputs,
                "env": common_env,
                "deps": ["static"],
                "options": {"cache": False},
            },
            "failure": {
                "command": [str(bun), "run", "moon-cache-task.ts", "failure"],
                "inputs": inputs,
                "env": common_env,
            },
        }
        for task in tasks.values():
            task["toolchains"] = ["system"]
            task["checks"] = [
                {"check": "fingerprint", "script": f"{bun} --version", "hash": "stdout"}
            ]
            task.setdefault("options", {})["cacheKey"] = f"{run_dir.name}-{extra_key}"
            task["options"]["shell"] = False
        if cas_integrity:
            tasks["before-unit"] = {
                "command": [
                    str(bun),
                    "-e",
                    "require('node:fs').rmSync('.outputs/unit.json', { force: true })",
                ],
                "inputs": [],
                "env": common_env,
                "options": {"cache": False, "shell": False},
                "toolchains": ["system"],
            }
            tasks["unit"]["deps"] = ["before-unit"]
        (fixture_root / "moon.yml").write_text(json.dumps({"tasks": tasks}))

    def settings_scheduler(
        *,
        static_failure: bool,
        expect_failure: bool = False,
        acquisition_failure: bool = False,
        foundation_failure: bool = False,
        preparation_failure: bool = False,
        pin_preflight_failure: bool = False,
        ast_failure: bool = False,
    ) -> None:
        config["staticFailure"] = static_failure
        config_path.write_text(json.dumps(config))
        tasks: dict[str, FixtureTask] = {}
        for name, deps in SCHEDULER_DEPENDENCIES.items():
            command = (
                "noop"
                if name in SCHEDULER_NOOP_TASKS
                else [
                    str(bun),
                    str(fixture_root / "moon-cache-task.ts"),
                    (
                        "failure"
                        if (
                            (acquisition_failure and name == "mathjax-owned-evidence")
                            or (
                                pin_preflight_failure
                                and name == "mathjax-import-pin-preflight"
                            )
                            or (foundation_failure and name == "typecheck")
                            or (
                                preparation_failure
                                and name == "rust-facts-host-prebuild"
                            )
                            or (ast_failure and name == "ast-lint")
                        )
                        else "static"
                        if name == "biome"
                        else name
                    ),
                ]
            )
            task: FixtureTask = {
                "command": command,
                "deps": deps,
                "env": common_env,
                "inputs": inputs,
                "toolchains": ["system"],
            }
            if expect_failure and name == "biome":
                task["options"] = {"expectFailure": True}
            tasks[name] = task
        (fixture_root / "moon.yml").write_text(
            json.dumps(
                {
                    "taskOptions": SCHEDULER_TASK_OPTIONS,
                    "tasks": tasks,
                }
            )
        )

    def events(phase: str) -> int:
        return sum(
            json.loads(line)["phase"] == phase
            for line in events_path.read_text().splitlines()
        )

    def invoke(label: str, target: str, *, succeeds: bool = True) -> dict[str, object]:
        started = time.monotonic_ns()
        completed = subprocess.run(
            [
                *moon_command,
                "run",
                f"fixture:{target}",
                "--no-actions",
                "--summary",
                "detailed",
                "--color=false",
            ],
            cwd=fixture_root,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        assert_source_fixture_unchanged()
        log = run_dir / f"{label}.log"
        log.write_text(completed.stdout + completed.stderr)
        record = {
            "label": label,
            "target": target,
            "exit_code": completed.returncode,
            "duration_ms": (time.monotonic_ns() - started) // 1_000_000,
            "log": str(log),
            "log_sha256": digest(log),
            "inputs": {
                str(path.relative_to(fixture_root)): digest(path)
                for path in (
                    *originals,
                    config_path,
                    lock_path,
                    fixture_root / "moon.yml",
                )
            },
            "environment": environment["CACHE_CONTRACT_ENV"],
            "events": {
                phase: events(phase)
                for phase in ("unit", "coverage", "static", "heavy", "failure")
            },
            "scheduler_events": {
                phase: events(phase) for phase in SCHEDULER_EVENT_PHASES
            },
        }
        results.append(record)
        assert (completed.returncode == 0) == succeeds, (
            f"{label}: exit={completed.returncode}; see {log}"
        )
        return record

    def miss(label: str) -> None:
        before = events("unit")
        invoke(label, "unit")
        assert events("unit") == before + 1, f"{label} did not execute changed inputs"

    def verify_coverage(path: Path, expected_hash: str) -> bool:
        if not path.is_file() or digest(path) != expected_hash:
            return False
        lines = path.read_text().splitlines()
        counts = [int(line.split(",")[1]) for line in lines if line.startswith("DA:")]
        return bool(counts) and all(count > 0 for count in counts)

    def verify_direct_input_contract() -> dict[str, object]:
        production_binding = production_task_input_binding(
            bun, moon_command, direct_run_dir
        )
        workspace_dir = direct_run_dir / "workspace"
        (workspace_dir / ".moon").mkdir(parents=True)
        (workspace_dir / "tmp").mkdir()
        (workspace_dir / "scripts/tmp").mkdir(parents=True)
        (workspace_dir / "scripts/.moon/cache").mkdir(parents=True)
        (workspace_dir / ".gitignore").write_text("/.moon/cache/\n/tmp/\n")
        (workspace_dir / ".moon/workspace.yml").write_text(
            json.dumps(
                {
                    "projects": {"probe": "."},
                    "versionConstraint": "=2.6.2",
                    "telemetry": False,
                    "pipeline": {
                        "installDependencies": False,
                        "syncProjects": False,
                        "syncWorkspace": False,
                    },
                    "vcs": {
                        "client": "git",
                        "defaultBranch": "master",
                        "sync": False,
                    },
                }
            )
        )
        (workspace_dir / "source.txt").write_text("baseline source\n")
        root_tmp = workspace_dir / "tmp/generated.txt"
        root_tmp.write_text("baseline generated data\n")
        managed_tmp = workspace_dir / "scripts/tmp/managed.txt"
        managed_tmp.write_text("baseline managed script data\n")
        managed_cache = workspace_dir / "scripts/.moon/cache/managed.txt"
        managed_cache.write_text("baseline managed nested Moon cache data\n")
        generated_inputs = {
            "root target": workspace_dir / "target/build-output.txt",
            "runtime crate target": workspace_dir
            / "crates/katana-render-runtime/target/build-output.txt",
            "ruff cache": workspace_dir / ".ruff_cache/entry",
            "nested mypy cache": workspace_dir / "scripts/.mypy_cache/entry",
            "pytest cache": workspace_dir / "tests/.pytest_cache/entry",
            "nested fixture Moon cache": workspace_dir
            / "tests/fixtures/quality-cache/.moon/cache/entry",
            "coverage LCOV temp": workspace_dir
            / "tests/fixtures/quality-cache/.outputs/coverage/.lcov.info.contract.tmp",
        }
        for label, generated_path in generated_inputs.items():
            generated_path.parent.mkdir(parents=True, exist_ok=True)
            generated_path.write_text(f"baseline {label}\n")
        executions = direct_run_dir / "executions.jsonl"
        command_code = (
            "from pathlib import Path; "
            f"Path({str(executions)!r}).open('a').write('ran\\n')"
        )
        (workspace_dir / "moon.yml").write_text(
            json.dumps(
                {
                    "tasks": {
                        "hash-probe": {
                            "command": [
                                str(shutil.which("python3") or "python3"),
                                "-c",
                                command_code,
                            ],
                            "inputs": DIRECT_INPUTS,
                            "toolchains": ["system"],
                            "options": {
                                "cache": True,
                                "cacheKey": f"{direct_run_dir.name}-direct-inputs",
                                "shell": False,
                            },
                        }
                    }
                }
            )
        )
        initialize_isolated_git(workspace_dir)

        def run(label: str) -> DirectInputRunRecord:
            started = time.monotonic_ns()
            direct_completed = subprocess.run(
                [
                    *moon_command,
                    "run",
                    "probe:hash-probe",
                    "--no-actions",
                    "--summary",
                    "detailed",
                    "--output-style",
                    "hash",
                    "--color=false",
                ],
                cwd=workspace_dir,
                env={**environment, "PWD": str(workspace_dir)},
                text=True,
                capture_output=True,
                check=False,
            )
            record = direct_input_run_record(
                workspace_dir,
                direct_run_dir,
                label,
                direct_completed,
                executions,
                (time.monotonic_ns() - started) // 1_000_000,
            )
            results.append({"label": f"direct-input-{label}", **record})
            return record

        baseline = run("baseline")
        nochange = run("nochange")
        root_tmp.write_text("root tmp changed\n")
        ignored = run("root-tmp-only-change")
        for label, generated_path in generated_inputs.items():
            generated_path.write_text(f"changed {label}\n")
        generated_ignored = run("generated-paths-only-change")
        source = workspace_dir / "source.txt"
        source.write_text("changed root source\n")
        source_changed = run("root-source-change")
        managed_tmp.write_text("changed managed scripts tmp\n")
        managed_changed = run("managed-scripts-tmp-change")
        generated_cache = workspace_dir / ".moon/cache/explicit-generated.txt"
        generated_cache.write_text("changed generated Moon cache\n")
        cache_changed = run("root-moon-cache-only-change")
        workspace_config = workspace_dir / ".moon/workspace.yml"
        workspace_config.write_text(
            json.dumps(json.loads(workspace_config.read_text()), indent=2) + "\n"
        )
        config_changed = run("workspace-config-byte-change")
        managed_cache.write_text("changed managed nested Moon cache\n")
        nested_cache_changed = run("managed-nested-moon-cache-change")
        failures: list[str] = []
        if baseline["execution_count"] != 1:
            failures.append("Baseline task did not execute exactly once")
        if (
            nochange["hash"] != baseline["hash"]
            or nochange["execution_count"] != baseline["execution_count"]
        ):
            failures.append("No-change inputs missed the real Moon cache")
        if (
            ignored["hash"] != baseline["hash"]
            or ignored["execution_count"] != baseline["execution_count"]
        ):
            failures.append("Root tmp change affected direct inputs")
        if (
            generated_ignored["hash"] != ignored["hash"]
            or generated_ignored["execution_count"] != ignored["execution_count"]
        ):
            failures.append(
                "Generated target/tool-cache changes affected direct inputs"
            )
        if (
            source_changed["hash"] == generated_ignored["hash"]
            or source_changed["execution_count"]
            != generated_ignored["execution_count"] + 1
        ):
            failures.append("Root source change did not invalidate task")
        if (
            managed_changed["hash"] == source_changed["hash"]
            or managed_changed["execution_count"]
            != source_changed["execution_count"] + 1
        ):
            failures.append("Managed scripts/tmp change did not invalidate task")
        if (
            cache_changed["hash"] != managed_changed["hash"]
            or cache_changed["execution_count"] != managed_changed["execution_count"]
        ):
            failures.append("Root generated Moon cache change affected direct inputs")
        if (
            config_changed["hash"] == cache_changed["hash"]
            or config_changed["execution_count"] != cache_changed["execution_count"] + 1
        ):
            failures.append("Workspace config byte change did not invalidate task")
        if (
            nested_cache_changed["hash"] == config_changed["hash"]
            or nested_cache_changed["execution_count"]
            != config_changed["execution_count"] + 1
        ):
            failures.append("Managed nested Moon cache change did not invalidate task")
        receipt_path = direct_run_dir / "receipt.json"
        receipt = {
            "status": "failed" if failures else "passed",
            "moon_version": subprocess.check_output(
                [*moon_command, "--version"], text=True
            ).strip(),
            "inputs": DIRECT_INPUTS,
            "production_binding": production_binding,
            "records": [
                baseline,
                nochange,
                ignored,
                generated_ignored,
                source_changed,
                managed_changed,
                cache_changed,
                config_changed,
                nested_cache_changed,
            ],
            "failures": failures,
        }
        receipt_path.write_text(
            json.dumps(receipt, ensure_ascii=False, indent=2) + "\n"
        )
        assert not failures, "; ".join(failures)
        return {"receipt": str(receipt_path), "accepted": True}

    def run_cas_integrity_contract() -> dict[str, object]:
        workspace_path = fixture_root / ".moon/workspace.yml"
        workspace_config = json.loads(workspace_path.read_text())
        workspace_config["cache"] = {"cas": {"verifyIntegrity": True}}
        workspace_config["experiments"] = {"casOutputsCache": True}
        workspace_path.write_text(json.dumps(workspace_config))
        settings("cas-integrity", cas_integrity=True)

        version = subprocess.check_output(
            [*moon_command, "--version"], cwd=fixture_root, env=environment, text=True
        ).strip()
        resolved_binary = subprocess.check_output(
            [
                str(bun),
                "-e",
                "process.stdout.write(require('@moonrepo/cli/utils').findMoonExe())",
            ],
            cwd=ROOT,
            env=environment,
            text=True,
        ).strip()
        binary_path = Path(resolved_binary).resolve(strict=True)
        cli_dir = ROOT / "node_modules/@moonrepo/cli"
        cli_inputs = {
            name: digest(cli_dir / name)
            for name in ("package.json", "moon.js", "utils.js")
        }
        output_path = fixture_root / ".outputs/unit.json"
        cache_root = fixture_root / ".moon/cache"
        event_start = events("unit")

        invoke("cas-first", "unit")
        first_output = output_path.read_bytes()
        first_event_count = events("unit")
        assert first_event_count == event_start + 1, (
            "First CAS invocation did not execute"
        )
        run_report = json.loads((cache_root / "runReport.json").read_text())
        unit_action = next(
            action
            for action in run_report["actions"]
            if action.get("node", {}).get("params", {}).get("target") == "fixture:unit"
        )
        action_hash = next(
            operation["meta"]["hash"]
            for operation in unit_action["operations"]
            if operation.get("meta", {}).get("type") == "hash-generation"
        )
        manifest_path = cache_root / "manifests" / action_hash[:2] / action_hash[2:]
        manifest = json.loads(manifest_path.read_text())
        unit_entry = next(
            file_entry
            for file_entry in manifest["files"]
            if file_entry.get("path") == ".outputs/unit.json"
        )
        blob_digest = unit_entry["digest"]["hash"]
        blob_path = cache_root / "blobs" / blob_digest[:2] / blob_digest[2:]
        blob_bytes = blob_path.read_bytes()
        assert len(blob_bytes) == unit_entry["digest"]["size"]
        assert hashlib.sha256(blob_bytes).hexdigest() == blob_digest
        assert blob_bytes == first_output, (
            "Moon CAS blob does not match the output manifest"
        )

        invoke("cas-same-input", "unit")
        assert events("unit") == first_event_count, (
            "Same-input task did not hit the cache"
        )
        assert output_path.read_bytes() == first_output, (
            "Same-input hit changed task output"
        )
        same_input_report = json.loads((cache_root / "runReport.json").read_text())
        same_input_unit = next(
            action
            for action in same_input_report["actions"]
            if action.get("node", {}).get("params", {}).get("target") == "fixture:unit"
        )
        assert any(
            operation.get("meta", {}).get("type") == "output-hydration"
            and operation.get("status") == "cached"
            for operation in same_input_unit["operations"]
        ), "CAS hit did not record successful output hydration"

        output_path.unlink()
        invoke("cas-output-restoration", "unit")
        baseline_output = output_path.read_bytes()
        assert baseline_output == first_output, (
            "Deleted output was not restored from CAS"
        )
        assert events("unit") == first_event_count, "Output restoration reran the task"

        config["revision"] = "cas-input-changed"
        config_path.write_text(json.dumps(config))
        invoke("cas-changed-input", "unit")
        assert events("unit") == first_event_count + 1, (
            "Changed input reused stale task output"
        )
        config["revision"] = "baseline"
        config_path.write_text(json.dumps(config))
        settings("cas-integrity", cas_integrity=True)
        invoke("cas-baseline-restoration", "unit")
        assert output_path.read_bytes() == baseline_output, (
            "Baseline CAS entry did not restore"
        )

        corrupted_bytes = bytearray(blob_bytes)
        assert len(corrupted_bytes) > 16, "CAS output blob is too small to corrupt"
        corrupted_bytes[len(corrupted_bytes) // 2] ^= 1
        before_corrupt_run_events = events("unit")
        corrupt_command = [
            *moon_command,
            "run",
            "fixture:unit",
            "--no-actions",
            "--summary",
            "detailed",
            "--color=false",
        ]
        output_path.unlink()
        blob_path.write_bytes(corrupted_bytes)
        try:
            corrupt_run = subprocess.run(
                corrupt_command,
                cwd=fixture_root,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            assert_source_fixture_unchanged()
            corrupt_log = run_dir / "cas-corrupt-hydration.log"
            corrupt_log.write_text(corrupt_run.stdout + corrupt_run.stderr)
            after_corrupt_run_events = events("unit")
            corrupt_output = output_path.read_bytes() if output_path.is_file() else None
            corrupted_blob_after_run_sha256 = hashlib.sha256(
                blob_path.read_bytes()
            ).hexdigest()
            corrupt_report = json.loads((cache_root / "runReport.json").read_text())
            corrupt_unit_action = next(
                action
                for action in corrupt_report["actions"]
                if action.get("node", {}).get("params", {}).get("target")
                == "fixture:unit"
            )
            corrupt_action_hash = next(
                operation["meta"]["hash"]
                for operation in corrupt_unit_action["operations"]
                if operation.get("meta", {}).get("type") == "hash-generation"
            )
            corrupt_hydration_status = next(
                operation.get("status")
                for operation in corrupt_unit_action["operations"]
                if operation.get("meta", {}).get("type") == "output-hydration"
            )
        finally:
            blob_path.write_bytes(blob_bytes)
        if corrupt_run.returncode == 0:
            corruption_rejected = (
                corrupt_output == baseline_output
                and after_corrupt_run_events == before_corrupt_run_events + 1
            )
            corrupt_behavior = (
                "integrity failure triggered task rerun and correct output"
                if corruption_rejected
                else "Moon returned a cache hit without restoring expected bytes"
            )
        else:
            corruption_rejected = (
                corrupt_output is None or corrupt_output == baseline_output
            )
            corrupt_behavior = "corrupt CAS hydration failed closed"

        return {
            "moon_version": version,
            "moon_command": moon_command,
            "moon_binary": {
                "path": str(binary_path),
                "sha256": digest(binary_path),
                "size_bytes": binary_path.stat().st_size,
            },
            "bun": {
                "path": str(bun),
                "sha256": digest(bun),
                "version": subprocess.check_output(
                    [str(bun), "--version"], text=True
                ).strip(),
            },
            "moon_cli_inputs_sha256": cli_inputs,
            "workspace_config_sha256": digest(workspace_path),
            "task_config_sha256": digest(fixture_root / "moon.yml"),
            "cas_configuration": {
                "verify_integrity": workspace_config["cache"]["cas"]["verifyIntegrity"],
                "outputs_cache": workspace_config["experiments"]["casOutputsCache"],
                "environment_override_present": "MOON_CACHE_CAS_VERIFY_INTEGRITY"
                in environment,
                "outputs_environment_override_present": "MOON_EXPERIMENT_CAS_OUTPUTS_CACHE"
                in environment,
            },
            "cache_observations": {
                "same_input_hit": True,
                "changed_input_rerun": True,
                "deleted_output_hydrated": True,
                "corrupt_cas_hydration": corrupt_behavior,
                "corrupt_hydration_exit": corrupt_run.returncode,
                "corrupt_hydration_log": str(corrupt_log),
                "output_manifest": str(manifest_path),
                "task_hash": action_hash,
                "output_manifest_sha256": digest(manifest_path),
                "output_blob": str(blob_path),
                "output_blob_sha256": blob_digest,
                "corrupted_blob_sha256": hashlib.sha256(corrupted_bytes).hexdigest(),
                "corrupted_blob_after_hydration_sha256": corrupted_blob_after_run_sha256,
                "corrupt_hydration_action_hash": corrupt_action_hash,
                "corrupt_hydration_status": corrupt_hydration_status,
                "expected_output_sha256": hashlib.sha256(baseline_output).hexdigest(),
                "actual_corrupt_output_sha256": (
                    hashlib.sha256(corrupt_output).hexdigest()
                    if corrupt_output is not None
                    else None
                ),
                "corrupt_task_event_delta": after_corrupt_run_events
                - before_corrupt_run_events,
                "corruption_rejected": corruption_rejected,
                "blob_restored": hashlib.sha256(blob_path.read_bytes()).hexdigest()
                == hashlib.sha256(blob_bytes).hexdigest(),
            },
        }

    if args.cas_integrity_only:
        status = "failed"
        report_path = run_dir / "cas-integrity-receipt.json"
        preserved_paths = (
            fixture_root / "moon.yml",
            fixture_root / ".moon/workspace.yml",
            config_path,
            lock_path,
            events_path,
        )
        saved_files = {
            path: path.read_bytes() for path in preserved_paths if path.exists()
        }
        output_path = fixture_root / ".outputs/unit.json"
        saved_output = output_path.read_bytes() if output_path.exists() else None
        try:
            cas_result = run_cas_integrity_contract()
            results.append(cas_result)
            cache_observations = moon_json_object(cas_result.get("cache_observations"))
            if not cache_observations.get("corruption_rejected"):
                raise AssertionError("Moon accepted the corrupted CAS output blob")
            status = "passed"
        except Exception as error:
            results.append({"error": str(error)})
            raise
        finally:
            for path, content in saved_files.items():
                path.write_bytes(content)
            if saved_output is None:
                output_path.unlink(missing_ok=True)
            else:
                output_path.write_bytes(saved_output)
            report_path.write_text(
                json.dumps(
                    {
                        "status": status,
                        "owner_sha256": digest(Path(__file__).resolve()),
                        "fixture_only": True,
                        "krr_coverage_achievement": False,
                        "other_native_platforms_verified": False,
                        "fixture_root": str(fixture_root),
                        "tracked_fixture_source_sha256": source_pins,
                        "results": results,
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n"
            )
            print(report_path)
        return 0

    status = "failed"
    try:
        settings()
        miss("unit-first")
        before = events("unit")
        original_report = (fixture_root / ".outputs/unit.json").read_bytes()
        invoke("unit-repeat", "unit")
        assert events("unit") == before, "Same input did not skip execution"
        (fixture_root / ".outputs/unit.json").unlink()
        invoke("unit-restore", "unit")
        assert (
            events("unit") == before
            and (fixture_root / ".outputs/unit.json").read_bytes() == original_report
        ), "Output was not restored from real moon cache"
        for filename, label in [
            ("moon-cache-subject.ts", "source-change"),
            ("moon-cache-subject.test.ts", "test-change"),
        ]:
            path = fixture_root / filename
            path.write_bytes(
                originals[path] + "\n// キャッシュ入力の変更を検証する。\n".encode()
            )
            miss(label)
        config["revision"] = "changed"
        config_path.write_text(json.dumps(config))
        miss("config-change")
        lock_path.write_text(json.dumps({"revision": "changed"}))
        miss("lock-change")
        environment["CACHE_CONTRACT_ENV"] = "changed"
        miss("environment-change")
        settings("changed")
        miss("task-config-change")
        config["staticFailure"] = True
        config_path.write_text(json.dumps(config))
        before_heavy = events("heavy")
        invoke("static-failure", "heavy", succeeds=False)
        assert events("static") > 0 and events("heavy") == before_heavy, (
            "Heavy task ran despite failed static dependency"
        )
        before_failure = events("failure")
        invoke("failure-first", "failure", succeeds=False)
        invoke("failure-repeat", "failure", succeeds=False)
        assert events("failure") == before_failure + 2, (
            "Failed result was reused as success"
        )
        config["staticFailure"] = False
        config_path.write_text(json.dumps(config))
        invoke("coverage-first", "coverage")
        lcov = fixture_root / ".outputs/coverage/lcov.info"
        coverage_hash = digest(lcov)
        assert verify_coverage(lcov, coverage_hash), (
            "Real Bun LCOV has uncovered/absent lines"
        )
        before = events("coverage")
        lcov.unlink()
        invoke("coverage-restore", "coverage")
        assert events("coverage") == before and verify_coverage(lcov, coverage_hash), (
            "Real LCOV was not restored without rerunning coverage"
        )
        lcov.write_text(lcov.read_text() + "DA:999,0\n")
        assert not verify_coverage(lcov, coverage_hash), "Tampered LCOV was accepted"
        invoke("coverage-tampered-hit", "coverage")
        if verify_coverage(lcov, coverage_hash):
            tamper_behavior = "moon restored original LCOV"
        else:
            tamper_behavior = (
                "moon retained tampered LCOV; KRR-specific verifier rejected it"
            )
        results.append(
            {
                "label": "coverage-tamper-rejected",
                "passed": True,
                "coverage_sha256": coverage_hash,
                "moon_behavior": tamper_behavior,
            }
        )
        shutil.copyfile(lcov, run_dir / "lcov-after-tampered-hit.info")
        lcov.unlink()
        invoke("coverage-recover", "coverage")
        assert events("coverage") == before and verify_coverage(lcov, coverage_hash), (
            "Rejected LCOV could not be restored from real cache"
        )
        shutil.copyfile(lcov, run_dir / "lcov-verified.info")
        results.append(
            {
                "label": "coverage-canonical-artifact",
                "path": str(run_dir / "lcov-verified.info"),
                "sha256": coverage_hash,
            }
        )
        settings_scheduler(static_failure=False)
        events_path.write_text("")
        invoke("scheduler-exact-success", "unit-test")
        success_phases = [
            json.loads(line)["phase"] for line in events_path.read_text().splitlines()
        ]
        assert scheduler_success_order_valid(success_phases), (
            f"scheduler success order changed: {success_phases}"
        )
        results.append(
            {
                "label": "scheduler-exact-success",
                "passed": True,
                "production_task_count": len(SCHEDULER_DEPENDENCIES),
                "event_phases": success_phases,
                "verified": "foundation precedes host prebuild, Biome, AST, Clippy, dependency leak, evidence acquisition, Bun consumer, and native unit",
            }
        )

        settings_scheduler(static_failure=False, ast_failure=True)
        events_path.write_text("")
        invoke("scheduler-ast-failure-barrier", "unit-test", succeeds=False)
        ast_failure_phases = [
            json.loads(line)["phase"] for line in events_path.read_text().splitlines()
        ]
        assert ast_failure_phases.count("failure") == 1, "AST failure event missing"
        assert set(SCHEDULER_PRE_LINT_PHASES) <= set(ast_failure_phases), (
            f"AST did not wait for the complete cheap static prerequisites: {ast_failure_phases}"
        )
        assert scheduler_failure_barrier_is_closed(ast_failure_phases), (
            f"Clippy, dependency-leak, evidence, or native work ran after AST failure: {ast_failure_phases}"
        )
        results.append(
            {
                "label": "scheduler-ast-failure-barrier",
                "passed": True,
                "event_phases": ast_failure_phases,
                "blocked_phases": list(SCHEDULER_BARRIER_PHASES),
            }
        )

        for label, failure_options, blocked_phases in [
            (
                "foundation",
                {"foundation_failure": True},
                (
                    "capacity-admission",
                    "rust-facts-host-prebuild",
                    "static",
                    *SCHEDULER_BARRIER_PHASES,
                ),
            ),
            (
                "preparation",
                {"preparation_failure": True},
                ("static", *SCHEDULER_BARRIER_PHASES),
            ),
            (
                "mathjax-pin-preflight",
                {"pin_preflight_failure": True},
                (
                    "capacity-admission",
                    "rust-facts-host-prebuild",
                    "source-static-foundation",
                    "biome",
                    "source-static",
                    "static",
                    "ast-lint",
                    "lint",
                    "dependency-leak",
                    "mathjax-owned-evidence",
                    "runtime-asset-script-preflight",
                    "runtime-classic-generation-evidence",
                    "runtime-asset-script-other",
                    "runtime-unused-script-regression",
                    "runtime-asset-script-test",
                    "unit-test",
                ),
            ),
        ]:
            settings_scheduler(static_failure=False, **failure_options)
            events_path.write_text("")
            invoke(f"scheduler-{label}-failure-barrier", "unit-test", succeeds=False)
            bootstrap_failure_phases = [
                json.loads(line)["phase"]
                for line in events_path.read_text().splitlines()
            ]
            assert bootstrap_failure_phases.count("failure") == 1, (
                "Bootstrap failure event missing"
            )
            assert all(
                phase not in bootstrap_failure_phases for phase in blocked_phases
            ), (
                f"A downstream task ran after {label} failure: {bootstrap_failure_phases}"
            )
            if label == "preparation":
                assert set(SCHEDULER_FOUNDATION_PHASES) <= set(bootstrap_failure_phases)
            results.append(
                {
                    "label": f"scheduler-{label}-failure-barrier",
                    "passed": True,
                    "event_phases": bootstrap_failure_phases,
                    "blocked_phases": list(blocked_phases),
                }
            )

        settings_scheduler(static_failure=True)
        events_path.write_text("")
        invoke("scheduler-cheap-failure-barrier", "unit-test", succeeds=False)
        failure_phases = [
            json.loads(line)["phase"] for line in events_path.read_text().splitlines()
        ]
        assert "static" in failure_phases and len(failure_phases) == len(
            set(failure_phases)
        ), f"The failing biome leaf did not emit exactly one event: {failure_phases}"
        assert set(failure_phases) <= set(SCHEDULER_PRE_LINT_PHASES), (
            f"A downstream task ran after a cheap static failure: {failure_phases}"
        )
        assert scheduler_failure_barrier_is_closed(failure_phases), (
            "A final-static producer, consumer, or native task ran after Biome failure"
        )
        results.append(
            {
                "label": "scheduler-cheap-failure-barrier",
                "passed": True,
                "event_phases": failure_phases,
                "blocked_phases": list(SCHEDULER_BARRIER_PHASES),
            }
        )

        settings_scheduler(static_failure=False, acquisition_failure=True)
        events_path.write_text("")
        invoke("scheduler-acquisition-failure-barrier", "unit-test", succeeds=False)
        acquisition_phases = [
            json.loads(line)["phase"] for line in events_path.read_text().splitlines()
        ]
        assert acquisition_phases.count("failure") == 1, (
            "Acquisition failure event missing"
        )
        assert acquisition_phases.index(
            "rust-facts-host-prebuild"
        ) < acquisition_phases.index("failure")
        assert all(
            phase not in acquisition_phases for phase in SCHEDULER_NATIVE_PHASES
        ), f"Consumer or native ran after failed acquisition: {acquisition_phases}"
        results.append(
            {
                "label": "scheduler-acquisition-failure-barrier",
                "passed": True,
                "event_phases": acquisition_phases,
                "blocked_phases": list(SCHEDULER_NATIVE_PHASES),
            }
        )

        settings_scheduler(static_failure=True, expect_failure=True)
        events_path.write_text("")
        invoke("scheduler-expectFailure-counterexample", "unit-test")
        counterexample_phases = [
            json.loads(line)["phase"] for line in events_path.read_text().splitlines()
        ]
        assert scheduler_success_order_valid(counterexample_phases), (
            f"expectFailure counterexample event order changed: {counterexample_phases}"
        )
        assert not scheduler_failure_barrier_is_closed(counterexample_phases), (
            "expectFailure counterexample did not demonstrate the barrier bypass"
        )
        assert all(
            phase in counterexample_phases for phase in SCHEDULER_BARRIER_PHASES
        ), "expectFailure counterexample no longer reaches every downstream phase"
        results.append(
            {
                "label": "scheduler-expectFailure-counterexample-refused",
                "passed": True,
                "event_phases": counterexample_phases,
                "refused_as_barrier_preserving": True,
            }
        )
        direct_input_result = verify_direct_input_contract()
        results.append(
            {"label": "durable-direct-input-exclusion", **direct_input_result}
        )
        status = "passed"
    except Exception as error:
        status = "failed"
        results.append({"error": str(error)})
        raise
    finally:
        for path, content in originals.items():
            path.write_bytes(content)
        receipt = {
            "status": status,
            "owner_sha256": digest(Path(__file__).resolve()),
            "moon_version": subprocess.check_output(
                [*moon_command, "--version"], text=True
            ).strip(),
            "bun_version": subprocess.check_output(
                [str(bun), "--version"], text=True
            ).strip(),
            "platform": {"system": platform.system(), "machine": platform.machine()},
            "fixture_only": True,
            "krr_coverage_achievement": False,
            "other_native_platforms_verified": False,
            "fixture_root": str(fixture_root),
            "tracked_fixture_source_sha256": source_pins,
            "results": results,
        }
        (run_dir / "receipt.json").write_text(
            json.dumps(receipt, ensure_ascii=False, indent=2) + "\n"
        )
        print(run_dir / "receipt.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
