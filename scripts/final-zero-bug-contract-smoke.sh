#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_file() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    printf 'missing required file: %s\n' "$file" >&2
    exit 1
  fi
}

require_executable() {
  local file="$1"
  require_file "$file"
  if [[ ! -x "$file" ]]; then
    printf 'required script is not executable: %s\n' "$file" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local needle="$2"
  require_file "$file"
  if ! grep -Fq "$needle" "$file"; then
    printf 'missing required text in %s: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_executable scripts/final-zero-bug-smoke.sh
require_text scripts/release-gate.sh "final-zero-bug-smoke"
require_text scripts/release-smoke.sh "final-zero-bug-contract-smoke"
require_text docs/release-0.1.30-plan.md "final zero-bug gate"
require_text docs/release-0.1.30-plan.zh-CN.md "final zero-bug gate"
require_text docs/release-0.1.30-status.md "P0/P1 TUI Backlog"
require_text docs/release-0.1.30-status.zh-CN.md "P0/P1 TUI Backlog"
require_text docs/tui-stability-zero-bug-gate.md "scripts/final-zero-bug-smoke.sh"
require_text docs/tui-stability-zero-bug-gate.zh-CN.md "scripts/final-zero-bug-smoke.sh"

printf 'Final zero-bug contract smoke passed\n'
