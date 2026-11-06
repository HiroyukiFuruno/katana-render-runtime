#!/usr/bin/env python3
"""Reject releases whose resolved dependencies or pinned runtime assets are stale."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, replace
from http.client import HTTPMessage
from functools import total_ordering
from pathlib import Path
from typing import IO
from urllib import error, parse, request

_loader_root = str(Path(__file__).resolve().parent)
sys.path.insert(0, _loader_root)
try:
    from toml_loader import load_toml_parser
finally:
    sys.path.remove(_loader_root)

_expected_loader = Path(_loader_root, "toml_loader.py").resolve()
if any(Path(origin).resolve() != _expected_loader for origin in (sys.modules["toml_loader"].__file__ or "", load_toml_parser.__code__.co_filename)):
    raise ImportError("loaded TOML helper is outside the owned release directory")
tomllib = load_toml_parser()

sys.path.insert(0, _loader_root)
try:
    from html_performance_harness_dependency_closure import (
        RETAINED_UTF8_ITER_VERSION,
        load_pinned_parent_lock,
        verify_utf8_iter_retention,
    )
    from update_html_performance_harness_lock import retention_plan, verify_retained_packages
finally:
    sys.path.remove(_loader_root)

TIMEOUT_SECONDS = 15
MAX_RESPONSE_BYTES = 1_000_000
RUNTIME_CATALOG = Path("scripts/runtime-assets/runtime-asset-common.ts")
RUNTIME_CATALOG_CLI = Path(__file__).resolve().parents[1] / "runtime-assets/runtime-asset-common.ts"
NON_RELEASE_CARGO_MANIFEST_PARTS = {"target", "vendor", ".git", ".worktrees", "tmp"}
KRR_SKRIFA_MANIFEST = Path("crates/katana-render-runtime/Cargo.toml")
KRR_SVG_ENGINE_ROOT = Path("crates/katana-render-runtime/vendor/svg-engine")
KRR_SVG_ENGINE_PROVENANCE = KRR_SVG_ENGINE_ROOT / "PROVENANCE.json"
KRR_SVG_ENGINE_COMMIT = "c01f0165059e472f45472cccddc573d32c13548a"
KRR_SVG_ENGINE_SOURCE_FILE_COUNT = 48
KRR_SVG_HARFRUST_ALIAS = "svg_harfrust"
KRR_SVG_HARFRUST_PACKAGE = "harfrust"
UTF8_ITER_LATEST_COMPATIBLE_VERSION = "1.1.0"
UTF_TYPES_LATEST_COMPATIBLE_VERSION = "1.0.0"
UTF8_ITER_RETENTION_CHANGES = {
    ("Adding", "utf_types", UTF_TYPES_LATEST_COMPATIBLE_VERSION, None),
    ("Updating", "utf8_iter", RETAINED_UTF8_ITER_VERSION,
     UTF8_ITER_LATEST_COMPATIBLE_VERSION),
}
CRC32FAST_RETENTION_CHANGES = {
    ("Updating", "crc32fast", "1.5.3", "1.5.4"),
}
LOCK_CHANGE_RE = re.compile(
    r"^\s+(Adding|Updating|Removing|Downgrading) ([A-Za-z0-9_-]+) v([^\s]+)"
    r"(?: -> v([^\s]+))?$"
)
LOCKING_RE = re.compile(r"\bLocking\s+([0-9]+)\s+packages?\b")


class _StripAuthorizationRedirectHandler(request.HTTPRedirectHandler):
    """転送先への認証情報流出を防ぐため、redirectでは認証ヘッダーを除去する。"""

    def redirect_request(
        self,
        req: request.Request,
        fp: IO[bytes],
        code: int,
        msg: str,
        headers: HTTPMessage,
        newurl: str,
    ) -> request.Request | None:
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is not None:
            redirected.remove_header("Authorization")
        return redirected


@total_ordering
@dataclass(frozen=True)
class Version:
    parts: tuple[int, int, int]
    stable: bool
    prerelease: tuple[tuple[int, int | str], ...]

    def __lt__(self, other: object) -> bool:
        if not isinstance(other, Version):
            return NotImplemented
        if self.parts != other.parts:
            return self.parts < other.parts
        if self.stable != other.stable:
            return not self.stable
        for index in range(min(len(self.prerelease), len(other.prerelease))):
            left, right = self.prerelease[index], other.prerelease[index]
            if left == right:
                continue
            left_kind, left_value = left
            right_kind, right_value = right
            if left_kind != right_kind:
                # SemVer では数値の prerelease identifier を非数値より前に置く。
                return left_kind < right_kind
            if isinstance(left_value, int) and isinstance(right_value, int):
                return left_value < right_value
            if isinstance(left_value, str) and isinstance(right_value, str):
                return left_value < right_value
            raise TypeError("prerelease identifier kind does not match its value")
        # それまで同一なら、identifier が短い prerelease の優先度を下げる。
        return len(self.prerelease) < len(other.prerelease)

    @classmethod
    def parse(cls, value: str) -> Version:
        number = r"(?:0|[1-9][0-9]*)"
        identifier = r"[0-9A-Za-z-]+"
        match = re.fullmatch(
            rf"({number})\.({number})\.({number})(?:-({identifier}(?:\.{identifier})*))?(?:\+{identifier}(?:\.{identifier})*)?",
            value,
        )
        if match is None:
            raise ValueError(f"unsupported semantic version: {value!r}")
        identifiers = match.group(4).split(".") if match.group(4) else []
        if any(item.isdigit() and len(item) > 1 and item.startswith("0") for item in identifiers):
            raise ValueError(f"unsupported semantic version: {value!r}")
        prerelease = tuple((0, int(item)) if item.isdigit() else (1, item) for item in identifiers)
        major, minor, patch = match.group(1, 2, 3)
        if major is None or minor is None or patch is None:
            raise ValueError(f"unsupported semantic version: {value!r}")
        return cls((int(major), int(minor), int(patch)), not identifiers, prerelease)


@dataclass(frozen=True)
class Package:
    ecosystem: str
    name: str
    current: str
    incompatible_prefix_length: int | None = None
    requirement_operator: str | None = None
    cargo_requirement: str | None = None
    skia_hold_candidate: bool = False
    ast_lint_hold_candidate: bool = False


@dataclass(frozen=True)
class AlternateRegistryDependency:
    package: Package
    registry: str


_CARGO_FULL_VERSION = (
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?:[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+(?:[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)
_CARGO_PRERELEASE_VERSION = (
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"-(?:[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)"
    r"(?:\+(?:[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)
_CARGO_REQUIREMENT_TERM = re.compile(
    r"\s*(?P<operator>\^|~|>=|<=|>|<|=)?\s*"
    rf"(?P<version>{_CARGO_FULL_VERSION}|(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*|[xX*])){{0,2}}|[xX*])\s*"
)
_CARGO_PRERELEASE_REQUIREMENT = re.compile(
    r"(?P<operator>\^|~|>=|<=|>|<|=)?"
    rf"(?P<version>{_CARGO_PRERELEASE_VERSION})"
)


@dataclass(frozen=True)
class CargoRequirementTerm:
    operator: str
    parts: tuple[int, ...]
    prerelease: Version | None = None


def _cargo_requirement_parts(value: str) -> tuple[int, ...] | None:
    """Return the numeric prefix of a Cargo requirement term.

    Wildcards are valid only as a trailing suffix.  The caller retains the
    original term so that matching can preserve Cargo's partial-version
    semantics.
    """
    if value in {"*", "x", "X"}:
        return ()
    parts = value.split(".")
    numeric: list[int] = []
    wildcard_seen = False
    for part in parts:
        if part in {"*", "x", "X"}:
            wildcard_seen = True
            continue
        if wildcard_seen:
            raise ValueError(f"unsupported Cargo version requirement: {value!r}")
        numeric.append(int(part))
    return tuple(numeric)


def _parse_cargo_requirement(requirement: str) -> list[CargoRequirementTerm]:
    """Parse Cargo's comma-conjoined numeric requirement syntax.

    Cargo accepts comparison clauses and wildcard prefixes in addition to the
    short forms handled by the release policy's historical compatibility
    model.  Parse those clauses here rather than rejecting a valid manifest.
    """
    terms: list[CargoRequirementTerm] = []
    for raw_term in requirement.split(","):
        match = _CARGO_REQUIREMENT_TERM.fullmatch(raw_term)
        if match is None:
            raise ValueError(f"unsupported Cargo version requirement: {requirement!r}")
        operator = match.group("operator") or ""
        raw_version = match.group("version")
        # Cargo accepts SemVer build metadata in requirements, but SemVer
        # precedence ignores it.  Remove it only for requirement matching;
        # retain the original requirement on Package for Cargo resolution.
        version_without_build = raw_version.split("+", 1)[0]
        prerelease = (
            Version.parse(raw_version)
            if "-" in version_without_build
            else None
        )
        parts = (
            prerelease.parts
            if prerelease is not None
            else _cargo_requirement_parts(version_without_build)
        )
        assert parts is not None
        if operator and not parts:
            raise ValueError(f"unsupported Cargo version requirement: {requirement!r}")
        if (
            not operator
            and any(part in {"*", "x", "X"} for part in version_without_build.split("."))
        ):
            operator = "*"
        terms.append(CargoRequirementTerm(operator, parts, prerelease))
    if not terms:
        raise ValueError(f"unsupported Cargo version requirement: {requirement!r}")
    return terms


def _embedded_harfrust_requirement(requirement: object) -> tuple[int, int, int] | None:
    if not isinstance(requirement, str):
        return None
    try:
        terms = _parse_cargo_requirement(requirement.strip())
    except ValueError:
        return None
    if len(terms) != 1:
        return None
    term = terms[0]
    if term.prerelease is not None or term.operator not in {"", "^"}:
        return None
    if len(term.parts) not in {2, 3} or term.parts[:2] != (0, 12):
        return None
    major = term.parts[0]
    minor = term.parts[1]
    patch = term.parts[2] if len(term.parts) == 3 else 0
    return major, minor, patch


def cargo_requirement_matches(requirement: str, version: Version) -> bool:
    """Return whether ``version`` satisfies a supported Cargo requirement."""
    terms = _parse_cargo_requirement(requirement)
    # Cargo prereleases only match when the requirement explicitly names a
    # prerelease for the identical release tuple.  Enforce this before each
    # comparator: a broad upper/lower bound must not admit a different tuple.
    if not version.stable and not any(
        term.prerelease is not None and term.prerelease.parts == version.parts
        for term in terms
    ):
        return False
    for term in terms:
        operator = term.operator
        parts = term.parts
        padded_parts = (*parts, 0, 0, 0)
        candidate = term.prerelease or Version(
            (padded_parts[0], padded_parts[1], padded_parts[2]), True, ()
        )
        prefix_matches = version.parts[:len(parts)] == parts
        if operator == "*":
            # A Cargo wildcard is an exact prefix range: `1.2.*` accepts
            # 1.2.x but must reject 1.3.0.
            allowed = prefix_matches
        elif operator == "=":
            allowed = version == candidate if term.prerelease is not None else prefix_matches
        elif operator == "^" or operator == "":
            if version < candidate:
                allowed = False
            elif parts[0] > 0:
                allowed = version.parts[0] == parts[0]
            elif len(parts) == 1:
                allowed = version.parts[0] == 0
            elif parts[1] > 0:
                allowed = version.parts[:2] == parts[:2]
            elif len(parts) == 2:
                allowed = version.parts[:2] == parts[:2]
            else:
                allowed = prefix_matches
        elif operator == "~":
            # `~1` is bounded by the major tuple, while `~1.2` and
            # `~1.2.3` are bounded by the major/minor tuple.
            allowed = version >= candidate and (
                version.parts[0] == parts[0]
                and (len(parts) == 1 or version.parts[1] == parts[1])
            )
        elif operator == ">=":
            allowed = version >= candidate
        elif operator == ">":
            # Cargo compares a partial strict lower bound at its supplied
            # precision: `>1.2` starts at 1.3.0 and `>1` starts at 2.0.0.
            allowed = (
                version.parts[:len(parts)] > parts
                if len(parts) < 3
                else version > candidate
            )
        elif operator == "<=":
            # Cargo treats a partial inclusive upper bound as applying to the
            # supplied precision: `<=1` includes 1.x and `<=1.2` includes
            # 1.2.x.  A fully specified bound retains normal semver ordering.
            allowed = (
                version.parts[:len(parts)] <= parts
                if len(parts) < 3
                else version <= candidate
            )
        elif operator == "<":
            allowed = version < candidate
        else:  # pragma: no cover - _CARGO_REQUIREMENT_TERM constrains this.
            raise ValueError(f"unsupported Cargo version requirement: {requirement!r}")
        if not allowed:
            return False
    return True


def cargo_requirement_anchor(requirement: str) -> str:
    """Choose a stable display anchor for a compound Cargo requirement."""
    lower_bounds = [
        term.parts for term in _parse_cargo_requirement(requirement)
        if term.parts and term.operator in {"", "*", "^", "~", "=", ">", ">="}
    ]
    parts = max(lower_bounds, default=(), key=lambda value: (*value, *(0 for _ in range(3 - len(value)))))
    return ".".join(str(part) for part in (*parts, *(0 for _ in range(3 - len(parts)))))


def cargo_requirement_package(name: str, requirement: str) -> Package:
    """Translate a Cargo requirement into the release freshness model."""
    prerelease_match = _CARGO_PRERELEASE_REQUIREMENT.fullmatch(requirement)
    if prerelease_match is not None:
        # Preserve the prerelease so Cargo's special prerelease matching rules
        # are evaluated instead of treating it as a stable numeric anchor.
        current = prerelease_match.group("version")
        Version.parse(current)
        return Package(
            "Rust manifest dependency",
            name,
            current,
            requirement_operator=prerelease_match.group("operator") or "",
            cargo_requirement=requirement,
        )
    match = re.fullmatch(r"(=|\^|~)?([0-9]+)(?:\.([0-9]+))?(?:\.([0-9]+))?", requirement)
    if match is None:
        current = cargo_requirement_anchor(requirement)
        Version.parse(current)
        return Package("Rust manifest dependency", name, current, requirement_operator="compound", cargo_requirement=requirement)

    operator = match.group(1)
    major = int(match.group(2))
    minor = match.group(3)
    patch = match.group(4)
    incompatible_prefix_length: int | None = None
    if operator == "~":
        incompatible_prefix_length = 1 if minor is None else 2
    elif operator == "^" or operator is None:
        if major > 0 or minor is None:
            incompatible_prefix_length = 1
        elif int(minor) > 0 or patch is None:
            incompatible_prefix_length = 2
        else:
            incompatible_prefix_length = 3
    elif operator == "=":
        # Cargo treats partial exact requirements as a prefix: `=1.2`
        # accepts 1.2.x but must reject a newly available 1.3.
        incompatible_prefix_length = len([part for part in (match.group(2), minor, patch) if part is not None])
    elif minor is None:
        incompatible_prefix_length = 1
    elif patch is None:
        incompatible_prefix_length = 1 if major > 0 else 2
    current = ".".join((match.group(2), minor or "0", patch or "0"))
    Version.parse(current)
    return Package("Rust manifest dependency", name, current, incompatible_prefix_length, operator)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    return parser.parse_args()


def load_bun_lock(path: Path) -> dict[str, object]:
    source = path.read_text(encoding="utf-8")
    result: list[str] = []
    quoted, escaped = False, False
    for index, character in enumerate(source):
        if quoted:
            result.append(character)
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quoted = False
            continue
        if character == '"':
            quoted = True
        elif character == ",":
            following = source[index + 1 :].lstrip()
            if following.startswith(("}", "]")):
                continue
        result.append(character)
    return _string_keyed_object(json.loads("".join(result)), "bun.lock must be a JSON object")


def _string_keyed_object(value: object, message: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ValueError(message)
    result: dict[str, object] = {}
    for key, item in value.items():
        if not isinstance(key, str):
            raise ValueError(message)
        result[key] = item
    return result


def cargo_metadata(root: Path) -> dict[str, object]:
    command = ["cargo", "metadata", "--format-version=1", "--all-features", "--locked", "--manifest-path", str(root / "Cargo.toml")]
    try:
        completed = subprocess.run(command, check=True, capture_output=True, text=True)
        payload = json.loads(completed.stdout)
    except (OSError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        raise ValueError(f"cargo metadata failed: {exc}") from exc
    return _string_keyed_object(payload, "cargo metadata response must be a JSON object")


def _runtime_metadata(metadata: dict[str, object]) -> tuple[dict[str, object], dict[str, object]]:
    packages = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    nodes = metadata.get("resolve")
    if not isinstance(packages, list) or not isinstance(workspace_members, list) or not isinstance(nodes, dict):
        raise ValueError("cargo metadata is missing KRR runtime resolution")
    runtime = [
        package
        for package in packages
        if isinstance(package, dict)
        and package.get("id") in workspace_members
        and package.get("name") == "katana-render-runtime"
    ]
    if len(runtime) != 1:
        raise ValueError("expected one resolved KRR runtime package")
    runtime_id = runtime[0].get("id")
    resolve_nodes = nodes.get("nodes")
    if not isinstance(runtime_id, str) or not isinstance(resolve_nodes, list):
        raise ValueError("cargo metadata is missing KRR runtime resolution nodes")
    runtime_nodes = [node for node in resolve_nodes if isinstance(node, dict) and node.get("id") == runtime_id]
    if len(runtime_nodes) != 1:
        raise ValueError("expected one resolved KRR runtime node")
    return runtime[0], runtime_nodes[0]


def verified_krr_usvg_skrifa_manifest(root: Path, metadata: dict[str, object]) -> Path | None:
    """Return KRR's embedded SVG-engine manifest after strict provenance validation."""
    manifest = root / KRR_SKRIFA_MANIFEST
    if not manifest.is_file():
        return None
    provenance_path = root / KRR_SVG_ENGINE_PROVENANCE
    engine_root = root / KRR_SVG_ENGINE_ROOT
    if not provenance_path.is_file() or not engine_root.is_dir():
        raise ValueError("KRR embedded SVG engine source or provenance is missing")
    try:
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"KRR embedded SVG engine provenance is invalid: {error}") from error
    if not isinstance(provenance, dict):
        raise ValueError("KRR embedded SVG engine provenance must be an object")
    if provenance.get("schema") != "krr-svg-engine-source-provenance-v1":
        raise ValueError("KRR embedded SVG engine provenance has an unexpected schema")
    if provenance.get("sourceCommit") != KRR_SVG_ENGINE_COMMIT:
        raise ValueError("KRR embedded SVG engine provenance has an unexpected source commit")
    if provenance.get("licenseExpression") != "Apache-2.0 OR MIT":
        raise ValueError("KRR embedded SVG engine provenance has an unexpected license")
    if provenance.get("sourceFileCount") != KRR_SVG_ENGINE_SOURCE_FILE_COUNT:
        raise ValueError("KRR embedded SVG engine provenance has an unexpected source file count")
    for relative_path in ("usvg/lib.rs", "resvg/lib.rs"):
        source_path = engine_root / relative_path
        if not source_path.is_file() or not source_path.read_bytes().strip():
            raise ValueError(f"KRR embedded SVG engine source is missing or empty: {source_path}")

    runtime, runtime_node = _runtime_metadata(metadata)
    metadata_manifest = runtime.get("manifest_path")
    if not isinstance(metadata_manifest, str) or Path(metadata_manifest).resolve() != manifest.resolve():
        raise ValueError("resolved KRR package does not match the expected runtime manifest")
    manifest_payload = _load_manifest_payload(root, manifest)
    dependencies = manifest_payload.get("dependencies")
    if not isinstance(dependencies, dict) or any(name in dependencies for name in ("resvg", "usvg")):
        raise ValueError("KRR manifest retains an external resvg/usvg dependency")
    alias_declaration = dependencies.get(KRR_SVG_HARFRUST_ALIAS)
    if not isinstance(alias_declaration, dict) or alias_declaration.get("package") != KRR_SVG_HARFRUST_PACKAGE:
        raise ValueError("KRR SVG HarfRust alias does not target the approved package")
    alias_requirement = _embedded_harfrust_requirement(alias_declaration.get("version"))
    if alias_requirement is None:
        raise ValueError("KRR SVG HarfRust alias requirement is not compatible with the embedded 0.12 API")

    runtime_dependencies = runtime.get("dependencies")
    runtime_edges = runtime_node.get("deps")
    if not isinstance(runtime_dependencies, list) or not isinstance(runtime_edges, list):
        raise ValueError("KRR runtime metadata is missing dependency edges")
    direct_harfrust = [
        dependency
        for dependency in runtime_dependencies
        if isinstance(dependency, dict)
        and dependency.get("name") == KRR_SVG_HARFRUST_PACKAGE
        and dependency.get("rename") == KRR_SVG_HARFRUST_ALIAS
        and dependency.get("kind") is None
    ]
    if len(direct_harfrust) != 1:
        raise ValueError("expected one direct normal SVG HarfRust alias in KRR")
    metadata_alias_requirement = _embedded_harfrust_requirement(direct_harfrust[0].get("req"))
    if metadata_alias_requirement is None:
        raise ValueError("KRR SVG HarfRust alias requirement is not compatible with the embedded 0.12 API")
    if metadata_alias_requirement != alias_requirement:
        raise ValueError("KRR SVG HarfRust alias requirement does not match its manifest")
    resolved_harfrust = [
        dependency
        for dependency in runtime_edges
        if isinstance(dependency, dict)
        and dependency.get("name") == KRR_SVG_HARFRUST_ALIAS
    ]
    if len(resolved_harfrust) != 1:
        raise ValueError("expected one resolved KRR SVG HarfRust alias")
    direct_skrifa = [
        dependency
        for dependency in runtime_dependencies
        if isinstance(dependency, dict) and dependency.get("name") == "skrifa" and dependency.get("kind") is None
    ]
    if len(direct_skrifa) != 1:
        raise ValueError("expected one direct normal skrifa dependency in KRR")
    requirement = direct_skrifa[0].get("req")
    if not isinstance(requirement, str) or not re.fullmatch(r"\^?0\.49(?:\.\d+)?", requirement.strip()):
        raise ValueError("KRR direct skrifa requirement is not compatible with the embedded 0.49 API")
    resolved_skrifa = [
        dependency
        for dependency in runtime_edges
        if isinstance(dependency, dict) and dependency.get("name") == "skrifa"
    ]
    if len(resolved_skrifa) != 1:
        raise ValueError("expected one resolved KRR skrifa dependency")
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise ValueError("cargo metadata is missing packages")
    harfrust_package_id = resolved_harfrust[0].get("pkg")
    resolved_harfrust_package = next(
        (package for package in packages if isinstance(package, dict) and package.get("id") == harfrust_package_id),
        None,
    )
    if (
        not isinstance(resolved_harfrust_package, dict)
        or resolved_harfrust_package.get("name") != KRR_SVG_HARFRUST_PACKAGE
        or not isinstance(resolved_harfrust_package.get("source"), str)
        or not resolved_harfrust_package["source"].startswith("registry+")
    ):
        raise ValueError("resolved KRR SVG HarfRust alias is not the approved registry package")
    if direct_harfrust[0].get("source") != resolved_harfrust_package["source"]:
        raise ValueError("KRR SVG HarfRust alias source does not match registry resolution")
    try:
        resolved_harfrust_version = Version.parse(str(resolved_harfrust_package.get("version", "")))
    except ValueError:
        resolved_harfrust_version = None
    if (
        resolved_harfrust_version is None
        or not resolved_harfrust_version.stable
        or resolved_harfrust_version.parts[:2] != (0, 12)
        or resolved_harfrust_version.parts < alias_requirement
    ):
        raise ValueError("resolved KRR SVG HarfRust alias is not compatible with the embedded 0.12 API")
    package_id = resolved_skrifa[0].get("pkg")
    resolved_package = next((package for package in packages if isinstance(package, dict) and package.get("id") == package_id), None)
    if not isinstance(resolved_package, dict) or not isinstance(resolved_package.get("source"), str) or not resolved_package["source"].startswith("registry+"):
        raise ValueError("resolved KRR skrifa dependency is not a registry package")
    declared_source = direct_skrifa[0].get("source")
    if declared_source != resolved_package["source"]:
        raise ValueError("KRR direct skrifa dependency source does not match registry resolution")
    if not re.fullmatch(r"0\.49(?:\.\d+)?", str(resolved_package.get("version", ""))):
        raise ValueError("resolved KRR skrifa dependency is not compatible with the embedded 0.49 API")
    return manifest.resolve()


