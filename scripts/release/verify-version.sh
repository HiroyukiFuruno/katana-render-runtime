#!/usr/bin/env bash
set -euo pipefail

input="${1:-}"
if [[ -z "${input}" ]]; then
  input="$(awk -F '"' '/^version = / { print "v" $2; exit }' Cargo.toml)"
fi

version="v${input#v}"
version_bare="${version#v}"

if [[ ! "${version_bare}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid release version: ${input}" >&2
  exit 1
fi

cargo_version="$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)"
if [[ "${cargo_version}" != "${version_bare}" ]]; then
  echo "Cargo.toml version (${cargo_version}) does not match ${version}" >&2
  exit 1
fi

echo "version=${version}"
echo "version_bare=${version_bare}"
