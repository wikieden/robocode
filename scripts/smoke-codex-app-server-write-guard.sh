#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary="$repo_root/target/debug/viden"

cargo build -p viden-cli >/dev/null

smoke_dir=$(mktemp -d /tmp/robocode-codex-app-server-write-guard.XXXXXX)
cleanup() {
  if [ "${ROBOCODE_KEEP_SMOKE_DIR:-}" != "1" ]; then
    rm -rf "$smoke_dir"
  else
    printf 'Keeping smoke workspace: %s\n' "$smoke_dir" >&2
  fi
}
trap cleanup EXIT

target_file="robocode_app_server_denied.txt"

cd "$smoke_dir"
git init >/dev/null 2>&1 || true
git config user.email robocode-smoke@example.local >/dev/null 2>&1 || true
git config user.name "RoboCode Smoke" >/dev/null 2>&1 || true
printf 'hello\n' > README.md
git add README.md >/dev/null 2>&1 || true
git commit -m initial >/dev/null 2>&1 || true

output_file="$smoke_dir/output.txt"
set +e
{
  printf '%s\n%s\n' \
    "/agent probe codex --turn-write Create ${target_file} with content ROBOCODE_WRITE_GUARD_SHOULD_NOT_LAND." \
    '/quit' |
    "$binary" --no-tui --provider fallback --model test-local
} >"$output_file" 2>&1
status=$?
set -e
output=$(cat "$output_file")

printf '%s\n' "$output"

test "$status" -ne 0
printf '%s\n' "$output" | grep -q 'turn-write` is disabled by default'

if [ -e "$smoke_dir/$target_file" ]; then
  printf 'write-guard smoke failed: %s was created despite default guard\n' "$target_file" >&2
  exit 1
fi

if [ -d "$smoke_dir/.robocode/agents" ] \
  && find "$smoke_dir/.robocode/agents" -name 'codex-app-server-*.jsonl' -print -quit | grep -q .; then
  printf 'write-guard smoke failed: app-server launched despite default guard\n' >&2
  exit 1
fi

printf 'Codex app-server write guard smoke passed.\n'