def rust_packages(root: Path, metadata: dict[str, object] | None = None) -> list[Package]:
    if metadata is None:
        metadata = cargo_metadata(root)
    packages = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    resolve = metadata.get("resolve")
    if not isinstance(packages, list) or not isinstance(workspace_members, list) or not isinstance(resolve, dict):
        raise ValueError("cargo metadata response is missing package resolution data")
    packages_by_id = {
        item.get("id"): item
        for item in packages
        if isinstance(item, dict) and isinstance(item.get("id"), str)
    }
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        raise ValueError("cargo metadata response is missing resolve nodes")
    nodes_by_id = {
        item.get("id"): item
        for item in nodes
        if isinstance(item, dict) and isinstance(item.get("id"), str)
    }
    unresolved = list(workspace_members)
    expanded: set[str] = set()
    resolved_ids: set[str] = set()
    direct_ids: set[str] = set()
    while unresolved:
        package_id = unresolved.pop()
        if package_id in expanded:
            continue
        node = nodes_by_id.get(package_id)
        if node is None:
            if package_id in workspace_members:
                raise ValueError(f"workspace member missing from cargo metadata resolution: {package_id}")
            continue
        expanded.add(package_id)
        dependencies = node.get("deps")
        if not isinstance(dependencies, list):
            raise ValueError(f"resolved Rust package missing dependency data: {package_id}")
        for dependency in dependencies:
            if not isinstance(dependency, dict):
                raise ValueError("invalid cargo metadata dependency edge")
            dependency_id = dependency.get("pkg")
            if not isinstance(dependency_id, str):
                raise ValueError("invalid cargo metadata dependency edge")
            if package_id in workspace_members:
                direct_ids.add(dependency_id)
            resolved_ids.add(dependency_id)
            unresolved.append(dependency_id)
    resolved: dict[str, Package] = {}
    for package_id in resolved_ids:
        package = packages_by_id.get(package_id)
        if package is None:
            kind = "direct" if package_id in direct_ids else "resolved"
            raise ValueError(f"{kind} Rust package missing from cargo metadata: {package_id}")
        if not str(package.get("source", "")).startswith("registry+"):
            continue
        name, version = package.get("name"), package.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            raise ValueError(f"invalid cargo metadata package: {package_id}")
        Version.parse(version)
        resolved[package_id] = Package("Rust", name, version)
    if not resolved:
        raise ValueError("cargo metadata did not contain resolved registry Rust dependencies")
    return sorted(resolved.values(), key=lambda package: (package.name, Version.parse(package.current)))


