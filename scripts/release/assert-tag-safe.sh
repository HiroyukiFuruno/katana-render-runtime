#!/usr/bin/env bash
set -euo pipefail

tag="${1:?tag is required}"
remote="${2:-origin}"
remote_source="${remote}"

if [[ "${remote}" == "origin" && -n "${GITHUB_REPOSITORY:-}" ]]; then
  github_token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
  if [[ -n "${github_token}" ]]; then
    remote_source="https://x-access-token:${github_token}@github.com/${GITHUB_REPOSITORY}.git"
  fi
fi

local_exists=false
local_target=""
if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  local_exists=true
  local_target="$(git rev-parse "${tag}^{}")"
fi

remote_target="$(
  git ls-remote --tags "${remote_source}" "refs/tags/${tag}^{}" |
    awk 'NR == 1 { print $1 }'
)"
if [[ -z "${remote_target}" ]]; then
  remote_target="$(
    git ls-remote --tags "${remote_source}" "refs/tags/${tag}" |
      awk 'NR == 1 { print $1 }'
  )"
fi

if [[ -z "${remote_target}" ]]; then
  echo "${tag} does not exist on ${remote}; creating a new tag is safe."
  exit 0
fi

if [[ "${local_exists}" != "true" ]]; then
  echo "${tag} already exists on ${remote}; fetch it before retrying release-tag." >&2
  exit 1
fi

if [[ "${local_target}" != "${remote_target}" ]]; then
  echo "${tag} target differs from ${remote}; refusing to overwrite a released tag." >&2
  echo "local:  ${local_target}" >&2
  echo "remote: ${remote_target}" >&2
  exit 1
fi

echo "${tag} target matches ${remote}; no tag overwrite is required."
