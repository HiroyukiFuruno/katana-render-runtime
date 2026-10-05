#!/usr/bin/env bash
set -euo pipefail

version="$(bash "$(dirname "$0")/verify-version.sh" "${1:-}" | awk -F= '$1 == "version_bare" { print $2 }')"

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "CARGO_REGISTRY_TOKEN is required." >&2
  exit 1
fi

publish_attempts="${PUBLISH_ATTEMPTS:-3}"
publish_retry_delay_seconds="${PUBLISH_RETRY_DELAY_SECONDS:-10}"

# 再試行時は不変の release-source checkout を cwd にして実行する。
metadata="$(cargo metadata --no-deps --format-version 1)"
package_plan="$(python3 -c '
import json
import os
import re
import sys

version = sys.argv[1]
try:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError(f"invalid release version: {version}")
    parts = tuple(int(part) for part in version.split("."))
    metadata = json.load(sys.stdin)
    root = metadata["workspace_root"]
    packages = metadata["packages"]
    if not isinstance(root, str) or not os.path.isabs(root) or not isinstance(packages, list):
        raise ValueError("invalid Cargo metadata workspace")

    by_name = {}
    for package in packages:
        if not isinstance(package, dict):
            raise ValueError("invalid Cargo metadata package")
        name = package.get("name")
        if name in ("katana-render-runtime", "katana-render-runtime-assets", "katana-render-runtime-cli"):
            if name in by_name:
                raise ValueError(f"duplicate package identity: {name}")
            by_name[name] = package

    required = ["katana-render-runtime", "katana-render-runtime-cli"]
    if parts >= (0, 4, 23):
        required = ["katana-render-runtime-assets", *required]
    for name in required:
        package = by_name.get(name)
        if package is None:
            raise ValueError(f"required package missing from release source: {name}")
        if package.get("version") != version or package.get("source") is not None:
            raise ValueError(f"invalid package identity for {name}")
        expected = os.path.join(root, "crates", name, "Cargo.toml")
        if package.get("manifest_path") != expected or not os.path.isfile(expected):
            raise ValueError(f"invalid manifest path for {name}: {package.get('manifest_path')}")
    print("\n".join(required))
except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
    print(f"invalid release workspace metadata: {error}", file=sys.stderr)
    sys.exit(1)
' "${version}" <<<"${metadata}")"

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

while IFS= read -r package; do
  publish_if_needed "${package}"
  wait_for_crate "${package}"
done <<<"${package_plan}"
