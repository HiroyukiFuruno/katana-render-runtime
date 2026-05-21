#!/usr/bin/env bash
set -euo pipefail

file="${1:?sha256 target file is required}"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$file" | awk '{ print $1 }'
  exit 0
fi

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$file" | awk '{ print $1 }'
  exit 0
fi

if command -v python3 >/dev/null 2>&1; then
  python3 - "$file" <<'PY'
import hashlib
import pathlib
import sys

print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())
PY
  exit 0
fi

if command -v python >/dev/null 2>&1; then
  python - "$file" <<'PY'
import hashlib
import pathlib
import sys

print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())
PY
  exit 0
fi

echo "sha256 command is not available" >&2
exit 1
