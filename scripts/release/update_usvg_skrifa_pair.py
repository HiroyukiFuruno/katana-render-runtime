#!/usr/bin/env python3
"""Keep KRR's direct skrifa dependency aligned with its selected usvg policy."""

from __future__ import annotations

import argparse
import json
import re
import shlex
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import Any, Union

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python 3.10
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:
        parser_root = str(Path(__file__).resolve().parent)
        if parser_root not in sys.path:
            sys.path.insert(0, parser_root)
        from _tomli import _parser as tomllib  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "crates/katana-render-runtime/Cargo.toml"
RUNTIME_PACKAGE = "katana-render-runtime"
USVG_PACKAGE = "usvg"
SKRIFA_PACKAGE = "skrifa"
CommandRunner = Callable[..., Union[subprocess.CompletedProcess[bytes], subprocess.CompletedProcess[str]]]


def parse_cargo_command(value: str) -> list[str]:
    """Parse the configured cargo command without invoking a shell."""

    if "\0" in value:
        raise ValueError("--cargo must not contain a NUL byte")
    try:
        command = shlex.split(value, posix=True)
    except ValueError as error:
        raise ValueError(f"invalid --cargo command: {error}") from error
    if not command:
        raise ValueError("--cargo must not be empty")
    return command


def _mapping(value: Any, description: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"cargo metadata {description} must be an object")
    return value


def _metadata_indexes(
    metadata: Mapping[str, Any],
) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    packages_value = metadata.get("packages")
    resolve_value = _mapping(metadata.get("resolve"), "resolve")
    nodes_value = resolve_value.get("nodes")
    if not isinstance(packages_value, list) or not isinstance(nodes_value, list):
        raise ValueError("cargo metadata is missing packages or resolve nodes")

    packages: dict[str, Mapping[str, Any]] = {}
    for item in packages_value:
        package = _mapping(item, "package")
        package_id = package.get("id")
        if not isinstance(package_id, str) or package_id in packages:
            raise ValueError("cargo metadata contains an invalid or duplicate package id")
        packages[package_id] = package

    nodes: dict[str, Mapping[str, Any]] = {}
    for item in nodes_value:
        node = _mapping(item, "resolve node")
        package_id = node.get("id")
        if not isinstance(package_id, str) or package_id in nodes:
            raise ValueError("cargo metadata contains an invalid or duplicate resolve node id")
        nodes[package_id] = node
    return packages, nodes


def _resolve_edges(node: Mapping[str, Any], package_name: str) -> set[str]:
    dependencies = node.get("deps")
    if not isinstance(dependencies, list):
        raise ValueError("cargo metadata resolve node is missing dependency edges")
    targets: set[str] = set()
    for edge_value in dependencies:
        edge = _mapping(edge_value, "resolve dependency edge")
        if edge.get("name") != package_name:
            continue
        kinds = edge.get("dep_kinds")
        if not isinstance(kinds, list):
            raise ValueError("cargo metadata dependency edge is missing dependency kinds")
        if any(_mapping(kind, "dependency kind").get("kind") is None for kind in kinds):
            package_id = edge.get("pkg")
            if not isinstance(package_id, str):
                raise ValueError("cargo metadata dependency edge has an invalid package id")
            targets.add(package_id)
    return targets


def select_usvg_package(
    metadata: Mapping[str, Any],
) -> tuple[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any], Mapping[str, Mapping[str, Any]]]:
    """Select the single usvg package reachable from KRR's resolved graph."""

    packages, nodes = _metadata_indexes(metadata)
    members = metadata.get("workspace_members")
    if not isinstance(members, list):
        raise ValueError("cargo metadata is missing workspace members")
    runtime_ids = [
        package_id
        for package_id in members
        if isinstance(package_id, str)
        and package_id in packages
        and packages[package_id].get("name") == RUNTIME_PACKAGE
    ]
    if len(runtime_ids) != 1:
        raise ValueError(f"expected one workspace {RUNTIME_PACKAGE} package, found {len(runtime_ids)}")

    reachable: set[str] = set()
    pending = [runtime_ids[0]]
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        if package_id not in packages or package_id not in nodes:
            raise ValueError(f"cargo metadata resolution is missing package node {package_id}")
        reachable.add(package_id)
        dependencies = nodes[package_id].get("deps")
        if not isinstance(dependencies, list):
            raise ValueError("cargo metadata resolve node is missing dependency edges")
        for edge_value in dependencies:
            edge = _mapping(edge_value, "resolve dependency edge")
            target = edge.get("pkg")
            if not isinstance(target, str):
                raise ValueError("cargo metadata dependency edge has an invalid package id")
            pending.append(target)

    usvg_ids = [
        package_id
        for package_id in reachable
        if packages[package_id].get("name") == USVG_PACKAGE
    ]
    if len(usvg_ids) != 1:
        raise ValueError(f"expected one resolved {USVG_PACKAGE} package reachable from KRR, found {len(usvg_ids)}")
    usvg_id = usvg_ids[0]
    usvg = packages[usvg_id]
    source = usvg.get("source")
    if not isinstance(source, str) or not source.startswith("registry+"):
        raise ValueError("selected usvg package is not from a registry")
    return packages[runtime_ids[0]], nodes[runtime_ids[0]], usvg, nodes


