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

uv venv --python "$(command -v python3)" "$tool_dir/python"
uv pip sync --python "$tool_dir/python/bin/python3" --require-hashes scripts/release/html-performance-requirements.lock
export PATH="$tool_dir/python/bin:$PATH"
# Gitのファイル名処理に影響させず、必要な回帰だけに実物のRTKを渡す。
export KRR_CONTRACT_RTK_DIRECTORY="$tool_dir"

if (( $# == 0 )); then
  just check-contracts
  exit
fi

# 失敗を調べる場合は、各スイートを中断せず一度ずつ実行する。
status=0
for suite in hooks release quality mermaid; do
  python3 -m unittest discover -s "scripts/$suite" -p '*_test.py' -v || status=$?
done
python3 -m unittest discover -s scripts/validation-timing -p test_measure.py -v || status=$?
just build-capacity-check || status=$?
exit "$status"
