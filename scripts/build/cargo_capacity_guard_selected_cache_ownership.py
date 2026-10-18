"""Metadata-bound ownership for selected-package incremental caches."""
from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
from typing import Literal

from cargo_capacity_guard_build_script_ownership import SourcePin, source_pin

@dataclass(frozen=True)
class SelectedCacheOwner:
    prefix: str
    kind: Literal["library", "binary"]
    package_id: str
    manifest: SourcePin
    source: SourcePin


@dataclass(frozen=True)
class SelectedCacheOwnership:
    root: Path
    target: Path
    profile: Path
    owners: tuple[SelectedCacheOwner, ...]
    verify: Callable[[], None]


_LIBRARY_KINDS = {"lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"}
_BINARY_KINDS = {"bin", "test", "example", "bench"}
_KNOWN_KINDS = _LIBRARY_KINDS | _BINARY_KINDS | {"custom-build"}
_EXCLUDED_PREFIXES = {"build_script_build-"}


def _tool_pin(
    executable: str, args: tuple[str, ...], role: Literal["cargo", "rustc"] | None = None,
    working_directory: Path | None = None,
) -> tuple[Path, SourcePin, str]:
    invocation = Path(executable)
    if not invocation.is_absolute():
        resolved = shutil.which(executable)
        if resolved is None:
            raise RuntimeError(f"selected cache tool is unavailable: {executable}")
        invocation = Path(resolved)
    if not invocation.is_absolute():
        raise RuntimeError(f"selected cache tool invocation is not absolute: {executable}")
    path = invocation.resolve(strict=True)
    pin = source_pin(path)
    expected = role or ("rustc" if invocation.name.startswith("rustc") else "cargo")
    try:
        result = subprocess.run([str(invocation), *args], text=True, capture_output=True,
                                cwd=working_directory, timeout=10, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError(f"selected cache tool version is unavailable: {path.name}") from error
    if result.returncode != 0:
        raise RuntimeError(f"selected cache tool version is unavailable: {path.name}")
    version = result.stdout.strip()
    if not version or not version.splitlines()[0].startswith(f"{expected} "):
        raise RuntimeError(f"selected cache tool version has unexpected identity: {expected}")
    return invocation, pin, version


def _classify_target(target: dict[str, object]) -> tuple[str, Literal["library", "binary"]] | None:
    from cargo_capacity_guard import _array, _text

    kinds = [_text(kind, "selected target kind") for kind in _array(target.get("kind"), "selected target kinds")]
    if not kinds or not set(kinds).issubset(_KNOWN_KINDS):
        raise RuntimeError("selected target kinds are incomplete or unknown")
    name = _text(target.get("name"), "selected target name")
    if re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_-]*", name) is None:
        raise RuntimeError("selected target name is invalid")
    prefix = name.replace("-", "_") + "-"
    if "custom-build" in kinds:
        return None
    if set(kinds).issubset(_LIBRARY_KINDS):
        return prefix, "library"
    if set(kinds).issubset(_BINARY_KINDS):
        return prefix, "binary"
    return None