def _load_manifest_payload(root: Path, manifest: Path) -> dict[str, object]:
    try:
        payload = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"failed to read Rust manifest {manifest}: {exc}") from exc
    return _string_keyed_object(payload, f"Rust manifest must be a table: {manifest}")


def _verified_skia_hold(root: Path, metadata: dict[str, object]) -> bool:
    """認証されたmacOS依存だけを性能上の保留候補にする。"""
    try:
        runtime, node = _runtime_metadata(metadata)
    except ValueError:
        return False
    manifest = runtime.get("manifest_path")
    if not isinstance(manifest, str) or Path(manifest).resolve() != (root / KRR_SKRIFA_MANIFEST).resolve():
        return False
    dependencies, edges, packages = runtime.get("dependencies"), node.get("deps"), metadata.get("packages")
    if not isinstance(dependencies, list) or not isinstance(edges, list) or not isinstance(packages, list):
        return False
    target = 'cfg(target_os = "macos")'
    registry = "registry+https://github.com/rust-lang/crates.io-index"
    direct = [d for d in dependencies if isinstance(d, dict) and d.get("name") == "skia-safe"
              and d.get("rename") is None and d.get("kind") is None and d.get("target") == target]
    resolved = [d for d in edges if isinstance(d, dict) and d.get("name") == "skia_safe"
                and d.get("dep_kinds") == [{"kind": None, "target": target}]]
    if len(direct) != 1 or len(resolved) != 1:
        return False
    if direct[0].get("req") != "=0.156.0" or direct[0].get("source") != registry:
        return False
    matches = [p for p in packages if isinstance(p, dict) and p.get("id") == resolved[0].get("pkg")]
    return (len(matches) == 1 and matches[0].get("name") == "skia-safe"
            and matches[0].get("version") == "0.156.0" and matches[0].get("source") == registry)


