#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

run_exact_test() {
  local package="$1"
  local test_target="$2"
  local test_name="$3"
  local output_file
  output_file="$(mktemp "${TMPDIR:-/tmp}/viden-fixture-parity.XXXXXX")"

  if ! cargo test -p "$package" --test "$test_target" "$test_name" -- --exact 2>&1 | tee "$output_file"; then
    rm -f "$output_file"
    return 1
  fi

  if ! grep -Eq 'test result: ok\. 1 passed; 0 failed' "$output_file"; then
    echo "fixture parity gate: expected exactly one passing test named $test_name" >&2
    rm -f "$output_file"
    return 1
  fi
  rm -f "$output_file"
}

run_exact_lib_test() {
  local package="$1"
  local test_name="$2"
  local output_file
  output_file="$(mktemp "${TMPDIR:-/tmp}/viden-fixture-parity.XXXXXX")"

  if ! cargo test -p "$package" --lib "$test_name" -- --exact 2>&1 | tee "$output_file"; then
    rm -f "$output_file"
    return 1
  fi

  if ! grep -Eq 'test result: ok\. 1 passed; 0 failed' "$output_file"; then
    echo "fixture parity gate: expected exactly one passing test named $test_name" >&2
    rm -f "$output_file"
    return 1
  fi
  rm -f "$output_file"
}

run_exact_test \
  viden-core \
  frontend_contract_v1 \
  interaction_closed_loop_fixture_replays_identically_after_a_gap
run_exact_lib_test \
  viden-tui \
  tui::app::tests::native_acp_fixture_render
run_exact_test viden-gui native_acp_fixture native_acp_fixture_projection

echo "native/ACP fixture parity: PASS"
