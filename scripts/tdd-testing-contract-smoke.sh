#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_text() {
  local file="$1"
  local needle="$2"
  if ! grep -Fq "$needle" "$file"; then
    printf 'missing required text in %s: %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

require_text docs/testing-validation-plan.md "TDD Release Contract"
require_text docs/testing-validation-plan.md "RED -> GREEN -> REFACTOR"
require_text docs/testing-validation-plan.md "one behavior, one failing test, one minimal implementation"
require_text docs/testing-validation-plan.zh-CN.md "TDD 发布合同"
require_text docs/testing-validation-plan.zh-CN.md "RED -> GREEN -> REFACTOR"
require_text docs/testing-validation-plan.zh-CN.md "一个行为、一个失败测试、一个最小实现"

require_text docs/release-0.1.25-plan.md "scripts/tdd-testing-contract-smoke.sh"
require_text docs/release-0.1.25-plan.zh-CN.md "scripts/tdd-testing-contract-smoke.sh"
require_text docs/spec-review-0.1.25.md "TDD testing contract smoke"
require_text docs/spec-review-0.1.25.zh-CN.md "TDD testing contract smoke"
require_text scripts/release-smoke.sh "tdd-testing-contract-smoke"
require_text scripts/release-smoke.sh "scripts/tdd-testing-contract-smoke.sh"

printf 'TDD testing contract smoke passed\n'
