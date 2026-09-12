#!/usr/bin/env bash
set -euo pipefail

repository_root="$(git rev-parse --show-toplevel)"
cd "${repository_root}"

# Hooks inherit the caller's repository environment.  The quality gate creates
# temporary Git repositories, so keeping it would redirect their fixtures into
# the branch that is being pushed.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
  GIT_CEILING_DIRECTORIES GIT_DISCOVERY_ACROSS_FILESYSTEM GIT_PREFIX

updates="$(mktemp)"
cleanup() {
  rm -f "${updates}"
}
trap cleanup EXIT

cat >"${updates}"
just check
if (($# > 0)); then
  verifier_arguments=(--remote "$1")
  if (($# > 1)); then
    verifier_arguments+=(--remote-url "$2")
  fi
  python3 scripts/hooks/verify_push_issue.py "${verifier_arguments[@]}" <"${updates}"
else
  python3 scripts/hooks/verify_push_issue.py <"${updates}"
fi
