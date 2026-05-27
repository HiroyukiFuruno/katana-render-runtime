#!/usr/bin/env bash
set -euo pipefail

tag="${1:?tag is required}"
remote="${2:-origin}"
attempt_limit="${RELEASE_TAG_PUSH_ATTEMPTS:-3}"
retry_delay_seconds="${RELEASE_TAG_PUSH_RETRY_DELAY_SECONDS:-5}"

if [[ ! "${attempt_limit}" =~ ^[1-9][0-9]*$ ]]; then
  echo "RELEASE_TAG_PUSH_ATTEMPTS must be a positive integer." >&2
  exit 1
fi

if [[ ! "${retry_delay_seconds}" =~ ^[0-9]+$ ]]; then
  echo "RELEASE_TAG_PUSH_RETRY_DELAY_SECONDS must be a non-negative integer." >&2
  exit 1
fi

remote_source="${remote}"
if [[ "${remote}" == "origin" && -n "${GITHUB_REPOSITORY:-}" ]]; then
  github_token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
  if [[ -n "${github_token}" ]]; then
    remote_source="https://x-access-token:${github_token}@github.com/${GITHUB_REPOSITORY}.git"
  fi
fi

expected_target="$(git rev-parse HEAD)"

remote_tag_target() {
  local peeled_target
  peeled_target="$(
    git ls-remote --tags "${remote_source}" "refs/tags/${tag}^{}" |
      awk 'NR == 1 { print $1 }'
  )"
  if [[ -n "${peeled_target}" ]]; then
    printf '%s\n' "${peeled_target}"
    return
  fi

  git ls-remote --tags "${remote_source}" "refs/tags/${tag}" |
    awk 'NR == 1 { print $1 }'
}

local_tag_target() {
  git rev-parse -q --verify "refs/tags/${tag}^{}" 2>/dev/null || true
}

ensure_remote_tag_is_usable() {
  local remote_target
  remote_target="$(remote_tag_target)"

  if [[ -z "${remote_target}" ]]; then
    return 1
  fi

  if [[ "${remote_target}" == "${expected_target}" ]]; then
    echo "Tag ${tag} already exists on ${remote} and points to HEAD."
    return 0
  fi

  echo "Tag ${tag} already exists on ${remote}, but it points to a different target." >&2
  echo "expected: ${expected_target}" >&2
  echo "remote:   ${remote_target}" >&2
  exit 1
}

ensure_local_tag() {
  local local_target
  local_target="$(local_tag_target)"

  if [[ -n "${local_target}" ]]; then
    if [[ "${local_target}" != "${expected_target}" ]]; then
      echo "Local tag ${tag} points to a different target." >&2
      echo "expected: ${expected_target}" >&2
      echo "local:    ${local_target}" >&2
      exit 1
    fi
    return
  fi

  git tag -a "${tag}" -m "Release ${tag}"
}

if ensure_remote_tag_is_usable; then
  exit 0
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
ensure_local_tag

attempt=1
while [[ "${attempt}" -le "${attempt_limit}" ]]; do
  if git push "${remote}" "refs/tags/${tag}"; then
    exit 0
  fi

  if ensure_remote_tag_is_usable; then
    exit 0
  fi

  if [[ "${attempt}" -ge "${attempt_limit}" ]]; then
    break
  fi

  echo "Retrying tag push for ${tag} in ${retry_delay_seconds} seconds..." >&2
  sleep "${retry_delay_seconds}"
  attempt=$((attempt + 1))
done

echo "Failed to push release tag ${tag} after ${attempt_limit} attempts." >&2
exit 1
