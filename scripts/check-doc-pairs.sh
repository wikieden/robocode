#!/usr/bin/env bash
set -euo pipefail

if (( $# == 0 )); then
  printf 'usage: %s <markdown-path> [...]\n' "$0" >&2
  exit 2
fi

status=0
for path in "$@"; do
  if [[ "$path" != *.md ]]; then
    printf 'not a Markdown path: %s\n' "$path" >&2
    status=1
    continue
  fi

  if [[ "$path" == *.zh-CN.md ]]; then
    pair="${path%.zh-CN.md}.md"
  else
    pair="${path%.md}.zh-CN.md"
  fi

  if [[ ! -f "$pair" ]]; then
    printf 'missing bilingual counterpart for %s: %s\n' "$path" "$pair" >&2
    status=1
  fi
done

exit "$status"
