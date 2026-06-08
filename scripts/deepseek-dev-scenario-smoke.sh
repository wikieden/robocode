#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL="${ROBOCODE_DEEPSEEK_SMOKE_MODEL:-deepseek-v4-flash}"
OUT_DIR=""

usage() {
  cat <<'EOF'
Usage: scripts/deepseek-dev-scenario-smoke.sh [--model <model>] [--out-dir <dir>]

Runs a billable, live DeepSeek development scenario:
  prompt -> provider turn -> write_file tools -> generated Python test -> token/cost summary

Required:
  DEEPSEEK_API_KEY

Optional:
  DEEPSEEK_API_BASE or ROBOCODE_LIVE_DEEPSEEK_API_BASE
  ROBOCODE_DEEPSEEK_INPUT_CNY_PER_MTOK
  ROBOCODE_DEEPSEEK_OUTPUT_CNY_PER_MTOK
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
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

if [[ -z "${DEEPSEEK_API_KEY:-}" ]]; then
  printf 'DEEPSEEK_API_KEY is required for DeepSeek development scenario smoke\n' >&2
  exit 2
fi

if [[ -z "$MODEL" ]]; then
  printf 'model is required\n' >&2
  exit 2
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/robocode-deepseek-dev-scenario.XXXXXX")"
fi
mkdir -p "$OUT_DIR"

LOG="$OUT_DIR/deepseek-dev-scenario.log"
USAGE_JSON="$OUT_DIR/usage.json"
SUMMARY="$OUT_DIR/summary.md"

(
  cd "$ROOT"
  ROBOCODE_LIVE_DEEPSEEK_MODEL="$MODEL" \
    cargo test -p robocode-core \
      deepseek_live_development_scenario_creates_and_runs_program \
      -- --ignored --nocapture --test-threads=1
) >"$LOG" 2>&1

usage_line="$(grep -o 'ROBOCODE_LIVE_USAGE_JSON=.*' "$LOG" | tail -1 || true)"
if [[ -z "$usage_line" ]]; then
  printf 'DeepSeek live smoke did not emit usage JSON. Log: %s\n' "$LOG" >&2
  tail -120 "$LOG" >&2 || true
  exit 1
fi
printf '%s\n' "${usage_line#ROBOCODE_LIVE_USAGE_JSON=}" >"$USAGE_JSON"

python3 - "$USAGE_JSON" "$SUMMARY" "$LOG" <<'PY'
import json
import sys
from pathlib import Path

usage_path = Path(sys.argv[1])
summary_path = Path(sys.argv[2])
log_path = Path(sys.argv[3])
payload = json.loads(usage_path.read_text(encoding="utf-8"))
cost = payload.get("estimated_cost_cny")
cost_text = "unknown" if cost is None else f"¥{cost:.6f} CNY"
summary = f"""# DeepSeek Development Scenario Smoke

- Provider: `deepseek`
- Model: `{payload.get("model")}`
- Scenario: `{payload.get("scenario")}`
- Workspace: `{payload.get("workspace")}`
- Requests: `{payload.get("request_count")}` ok=`{payload.get("success_count")}` err=`{payload.get("failure_count")}`
- Tokens: input=`{payload.get("input_tokens")}` output=`{payload.get("output_tokens")}` total=`{payload.get("total_tokens")}`
- Estimated cost: `{cost_text}`
- Pricing basis: `{payload.get("pricing_basis")}`; input cache-miss `¥{payload.get("input_cny_per_million_cache_miss")}/1M`, output `¥{payload.get("output_cny_per_million")}/1M`
- Raw usage: `{usage_path}`
- Log: `{log_path}`

Result: passed
"""
summary_path.write_text(summary, encoding="utf-8")
print(
    "DeepSeek dev scenario passed: "
    f"model={payload.get('model')} "
    f"input_tokens={payload.get('input_tokens')} "
    f"output_tokens={payload.get('output_tokens')} "
    f"total_tokens={payload.get('total_tokens')} "
    f"estimated_cost_cny={payload.get('estimated_cost_cny')}"
)
PY

printf '%s\n' "$OUT_DIR"
