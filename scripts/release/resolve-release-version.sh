#!/usr/bin/env bash
set -euo pipefail

event_name="${1:?event name is required}"
input_version="${2:-}"
pr_head_ref="${3:-}"

if [[ "${event_name}" == "pull_request" ]]; then
  if [[ ! "${pr_head_ref}" =~ ^release/v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Release branch must be exactly release/vX.Y.Z: ${pr_head_ref}" >&2
    exit 1
  fi

  bash scripts/release/verify-version.sh "${pr_head_ref#release/}"
elif [[ -n "${input_version}" ]]; then
  bash scripts/release/verify-version.sh "${input_version}"
else
  bash scripts/release/verify-version.sh
fi
