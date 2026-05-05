#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

dependency_line="$(grep '^katana-canvas-forge = ' crates/katana-canvas-forge-cli/Cargo.toml)"
if [[ "${dependency_line}" != *"version = \"${version}\""* ]]; then
  echo "katana-canvas-forge-cli must depend on katana-canvas-forge version ${version}" >&2
  exit 1
fi

echo "internal dependency versions match ${version}"
