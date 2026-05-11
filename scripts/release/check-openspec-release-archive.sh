#!/usr/bin/env bash
set -euo pipefail

error() { printf '[ERROR] %s\n' "$*" >&2; }
success() { printf '[OK] %s\n' "$*"; }

version_from_branch() {
  local branch="$1"
  if [[ "${branch}" =~ ^release/v([0-9]+)\.([0-9]+)\.([0-9]+)(-[A-Za-z0-9._-]+)?$ ]]; then
    printf '%s.%s.%s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

current_branch() {
  if [[ -n "${GITHUB_HEAD_REF:-}" ]]; then
    printf '%s\n' "${GITHUB_HEAD_REF}"
    return
  fi
  git branch --show-current
}

parse_version() {
  local version="$1"
  version="${version#v}"
  if [[ "${version}" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    printf '%s %s %s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

change_version_at_or_before_target() {
  local change_name="$1"
  local target_major="$2"
  local target_minor="$3"
  local target_patch="$4"

  if [[ ! "${change_name}" =~ ^v([0-9]+)-([0-9]+)-([0-9]+)- ]]; then
    return 1
  fi

  local major="${BASH_REMATCH[1]}"
  local minor="${BASH_REMATCH[2]}"
  local patch="${BASH_REMATCH[3]}"

  if [[ "${major}" -lt "${target_major}" ]]; then
    return 0
  fi
  if [[ "${major}" -gt "${target_major}" ]]; then
    return 1
  fi
  if [[ "${minor}" -lt "${target_minor}" ]]; then
    return 0
  fi
  if [[ "${minor}" -gt "${target_minor}" ]]; then
    return 1
  fi
  [[ "${patch}" -le "${target_patch}" ]]
}

target_version="${1:-}"
if [[ -z "${target_version}" ]]; then
  branch="$(current_branch)"
  if ! target_version="$(version_from_branch "${branch}")"; then
    success "release/v* branch ではないため OpenSpec archive 確認をスキップしました。"
    exit 0
  fi
fi

if ! read -r target_major target_minor target_patch < <(parse_version "${target_version}"); then
  error "invalid release version: ${target_version}"
  exit 1
fi

if [[ ! -d openspec/changes ]]; then
  success "OpenSpec change directory が無いため archive 確認をスキップしました。"
  exit 0
fi

remaining_changes=()
for change_dir in openspec/changes/v*-*-*-*; do
  [[ -d "${change_dir}" ]] || continue
  change_name="$(basename "${change_dir}")"
  if change_version_at_or_before_target \
    "${change_name}" \
    "${target_major}" \
    "${target_minor}" \
    "${target_patch}"; then
    remaining_changes+=("${change_name}")
  fi
done

if [[ "${#remaining_changes[@]}" -gt 0 ]]; then
  error "v${target_version#v} 以前の OpenSpec change が active のまま残っています。"
  error "release/v* の PR 作成前に archive へ移動してください。"
  for change_name in "${remaining_changes[@]}"; do
    error " - ${change_name}"
  done
  exit 1
fi

success "v${target_version#v} 以前の OpenSpec change は archive 済みです。"
