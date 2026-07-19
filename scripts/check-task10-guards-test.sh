#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/viden-task10-guards.XXXXXX")"
trap 'rm -rf "$TMP_ROOT"' EXIT

expect_success() {
  local label="$1"
  shift
  if ! "$@" >"$TMP_ROOT/$label.out" 2>&1; then
    cat "$TMP_ROOT/$label.out" >&2
    printf 'expected success: %s\n' "$label" >&2
    return 1
  fi
}

expect_failure() {
  local label="$1"
  shift
  if "$@" >"$TMP_ROOT/$label.out" 2>&1; then
    cat "$TMP_ROOT/$label.out" >&2
    printf 'expected failure: %s\n' "$label" >&2
    return 1
  fi
}

mkdir -p "$TMP_ROOT/docs/guide" "$TMP_ROOT/apps/tui"
printf '# English\n\n[Guide](guide/index.md)\n' >"$TMP_ROOT/docs/overview.md"
printf '# Chinese\n\n[Guide](guide/index.md)\n' >"$TMP_ROOT/docs/overview.zh-CN.md"
printf '# Guide\n' >"$TMP_ROOT/docs/guide/index.md"

expect_success doc-pair-valid \
  "$ROOT/scripts/check-doc-pairs.sh" \
  "$TMP_ROOT/docs/overview.md" "$TMP_ROOT/docs/overview.zh-CN.md"

rm "$TMP_ROOT/docs/overview.zh-CN.md"
expect_failure doc-pair-missing \
  "$ROOT/scripts/check-doc-pairs.sh" "$TMP_ROOT/docs/overview.md"
printf '# Chinese\n\n[Guide](guide/index.md)\n' >"$TMP_ROOT/docs/overview.zh-CN.md"

expect_success doc-link-valid \
  "$ROOT/scripts/check-doc-links.sh" "$TMP_ROOT/docs/overview.md"
printf '# English\n\n[Missing](guide/missing.md)\n' >"$TMP_ROOT/docs/overview.md"
expect_failure doc-link-missing \
  "$ROOT/scripts/check-doc-links.sh" "$TMP_ROOT/docs/overview.md"

cat >"$TMP_ROOT/apps/tui/Cargo.toml" <<'EOF'
[package]
name = "fixture-tui"
version = "0.0.0"

[dependencies]
viden-core = { path = "../../../crates/core" }
viden-types = { path = "../../../crates/types" }
EOF
expect_success dependency-valid \
  "$ROOT/scripts/check-dependency-boundaries.sh" "$TMP_ROOT/apps/tui/Cargo.toml"

cat >>"$TMP_ROOT/apps/tui/Cargo.toml" <<'EOF'
viden-runtime = { path = "../../../crates/runtime" }
EOF
expect_failure dependency-forbidden \
  "$ROOT/scripts/check-dependency-boundaries.sh" "$TMP_ROOT/apps/tui/Cargo.toml"

printf 'Task 10 guard fixture tests passed.\n'
