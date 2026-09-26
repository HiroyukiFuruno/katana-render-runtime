#!/usr/bin/env python3
"""Reject releases whose resolved dependencies or pinned runtime assets are stale."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
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
    incompatible_prefix_length: int | None = None
    requirement_operator: str | None = None


@dataclass(frozen=True)
class AlternateRegistryDependency:
    package: Package
    registry: str


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


def rust_manifest_dependencies(root: Path) -> list[Package]:
    """Return direct registry requirements updated by ``cargo upgrade``."""
    dependencies_by_version: dict[tuple[str, str, int | None, str | None], Package] = {}

    def dependency_tables(payload: dict[str, object]) -> list[object]:
        sections = ("dependencies", "dev-dependencies", "build-dependencies")
        tables = [payload.get(section) for section in sections]
        target = payload.get("target")
        if isinstance(target, dict):
            for target_config in target.values():
                if not isinstance(target_config, dict):
                    continue
                tables.extend(target_config.get(section) for section in sections)
        workspace = payload.get("workspace")
        if isinstance(workspace, dict):
            tables.append(workspace.get("dependencies"))
        return tables

    for manifest in sorted(root.glob("**/Cargo.toml")):
        if any(part in {"target", "vendor", ".git", ".worktrees"} for part in manifest.relative_to(root).parts):
            continue
        try:
            payload = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as exc:
            raise ValueError(f"failed to read Rust manifest {manifest}: {exc}") from exc
        if not isinstance(payload, dict):
            raise ValueError(f"Rust manifest must be a table: {manifest}")
        for dependencies in dependency_tables(payload):
            if not isinstance(dependencies, dict):
                continue
            for declared_name, declaration in dependencies.items():
                if not isinstance(declared_name, str):
                    raise ValueError(f"invalid Rust dependency name in {manifest}")
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
                match = re.fullmatch(r"(=|\^|~)?([0-9]+)(?:\.([0-9]+))?(?:\.([0-9]+))?", version)
                if match is None:
                    raise ValueError(f"unsupported Cargo version requirement in {manifest}: {declared_name} = {version!r}")
                operator = match.group(1)
                major = int(match.group(2))
                minor = match.group(3)
                patch = match.group(4)
                incompatible_prefix_length: int | None = None
                if operator == "~":
                    # Tilde requirements are limited to the specified
                    # minor; a requirement containing only a major advances
                    # at the next major.
                    incompatible_prefix_length = 1 if minor is None else 2
                elif operator == "^":
                    # Keep Cargo's caret compatibility for explicit caret
                    # requirements. Bare requirements retain their existing
                    # release freshness contract below.
                    if major > 0:
                        incompatible_prefix_length = 1
                    elif minor is None:
                        incompatible_prefix_length = 1
                    elif int(minor) > 0:
                        incompatible_prefix_length = 2
                    elif patch is not None:
                        incompatible_prefix_length = 3
                    else:
                        incompatible_prefix_length = 2
                elif minor is None:
                    incompatible_prefix_length = 1
                elif patch is None:
                    incompatible_prefix_length = 1 if major > 0 else 2
                # ``depends-update-all`` permits incompatible upgrades, so
                # broad requirements such as `1` must observe a new major
                # that Cargo's compatible lockfile resolver cannot see. Cargo
                # treats pre-1.0 `0.1` as compatible only within that minor.
                # Compatible updates remain Cargo's lockfile responsibility.
                declared = ".".join(
                    (match.group(2), minor or "0", patch or "0")
                )
                Version.parse(declared)
                dependencies_by_version[(package_name, declared, incompatible_prefix_length, operator)] = Package(
                    "Rust manifest dependency", package_name, declared, incompatible_prefix_length, operator
                )
    return sorted(dependencies_by_version.values(), key=lambda package: (package.name, Version.parse(package.current)))


def rust_alternate_registry_dependencies(root: Path) -> list[AlternateRegistryDependency]:
    """Return direct requirements resolved through named Cargo registries."""
    dependencies: dict[tuple[str, str, str, int | None, str | None], AlternateRegistryDependency] = {}
    for manifest in sorted(root.glob("**/Cargo.toml")):
        if any(part in {"target", "vendor", ".git", ".worktrees"} for part in manifest.relative_to(root).parts):
            continue
        try:
            payload = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as exc:
            raise ValueError(f"failed to read Rust manifest {manifest}: {exc}") from exc
        if not isinstance(payload, dict):
            raise ValueError(f"Rust manifest must be a table: {manifest}")
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
                match = re.fullmatch(r"(=|\^|~)?([0-9]+)(?:\.([0-9]+))?(?:\.([0-9]+))?", version)
                if match is None:
                    raise ValueError(f"unsupported Cargo version requirement in {manifest}: {declared_name} = {version!r}")
                operator, major_text, minor, patch = match.groups()
                major = int(major_text)
                if operator == "~":
                    prefix_length = 1 if minor is None else 2
                elif operator == "^":
                    prefix_length = 1 if major > 0 or minor is None else (2 if int(minor) > 0 or patch is None else 3)
                elif minor is None:
                    prefix_length = 1
                elif patch is None:
                    prefix_length = 1 if major > 0 else 2
                else:
                    prefix_length = None
                current = ".".join((major_text, minor or "0", patch or "0"))
                Version.parse(current)
                package = Package("Rust manifest dependency", package_name, current, prefix_length, operator)
                key = (registry, package.name, package.current, package.incompatible_prefix_length, package.requirement_operator)
                dependencies[key] = AlternateRegistryDependency(package, registry)
    return sorted(dependencies.values(), key=lambda dependency: (dependency.registry, dependency.package.name, Version.parse(dependency.package.current)))


def cargo_registry_latest(root: Path, dependency: AlternateRegistryDependency) -> str:
    """Query a named Cargo registry without exposing its response or credentials."""
    completed = subprocess.run(
        [
            "cargo", "search", "--registry", dependency.registry,
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
    """Use Cargo's resolver to validate direct and transitive lock freshness.

    Registry ``newest_version`` can be a prerelease or a major version outside
    the workspace's semver requirements.  Cargo is the authority for the
    highest compatible resolution, including transitive dependencies.
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
    # Cargo has changed the human-readable suffix over time (for example,
    # ``version`` versus ``versions`` and an optional Rust version).  The
    # resolver's only stable signal here is the number after ``Locking``;
    # keep the rest of the line out of the freshness contract.
    match = re.search(r"\bLocking\s+([0-9]+)\s+packages?\b", output)
    # Cargo omits the Locking line when the existing lockfile already resolves
    # every compatible dependency; a successful dry-run is then the no-update result.
    return match is None or match.group(1) == "0"


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
    current = Version.parse(package.current)
    resolved = Version.parse(resolved_version)
    if package.incompatible_prefix_length is not None:
        length = package.incompatible_prefix_length
        return resolved.parts[:length] > current.parts[:length]
    return resolved > current


