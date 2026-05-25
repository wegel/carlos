#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

mode="${1:---staged}"
fail=0

usage() {
  printf 'usage: %s (--staged|--all)\n' "$0" >&2
  exit 2
}

rust_paths() {
  case "$mode" in
    --staged)
      git diff --cached --name-only --diff-filter=ACMR -- '*.rs'
      ;;
    --all)
      git ls-files -- '*.rs'
      ;;
    *)
      usage
      ;;
  esac
}

file_contents() {
  local path="$1"
  case "$mode" in
    --staged)
      git show ":$path"
      ;;
    --all)
      cat "$path"
      ;;
    *)
      usage
      ;;
  esac
}

match_file="$(mktemp /tmp/carlos-style-grep.XXXXXX)"
cleanup() {
  rm -f "$match_file"
}
trap cleanup EXIT

while IFS= read -r path; do
  [[ -n "$path" ]] || continue

  line_count="$(file_contents "$path" | wc -l | tr -d ' ')"
  if (( line_count > 400 )); then
    if ! file_contents "$path" | sed -n '1,20p' | grep -Fq 'STYLE-EXCEPTION(file-size):'; then
      printf 'STYLE-FILE-001 %s file has %s lines and no STYLE-EXCEPTION(file-size): marker near the top\n' "$path" "$line_count" >&2
      fail=1
    fi
  fi

  if file_contents "$path" | grep -nE '^[[:space:]]*use[[:space:]].*::\*;' >"$match_file"; then
    while IFS=: read -r line _match; do
      printf 'STYLE-IMP-001 %s:%s glob imports are not allowed\n' "$path" "$line" >&2
    done <"$match_file"
    fail=1
  fi
done < <(rust_paths)

exit "$fail"
