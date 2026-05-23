#!/usr/bin/env bash
set -euo pipefail

case "$(uname -s)" in
  Darwin)
    if [ -n "${HOME:-}" ]; then
      printf '%s\n' "$HOME/Library/Caches/krr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/krr/plantuml"
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [ -n "${LOCALAPPDATA:-}" ]; then
      printf '%s\n' "$LOCALAPPDATA/krr/plantuml"
    elif [ -n "${USERPROFILE:-}" ]; then
      printf '%s\n' "$USERPROFILE/AppData/Local/krr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/krr/plantuml"
    fi
    ;;
  *)
    if [ -n "${XDG_CACHE_HOME:-}" ]; then
      printf '%s\n' "$XDG_CACHE_HOME/krr/plantuml"
    elif [ -n "${HOME:-}" ]; then
      printf '%s\n' "$HOME/.cache/krr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/krr/plantuml"
    fi
    ;;
esac
