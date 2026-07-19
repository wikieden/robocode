#!/usr/bin/env bash
set -euo pipefail

FIXTURES=""
PROVIDER=""
MODEL="${VIDEN_LIVE_DEEPSEEK_MODEL:-deepseek-v4-flash}"
RUNS=3
OUT_DIR=""

usage() {
  cat <<'EOF'
Usage: scripts/context-engine-benchmark.sh [--fixtures <dir> | --provider deepseek --model <model>] --out-dir <dir> [--runs <n>]

Runs the context engine A/B release gate.

Deterministic fixture mode is offline and validates per-run usage JSON from:
  <fixtures>/runs/*.json

Live mode runs the same DeepSeek development scenario with VIDEN_CONTEXT_ENGINE
off and on for N runs each. Live mode requires DEEPSEEK_API_KEY and is billable.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fixtures)
      FIXTURES="${2:-}"
      shift 2
      ;;
    --provider)
      PROVIDER="${2:-}"
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --runs)
      RUNS="${2:-}"
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

if [[ -z "$OUT_DIR" ]]; then
  printf '--out-dir is required\n\n' >&2
  usage >&2
  exit 2
fi
if [[ -n "$FIXTURES" && -n "$PROVIDER" ]]; then
  printf 'choose either --fixtures or --provider, not both\n' >&2
  exit 2
fi
if [[ -z "$FIXTURES" && -z "$PROVIDER" ]]; then
  printf 'either --fixtures or --provider is required\n' >&2
  exit 2
fi
if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [[ "$RUNS" -lt 3 ]]; then
  printf '%s\n' '--runs must be an integer >= 3' >&2
  exit 2
fi

mkdir -p "$OUT_DIR/runs"

if [[ -n "$FIXTURES" ]]; then
  if [[ -d "$FIXTURES/valid/runs" ]]; then
    FIXTURES="$FIXTURES/valid"
  fi
  if [[ ! -d "$FIXTURES/runs" ]]; then
    printf 'fixture runs directory not found: %s/runs\n' "$FIXTURES" >&2
    exit 2
  fi
  cp "$FIXTURES"/runs/*.json "$OUT_DIR/runs/"
else
  if [[ "$PROVIDER" != "deepseek" ]]; then
    printf 'Task 9 live gate currently supports --provider deepseek only\n' >&2
    exit 2
  fi
  if [[ -z "${DEEPSEEK_API_KEY:-}" ]]; then
    printf 'DEEPSEEK_API_KEY is required for billable context engine live gate\n' >&2
    exit 2
  fi
  for mode in off on; do
    for run in $(seq 1 "$RUNS"); do
      run_dir="$OUT_DIR/live-$mode-$run"
      mkdir -p "$run_dir"
      start_epoch="$(date +%s)"
      VIDEN_CONTEXT_ENGINE="$mode" \
        VIDEN_LIVE_SMOKE_RUN_ID="context-engine-${mode}-${run}" \
        scripts/deepseek-dev-scenario-smoke.sh --model "$MODEL" --out-dir "$run_dir"
      end_epoch="$(date +%s)"
      python3 - "$run_dir/usage.json" "$OUT_DIR/runs/${mode}-${run}.json" "$mode" "$run" "$((end_epoch - start_epoch))" <<'PY'
import json
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
mode = sys.argv[3]
run_index = int(sys.argv[4])
payload = json.loads(source.read_text(encoding="utf-8"))
payload["engine_mode"] = mode
payload["run_index"] = run_index
target.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
PY
    done
  done
fi

python3 - "$OUT_DIR" "$RUNS" <<'PY'
import json
import math
import shutil
import sys
from pathlib import Path

out_dir = Path(sys.argv[1])
expected_runs = int(sys.argv[2])
runs_dir = out_dir / "runs"
comparison_path = out_dir / "comparison.json"
summary_path = out_dir / "summary.md"
failure_path = out_dir / "failure-classification.json"

required = {
    "prompt_version",
    "provider",
    "model",
    "scenario",
    "engine_mode",
    "run_index",
    "task_success",
    "test_success",
    "evidence_hashes",
    "input_tokens",
    "output_tokens",
    "cached_input_tokens",
    "total_tokens",
    "estimated_cost_cny",
    "actual_cost_cny",
    "first_token_latency_ms",
    "total_latency_ms",
    "request_input_chars",
    "projection_chars",
    "raw_baseline_chars",
    "retrieval_count",
    "context_event_count",
    "retry_count",
    "compression_ratio",
    "failure_class",
    "bundle_build_ms",
    "provider_413",
    "permission_bypass",
}

def fail(reason, detail, runs=None, comparison=None):
    payload = {
        "status": "failed",
        "reason": reason,
        "detail": detail,
    }
    failure_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    if comparison is not None:
        comparison_path.write_text(json.dumps(comparison, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    summary_path.write_text(
        "# Context Engine Benchmark\n\n"
        f"Result: failed\n\n- Reason: `{reason}`\n- Detail: {detail}\n",
        encoding="utf-8",
    )
    raise SystemExit(1)

def median(values):
    values = sorted(values)
    if not values:
        return 0
    mid = len(values) // 2
    if len(values) % 2:
        return values[mid]
    return (values[mid - 1] + values[mid]) / 2

def percentile(values, percentile):
    values = sorted(values)
    if not values:
        return 0
    index = max(0, math.ceil((percentile / 100.0) * len(values)) - 1)
    return values[index]

def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)

def validate_types(payload, file_name):
    string_fields = ("prompt_version", "provider", "model", "scenario", "engine_mode", "failure_class")
    boolean_fields = ("task_success", "test_success", "provider_413", "permission_bypass")
    integer_fields = (
        "run_index", "input_tokens", "output_tokens", "cached_input_tokens", "total_tokens",
        "total_latency_ms", "request_input_chars", "projection_chars", "raw_baseline_chars",
        "retrieval_count", "context_event_count", "retry_count", "bundle_build_ms",
    )
    number_fields = ("compression_ratio",)
    nullable_number_fields = ("estimated_cost_cny", "actual_cost_cny", "first_token_latency_ms")

    for field in string_fields:
        if not isinstance(payload[field], str) or not payload[field].strip():
            fail("invalid_field_type", f"{file_name}: {field} must be a non-empty string")
    for field in boolean_fields:
        if not isinstance(payload[field], bool):
            fail("invalid_field_type", f"{file_name}: {field} must be a boolean")
    for field in integer_fields:
        if not isinstance(payload[field], int) or isinstance(payload[field], bool) or payload[field] < 0:
            fail("invalid_field_type", f"{file_name}: {field} must be a non-negative integer")
    for field in number_fields:
        if not is_number(payload[field]) or payload[field] < 0:
            fail("invalid_field_type", f"{file_name}: {field} must be a non-negative finite number")
    for field in nullable_number_fields:
        if payload[field] is not None and (not is_number(payload[field]) or payload[field] < 0):
            fail("invalid_field_type", f"{file_name}: {field} must be null or a non-negative finite number")
    evidence = payload["evidence_hashes"]
    if not isinstance(evidence, list) or any(not isinstance(value, str) or not value.strip() for value in evidence):
        fail("invalid_field_type", f"{file_name}: evidence_hashes must be an array of non-empty strings")

runs = []
for path in sorted(runs_dir.glob("*.json")):
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        fail("malformed_json", f"{path.name}: {exc}")
    missing = sorted(required - payload.keys())
    if missing:
        fail("missing_required_field", f"{path.name}: {', '.join(missing)}")
    validate_types(payload, path.name)
    payload["_file"] = path.name
    runs.append(payload)

if not runs:
    fail("missing_runs", "no usage JSON files found")

modes = {"off": [], "on": []}
for run in runs:
    mode = run.get("engine_mode")
    if mode not in modes:
        fail("invalid_engine_mode", f"{run['_file']}: {mode}")
    modes[mode].append(run)
if not modes["off"] or not modes["on"]:
    fail("missing_cohort", "both engine_mode off and on runs are required")
if len(modes["off"]) != expected_runs or len(modes["on"]) != expected_runs:
    fail(
        "run_count_mismatch",
        f"expected exactly {expected_runs} runs per cohort, got off={len(modes['off'])} on={len(modes['on'])}",
    )
for mode, mode_runs in modes.items():
    indexes = sorted(int(run["run_index"]) for run in mode_runs)
    if indexes != list(range(1, expected_runs + 1)):
        fail("run_index_mismatch", f"{mode} run_index values must be 1..{expected_runs}, got {indexes}")

scenario_keys = {(run["prompt_version"], run["provider"], run["model"], run["scenario"]) for run in runs}
if len(scenario_keys) != 1:
    fail("scenario_mismatch", f"expected one prompt/provider/model/scenario, got {sorted(scenario_keys)}")

all_task = all(bool(run["task_success"]) for run in runs)
all_test = all(bool(run["test_success"]) for run in runs)
off_task = {bool(run["task_success"]) for run in modes["off"]}
on_task = {bool(run["task_success"]) for run in modes["on"]}
off_test = {bool(run["test_success"]) for run in modes["off"]}
on_test = {bool(run["test_success"]) for run in modes["on"]}
if off_task != on_task:
    fail("task_success_mismatch", "engine off/on task_success cohorts differ")
if off_test != on_test:
    fail("test_success_mismatch", "engine off/on test_success cohorts differ")
if not all_task:
    fail("task_failed", "all six benchmark runs must complete the task")
if not all_test:
    fail("test_failed", "all six benchmark runs must pass tests")

off_evidence = {tuple(sorted(run["evidence_hashes"])) for run in modes["off"]}
on_evidence = {tuple(sorted(run["evidence_hashes"])) for run in modes["on"]}
if any(not run["evidence_hashes"] for run in runs):
    fail("missing_evidence", "each run must include non-empty evidence_hashes")
if len(off_evidence) != 1 or len(on_evidence) != 1 or off_evidence != on_evidence:
    fail("evidence_mismatch", "required evidence hashes must match across both cohorts")

for run in runs:
    failure = str(run["failure_class"]).strip().lower()
    if bool(run["permission_bypass"]):
        fail("permission_bypass", f"{run['_file']} reported permission bypass")
    if bool(run["provider_413"]) or failure in {"413", "context_overflow", "context_too_large"}:
        fail("provider_413", f"{run['_file']} reported provider 413/context overflow")
    if failure in {"unclassified", "unknown"}:
        fail("unclassified_failure", f"{run['_file']} reported unclassified failure")
    if failure not in {"", "none", "ok"}:
        fail("failure_class", f"{run['_file']} reported failure_class={run['failure_class']}")
    if int(run["request_input_chars"]) <= 0:
        fail("missing_request_metrics", f"{run['_file']} request_input_chars must be positive")
    if int(run["raw_baseline_chars"]) <= 0:
        fail("missing_request_metrics", f"{run['_file']} raw_baseline_chars must be positive")
    if int(run["context_event_count"]) <= 0:
        fail("missing_context_metrics", f"{run['_file']} context_event_count must be positive")
    if run["engine_mode"] == "on" and int(run["projection_chars"]) <= 0:
        fail("missing_projection_metrics", f"{run['_file']} engine-on projection_chars must be positive")
    if run["engine_mode"] == "off" and int(run["projection_chars"]) != 0:
        fail("projection_mode_mismatch", f"{run['_file']} engine-off projection_chars must be zero")

off_request_median = median([int(run["request_input_chars"]) for run in modes["off"]])
on_request_median = median([int(run["request_input_chars"]) for run in modes["on"]])
if off_request_median == on_request_median:
    fail("request_metrics_not_distinct", "engine off/on request_input_chars medians must differ")

off_input_median = median([int(run["input_tokens"]) for run in modes["off"]])
on_input_median = median([int(run["input_tokens"]) for run in modes["on"]])
reduction = 0 if off_input_median == 0 else (off_input_median - on_input_median) / off_input_median
on_bundle_p95 = percentile([float(run["bundle_build_ms"]) for run in modes["on"]], 95)
comparison = {
    "status": "passed",
    "run_count": len(runs),
    "cohorts": {"off": len(modes["off"]), "on": len(modes["on"])},
    "prompt_version": runs[0]["prompt_version"],
    "provider": runs[0]["provider"],
    "model": runs[0]["model"],
    "scenario": runs[0]["scenario"],
    "median_input_tokens": {"off": off_input_median, "on": on_input_median},
    "median_request_input_chars": {"off": off_request_median, "on": on_request_median},
    "median_input_token_reduction_ratio": round(reduction, 6),
    "p95_bundle_build_ms_on": on_bundle_p95,
    "total_tokens": sum(int(run["total_tokens"]) for run in runs),
    "total_cost_cny": round(sum(float(run["estimated_cost_cny"] or 0) for run in runs), 8),
    "total_duration_ms": sum(int(run["total_latency_ms"] or 0) for run in runs),
    "median_total_latency_ms": {"off": median([int(run["total_latency_ms"] or 0) for run in modes["off"]]), "on": median([int(run["total_latency_ms"] or 0) for run in modes["on"]])},
    "median_retrieval_count": {"off": median([int(run["retrieval_count"]) for run in modes["off"]]), "on": median([int(run["retrieval_count"]) for run in modes["on"]])},
    "median_retry_count": {"off": median([int(run["retry_count"]) for run in modes["off"]]), "on": median([int(run["retry_count"]) for run in modes["on"]])},
    "median_compression_ratio": {"off": median([float(run["compression_ratio"]) for run in modes["off"]]), "on": median([float(run["compression_ratio"]) for run in modes["on"]])},
    "evidence_hashes": list(off_evidence.pop()),
}
if reduction < 0.20:
    fail("input_token_reduction_below_threshold", f"median reduction {reduction:.2%} is below 20%", comparison=comparison)
if on_bundle_p95 > 200:
    fail("bundle_build_p95_over_threshold", f"engine-on p95 bundle build {on_bundle_p95}ms exceeds 200ms", comparison=comparison)

comparison_path.write_text(json.dumps(comparison, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
failure_path.write_text(json.dumps({"status": "passed", "reason": "none", "detail": ""}, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
summary_path.write_text(
    "# Context Engine Benchmark\n\n"
    "Result: passed\n\n"
    f"- Provider/model: `{comparison['provider']}` / `{comparison['model']}`\n"
    f"- Scenario: `{comparison['scenario']}`\n"
    f"- Runs: off=`{comparison['cohorts']['off']}` on=`{comparison['cohorts']['on']}` total=`{comparison['run_count']}`\n"
    f"- Median input tokens: off=`{off_input_median}` on=`{on_input_median}` reduction=`{reduction:.2%}`\n"
    f"- Engine-on p95 bundle build: `{on_bundle_p95}` ms\n"
    f"- Total tokens: `{comparison['total_tokens']}`\n"
    f"- Total estimated cost: `¥{comparison['total_cost_cny']:.8f}` CNY\n"
    f"- Total duration: `{comparison['total_duration_ms']}` ms\n"
    f"- Comparison JSON: `{comparison_path}`\n",
    encoding="utf-8",
)
print(f"Context engine benchmark passed: {comparison_path}")
PY

printf '%s\n' "$OUT_DIR"
