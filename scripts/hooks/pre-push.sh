#!/usr/bin/env bash
set -euo pipefail

repository_root="$(git rev-parse --show-toplevel)"
cd "${repository_root}"

# macOS Git hooks can inherit /usr/bin ahead of Homebrew; the release gates require tomllib.
python3_path="$(command -v python3 || true)"
if [[ "${python3_path}" == "/usr/bin/python3" ]]; then
  for homebrew_bin in /opt/homebrew/bin /usr/local/bin; do
    if [[ -x "${homebrew_bin}/python3" ]]; then
      export PATH="${homebrew_bin}:${PATH}"
      break
    fi
  done
fi

# Hooks inherit the caller's repository environment.  The quality gate creates
# temporary Git repositories, so keeping it would redirect their fixtures into
# the branch that is being pushed.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
  GIT_CEILING_DIRECTORIES GIT_DISCOVERY_ACROSS_FILESYSTEM GIT_PREFIX

is_release_verify_draft_push() {
  if (($# != 4)); then
    return 1
  fi
  local updates_path="$1"
  local head_sha="$2"
  local hook_remote="$3"
  local hook_remote_url="$4"
  local configured_push_url local_ref local_sha remote_ref remote_sha extra first_update second_update
  [[ "${hook_remote}" == "origin" ]] || return 1
  configured_push_url="$(git remote get-url --push origin)" || return 1
  [[ "${hook_remote_url}" == "${configured_push_url}" ]] || return 1
  {
    IFS= read -r first_update || return 1
    if IFS= read -r second_update || [[ -n "${second_update:-}" ]]; then
      return 1
    fi
  } <"${updates_path}"
  read -r local_ref local_sha remote_ref remote_sha extra <<<"${first_update}"
  [[ -z "${extra}" ]] || return 1
  [[ "$(git branch --show-current)" == "release/verify" ]] || return 1
  [[ "${local_ref}" == "refs/heads/release/verify" ]] || return 1
  [[ "${remote_ref}" == "refs/heads/release/verify" ]] || return 1
  [[ "${local_sha}" =~ ^[0-9a-f]{40}$ && "${remote_sha}" =~ ^[0-9a-f]{40}$ ]] || return 1
  [[ "${local_sha}" != "0000000000000000000000000000000000000000" ]] || return 1
  [[ "${local_sha}" == "${head_sha}" ]]
}

updates="$(mktemp)"
cleanup() {
  rm -f "${updates}"
}
trap cleanup EXIT

cat >"${updates}"
reviewed_head="$(git rev-parse HEAD)"
if is_release_verify_draft_push "${updates}" "${reviewed_head}" "$@"; then
  exit 0
fi
if (($# > 0)); then
  export REVIEW_REMOTE="$1"
  if (($# > 1)); then
    export REVIEW_REMOTE_URL="$2"
  else
    unset REVIEW_REMOTE_URL
  fi
  verifier_arguments=(--remote "$1")
  if (($# > 1)); then
    verifier_arguments+=(--remote-url "$2")
  fi
  python3 scripts/hooks/verify_push_issue.py "${verifier_arguments[@]}" <"${updates}"
else
  export REVIEW_REMOTE=origin
  unset REVIEW_REMOTE_URL
  python3 scripts/hooks/verify_push_issue.py <"${updates}"
fi
python3 scripts/hooks/pre_push_head_guard.py "${reviewed_head}" <"${updates}"
validation_scope=full
branch="$(git branch --show-current)"
if [[ "${REVIEW_REMOTE}" == origin && "${branch}" == release/ci-* ]]; then
  base_sha="$(git ls-remote --exit-code origin refs/heads/master | cut -f1)"
  validation_scope="$(python3 scripts/hooks/validation_scope.py --base "${base_sha}" --head "${reviewed_head}")"
fi
case "${validation_scope}" in
  automation) python3 scripts/hooks/run_locked_command.py -- bash scripts/hooks/check_automation_changes.sh ;;
  full) just check ;;
  *) exit 1 ;;
esac
# 品質確認中にcheckoutが切り替わっていないことを確かめ、検証済みHEADとの対応を保つ。
python3 scripts/hooks/pre_push_head_guard.py "${reviewed_head}" <"${updates}"