def _verified_ast_lint_hold(root: Path, metadata: dict[str, object]) -> bool:
    """既知の検査回帰に対し、実際のlinter依存だけを保留する。"""
    packages, members, resolve = metadata.get("packages"), metadata.get("workspace_members"), metadata.get("resolve")
    if not isinstance(packages, list) or not isinstance(members, list) or not isinstance(resolve, dict):
        return False
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        return False
    workspace = [p for p in packages if isinstance(p, dict) and p.get("id") in members]
    if len(workspace) != len(members) or any(not isinstance(p.get("dependencies"), list) for p in workspace):
        return False
    owners = [p for p in workspace if any(isinstance(d, dict) and d.get("name") == "katana-ast-lint"
                                           for d in p["dependencies"])]
    if len(owners) != 1:
        return False
    owner = owners[0]
    manifest = owner.get("manifest_path")
    if (owner.get("name") != "kdr-linter" or owner.get("source") is not None
            or not isinstance(manifest, str)
            or Path(manifest).resolve() != (root / "crates/kdr-linter/Cargo.toml").resolve()):
        return False
    declaration = _load_manifest_payload(root, Path(manifest)).get("dependencies")
    if not isinstance(declaration, dict) or declaration.get("katana-ast-lint") != {"workspace": True}:
        return False
    registry = "registry+https://github.com/rust-lang/crates.io-index"
    direct = [d for d in owner["dependencies"] if isinstance(d, dict) and d.get("name") == "katana-ast-lint"]
    if (len(direct) != 1 or direct[0].get("rename") is not None or direct[0].get("kind") is not None
            or direct[0].get("target") is not None or direct[0].get("req") != "=0.5.3"
            or direct[0].get("source") != registry):
        return False
    owner_nodes = [n for n in nodes if isinstance(n, dict) and n.get("id") == owner.get("id")]
    if len(owner_nodes) != 1 or not isinstance(owner_nodes[0].get("deps"), list):
        return False
    edges = [e for e in owner_nodes[0]["deps"] if isinstance(e, dict) and e.get("name") == "katana_ast_lint"]
    if len(edges) != 1 or edges[0].get("dep_kinds") != [{"kind": None, "target": None}]:
        return False
    resolved = [p for p in packages if isinstance(p, dict) and p.get("id") == edges[0].get("pkg")]
    return (len(resolved) == 1 and resolved[0].get("name") == "katana-ast-lint"
            and resolved[0].get("version") == "0.5.3" and resolved[0].get("source") == registry)


