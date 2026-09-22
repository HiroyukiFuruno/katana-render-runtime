#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "CARGO_REGISTRY_TOKEN is required." >&2
  exit 1
fi

publish_attempts="${PUBLISH_ATTEMPTS:-3}"
publish_retry_delay_seconds="${PUBLISH_RETRY_DELAY_SECONDS:-10}"

publish_if_needed() {
  local package="$1"
  local attempt
  local delay
  for attempt in $(seq 1 "${publish_attempts}"); do
    if cargo info "${package}@${version}" --registry crates-io >/dev/null 2>&1; then
      echo "${package} ${version} already published; skipping."
      return
    fi
    if cargo publish -p "${package}" --locked --token "${CARGO_REGISTRY_TOKEN}"; then
      return
    fi
    if [[ "${attempt}" == "${publish_attempts}" ]]; then
      echo "${package} ${version} publish failed after ${publish_attempts} attempts." >&2
      exit 1
    fi
    delay=$((publish_retry_delay_seconds * attempt))
    echo "${package} ${version} publish failed; retrying in ${delay}s (${attempt}/${publish_attempts})." >&2
    sleep "${delay}"
  done
}

wait_for_crate() {
  local package="$1"
  for _ in {1..30}; do
    if cargo info "${package}@${version}" --registry crates-io >/dev/null 2>&1; then
      return
    fi
    sleep 10
  done
  echo "${package} ${version} did not become visible on crates.io in time." >&2
  exit 1
}

publish_if_needed katana-render-runtime
wait_for_crate katana-render-runtime
publish_if_needed katana-render-runtime-cli
wait_for_crate katana-render-runtime-cli
