#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary="$repo_root/target/debug/robocode-cli"

cargo build -p robocode-cli >/dev/null

smoke_dir=$(mktemp -d /tmp/robocode-codex-app-server-smoke.XXXXXX)
cleanup() {
  if [ "${ROBOCODE_KEEP_SMOKE_DIR:-}" != "1" ]; then
    rm -rf "$smoke_dir"
  else
    printf 'Keeping smoke workspace: %s\n' "$smoke_dir" >&2
  fi
}
trap cleanup EXIT

cd "$smoke_dir"
git init >/dev/null 2>&1 || true
git config user.email robocode-smoke@example.local >/dev/null 2>&1 || true
git config user.name "RoboCode Smoke" >/dev/null 2>&1 || true
printf 'hello\n' > README.md
git add README.md >/dev/null 2>&1 || true
git commit -m initial >/dev/null 2>&1 || true

output=$(
  printf '%s\n%s\n%s\n' \
    '/agent probe codex --turn Say exactly ROBOCODE_APP_SERVER_SMOKE_OK and do not edit files.' \
    '/agent status' \
    '/quit' |
    "$binary" --no-tui --provider fallback --model test-local
)

printf '%s\n' "$output"

printf '%s\n' "$output" | grep -q 'Codex app-server probe ok.'
printf '%s\n' "$output" | grep -q 'tracked_job:'
printf '%s\n' "$output" | grep -q 'finished'

result_file=$(find "$smoke_dir/.robocode/agents" -name '*.result.md' -print -quit)
test -n "$result_file"
grep -q '^thread: ' "$result_file"
grep -q '^turn: ' "$result_file"
grep -q '^status: completed' "$result_file"
grep -q '^resume: ' "$result_file"
grep -q '^message: ROBOCODE_APP_SERVER_SMOKE_OK' "$result_file"

log_file=$(find "$smoke_dir/.robocode/agents" -name 'codex-app-server-*.jsonl' -print -quit)
test -n "$log_file"
grep -q 'ROBOCODE_APP_SERVER_SMOKE_OK' "$log_file"

printf 'Codex app-server smoke passed.\n'