def main(argv: list[str] | None = None) -> int:
    args = parse_args() if argv is None else argparse.Namespace(root=Path(argv[0]))
    try:
        rust = rust_packages(args.root)
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
        rust_manifest = rust_manifest_dependencies(args.root)
        javascript = js_packages(args.root)
        if not js_lock_is_latest_compatible(args.root):
            print("Dependency freshness check rejected this release:", file=sys.stderr)
            print("JavaScript bun.lock is not at the latest compatible resolution.", file=sys.stderr)
            print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
            return 1
        packages = runtime_assets(args.root)
        freshness_packages = [*rust_manifest, *packages]
        with ThreadPoolExecutor(max_workers=8) as executor:
            resolved = list(zip(freshness_packages, executor.map(latest, freshness_packages, timeout=TIMEOUT_SECONDS * len(freshness_packages))))
    except (ValueError, TimeoutError, subprocess.TimeoutExpired, json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"Dependency freshness check failed closed: {exc}", file=sys.stderr)
        return 1
    stale = [f"{package.ecosystem} {display_name(package)}: {package.current} -> {resolved_version}" for package, resolved_version in resolved if is_stale(package, resolved_version)]
    if stale:
        print("Dependency freshness check rejected this release:", file=sys.stderr)
        print("\n".join(stale), file=sys.stderr)
        print("Run `just depends-update-all`, review its generated changes, then rerun `just release-check`.", file=sys.stderr)
        return 1
    print(f"Dependency freshness check passed ({len(rust) + len(javascript) + len(packages)} resolved dependencies and pinned runtime assets).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
