#!/usr/bin/env bash
set -euo pipefail

work_dir="${1:?usage: bash scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.build.sh WORK_DIR}"
version="${2:-12.2.0}"
mode="${3:-verify}"
repo_root="$(git rev-parse --show-toplevel)"
archive_url="https://codeload.github.com/mermaid-js/mermaid/tar.gz/1fb7869866dd619168d840291c9c54b4f129758b"
archive_sha256="cc9efe255ea61aa2e991ea4c9c0c49f286b3f81e590fa58d04750723da758476"
patch_path="$repo_root/scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.patch"
patch_sha256="83b12555170e0bef0303a0995d19f8524daf909d90862f052fec863ad9f5dddf"
license_sha256="ec9fb67dcb25eccc416ed56e1aab819222c805a2a4bfe4cb19e7556bf2ffde80"
lock_sha256="d75b263b5a0ace23797e375423a7d24e6e208c440c7eb75d4cc4a422de5973ca"
output_sha256="831ed9d054742315d8edcb9bb933c6e56fd79d0c187a2cdf41ad48e388583a4c"
output_bytes="5494857"
pnpm_cjs_path="${KRR_PNPM_CJS_PATH:?set KRR_PNPM_CJS_PATH to the verified pnpm 10.30.3 cjs binary}"
pnpm_cjs_sha256="b276da51dc8ca5b0d3ee3371695b50fc8b3244b281b091c63a3f082a88dadeb9"
case "$version" in
  12.2.0)
    if [ "$mode" != verify ] || [ "$#" -gt 2 ]; then
      printf '12.2.0 uses its fixed verification pins\n' >&2
      exit 2
    fi
    ;;
  12.3.0)
    # shellcheck source=mermaid-12.3.0-krr.pr6823.1.inputs.sh
    source "$repo_root/scripts/runtime-assets/mermaid-12.3.0-krr.pr6823.1.inputs.sh"
    case "$mode" in
      candidate)
        if [ "$#" -ne 3 ]; then exit 2; fi
        ;;
      verify)
        if [ "$#" -ne 5 ] || [[ ! "$4" =~ ^[a-f0-9]{64}$ ]] || [[ ! "$5" =~ ^[1-9][0-9]*$ ]]; then
          printf '12.3.0 verify requires an independently pinned output SHA-256 and byte count\n' >&2
          exit 2
        fi
        output_sha256="$4"
        output_bytes="$5"
        ;;
      *) printf 'unknown 12.3.0 build mode\n' >&2; exit 2 ;;
    esac
    ;;
  *) printf 'unsupported Mermaid version\n' >&2; exit 2 ;;
esac
mkdir -p "$work_dir"
work_dir="$(cd "$work_dir" && pwd -P)"
pnpm_cjs_path="$(cd "$(dirname "$pnpm_cjs_path")" && pwd -P)/$(basename "$pnpm_cjs_path")"
export GIT_CEILING_DIRECTORIES="$work_dir"
export HUSKY=0
archive="$work_dir/mermaid-$version-source.tar.gz"
source="$work_dir/source"
curl --fail --location --output "$archive" "$archive_url"
printf '%s  %s\n' "$archive_sha256" "$archive" | shasum -a 256 -c -
printf '%s  %s\n' "$patch_sha256" "$patch_path" | shasum -a 256 -c -
printf '%s  %s\n' "$pnpm_cjs_sha256" "$pnpm_cjs_path" | shasum -a 256 -c -
if [ -e "$source" ]; then
  printf 'refusing to overwrite existing source directory: %s\n' "$source" >&2
  exit 1
fi
mkdir "$source"
tar -xzf "$archive" --strip-components=1 -C "$source"
printf '%s  %s\n' "$license_sha256" "$source/LICENSE" | shasum -a 256 -c -
printf '%s  %s\n' "$lock_sha256" "$source/pnpm-lock.yaml" | shasum -a 256 -c -
git -C "$source" apply --check "$patch_path"
git -C "$source" apply "$patch_path"
if [ "$(node --version)" != "v26.10.0" ]; then
  printf 'node v26.10.0 is required\n' >&2
  exit 1
fi
if [ "$(node "$pnpm_cjs_path" --version)" != "10.30.3" ]; then
  printf 'pnpm 10.30.3 is required\n' >&2
  exit 1
fi
cd "$source"
node "$pnpm_cjs_path" install --frozen-lockfile
node "$pnpm_cjs_path" build:mermaid
actual_output="packages/mermaid/dist/mermaid.min.js"
if [ "$mode" = candidate ]; then
  printf 'candidate only; output has not been reproducibility-verified\n'
  shasum -a 256 "$actual_output"
  wc -c < "$actual_output"
else
  printf '%s  %s\n' "$output_sha256" "$actual_output" | shasum -a 256 -c -
  if [ "$(wc -c < "$actual_output" | tr -d ' ')" != "$output_bytes" ]; then
    printf 'unexpected Mermaid output byte count\n' >&2
    exit 1
  fi
fi