def rust_manifest_dependencies(root: Path, coupled_skrifa_manifest: Path | None = None,
                               metadata: dict[str, object] | None = None) -> list[Package]:
    """Return direct registry requirements updated by ``cargo upgrade``."""
    dependencies_by_version: dict[tuple[str, str, int | None, str | None, str | None, bool, bool], Package] = {}

    def dependency_tables(payload: dict[str, object]) -> list[tuple[str, object]]:
        sections = ("dependencies", "dev-dependencies", "build-dependencies")
        tables = [(section, payload.get(section)) for section in sections]
        target = payload.get("target")
        if isinstance(target, dict):
            for target_name, target_config in target.items():
                if not isinstance(target_config, dict):
                    continue
                tables.extend((f"target.{target_name}.{section}", target_config.get(section)) for section in sections)
        workspace = payload.get("workspace")
        if isinstance(workspace, dict):
            tables.append(("workspace.dependencies", workspace.get("dependencies")))
        return tables

    coupled_manifest = coupled_skrifa_manifest.resolve() if coupled_skrifa_manifest is not None else None
    for manifest in release_cargo_manifests(root):
        payload = _load_manifest_payload(root, manifest)
        for scope, dependencies in dependency_tables(payload):
            if not isinstance(dependencies, dict):
                continue
            for declared_name, declaration in dependencies.items():
                if not isinstance(declared_name, str):
                    raise ValueError(f"invalid Rust dependency name in {manifest}")
                if (
                    coupled_manifest is not None
                    and manifest.resolve() == coupled_manifest
                    and scope == "dependencies"
                    and declared_name in {"skrifa", KRR_SVG_HARFRUST_ALIAS}
                ):
                    continue
                version: object | None = declaration
                package_name = declared_name
                if isinstance(declaration, dict):
                    if "path" in declaration or "git" in declaration or declaration.get("workspace") is True:
                        continue
                    # Cargo resolves named alternate registries through their
                    # configured index.  crates.io's API cannot establish
                    # freshness for the same package name there, so leave
                    # this requirement to the Cargo lockfile resolver below.
                    if "registry" in declaration:
                        registry = declaration.get("registry")
                        if not isinstance(registry, str) or not registry:
                            raise ValueError(f"invalid Rust dependency registry in {manifest}: {declared_name}")
                        continue
                    version = declaration.get("version")
                    package = declaration.get("package")
                    if package is not None:
                        if not isinstance(package, str):
                            raise ValueError(f"invalid Rust dependency package name in {manifest}: {declared_name}")
                        package_name = package
                if not isinstance(version, str):
                    continue
                try:
                    package = cargo_requirement_package(package_name, version)
                except ValueError as exc:
                    raise ValueError(
                        f"unsupported Cargo version requirement in {manifest}: {declared_name} = {version!r}"
                    ) from exc
                if (metadata is not None and manifest.resolve() == (root / KRR_SKRIFA_MANIFEST).resolve()
                        and scope == 'target.cfg(target_os = "macos").dependencies'
                        and declared_name == "skia-safe"
                        and declaration == {"version": "=0.156.0", "default-features": False,
                                            "features": ["binary-cache", "pdf"]}
                        and _verified_skia_hold(root, metadata)):
                    package = replace(package, skia_hold_candidate=True)
                if (metadata is not None and manifest.resolve() == (root / "Cargo.toml").resolve()
                        and scope == "workspace.dependencies" and declared_name == "katana-ast-lint"
                        and declaration == "=0.5.3" and _verified_ast_lint_hold(root, metadata)):
                    package = replace(package, ast_lint_hold_candidate=True)
                dependencies_by_version[
                    (
                        package.name, package.current, package.incompatible_prefix_length,
                        package.requirement_operator, package.cargo_requirement, package.skia_hold_candidate,
                        package.ast_lint_hold_candidate,
                    )
                ] = package
    return sorted(dependencies_by_version.values(), key=lambda package: (package.name, Version.parse(package.current)))


