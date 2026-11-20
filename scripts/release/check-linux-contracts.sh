#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )) || [[ $# -eq 1 && "$1" != --collect-failures ]]; then
  printf 'Usage: %s [--collect-failures]\n' "$0" >&2
  exit 2
fi

tool_dir="$(mktemp -d "${RUNNER_TEMP:?}/krr-contract-rtk.XXXXXXXX")"
trap 'rm -rf -- "$tool_dir"' EXIT
archive="$tool_dir/rtk.tar.gz"
curl --fail --location --retry 3 --silent --show-error --output "$archive" \
  https://github.com/rtk-ai/rtk/releases/download/v1.2.0/rtk-x86_64-unknown-linux-musl.tar.gz
printf '%s  %s\n' 70fee7b4a548c43dde94c77e2335550981b0bc2fb013691d8c8109b5bd0eda37 "$archive" | sha256sum --check
tar -xzf "$archive" -C "$tool_dir" rtk
test "$("$tool_dir/rtk" --version)" = 'rtk 1.2.0'

# RTKの子プロセス契約は実物で検査し、既存レシピの実行コマンドは変えない。
export PATH="$tool_dir:$PATH"
export RTK=

if just check-contracts; then
  exit 0
else
  status=$?
fi

if [[ ${1:-} == --collect-failures ]]; then
  test_python=tmp/html-performance-python-venv/bin/python
  if [[ -x "$test_python" ]]; then
    "$test_python" -m unittest discover -s scripts/release -p '*_test.py' -v || \
      printf 'Release test diagnostics exited %d\n' "$?" >&2
  fi
  just build-capacity-check || printf 'Capacity test diagnostics exited %d\n' "$?" >&2
fi
exit "$status"