def skrifa_requirement(usvg: Mapping[str, Any]) -> str:
    """Read usvg's single normal registry requirement for skrifa."""

    source = usvg.get("source")
    dependencies = usvg.get("dependencies")
    if not isinstance(source, str) or not isinstance(dependencies, list):
        raise ValueError("selected usvg package is missing registry source or dependencies")
    candidates = []
    for item in dependencies:
        dependency = _mapping(item, "package dependency")
        if dependency.get("name") != SKRIFA_PACKAGE or dependency.get("kind") is not None:
            continue
        if dependency.get("source") != source:
            raise ValueError("usvg's skrifa dependency uses a different registry")
        requirement = dependency.get("req")
        if not isinstance(requirement, str) or not requirement.strip():
            raise ValueError("usvg's skrifa dependency has an invalid requirement")
        candidates.append(requirement)
    if len(candidates) != 1:
        raise ValueError(f"expected one normal registry {SKRIFA_PACKAGE} dependency in usvg, found {len(candidates)}")
    return candidates[0]


def runtime_skrifa_requirement(runtime: Mapping[str, Any], registry: str) -> str:
    dependencies = runtime.get("dependencies")
    if not isinstance(dependencies, list):
        raise ValueError("KRR runtime package is missing dependency metadata")
    candidates = []
    for item in dependencies:
        dependency = _mapping(item, "package dependency")
        if dependency.get("name") != SKRIFA_PACKAGE or dependency.get("kind") is not None:
            continue
        if dependency.get("source") != registry:
            raise ValueError("KRR's skrifa dependency uses a different registry")
        requirement = dependency.get("req")
        if not isinstance(requirement, str) or not requirement.strip():
            raise ValueError("KRR's skrifa dependency has an invalid requirement")
        candidates.append(requirement)
    if len(candidates) != 1:
        raise ValueError(f"expected one direct normal registry {SKRIFA_PACKAGE} dependency in KRR, found {len(candidates)}")
    return candidates[0]


def _caret_equivalent(left: str, right: str) -> bool:
    """Treat only Cargo's omitted-caret spelling as equivalent."""

    return (left[1:] if left.startswith("^") else left) == (right[1:] if right.startswith("^") else right)


def sync_manifest_text(contents: str, requirement: str) -> tuple[str, bool]:
    """Update only the direct string dependency in the package's [dependencies]."""

    if not requirement or requirement != requirement.strip() or '"' in requirement or "\n" in requirement:
        raise ValueError("usvg's skrifa requirement cannot be written as a TOML string")
    try:
        manifest = tomllib.loads(contents)
    except tomllib.TOMLDecodeError as error:
        raise ValueError(f"cannot parse KRR Cargo.toml: {error}") from error
    dependencies = manifest.get("dependencies")
    if not isinstance(dependencies, dict) or not isinstance(dependencies.get(SKRIFA_PACKAGE), str):
        raise ValueError("KRR direct skrifa dependency must be a string requirement")
    current = dependencies[SKRIFA_PACKAGE]
    if _caret_equivalent(current, requirement):
        return contents, False

    lines = contents.splitlines(keepends=True)
    section: str | None = None
    matches: list[tuple[int, re.Match[str]]] = []
    assignment = re.compile(r'^(\s*skrifa\s*=\s*)"([^"\r\n]*)"(\s*(?:#.*)?)(\r?\n)?$')
    for index, line in enumerate(lines):
        header = re.match(r"^\s*\[([^\]]+)\]\s*(?:#.*)?(?:\r?\n)?$", line)
        if header is not None:
            section = header.group(1)
        elif section == "dependencies":
            match = assignment.match(line)
            if match is not None:
                matches.append((index, match))
    if len(matches) != 1:
        raise ValueError(f"expected one string skrifa assignment in [dependencies], found {len(matches)}")
    index, match = matches[0]
    lines[index] = f'{match.group(1)}"{requirement}"{match.group(3)}{match.group(4) or ""}'
    return "".join(lines), True


