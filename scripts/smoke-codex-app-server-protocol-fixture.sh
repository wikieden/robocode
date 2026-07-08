#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary="$repo_root/target/debug/viden"

cargo build -p viden-cli >/dev/null

smoke_dir=$(mktemp -d /tmp/robocode-codex-app-server-protocol.XXXXXX)
cleanup() {
  if [ "${ROBOCODE_KEEP_SMOKE_DIR:-}" != "1" ]; then
    rm -rf "$smoke_dir"
  else
    printf 'Keeping smoke workspace: %s\n' "$smoke_dir" >&2
  fi
}
trap cleanup EXIT

mock_codex="$smoke_dir/mock-codex"
cat >"$mock_codex" <<'MOCK'
#!/usr/bin/env sh
set -eu

if [ "${1:-}" != "app-server" ]; then
  printf 'mock codex only supports app-server\n' >&2
  exit 2
fi

read init
case "$init" in *'"method":"initialize"'*) ;; *) exit 3 ;; esac
printf '%s\n' '{"id":1,"result":{"userAgent":"Codex Desktop/mock protocol fixture","codexHome":"/tmp/codex-home","platformFamily":"unix","platformOs":"macos"}}'

read thread
case "$thread" in *'"method":"thread/start"'*) ;; *) exit 4 ;; esac
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thread_fixture","sessionId":"thread_fixture","turns":[]},"model":"gpt-test"}}'
printf '%s\n' '{"method":"thread/started","params":{"thread":{"id":"thread_fixture"}}}'

read turn
case "$turn" in *'"method":"turn/start"'*) ;; *) exit 5 ;; esac
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn_fixture","items":[],"itemsView":"complete","status":"inProgress","error":null,"startedAt":1,"completedAt":null,"durationMs":null}}}'
printf '%s\n' '{"method":"turn/started","params":{"threadId":"thread_fixture","turn":{"id":"turn_fixture","status":"inProgress"}}}'
printf '%s\n' '{"method":"item/commandExecution/outputDelta","params":{"threadId":"thread_fixture","turnId":"turn_fixture","command":"cargo test","delta":"running"}}'
printf '%s\n' '{"method":"item/fileChange/outputDelta","params":{"threadId":"thread_fixture","turnId":"turn_fixture","path":"src/config.rs","delta":"+ changed"}}'
printf '%s\n' '{"method":"item/fileChange/patchUpdated","params":{"threadId":"thread_fixture","turnId":"turn_fixture","path":"src/config.rs"}}'
printf '%s\n' '{"method":"turn/diff/updated","params":{"threadId":"thread_fixture","turnId":"turn_fixture","files":["src/config.rs"]}}'
printf '%s\n' '{"method":"fs/changed","params":{"path":"src/config.rs"}}'
printf '%s\n' '{"method":"item/started","params":{"threadId":"thread_fixture","turnId":"turn_fixture","item":{"type":"mcpToolCall","id":"call_fixture","server":"node_repl","tool":"js","status":"inProgress","arguments":{"code":"await fs.writeFile(\"live.txt\", \"ok\")"}}}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread_fixture","turnId":"turn_fixture","item":{"type":"mcpToolCall","id":"call_fixture","server":"node_repl","tool":"js","status":"completed","arguments":{"code":"await fs.writeFile(\"live.txt\", \"ok\")"},"result":{"content":[{"type":"text","text":"ok"}]}}}}'
printf '%s\n' '{"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread_fixture","turnId":"turn_fixture","itemId":"cmd_1","command":"cargo test","cwd":"/tmp"}}'

read approval
case "$approval" in *'"id":9'*'"decision":"decline"'*) ;; *) exit 6 ;; esac
printf '%s\n' '{"method":"error","params":{"message":"mock recoverable app-server error"}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread_fixture","turnId":"turn_fixture","item":{"type":"agentMessage","text":"protocol fixture complete"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread_fixture","turn":{"id":"turn_fixture","status":"completed"}}}'
MOCK
chmod +x "$mock_codex"

cd "$smoke_dir"
git init >/dev/null 2>&1 || true
git config user.email robocode-smoke@example.local >/dev/null 2>&1 || true
git config user.name "RoboCode Smoke" >/dev/null 2>&1 || true
printf 'hello\n' > README.md
git add README.md >/dev/null 2>&1 || true
git commit -m initial >/dev/null 2>&1 || true

export ROBOCODE_AGENT_CODEX_COMMAND="$mock_codex"
output=$(
  printf '%s\n%s\n%s\n' \
    '/agent probe codex --turn Exercise command file approval and error protocol fixture.' \
    '/agent result codex-app-turn_fixture' \
    '/quit' |
    "$binary" --no-tui --provider fallback --model test-local
)

printf '%s\n' "$output"

printf '%s\n' "$output" | grep -q 'Codex app-server probe ok.'
printf '%s\n' "$output" | grep -q 'tracked_job: codex-app-turn_fixture'
printf '%s\n' "$output" | grep -q 'message: protocol fixture complete'
printf '%s\n' "$output" | grep -q 'approvals: item/commandExecution/requestApproval'
printf '%s\n' "$output" | grep -q 'signals: command-output, file-change, file-patch, diff-updated, fs-changed, mcp-tool-call, mcp-tool-completed, mcp-fs-write, app-server-error'

result_file="$smoke_dir/.robocode/agents/codex-app-turn_fixture.result.md"
test -f "$result_file"
grep -q '^thread: thread_fixture' "$result_file"
grep -q '^turn: turn_fixture' "$result_file"
grep -q '^status: completed' "$result_file"
grep -q '^resume: thread_fixture' "$result_file"
grep -q '^message: protocol fixture complete' "$result_file"
grep -q '^approvals: item/commandExecution/requestApproval' "$result_file"
grep -q '^signals: command-output, file-change, file-patch, diff-updated, fs-changed, mcp-tool-call, mcp-tool-completed, mcp-fs-write, app-server-error' "$result_file"

log_file=$(find "$smoke_dir/.robocode/agents" -name 'codex-app-server-*.jsonl' -print -quit)
test -n "$log_file"
grep -q 'item/commandExecution/outputDelta' "$log_file"
grep -q 'item/fileChange/outputDelta' "$log_file"
grep -q 'item/fileChange/patchUpdated' "$log_file"
grep -q 'turn/diff/updated' "$log_file"
grep -q 'fs/changed' "$log_file"
grep -q 'mcpToolCall' "$log_file"
grep -q 'item/commandExecution/requestApproval' "$log_file"
grep -q '\\"decision\\":\\"decline\\"' "$log_file"
grep -q '\\"method\\":\\"error\\"' "$log_file"

printf 'Codex app-server protocol fixture smoke passed.\n'