def rust_alternate_registry_dependencies(root: Path) -> list[AlternateRegistryDependency]:
    """Return direct requirements resolved through named Cargo registries."""
    dependencies: dict[tuple[str, str, str, int | None, str | None, str | None], AlternateRegistryDependency] = {}
    for manifest in release_cargo_manifests(root):
        payload = _load_manifest_payload(root, manifest)
        sections = ("dependencies", "dev-dependencies", "build-dependencies")
        tables: list[object] = [payload.get(section) for section in sections]
        target = payload.get("target")
        if isinstance(target, dict):
            for target_config in target.values():
                if isinstance(target_config, dict):
                    tables.extend(target_config.get(section) for section in sections)
        workspace = payload.get("workspace")
        if isinstance(workspace, dict):
            tables.append(workspace.get("dependencies"))
        for declarations in tables:
            if not isinstance(declarations, dict):
                continue
            for declared_name, declaration in declarations.items():
                if not isinstance(declared_name, str) or not isinstance(declaration, dict):
                    continue
                registry = declaration.get("registry")
                if registry is None:
                    continue
                if not isinstance(registry, str) or not registry:
                    raise ValueError(f"invalid Rust dependency registry in {manifest}: {declared_name}")
                version = declaration.get("version")
                if not isinstance(version, str):
                    continue
                package_name = declaration.get("package", declared_name)
                if not isinstance(package_name, str):
                    raise ValueError(f"invalid Rust dependency package name in {manifest}: {declared_name}")
                try:
                    package = cargo_requirement_package(package_name, version)
                except ValueError as exc:
                    raise ValueError(
                        f"unsupported Cargo version requirement in {manifest}: {declared_name} = {version!r}"
                    ) from exc
                key = (
                    registry, package.name, package.current, package.incompatible_prefix_length,
                    package.requirement_operator, package.cargo_requirement,
                )
                dependencies[key] = AlternateRegistryDependency(package, registry)
    return sorted(dependencies.values(), key=lambda dependency: (dependency.registry, dependency.package.name, Version.parse(dependency.package.current)))


def cargo_registry_latest(root: Path, dependency: AlternateRegistryDependency) -> str:
    """Query a Cargo registry without exposing its response or credentials."""
    registry_option = "--index" if "://" in dependency.registry else "--registry"
    completed = subprocess.run(
        [
            "cargo", "search", registry_option, dependency.registry,
            "--limit", "1", "--color", "never", dependency.package.name,
        ],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        timeout=120,
    )
    if completed.returncode != 0:
        raise ValueError(f"Cargo registry query failed for {dependency.registry}:{dependency.package.name}")
    pattern = rf"(?m)^{re.escape(dependency.package.name)}\s*=\s*\"([^\"]+)\"(?:\s|$)"
    match = re.search(pattern, completed.stdout)
    if match is None:
        raise ValueError(f"Cargo registry query returned no exact package for {dependency.registry}:{dependency.package.name}")
    try:
        Version.parse(match.group(1))
    except ValueError as exc:
        raise ValueError(f"Cargo registry query returned an invalid version for {dependency.registry}:{dependency.package.name}") from exc
    return match.group(1)


def rust_alternate_registries_are_fresh(root: Path, dependencies: list[AlternateRegistryDependency]) -> bool:
    """Apply ``cargo upgrade --incompatible allow`` semantics per named registry."""
    return all(not is_stale(dependency.package, cargo_registry_latest(root, dependency)) for dependency in dependencies)


