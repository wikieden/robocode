#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-"$(mktemp -d /tmp/robocode-daily-loop-smoke.XXXXXX)"}"
mkdir -p "$OUT_DIR"

WORK_DIR="$OUT_DIR/workspace"
TRANSCRIPT="$OUT_DIR/daily-loop-transcript.log"
DIFF_OUT="$OUT_DIR/daily-loop.diff"
PREVIEW_ANSI="$OUT_DIR/daily-loop-tui-preview.ansi"
SUMMARY="$OUT_DIR/summary.md"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

(
  cd "$WORK_DIR"
  git init >/dev/null
  git config user.email smoke@example.com
  git config user.name "RoboCode Smoke"
  printf '# daily loop fixture\n' >README.md
  git add README.md
  git commit -m initial >/dev/null

  printf '/brief create hello.py and verify the daily loop\ny\n/brief steering init\ny\n/brief show\ntool write_file path=hello.py content=print("daily-loop-ok")\ny\n/test python3 hello.py\ny\n/diff\n/status\n/exit\n' |
    cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
      --no-tui \
      --provider fallback \
      --model test-local
) >"$TRANSCRIPT" 2>&1

git -C "$WORK_DIR" add -N hello.py
git -C "$WORK_DIR" diff -- hello.py >"$DIFF_OUT"
cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
  --provider fallback \
  --model test-local \
  --tui-preview-ansi >"$PREVIEW_ANSI" 2>&1

grep -Fq "write_file" "$TRANSCRIPT"
grep -Fq "Active brief" "$TRANSCRIPT"
grep -Fq "Steering files ready" "$TRANSCRIPT"
grep -Fq "Test result:" "$TRANSCRIPT"
grep -Fq "status: passed" "$TRANSCRIPT"
grep -Fq "daily-loop-ok" "$TRANSCRIPT"
grep -Fq "Latest diff" "$TRANSCRIPT"
grep -Fq "hello.py" "$TRANSCRIPT"
grep -Fq 'print("daily-loop-ok")' "$DIFF_OUT"
test -f "$WORK_DIR/.robocode/briefs/active.md"
test -f "$WORK_DIR/.robocode/steering/conventions.md"
[[ -s "$PREVIEW_ANSI" ]]

cat >"$SUMMARY" <<EOF
# RoboCode Daily Loop Smoke

- Workspace: \`$WORK_DIR\`
- Transcript: \`$TRANSCRIPT\`
- Diff: \`$DIFF_OUT\`
- TUI ANSI preview: \`$PREVIEW_ANSI\`
- Provider: \`fallback / test-local\`
- Result: passed
EOF

printf '%s\n' "$OUT_DIR"
