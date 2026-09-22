#!/usr/bin/env python3
"""Reject releases whose resolved dependencies or pinned runtime assets are stale."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from functools import total_ordering
from pathlib import Path
from urllib import error, parse, request

TIMEOUT_SECONDS = 15
MAX_RESPONSE_BYTES = 1_000_000
RUNTIME_CATALOG = Path("scripts/runtime-assets/runtime-asset-common.ts")


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
        for left, right in zip(self.prerelease, other.prerelease):
            if left == right:
                continue
            left_kind, left_value = left
            right_kind, right_value = right
            if left_kind != right_kind:
                # SemVer では数値の prerelease identifier を非数値より前に置く。
                return left_kind < right_kind
            return left_value < right_value
        # それまで同一なら、identifier が短い prerelease の優先度を下げる。
        return len(self.prerelease) < len(other.prerelease)

    @classmethod
    def parse(cls, value: str) -> "Version":
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
        return cls(tuple(int(part) for part in match.group(1, 2, 3)), not identifiers, prerelease)


@dataclass(frozen=True)
class Package:
    ecosystem: str
    name: str
    current: str


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
    return json.loads("".join(result))


def cargo_metadata(root: Path) -> dict[str, object]:
    command = ["cargo", "metadata", "--format-version=1", "--all-features", "--locked", "--manifest-path", str(root / "Cargo.toml")]
    try:
        completed = subprocess.run(command, check=True, capture_output=True, text=True)
        payload = json.loads(completed.stdout)
    except (OSError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        raise ValueError(f"cargo metadata failed: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError("cargo metadata response must be a JSON object")
    return payload


def rust_packages(root: Path) -> list[Package]:
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
        prefix = f"{name}@"
        if not entry[0].startswith(prefix):
            raise ValueError(f"invalid bun.lock package entry: {name}")
        packages.append(Package("JavaScript", name, entry[0][len(prefix) :]))
    return packages


def runtime_assets(root: Path) -> list[Package]:
    source = (root / RUNTIME_CATALOG).read_text(encoding="utf-8")
    entries: list[str] = []
    depth, start = 0, 0
    quote: str | None = None
    escaped = False
    for index, character in enumerate(source):
        if quote is not None:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
            continue
        if character in ('"', "'", "`"):
            quote = character
        elif character == "{":
            if depth == 0:
                start = index + 1
            depth += 1
        elif character == "}":
            depth -= 1
            if depth < 0:
                raise ValueError("runtime asset catalog has unbalanced braces")
            if depth == 0:
                entries.append(source[start:index])
    if quote is not None or depth != 0:
        raise ValueError("runtime asset catalog has unbalanced syntax")
    assets: list[Package] = []
    for entry in entries:
        properties = {
            name: match.group("value")
            for name in ("kind", "version", "latestUrl")
            if (match := re.search(rf'\b{name}:\s*"(?P<value>[^"]+)"', entry)) is not None
        }
        if "kind" not in properties or "{" in properties["kind"]:
            continue
        if set(properties) != {"kind", "version", "latestUrl"}:
            raise ValueError("runtime asset catalog entry is missing a pinned asset property")
        assets.append(Package("Runtime asset", f"{properties['kind']}|{properties['latestUrl']}", properties["version"]))
    if not assets:
        raise ValueError("runtime asset catalog did not contain pinned assets")
    return assets


def fetch(url: str) -> bytes:
    headers = {"Accept": "application/json", "User-Agent": "katana-render-runtime-release-freshness"}
    try:
        with request.urlopen(request.Request(url, headers=headers), timeout=TIMEOUT_SECONDS) as response:
            if not 200 <= response.status < 300:
                raise ValueError(f"HTTP {response.status}")
            body = response.read(MAX_RESPONSE_BYTES + 1)
    except (OSError, error.URLError, error.HTTPError) as exc:
        raise ValueError(f"request failed for {url}: {exc}") from exc
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


def main(argv: list[str] | None = None) -> int:
    args = parse_args() if argv is None else argparse.Namespace(root=Path(argv[0]))
    try:
        packages = rust_packages(args.root) + js_packages(args.root) + runtime_assets(args.root)
        with ThreadPoolExecutor(max_workers=8) as executor:
            resolved = list(zip(packages, executor.map(latest, packages, timeout=TIMEOUT_SECONDS * len(packages))))
    except (ValueError, TimeoutError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"Dependency freshness check failed closed: {exc}", file=sys.stderr)
        return 1
    stale = [f"{package.ecosystem} {display_name(package)}: {package.current} -> {resolved_version}" for package, resolved_version in resolved if Version.parse(resolved_version) > Version.parse(package.current)]
    if stale:
        print("Dependency freshness check rejected this release:", file=sys.stderr)
        print("\n".join(stale), file=sys.stderr)
        print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
        return 1
    print(f"Dependency freshness check passed ({len(packages)} resolved dependencies and pinned runtime assets).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
