#!/usr/bin/env python3
"""Collect a privacy-safe local macOS host context for native coverage reuse."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import time
from collections.abc import Mapping
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import cast
from native_coverage_dependency_tree import (
    LocalCoverageContextError as LocalCoverageContextError,
    _file_identity as _file_identity,
    _internal_file_symlink_entry,
)

SCHEMA = "krr-native-local-coverage-context-v1"
CONTEXT_STAGE_WORKERS = 3
TOOLS = ("rustc", "cargo", "cargoLlvmCov", "cargoNextest", "llvm", "java", "bun", "node", "python", "graphviz", "plantuml")
VOLATILE_ENV = frozenset({
    "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB", "GITHUB_ACTION", "GITHUB_ACTIONS",
    "_", "PWD", "OLDPWD", "SHLVL",
    "MOON_CACHE_DIR", "MOON_PROJECT_ID", "MOON_PROJECT_ROOT", "MOON_PROJECT_SOURCE",
    "MOON_PROJECT_SNAPSHOT", "MOON_TARGET", "MOON_TASK_ID", "MOON_TASK_HASH",
    # 再試行カウンタは入力ではなく実行時だけ付くため、キー計算時との一致を保つ。
    # https://moonrepo.dev/docs/env-vars#during-retries
    "MOON_TASK_RETRY_ATTEMPT", "MOON_TASK_RETRY_TOTAL",
    "MOON_WORKSPACE_ROOT", "MOON_WORKING_DIR", "KRR_LOCAL_COVERAGE_PROFILE_PATH",
    "CODEX_TOOL_CALL_ID",
})
UNSUPPORTED_ENV = frozenset({
    "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_ENCODED_RUSTFLAGS", "CARGO_HOME",
    "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "RUSTDOCFLAGS",
    "LLVM_COV", "LLVM_PROFDATA", "LLVM_PROFILE_FILE", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS",
    "_JAVA_OPTIONS", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "LD_PRELOAD", "LD_AUDIT",
    "FONTCONFIG_FILE", "FONTCONFIG_PATH", "FONTCONFIG_SYSROOT", "KRR_PLANTUML_CACHE_DIR",
})
MISS_REASON_CODES = frozenset({
    "required_identity_command_unavailable", "required_identity_command_failed", "identity_file_not_regular",
    "identity_file_unavailable", "tool_version_unavailable", "tool_version_not_path_free",
    "llvm_version_unavailable", "required_tool_unavailable", "required_tool_not_regular",
    "unsupported_tool_override", "macho_dependency_list_unavailable", "macho_dependency_unresolved",
    "rust_target_unavailable", "plantuml_cache_override", "plantuml_pin_unavailable",
    "plantuml_checksum_mismatch", "rust_standard_library_unavailable", "font_inventory_invalid", "font_inventory_empty",
    "font_inventory_incomplete", "font_file_unavailable", "font_face_inventory_incomplete",
    "unsupported_host_profile", "cargo_dependency_metadata_unavailable",
    "cargo_dependency_graph_invalid", "cargo_dependency_source_unavailable",
    "cargo_dependency_source_changed", "cargo_dependency_source_symlink",
    "cargo_dependency_source_unsupported_file", "identity_record_collision",
})


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def _run(argv: list[str], *, timeout: int = 45) -> str:
    try:
        result = subprocess.run(argv, text=True, capture_output=True, check=False, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LocalCoverageContextError("required_identity_command_unavailable") from error
    if result.returncode:
        raise LocalCoverageContextError("required_identity_command_failed")
    return result.stdout.strip() if result.stdout.strip() else result.stderr.strip()


def _record_file(path: Path, records: dict[str, dict[str, object]], role: str) -> bytes:
    try:
        metadata = path.stat()
        if not stat.S_ISREG(metadata.st_mode):
            raise LocalCoverageContextError("identity_file_not_regular")
        data = path.read_bytes()
    except OSError as error:
        raise LocalCoverageContextError("identity_file_unavailable") from error
    records[role] = {"sha256": _sha256(data), "size": len(data)}
    return data


def _path_id(path: Path) -> str:
    return _sha256(os.fsencode(str(path.resolve(strict=True))))


def _tool_version(argv: list[str], label: str) -> str:
    output = _run(argv)
    first = next((" ".join(line.split()) for line in output.splitlines() if line.strip()), "")
    if not first or len(first) > 120 or any(ord(char) < 32 for char in first):
        raise LocalCoverageContextError("tool_version_unavailable")
    summary = " ".join(first.split())
    if "/" in summary or "\\" in summary:
        raise LocalCoverageContextError("tool_version_not_path_free")
    return summary


def _llvm_version(path: Path) -> str:
    output = _run([str(path), "--version"])
    match = re.search(r"\bLLVM version ([0-9]+(?:\.[0-9]+){1,3}(?:[-+][A-Za-z0-9._-]+)?)", output)
    if match is None:
        raise LocalCoverageContextError("llvm_version_unavailable")
    return f"LLVM {match.group(1)}"


def _resolve_tool(name: str) -> Path:
    found = shutil.which(name)
    if found is None:
        raise LocalCoverageContextError("required_tool_unavailable")
    try:
        path = Path(found).resolve(strict=True)
        if not stat.S_ISREG(path.stat().st_mode):
            raise LocalCoverageContextError("required_tool_not_regular")
    except OSError as error:
        raise LocalCoverageContextError("required_tool_unavailable") from error
    return path


def _record_environment(env: Mapping[str, str]) -> str:
    unexpected = [name for name, value in env.items() if value and name in UNSUPPORTED_ENV]
    if unexpected:
        raise LocalCoverageContextError("unsupported_tool_override")
    rustflags = env.get("RUSTFLAGS", "")
    if rustflags and rustflags != "-D warnings":
        raise LocalCoverageContextError("unsupported_tool_override")
    values: dict[str, str] = {}
    for name, value in env.items():
        if name in VOLATILE_ENV:
            continue
        values[name] = value
    return _sha256(_canonical(values))


def _record_command(name: str, version_argv: list[str], records: dict[str, dict[str, object]], role: str) -> str:
    path = _resolve_tool(name)
    _record_file(path, records, f"tool:{role}:{_path_id(path)}")
    _record_macho_closure(path, records, role)
    return _tool_version(version_argv, role)


def _macho_rpaths(binary: Path) -> list[str]:
    output = _run(["otool", "-l", str(binary)])
    lines = output.splitlines()
    paths: list[str] = []
    for index, line in enumerate(lines):
        if line.strip() != "cmd LC_RPATH":
            continue
        for candidate in lines[index + 1:index + 6]:
            stripped = candidate.strip()
            if stripped.startswith("path "):
                paths.append(stripped[5:].split(" (", 1)[0])
                break
    return paths


def _resolve_macho_dependency(raw: str, binary: Path, rpaths: list[str]) -> Path | None:
    if raw.startswith(("/System/Library/", "/usr/lib/")):
        return None
    if raw.startswith("/"):
        candidate = Path(raw)
        return candidate if candidate.is_file() else None
    if raw.startswith("@loader_path/"):
        candidates = [binary.parent / raw.removeprefix("@loader_path/")]
    elif raw.startswith("@executable_path/"):
        candidates = [binary.parent / raw.removeprefix("@executable_path/")]
    elif raw.startswith("@rpath/"):
        suffix = raw.removeprefix("@rpath/")
        candidates = []
        for entry in rpaths:
            entry = entry.replace("@loader_path", str(binary.parent)).replace("@executable_path", str(binary.parent))
            candidates.append(Path(entry) / suffix)
        candidates.append(binary.parent.parent / "lib" / suffix)
    else:
        return None
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve(strict=True)
    return None


def _record_macho_closure(path: Path, records: dict[str, dict[str, object]], role: str) -> None:
    queue = [path.resolve(strict=True)]
    visited: set[Path] = set()
    while queue:
        binary = queue.pop()
        if binary in visited:
            continue
        visited.add(binary)
        lines = _run(["otool", "-L", str(binary)]).splitlines()
        if not lines:
            raise LocalCoverageContextError("macho_dependency_list_unavailable")
        rpaths = _macho_rpaths(binary)
        for line in lines[1:]:
            raw = line.strip().split(" (", 1)[0]
            if not raw:
                continue
            dependency = _resolve_macho_dependency(raw, binary, rpaths)
            if dependency is None:
                if raw.startswith(("/System/Library/", "/usr/lib/")):
                    continue
                raise LocalCoverageContextError("macho_dependency_unresolved")
            _record_file(dependency, records, f"macho:{role}:{_path_id(dependency)}")
            queue.append(dependency)


def _rust_tool_paths(records: dict[str, dict[str, object]]) -> str:
    rustup = _resolve_tool("rustup")
    _record_file(rustup, records, f"tool:rustup:{_path_id(rustup)}")
    rtk = _resolve_tool("rtk")
    _record_file(rtk, records, f"tool:rtk:{_path_id(rtk)}")
    _tool_version(["rtk", "--version"], "rtk")
    rustc = Path(_run(["rustup", "which", "rustc"])).resolve(strict=True)
    cargo = Path(_run(["rustup", "which", "cargo"])).resolve(strict=True)
    _record_file(rustc, records, f"rustup:rustc:{_path_id(rustc)}")
    _record_file(cargo, records, f"rustup:cargo:{_path_id(cargo)}")
    _record_macho_closure(rustc, records, "rustc")
    _record_macho_closure(cargo, records, "cargo")
    sysroot = Path(_run(["rustc", "--print", "sysroot"])).resolve(strict=True)
    host_line = next((line.removeprefix("host: ") for line in _run(["rustc", "-vV"]).splitlines()
                      if line.startswith("host: ")), "")
    if not host_line:
        raise LocalCoverageContextError("rust_target_unavailable")
    host = host_line
    llvm_root = sysroot / "lib" / "rustlib" / host / "bin"
    for name in ("llvm-cov", "llvm-profdata"):
        candidate = llvm_root / name
        if not candidate.is_file():
            candidate = candidate.with_suffix(".exe")
        _record_file(candidate, records, f"rustup-llvm:{name}:{_path_id(candidate)}")
        _record_macho_closure(candidate, records, name)
        if name == "llvm-cov":
            version = _llvm_version(candidate)
            records["llvm-version"] = {"sha256": _sha256(version.encode()), "size": len(version)}
    standard_library = sysroot / "lib" / "rustlib" / host / "lib"
    try:
        library_files = sorted(item for item in standard_library.iterdir() if item.is_file())
    except OSError as error:
        raise LocalCoverageContextError("rust_standard_library_unavailable") from error
    if not library_files:
        raise LocalCoverageContextError("rust_standard_library_unavailable")
    for item in library_files:
        _record_file(item, records, f"rust-standard-library:{_path_id(item)}")
    return version


def _record_plantuml(repo: Path, records: dict[str, dict[str, object]], env: Mapping[str, str]) -> str:
    if env.get("KRR_PLANTUML_CACHE_DIR"):
        raise LocalCoverageContextError("plantuml_cache_override")
    justfile = (repo / "Justfile").read_text(encoding="utf-8")
    import re

    versions = re.findall(r'^PLANTUML_JAR_VERSION := "([^\"]+)"$', justfile, re.M)
    checksums = re.findall(r'^PLANTUML_JAR_CHECKSUM := "([0-9a-f]{64})"$', justfile, re.M)
    if len(versions) != 1 or len(checksums) != 1:
        raise LocalCoverageContextError("plantuml_pin_unavailable")
    cache_root = Path(_run(["bash", "scripts/plantuml/cache-dir.sh"], timeout=10))
    jar = cache_root / versions[0] / "plantuml.jar"
    jar_bytes = _record_file(jar, records, "plantuml:jar")
    digest = _sha256(jar_bytes)
    if digest != checksums[0]:
        raise LocalCoverageContextError("plantuml_checksum_mismatch")
    records["plantuml:version"] = {"sha256": _sha256(versions[0].encode()), "size": len(versions[0])}
    return f"PlantUML {versions[0]}"


def _record_fonts(records: dict[str, dict[str, object]]) -> int:
    output = _run(["/usr/sbin/system_profiler", "SPFontsDataType", "-json"], timeout=60)
    try:
        payload = json.loads(output)
    except json.JSONDecodeError as error:
        raise LocalCoverageContextError("font_inventory_invalid") from error
    rows = payload.get("SPFontsDataType") if isinstance(payload, dict) else None
    if not isinstance(rows, list) or not rows:
        raise LocalCoverageContextError("font_inventory_empty")
    return _record_font_rows(rows, records)


def _record_font_rows(rows: list[object], records: dict[str, dict[str, object]]) -> int:
    seen: dict[str, str] = {}
    count = 0
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or row.get("enabled") != "yes" or row.get("valid") != "yes":
            raise LocalCoverageContextError("font_inventory_incomplete")
        raw_path = row.get("path")
        faces = row.get("typefaces")
        if not isinstance(raw_path, str) or not raw_path or not isinstance(faces, list) or not faces:
            raise LocalCoverageContextError("font_inventory_incomplete")
        try:
            path = Path(raw_path).resolve(strict=True)
        except OSError as error:
            raise LocalCoverageContextError("font_file_unavailable") from error
        path_id = _path_id(path)
        if path_id not in seen:
            data = _record_file(path, records, f"font-file:{path_id}")
            seen[path_id] = _sha256(data)
        if any(not isinstance(face, dict) or face.get("valid") != "yes" for face in faces):
            raise LocalCoverageContextError("font_face_inventory_incomplete")
        row_digest = _sha256(_canonical({
            "enabled": row["enabled"], "valid": row["valid"], "type": row.get("type", ""),
            "fontSha256": seen[path_id], "faces": faces,
        }))
        records[f"font-row:{index}"] = {"sha256": row_digest, "size": len(faces)}
        count += len(faces)
    if not count:
        raise LocalCoverageContextError("font_inventory_empty")
    return count


def _cargo_metadata_commands(repo: Path, host: str) -> list[tuple[str, list[str]]]:
    common = ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
              "--filter-platform", host, "--manifest-path"]
    return [
        ("root", [*common, str(repo / "Cargo.toml"), "--all-features"]),
        ("performance-baseline", [*common,
                                   str(repo / "scripts/html/performance-harness/Cargo.toml"),
                                   "--no-default-features", "--features", "baseline"]),
        ("performance-candidate", [*common,
                                    str(repo / "scripts/html/performance-harness/Cargo.toml"),
                                    "--no-default-features", "--features", "candidate"]),
    ]


def _dependency_tree_digest(root: Path) -> tuple[str, int, int]:
    """Hash package files and safe internal regular-file symlinks."""
    root = root.resolve(strict=True)
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        root_fd = os.open(root, directory_flags | nofollow)
    except OSError as error:
        raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
    entries: list[dict[str, object]] = []
    symlink_targets: list[str] = []
    total_bytes = 0

    def visit(directory_fd: int, relative: str) -> None:
        nonlocal total_bytes
        before_directory = os.fstat(directory_fd)
        scan_fd = -1
        try:
            scan_fd = os.dup(directory_fd)
            with os.scandir(scan_fd) as iterator:
                children = sorted(iterator, key=lambda item: os.fsencode(item.name))
        except OSError as error:
            raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
        finally:
            if scan_fd >= 0:
                os.close(scan_fd)
        for child in children:
            name = child.name
            child_relative = f"{relative}/{name}" if relative else name
            try:
                before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            except OSError as error:
                raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
            if stat.S_ISLNK(before.st_mode):
                entry, target = _internal_file_symlink_entry(
                    root, directory_fd, name, child_relative, before
                )
                entries.append(entry)
                symlink_targets.append(target)
                continue
            if stat.S_ISDIR(before.st_mode):
                entries.append({"path": child_relative, "type": "directory"})
                try:
                    child_fd = os.open(name, directory_flags | nofollow, dir_fd=directory_fd)
                except OSError as error:
                    raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
                try:
                    if _file_identity(os.fstat(child_fd)) != _file_identity(before):
                        raise LocalCoverageContextError("cargo_dependency_source_changed")
                    visit(child_fd, child_relative)
                    if _file_identity(os.fstat(child_fd)) != _file_identity(before):
                        raise LocalCoverageContextError("cargo_dependency_source_changed")
                finally:
                    os.close(child_fd)
                continue
            if not stat.S_ISREG(before.st_mode):
                raise LocalCoverageContextError("cargo_dependency_source_unsupported_file")
            try:
                file_fd = os.open(name, os.O_RDONLY | nofollow, dir_fd=directory_fd)
            except OSError as error:
                raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
            try:
                opened = os.fstat(file_fd)
                if _file_identity(opened) != _file_identity(before):
                    raise LocalCoverageContextError("cargo_dependency_source_changed")
                with os.fdopen(os.dup(file_fd), "rb") as source:
                    digest = hashlib.file_digest(source, "sha256").hexdigest()
                after = os.fstat(file_fd)
                if _file_identity(after) != _file_identity(opened):
                    raise LocalCoverageContextError("cargo_dependency_source_changed")
                entries.append({"path": child_relative, "type": "file", "size": opened.st_size,
                                "sha256": digest})
                total_bytes += opened.st_size
            except OSError as error:
                raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
            finally:
                os.close(file_fd)
        if _file_identity(os.fstat(directory_fd)) != _file_identity(before_directory):
            raise LocalCoverageContextError("cargo_dependency_source_changed")

    try:
        visit(root_fd, "")
    finally:
        os.close(root_fd)
    if not entries:
        raise LocalCoverageContextError("cargo_dependency_source_unavailable")
    entries.sort(key=lambda item: cast(str, item["path"]))
    regular_files = {
        cast(str, entry["path"]) for entry in entries if entry["type"] == "file"
    }
    if any(target not in regular_files for target in symlink_targets):
        raise LocalCoverageContextError("cargo_dependency_source_changed")
    return _sha256(_canonical(entries)), len(entries), total_bytes


def _record_cargo_metadata(repo: Path, label: str, metadata: object,
                           records: dict[str, dict[str, object]],
                           dependency_digests: dict[Path, tuple[str, int, int]]) -> None:
    if not isinstance(metadata, dict):
        raise LocalCoverageContextError("cargo_dependency_graph_invalid")
    packages = metadata.get("packages")
    resolve = metadata.get("resolve")
    nodes = resolve.get("nodes") if isinstance(resolve, dict) else None
    if not isinstance(packages, list) or not isinstance(nodes, list):
        raise LocalCoverageContextError("cargo_dependency_graph_invalid")
    selected: dict[str, dict[str, object]] = {}
    node_ids: set[str] = set()
    for node in nodes:
        if not isinstance(node, dict) or not isinstance(node.get("id"), str):
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        node_id = cast(str, node["id"])
        if node_id in node_ids:
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        node_ids.add(node_id)
        selected[node_id] = node
    package_by_id: dict[str, dict[str, object]] = {}
    for package in packages:
        if not isinstance(package, dict) or not isinstance(package.get("id"), str):
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        package_id = cast(str, package["id"])
        if package_id in package_by_id:
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        package_by_id[package_id] = package
    if node_ids - package_by_id.keys():
        raise LocalCoverageContextError("cargo_dependency_graph_invalid")
    resolution_rows = []
    external: dict[Path, list[dict[str, str]]] = {}
    repo_root = repo.resolve(strict=True)
    for package_id in sorted(node_ids):
        node = selected[package_id]
        features = node.get("features")
        dependencies = node.get("deps")
        if (not isinstance(features, list) or any(not isinstance(item, str) for item in features)
                or not isinstance(dependencies, list)):
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        deps_rows = []
        for dependency in dependencies:
            if not isinstance(dependency, dict) or not isinstance(dependency.get("pkg"), str):
                raise LocalCoverageContextError("cargo_dependency_graph_invalid")
            dependency_name = dependency.get("name")
            dep_kinds = dependency.get("dep_kinds")
            if (not isinstance(dependency_name, str) or not isinstance(dep_kinds, list)
                    or any(not isinstance(item, dict) for item in dep_kinds)):
                raise LocalCoverageContextError("cargo_dependency_graph_invalid")
            deps_rows.append({
                "name": dependency_name, "package": dependency["pkg"],
                "kinds": dep_kinds,
            })
        resolution_rows.append({"package": package_id, "features": sorted(features),
                                "dependencies": sorted(deps_rows, key=lambda item: (
                                    cast(str, item["package"]), cast(str, item["name"])))})
        package = package_by_id[package_id]
        manifest = package.get("manifest_path")
        name = package.get("name")
        version = package.get("version")
        source = package.get("source")
        if (not isinstance(manifest, str) or not isinstance(name, str) or not isinstance(version, str)
                or (source is not None and not isinstance(source, str))):
            raise LocalCoverageContextError("cargo_dependency_graph_invalid")
        manifest_path = Path(manifest)
        try:
            if (stat.S_ISLNK(manifest_path.lstat().st_mode)
                    or stat.S_ISLNK(manifest_path.parent.lstat().st_mode)):
                raise LocalCoverageContextError("cargo_dependency_source_symlink")
        except OSError as error:
            raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
        try:
            package_root = manifest_path.parent.resolve(strict=True)
            package_root.relative_to(repo_root)
            inside_repo = True
        except ValueError:
            inside_repo = False
        except OSError as error:
            raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
        if inside_repo:
            continue
        try:
            manifest_path.resolve(strict=True).relative_to(package_root)
        except (OSError, ValueError) as error:
            raise LocalCoverageContextError("cargo_dependency_source_unavailable") from error
        source_digest = _sha256((source or "path-dependency").encode())
        package_identity = {"name": name, "version": version, "sourceSha256": source_digest,
                            "rootPathSha256": _path_id(package_root)}
        external.setdefault(package_root, []).append(package_identity)
    sorted_external = sorted(external.items(), key=lambda item: str(item[0]))
    unseen_roots = [package_root for package_root, _ in sorted_external
                    if package_root not in dependency_digests]
    if len(unseen_roots) > 1:
        with ThreadPoolExecutor(max_workers=3) as executor:
            futures = [executor.submit(_dependency_tree_digest, package_root)
                       for package_root in unseen_roots]
            try:
                unseen_digests = [future.result() for future in futures]
            except BaseException:
                for future in futures:
                    future.cancel()
                raise
    else:
        unseen_digests = [_dependency_tree_digest(package_root) for package_root in unseen_roots]
    computed_digests = dict(zip(unseen_roots, unseen_digests, strict=True))
    pending_records: dict[str, dict[str, object]] = {
        f"cargo-resolution:{label}": {
            "sha256": _sha256(_canonical(resolution_rows)), "size": len(resolution_rows),
        },
    }
    for package_root, identities in sorted_external:
        digest = dependency_digests.get(package_root)
        if digest is None:
            digest = computed_digests[package_root]
        tree_digest, entry_count, _ = digest
        for identity in identities:
            package_key = _sha256(_canonical(identity))
            pending_records[f"cargo-package:{label}:{package_key}"] = {
                "sha256": tree_digest, "entryCount": entry_count,
            }
    dependency_digests.update(computed_digests)
    records.update(pending_records)


def _record_cargo_dependencies(repo: Path, records: dict[str, dict[str, object]],
                               profile: dict[str, object] | None = None) -> None:
    host_output = _run(["rustc", "-vV"])
    host = next((line.removeprefix("host: ") for line in host_output.splitlines()
                 if line.startswith("host: ")), "")
    if not host or any(character.isspace() for character in host):
        raise LocalCoverageContextError("rust_target_unavailable")
    resolved_domains: list[tuple[str, object]] = []
    metadata_seconds: dict[str, float] = {}
    for label, argv in _cargo_metadata_commands(repo, host):
        started = time.perf_counter()
        try:
            output = subprocess.run(argv, cwd=repo, text=True, capture_output=True,
                                    check=False, timeout=60)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise LocalCoverageContextError("cargo_dependency_metadata_unavailable") from error
        if output.returncode != 0:
            raise LocalCoverageContextError("cargo_dependency_metadata_unavailable")
        try:
            metadata = json.loads(output.stdout)
        except json.JSONDecodeError as error:
            raise LocalCoverageContextError("cargo_dependency_graph_invalid") from error
        metadata_seconds[label] = time.perf_counter() - started
        resolved_domains.append((label, metadata))

    dependency_digests: dict[Path, tuple[str, int, int]] = {}
    hash_started = time.perf_counter()
    for label, metadata in resolved_domains:
        _record_cargo_metadata(repo, label, metadata, records, dependency_digests)
    if profile is not None:
        profile["cargoMetadataSecondsByDomain"] = metadata_seconds
        profile["cargoDependencyHashSeconds"] = time.perf_counter() - hash_started
        profile["cargoDependencyRootCount"] = len(dependency_digests)
        profile["cargoDependencyEntryCount"] = sum(item[1] for item in dependency_digests.values())
        profile["cargoDependencyFileBytes"] = sum(item[2] for item in dependency_digests.values())


def _collect_tool_stage(repo: Path, env: Mapping[str, str]) -> tuple[dict[str, str], dict[str, dict[str, object]], float]:
    started = time.perf_counter()
    records: dict[str, dict[str, object]] = {}
    tools: dict[str, str] = {}
    tools["rustc"] = _record_command("rustc", ["rustc", "--version"], records, "rustc")
    tools["cargo"] = _record_command("cargo", ["cargo", "--version"], records, "cargo")
    tools["cargoLlvmCov"] = _record_command("cargo-llvm-cov", ["cargo", "llvm-cov", "--version"], records, "cargoLlvmCov")
    tools["cargoNextest"] = _record_command("cargo-nextest", ["cargo", "nextest", "--version"], records, "cargoNextest")
    tools["llvm"] = ""
    tools["java"] = _record_command("java", ["java", "-version"], records, "java")
    tools["bun"] = _record_command("bun", ["bun", "--version"], records, "bun")
    tools["node"] = _record_command("node", ["node", "--version"], records, "node")
    tools["python"] = _record_command("python3", ["python3", "--version"], records, "python")
    tools["graphviz"] = _record_command("dot", ["dot", "-V"], records, "graphviz")
    tools["plantuml"] = _record_plantuml(repo, records, env)
    tools["llvm"] = _rust_tool_paths(records)
    system_profiler = Path("/usr/sbin/system_profiler")
    _record_file(system_profiler, records, "tool:system-profiler")
    _record_macho_closure(system_profiler, records, "system-profiler")
    java_home = Path(_run(["/usr/libexec/java_home"])).resolve(strict=True)
    java_home_tool = Path("/usr/libexec/java_home")
    _record_file(java_home_tool, records, "tool:java-home")
    for relative in ("release", "lib/modules", "lib/server/libjvm.dylib"):
        runtime_file = java_home / relative
        _record_file(runtime_file, records, f"java-runtime:{relative}")
        if relative.endswith(".dylib"):
            _record_macho_closure(runtime_file, records, "java-runtime")
    for base in (repo, *repo.parents):
        for relative in (".cargo/config", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"):
            candidate = base / relative
            if candidate.exists() or candidate.is_symlink():
                _record_file(candidate, records, f"repo-config:{_path_id(candidate)}")
    for config_path in (Path.home() / ".cargo" / "config", Path.home() / ".cargo" / "config.toml",
                        Path.home() / ".rustup" / "settings.toml"):
        if config_path.exists() or config_path.is_symlink():
            _record_file(config_path, records, f"user-config:{_path_id(config_path)}")
    return tools, records, time.perf_counter() - started


def _collect_cargo_stage(repo: Path) -> tuple[dict[str, dict[str, object]], dict[str, object], float]:
    started = time.perf_counter()
    records: dict[str, dict[str, object]] = {}
    profile: dict[str, object] = {}
    _record_cargo_dependencies(repo, records, profile)
    return records, profile, time.perf_counter() - started


def _collect_font_stage() -> tuple[dict[str, dict[str, object]], float]:
    started = time.perf_counter()
    records: dict[str, dict[str, object]] = {}
    _record_fonts(records)
    return records, time.perf_counter() - started


def collect_font_inventory() -> tuple[str, int]:
    """Return the validated macOS font inventory digest and record count."""
    records: dict[str, dict[str, object]] = {}
    _record_fonts(records)
    return _sha256(_canonical(records)), len(records)


def _merge_stage_records(stage_records: tuple[dict[str, dict[str, object]], ...]) -> dict[str, dict[str, object]]:
    records: dict[str, dict[str, object]] = {}
    for stage in stage_records:
        overlap = records.keys() & stage.keys()
        if overlap:
            raise LocalCoverageContextError("identity_record_collision")
        records.update(stage)
    return records


def _collect_stages(repo: Path, env: Mapping[str, str]) -> tuple[
    dict[str, str], dict[str, dict[str, object]], dict[str, object], dict[str, float]
]:
    with ThreadPoolExecutor(max_workers=CONTEXT_STAGE_WORKERS) as executor:
        tool_future = executor.submit(_collect_tool_stage, repo, env)
        cargo_future = executor.submit(_collect_cargo_stage, repo)
        font_future = executor.submit(_collect_font_stage)
        tools, tool_records, tool_seconds = tool_future.result()
        cargo_records, cargo_profile, cargo_seconds = cargo_future.result()
        font_records, font_seconds = font_future.result()
    records = _merge_stage_records((tool_records, cargo_records, font_records))
    timings = {
        "toolAndRuntimeIdentitySeconds": tool_seconds,
        "cargoMetadataAndDependencySeconds": cargo_seconds,
        "fontInventorySeconds": font_seconds,
    }
    return tools, records, cargo_profile, timings


def _collect(repo: Path, env: Mapping[str, str], profile: dict[str, object] | None = None) -> dict[str, object]:
    started = time.perf_counter()
    if platform.system() != "Darwin" or platform.machine() not in {"arm64", "aarch64"}:
        raise LocalCoverageContextError("unsupported_host_profile")
    tools, records, cargo_profile, stage_timings = _collect_stages(repo, env)
    environment_sha = _record_environment(env)
    host = {
        "os": "macOS", "productVersion": _run(["sw_vers", "-productVersion"]),
        "buildVersion": _run(["sw_vers", "-buildVersion"]), "architecture": platform.machine(),
    }
    resources_sha = _sha256(_canonical(records))
    if profile is not None:
        profile.update(cargo_profile)
        profile.update({
            "schema": "krr-native-local-coverage-profile-v1",
            "cacheable": True,
            "recordCount": len(records),
            **stage_timings,
            "totalSeconds": time.perf_counter() - started,
        })
    return {
        "schema": SCHEMA, "cacheable": True, "missReasons": [], "host": host, "tools": tools,
        "environmentSha256": environment_sha,
        "resourcesSha256": resources_sha,
    }


def collect_local_context(repo: Path, env: Mapping[str, str] | None = None) -> dict[str, object]:
    """Return a stable local context; unsupported/incomplete hosts are non-cacheable."""
    active_env = os.environ if env is None else env
    profile_path = active_env.get("KRR_LOCAL_COVERAGE_PROFILE_PATH", "")
    profile: dict[str, object] | None = {} if profile_path else None
    try:
        context = _collect(repo.resolve(strict=True), active_env, profile)
    except (LocalCoverageContextError, OSError, ValueError, RuntimeError, UnicodeError, KeyError, TypeError, IndexError) as error:
        reason = str(error) if isinstance(error, LocalCoverageContextError) else "host_context_unavailable"
        if reason not in MISS_REASON_CODES:
            reason = "host_context_unavailable"
        context = {
            "schema": SCHEMA, "cacheable": False, "missReasons": [reason],
            "host": {"os": platform.system(), "productVersion": "unknown", "buildVersion": "unknown",
                     "architecture": platform.machine()},
            "tools": {key: "unknown" for key in TOOLS},
            "environmentSha256": _sha256(b"unavailable"), "resourcesSha256": _sha256(b"unavailable"),
        }
    if profile is not None:
        profile.setdefault("schema", "krr-native-local-coverage-profile-v1")
        profile["cacheable"] = context["cacheable"]
        profile["missReasons"] = context["missReasons"]
        try:
            profile_fd = os.open(profile_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                                 getattr(os, "O_NOFOLLOW", 0), 0o600)
            with os.fdopen(profile_fd, "wb") as output:
                output.write(_canonical(profile) + b"\n")
        except OSError as error:
            raise LocalCoverageContextError("host_context_unavailable") from error
    context["hostContextSha256"] = _sha256(_canonical(context))
    return context
