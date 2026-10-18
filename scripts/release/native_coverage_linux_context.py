#!/usr/bin/env python3
"""Collect the bounded Linux execution context used by KRR coverage reuse."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

import native_coverage_result as coverage_result


SCHEMA = coverage_result.COVERAGE_CONTEXT_SCHEMA
FIXED_ENV = {
    "CARGO_INCREMENTAL": "0",
    "CARGO_TERM_COLOR": "always",
    "RUSTFLAGS": "-D warnings",
    "TEST_THREADS": "1",
}
UNSUPPORTED_OVERRIDES = (
    "AR", "BINDGEN_EXTRA_CLANG_ARGS", "BUN_INSTALL", "CARGO", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_HOME", "CARGO_LLVM_COV_TARGET_DIR",
    "CARGO_TARGET_DIR", "CC", "CFLAGS", "CXX", "CXXFLAGS", "FONTCONFIG_FILE", "FONTCONFIG_PATH",
    "FONTCONFIG_SYSROOT", "GIT_CONFIG_COUNT", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_KEY_0", "GIT_CONFIG_NOSYSTEM",
    "GIT_DIR", "GIT_INDEX_FILE", "GIT_OPTIONAL_LOCKS", "GIT_WORK_TREE", "JAVA_TOOL_OPTIONS",
    "JDK_JAVA_OPTIONS", "KRR_PLANTUML_CACHE_DIR", "LD_AUDIT", "LD_LIBRARY_PATH", "LD_PRELOAD",
    "LLVM_COV", "LLVM_PROFDATA", "LLVM_PROFILE_FILE", "NODE_OPTIONS",
    "PKG_CONFIG_LIBDIR", "PKG_CONFIG_PATH", "PKG_CONFIG_SYSROOT_DIR", "PLANTUML_CACHE_DIR", "PYTHONHOME",
    "PYTHONPATH", "RUSTC", "RUSTC_BOOTSTRAP", "RUSTC_WRAPPER", "RUSTDOC", "RUSTDOCFLAGS", "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN", "RUSTC_WORKSPACE_WRAPPER", "XDG_CONFIG_HOME",
    "_JAVA_OPTIONS",
)
VERSION_RE = re.compile(r"(?<![A-Za-z0-9])([0-9]+(?:\.[0-9]+){1,3}(?:[-+][A-Za-z0-9._-]+)?)")
SAFE_SUMMARY_RE = re.compile(r"[A-Za-z0-9 .()+_-]{1,120}\Z")
OPAQUE_VERSION_MAX_BYTES = 256
FONT_PACKAGES = (
    "fonts-noto-cjk", "fontconfig", "graphviz", "libc6", "libfontconfig1", "libfreetype6", "libharfbuzz0b",
    "libgvc6", "libcgraph6", "libpathplan4", "libcdt5", "libexpat1", "binutils", "gcc", "pkg-config",
)
EXTRA_TOOLS = (
    "rustup", "cargo-sweep", "just", "git", "fc-list", "fc-match", "dpkg-query", "locale", "bash",
    "cc", "gcc", "ld", "ar", "pkg-config",
)


class LinuxCoverageContextError(ValueError):
    """Raised when the live host cannot be proven to match the supported profile."""


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _run(argv: list[str], *, check: bool = True) -> str:
    try:
        result = subprocess.run(argv, text=True, capture_output=True, check=False, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LinuxCoverageContextError("a required Linux identity command failed") from error
    output = (result.stdout + result.stderr).strip()
    if check and result.returncode != 0:
        raise LinuxCoverageContextError("a required Linux identity command failed")
    return output


def _safe_text(value: str, label: str) -> str:
    normalized = " ".join(value.split())
    if not normalized or SAFE_SUMMARY_RE.fullmatch(normalized) is None:
        raise LinuxCoverageContextError(f"{label} is outside the safe summary format")
    return normalized


def _version(argv: list[str], label: str) -> str:
    output = _run(argv)
    match = VERSION_RE.search(output)
    if match is None:
        raise LinuxCoverageContextError(f"{label} version could not be identified")
    return _safe_text(f"{label} {match.group(1)}", label)


def _file_digest(path: Path, records: dict[str, str], role: str) -> bytes:
    try:
        mode = path.stat().st_mode
        if not stat.S_ISREG(mode):
            raise LinuxCoverageContextError("a required identity file is not regular")
        data = path.read_bytes()
    except OSError as error:
        raise LinuxCoverageContextError("a required identity file is unavailable") from error
    records[role] = _sha256(data)
    return data


def _record_tree(path: Path, records: dict[str, str], role: str,
                 ancestors: frozenset[tuple[int, int]] = frozenset()) -> None:
    try:
        metadata = path.stat()
    except FileNotFoundError:
        if path.is_symlink():
            raise LinuxCoverageContextError("an identity tree contains a broken symlink") from None
        return
    except OSError as error:
        raise LinuxCoverageContextError("an identity tree is unavailable") from error
    if stat.S_ISREG(metadata.st_mode):
        _file_digest(path, records, role)
        return
    if not stat.S_ISDIR(metadata.st_mode):
        raise LinuxCoverageContextError("an identity tree has an unsupported file type")
    identity = (metadata.st_dev, metadata.st_ino)
    if identity in ancestors:
        raise LinuxCoverageContextError("an identity tree contains a directory cycle")
    children = sorted(path.iterdir(), key=lambda item: item.name)
    if not children:
        records[f"{role}:empty"] = "empty"
    next_ancestors = ancestors | {identity}
    for child in children:
        _record_tree(child, records, f"{role}/{child.name}", next_ancestors)


def _command_path(name: str) -> Path:
    resolved = shutil.which(name)
    if resolved is None:
        raise LinuxCoverageContextError("a required Linux tool is unavailable")
    path = Path(resolved)
    try:
        if not stat.S_ISREG(path.stat().st_mode):
            raise LinuxCoverageContextError("a required Linux tool is not a regular file")
        path.resolve(strict=True)
    except OSError as error:
        raise LinuxCoverageContextError("a required Linux tool is unavailable") from error
    return path


def _record_tools(records: dict[str, str], rustup_llvm_cov: Path) -> dict[str, str]:
    commands = {
        "rustc": (["rustc", "--version"], "rustc"),
        "cargo": (["cargo", "--version"], "cargo"),
        "cargoLlvmCov": (["cargo", "llvm-cov", "--version"], "cargo-llvm-cov"),
        "java": (["java", "-version"], "java"),
        "bun": (["bun", "--version"], "bun"),
        "node": (["node", "--version"], "node"),
        "python": (["python3", "--version"], "python3"),
    }
    tools: dict[str, str] = {}
    for key, (argv, executable) in commands.items():
        path = _command_path(executable)
        _file_digest(path, records, f"tool:{key}")
        tools[key] = _version(argv, key)
    tools["llvm"] = _record_llvm_version(records, rustup_llvm_cov)
    for name in EXTRA_TOOLS:
        path = _command_path(name)
        _file_digest(path, records, f"extra-tool:{name}")
        _version([name, "--version"], name)
    return tools


def _record_llvm_version(records: dict[str, str], path: Path) -> str:
    _file_digest(path, records, "rustup-llvm:llvm-cov-selected")
    return _version([str(path), "--version"], "llvm")


def _validate_environment(env: Mapping[str, str], system: str, machine: str) -> dict[str, str]:
    if system != "Linux" or machine not in {"x86_64", "amd64"}:
        raise LinuxCoverageContextError("only the Linux x86_64 hosted profile is supported")
    expected = {**FIXED_ENV, "RUNNER_OS": "Linux", "RUNNER_ARCH": "X64"}
    if any(env.get(key) != value for key, value in expected.items()):
        raise LinuxCoverageContextError("runner or fixed test environment differs from the reviewed profile")
    if not env.get("ImageOS") or not env.get("ImageVersion") or not env.get("GITHUB_WORKFLOW_SHA"):
        raise LinuxCoverageContextError("hosted image or workflow identity is unavailable")
    for key in UNSUPPORTED_OVERRIDES:
        if env.get(key):
            raise LinuxCoverageContextError("an unsupported toolchain or runtime override is present")
    if any(key.startswith(("CARGO_", "RUST")) and key not in FIXED_ENV for key in env):
        raise LinuxCoverageContextError("an unsupported Cargo or Rust override is present")
    if any(env.get(key) for key in env if key.startswith(("GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_"))):
        raise LinuxCoverageContextError("a Git configuration override is present")
    if env.get("TEST_THREADS") != "1":
        raise LinuxCoverageContextError("coverage requires one test thread")
    return {
        "runnerOs": "Linux", "runnerArch": "X64", "imageOS": _safe_text(env["ImageOS"], "image OS"),
        "imageVersion": _safe_text(env["ImageVersion"], "image version"),
        "testThreads": "1",
    }


def _record_environment(env: Mapping[str, str], records: dict[str, str]) -> None:
    for key in (*FIXED_ENV, "LANG", "LC_ALL", "LC_CTYPE", "TZ"):
        records[f"environment:{key}"] = _sha256(env.get(key, "<absent>").encode())
    locale_output = _run(["locale"])
    records["locale:effective"] = _sha256(locale_output.encode())


def _record_runtime_configs(repo: Path, records: dict[str, str], env: Mapping[str, str]) -> None:
    cursors = [repo, *repo.parents]
    for index, base in enumerate(cursors):
        for name in (".cargo/config", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"):
            candidate = base / name
            if candidate.exists() or candidate.is_symlink():
                _record_tree(candidate, records, f"repo-config-{index}/{name}")
    cargo_home = Path.home() / ".cargo"
    for name in ("config", "config.toml"):
        _record_tree(cargo_home / name, records, f"cargo-home/{name}")
    _record_tree(Path.home() / ".rustup" / "settings.toml", records, "rustup/settings.toml")
    java_home = env.get("JAVA_HOME")
    if not java_home:
        raise LinuxCoverageContextError("JAVA_HOME is unavailable")
    java_root = Path(java_home)
    for relative in ("release", "lib/modules", "lib/server/libjvm.so", "lib/libjava.so"):
        _record_tree(java_root / relative, records, f"java/{relative}")
    _record_tree(Path("/etc/fonts"), records, "fontconfig/etc")
    _record_tree(Path("/usr/share/fontconfig"), records, "fontconfig/share")
    _record_tree(Path.home() / ".fonts.conf", records, "fontconfig/home-fonts.conf")
    _record_tree(Path.home() / ".config" / "fontconfig", records, "fontconfig/home")
    _record_fontconfig_closure(records)


def _record_fontconfig_closure(records: dict[str, str]) -> None:
    executable = _command_path("fc-conflist")
    _file_digest(executable, records, "fontconfig:fc-conflist")
    config_paths = _fontconfig_config_paths(_run([str(executable)]))
    for index, config_path in enumerate(config_paths):
        _record_tree(config_path, records, f"fontconfig:ruleset-{index}")


def _fontconfig_config_paths(output: str) -> list[Path]:
    config_paths: list[Path] = []
    for line in output.splitlines():
        marker, separator, entry = line.partition(" ")
        raw_path, delimiter, _details = entry.partition(":")
        candidate = Path(raw_path.strip())
        if marker not in {"+", "-"} or not separator or not delimiter or not candidate.is_absolute():
            raise LinuxCoverageContextError("fontconfig configuration closure is unknown")
        config_paths.append(candidate)
    if not config_paths:
        raise LinuxCoverageContextError("fontconfig configuration closure is empty")
    return config_paths


def _font_inventory_digest(output: str, records: dict[str, str]) -> str:
    entries: list[str] = []
    for row in output.splitlines():
        fields = row.split("\t", 3)
        if len(fields) != 4 or not all(fields[:3]) or not fields[3]:
            raise LinuxCoverageContextError("fontconfig returned an incomplete inventory")
        family, style, index, raw_path = fields
        path = Path(raw_path)
        if not path.is_absolute() or not path.exists():
            raise LinuxCoverageContextError("fontconfig returned an unavailable font file")
        digest = _sha256(_file_digest(path, records, f"font:{family}:{style}:{index}"))
        entries.append("\t".join((family, style, index, digest)))
    if not entries:
        raise LinuxCoverageContextError("fontconfig inventory is empty")
    return _sha256("\n".join(sorted(entries)).encode())


def _record_fonts(records: dict[str, str]) -> None:
    output = _run(["fc-list", "--format", "%{family}\t%{style}\t%{index}\t%{file}\n"])
    records["fonts:inventory"] = _font_inventory_digest(output, records)
    for family in ("sans-serif", "serif", "monospace"):
        match = _run(["fc-match", "--format", "%{family}\t%{style}\t%{index}\t%{file}", family])
        fields = match.split("\t", 3)
        if len(fields) != 4 or not fields[3]:
            raise LinuxCoverageContextError("fontconfig family resolution is incomplete")
        matched_path = Path(fields[3])
        matched_sha = _sha256(_file_digest(matched_path, records, f"font-match:{family}"))
        records[f"font-match-result:{family}"] = _sha256("\t".join((*fields[:3], matched_sha)).encode())


def _record_rust_toolchain(records: dict[str, str]) -> Path:
    active = _run(["rustup", "show", "active-toolchain"])
    if not re.fullmatch(r"stable-[A-Za-z0-9_]+-unknown-linux-gnu(?: \(default\))?", active):
        raise LinuxCoverageContextError("Rustup active toolchain is outside the reviewed stable profile")
    components = _run(["rustup", "component", "list", "--installed"])
    if not any(line.startswith("llvm-tools-") and line.endswith("-unknown-linux-gnu")
               for line in components.splitlines()):
        raise LinuxCoverageContextError("Rustup LLVM tools component is unavailable")
    verbose = _run(["rustc", "-vV"])
    host = re.search(r"^host: ([A-Za-z0-9_-]+)$", verbose, re.M)
    sysroot = _run(["rustc", "--print", "sysroot"])
    if host is None:
        raise LinuxCoverageContextError("Rust compiler host identity is unavailable")
    for name in ("rustc", "cargo"):
        selected = Path(_run(["rustup", "which", name]))
        _file_digest(selected, records, f"rustup-selected:{name}")
    tool_dir = Path(sysroot) / "lib/rustlib" / host.group(1) / "bin"
    for name in ("llvm-cov", "llvm-profdata"):
        _file_digest(tool_dir / name, records, f"rustup-llvm:{name}")
    records["rustup:active"] = _sha256(active.encode())
    records["rustup:components"] = _sha256(components.encode())
    records["rustup:host"] = _sha256(host.group(1).encode())
    return tool_dir / "llvm-cov"


def _opaque_version_digest(version: str, label: str) -> str:
    normalized = version.strip()
    encoded = normalized.encode()
    if not encoded or len(encoded) > OPAQUE_VERSION_MAX_BYTES or any(byte < 0x20 or byte > 0x7E for byte in encoded):
        raise LinuxCoverageContextError(f"{label} version is invalid")
    return _sha256(encoded)


def _record_packages(repo: Path, records: dict[str, str]) -> None:
    for package in FONT_PACKAGES:
        version = _run(["dpkg-query", "-W", "-f=${Version}", package])
        records[f"package:{package}"] = _opaque_version_digest(version, "package")
    dot = _command_path("dot")
    _file_digest(dot, records, "graphviz:dot")
    records["graphviz:version"] = _sha256(_run(["dot", "-V"]).encode())
    cache_dir = _run(["bash", str(repo / "scripts/plantuml/cache-dir.sh")])
    try:
        justfile = (repo / "Justfile").read_text()
    except OSError as error:
        raise LinuxCoverageContextError("PlantUML repository pin is unavailable") from error
    versions = re.findall(r'^PLANTUML_JAR_VERSION := "([^"]+)"$', justfile, re.M)
    if len(versions) != 1:
        raise LinuxCoverageContextError("PlantUML version pin is unavailable")
    jar = Path(cache_dir) / versions[0] / "plantuml.jar"
    try:
        jar_bytes = jar.read_bytes()
        checksum_manifest = (repo / "crates/katana-render-runtime/vendor/plantuml" / versions[0] / "plantuml.jar.sha256").read_text()
    except OSError as error:
        raise LinuxCoverageContextError("PlantUML runtime is unavailable") from error
    expected = re.findall(r"^([0-9a-f]{64})  plantuml\.jar$", checksum_manifest, re.M)
    if len(expected) != 1 or _sha256(jar_bytes) != expected[0]:
        raise LinuxCoverageContextError("PlantUML runtime differs from the repository checksum pin")
    records["plantuml:version"] = versions[0]
    records["plantuml:jar"] = _sha256(jar_bytes)


def _environment_hash(records: Mapping[str, str]) -> str:
    encoded = json.dumps(dict(sorted(records.items())), sort_keys=True, separators=(",", ":")).encode()
    return _sha256(encoded)


def collect_context(repo: Path, env: Mapping[str, str] | None = None, *, system: str | None = None,
                    machine: str | None = None) -> coverage_result.JsonObject:
    """Collect the supported GitHub-hosted Linux context without exposing host paths."""
    environment = os.environ if env is None else env
    profile = _validate_environment(environment, system or platform.system(), machine or platform.machine())
    workflow_sha = environment["GITHUB_WORKFLOW_SHA"]
    if coverage_result.COMMIT_RE.fullmatch(workflow_sha) is None:
        raise LinuxCoverageContextError("workflow identity is invalid")
    root = repo.resolve(strict=True)
    try:
        head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, text=True, capture_output=True, check=True)
    except (OSError, subprocess.CalledProcessError) as error:
        raise LinuxCoverageContextError("tested source identity is unavailable") from error
    tested_sha = head.stdout.strip()
    coverage_result._verify_tested_source(root, tested_sha)
    records: dict[str, str] = {}
    _record_environment(environment, records)
    try:
        _record_runtime_configs(root, records, environment)
        _record_fonts(records)
        _record_packages(root, records)
        llvm_cov = _record_rust_toolchain(records)
        tools = _record_tools(records, llvm_cov)
    except OSError as error:
        raise LinuxCoverageContextError("Linux runtime identity could not be read") from error
    result: coverage_result.JsonObject = {
        "schema": SCHEMA, "testedSha": tested_sha, "workflowSha": workflow_sha,
        "profile": {**profile, "environmentSha256": _environment_hash(records)}, "tools": tools,
    }
    parsed = coverage_result.parse_coverage_context(result)
    coverage_result._verify_tested_source(root, tested_sha)
    return parsed


def main() -> int:
    try:
        result = collect_context(Path.cwd())
    except (OSError, coverage_result.CoverageResultError, LinuxCoverageContextError) as error:
        print(f"coverage context: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
