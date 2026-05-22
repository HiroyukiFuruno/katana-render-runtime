#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"
packages=(katana-render-runtime katana-diagram-renderer katana-diagram-renderer-cli)

for package in "${packages[@]}"; do
  if cargo info "${package}@${version}" --registry crates-io >/dev/null 2>&1; then
    echo "${package} ${version} is already published on crates.io." >&2
    exit 1
  fi
done

echo "crates.io target versions are unpublished"
