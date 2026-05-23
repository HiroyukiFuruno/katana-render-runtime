#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

cli_dependency_line="$(grep '^katana-render-runtime = ' crates/katana-render-runtime-cli/Cargo.toml)"
workspace_dependency_line="$(grep '^katana-render-runtime = ' Cargo.toml)"
expected_workspace_path='path = "crates/katana-render-runtime"'
expected_workspace_version="version = \"${version}\""
if [[ "${cli_dependency_line}" != *"workspace = true"* ]]; then
  echo "katana-render-runtime-cli must depend on katana-render-runtime from the workspace" >&2
  exit 1
fi
if [[ "${workspace_dependency_line}" != *"${expected_workspace_path}"* ]]; then
  echo "workspace katana-render-runtime dependency must point to crates/katana-render-runtime" >&2
  exit 1
fi
if [[ "${workspace_dependency_line}" != *"${expected_workspace_version}"* ]]; then
  echo "workspace katana-render-runtime dependency must use version ${version}" >&2
  exit 1
fi

echo "internal dependency versions match ${version}"