def rust_lock_is_latest_compatible(root: Path) -> bool:
    """Use Cargo's resolver, with exact immutable-parent retentions.

    Registry ``newest_version`` can be a prerelease or a major version outside
    the workspace's semver requirements.  Cargo is the authority for the
    highest compatible resolution, including transitive dependencies. The
    pinned utf8_iter and crc32fast transitions are accepted only as known diffs.
    """
    completed = subprocess.run(
        # リリース workflow は CARGO_TERM_COLOR=always を設定するため、Locking 件数を
        # 安定して判定できるよう Cargo 出力を明示的に無色化する。
        ["cargo", "update", "--dry-run", "--color=never", "--manifest-path", str(root / "Cargo.toml")],
        check=False,
        capture_output=True,
        text=True,
        timeout=120,
    )
    if completed.returncode != 0:
        raise ValueError(f"cargo update dry-run failed: {completed.stderr.strip()}")
    output = f"{completed.stdout}\n{completed.stderr}"
    # WHY: Cargoの要約文言変更に依存せず、件数と実際の変更packageを照合する。
    changes = _cargo_lock_changes(output)
    return changes in (
        set(), UTF8_ITER_RETENTION_CHANGES, CRC32FAST_RETENTION_CHANGES,
        UTF8_ITER_RETENTION_CHANGES | CRC32FAST_RETENTION_CHANGES,
    )


def _cargo_lock_changes(output: str) -> set[tuple[str, str, str, str | None]] | None:
    match = LOCKING_RE.search(output)
    changes: set[tuple[str, str, str, str | None]] = set()
    for line in output.splitlines():
        if line.strip() == "Updating crates.io index":
            continue
        change = LOCK_CHANGE_RE.match(line)
        if change is not None:
            action, package, current, target = change.groups()
            changes.add((action, package, current, target))
        elif re.match(r"^\s+(Adding|Updating|Removing|Downgrading)\b", line):
            return None
    if match is None:
        return changes if not changes else None
    if int(match.group(1)) != len(changes):
        return None
    return changes


def verify_current_utf8_iter_retention(root: Path) -> None:
    paths = (root / "Cargo.lock", root / "scripts/html/performance-harness/Cargo.lock")
    if any(path.is_symlink() or not path.is_file() for path in paths):
        raise ValueError("root or HTML harness Cargo.lock is unavailable")
    parent_lock = load_pinned_parent_lock(root)
    root_lock, harness_lock = paths[0].read_bytes(), paths[1].read_bytes()
    verify_retained_packages(parent_lock, (root_lock, harness_lock), retention_plan(parent_lock))
    verify_utf8_iter_retention(root_lock, harness_lock, parent_lock)


def release_cargo_manifests(root: Path) -> list[Path]:
    """List product manifests while excluding generated and diagnostic trees."""
    return [
        manifest
        for manifest in sorted(root.glob("**/Cargo.toml"))
        if not any(part in NON_RELEASE_CARGO_MANIFEST_PARTS for part in manifest.relative_to(root).parts)
    ]


def js_packages(root: Path) -> list[Package]:
    lock = load_bun_lock(root / "bun.lock")
    entries = lock.get("packages", {})
    if not isinstance(entries, dict):
        raise ValueError("bun.lock is missing package resolution data")
    packages: list[Package] = []
    for name in sorted(entries):
        if not isinstance(name, str):
            raise ValueError("invalid bun.lock package name")
        entry = entries.get(name)
        if not isinstance(entry, list) or not entry or not isinstance(entry[0], str):
            raise ValueError(f"resolved JavaScript package missing from bun.lock: {name}")
        resolved_name, separator, resolved_version = entry[0].rpartition("@")
        if not separator or not resolved_name or not resolved_version:
            raise ValueError(f"invalid bun.lock package entry: {name}")
        packages.append(Package("JavaScript", resolved_name, resolved_version))
    return packages


def js_lock_is_latest_compatible(root: Path) -> bool:
    """Use Bun's resolver for direct and transitive JavaScript freshness."""
    package = root / "package.json"
    lock = root / "bun.lock"
    with tempfile.TemporaryDirectory() as temporary_directory:
        candidate = Path(temporary_directory)
        for source in (package, lock):
            shutil.copy2(source, candidate / source.name)
        completed = subprocess.run(
            ["bun", "update", "--latest"], cwd=candidate, check=False,
            capture_output=True, text=True,
            timeout=120,
        )
        if completed.returncode != 0:
            raise ValueError(f"bun update failed: {completed.stderr.strip()}")
        return all(
            source.read_bytes() == (candidate / source.name).read_bytes()
            for source in (package, lock)
        )


