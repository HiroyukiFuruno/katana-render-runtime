"""管理済みCLI buildの固定入力と既存ownerへの束縛。"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import cargo_capacity_guard_native_cli_observation as guard
import produce_html_performance_evidence as owner
import verify_html_performance_evidence as gate
from cargo_capacity_guard import (
    JsonObject,
    _array,
    _object,
    _text,
    cargo_configuration_paths,
    load_state,
)

SCHEMA = "krr-native-cli-managed-build-v3"
CONTROL_MANIFEST_SCHEMA = "krr-native-cli-managed-control-inputs-v1"
CONTROL_MANIFEST_NAME = "scripts/mermaid/native-cli-managed-control-inputs.json"
CONTROL_MANIFEST = Path(__file__).with_name("native-cli-managed-control-inputs.json")


def parse_control_manifest(value: object) -> tuple[str, ...]:
    manifest = guard.exact(value, {"schema", "control"}, "managed control inventory")
    guard.require(manifest["schema"] == CONTROL_MANIFEST_SCHEMA,
                  "managed control inventory schema differs")
    values = tuple(_text(item, "managed control path")
                   for item in _array(manifest["control"], "managed control paths"))
    guard.require(len(values) == 27 and len(set(values)) == len(values),
                  "managed control inventory is empty or duplicated")
    for name in values:
        parts = name.split("/")
        guard.require(re.fullmatch(r"[A-Za-z0-9_./-]+", name) is not None
                      and not Path(name).is_absolute()
                      and all(part not in {"", ".", ".."} for part in parts),
                      "managed control path is not canonical relative ASCII")
    guard.require(CONTROL_MANIFEST_NAME in values, "managed control inventory omits itself")
    return values


CONTROL = parse_control_manifest(guard.parse_json(CONTROL_MANIFEST.read_bytes()))


def current_source(root: Path, query: owner.NativeCliCommandEvidence,
                   untracked: owner.NativeCliCommandEvidence) -> list[JsonObject]:
    names: set[str] = set()
    for result in (query, untracked):
        pin = guard.validate_record(result.observation["stdout"])
        for name in Path(_text(pin["path"], "Git raw")).read_bytes().decode("utf-8").split("\0"):
            if name:
                candidate = root / name
                guard.require(candidate.is_relative_to(root) and not candidate.is_relative_to(root / "tmp")
                              and not candidate.is_relative_to(root / "target"), "Git input outside source")
                names.add(str(guard.canonical(str(candidate), regular=True)))
    return [guard.file_record(Path(name)) for name in sorted(names, key=lambda name: name.encode())]


def admit_source(expected: JsonObject, records: list[JsonObject]) -> None:
    guard.exact(expected, {"schema", "digest", "files"}, "independent expected source")
    guard.require(expected["schema"] == "krr-native-build-source-manifest-v1"
                  and expected["files"] == records and expected["digest"] == guard.source_digest(records),
                  "independent expected source differs from current Git-visible bytes")


def base_environment(protected: list[str]) -> dict[str, str]:
    allowed = guard.ENVIRONMENT_KEYS - {"PWD", "SHLVL", "_", "CARGO", "CARGO_BUILD_JOBS", "TEST_THREADS"} - guard.EMPTY_JUST_ENVIRONMENT_KEYS
    environment = {key: value for key, value in os.environ.items() if key in allowed}
    guard.require(not any(os.environ.get(key) for key in guard.EMPTY_JUST_ENVIRONMENT_KEYS)
                  and os.environ.get("RUSTFLAGS", "-D warnings") == "-D warnings",
                  "nonempty Just selector or different RUSTFLAGS")
    guard.require(bool(environment.get("PATH")) and bool(environment.get("HOME")), "PATH/HOME required")
    environment.update({key: "" for key in guard.EMPTY_JUST_ENVIRONMENT_KEYS})
    environment.update({"RUSTFLAGS": "-D warnings", "SHLVL": "0", "LC_ALL": "C.UTF-8",
                        "LC_CTYPE": "C.UTF-8",
                        "CARGO_CAPACITY_PROTECTED_REFS": json.dumps(protected, separators=(",", ":"))})
    environment.update({"CARGO": " ".join(guard.cargo_proxy_command(environment)),
                        "CARGO_BUILD_JOBS": guard.JUST_DEFAULT_CONCURRENCY,
                        "TEST_THREADS": guard.JUST_DEFAULT_CONCURRENCY})
    guard.require(not environment.get("CARGO_TARGET_DIR"), "alternate Cargo target is unsupported")
    return environment


def guard_environment(base: dict[str, str], root: Path) -> dict[str, str]:
    python = shutil.which("python3", path=base["PATH"])
    guard.require(python is not None and Path(python).is_absolute(), "PATH python3 unavailable")
    return {**base, "PWD": str(root), "_": str(python)}


def validate_just(root: Path) -> None:
    text = (root / "Justfile").read_text()
    required = ['set shell := ["bash", "-uc"]',
                'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
                'JOBS := env_var_or_default("JOBS", "2")',
                'export CARGO_BUILD_JOBS := JOBS',
                'export TEST_THREADS := env_var_or_default("TEST_THREADS", "2")',
                'CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO',
                'export RUSTFLAGS := env_var_or_default("RUSTFLAGS", "-D warnings")',
                'rust-facts-host-prebuild:\n    bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare {{quote(REPO_ROOT)}} {{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}\n',
                'krr-build:\n    {{CARGO_BUILD}} build -p katana-render-runtime-cli\n',
                'protected-capacity-admission protected_registry:\n'
                '    python3 scripts/mermaid/native_cli_build_bindings.py --repo {{quote(REPO_ROOT)}} '
                '--protected-registry {{quote(protected_registry)}}\n']
    required += [f'export {key} := env_var_or_default("{key}", "")'
                 for key in guard.EMPTY_JUST_ENVIRONMENT_KEYS]
    guard.require(all(text.count(line) == 1 for line in required), "fixed Just route differs")
    host_prebuild = re.search(r"^rust-facts-host-prebuild:\n((?:[ \t]+.*\n|\n)*)", text, re.MULTILINE)
    guard.require(host_prebuild is not None and host_prebuild[1].strip()
                  == 'bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare {{quote(REPO_ROOT)}} '
                     '{{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}',
                  "fixed Rust facts host prebuild route differs")
    recipe = re.search(r"^krr-build:\n((?:[ \t]+.*\n|\n)*)", text, re.MULTILINE)
    guard.require(recipe is not None and recipe[1].strip()
                  == "{{CARGO_BUILD}} build -p katana-render-runtime-cli", "extra Just build command")
    admission = re.search(r"^protected-capacity-admission protected_registry:\n((?:[ \t]+.*\n|\n)*)",
                          text, re.MULTILINE)
    guard.require(admission is not None and admission[1].strip()
                  == "python3 scripts/mermaid/native_cli_build_bindings.py --repo {{quote(REPO_ROOT)}} "
                     "--protected-registry {{quote(protected_registry)}}",
                  "protected capacity admission Just route differs")


@dataclass(frozen=True)
class Bindings:
    root: Path
    attempt: Path
    tools: dict[str, JsonObject]
    tool_launchers: dict[str, JsonObject]
    environment: dict[str, str]
    child_environment: dict[str, str]
    capacity: JsonObject
    expected: JsonObject

    def run(self, argv: list[str], role: owner.MeasurementRole, label: str,
            *, environment: dict[str, str] | None = None) -> owner.NativeCliCommandEvidence:
        return owner.observe_native_cli_command(argv, role=role, cwd=self.root,
            env=self.child_environment if environment is None else environment,
            log_directory=self.attempt / "lifecycle", attempt_token=self.attempt.name, label=label)

    def source(self) -> list[JsonObject]:
        git = _text(self.tools["git"]["path"], "Git path")
        return current_source(self.root,
            self.run([git, "ls-files", "-z"], "tool-query", "probe-1-source"),
            self.run([git, "ls-files", "--others", "--exclude-standard", "-z"],
                     "tool-query", "probe-2-source"))

    def snapshot(self) -> JsonObject:
        source = self.source()
        admit_source(self.expected, source)
        tool_launchers = self.resolve_tool_launchers()
        source_paths = {_text(row["path"], "source path") for row in source}
        guard.require(all(str(self.root / name) in source_paths for name in CONTROL), "managed code absent from source")
        configs = cargo_configuration_paths(self.root, self.child_environment)
        guard.require(not configs, "Cargo configurations unsupported")
        toolchains = [base / name for base in (self.root, *self.root.parents)
                      for name in ("rust-toolchain", "rust-toolchain.toml")]
        return {"source": source, "tools": {key: guard.file_record(Path(_text(pin["path"], key)))
                                            for key, pin in self.tools.items()},
                "toolLaunchers": tool_launchers,
                "control": [guard.file_record(self.root / name) for name in CONTROL],
                "environment": self.child_environment,
                "environmentSha256": guard.environment_digest(self.child_environment),
                "capacityAdmission": capacity_admission(self.root, Path(_text(
                    _object(self.capacity["protectedRegistry"], "registry")["path"], "registry path"))),
                "toolchainFiles": [guard.file_record(path) for path in toolchains if path.exists()],
                "toolchainAbsent": [str(path) for path in toolchains if not path.exists()]}

    def resolve_tool_launchers(self) -> dict[str, JsonObject]:
        resolved: dict[str, JsonObject] = {}
        for name in ("cargo", "rustc"):
            tool = gate.resolve_rust_tool(
                self.root, name, self.child_environment, cwd=self.root)
            executable = Path(_text(self.tools[name]["path"], name))
            guard.require(tool.executable == executable, name + " Rustup resolution changed")
            guard.require(tool.dispatcher.name == "rustup", name + " Rustup launcher is required")
            resolved[name] = gate.rust_tool_launcher_pin(tool)
        guard.require(resolved == self.tool_launchers, "Rustup launcher identity changed")
        return resolved


def capacity_admission(root: Path, registry_path: Path) -> JsonObject:
    state = guard.file_record(root / "tmp/.cargo-capacity-guard.json")
    saved = load_state(root / "tmp/.cargo-capacity-guard.json", _text(state["sha256"], "state SHA"))
    for key, expected in guard.policy(root / "target").items():
        if key != "threshold_bytes":
            guard.require(saved.get(key) == expected and not isinstance(saved.get(key), bool),
                          "fixed saved capacity state differs: " + key)
    value = guard.exact(guard.parse_json(registry_path.read_bytes()),
                        {"schema", "physicalRefCount", "refs"}, "protected registry")
    refs = [guard.protected_identity(row) for row in _array(value["refs"], "protected refs")]
    guard.require(value["schema"] == "krr-protected-refs-successor-v1"
                  and value["physicalRefCount"] == len(refs) > 0, "protected registry count differs")
    paths = [_text(row["path"], "protected path") for row in refs]
    guard.require(len(paths) == len(set(paths)), "protected paths not unique")
    encoded = json.dumps(paths, separators=(",", ":"))
    return {"state": state,
            "protectedRegistry": guard.file_record(registry_path), "refs": refs,
            "capacityPolicy": guard.policy(root / "target"), "environmentValue": encoded,
            "environmentSha256": hashlib.sha256(encoded.encode()).hexdigest()}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="既存guardで固定容量stateと保護refを検証する")
    parser.add_argument("--repo", required=True)
    parser.add_argument("--protected-registry", required=True)
    args = parser.parse_args(argv)
    root = guard.canonical(args.repo)
    registry = guard.canonical(args.protected_registry, regular=True)
    guard.require(root.is_dir(), "repo path is not a directory")
    json.dump(capacity_admission(root, registry), sys.stdout, separators=(",", ":"), ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


def construct_bindings(root: Path, attempt: Path, expected_path: Path, expected_sha: str,
                       registry_path: Path, registry_sha: str, cargo: Path, rustc: Path) -> Bindings:
    guard.canonical(str(root))
    guard.canonical(str(attempt))
    guard.require(expected_path.is_relative_to(root / "tmp") and registry_path.is_relative_to(root / "tmp")
                  and not registry_path.is_relative_to(attempt), "independent admission outside repository tmp")
    guard.require(shlex.quote(str(cargo)) == str(cargo), "Cargo path requires unsupported Just shell quoting")
    guard.require(attempt.is_relative_to(root / "tmp") and attempt.name == guard.digest(attempt.name),
                  "attempt path differs")
    expected_pin = guard.file_record(expected_path)
    guard.require(expected_pin["sha256"] == guard.digest(expected_sha)
                  and not expected_path.is_relative_to(attempt), "independent source pin differs")
    expected = guard.parse_json(expected_path.read_bytes())
    guard.require(guard.file_record(registry_path)["sha256"] == guard.digest(registry_sha),
                  "independently approved protected registry SHA differs")
    capacity = capacity_admission(root, registry_path)
    protected = [_text(row["path"], "protected path") for row in _array(capacity["refs"], "refs")
                 if isinstance(row, dict)]
    environment = base_environment(protected)
    cargo_tool = gate.resolve_rust_tool(root, "cargo", environment, cwd=root)
    rustc_tool = gate.resolve_rust_tool(root, "rustc", environment, cwd=root)
    guard.require(cargo_tool.executable == cargo and rustc_tool.executable == rustc,
                  "selected Cargo/Rustc executable differs from Rustup resolution")
    guard.require(cargo_tool.dispatcher.name == rustc_tool.dispatcher.name == "rustup",
                  "Rustup launchers are required")
    guard.require(cargo_tool.executable.parent == rustc_tool.executable.parent,
                  "Cargo/rustc must be one selected toolchain directory")
    paths = {"cargo": cargo, "rustc": rustc}
    paths["lsof"] = owner.native_raw_fd_observer_path()
    for name in ("rtk", "just", "python3", "git", "bash", "bun"):
        resolved = shutil.which(name, path=environment["PATH"])
        guard.require(resolved is not None, "required tool unavailable: " + name)
        paths[name] = Path(str(resolved)).resolve(strict=True)
    tools = {name: guard.file_record(path) for name, path in paths.items()}
    guard.require(all(path.stat().st_mode & 0o111 for path in paths.values())
                  and cargo.name == "cargo" and rustc.name == "rustc", "tool executable/name differs")
    validate_just(root)
    tool_launchers = {name: gate.rust_tool_launcher_pin(tool)
                      for name, tool in (("cargo", cargo_tool), ("rustc", rustc_tool))}
    return Bindings(root, attempt, tools, tool_launchers, environment,
                    guard_environment(environment, root), capacity, expected)
