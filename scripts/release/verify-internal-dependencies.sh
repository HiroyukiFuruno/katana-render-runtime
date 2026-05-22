#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

cli_dependency_line="$(grep '^katana-render-runtime = ' crates/katana-diagram-renderer-cli/Cargo.toml)"
wrapper_dependency_line="$(grep '^katana-render-runtime = ' crates/katana-diagram-renderer/Cargo.toml)"
if [[ "${cli_dependency_line}" != *"workspace = true"* ]]; then
  echo "katana-diagram-renderer-cli must depend on katana-render-runtime from the workspace" >&2
  exit 1
fi
if [[ "${wrapper_dependency_line}" != *"workspace = true"* ]]; then
  echo "katana-diagram-renderer wrapper must depend on katana-render-runtime from the workspace" >&2
  exit 1
fi

echo "internal dependency versions match ${version}"
