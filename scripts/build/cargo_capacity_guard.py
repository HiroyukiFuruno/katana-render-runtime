#!/usr/bin/env python3
"""Run a Cargo command while guarding the repository's real target directory.

The guard records one successful build's target size and only removes rebuild
state after the measured target exceeds the configured multiple of that fixed
baseline.  It never follows a symlink target and refuses to clean while a
process appears to use the target.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import logging
import os
import math
import re
import subprocess
import shutil
import stat
import sys
import time
from functools import partial
from pathlib import Path
from collections.abc import Callable, Iterator, Mapping
from types import FunctionType, ModuleType
from dataclasses import replace
from typing import TextIO, cast

from cargo_capacity_guard_build_script_ownership import BuildScriptOwnership, build_script_ownership
from cargo_capacity_guard_build_script_ownership import SourcePin, source_pin
from cargo_capacity_guard_selected_cache_ownership import (
    SelectedCacheOwnership, _tool_pin, create_selected_cache_ownership,
)
from cargo_capacity_guard_library_alias_ownership import LibraryAliasOwnership, create_library_alias_ownership
from cargo_capacity_guard_debug_object_ownership import PROGRESS_LOGGER_NAME
from cargo_capacity_guard_lock import (GuardLock, ProfileLocks, busy_processes,
                                       evict_artifacts, is_successful_build, retained_profile_paths)
import cargo_capacity_guard_state as capacity_state

DEFAULT_FACTOR = 2.0
COVERAGE_DOMAINS = ("root", "performance-baseline", "performance-candidate")
CHECK_DOMAINS = COVERAGE_DOMAINS
CLIPPY_DENY_FLAGS = [
    "-D", "warnings", "-D", "clippy::unwrap_used", "-D", "clippy::expect_used",
    "-D", "clippy::todo", "-D", "clippy::unimplemented", "-D", "clippy::dbg_macro",
    "-D", "clippy::panic", "-D", "clippy::wildcard_imports", "-D", "clippy::too_many_lines",
    "-D", "clippy::cognitive_complexity",
]
CAPACITY_METRIC = "unique-inode-allocated-bytes"
CAPACITY_METRIC_VERSION = 1
JsonObject = dict[str, object]
SWIFTPM_RELEASE_LIBRARIES = {"apple-cf": "AppleCFBridge", "coretext-rs": "CoreTextBridge"}
SWIFTPM_COVERAGE_TARGETS = frozenset({
    "llvm-cov-target", "llvm-cov-performance-baseline", "llvm-cov-performance-candidate",
})


def _write_text(stream: TextIO, value: str) -> None:
    try:
        descriptor = stream.fileno()
    except (AttributeError, OSError, ValueError):
        if stream.write(value) != len(value):
            raise OSError("short write while reporting capacity result") from None
        stream.flush()
        return
    was_blocking = os.get_blocking(descriptor)
    try:
        if not was_blocking:
            os.set_blocking(descriptor, True)
        if stream.write(value) != len(value):
            raise OSError("short write while reporting capacity result")
        stream.flush()
    finally:
        if not was_blocking:
            os.set_blocking(descriptor, False)


@contextlib.contextmanager
def _progress_logging(enabled: bool) -> Iterator[None]:
    logger = logging.getLogger(PROGRESS_LOGGER_NAME)
    original_level = logger.level
    original_propagate = logger.propagate
    original_handlers = list(logger.handlers)
    handler = logging.StreamHandler(sys.stderr) if enabled else None
    try:
        logger.setLevel(logging.INFO if enabled else logging.WARNING)
        logger.propagate = False
        logger.handlers[:] = [handler] if handler is not None else []
        yield
    finally:
        logger.handlers[:] = original_handlers
        logger.setLevel(original_level)
        logger.propagate = original_propagate
        if handler is not None:
            handler.close()


def _object(value: object, label: str) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise RuntimeError(f"{label} must be a JSON object")
    return cast(JsonObject, value)


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise RuntimeError(f"{label} must be a JSON array")
    return cast(list[object], value)


def _text(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be a string")
    return value


def _number(value: object, label: str) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise RuntimeError(f"{label} must be a finite number")
    return value


def _integer(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise RuntimeError(f"{label} must be an integer")
    return value


def _root() -> Path:
    return Path(os.environ.get("CARGO_CAPACITY_GUARD_ROOT", Path(__file__).resolve().parents[2])).resolve()


def resolve_target(root: Path, value: str | None = None, incremental_prefixes: list[str] | None = None,
                   inactive_prefixes: list[str] | None = None, build_metadata: JsonObject | None = None,
                   metadata_timeout: float | None = None) -> Path:
    env_target = os.environ.get("CARGO_TARGET_DIR")
    if value is None and env_target is not None and not Path(env_target).is_absolute():
        raise RuntimeError("CARGO_TARGET_DIR must be absolute")
    configured = value or env_target
    if configured is None:
        manifest = root / "Cargo.toml"
        if metadata_timeout is not None:
            try:
                metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)], cwd=root, text=True, capture_output=True, timeout=metadata_timeout, check=False)
            except subprocess.TimeoutExpired as error:
                raise RuntimeError("native target metadata deadline exceeded") from error
        else:
            metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)], cwd=root, text=True, capture_output=True, check=False)
        if metadata.returncode != 0:
            raise RuntimeError("cargo metadata failed while resolving target")
        try:
            metadata_value = _object(json.loads(metadata.stdout), "cargo metadata")
            configured = _text(metadata_value["target_directory"], "cargo target directory")
        except (KeyError, json.JSONDecodeError) as exc:
            raise RuntimeError("cargo metadata returned no target directory") from exc
    raw = Path(configured)
    target = raw if raw.is_absolute() else root / raw
    root_real = root.resolve()
    # The target itself is checked before resolution; ancestor links are
    # covered by the resolved containment check below (a link escaping the
    # repository can never produce an in-repository resolved target).
    if target.is_symlink():
        raise RuntimeError(f"refusing symlink target component: {target}")
    lexical = Path(os.path.abspath(str(target)))
    try:
        relative_lexical = lexical.relative_to(root_real)
    except ValueError:
        relative_lexical = None
    if relative_lexical is not None:
        cursor = root_real
        for part in relative_lexical.parts:
            cursor /= part
            if cursor.is_symlink():
                raise RuntimeError(f"refusing symlink target component: {cursor}")
    if target == root_real:
        raise RuntimeError("target must be a child of repository root")
    if target.exists():
        for child in target.rglob("*"):
            if child.is_symlink() and _swiftpm_release_alias_stat(child, target) is None:
                raise RuntimeError(f"refusing symlink inside target: {child}")
    target = target.resolve()
    try:
        target.relative_to(root_real)
    except ValueError as exc:
        raise RuntimeError(f"target escapes repository: {target}") from exc
    manifest = root_real / "Cargo.toml"
    if manifest.exists():
        if metadata_timeout is not None:
            try:
                metadata = subprocess.run(
                    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)],
                    cwd=root_real, text=True, capture_output=True, timeout=metadata_timeout, check=False,
                )
            except subprocess.TimeoutExpired as error:
                raise RuntimeError("native validation metadata deadline exceeded") from error
        else:
            metadata = subprocess.run(
                ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)],
                cwd=root_real, text=True, capture_output=True, check=False,
            )
        if metadata.returncode != 0:
            raise RuntimeError("cargo metadata failed while validating target")
        metadata_value = _object(json.loads(metadata.stdout), "cargo metadata")
        if build_metadata is not None:
            build_metadata.update(metadata_value)
        expected = Path(_text(metadata_value["target_directory"], "cargo target directory")).resolve()
        if expected != target:
            raise RuntimeError(f"target differs from cargo metadata: {target} != {expected}")
        if incremental_prefixes is not None:
            incremental_prefixes.extend(runtime_incremental_prefixes(root_real, metadata_value))
        if inactive_prefixes is not None:
            inactive_prefixes.extend(workspace_incremental_prefixes(root_real, metadata_value))
    return target


def _incremental_names(package: JsonObject, scope: str, inventory: bool = False) -> list[str]:
    names = []
    targets = _array(package.get("targets"), f"{scope} targets")
    if inventory and not targets:
        raise RuntimeError(f"{scope} targets are incomplete")
    for item in targets:
        target = _object(item, f"{scope} target")
        kinds = [_text(kind, f"{scope} target kind")
                 for kind in _array(target.get("kind"), f"{scope} target kinds")]
        if inventory and (not kinds or not set(kinds).issubset({
            "bin", "lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro",
            "example", "test", "bench", "custom-build",
        })):
            raise RuntimeError(f"{scope} target kinds are incomplete or unknown")
        if "custom-build" in kinds and not inventory:
            continue
        name = target.get("name")
        pattern = r"[A-Za-z0-9_][A-Za-z0-9_-]*" if inventory else r"[A-Za-z_][A-Za-z0-9_-]*"
        if not isinstance(name, str) or not re.fullmatch(pattern, name):
            raise RuntimeError(f"{scope} target crate name is invalid")
        names.append(name.replace("-", "_") + "-")
        if inventory and "custom-build" in kinds:
            # Cargo の汎用 build script 名は metadata target 名にかかわらず保護する。
            names.append("build_script_build-")
    return names


def runtime_incremental_prefixes(root: Path, metadata: object) -> list[str]:
    value = _object(metadata, "cargo metadata")
    manifest = str(root / "crates/katana-render-runtime/Cargo.toml")
    packages = [_object(package, "cargo package")
                for package in _array(value.get("packages", []), "cargo packages")]
    packages = [package for package in packages
                if package.get("manifest_path") == manifest]
    if not packages:
        return []
    if len(packages) != 1:
        raise RuntimeError("runtime package metadata is not unique")
    return sorted(set(_incremental_names(packages[0], "runtime")))


def cargo_configuration_paths(root: Path, environment: Mapping[str, str]) -> list[Path]:
    home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo")))
    if not home.is_absolute():
        raise RuntimeError("retained build profile requires absolute CARGO_HOME")
    bases = [directory / ".cargo" for directory in (root, *root.parents)] + [home]
    paths = []
    for base in dict.fromkeys(bases):
        for name in ("config", "config.toml"):
            path = base / name
            try:
                path.lstat()
            except FileNotFoundError:
                continue
            except OSError as exc:
                raise RuntimeError("Cargo configuration selection is unavailable") from exc
            paths.append(path)
            break
    return paths


def selected_package_incremental_prefixes(root: Path, metadata: object) -> list[str]:
    value = _object(metadata, "selected package metadata")
    members = {_text(item, "workspace member")
               for item in _array(value["workspace_members"], "workspace members")}
    packages: dict[str, JsonObject] = {}
    namespaces: dict[str, set[str]] = {}
    custom_names = {"build_script_build-"}
    for item in _array(value["packages"], "selected packages"):
        package = _object(item, "selected package")
        identity = _text(package["id"], "package id")
        if identity in packages:
            raise RuntimeError("duplicate selected package metadata")
        packages[identity] = package
        for name in _incremental_names(package, "selected package", inventory=True):
            namespaces.setdefault(name, set()).add(identity)
        for target in _array(package["targets"], "inventory targets"):
            if "custom-build" in _array(_object(target, "inventory target")["kind"], "inventory target kinds"):
                custom_names.update(_incremental_names({"targets": [target]}, "custom-build", inventory=True))
    if not members.issubset(packages):
        raise RuntimeError("workspace package metadata is incomplete")
    selected = [identity for identity in members
                if packages[identity].get("manifest_path") == str(root / "crates/kdr-linter/Cargo.toml")
                and packages[identity].get("name") == "kdr-linter"]
    if len(selected) != 1 or sum(
        _object(item, "selected target").get("name") == "mathjax_consumer_facts"
        and _object(item, "selected target").get("kind") == ["bin"]
        for item in _array(packages[selected[0]]["targets"], "selected targets")
    ) != 1:
        raise RuntimeError("canonical selected host package is missing")
    resolve = _object(value["resolve"], "selected resolve")
    edges: dict[str, list[str]] = {}
    for item in _array(resolve["nodes"], "resolve nodes"):
        node = _object(item, "resolve node")
        identity = _text(node["id"], "resolve id")
        if identity in edges or identity not in packages:
            raise RuntimeError("duplicate or unknown resolve node")
        edges[identity] = [_text(_object(dep, "resolve dependency")["pkg"], "dependency package")
                           for dep in _array(node["deps"], "resolve dependencies")]
    if set(edges) != set(packages) or any(dep not in edges for deps in edges.values() for dep in deps):
        raise RuntimeError("selected dependency graph is incomplete")
    closure: set[str] = set()
    pending = selected.copy()
    while pending:
        identity = pending.pop()
        if identity not in closure:
            closure.add(identity)
            pending.extend(edges[identity])
    eligible: set[str] = set()
    for identity in members - closure:
        manifest = Path(_text(packages[identity]["manifest_path"], "workspace manifest"))
        if not manifest.is_absolute() or manifest.resolve() != manifest or not manifest.is_relative_to(root):
            raise RuntimeError("workspace manifest is not canonical and contained")
        eligible.update(name for name in _incremental_names(packages[identity], "unselected workspace")
                        if name not in custom_names and namespaces[name] == {identity})
    return sorted(eligible)


def _selected_input_check(root: Path, metadata: JsonObject, *, include_target_inputs: bool = False) -> Callable[[], None]:
    environment = os.environ.copy()
    configurations = cargo_configuration_paths(root, environment)
    members = {_text(item, "workspace member")
               for item in _array(metadata["workspace_members"], "workspace members")}
    manifests = [root / "Cargo.toml", root / "Cargo.lock"]
    for item in _array(metadata["packages"], "cargo packages"):
        package = _object(item, "cargo package")
        if package.get("id") in members:
            manifest = Path(_text(package["manifest_path"], "workspace manifest"))
            if not manifest.is_absolute() or manifest.resolve() != manifest or not manifest.is_relative_to(root):
                raise RuntimeError("workspace manifest is not canonical and contained")
            manifests.append(manifest)
    # glob workspace と config の追加も入力変更として検出し、metadata 後の依存追加を見逃さない。
    directories = ({parent for path in manifests for parent in (path.parent, *path.parents)
                    if parent.is_relative_to(root)} if not include_target_inputs else set())
    source_roots: set[Path] = set()
    optional_paths: set[Path] = set()
    for item in _array(metadata["packages"], "cargo packages"):
        package = _object(item, "cargo package")
        if package.get("id") not in members:
            continue
        package_root = Path(_text(package["manifest_path"], "workspace manifest")).parent
        if include_target_inputs:
            source_roots.update(package_root / name for name in ("src", "examples", "tests", "benches"))
        build_script = package_root / "build.rs"
        if include_target_inputs:
            optional_paths.add(build_script)
        if include_target_inputs:
            for target_item in _array(package.get("targets"), "Cargo target inputs"):
                target = _object(target_item, "Cargo target input")
                source = Path(_text(target.get("src_path"), "Cargo target source path"))
                if (not source.is_absolute() or source.resolve() != source
                        or not source.is_relative_to(package_root)):
                    raise RuntimeError("Cargo target source path is not canonical and package-contained")
                manifests.append(source)
    source_files: list[Path] = []
    for source_root in source_roots:
        if source_root.is_relative_to(root):
            directories.add(source_root)
            if source_root.exists():
                if source_root.is_symlink():
                    raise RuntimeError("selected target source tree contains a symlink")
                for directory, child_directories, file_names in os.walk(source_root, followlinks=False):
                    current = Path(directory)
                    directories.add(current)
                    if any((current / child).is_symlink() for child in child_directories):
                        raise RuntimeError("selected target source tree contains a symlink")
                    source_files.extend(current / name for name in file_names)
    config_bases = set() if include_target_inputs else {
        directory / ".cargo" for directory in (root, *root.parents)
    }
    if not include_target_inputs:
        config_bases.add(Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo"))))
    paths = sorted(set(manifests + configurations + source_files))

    def stamp(value: os.stat_result) -> tuple[int, ...]:
        return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
                value.st_size, value.st_mtime_ns, value.st_ctime_ns)

    def snapshot() -> tuple[list[tuple[Path, tuple[int, ...], str]],
                            dict[Path, list[str] | None], dict[Path, list[str] | None]]:
        files = []
        for path in paths:
            descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
            try:
                before = os.fstat(descriptor)
                if not stat.S_ISREG(before.st_mode):
                    raise RuntimeError("selected metadata input is not a regular file")
                with os.fdopen(os.dup(descriptor), "rb") as handle:
                    digest = hashlib.sha256(handle.read()).hexdigest()
                after = os.fstat(descriptor)
                if stamp(before) != stamp(after) or stamp(path.lstat()) != stamp(after):
                    raise RuntimeError("selected metadata input changed while reading")
                files.append((path, stamp(after), digest))
            finally:
                os.close(descriptor)
        for path in sorted(optional_paths):
            try:
                descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
            except FileNotFoundError:
                files.append((path, (-1,), ""))
                continue
            try:
                before = os.fstat(descriptor)
                if not stat.S_ISREG(before.st_mode):
                    raise RuntimeError("optional Cargo build script is not a regular file")
                with os.fdopen(os.dup(descriptor), "rb") as handle:
                    digest = hashlib.sha256(handle.read()).hexdigest()
                after = os.fstat(descriptor)
                if stamp(before) != stamp(after) or stamp(path.lstat()) != stamp(after):
                    raise RuntimeError("optional Cargo build script changed while reading")
                files.append((path, stamp(after), digest))
            finally:
                os.close(descriptor)
        return (files, {path: sorted(os.listdir(path)) if path.is_dir() else None for path in directories},
                {path: sorted(os.listdir(path)) if path.exists() else None for path in config_bases})

    initial = snapshot()

    def verify() -> None:
        current_environment = os.environ.copy()
        current_configurations = cargo_configuration_paths(root, environment)
        current = snapshot()
        environment_keys = sorted(
            key for key in environment.keys() | current_environment.keys()
            if environment.get(key) != current_environment.get(key)
        )
        if (environment_keys or current_configurations != configurations or current != initial):
            try:
                diagnostic_configurations = cargo_configuration_paths(root, current_environment)
            except (OSError, RuntimeError):
                diagnostic_configurations = configurations
            initial_files, initial_directories, initial_config_entries = initial
            current_files, current_directories, current_config_entries = current
            initial_file_map = {path: (file_stamp, digest) for path, file_stamp, digest in initial_files}
            current_file_map = {path: (file_stamp, digest) for path, file_stamp, digest in current_files}

            def bounded(items: set[str]) -> list[str]:
                ordered = sorted(items)
                result = [item if len(item) <= 128 else f"{item[:64]}...{item[-64:]}"
                          for item in ordered[:3]]
                if len(ordered) > len(result):
                    result.append(f"... (+{len(ordered) - len(result)} more)")
                return result

            diagnostics = [f"environment_keys_changed={bounded(set(environment_keys))}"]
            added_configs = {str(path) for path in diagnostic_configurations} - {str(path) for path in configurations}
            removed_configs = {str(path) for path in configurations} - {str(path) for path in diagnostic_configurations}
            if added_configs or removed_configs:
                diagnostics.append(
                    f"config_paths_added={bounded(added_configs)} config_paths_removed={bounded(removed_configs)}"
                )
            changed_files = {
                str(path) for path in initial_file_map.keys() | current_file_map.keys()
                if initial_file_map.get(path) != current_file_map.get(path)
            }
            if changed_files:
                diagnostics.append(f"input_files_changed={bounded(changed_files)}")

            def entry_deltas(
                before: Mapping[Path, list[str] | None],
                after: Mapping[Path, list[str] | None],
            ) -> tuple[set[str], set[str]]:
                added: set[str] = set()
                removed: set[str] = set()
                for directory in before.keys() | after.keys():
                    before_names = set(before.get(directory) or [])
                    after_names = set(after.get(directory) or [])
                    added.update(f"{directory / name}" for name in after_names - before_names)
                    removed.update(f"{directory / name}" for name in before_names - after_names)
                return added, removed

            added_entries, removed_entries = entry_deltas(initial_directories, current_directories)
            if added_entries or removed_entries:
                diagnostics.append(
                    f"directory_entries_added={bounded(added_entries)} directory_entries_removed={bounded(removed_entries)}"
                )
            added_entries, removed_entries = entry_deltas(initial_config_entries, current_config_entries)
            if added_entries or removed_entries:
                diagnostics.append(
                    f"config_entries_added={bounded(added_entries)} config_entries_removed={bounded(removed_entries)}"
                )
            changed_config_bases = {
                str(path) for path in initial_config_entries.keys() | current_config_entries.keys()
                if (initial_config_entries.get(path) is None) != (current_config_entries.get(path) is None)
            }
            if changed_config_bases:
                diagnostics.append(f"config_base_presence_changed={bounded(changed_config_bases)}")
            message = "selected metadata inputs changed; artifacts retained"
            detail = "; ".join(diagnostics)
            raise RuntimeError(f"{message}; diagnostics: {detail[:4096]}")

    return verify


def selected_namespace_supported(root: Path) -> bool:
    # crate 名を変える設定の下では、Cargo target 名による namespace 証明を使わない。
    if (any(os.environ.get(key) for key in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTC_BOOTSTRAP", "CARGO_ENCODED_RUSTFLAGS"))
            or any(value for key, value in os.environ.items()
                   if key.startswith("CARGO_BUILD_RUST") or key.startswith("CARGO_TARGET_") and key.endswith("_RUSTFLAGS"))
            or os.environ.get("RUSTFLAGS", "") not in ("", "-D warnings")):
        return False
    for path in cargo_configuration_paths(root, os.environ):
        configuration = _cargo_configuration(path)
        build = _object(configuration.get("build", {}), "Cargo build configuration")
        if (any(key in build for key in ("rustc", "rustc-wrapper", "rustc-workspace-wrapper", "rustflags"))
                or "target" in configuration):
            return False
    return True


def selected_package_cleanup(
    root: Path, target: Path, command: list[str], metadata: JsonObject,
    capture_ownership: Callable[[BuildScriptOwnership], None] | None = None,
    capture_selected_cache: Callable[[SelectedCacheOwnership], None] | None = None,
) -> tuple[list[str], Callable[[], None] | None]:
    cargo = list(command)
    if cargo and Path(cargo[0]).name == "rtk":
        cargo.pop(0)
        if cargo and cargo[0] == "proxy":
            cargo.pop(0)
    if (not cargo or Path(cargo[0]).name != "cargo" or cargo[1:] not in (
        ["build", "--locked", "-p", "kdr-linter", "--bin", "mathjax_consumer_facts"],
        ["build", "--offline", "--locked", "-p", "kdr-linter", "--bin", "mathjax_consumer_facts"],
    )):
        return [], None
    verify = _selected_input_check(root, metadata)
    verify()
    if not selected_namespace_supported(root):
        return [], None
    verify()
    result = subprocess.run(
        [cargo[0], "metadata", "--offline", "--locked", "--all-features", "--format-version", "1",
         "--manifest-path", str(root / "Cargo.toml")],
        cwd=root, env=os.environ.copy(), text=True, capture_output=True, check=False,
    )
    if result.returncode != 0:
        raise RuntimeError("selected dependency metadata is unavailable; artifacts retained")
    full = _object(json.loads(result.stdout), "selected dependency metadata")
    if (full.get("target_directory") != str(target) or full.get("build_directory") != metadata.get("build_directory")
            or full.get("workspace_members") != metadata.get("workspace_members")
            or sorted((_text(_object(item, "package")["id"], "package id"),
                       _text(_object(item, "package")["manifest_path"], "manifest path"))
                      for item in _array(full["packages"], "packages")
                      if _object(item, "package")["id"] in _array(full["workspace_members"], "workspace members"))
            != sorted((_text(_object(item, "package")["id"], "package id"),
                       _text(_object(item, "package")["manifest_path"], "manifest path"))
                      for item in _array(metadata["packages"], "packages"))):
        raise RuntimeError("selected dependency metadata differs from retained build inputs")
    prefixes = selected_package_incremental_prefixes(root, full)
    verify()
    if capture_ownership is not None:
        proof = build_script_ownership(root, target, full, verify)
        if capture_selected_cache is not None:
            original_orphan = proof
            selected_cache = create_selected_cache_ownership(root, target, full, verify, cargo[0])
            original_cache_check = selected_cache.verify

            def verify_combined() -> None:
                original_orphan.verify()
                original_cache_check()

            proof = replace(original_orphan, verify=verify_combined)
            capture_selected_cache(replace(selected_cache, verify=verify_combined))
        capture_ownership(proof)
        return prefixes, proof.verify
    return prefixes, verify


def _cargo_configuration(path: Path) -> JsonObject:
    parser_root = Path(__file__).resolve().parents[1] / "release"
    cached = sys.modules.get("toml_loader")
    if cached is not None:
        if type(cached) is not ModuleType or "__getattr__" in vars(cached):
            raise ImportError("Cargo configuration parser cache has an unsupported module")
        factory = vars(cached).get("load_toml_parser")
        if not isinstance(factory, FunctionType) or Path(factory.__code__.co_filename).resolve() != parser_root / "toml_loader.py":
            raise ImportError("Cargo configuration parser cache has a foreign origin")
    # build CLI からも検証済みの共有 parser を使い、探索範囲を残さない。
    sys.path.insert(0, str(parser_root))
    try:
        from toml_loader import load_toml_parser
    finally:
        sys.path.remove(str(parser_root))
    if not isinstance(load_toml_parser, FunctionType) or Path(load_toml_parser.__code__.co_filename).resolve() != parser_root / "toml_loader.py":
        raise ImportError("Cargo configuration parser has a foreign origin")
    return _object(load_toml_parser().loads(path.read_text(encoding="utf-8")), "Cargo configuration")


def validate_build_selection(root: Path, environment: Mapping[str, str] | None = None) -> None:
    environment = os.environ if environment is None else environment
    if "CARGO_BUILD_TARGET" in environment:
        raise RuntimeError("retained build profile does not support environment target selection")
    for path in cargo_configuration_paths(root, environment):
        try:
            config = _cargo_configuration(path)
        except (ImportError, OSError, ValueError) as exc:
            raise RuntimeError("Cargo target selection cannot be verified") from exc
        build = config.get("build", {})
        unstable = config.get("unstable", {})
        if not isinstance(unstable, dict) or unstable:
            raise RuntimeError("retained build profile does not support unstable Cargo configuration")
        if not isinstance(build, dict) or "include" in config or "target" in build:
            raise RuntimeError("retained build profile does not support configured or unknown target selection")


def validate_build_profile(profile: str, command: list[str]) -> None:
    if not is_successful_build(command) or Path(command[0]).name not in {"cargo", "rtk"}:
        raise RuntimeError("retained build profile requires direct Cargo build in the guard environment")
    if any(part.startswith("+") or (part.startswith("-") and not part.startswith("--")
                                    and "Z" in part[1:]) for part in command):
        raise RuntimeError("retained build profile does not support child toolchain or unstable options")
    if any(part in {"--profile", "--target", "--target-dir", "--config", "--manifest-path", "-C"}
           or part.startswith(("--profile=", "--target=", "--target-dir=", "--config=", "--manifest-path=", "-C="))
           or (part.startswith("-") and not part.startswith("--")
               and len(part) > 2 and ("r" in part[1:] or part.startswith("-C"))) for part in command):
        raise RuntimeError("retained build profile does not support child profile/directory/config overrides")
    release = "--release" in command or "-r" in command
    if release != (profile == "release"):
        raise RuntimeError("retained build profile differs from child Cargo build")


def validate_build_executable(root: Path, command: list[str],
                              environment: Mapping[str, str] | None = None) -> None:
    index = 0 if Path(command[0]).name == "cargo" else (2 if command[1] == "proxy" else 1)
    search_path = os.pathsep.join(str(Path(part) if Path(part).is_absolute() else root / part)
                                  for part in (os.get_exec_path() if environment is None else os.get_exec_path(environment)))
    token = command[index]
    has_directory = os.sep in token or (os.altsep is not None and os.altsep in token)
    if Path(token).is_absolute():
        value = str(Path(token))
    elif has_directory:
        value = str(root / token)
    else:
        value = token
    expected = shutil.which("cargo", path=search_path)
    actual = shutil.which(value, path=search_path)
    try:
        if expected is None or actual is None or Path(expected).resolve(strict=True) != Path(actual).resolve(strict=True):
            raise RuntimeError("retained build profile Cargo executable differs from metadata")
    except OSError as exc:
        raise RuntimeError("retained build profile Cargo executable cannot be verified") from exc


def current_build_profiles(target: Path, metadata: object, profile: str) -> tuple[Path, ...]:
    metadata = _object(metadata, "cargo metadata")
    if "target_directory" not in metadata:
        raise RuntimeError("retained build profile requires Cargo metadata")
    values = []
    for value in (metadata["target_directory"], metadata.get("build_directory", metadata["target_directory"])):
        base = Path(_text(value, "cargo build directory"))
        if not base.is_absolute():
            raise RuntimeError("Cargo metadata build directory must be absolute")
        path = base / profile
        try:
            parts = path.relative_to(target).parts
        except ValueError as exc:
            raise RuntimeError("Cargo metadata build profile escapes target") from exc
        if (not parts or ".." in parts or len(parts) > 3
                or (len(parts) == 3 and parts[1] != "build")):
            raise RuntimeError("Cargo metadata build profile is outside known profile scope")
        if path.exists() or path.is_symlink():
            values.append(path)
    return retained_profile_paths(target, values)


def workspace_incremental_prefixes(root: Path, metadata: object) -> list[str]:
    value = _object(metadata, "cargo metadata")
    members = {_text(member, "cargo workspace member")
               for member in _array(value.get("workspace_members", []), "cargo workspace members")}
    names = []
    for item in _array(value.get("packages", []), "cargo packages"):
        package = _object(item, "cargo package")
        package_id = package.get("id")
        if package_id is not None:
            package_id = _text(package_id, "cargo package id")
        if package_id not in members:
            continue
        manifest = Path(_text(package["manifest_path"], "workspace manifest"))
        if not manifest.is_absolute() or manifest.resolve() != manifest:
            raise RuntimeError("workspace manifest must be canonical and absolute")
        if not manifest.is_relative_to(root):
            continue
        names.extend(_incremental_names(package, "workspace"))
    return sorted(set(names))


def _stat_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev, value.st_ino, value.st_mode, value.st_nlink, value.st_size,
        value.st_mtime_ns, value.st_ctime_ns, getattr(value, "st_blocks", -1),
    )


def _path_has_no_symlink_components(path: Path, root: Path) -> bool:
    try:
        relative = path.relative_to(root)
    except ValueError:
        return False
    cursor = root
    if cursor.is_symlink():
        return False
    for part in relative.parts:
        cursor /= part
        if cursor.is_symlink():
            return False
    return True


def _swiftpm_alias_producer_is_pinned(path: Path, target: Path, cargo_root: Path, package: str) -> bool:
    build = cargo_root / "debug" / "build" / package
    swift_root = path.parent
    output = build / "output"
    marker = swift_root / ".buildSystem_release"
    manifest = swift_root / "manifest.pif"
    workspace_state = swift_root / "workspace-state.json"
    expected_link = f"cargo:rustc-link-search=native={path}"
    if not all(_path_has_no_symlink_components(item, target) for item in (cargo_root, build, swift_root)):
        return False
    try:
        output_stat = output.lstat()
        marker_stat = marker.lstat()
        manifest_stat = manifest.lstat()
        workspace_stat = workspace_state.lstat()
        if not all(stat.S_ISREG(value.st_mode) for value in
                   (output_stat, marker_stat, manifest_stat, workspace_stat)):
            return False
        if marker.read_bytes() != b"swiftbuild" or manifest_stat.st_size == 0:
            return False
        state = json.loads(workspace_state.read_text(encoding="utf-8"))
        state_object = _object(state, "SwiftPM workspace state object")
        state_value = _object(state_object.get("object"), "SwiftPM workspace state")
        if (state_object.get("version") != 7
                or not all(isinstance(state_value.get(key), list)
                           for key in ("artifacts", "dependencies", "prebuilts"))):
            return False
        lines = output.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, RuntimeError):
        return False
    crate = package.rsplit("-", 1)[0]
    library = SWIFTPM_RELEASE_LIBRARIES.get(crate)
    return library is not None and expected_link in lines and f"cargo:rustc-link-lib=static={library}" in lines


def _swiftpm_release_alias_stat(path: Path, target: Path) -> os.stat_result | None:
    try:
        canonical_target = target.resolve(strict=True)
        canonical_path = path.parent.resolve(strict=True) / path.name
        relative = canonical_path.relative_to(canonical_target)
        cargo_root = canonical_target
        parts = relative.parts
        if parts and parts[0] in SWIFTPM_COVERAGE_TARGETS:
            cargo_root /= parts[0]
            parts = parts[1:]
        if (len(parts) != 6 or parts[0] != "debug" or parts[1] != "build"
                or re.fullmatch(r"(apple-cf|coretext-rs)-[a-f0-9]{16}", parts[2]) is None
                or parts[3:] != ("out", "swift-build", "release")):
            return None
        value = canonical_path.lstat()
        link_text = os.readlink(canonical_path)
        if not stat.S_ISLNK(value.st_mode) or link_text != "out/Products/Release":
            return None
        swift_root = canonical_path.parent
        expected = swift_root / "out" / "Products" / "Release"
        if (not _path_has_no_symlink_components(swift_root, canonical_target)
                or not _path_has_no_symlink_components(expected, canonical_target)
                or not expected.is_dir() or (canonical_path.parent / link_text).resolve(strict=True) != expected):
            return None
        if not _swiftpm_alias_producer_is_pinned(canonical_path, canonical_target, cargo_root, parts[2]):
            return None
        after = canonical_path.lstat()
        if _stat_identity(value) != _stat_identity(after):
            return None
        return after
    except (OSError, ValueError, RuntimeError):
        return None


def _unique_inode_bytes(value: os.stat_result, seen: set[tuple[int, int]]) -> int | None:
    if not hasattr(value, "st_blocks"):
        return None
    identity = (value.st_dev, value.st_ino)
    if identity in seen:
        return 0
    seen.add(identity)
    return value.st_blocks * 512


def _target_entry_bytes(
    path: Path,
    target: Path,
    seen: set[tuple[int, int]],
    aliases: dict[Path, tuple[int, ...]],
) -> int | None:
    value = path.lstat()
    if stat.S_ISLNK(value.st_mode):
        alias = _swiftpm_release_alias_stat(path, target)
        if alias is not None:
            aliases[path] = _stat_identity(alias)
        return _unique_inode_bytes(alias or value, seen)
    if not stat.S_ISREG(value.st_mode):
        return 0
    return _unique_inode_bytes(value, seen)


def _verify_swiftpm_aliases(aliases: Mapping[Path, tuple[int, ...]], target: Path) -> None:
    for path, stamp in aliases.items():
        current = _swiftpm_release_alias_stat(path, target)
        if current is None or _stat_identity(current) != stamp:
            raise RuntimeError("SwiftPM release alias changed during capacity measurement")


def size_bytes(target: Path) -> int | None:
    if not target.exists():
        return 0
    seen: set[tuple[int, int]] = set()
    aliases: dict[Path, tuple[int, ...]] = {}
    total = 0
    for path in target.rglob("*"):
        entry_bytes = _target_entry_bytes(path, target, seen, aliases)
        if entry_bytes is None:
            return None
        total += entry_bytes
    _verify_swiftpm_aliases(aliases, target)
    return total


def load_state(path: Path, expected_sha256: str | None = None) -> JsonObject:
    if expected_sha256 is not None:
        if not re.fullmatch(r"[a-f0-9]{64}", expected_sha256):
            raise RuntimeError("expected capacity state SHA256 is invalid")
        if path.is_symlink():
            raise RuntimeError("refusing symlink expected capacity state")
        try:
            raw = path.read_bytes()
        except OSError as error:
            raise RuntimeError("expected capacity state is unavailable") from error
        if hashlib.sha256(raw).hexdigest() != expected_sha256:
            raise RuntimeError("capacity state SHA256 differs")
        try:
            return _object(json.loads(raw), "capacity state")
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise RuntimeError("expected capacity state is not valid JSON") from error
    try:
        return _object(json.loads(path.read_text()), "capacity state")
    except (FileNotFoundError, json.JSONDecodeError):
        return {}


def save_state(path: Path, state: JsonObject) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
    os.replace(temporary, path)


def state_identity(path: Path) -> tuple[int, int, str] | None:
    """Admission中のstate inodeと内容を固定し、変更検出に使う。"""
    if path.is_symlink():
        raise RuntimeError(f"refusing symlink state: {path}")
    try:
        before = path.stat()
        raw = path.read_bytes()
        after = path.stat()
    except FileNotFoundError:
        return None
    identity = (before.st_dev, before.st_ino, hashlib.sha256(raw).hexdigest())
    if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) != (
        before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns
    ):
        raise RuntimeError("capacity state changed while reading")
    return identity


def validate_recorded_baseline(state: JsonObject, target: Path) -> int | float | None:
    recorded_target = state.get("target")
    if "baseline_bytes" in state and not isinstance(recorded_target, str):
        raise RuntimeError("capacity baseline requires its recorded target")
    if "baseline_bytes" in state and (
        state.get("metric") != CAPACITY_METRIC or state.get("metric_version") != CAPACITY_METRIC_VERSION
    ):
        raise RuntimeError("capacity baseline metric is missing or obsolete")
    if recorded_target and Path(_text(recorded_target, "capacity target")).resolve() != target:
        raise RuntimeError("capacity state belongs to a different target")
    baseline = state.get("baseline_bytes")
    if baseline is not None and (isinstance(baseline, bool) or not isinstance(baseline, (int, float))
                                 or not math.isfinite(baseline) or baseline <= 0):
        raise RuntimeError("capacity baseline must be a finite positive number")
    return baseline if isinstance(baseline, (int, float)) and not isinstance(baseline, bool) else None


def inspect(root: Path, target: Path, state_path: Path, factor: float,
            expected_state_sha256: str | None = None) -> JsonObject:
    state = load_state(state_path, expected_state_sha256)
    try:
        capacity_state.validate_state(state, root, target)
    except ValueError as error:
        raise RuntimeError("invalid capacity state") from error
    baseline = validate_recorded_baseline(state, target)
    current = size_bytes(target)
    if current is None:
        return {"target": str(target), "metric": CAPACITY_METRIC, "metric_version": CAPACITY_METRIC_VERSION, "metric_available": False, "current_bytes": None, "baseline_bytes": baseline, "factor": factor, "threshold_bytes": None, "over_capacity": False, "state": str(state_path)}
    threshold = baseline * factor if isinstance(baseline, (int, float)) else None
    return {"target": str(target), "metric": CAPACITY_METRIC, "metric_version": CAPACITY_METRIC_VERSION, "metric_available": True, "current_bytes": current, "baseline_bytes": baseline, "factor": factor, "threshold_bytes": threshold, "over_capacity": threshold is not None and current > threshold, "state": str(state_path)}


def record_cleanup_result(report: object, target: Path, code: int, output: str, dry_run: bool) -> None:
    """Record measured cleanup attempts instead of inferring them from sweep output."""
    report = _object(report, "capacity report")
    current = _integer(report["current_bytes"], "current bytes")
    before_value = report.get("post_clean_bytes", current)
    before_clean = None if before_value is None else _integer(before_value, "pre-clean bytes")
    threshold_value = report["threshold_bytes"]
    threshold = None if threshold_value is None else _number(threshold_value, "threshold bytes")
    maximum_value = report.get("active_sweep_maxsize_bytes", threshold)
    maximum = None if maximum_value is None else _number(maximum_value, "sweep maximum bytes")
    attempts = _array(report.get("cleanup_attempts", []), "cleanup attempts")
    for item in attempts:
        previous = _object(item, "cleanup attempt")
        for field in ("attempt", "exit_code", "actual_removed_bytes"):
            _integer(previous[field], f"cleanup attempt {field}")
        for field in ("pre_clean_bytes", "post_clean_bytes"):
            if previous[field] is not None:
                _integer(previous[field], f"cleanup attempt {field}")
        if previous["maxsize_bytes"] is not None:
            _number(previous["maxsize_bytes"], "cleanup attempt maximum bytes")
        _text(previous["output"], "cleanup attempt output")
        if not isinstance(previous["dry_run"], bool):
            raise RuntimeError("cleanup attempt dry_run must be a boolean")
    report["clean_exitcode"] = code
    post_clean = size_bytes(target)
    attempt_removed = max(before_clean - post_clean, 0) if before_clean is not None and post_clean is not None else 0
    report["cleanup_attempts"] = attempts
    attempts.append({
        "attempt": len(attempts) + 1,
        "maxsize_bytes": maximum,
        "exit_code": code,
        "output": output,
        "pre_clean_bytes": before_clean,
        "post_clean_bytes": post_clean,
        "actual_removed_bytes": attempt_removed,
        "dry_run": dry_run,
    })
    report["clean_output"] = "\n".join(
        f"attempt {attempt['attempt']} ({attempt['maxsize_bytes']}B): {attempt['output']}"
        for item in attempts for attempt in [_object(item, "cleanup attempt")]
    )
    report["post_clean_bytes"] = post_clean
    report["actual_removed_bytes"] = max(current - post_clean, 0) if post_clean is not None else 0
    report["cleanup_goal_bytes"] = maximum
    report["cleanup_goal_reached"] = (
        post_clean is not None and maximum is not None and post_clean <= maximum
    )
    report["remaining_over_capacity"] = threshold is not None and (
        post_clean is None or post_clean > threshold
    )
    if code != 0:
        report["cleanup_status"] = "failed"
    elif dry_run:
        report["cleanup_status"] = "dry-run"
    elif post_clean is None:
        report["cleanup_status"] = "metric-unavailable"
    elif maximum is not None and not report["cleanup_goal_reached"]:
        report["cleanup_status"] = "incomplete"
    elif report["remaining_over_capacity"]:
        report["cleanup_status"] = "incomplete"
    else:
        report["cleanup_status"] = "completed"


def _safe_incomplete_cleanup_below_hardcap(report: object) -> bool:
    """固定hardcap以下の実整理だけを、goal未達のまま安全に許可する。"""
    value = _object(report, "capacity report")
    threshold = value.get("threshold_bytes")
    post_clean = value.get("post_clean_bytes")
    if (value.get("cleanup_status") != "incomplete" or value.get("cleanup_goal_reached") is not False
            or not isinstance(value.get("clean_exitcode"), int)
            or isinstance(value.get("clean_exitcode"), bool) or value.get("clean_exitcode") != 0
            or value.get("metric_available") is not True
            or value.get("remaining_over_capacity") is not False
            or isinstance(threshold, bool) or not isinstance(threshold, (int, float))
            or threshold <= 0
            or not math.isfinite(threshold) or isinstance(post_clean, bool)
            or not isinstance(post_clean, int) or post_clean < 0 or post_clean > threshold):
        return False
    attempts = value.get("cleanup_attempts")
    finite = value.get("finite_cleanup")
    if not isinstance(attempts, list) or not attempts or not isinstance(finite, dict) or "error" in finite:
        return False
    if not all(isinstance(item, dict)
               and isinstance(item.get("exit_code"), int)
               and not isinstance(item.get("exit_code"), bool)
               and item.get("exit_code") == 0
               and item.get("dry_run") is False
               for item in attempts):
        return False
    last_attempt = _object(attempts[-1], "cleanup attempt")
    last_post = last_attempt.get("post_clean_bytes")
    return isinstance(last_post, int) and not isinstance(last_post, bool) and last_post == post_clean


def collect_protected_references(cli_refs: object, environment_value: str | None,
                                 *, allow_empty_environment: bool = False) -> list[str]:
    """Validate and combine every declared Cargo consumer reference."""
    if cli_refs is None:
        cli_refs = []
    if not isinstance(cli_refs, list):
        raise RuntimeError("--protected-ref values must be a list of absolute paths")

    sources = [("--protected-ref", _array(cli_refs, "--protected-ref values"))]
    if environment_value is not None:
        try:
            environment_refs: object = json.loads(environment_value)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                "CARGO_CAPACITY_PROTECTED_REFS must be a JSON list of absolute paths"
            ) from error
        if not isinstance(environment_refs, list) or (not environment_refs and not allow_empty_environment):
            raise RuntimeError("CARGO_CAPACITY_PROTECTED_REFS must be a JSON list of absolute paths")
        sources.append(("CARGO_CAPACITY_PROTECTED_REFS", _array(environment_refs, "protected references")))

    validated = []
    for source_name, references in sources:
        for reference in references:
            if not isinstance(reference, (str, os.PathLike)):
                raise RuntimeError(f"{source_name} values must be absolute paths")
            value = os.fspath(reference)
            if not isinstance(value, str) or not Path(value).is_absolute():
                raise RuntimeError(f"{source_name} values must be absolute paths")
            validated.append(value)
    return list(dict.fromkeys(validated))


def coverage_selection(root: Path, target: Path, domain: str, command: list[str],
                       environment: Mapping[str, str]) -> tuple[str, Path]:
    if domain not in COVERAGE_DOMAINS or target != root / "target":
        raise RuntimeError("coverage domain requires the canonical aggregate target")
    selected = target / ("llvm-cov-target" if domain == "root" else f"llvm-cov-{domain}")
    if (environment.get("CARGO_TARGET_DIR") != str(target)
            or environment.get("CARGO_LLVM_COV_TARGET_DIR") != str(selected)):
        raise RuntimeError("coverage target environment differs from its fixed domain")
    if any(name in environment for name in ("CARGO_LLVM_COV_BUILD_DIR", "CARGO_BUILD_BUILD_DIR",
                                            "LLVM_PROFILE_FILE_NAME")):
        raise RuntimeError("coverage directory/profile overrides are unsupported")
    words = list(command)
    if words and Path(words[0]).name == "rtk":
        words.pop(0)
        if words and words[0] == "proxy":
            words.pop(0)
    if len(words) < 2 or Path(words[0]).name != "cargo" or words[1] != "llvm-cov":
        raise RuntimeError("coverage domain requires the fixed Cargo llvm-cov caller")
    words = words[2:]
    action = "clean" if words[:1] == ["clean"] else "test"
    if action == "clean":
        words = words[1:]
    if "--" in words:
        boundary = words.index("--")
        threads = words[boundary + 1:]
        value = (threads[0].split("=", 1)[1] if len(threads) == 1
                 and threads[0].startswith("--test-threads=") else
                 threads[1] if len(threads) == 2 and threads[0] == "--test-threads" else "")
        if action != "test" or not value.isdecimal() or int(value) < 1:
            raise RuntimeError("unsupported coverage test arguments")
        words = words[:boundary]
    switches = {"--workspace", "--locked"} if action == "clean" else {
        "--workspace", "--locked", "--all-targets", "--all-features", "--no-default-features",
        "--summary-only", "--json",
    }
    values = {"--manifest-path"} if action == "clean" else {
        "--manifest-path", "--features", "--fail-under-lines", "--fail-uncovered-lines",
        "--output-path", "--jobs", "-j",
    }
    options: dict[str, str] = {}
    index = 0
    while index < len(words):
        word = words[index]
        key = "--jobs" if word == "-j" else word
        if key in options or word not in switches | values:
            raise RuntimeError("unknown or repeated coverage option")
        value = ""
        if word in values:
            index += 1
            if index >= len(words):
                raise RuntimeError("missing coverage option value")
            value = words[index]
        options[key] = value
        index += 1
    manifest = root / ("Cargo.toml" if domain == "root" else "scripts/html/performance-harness/Cargo.toml")
    manifest_value = options.get("--manifest-path")
    if manifest_value is not None and manifest_value not in {str(manifest), str(manifest.relative_to(root))}:
        raise RuntimeError("coverage manifest differs from its fixed domain")
    required = {"--workspace"}
    if domain != "root":
        required |= {"--manifest-path", "--locked"}
    if action == "test":
        required |= {"--locked", "--all-targets", "--fail-under-lines", "--fail-uncovered-lines"}
        if options.get("--fail-under-lines") != "100" or options.get("--fail-uncovered-lines") != "0":
            raise RuntimeError("coverage thresholds must remain 100/0")
        if domain == "root":
            required.add("--all-features")
            if "--features" in options or "--no-default-features" in options:
                raise RuntimeError("root coverage feature selection differs")
        else:
            required |= {"--no-default-features", "--features"}
            if "--all-features" in options or options.get("--features") != domain.removeprefix("performance-"):
                raise RuntimeError("coverage arm feature selection differs")
        jobs = options.get("--jobs", "1")
        if not jobs.isdecimal() or int(jobs) < 1:
            raise RuntimeError("coverage jobs must be positive")
        if "--output-path" in options:
            output = root / "tmp/quality/standalone-performance-coverage" / f"{domain.removeprefix('performance-')}.json"
            if options["--output-path"] not in {str(output), str(output.relative_to(root))}:
                raise RuntimeError("coverage report differs from its fixed domain")
    if not required.issubset(options):
        raise RuntimeError("coverage command omits required domain options")
    _coverage_stamp(manifest, regular=True)
    for config_path in cargo_configuration_paths(root, environment):
        _coverage_stamp(config_path, regular=True)
    validate_build_selection(root, environment)
    for config_path in cargo_configuration_paths(root, environment):
        config = _cargo_configuration(config_path)
        build = _object(config.get("build", {}), "Cargo build configuration")
        configured_env = _object(config.get("env", {}), "Cargo environment configuration")
        if "build-dir" in build or any(name in configured_env for name in (
                "CARGO_TARGET_DIR", "CARGO_LLVM_COV_TARGET_DIR", "CARGO_LLVM_COV_BUILD_DIR",
                "CARGO_BUILD_BUILD_DIR", "LLVM_PROFILE_FILE_NAME")):
            raise RuntimeError("configured coverage directory/profile overrides are unsupported")
    return action, selected


def _unwrap_cargo(command: list[str]) -> list[str]:
    words = list(command)
    if words and Path(words[0]).name == "rtk":
        words.pop(0)
        if words and words[0] == "proxy":
            words.pop(0)
    return words


def _check_domain_command(root: Path, domain: str, command: list[str]) -> str:
    words = _unwrap_cargo(command)
    if not words or Path(words[0]).name != "cargo" or len(words) < 2 or words[1] != "clippy":
        raise RuntimeError("check domain only admits the fixed Cargo clippy commands")
    cargo_token = words[0]
    words = words[2:]
    expected_tail = ["--", *CLIPPY_DENY_FLAGS]
    if len(words) < len(expected_tail) or words[-len(expected_tail):] != expected_tail:
        raise RuntimeError("check domain Clippy deny flags differ from the fixed strict recipe")
    options = words[:-len(expected_tail)]
    if len(options) < 2 or options[:1] != ["-j"] or not options[1].isdecimal() or int(options[1]) < 1:
        raise RuntimeError("check domain requires a positive fixed Clippy job count")
    options = options[2:]
    if domain == "root":
        expected = ["--workspace", "--all-targets", "--all-features"]
        if options != expected:
            raise RuntimeError("root check domain requires the fixed workspace Clippy recipe")
    else:
        feature = domain.removeprefix("performance-")
        expected = ["--manifest-path", "scripts/html/performance-harness/Cargo.toml",
                    "--all-targets", "--no-default-features", "--features", feature, "--locked"]
        if options != expected:
            raise RuntimeError("performance check domain requires its fixed standalone feature recipe")
    return cargo_token


def _command_evidence(root: Path, cargo: str, domain: str,
                      environment: Mapping[str, str]) -> tuple[JsonObject, str]:
    manifest = root / ("Cargo.toml" if domain == "root" else "scripts/html/performance-harness/Cargo.toml")
    command = [cargo, "metadata", "--no-deps", "--locked", "--format-version", "1",
               "--manifest-path", str(manifest)]
    if domain == "root":
        command.append("--all-features")
    else:
        command += ["--no-default-features", "--features", domain.removeprefix("performance-")]
    try:
        result = subprocess.run(command, cwd=root, env=dict(environment), text=True,
                                capture_output=True, timeout=120, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError("check domain Cargo metadata or toolchain is unavailable") from error
    if result.returncode != 0:
        raise RuntimeError("check domain Cargo metadata query failed")
    try:
        metadata = _object(json.loads(result.stdout), "check domain Cargo metadata")
    except json.JSONDecodeError as error:
        raise RuntimeError("check domain Cargo metadata is invalid JSON") from error
    return metadata, result.stdout


def _check_domain_tool_pins(root: Path, cargo: str, environment: Mapping[str, str]) -> tuple[
        tuple[Path, SourcePin, str], tuple[Path, SourcePin, str],
        tuple[SourcePin, str], tuple[SourcePin, str], SourcePin]:
    path = os.pathsep.join(str(Path(part) if Path(part).is_absolute() else root / part)
                           for part in os.get_exec_path(environment))
    cargo_path = shutil.which(cargo, path=path)
    rustup = shutil.which("rustup", path=path)
    if cargo_path is None or rustup is None:
        raise RuntimeError("check domain physical Cargo/rustup tools cannot be proven")
    cargo_tool = _tool_pin(cargo_path, ("-Vv",), "cargo", root)
    rustup_pin = source_pin(Path(rustup).resolve(strict=True))
    try:
        selection = subprocess.run([rustup, "which", "rustc"], cwd=root, env=dict(environment),
                                   text=True, capture_output=True, timeout=15, check=False)
        clippy_selection = subprocess.run([rustup, "which", "clippy-driver"], cwd=root,
                                          env=dict(environment), text=True, capture_output=True,
                                          timeout=15, check=False)
        cargo_clippy_selection = subprocess.run([rustup, "which", "cargo-clippy"], cwd=root,
                                                env=dict(environment), text=True, capture_output=True,
                                                timeout=15, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError("check domain effective Rust toolchain is unavailable") from error
    if selection.returncode or clippy_selection.returncode or cargo_clippy_selection.returncode:
        raise RuntimeError("check domain effective Rust toolchain is unavailable")
    rustc_path = Path(selection.stdout.strip())
    clippy_path = Path(clippy_selection.stdout.strip())
    cargo_clippy_path = Path(cargo_clippy_selection.stdout.strip())
    if (not rustc_path.is_absolute() or rustc_path.resolve() != rustc_path
            or not clippy_path.is_absolute() or clippy_path.resolve() != clippy_path
            or not cargo_clippy_path.is_absolute() or cargo_clippy_path.resolve() != cargo_clippy_path):
        raise RuntimeError("check domain effective Rust tool paths are not canonical")
    rustc_tool = _tool_pin(str(rustc_path), ("-vV",), "rustc", root)
    try:
        clippy_version = subprocess.run([str(clippy_path), "--version"], cwd=root,
                                        env=dict(environment), text=True, capture_output=True,
                                        timeout=15, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError("check domain effective Clippy driver is unavailable") from error
    if clippy_version.returncode or not clippy_version.stdout.strip().startswith("clippy "):
        raise RuntimeError("check domain effective Clippy driver identity is unsupported")
    driver_pin = source_pin(clippy_path)
    try:
        cargo_clippy_version = subprocess.run([str(cargo_clippy_path), "--version"], cwd=root,
                                              env=dict(environment), text=True, capture_output=True,
                                              timeout=15, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError("check domain Cargo Clippy subcommand is unavailable") from error
    if (cargo_clippy_version.returncode or cargo_clippy_version.stdout.strip()
            != clippy_version.stdout.strip()):
        raise RuntimeError("check domain Cargo Clippy subcommand and driver do not match")
    cargo_clippy_pin = source_pin(cargo_clippy_path)
    return (cargo_tool, rustc_tool, (cargo_clippy_pin, cargo_clippy_version.stdout.strip()),
            (driver_pin, clippy_version.stdout.strip()), rustup_pin)


def _verify_check_domain_tool_pins(
    pins: tuple[tuple[Path, SourcePin, str], tuple[Path, SourcePin, str],
                tuple[SourcePin, str], tuple[SourcePin, str], SourcePin],
) -> None:
    cargo, rustc, cargo_clippy, clippy, rustup = pins
    if (cargo[0].resolve(strict=True) != cargo[1].path
            or source_pin(cargo[1].path) != cargo[1]
            or rustc[0].resolve(strict=True) != rustc[1].path
            or source_pin(rustc[1].path) != rustc[1]
            or source_pin(cargo_clippy[0].path) != cargo_clippy[0]
            or source_pin(clippy[0].path) != clippy[0]
            or source_pin(rustup.path) != rustup):
        raise RuntimeError("check domain physical toolchain changed during cleanup")


def check_domain_selection(root: Path, target: Path, domain: str, command: list[str],
                           environment: Mapping[str, str]) -> tuple[JsonObject, Callable[[], None]]:
    if domain not in CHECK_DOMAINS or target != root / "target":
        raise RuntimeError("check domain requires the canonical aggregate target")
    if (environment.get("CARGO_TARGET_DIR") != str(target)
            or any(name in environment for name in (
                "CARGO_BUILD_TARGET", "CARGO_BUILD_BUILD_DIR", "CARGO_LLVM_COV_TARGET_DIR",
                "CARGO_LLVM_COV_BUILD_DIR", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "RUSTC_BOOTSTRAP", "RUSTUP_TOOLCHAIN", "CARGO_ENCODED_RUSTFLAGS",
                "LLVM_PROFILE_FILE_NAME"))
            or any(name.startswith(("CARGO_TARGET_", "CARGO_PROFILE_"))
                   and name != "CARGO_TARGET_DIR" for name in environment)
            or environment.get("RUSTFLAGS") != "-D warnings"):
        raise RuntimeError("check domain target, compiler, or profile environment is unsupported")
    cargo_token = _check_domain_command(root, domain, command)
    validate_build_selection(root, environment)
    validate_build_executable(root, command, environment)
    if not selected_namespace_supported(root):
        raise RuntimeError("check domain compiler or Cargo configuration cannot be proven")
    selected_manifest = root / "scripts/html/performance-harness/Cargo.toml"
    workspace_manifests = [root / "Cargo.toml"]
    if domain != "root":
        workspace_manifests.append(selected_manifest)
    for workspace_manifest in workspace_manifests:
        workspace_config = _cargo_configuration(workspace_manifest).get("workspace", {})
        if not isinstance(workspace_config, dict):
            raise RuntimeError("check domain workspace selection is unsupported")
        workspace_members = workspace_config.get("members", [])
        if (not isinstance(workspace_members, list)
                or any(not isinstance(member, str) or any(token in member for token in ("*", "?", "["))
                       for member in workspace_members)):
            raise RuntimeError("check domain workspace globs are unsupported")
    for config_path in cargo_configuration_paths(root, environment):
        configuration = _cargo_configuration(config_path)
        configured_env = _object(configuration.get("env", {}), "Cargo environment configuration")
        if any(name in configured_env for name in (
                "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "CARGO_BUILD_BUILD_DIR", "RUSTFLAGS",
                "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")):
            raise RuntimeError("check domain configured environment override is unsupported")
    manifest = selected_manifest
    root_manifest = root / "Cargo.toml"
    pinned_files = [root_manifest, root / "Cargo.lock", manifest, manifest.with_name("Cargo.lock"),
                    root / "rust-toolchain", root / "rust-toolchain.toml",
                    manifest.parent / "rust-toolchain", manifest.parent / "rust-toolchain.toml"]
    rustup_home = Path(environment.get("RUSTUP_HOME", str(Path.home() / ".rustup")))
    if rustup_home.is_absolute():
        pinned_files.append(rustup_home / "settings.toml")

    def file_snapshot() -> dict[Path, tuple[int, int, int, int, int, int, str]]:
        values: dict[Path, tuple[int, int, int, int, int, int, str]] = {}
        try:
            for path in dict.fromkeys((*pinned_files, *cargo_configuration_paths(root, environment))):
                if not path.exists():
                    continue
                stamp = _coverage_stamp(path, regular=True)
                values[path] = (*stamp, hashlib.sha256(path.read_bytes()).hexdigest())
        except OSError as error:
            raise RuntimeError("check domain source, lockfile, or Cargo configuration is unavailable") from error
        return values

    before_files = file_snapshot()
    cargo = shutil.which(cargo_token, path=os.pathsep.join(
        str(Path(part) if Path(part).is_absolute() else root / part)
        for part in os.get_exec_path(environment)))
    if cargo is None:
        raise RuntimeError("check domain Cargo executable is unavailable")
    tools = _check_domain_tool_pins(root, cargo, environment)
    root_metadata, root_json = _command_evidence(root, cargo, "root", environment)
    selected_metadata, selected_json = _command_evidence(root, cargo, domain, environment)
    _verify_check_domain_tool_pins(tools)
    expected_workspace = root if domain == "root" else manifest.parent
    for metadata in (root_metadata, selected_metadata):
        workspace_root = Path(_text(metadata.get("workspace_root"), "Cargo workspace root"))
        if not workspace_root.is_absolute() or workspace_root.resolve() != workspace_root:
            raise RuntimeError("check domain Cargo workspace root is not canonical")
    if Path(_text(root_metadata.get("workspace_root"), "root workspace root")) != root:
        raise RuntimeError("root Cargo metadata belongs to another workspace")
    if Path(_text(selected_metadata.get("workspace_root"), "selected workspace root")) != expected_workspace:
        raise RuntimeError("selected Cargo metadata belongs to another workspace")
    selected_members = set(_text(value, "selected workspace member") for value in
                           _array(selected_metadata.get("workspace_members"), "selected workspace members"))
    selected_packages = [_object(value, "selected package") for value in
                         _array(selected_metadata.get("packages"), "selected packages")]
    selected_manifests = [Path(_text(package.get("manifest_path"), "selected package manifest"))
                          for package in selected_packages if package.get("id") in selected_members]
    if domain == "root" and any(
            not value.is_absolute() or value.resolve() != value or not value.is_relative_to(root)
            for value in selected_manifests):
        raise RuntimeError("root Cargo workspace contains a noncanonical or external manifest")
    if (domain != "root" and (len(selected_members) != 1 or len(selected_manifests) != 1
                               or selected_manifests[0] != manifest)):
        raise RuntimeError("selected Cargo metadata is not owned by the harness manifest")
    for metadata in (root_metadata, selected_metadata):
        if (metadata.get("target_directory") != str(target)
                or Path(_text(metadata.get("build_directory"), "Cargo build directory")).resolve()
                != Path(_text(root_metadata.get("build_directory"), "root Cargo build directory")).resolve()):
            raise RuntimeError("check domain metadata target or build directory differs from root Cargo")
    if Path(_text(root_metadata.get("build_directory"), "Cargo build directory")).resolve() != target:
        raise RuntimeError("check domain requires the canonical root Cargo build directory")
    if file_snapshot() != before_files:
        raise RuntimeError("check domain source, lockfile, or Cargo configuration changed during metadata")

    input_check = _selected_input_check(root, root_metadata, include_target_inputs=True)
    same_input_scope = domain == "root" and root_json == selected_json
    selected_input_check = (
        input_check if same_input_scope
        else _selected_input_check(root, selected_metadata, include_target_inputs=True)
    )

    def verify() -> None:
        if dict(environment) != dict(os.environ):
            raise RuntimeError("check domain environment changed during cleanup")
        if file_snapshot() != before_files:
            raise RuntimeError("check domain source, lockfile, or Cargo configuration changed during cleanup")
        input_check()
        if selected_input_check is not input_check:
            selected_input_check()
        _verify_check_domain_tool_pins(tools)
        if not root_json or not selected_json:
            raise RuntimeError("check domain Cargo metadata evidence is incomplete")

    return root_metadata, verify


def _coverage_stamp(path: Path, regular: bool = False) -> tuple[int, int, int, int, int, int]:
    cursor = Path(path.anchor)
    for part in path.parts[1:]:
        cursor /= part
        if stat.S_ISLNK(cursor.lstat().st_mode):
            raise RuntimeError("coverage protection path contains a symlink")
    value = path.lstat()
    if not stat.S_ISREG(value.st_mode) and (regular or not stat.S_ISDIR(value.st_mode)):
        raise RuntimeError("coverage protection path has an unknown kind")
    return value.st_dev, value.st_ino, value.st_mode, value.st_size, value.st_mtime_ns, value.st_ctime_ns


def _coverage_visit_stamp(path: Path, target: Path,
                          aliases: dict[Path, tuple[int, ...]]) -> tuple[int, int, int, int, int, int]:
    value = path.lstat()
    if not stat.S_ISLNK(value.st_mode):
        return _coverage_stamp(path)
    alias = _swiftpm_release_alias_stat(path, target)
    if alias is None:
        raise RuntimeError("coverage protection path contains a symlink")
    aliases[path] = _stat_identity(alias)
    return alias.st_dev, alias.st_ino, alias.st_mode, alias.st_size, alias.st_mtime_ns, alias.st_ctime_ns


def coverage_cleanup_preflight(root: Path, target: Path, selected: Path,
                               protected_refs: list[str]) -> None:
    references = {Path(name): _coverage_stamp(Path(name), regular=True) for name in protected_refs}
    protected = {value[:2] for value in references.values()}
    # 公式0.9.1のworkspace cleanとrunの暗黙cleanはmetadata targetの共有出力も消す。
    footprint = (selected, target / "llvm-cov/html", target / "llvm-cov/text", target / "ui",
                 target / "tests/trybuild", target / "tests/target")
    aliases: dict[Path, tuple[int, ...]] = {}
    for reference in references:
        if any(reference == path or path in reference.parents for path in footprint):
            raise RuntimeError("coverage cleanup intersects a protected consumer path")
    def visit(path: Path) -> None:
        try:
            value = _coverage_visit_stamp(path, target, aliases)
        except FileNotFoundError:
            return
        if value[0] != target.lstat().st_dev:
            raise RuntimeError("coverage cleanup crosses a device boundary")
        if value[:2] in protected:
            raise RuntimeError("coverage cleanup intersects a protected consumer inode")
        if stat.S_ISDIR(value[2]):
            for child in path.iterdir():
                visit(child)
    if busy_processes(target, root):
        raise RuntimeError("coverage cleanup target is in use or inspection is unavailable")
    try:
        with ProfileLocks(target) as epoch:
            if busy_processes(target, root):
                raise RuntimeError("coverage cleanup target became busy")
            for path in footprint:
                visit(path)
            _verify_swiftpm_aliases(aliases, target)
            for reference, value in references.items():
                if _coverage_stamp(reference, regular=True) != value:
                    raise RuntimeError("coverage protected consumer changed during inspection")
            epoch.verify()
    except BlockingIOError as error:
        raise RuntimeError("coverage cleanup Cargo lock is busy") from error
    # Cargoが同lockを取得するのでProfileLocksは子の起動前に解放し、外側GuardLockは維持する。
    _verify_swiftpm_aliases(aliases, target)
    for reference, value in references.items():
        if _coverage_stamp(reference, regular=True) != value:
            raise RuntimeError("coverage protected consumer changed before execution")


def coverage_tool_version(root: Path, command: list[str]) -> JsonObject:
    query = command[:command.index("llvm-cov") + 1] + ["--version"]
    evidence: JsonObject = {"argv": query, "stdout_hex": "", "stderr_hex": "", "exit": None}
    try:
        result = subprocess.run(query, cwd=root, env=os.environ.copy(), capture_output=True,
                                timeout=30, check=False)
        evidence.update(stdout_hex=result.stdout.hex(), stderr_hex=result.stderr.hex(), exit=result.returncode)
        if result.returncode != 0 or result.stdout.strip() != b"cargo-llvm-cov 0.9.1":
            raise RuntimeError("coverage cleanup requires admitted cargo-llvm-cov 0.9.1")
    except subprocess.TimeoutExpired as error:
        evidence["failure"] = "timeout"
        for name, value in (("stdout_hex", error.stdout), ("stderr_hex", error.stderr)):
            if isinstance(value, bytes):
                evidence[name] = value.hex()
        raise RuntimeError("coverage tool version query timed out") from error
    except OSError as error:
        evidence["failure"] = str(error)
        raise RuntimeError("coverage tool version query is unavailable") from error
    finally:
        # queryの失敗もcallerのraw logへ残し、version不明を成功にしない。
        print(json.dumps({"coverage_tool_query": evidence}, sort_keys=True), file=sys.stderr)
    return evidence


def library_alias_cargo_command(command: list[str]) -> str:
    words = list(command)
    if words and Path(words[0]).name == "rtk":
        words.pop(0)
        if words and words[0] == "proxy":
            words.pop(0)
    if (not words or Path(words[0]).name != "cargo"
            or words[1:] != ["build", "-p", "katana-render-runtime-cli"]):
        raise RuntimeError("library alias cache caller differs from the fixed CLI build")
    return words[0]


def run(args: argparse.Namespace) -> int:
    require_capacity = bool(getattr(args, "require_capacity", False))
    if require_capacity and args.command:
        raise RuntimeError("--require-capacity is only valid for a no-child admission")
    metadata_only_alias_cache = bool(getattr(args, "library_alias_cache_metadata_only", False))
    archives_only_alias_cache = bool(getattr(args, "library_alias_cache_archives_only", False))
    if metadata_only_alias_cache and not getattr(args, "library_alias_cache", False):
        raise RuntimeError("library alias metadata-only mode requires --library-alias-cache")
    if archives_only_alias_cache and not getattr(args, "library_alias_cache", False):
        raise RuntimeError("library alias archives-only mode requires --library-alias-cache")
    if metadata_only_alias_cache and archives_only_alias_cache:
        raise RuntimeError("library alias metadata-only and archives-only modes are mutually exclusive")
    if getattr(args, "library_alias_cache", False) and os.environ.get("RUSTFLAGS") != "-D warnings":
        raise RuntimeError("library alias cache requires the standard RUSTFLAGS=-D warnings")
    native_enabled = ("KRR_NATIVE_CLI_GUARD_REQUEST" in os.environ
                      or "KRR_NATIVE_CLI_GUARD_REQUEST_SHA256" in os.environ)
    if native_enabled:
        from cargo_capacity_guard_native_cli_observation import observe, validate_capacity_report, validate_request
        native_root = _root()
        native_admission = validate_request(native_root, args.command, os.environ)
        if (args.dry_run or getattr(args, "coverage_domain", None) is not None
                or getattr(args, "retain_build_profile", None) != "debug"
                or getattr(args, "retain_profile", [])
                or args.state not in (None, str(native_admission.state))
                or args.factor != DEFAULT_FACTOR
                or args.target not in (None, str(native_root / "target"))
                or args.target_dir not in (None, str(native_root / "target"))):
            raise RuntimeError("native observation requires the canonical retained CLI build")
        native_refs = collect_protected_references(
            getattr(args, "protected_ref", []), os.environ.get("CARGO_CAPACITY_PROTECTED_REFS"),
        )
        if native_refs != native_admission.protected:
            raise RuntimeError("guard protected references differ from native admission")
    coverage_value: object = getattr(args, "coverage_domain", None)
    check_value: object = getattr(args, "check_domain", None)
    try:
        configured_factor = float(os.environ.get("CARGO_CAPACITY_FACTOR", str(DEFAULT_FACTOR)))
    except ValueError as error:
        raise RuntimeError("capacity factor must be numeric") from error
    if check_value is not None and (native_enabled or coverage_value is not None
                                    or getattr(args, "library_alias_cache", False)
                                    or getattr(args, "retain_profile", [])
                                    or getattr(args, "retain_build_profile", None) is not None):
        raise RuntimeError("check domain cannot be combined with another ownership mode")
    if check_value is not None and (
            args.factor != DEFAULT_FACTOR
            or configured_factor != DEFAULT_FACTOR
            or args.state not in (None, str(_root() / "tmp/.cargo-capacity-guard.json"))
            or getattr(args, "expected_state_sha256", None) is not None):
        raise RuntimeError("check domain requires the existing canonical capacity state and factor")
    expected_value: object = getattr(args, "expected_state_sha256", None)
    if coverage_value is not None:
        expected_environment = os.environ.get("CARGO_CAPACITY_EXPECTED_STATE_SHA256")
        if expected_environment is not None:
            if not re.fullmatch(r"[a-f0-9]{64}", expected_environment):
                raise RuntimeError("coverage expected state environment SHA256 is invalid")
            if expected_value is not None and expected_value != expected_environment:
                raise RuntimeError("coverage expected state CLI/environment SHA256 conflict")
            expected_value = expected_environment
    expected_state_sha256 = None if expected_value is None else _text(expected_value, "expected capacity state SHA256")
    protected_refs = collect_protected_references(
        getattr(args, "protected_ref", []),
        os.environ.get("CARGO_CAPACITY_PROTECTED_REFS"),
        allow_empty_environment=coverage_value is not None,
    )
    root = _root()
    if require_capacity:
        canonical_target = root / "target"
        canonical_state = root / "tmp/.cargo-capacity-guard.json"
        configured_factor = float(os.environ.get("CARGO_CAPACITY_FACTOR", str(DEFAULT_FACTOR)))
        if (not isinstance(args.target, str) or not isinstance(args.state, str)
                or Path(args.target).resolve() != canonical_target or args.target_dir is not None
                or Path(args.state).resolve() != canonical_state or args.factor != DEFAULT_FACTOR
                or configured_factor != DEFAULT_FACTOR):
            raise RuntimeError("--require-capacity requires the canonical root, target, state, and factor")
    if args.target and args.target_dir:
        raise RuntimeError("--target and --target-dir cannot be combined")
    incremental_prefixes: list[str] = []
    retained_args = getattr(args, "retain_profile", [])
    build_profile = getattr(args, "retain_build_profile", None)
    verified_build_profiles: tuple[Path, ...] = ()
    check_input_check: Callable[[], None] | None = None
    if build_profile is not None:
        validate_build_profile(build_profile, args.command)
        validate_build_selection(root)
        validate_build_executable(root, args.command)
    check_enabled = check_value is not None
    inactive_prefixes: list[str] | None = [] if retained_args or build_profile is not None or check_enabled else None
    build_metadata: JsonObject | None = {} if build_profile is not None or check_enabled else None
    if native_enabled:
        target = resolve_target(root, args.target_dir or args.target, incremental_prefixes,
                                inactive_prefixes, build_metadata, metadata_timeout=120)
    else:
        target = resolve_target(root, args.target_dir or args.target, incremental_prefixes, inactive_prefixes, build_metadata)
    retained_profiles = retained_profile_paths(target, retained_args)
    if build_profile is not None:
        verified_build_profiles = current_build_profiles(target, build_metadata, build_profile)
        retained_profiles += verified_build_profiles
        retained_profiles = tuple(dict.fromkeys(retained_profiles))
    if check_enabled:
        canonical_target = root / "target"
        if (args.target not in (None, str(canonical_target))
                or args.target_dir not in (None, str(canonical_target))):
            raise RuntimeError("check domain requires the canonical aggregate target")
        check_metadata, check_input_check = check_domain_selection(
            root, target, _text(check_value, "check domain"), args.command, os.environ,
        )
        root_metadata = _object(build_metadata, "root Cargo metadata")
        if (check_metadata.get("target_directory") != root_metadata.get("target_directory")
                or check_metadata.get("build_directory") != root_metadata.get("build_directory")):
            raise RuntimeError("check domain metadata differs from resolved root Cargo target")
        if not protected_refs:
            verified_build_profiles = current_build_profiles(target, root_metadata, "debug")
            retained_profiles = tuple(dict.fromkeys((*retained_profiles, *verified_build_profiles)))
    auxiliary_only_profiles = (
        verified_build_profiles
        if verified_build_profiles and not protected_refs and coverage_value is None
        else ()
    )
    unselected_prefixes: list[str] = []
    selected_input_check: Callable[[], None] | None = check_input_check
    ownership: list[BuildScriptOwnership] = []
    selected_caches: list[SelectedCacheOwnership] = []
    if verified_build_profiles and build_profile == "debug" and coverage_value is None:
        unselected_prefixes, selected_input_check = selected_package_cleanup(
            root, target, args.command, _object(build_metadata, "retained build metadata"),
            ownership.append if protected_refs else None,
            selected_caches.append if protected_refs else None,
        )
    if len(ownership) > 1 or len(selected_caches) > 1:
        raise RuntimeError("selected host ownership capture is ambiguous")
    owned_scripts = ownership[0] if ownership else None
    selected_cache = selected_caches[0] if selected_caches else None
    selected_package_profiles = verified_build_profiles if unselected_prefixes or owned_scripts else ()
    library_alias: LibraryAliasOwnership | None = None
    if getattr(args, "library_alias_cache", False):
        cargo_token = library_alias_cargo_command(args.command)
        if (build_profile != "debug" or not protected_refs or native_enabled
                or coverage_value is not None or owned_scripts is not None or selected_cache is not None
                or args.factor != DEFAULT_FACTOR
                or float(os.environ.get("CARGO_CAPACITY_FACTOR", args.factor)) != DEFAULT_FACTOR
                or os.name == "nt"):
            raise RuntimeError("library alias cache requires the canonical retained CLI build and consumers")
        cargo = shutil.which(cargo_token)
        if cargo is None:
            raise RuntimeError("library alias canonical Cargo is unavailable")
        library_alias = create_library_alias_ownership(
            root, target, _object(build_metadata, "library alias metadata"), cargo,
            root / "tmp/quality/capacity-library-alias-queries" / str(time.time_ns()),
            metadata_only=metadata_only_alias_cache,
            archives_only=archives_only_alias_cache,
        )
        selected_input_check = library_alias.verify
        selected_cache = library_alias.cache
        selected_package_profiles = verified_build_profiles
    state_path = Path(args.state or root / "tmp/.cargo-capacity-guard.json")
    if state_path.is_symlink():
        raise RuntimeError(f"refusing symlink state: {state_path}")
    factor = float(os.environ.get("CARGO_CAPACITY_FACTOR", args.factor))
    if not math.isfinite(factor) or factor <= 1.0:
        raise RuntimeError("capacity factor must be finite and greater than 1")
    coverage = None if coverage_value is None else coverage_selection(
        root, target, _text(coverage_value, "coverage domain"), args.command, os.environ,
    )
    if coverage is not None and (state_path != root / "tmp/.cargo-capacity-guard.json" or factor != DEFAULT_FACTOR):
        raise RuntimeError("coverage requires the canonical state and existing capacity factor")
    lock_path = root / "tmp/.cargo-capacity-guard.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with GuardLock(lock_path):
        admission_state_before = state_identity(state_path) if require_capacity else None
        admission_expected_state_sha256 = (
            admission_state_before[2] if admission_state_before is not None else None
        )
        if (
            require_capacity and expected_state_sha256 is not None
            and expected_state_sha256 != admission_expected_state_sha256
        ):
            raise RuntimeError("expected capacity state SHA256 differs")
        report = inspect(
            root, target, state_path,
            factor,
            admission_expected_state_sha256 if require_capacity else expected_state_sha256,
        )
        if require_capacity and state_identity(state_path) != admission_state_before:
            raise RuntimeError("capacity state changed before eviction")
        if coverage is not None and (report.get("metric_available", True) is not True
                                     or report["baseline_bytes"] is None):
            raise RuntimeError("coverage capacity admission requires a measured canonical baseline")
        if coverage is not None:
            report["coverage_inputs"] = {"domain": coverage_value, "action": coverage[0],
                                         "target": str(coverage[1]), "protected_references": protected_refs}
        if native_enabled:
            if (target != native_root / "target" or state_path != native_admission.state
                    or factor != DEFAULT_FACTOR
                    or expected_state_sha256 not in (None, native_admission.state_sha256)):
                raise RuntimeError("guard capacity inputs differ from native admission")
            validate_capacity_report(report, target)
        baseline_eligible = (
            report.get("metric_available", True) is True and report.get("current_bytes") == 0
        )
        report["baseline_eligible"] = baseline_eligible
        removed: list[str] = []
        if report["over_capacity"]:
            cleanup_goal = int(_number(report["baseline_bytes"], "baseline bytes"))
            report["active_sweep_maxsize_bytes"] = cleanup_goal
            report["cleanup_goal_bytes"] = cleanup_goal
            report["cleanup_goal_reached"] = False
            busy = busy_processes(target, root)
            if busy:
                report["clean_skipped"] = "busy"
                report["busy_processes"] = busy
            else:
                try:
                    with ProfileLocks(target) as epoch:
                        epoch.input_check = selected_input_check
                        busy = busy_processes(target, root)
                        if busy:
                            report["clean_skipped"] = "busy"
                            report["busy_processes"] = busy
                        else:
                            if expected_state_sha256 is not None:
                                load_state(state_path, expected_state_sha256)
                            eviction = (partial(evict_artifacts, library_alias_ownership=library_alias)
                                        if library_alias is not None else evict_artifacts)
                            code, details = eviction(
                                target, cleanup_goal,
                                _integer(report["current_bytes"], "current bytes"), args.dry_run,
                                protected_refs, epoch, incremental_prefixes=incremental_prefixes,
                                inactive_prefixes=inactive_prefixes or (), retained_profiles=retained_profiles,
                                auxiliary_only_profiles=auxiliary_only_profiles,
                                unselected_incremental_prefixes=unselected_prefixes,
                                selected_package_profiles=selected_package_profiles,
                                build_script_ownership=owned_scripts,
                                selected_cache_ownership=selected_cache,
                            )
                            report["finite_cleanup"] = details
                            record_cleanup_result(
                                report, target, code, "finite artifact eviction", args.dry_run,
                            )
                            epoch.verify()
                except BlockingIOError as error:
                    report["clean_skipped"] = "cargo-lock-busy"
                    report["busy_lock"] = str(error)
                if (
                    not args.dry_run
                    and report.get("cleanup_status") == "completed"
                    and report.get("cleanup_attempts")
                ):
                    removed = ["finite artifact eviction"]
        if "cleanup_status" not in report:
            report["post_clean_bytes"] = None
            report["actual_removed_bytes"] = 0
            report["remaining_over_capacity"] = report["over_capacity"]
            if not report.get("metric_available", True):
                report["cleanup_status"] = "metric-unavailable"
            elif report["baseline_bytes"] is None:
                report["cleanup_status"] = "uncalibrated"
            else:
                report["cleanup_status"] = "not-needed" if not report["over_capacity"] else "skipped"
        if check_input_check is not None:
            check_input_check()
        report["removed"] = removed
        if args.print_json:
            _write_text(sys.stdout, json.dumps(report, indent=2, sort_keys=True) + "\n")
        else:
            _write_text(sys.stderr, json.dumps(report, sort_keys=True, separators=(",", ":")) + "\n")
        if args.dry_run or not args.command:
            if expected_state_sha256 is not None:
                load_state(state_path, expected_state_sha256)
            if require_capacity:
                admission_state_after = state_identity(state_path)
                if admission_state_before != admission_state_after:
                    raise RuntimeError("capacity state changed during admission")
                bootstrap = report.get("baseline_bytes") is None and report.get("current_bytes") == 0
                admitted = (
                    report.get("metric_available") is True
                    and (report.get("baseline_bytes") is not None or bootstrap)
                    and (
                        report.get("cleanup_status") in {"completed", "not-needed"}
                        or _safe_incomplete_cleanup_below_hardcap(report)
                        or (bootstrap and report.get("cleanup_status") == "uncalibrated")
                    )
                    and report.get("remaining_over_capacity") is False
                )
                if not admitted:
                    return 1
                return 0
            return 1 if report["cleanup_status"] in {"incomplete", "failed"} else 0
        allow_incomplete_child = (
            (check_enabled or coverage is not None)
            and not native_enabled
            and build_profile is None
            and library_alias is None
            and _safe_incomplete_cleanup_below_hardcap(report)
        )
        if report["over_capacity"] and report["cleanup_status"] != "completed" and not allow_incomplete_child:
            return 1
        if expected_state_sha256 is not None:
            load_state(state_path, expected_state_sha256)
        if check_input_check is not None:
            check_input_check()
        if coverage is not None:
            coverage_tool_version(root, args.command)
            coverage_cleanup_preflight(root, target, coverage[1], protected_refs)
            if expected_state_sha256 is not None:
                load_state(state_path, expected_state_sha256)
        if native_enabled:
            return observe(
                native_admission, args.command, root, report,
                lambda: inspect(root, target, state_path, factor, native_admission.state_sha256),
            )
        else:
            completed = subprocess.run(args.command, cwd=root, env=os.environ.copy(), check=False)
        if expected_state_sha256 is not None:
            load_state(state_path, expected_state_sha256)
        is_build = is_successful_build(args.command)
        used = size_bytes(target)
        if expected_state_sha256 is None and completed.returncode == 0 and is_build and baseline_eligible and used is not None and used > 0 and "baseline_bytes" not in load_state(state_path):
            state = capacity_state.create_runner_measurement(root, target, used, time.time(), factor)
            save_state(state_path, state)
        return completed.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target")
    parser.add_argument("--target-dir")
    parser.add_argument("--root")
    parser.add_argument("--state")
    parser.add_argument("--expected-state-sha256")
    parser.add_argument("--coverage-domain", choices=COVERAGE_DOMAINS)
    parser.add_argument("--check-domain", choices=CHECK_DOMAINS, action="append")
    parser.add_argument("--factor", type=float, default=DEFAULT_FACTOR)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--progress", action="store_true",
                        help="write Cargo capacity guard progress to stderr")
    parser.add_argument("--require-capacity", action="store_true",
                        help="require safe capacity admission for a no-child invocation")
    parser.add_argument("--library-alias-cache", action="store_true")
    alias_modes = parser.add_mutually_exclusive_group()
    alias_modes.add_argument("--library-alias-cache-metadata-only", action="store_true",
                             help="use Cargo-owned metadata and selected-cache candidates without Mach-O object eviction")
    alias_modes.add_argument("--library-alias-cache-archives-only", action="store_true",
                             help="admit only LLVM-verified Cargo rlib archives")
    parser.add_argument("--print-json", action="store_true")
    parser.add_argument("--protected-ref", action="append", default=[])
    parser.add_argument("--retain-profile", action="append", default=[],
                        help="retain active Cargo output/build profile; authorize inactive workspace incremental closure")
    parser.add_argument("--retain-build-profile", choices=("debug", "release"),
                        help="retain existing metadata output/build profiles for a same-environment Cargo build")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if len(args.check_domain or []) > 1:
        print("check domain must be declared once", file=sys.stderr)
        return 2
    args.check_domain = args.check_domain[0] if args.check_domain else None
    if args.root:
        os.environ["CARGO_CAPACITY_GUARD_ROOT"] = args.root
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    try:
        with _progress_logging(args.progress):
            return run(args)
    except RuntimeError as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
