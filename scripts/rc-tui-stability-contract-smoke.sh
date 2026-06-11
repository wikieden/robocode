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

require_executable scripts/rc-tui-stability-smoke.sh
require_text scripts/release-smoke.sh "rc-tui-stability-smoke"
require_text scripts/release-smoke.sh "scripts/rc-tui-stability-smoke.sh"
require_text docs/release-0.1.29-plan.md "RC TUI stability smoke"
require_text docs/release-0.1.29-plan.zh-CN.md "RC TUI stability smoke"
require_text docs/release-0.1.29-status.md "P0/P1 TUI Backlog"
require_text docs/release-0.1.29-status.zh-CN.md "P0/P1 TUI Backlog"
require_text docs/tui-stability-zero-bug-gate.md "scripts/rc-tui-stability-smoke.sh"
require_text docs/tui-stability-zero-bug-gate.zh-CN.md "scripts/rc-tui-stability-smoke.sh"

printf 'RC TUI stability contract smoke passed\n'
