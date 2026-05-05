#!/usr/bin/env bash
set -euo pipefail

package="${1:-}"
version_input="${2:-}"
max_bytes="${3:-10000000}"

if [[ -z "${package}" ]]; then
  echo "package name is required." >&2
  exit 1
fi

version="$(
  bash "$(dirname "$0")/verify-version.sh" "${version_input}" \
    | awk -F= '$1 == "version_bare" { print $2 }'
)"
crate_path="target/package/${package}-${version}.crate"

if [[ ! -f "${crate_path}" ]]; then
  echo "crate package not found: ${crate_path}" >&2
  exit 1
fi

size_bytes="$(wc -c < "${crate_path}" | tr -d '[:space:]')"
if [[ "${size_bytes}" -gt "${max_bytes}" ]]; then
  echo "${crate_path} is ${size_bytes} bytes; max is ${max_bytes} bytes." >&2
  exit 1
fi

echo "${crate_path} size ${size_bytes}/${max_bytes} bytes"