def _metadata(cargo: Sequence[str], runner: CommandRunner) -> Mapping[str, Any]:
    command = [*cargo, "metadata", "--format-version", "1"]
    result = runner(command, cwd=ROOT, check=True, capture_output=True, text=True)
    output = result.stdout
    if isinstance(output, bytes):
        output = output.decode("utf-8")
    try:
        parsed = json.loads(output)
    except (TypeError, json.JSONDecodeError) as error:
        raise ValueError(f"cargo metadata returned invalid JSON: {error}") from error
    return _mapping(parsed, "response")


def _resolved_registry_skrifa(
    node: Mapping[str, Any],
    packages: Mapping[str, Mapping[str, Any]],
    nodes: Mapping[str, Mapping[str, Any]],
    parent_source: str,
) -> Mapping[str, Any]:
    dependency_ids = _resolve_edges(node, SKRIFA_PACKAGE)
    if len(dependency_ids) != 1:
        raise ValueError(f"expected one resolved normal {SKRIFA_PACKAGE} edge, found {len(dependency_ids)}")
    dependency_id = next(iter(dependency_ids))
    dependency = packages.get(dependency_id)
    if dependency is None or dependency_id not in nodes:
        raise ValueError("cargo metadata resolution is missing the selected skrifa package")
    source = dependency.get("source")
    if not isinstance(source, str) or not source.startswith("registry+") or source != parent_source:
        raise ValueError("resolved skrifa dependency uses a different or non-registry source")
    return dependency


def validate_resolution(metadata: Mapping[str, Any], expected_requirement: str) -> None:
    runtime, runtime_node, usvg, nodes = select_usvg_package(metadata)
    packages, _ = _metadata_indexes(metadata)
    usvg_id = usvg.get("id")
    if not isinstance(usvg_id, str):
        raise ValueError("selected usvg package has an invalid id")
    usvg_node = nodes.get(usvg_id)
    if usvg_node is None:
        raise ValueError("cargo metadata resolution is missing the selected usvg node")
    usvg_source = usvg.get("source")
    assert isinstance(usvg_source, str)
    usvg_skrifa = _resolved_registry_skrifa(usvg_node, packages, nodes, usvg_source)
    runtime_source = runtime.get("source")
    if runtime_source is not None:
        raise ValueError("KRR runtime package unexpectedly has a registry source")
    runtime_skrifa = _resolved_registry_skrifa(runtime_node, packages, nodes, usvg_source)
    if runtime_skrifa.get("id") != usvg_skrifa.get("id"):
        raise ValueError("KRR direct skrifa and selected usvg resolve to different package versions")
    if skrifa_requirement(usvg) != expected_requirement:
        raise ValueError("usvg skrifa requirement changed during resolution")
    if not _caret_equivalent(runtime_skrifa_requirement(runtime, usvg_source), expected_requirement):
        raise ValueError("KRR direct skrifa requirement does not match usvg's requirement")


def run_update(
    cargo: Sequence[str],
    runner: CommandRunner = subprocess.run,
    manifest_path: Path = MANIFEST,
) -> None:
    before = _metadata(cargo, runner)
    runtime, runtime_node, usvg, nodes = select_usvg_package(before)
    requirement = skrifa_requirement(usvg)
    manifest_text = manifest_path.read_text(encoding="utf-8")
    updated_text, changed = sync_manifest_text(manifest_text, requirement)

    packages, _ = _metadata_indexes(before)
    usvg_id = usvg.get("id")
    usvg_source = usvg.get("source")
    if not isinstance(usvg_id, str) or not isinstance(usvg_source, str):
        raise ValueError("selected usvg package has invalid metadata")
    usvg_node = nodes.get(usvg_id)
    if usvg_node is None:
        raise ValueError("cargo metadata resolution is missing the selected usvg node")
    usvg_skrifa = _resolved_registry_skrifa(usvg_node, packages, nodes, usvg_source)
    direct_skrifa = _resolved_registry_skrifa(runtime_node, packages, nodes, usvg_source)
    if changed:
        manifest_path.write_text(updated_text, encoding="utf-8")
    if changed or direct_skrifa.get("id") != usvg_skrifa.get("id"):
        old_version = direct_skrifa.get("version")
        if not isinstance(old_version, str):
            raise ValueError("currently resolved KRR skrifa package has an invalid version")
        runner([*cargo, "update", "-p", f"{SKRIFA_PACKAGE}@{old_version}"], cwd=ROOT, check=True)

    after = _metadata(cargo, runner)
    validate_resolution(after, requirement)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo", required=True, help="cargo command, including an optional wrapper")
    args = parser.parse_args(argv)
    try:
        cargo = parse_cargo_command(args.cargo)
        run_update(cargo)
    except ValueError as error:
        print(f"usvg/skrifa compatibility update failed: {error}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        print(f"usvg/skrifa compatibility update failed: {shlex.join(error.cmd)}", file=sys.stderr)
        return error.returncode or 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
