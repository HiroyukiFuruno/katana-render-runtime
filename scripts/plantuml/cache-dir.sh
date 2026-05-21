#!/usr/bin/env bash
set -euo pipefail

case "$(uname -s)" in
  Darwin)
    if [ -n "${HOME:-}" ]; then
      printf '%s\n' "$HOME/Library/Caches/kdr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/kdr/plantuml"
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [ -n "${LOCALAPPDATA:-}" ]; then
      printf '%s\n' "$LOCALAPPDATA/kdr/plantuml"
    elif [ -n "${USERPROFILE:-}" ]; then
      printf '%s\n' "$USERPROFILE/AppData/Local/kdr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/kdr/plantuml"
    fi
    ;;
  *)
    if [ -n "${XDG_CACHE_HOME:-}" ]; then
      printf '%s\n' "$XDG_CACHE_HOME/kdr/plantuml"
    elif [ -n "${HOME:-}" ]; then
      printf '%s\n' "$HOME/.cache/kdr/plantuml"
    else
      printf '%s\n' "${TMPDIR:-/tmp}/kdr/plantuml"
    fi
    ;;
esac
