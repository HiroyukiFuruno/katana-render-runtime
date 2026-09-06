#!/usr/bin/env bash
set -euo pipefail

repository_root="$(git rev-parse --show-toplevel)"
cd "${repository_root}"

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
