#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SCRIPT="scripts/context-engine-benchmark.sh"
FIXTURES="crates/runtime/src/tests/fixtures/context-benchmark"
OUT_ROOT="${1:-$(mktemp -d /tmp/viden-context-benchmark-contract.XXXXXX)}"
mkdir -p "$OUT_ROOT"

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

set +e
"$SCRIPT" --fixtures "$FIXTURES/valid" --runs 2 --out-dir "$OUT_ROOT/runs-too-low" >"$OUT_ROOT/runs-too-low.stdout" 2>"$OUT_ROOT/runs-too-low.stderr"
rc=$?
set -e
if [[ "$rc" == "0" ]]; then
  printf 'expected --runs 2 to fail\n' >&2
  exit 1
fi
grep -Fq -- "--runs must be an integer >= 3" "$OUT_ROOT/runs-too-low.stderr"

"$SCRIPT" --fixtures "$FIXTURES/valid" --out-dir "$OUT_ROOT/valid" >/tmp/viden-context-benchmark-contract-valid.log
test -f "$OUT_ROOT/valid/summary.md"
test -f "$OUT_ROOT/valid/comparison.json"
test -f "$OUT_ROOT/valid/failure-classification.json"
test -f "$OUT_ROOT/valid/runs/off-1.json"
grep -Fq "Result: passed" "$OUT_ROOT/valid/summary.md"
grep -Fq '"median_input_token_reduction_ratio": 0.25' "$OUT_ROOT/valid/comparison.json"

"$SCRIPT" --fixtures "$FIXTURES" --runs 3 --out-dir "$OUT_ROOT/fixture-root" >/tmp/viden-context-benchmark-contract-root.log
grep -Fq "Result: passed" "$OUT_ROOT/fixture-root/summary.md"

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

cp -R "$FIXTURES/valid" "$OUT_ROOT/wrong-run-count"
rm "$OUT_ROOT/wrong-run-count/runs/on-3.json"
set +e
"$SCRIPT" --fixtures "$OUT_ROOT/wrong-run-count" --runs 3 --out-dir "$OUT_ROOT/wrong-run-count-out" >"$OUT_ROOT/wrong-run-count.stdout" 2>"$OUT_ROOT/wrong-run-count.stderr"
rc=$?
set -e
if [[ "$rc" == "0" ]]; then
  printf 'expected wrong-run-count fixture to fail\n' >&2
  exit 1
fi
grep -Fq "run_count_mismatch" "$OUT_ROOT/wrong-run-count-out/failure-classification.json"

cp -R "$FIXTURES/valid" "$OUT_ROOT/empty-evidence"
python3 - "$OUT_ROOT/empty-evidence/runs/on-1.json" <<'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
payload = json.loads(path.read_text(encoding="utf-8"))
payload["evidence_hashes"] = []
path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
set +e
"$SCRIPT" --fixtures "$OUT_ROOT/empty-evidence" --runs 3 --out-dir "$OUT_ROOT/empty-evidence-out" >"$OUT_ROOT/empty-evidence.stdout" 2>"$OUT_ROOT/empty-evidence.stderr"
rc=$?
set -e
if [[ "$rc" == "0" ]]; then
  printf 'expected empty-evidence fixture to fail\n' >&2
  exit 1
fi
grep -Fq "missing_evidence" "$OUT_ROOT/empty-evidence-out/failure-classification.json"

cp -R "$FIXTURES/valid" "$OUT_ROOT/invalid-field-type"
python3 - "$OUT_ROOT/invalid-field-type/runs/on-1.json" <<'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
payload = json.loads(path.read_text(encoding="utf-8"))
payload["task_success"] = "false"
path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
set +e
"$SCRIPT" --fixtures "$OUT_ROOT/invalid-field-type" --runs 3 --out-dir "$OUT_ROOT/invalid-field-type-out" >"$OUT_ROOT/invalid-field-type.stdout" 2>"$OUT_ROOT/invalid-field-type.stderr"
rc=$?
set -e
if [[ "$rc" == "0" ]]; then
  printf 'expected invalid-field-type fixture to fail\n' >&2
  exit 1
fi
grep -Fq "invalid_field_type" "$OUT_ROOT/invalid-field-type-out/failure-classification.json"

scripts/release-gate.sh --version 0.1.30 --phase prepublish --dry-run --out-dir "$OUT_ROOT/release-dry-run" >"$OUT_ROOT/release-dry-run.stdout"
grep -Fq "scripts/check-task10-guards-test.sh" "$OUT_ROOT/release-dry-run.stdout"

printf 'context engine benchmark contract smoke passed: %s\n' "$OUT_ROOT"
