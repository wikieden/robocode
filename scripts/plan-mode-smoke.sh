#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-"$(mktemp -d /tmp/robocode-plan-mode-smoke.XXXXXX)"}"
mkdir -p "$OUT_DIR"

WORK_DIR="$OUT_DIR/workspace"
SESSION_HOME="$OUT_DIR/session-home"
TRANSCRIPT="$OUT_DIR/plan-mode-transcript.log"
SUMMARY="$OUT_DIR/summary.md"

rm -rf "$WORK_DIR" "$SESSION_HOME"
mkdir -p "$WORK_DIR" "$SESSION_HOME"

(
  cd "$WORK_DIR"
  printf '/plan on\ntool write_file path=blocked.txt content=blocked\n/test printf plan-should-not-run\n/plan off\ntool write_file path=allowed.txt content=allowed\ny\n/status\n/exit\n' |
    ROBOCODE_SESSION_HOME="$SESSION_HOME" \
      cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
        --no-tui \
        --provider fallback \
        --model test-local
) >"$TRANSCRIPT" 2>&1

grep -Fq "Plan mode is now on" "$TRANSCRIPT"
grep -Fq "tool: write_file" "$TRANSCRIPT"
grep -Fq "reason: PlanMode" "$TRANSCRIPT"
grep -Fq "write_file is blocked while plan mode is active" "$TRANSCRIPT"
grep -Fq "Test result:" "$TRANSCRIPT"
grep -Fq "status: failed" "$TRANSCRIPT"
grep -Fq "tool: shell" "$TRANSCRIPT"
grep -Fq "shell is blocked while plan mode is active" "$TRANSCRIPT"
grep -Fq "Plan mode is now off" "$TRANSCRIPT"
grep -Fq "write_file completed" "$TRANSCRIPT"
grep -Fq "Last test: failed" "$TRANSCRIPT"

test ! -e "$WORK_DIR/blocked.txt"
test -f "$WORK_DIR/allowed.txt"
grep -Fq "allowed" "$WORK_DIR/allowed.txt"

if grep -Fxq "    plan-should-not-run" "$TRANSCRIPT"; then
  printf 'blocked /test command appeared to execute; transcript: %s\n' "$TRANSCRIPT" >&2
  exit 1
fi

cat >"$SUMMARY" <<EOF
# RoboCode Plan Mode Smoke

- Workspace: \`$WORK_DIR\`
- Session home: \`$SESSION_HOME\`
- Transcript: \`$TRANSCRIPT\`
- Provider: \`fallback / test-local\`
- Covered:
  - \`/plan on\` blocks mutating \`write_file\`
  - \`/plan on\` blocks shell-backed \`/test\` before command execution
  - \`/plan off\` allows the same write path after approval
- Result: passed
EOF

printf '%s\n' "$OUT_DIR"
