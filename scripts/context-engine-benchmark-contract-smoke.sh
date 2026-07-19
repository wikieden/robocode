#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SCRIPT="scripts/context-engine-benchmark.sh"
FIXTURES="crates/runtime/src/tests/fixtures/context-benchmark"
OUT_ROOT="${1:-$(mktemp -d /tmp/viden-context-benchmark-contract.XXXXXX)}"

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    printf 'missing required file: %s\n' "$path" >&2
    exit 1
  fi
}

require_file "$SCRIPT"
require_file "$FIXTURES/valid/runs/off-1.json"
require_file "$FIXTURES/valid/runs/on-1.json"

"$SCRIPT" --fixtures "$FIXTURES/valid" --out-dir "$OUT_ROOT/valid" >/tmp/viden-context-benchmark-contract-valid.log
test -f "$OUT_ROOT/valid/summary.md"
test -f "$OUT_ROOT/valid/comparison.json"
test -f "$OUT_ROOT/valid/failure-classification.json"
test -f "$OUT_ROOT/valid/runs/off-1.json"
grep -Fq "Result: passed" "$OUT_ROOT/valid/summary.md"
grep -Fq '"median_input_token_reduction_ratio": 0.25' "$OUT_ROOT/valid/comparison.json"

expect_failure() {
  local name="$1"
  local needle="$2"
  local out_dir="$OUT_ROOT/$name"
  set +e
  "$SCRIPT" --fixtures "$FIXTURES/$name" --out-dir "$out_dir" >"$out_dir.stdout" 2>"$out_dir.stderr"
  local rc=$?
  set -e
  if [[ "$rc" == "0" ]]; then
    printf 'expected fixture %s to fail\n' "$name" >&2
    exit 1
  fi
  grep -Fq "$needle" "$out_dir/failure-classification.json"
}

expect_failure "missing-field" "missing_required_field"
expect_failure "task-mismatch" "task_success_mismatch"
expect_failure "test-mismatch" "test_success_mismatch"
expect_failure "evidence-mismatch" "evidence_mismatch"
expect_failure "low-reduction" "input_token_reduction_below_threshold"
expect_failure "provider-413" "provider_413"
expect_failure "unclassified" "unclassified_failure"
expect_failure "permission-bypass" "permission_bypass"
expect_failure "slow-p95" "bundle_build_p95_over_threshold"

printf 'context engine benchmark contract smoke passed: %s\n' "$OUT_ROOT"