def create_selected_cache_ownership(
    root: Path,
    target: Path,
    metadata: Mapping[str, object],
    input_check: Callable[[], None],
    cargo_executable: str = "cargo",
    *,
    library_alias: bool = False,
) -> SelectedCacheOwnership:
    from cargo_capacity_guard import _array, _object, _text, selected_package_incremental_prefixes
    array, obj, text, selected_prefixes = _array, _object, _text, selected_package_incremental_prefixes
    if not root.is_absolute() or root.resolve() != root:
        raise RuntimeError("selected cache ownership requires a canonical repository root")
    if not target.is_absolute() or target.resolve() != target or target != root / "target":
        raise RuntimeError("selected cache ownership requires the canonical host target")
    target_metadata = Path(text(metadata.get("target_directory"), "cargo target directory"))
    build_metadata = Path(text(metadata.get("build_directory", metadata.get("target_directory")), "cargo build directory"))
    if not target_metadata.is_absolute() or not build_metadata.is_absolute() \
            or target_metadata.resolve() != target or build_metadata.resolve() != target:
        raise RuntimeError("selected cache ownership requires the canonical metadata target")
    input_check()
    selected_prefixes(root, metadata)

    packages: dict[str, dict[str, object]] = {}
    namespaces: dict[str, set[str]] = {}
    namespace_kinds: dict[str, set[str]] = {}
    namespace_target_counts: dict[str, int] = {}
    for item in array(metadata.get("packages"), "selected packages"):
        package = obj(item, "selected package")
        identity = text(package.get("id"), "package id")
        if identity in packages:
            raise RuntimeError("duplicate selected package metadata")
        packages[identity] = package
        for item_target in array(package.get("targets"), "selected targets"):
            target_info = obj(item_target, "selected target")
            target_prefix = text(target_info.get("name"), "selected target name").replace("-", "_") + "-"
            namespace_target_counts[target_prefix] = namespace_target_counts.get(target_prefix, 0) + 1
            classified = _classify_target(target_info)
            if classified is not None:
                prefix, kind = classified
                if prefix in _EXCLUDED_PREFIXES:
                    continue
                namespaces.setdefault(prefix, set()).add(identity)
                namespace_kinds.setdefault(prefix, set()).add(kind)
            else:
                kinds = [text(kind, "selected target kind")
                         for kind in array(target_info.get("kind"), "selected target kinds")]
                if "custom-build" in kinds:
                    name = text(target_info.get("name"), "custom-build target name")
                    prefix = name.replace("-", "_") + "-"
                    namespaces.setdefault(prefix, set()).add(identity)
                    namespace_kinds.setdefault(prefix, set()).add("custom-build")

    members = {text(value, "workspace member") for value in array(metadata.get("workspace_members"), "workspace members")}
    if not members.issubset(packages):
        raise RuntimeError("workspace package metadata is incomplete")
    edges: dict[str, list[str]] = {}
    resolve = obj(metadata.get("resolve"), "selected resolve")
    for item in array(resolve.get("nodes"), "resolve nodes"):
        node = obj(item, "resolve node")
        identity = text(node.get("id"), "resolve id")
        if identity in edges or identity not in packages:
            raise RuntimeError("duplicate or unknown resolve node")
        edges[identity] = [text(obj(dep, "resolve dependency").get("pkg"), "dependency package")
                           for dep in array(node.get("deps"), "resolve dependencies")]
    if set(edges) != set(packages) or any(dep not in edges for deps in edges.values() for dep in deps):
        raise RuntimeError("selected dependency graph is incomplete")
    selected = [identity for identity in members
                if packages[identity].get("manifest_path") == str(root / "crates/kdr-linter/Cargo.toml")
                and packages[identity].get("name") == "kdr-linter"]
    if len(selected) != 1:
        raise RuntimeError("canonical selected host package is missing")
    if library_alias:
        # alias入口で検証したruntimeはHostの依存ではないため、同じcanonical packageを独立rootに束縛する。
        runtime = [identity for identity in members
                   if packages[identity].get("manifest_path") == str(root / "crates/katana-render-runtime/Cargo.toml")
                   and packages[identity].get("name") == "katana-render-runtime"]
        if len(runtime) != 1:
            raise RuntimeError("canonical library alias runtime package is missing or ambiguous")
        selected.extend(runtime)
    closure: set[str] = set()
    pending = selected.copy()
    while pending:
        identity = pending.pop()
        if identity not in closure:
            closure.add(identity)
            pending.extend(edges[identity])

    owners: list[SelectedCacheOwner] = []
    for identity in sorted(members & closure):
        package = packages[identity]
        manifest = Path(text(package.get("manifest_path"), "workspace manifest"))
        if not manifest.is_absolute() or manifest.resolve() != manifest or not manifest.is_relative_to(root):
            raise RuntimeError("workspace manifest is not canonical and contained")
        for item_target in array(package.get("targets"), "workspace targets"):
            target_info = obj(item_target, "workspace target")
            classified = _classify_target(target_info)
            if classified is None:
                continue
            prefix, kind = classified
            if prefix in _EXCLUDED_PREFIXES:
                continue
            if (namespaces[prefix] != {identity} or namespace_kinds[prefix] != {kind}
                    or namespace_target_counts[prefix] != 1
                    or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*-", prefix) is None):
                continue
            source = Path(text(target_info.get("src_path"), "workspace target source"))
            if not source.is_absolute() or source.resolve() != source or not source.is_relative_to(manifest.parent):
                raise RuntimeError("workspace target source is not canonical and contained")
            owners.append(SelectedCacheOwner(prefix, kind, identity, source_pin(manifest), source_pin(source)))
    pinned = tuple(owners)
    cargo_tool = _tool_pin(cargo_executable, ("-Vv",), "cargo", root)
    rustc = os.environ.get("RUSTC", "rustc")
    rustc_tool = _tool_pin(rustc, ("-vV",), "rustc", root)
    environment = dict(os.environ)
    profile = target / "debug"

    def verify() -> None:
        input_check()
        selected_prefixes(root, metadata)
        current_cargo = _tool_pin(str(cargo_tool[0]), ("-Vv",), "cargo", root)
        current_rustc = _tool_pin(str(rustc_tool[0]), ("-vV",), "rustc", root)
        if current_cargo[:2] != cargo_tool[:2] or current_rustc[:2] != rustc_tool[:2]:
            raise RuntimeError("selected cache tool changed")
        if current_cargo != cargo_tool or current_rustc != rustc_tool:
            raise RuntimeError("selected cache tool version changed")
        if dict(os.environ) != environment:
            raise RuntimeError("selected cache environment changed")
        for owner in pinned:
            if source_pin(owner.manifest.path) != owner.manifest or source_pin(owner.source.path) != owner.source:
                raise RuntimeError("selected cache owner source inputs changed")

    verify()
    return SelectedCacheOwnership(root, target, profile, pinned, verify)


def binary_cache_members(session_fd: int, names: list[str]) -> bool:
    """Accept finalized binary sessions, including a graph-less residual leaf pair."""
    from cargo_capacity_guard_lock import INCREMENTAL_INDEX_NAMES, _known_session_members
    if len(names) != len(set(names)) or "work-products.bin" not in names or "metadata.rmeta" in names:
        return False
    indexes = INCREMENTAL_INDEX_NAMES.intersection(names)
    if "dep-graph.bin" in indexes:
        if indexes != INCREMENTAL_INDEX_NAMES or not _known_session_members(session_fd, names, True, False):
            return False
    elif indexes != {"query-cache.bin", "work-products.bin"}:
        return False
    else:
        # A partial purge may leave query-cache and work-products without rmeta.
        for name in names:
            if name not in {"query-cache.bin", "work-products.bin"} and Path(name).suffix != ".o":
                return False
            try:
                value = os.stat(name, dir_fd=session_fd, follow_symlinks=False)
            except OSError:
                return False
            if not stat.S_ISREG(value.st_mode):
                return False
    for name in indexes:
        try:
            value = os.stat(name, dir_fd=session_fd, follow_symlinks=False)
        except OSError:
            return False
        if value.st_nlink != 1 or getattr(value, "st_flags", 0) != 0:
            return False
    return True