def runtime_assets(root: Path) -> list[Package]:
    catalog = (root / RUNTIME_CATALOG).resolve(strict=True)
    cli = RUNTIME_CATALOG_CLI.resolve(strict=True)
    try:
        completed = subprocess.run(
            ["bun", "run", str(cli), "--freshness-catalog", str(catalog)],
            # Bun/TypeScript を所有するリポジトリからCLIを起動し、fixture側の
            # cwdによってAST compilerの解決先が変わらないようにする。
            cwd=RUNTIME_CATALOG_CLI.resolve().parents[2],
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except OSError as exc:
        raise ValueError("runtime asset catalog AST CLI failed to start") from exc
    if completed.returncode != 0:
        detail = completed.stderr.strip()
        suffix = f": {detail}" if detail else ""
        raise ValueError(f"runtime asset catalog AST CLI exited with {completed.returncode}{suffix}")

    def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("runtime asset catalog AST CLI returned duplicate JSON properties")
            result[key] = value
        return result

    payload = json.loads(completed.stdout, object_pairs_hook=unique_object)
    if not isinstance(payload, list) or not payload:
        raise ValueError("runtime asset catalog AST CLI must return a non-empty JSON array")
    assets: list[Package] = []
    seen_kinds: set[str] = set()
    required_keys = {"kind", "version", "latestUrl"}
    for item in payload:
        if not isinstance(item, dict) or set(item) != required_keys:
            raise ValueError("runtime asset catalog AST CLI returned an invalid entry schema")
        kind, version, latest_url = item["kind"], item["version"], item["latestUrl"]
        if (
            not isinstance(kind, str) or not kind.strip()
            or not isinstance(version, str) or not version.strip()
            or not isinstance(latest_url, str) or not latest_url.strip()
        ):
            raise ValueError("runtime asset catalog AST CLI returned an empty or non-string entry value")
        if kind in seen_kinds:
            raise ValueError("runtime asset catalog AST CLI returned duplicate asset kinds")
        seen_kinds.add(kind)
        assets.append(Package("Runtime asset", f"{kind}|{latest_url}", version))
    return assets


def fetch(url: str) -> bytes:
    headers = {"Accept": "application/json", "User-Agent": "katana-render-runtime-release-freshness"}
    parsed_url = parse.urlsplit(url)
    if (
        parsed_url.scheme == "https"
        and parsed_url.hostname == "api.github.com"
        and parsed_url.port in (None, 443)
    ):
        token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
        if token:
            # ヘッダー例外へ秘密値が混入しないよう、可視 ASCII 以外を送信前に拒否する。
            if any(not 0x21 <= ord(character) <= 0x7E for character in token):
                raise ValueError("GitHub API token contains invalid header characters")
            headers["Authorization"] = f"Bearer {token}"
    try:
        opener = request.build_opener(_StripAuthorizationRedirectHandler())
        with opener.open(request.Request(url, headers=headers), timeout=TIMEOUT_SECONDS) as response:
            if not 200 <= response.status < 300:
                raise ValueError(f"HTTP {response.status}")
            body = response.read(MAX_RESPONSE_BYTES + 1)
    except (OSError, error.URLError, error.HTTPError) as exc:
        raise ValueError(f"request failed for {url}: {exc}") from exc
    if not isinstance(body, bytes):
        raise ValueError(f"response body is not bytes: {url}")
    if len(body) > MAX_RESPONSE_BYTES:
        raise ValueError(f"response exceeds {MAX_RESPONSE_BYTES} bytes: {url}")
    return body


def latest(package: Package) -> str:
    if package.ecosystem == "Rust":
        payload = json.loads(fetch(f"https://crates.io/api/v1/crates/{parse.quote(package.name, safe='')}").decode("utf-8"))
        if not isinstance(payload, dict) or not isinstance(payload.get("crate"), dict):
            raise ValueError(f"latest version response must be a JSON object: {package.name}")
        value = payload["crate"].get("newest_version")
    elif package.ecosystem == "JavaScript":
        payload = json.loads(fetch(f"https://registry.npmjs.org/{parse.quote(package.name, safe='@')}/latest").decode("utf-8"))
        if not isinstance(payload, dict):
            raise ValueError(f"latest version response must be a JSON object: {package.name}")
        value = payload.get("version")
    elif package.ecosystem == "Rust manifest dependency":
        payload = json.loads(fetch(f"https://crates.io/api/v1/crates/{parse.quote(package.name, safe='')}").decode("utf-8"))
        if not isinstance(payload, dict) or not isinstance(payload.get("crate"), dict):
            raise ValueError(f"latest version response must be a JSON object: {package.name}")
        value = payload["crate"].get("newest_version")
    else:
        kind, url = package.name.split("|", maxsplit=1)
        body = fetch(url).decode("utf-8")
        if kind == "drawio":
            payload = json.loads(body)
            if not isinstance(payload, dict):
                raise ValueError(f"latest version response must be a JSON object: {package.name}")
            value = payload.get("tag_name", "").removeprefix("v")
        elif kind == "plantuml":
            versions = re.findall(r"<version>([^<]+)</version>", body)
            value = max(versions, key=Version.parse, default="")
        else:
            payload = json.loads(body)
            if not isinstance(payload, dict):
                raise ValueError(f"latest version response must be a JSON object: {package.name}")
            value = payload.get("version")
    if not isinstance(value, str):
        raise ValueError(f"latest version missing for {package.ecosystem} dependency {package.name}")
    Version.parse(value)
    return value


def display_name(package: Package) -> str:
    return package.name.split("|", maxsplit=1)[0]


def is_stale(package: Package, resolved_version: str) -> bool:
    # Issue #109: R7924 0.157 RSS +0.125 MiB; R7940 0.156 +0.015625 MiB.
    # 各25回の同一入力診断に基づく候補保留であり、全84条件の受入を意味しない。
    if package.skia_hold_candidate and resolved_version == "0.157.0":
        return False
    if package.ast_lint_hold_candidate and resolved_version == "0.5.4":
        return False
    current = Version.parse(package.current)
    resolved = Version.parse(resolved_version)
    if package.cargo_requirement is not None:
        return not cargo_requirement_matches(package.cargo_requirement, resolved)
    if package.incompatible_prefix_length is not None:
        length = package.incompatible_prefix_length
        return resolved.parts[:length] > current.parts[:length]
    return resolved > current


def main(argv: list[str] | None = None) -> int:
    args = parse_args() if argv is None else argparse.Namespace(root=Path(argv[0]))
    try:
        verify_current_utf8_iter_retention(args.root)
        metadata = cargo_metadata(args.root)
        rust = rust_packages(args.root, metadata)
        coupled_skrifa_manifest = verified_krr_usvg_skrifa_manifest(args.root, metadata)
        if not rust_lock_is_latest_compatible(args.root):
            print("Dependency freshness check rejected this release:", file=sys.stderr)
            print("Rust Cargo.lock is not at the latest compatible resolution.", file=sys.stderr)
            print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
            return 1
        if not rust_alternate_registries_are_fresh(args.root, rust_alternate_registry_dependencies(args.root)):
            print("Dependency freshness check rejected this release:", file=sys.stderr)
            print("A Cargo alternate-registry dependency has an incompatible update available.", file=sys.stderr)
            print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
            return 1
        rust_manifest = rust_manifest_dependencies(args.root, coupled_skrifa_manifest, metadata)
        javascript = js_packages(args.root)
        if not js_lock_is_latest_compatible(args.root):
            print("Dependency freshness check rejected this release:", file=sys.stderr)
            print("JavaScript bun.lock is not at the latest compatible resolution.", file=sys.stderr)
            print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
            return 1
        packages = runtime_assets(args.root)
        freshness_packages = [*rust_manifest, *packages]
        with ThreadPoolExecutor(max_workers=8) as executor:
            latest_versions = executor.map(
                latest,
                freshness_packages,
                timeout=TIMEOUT_SECONDS * len(freshness_packages),
            )
            resolved = [
                (freshness_packages[index], next(latest_versions))
                for index in range(len(freshness_packages))
            ]
    except (ValueError, TimeoutError, subprocess.TimeoutExpired, json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"Dependency freshness check failed closed: {exc}", file=sys.stderr)
        return 1
    stale = [f"{package.ecosystem} {display_name(package)}: {package.current} -> {resolved_version}" for package, resolved_version in resolved if is_stale(package, resolved_version)]
    if stale:
        print("Dependency freshness check rejected this release:", file=sys.stderr)
        print("\n".join(stale), file=sys.stderr)
        print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
        return 1
    print(
        f"Dependency freshness check passed ({len(rust) + len(javascript) + len(packages)} "
        "resolved dependencies and pinned runtime assets; utf8_iter 1.0.4 is retained to "
        "preserve the immutable HTML performance parent closure)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
