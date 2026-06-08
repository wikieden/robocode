#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROVIDER=""
MODEL=""
OUT_DIR=""
PROMPT="Reply with exactly: robocode-live-smoke-ok"

usage() {
  cat <<'EOF'
Usage: scripts/provider-live-smoke.sh --provider <id> [--model <model>] [--out-dir <dir>]

Runs one real non-TUI provider request and stores evidence in the output
directory. The script never prints API key values; it only checks whether the
provider's key environment variable is present.

Examples:
  scripts/provider-live-smoke.sh --provider deepseek --model deepseek-v4-flash
  scripts/provider-live-smoke.sh --provider dashscope-coding-plan --model qwen3.6-plus
  scripts/provider-live-smoke.sh --provider dashscope-tokenplan --model qwen3.6-plus
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --provider)
      PROVIDER="${2:-}"
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
      shift 2
      ;;
    --prompt)
      PROMPT="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$PROVIDER" ]]; then
  usage >&2
  exit 2
fi

default_model_for_provider() {
  case "$1" in
    deepseek|deepseek-anthropic) printf 'deepseek-v4-flash\n' ;;
    dashscope-coding-plan|dashscope-coding-plan-anthropic) printf 'qwen3.6-plus\n' ;;
    dashscope-tokenplan) printf 'qwen3.6-plus\n' ;;
    dashscope-tokenplan-anthropic) printf 'deepseek-v4-flash\n' ;;
    openrouter) printf 'openai/gpt-oss-20b\n' ;;
    openai|openai-compatible) printf 'gpt-4o-mini\n' ;;
    anthropic) printf 'claude-sonnet-4-6\n' ;;
    kimi) printf 'kimi-k2.6\n' ;;
    qwen) printf 'qwen-plus\n' ;;
    groq) printf 'openai/gpt-oss-20b\n' ;;
    mistral) printf 'mistral-medium-latest\n' ;;
    ollama) printf 'llama3.1\n' ;;
    fallback) printf 'test-local\n' ;;
    *) printf 'test-local\n' ;;
  esac
}

key_env_for_provider() {
  case "$1" in
    deepseek|deepseek-anthropic) printf 'DEEPSEEK_API_KEY\n' ;;
    dashscope-coding-plan|dashscope-coding-plan-anthropic) printf 'DASHSCOPE_CODING_PLAN_API_KEY\n' ;;
    dashscope-tokenplan|dashscope-tokenplan-anthropic) printf 'DASHSCOPE_API_KEY\n' ;;
    openrouter) printf 'OPENROUTER_API_KEY\n' ;;
    openai|openai-compatible) printf 'OPENAI_API_KEY\n' ;;
    anthropic) printf 'ANTHROPIC_API_KEY\n' ;;
    kimi) printf 'MOONSHOT_API_KEY\n' ;;
    qwen) printf 'DASHSCOPE_API_KEY\n' ;;
    groq) printf 'GROQ_API_KEY\n' ;;
    mistral) printf 'MISTRAL_API_KEY\n' ;;
    ollama|fallback) printf '\n' ;;
    *) printf '\n' ;;
  esac
}

if [[ -z "$MODEL" ]]; then
  MODEL="$(default_model_for_provider "$PROVIDER")"
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/robocode-provider-live-smoke-${PROVIDER}.XXXXXX")"
fi
mkdir -p "$OUT_DIR"

KEY_ENV="$(key_env_for_provider "$PROVIDER")"
if [[ -n "$KEY_ENV" && -z "${!KEY_ENV:-}" ]]; then
  printf '%s is required for provider %s\n' "$KEY_ENV" "$PROVIDER" >&2
  exit 2
fi

WORK_DIR="$OUT_DIR/workspace"
TRANSCRIPT="$OUT_DIR/provider-live-transcript.log"
SUMMARY="$OUT_DIR/summary.md"
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

(
  cd "$WORK_DIR"
  git init >/dev/null
  git config user.email smoke@example.com
  git config user.name "RoboCode Smoke"
  printf 'provider live smoke\n' >README.md
  git add README.md
  git commit -m initial >/dev/null
  printf '%s\n/exit\n' "$PROMPT" |
    cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
      --no-tui \
      --provider "$PROVIDER" \
      --model "$MODEL"
) >"$TRANSCRIPT" 2>&1

grep -Fq "robocode-live-smoke-ok" "$TRANSCRIPT"

cat >"$SUMMARY" <<EOF
# RoboCode Provider Live Smoke

- Provider: \`$PROVIDER\`
- Model: \`$MODEL\`
- Key env: \`${KEY_ENV:-not required}\`
- Workspace: \`$WORK_DIR\`
- Transcript: \`$TRANSCRIPT\`
- Result: passed
EOF

printf '%s\n' "$OUT_DIR"
