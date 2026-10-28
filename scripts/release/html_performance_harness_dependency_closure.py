"""Strict comparison of the HTML harness's non-SDK Cargo.lock closure."""
from __future__ import annotations

import hashlib
import re
import tomllib
from collections.abc import Mapping

PackageId = tuple[str, str, str | None]
PackageRecord = tuple[str | None, tuple[PackageId, ...]]
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_SDK_PACKAGE = "katana-render-runtime"


def _package_id(record: Mapping[str, object]) -> PackageId:
    name, version = record.get("name"), record.get("version")
    source = record.get("source")
    if (not isinstance(name, str) or not name or not isinstance(version, str) or not version
            or (source is not None and (not isinstance(source, str) or not source))):
        raise ValueError("Cargo.lock package identity is invalid")
    checksum = record.get("checksum")
    if checksum is not None and (not isinstance(checksum, str) or not _SHA256.fullmatch(checksum)):
        raise ValueError("Cargo.lock package checksum is invalid")
    if source is not None and source.startswith("registry+") and checksum is None:
        raise ValueError("Cargo.lock registry package checksum is missing")
    return name, version, source


def _dependency_ref(value: object) -> tuple[str, str | None, str | None]:
    if not isinstance(value, str) or not value:
        raise ValueError("Cargo.lock dependency reference is invalid")
    parts = value.split()
    if len(parts) == 1:
        return parts[0], None, None
    if len(parts) == 2 and parts[1].startswith("(") and parts[1].endswith(")"):
        return parts[0], None, parts[1][1:-1]
    if len(parts) == 2 and parts[1][0].isdigit():
        return parts[0], parts[1], None
    if (len(parts) == 3 and parts[1][0].isdigit()
            and parts[2].startswith("(") and parts[2].endswith(")")):
        return parts[0], parts[1], parts[2][1:-1]
    raise ValueError("Cargo.lock dependency reference is ambiguous")


def _raw_packages(lock_bytes: bytes) -> dict[PackageId, tuple[str | None, list[object]]]:
    try:
        document = tomllib.loads(lock_bytes.decode("utf8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ValueError("Cargo.lock is not valid UTF-8 TOML") from error
    if document.get("version") not in (3, 4):
        raise ValueError("Cargo.lock format version is unsupported")
    packages = document.get("package")
    if not isinstance(packages, list) or not packages:
        raise ValueError("Cargo.lock package table is missing")
    raw: dict[PackageId, tuple[str | None, list[object]]] = {}
    versions: set[tuple[str, str]] = set()
    for package in packages:
        if not isinstance(package, dict):
            raise ValueError("Cargo.lock package record is invalid")
        identity = _package_id(package)
        version_key = identity[0], identity[1]
        if version_key in versions:
            raise ValueError("Cargo.lock has multiple identities for one package version")
        versions.add(version_key)
        checksum_value = package.get("checksum")
        if checksum_value is not None and not isinstance(checksum_value, str):
            raise ValueError("Cargo.lock package checksum is invalid")
        checksum: str | None = checksum_value
        dependencies = package.get("dependencies", [])
        if not isinstance(dependencies, list):
            raise ValueError("Cargo.lock dependencies must be an array")
        if identity in raw:
            raise ValueError("Cargo.lock has duplicate package identities")
        raw[identity] = (checksum, dependencies)
    return raw


def _name_index(raw: Mapping[PackageId, object]) -> dict[str, list[PackageId]]:
    by_name: dict[str, list[PackageId]] = {}
    for identity in raw:
        by_name.setdefault(identity[0], []).append(identity)
    return by_name


def _resolved_graph(
    raw: Mapping[PackageId, tuple[str | None, list[object]]],
    by_name: Mapping[str, list[PackageId]],
) -> dict[PackageId, PackageRecord]:
    graph: dict[PackageId, PackageRecord] = {}
    for identity, (checksum, references) in raw.items():
        edges: list[PackageId] = []
        for reference in references:
            name, version, source = _dependency_ref(reference)
            matches = [candidate for candidate in by_name.get(name, [])
                       if (version is None or candidate[1] == version)
                       and (source is None or candidate[2] == source)]
            if len(matches) != 1:
                raise ValueError("Cargo.lock dependency does not resolve uniquely: " + str(reference))
            edges.append(matches[0])
        if len(set(edges)) != len(edges):
            raise ValueError("Cargo.lock has duplicate dependency edges")
        graph[identity] = (checksum, tuple(sorted(edges)))
    return graph


def _graph(lock_bytes: bytes) -> tuple[dict[PackageId, PackageRecord], PackageId]:
    raw = _raw_packages(lock_bytes)
    graph = _resolved_graph(raw, _name_index(raw))
    harnesses = [identity for identity in graph if identity[0] == "krr-html-performance-harness"]
    if len(harnesses) != 1:
        raise ValueError("Cargo.lock must contain one harness package identity")
    return graph, harnesses[0]


def _harness_closure(
    lock_bytes: bytes,
) -> tuple[PackageId, tuple[PackageId, ...], dict[PackageId, PackageRecord]]:
    graph, harness = _graph(lock_bytes)
    roots = tuple(sorted(edge for edge in graph[harness][1] if edge[0] != _SDK_PACKAGE))
    pending = list(roots)
    closure: dict[PackageId, PackageRecord] = {}
    while pending:
        identity = pending.pop()
        if identity[0] == _SDK_PACKAGE or identity in closure:
            continue
        record = graph.get(identity)
        if record is None:
            raise ValueError("Cargo.lock closure references a missing package")
        if any(edge[0] == _SDK_PACKAGE for edge in record[1]):
            raise ValueError("non-SDK harness dependency edge reaches the SDK package")
        closure[identity] = record
        pending.extend(closure[identity][1])
    if not closure:
        raise ValueError("Cargo.lock harness dependency closure is empty")
    return harness, roots, closure


def verify_harness_closure(parent_lock: bytes, current_lock: bytes, expected_parent_sha256: str) -> None:
    """Require the pinned parent lock and candidate to have identical non-SDK closure."""
    if not isinstance(expected_parent_sha256, str) or not _SHA256.fullmatch(expected_parent_sha256):
        raise ValueError("parent Cargo.lock pin is invalid")
    if hashlib.sha256(parent_lock).hexdigest() != expected_parent_sha256:
        raise ValueError("parent Cargo.lock bytes differ from immutable pin")
    if _harness_closure(parent_lock) != _harness_closure(current_lock):
        raise ValueError("HTML harness non-SDK Cargo dependency closure differs")
