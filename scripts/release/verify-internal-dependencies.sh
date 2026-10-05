#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

python3 - "${version}" <<'PY'
from __future__ import annotations

import sys
import tomllib
from pathlib import Path


version = sys.argv[1]
root = Path.cwd()


def load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as manifest_file:
        value = tomllib.load(manifest_file)
    if not isinstance(value, dict):
        raise ValueError(f"invalid TOML manifest: {path}")
    return value


workspace = load_manifest(root / "Cargo.toml")
workspace_package = workspace.get("workspace", {}).get("package", {})
workspace_dependencies = workspace.get("workspace", {}).get("dependencies", {})
if workspace_package.get("version") != version:
    raise SystemExit(f"workspace package version must be {version}")

packages = {
    "katana-render-runtime-assets": root / "crates/katana-render-runtime-assets/Cargo.toml",
    "katana-render-runtime": root / "crates/katana-render-runtime/Cargo.toml",
    "katana-render-runtime-cli": root / "crates/katana-render-runtime-cli/Cargo.toml",
}
for package_name, manifest_path in packages.items():
    manifest = load_manifest(manifest_path)
    package = manifest.get("package", {})
    if package.get("name") != package_name:
        raise SystemExit(f"{manifest_path} must declare package {package_name}")
    if package.get("version") != {"workspace": True}:
        raise SystemExit(f"{package_name} must inherit the workspace package version")


def assert_workspace_dependency(
    package_name: str,
    dependency_name: str,
    expected_path: str,
    *,
    exact_version: bool = False,
) -> None:
    manifest = load_manifest(packages[package_name])
    dependency = manifest.get("dependencies", {}).get(dependency_name)
    if dependency != {"workspace": True}:
        raise SystemExit(f"{package_name} must depend on {dependency_name} from the workspace")

    workspace_dependency = workspace_dependencies.get(dependency_name)
    if not isinstance(workspace_dependency, dict):
        raise SystemExit(f"workspace dependency {dependency_name} must be a table")
    if workspace_dependency.get("path") != expected_path:
        raise SystemExit(f"workspace {dependency_name} dependency must point to {expected_path}")
    expected_version = f"={version}" if exact_version else version
    if workspace_dependency.get("version") != expected_version:
        qualifier = "exact version" if exact_version else "version"
        raise SystemExit(
            f"workspace {dependency_name} dependency must use {qualifier} {expected_version}"
        )


assert_workspace_dependency(
    "katana-render-runtime",
    "katana-render-runtime-assets",
    "crates/katana-render-runtime-assets",
    exact_version=True,
)
assert_workspace_dependency(
    "katana-render-runtime-cli",
    "katana-render-runtime",
    "crates/katana-render-runtime",
)
print(f"all three package versions and internal dependencies match {version}")
PY
