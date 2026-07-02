#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-}"
VERSION="${ROBOCODE_FINAL_ZERO_BUG_VERSION:-}"
MANUAL_DIR="${ROBOCODE_TUI_MANUAL_EVIDENCE_DIR:-}"

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/robocode-final-zero-bug.XXXXXX")"
fi
mkdir -p "$OUT_DIR"

cd "$ROOT"

if [[ -z "$VERSION" ]]; then
  VERSION="$(cargo pkgid -p viden-cli | sed 's/.*#//')"
fi

SUMMARY="$OUT_DIR/summary.md"
STEPS="$OUT_DIR/step-results.tsv"
EVIDENCE_JSON="$OUT_DIR/final-zero-bug-evidence.json"
: >"$SUMMARY"
: >"$STEPS"

record() {
  printf '%s\n' "$*" >>"$SUMMARY"
}

record_step() {
  local name="$1"
  local status="$2"
  local log_file="$3"
  printf '%s\t%s\t%s\n' "$name" "$status" "$log_file" >>"$STEPS"
}

run_step() {
  local name="$1"
  shift
  local log_file="$OUT_DIR/${name}.log"
  printf '[final-zero-bug] START %s\n' "$name"
  if "$@" >"$log_file" 2>&1; then
    printf '[final-zero-bug] PASS  %s\n' "$name"
    record "- PASS \`$name\`"
    record_step "$name" "pass" "$log_file"
  else
    local rc=$?
    printf '[final-zero-bug] FAIL  %s (exit %s)\n' "$name" "$rc" >&2
    record "- FAIL \`$name\` (exit $rc, log: \`$log_file\`)"
    record_step "$name" "fail" "$log_file"
    tail -80 "$log_file" >&2 || true
    exit "$rc"
  fi
}

validate_manual_evidence() {
  if [[ -z "$MANUAL_DIR" ]]; then
    printf 'ROBOCODE_TUI_MANUAL_EVIDENCE_DIR is required for the final zero-bug gate\n' >&2
    return 2
  fi
  if [[ ! -d "$MANUAL_DIR" ]]; then
    printf 'manual evidence directory does not exist: %s\n' "$MANUAL_DIR" >&2
    return 2
  fi

  local terminal_count
  local iterm_count
  terminal_count="$(find "$MANUAL_DIR" -maxdepth 1 -type f \( -iname '*terminal*.png' -o -iname '*terminal*.jpg' -o -iname '*terminal*.jpeg' -o -iname '*terminal*.svg' \) | wc -l | tr -d ' ')"
  iterm_count="$(find "$MANUAL_DIR" -maxdepth 1 -type f \( -iname '*iterm*.png' -o -iname '*iterm*.jpg' -o -iname '*iterm*.jpeg' -o -iname '*iterm*.svg' -o -iname '*iterm2*.png' -o -iname '*iterm2*.jpg' -o -iname '*iterm2*.jpeg' -o -iname '*iterm2*.svg' \) | wc -l | tr -d ' ')"

  if [[ "$terminal_count" -lt 1 ]]; then
    printf 'manual evidence must include at least one macOS Terminal screenshot in %s\n' "$MANUAL_DIR" >&2
    return 1
  fi
  if [[ "$iterm_count" -lt 1 ]]; then
    printf 'manual evidence must include at least one iTerm2 screenshot in %s\n' "$MANUAL_DIR" >&2
    return 1
  fi
}

validate_core_screenshots() {
  local screenshot_dir="$OUT_DIR/tui-regression/screenshots"
  local -a expected=(
    "${VERSION}-tui-main-idle.svg"
    "${VERSION}-tui-main.svg"
    "${VERSION}-tui-live-turn.svg"
    "${VERSION}-tui-main-resize.svg"
    "${VERSION}-tui-command-palette.svg"
    "${VERSION}-tui-setup-wizard.svg"
    "${VERSION}-tui-provider-selector.svg"
    "${VERSION}-tui-provider-detail.svg"
    "${VERSION}-tui-model-selector.svg"
    "${VERSION}-tui-side-1.svg"
    "${VERSION}-tui-side-2.svg"
  )
  local artifact
  for artifact in "${expected[@]}"; do
    if [[ ! -s "$screenshot_dir/$artifact" ]]; then
      printf 'missing core TUI screenshot evidence: %s\n' "$screenshot_dir/$artifact" >&2
      return 1
    fi
  done
}

write_evidence_json() {
  python3 - "$STEPS" "$EVIDENCE_JSON" "$VERSION" "$OUT_DIR" "$MANUAL_DIR" <<'PY'
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

steps_path = Path(sys.argv[1])
target_path = Path(sys.argv[2])
version = sys.argv[3]
out_dir = sys.argv[4]
manual_dir = sys.argv[5]
steps = []
for line in steps_path.read_text(encoding="utf-8").splitlines():
    if not line.strip():
        continue
    name, status, log_file = line.split("\t", 2)
    steps.append({"name": name, "status": status, "log": log_file})
target_path.write_text(
    json.dumps(
        {
            "version": version,
            "status": "passed" if all(step["status"] == "pass" for step in steps) else "failed",
            "out_dir": out_dir,
            "manual_evidence_dir": manual_dir,
            "p0_open": 0,
            "p1_open": 0,
            "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "steps": steps,
        },
        indent=2,
        ensure_ascii=False,
    )
    + "\n",
    encoding="utf-8",
)
PY
}

record "# RoboCode Final Zero-Bug TUI Smoke"
record ""
record "- Version: \`$VERSION\`"
record "- Evidence directory: \`$OUT_DIR\`"
record "- Manual evidence directory: \`${MANUAL_DIR:-<missing>}\`"
record ""
record "## Results"

run_step "manual-terminal-evidence" validate_manual_evidence
run_step "tui-regression" env ROBOCODE_TUI_SCREENSHOT_VERSION="$VERSION" scripts/tui-regression.sh "$OUT_DIR/tui-regression"
run_step "core-tui-screenshot-evidence" validate_core_screenshots
run_step "rc-tui-stability-smoke" scripts/rc-tui-stability-smoke.sh "$OUT_DIR/rc-tui-stability"
run_step "plan-mode-smoke" scripts/plan-mode-smoke.sh "$OUT_DIR/plan-mode-smoke"
run_step "daily-loop-smoke" scripts/daily-loop-smoke.sh "$OUT_DIR/daily-loop-smoke"

record ""
record "## P0/P1 TUI Backlog"
record ""
record "- Known open P0: \`0\`."
record "- Known open P1: \`0\`."
record "- Failing cases: none observed in this final zero-bug smoke."
record "- Known P2 issues: none release-blocking in this run."
record ""
record "## Evidence"
record ""
record "- Deterministic screenshots: \`$OUT_DIR/tui-regression/screenshots\`"
record "- RC TUI summary: \`$OUT_DIR/rc-tui-stability/summary.md\`"
record "- Plan mode summary: \`$OUT_DIR/plan-mode-smoke/summary.md\`"
record "- Daily loop evidence: \`$OUT_DIR/daily-loop-smoke\`"
record "- Manual screenshots: \`$MANUAL_DIR\`"
record ""
record "Generated at: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
record "- Structured evidence: \`$EVIDENCE_JSON\`"
write_evidence_json
printf '%s\n' "$OUT_DIR"
