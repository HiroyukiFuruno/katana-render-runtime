#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"
packages=(katana-render-runtime-assets katana-render-runtime katana-render-runtime-cli)
allow_published_prefix="${ALLOW_PUBLISHED_PREFIX:-false}"
seen_unpublished=false

for package in "${packages[@]}"; do
  if output="$(cargo info "${package}@${version}" --registry crates-io 2>&1)"; then
    if [[ "${allow_published_prefix}" == "true" && "${seen_unpublished}" == "false" ]]; then
      echo "${package} ${version} is already published; release retry will skip it."
      continue
    fi
    echo "${package} ${version} is already published on crates.io." >&2
    exit 1
  elif [[ "${output}" == *"could not find"*"in registry"* ]]; then
    seen_unpublished=true
  else
    echo "Could not determine whether ${package} ${version} is published: ${output}" >&2
    exit 1
  fi
done

if [[ "${allow_published_prefix}" == "true" ]]; then
  echo "crates.io target versions form a valid published prefix"
else
  echo "crates.io target versions are unpublished"
fi
