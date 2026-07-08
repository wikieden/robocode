# Testing and Validation Plan

Chinese version: [testing-validation-plan.zh-CN.md](testing-validation-plan.zh-CN.md)

Last updated: 2026-06-07

## Purpose

Viden should be validated like a developer tool that people will use in
long terminal sessions, not like a library that only needs unit tests. The
validation system must prove behavior, safety, release readiness, and visible
TUI quality.

This guide is the standing verification contract for `0.1.10` and later.

## Validation Layers

### 1. Local Fast Checks

Run before or during ordinary feature work:

```bash
cargo fmt --check
cargo test -p <touched-crate> --quiet
scripts/release-smoke.sh --quick
```

Use this layer for quick feedback while implementation is still moving.

### 2. Workspace Checks

Run before claiming a code change is complete:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
```

If a change touches TUI rendering, lane orchestration, permissions, provider
protocols, release packaging, or install flows, also run the relevant focused
smoke script.

### 3. Spec Drift Checks

For any user-visible behavior, architecture boundary, command semantics, or
configuration-flow change, check the relevant spec document first:

- If docs say "current", "implemented", or "available", they must point to
  code, tests, screenshots, or smoke evidence.
- If the code does not yet do it, the docs must say "target", "planned", or
  "future version", and the release plan must include an acceptance gate.
- Long-running TUI work must not add new nested input loops. Provider turns,
  approval, doctor/probe, context building, and tool/lane jobs must report
  through the main event loop or background job events.
- `/connect`, `/models`, `/setup`, `/permissions`, `/theme`, and similar
  interactive settings must not regress into "show command instructions and
  make the user guess". In TUI they should be selector/form/modal first; core
  command text is only a no-TUI fallback.

For 0.1.24 operator-loop or provider setup work, also review
`docs/spec-review-0.1.24.md`. A release cannot be marked complete while P0 gaps
from that review remain open.

Suggested focused gates:

```bash
rg -n "event::read\\(" viden-cli/src/tui
scripts/plan-mode-smoke.sh /tmp/viden-spec-plan-smoke
scripts/tui-regression.sh docs/previews/generated
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
```

The `event::read()` grep is not a permanent ban on all terminal event reads; it
is a review reminder to check whether a modal, approval flow, or active turn has
introduced a blocking reader that takes over the main loop.

### 4. TDD Release Contract

All behavior changes must follow `RED -> GREEN -> REFACTOR`. Behavior includes
code capability, TUI interaction, provider adaptation, release gates, testing
scripts, and documentation contracts themselves.

Each TDD vertical slice must satisfy:

- one behavior, one failing test, one minimal implementation; do not write a
  batch of tests horizontally and then fill in all implementation later.
- RED: add or adjust one observable behavior test, run it, and confirm it fails
  because the target behavior is missing.
- GREEN: write only the minimum implementation needed for the current test.
- REFACTOR: clean up names, duplication, and boundaries only after relevant
  tests are green.
- Completion notes should record the red command, green command, changed files,
  and whether screenshot evidence is required.

Before release, run the testing contract smoke:

```bash
scripts/tdd-testing-contract-smoke.sh
```

This smoke confirms that the testing plan, release plan, spec review, and
`scripts/release-smoke.sh` still contain the TDD gate. It does not replace
behavior tests; it keeps the testing process itself from drifting out of the
release flow.

### 5. TUI Visual Checks

Every TUI-visible change requires a visual artifact. Prefer deterministic SVG
or ANSI snapshots when possible; use real terminal screenshots when the feature
depends on actual terminal behavior such as IME placement, cursor behavior,
mouse interaction, or resize.

Run the deterministic TUI regression entrypoint:

```bash
scripts/tui-regression.sh docs/previews/generated
```

Required states for TUI feature work:

- main idle;
- main active / thinking / streaming;
- approval overlay;
- command palette or slash-command suggestions when applicable;
- test result evidence;
- side-1 lane view when lanes are affected;
- side-2 ops/evidence view when diagnostics or tests are affected;
- compact and wide terminal sizes when layout changes.

Feature completion reports must include the artifact path and a short note
describing what the screenshot proves.

### 6. Safety and Permission Checks

Run when touching tools, approvals, permissions, lanes, app-server integration,
plugins, MCP, skills, or workflow state:

```bash
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-lane-operator-loop.sh
scripts/plan-mode-smoke.sh
```

The expected default is fail-closed. A mutating path without a valid permission
decision should not proceed. `scripts/plan-mode-smoke.sh` runs a real no-TUI
session that proves Plan mode blocks both direct file mutation and shell-backed
`/test` execution before allowing the same write path after `/plan off`.

### 7. Live Provider Checks

Use deterministic fallback tests for default CI. Run live providers only when
credentials and rate limits are available:

```bash
scripts/release-smoke.sh --quick --deepseek
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
```

The DeepSeek smoke is a real development scenario, not an echo test. It creates
and tests a generated Python module, then records token usage and estimated CNY
cost in `usage.json` and `summary.md`. Live checks should prove provider
compatibility, not replace deterministic fixtures.

### 8. Mandatory Release Checks

Every release must pass the release gate. Do not tag, publish, or mark a
version complete from ad-hoc local checks.

Before tagging or publishing a release candidate, run the prepublish gate:

```bash
scripts/release-gate.sh --version <version>
```

The prepublish gate wraps full `scripts/release-smoke.sh --deepseek`, so it
requires `DEEPSEEK_API_KEY` and records the live DeepSeek development scenario
token/cost summary. If the key is unavailable, the release is blocked; it can be
called a local RC at most, not a completed release.

If you need the lower-level command for debugging, it is:

```bash
scripts/release-smoke.sh --version <version> --deepseek
```

After publishing GitHub assets and updating Homebrew, run the postpublish gate:

```bash
scripts/release-gate.sh --version <version> --phase postpublish
```

This wraps:

```bash
gh release view v<version>
scripts/release-smoke.sh --version <version> --github-release-assets --homebrew --skip-package
```

GitHub Release and Homebrew are a single release unit. Every published GitHub
release must update `wikieden/homebrew-tap` to the same version before the
release is considered complete. If the tap is not updated or the Homebrew check
is skipped, record the release as incomplete, not merely partially verified.

The release status document must record the exact gate command, evidence
directory, release URL, workflow run, assets, Homebrew result, live provider
token/cost summary, and remaining risks.

## Screenshot Evidence Contract

Every user-visible feature must end with real-use visual evidence. A feature is
not done until the product owner can inspect the artifact.

Required fields in the final feature note:

```text
Feature:
Scenario:
Command/workflow:
Artifact:
Proves:
Remaining visual risk:
```

Preferred artifact locations:

```text
docs/previews/generated/
/tmp/viden-<version>-*/screenshots/
```

Use stable names:

```text
0.1.10-<feature>-main.svg
0.1.10-<feature>-approval.svg
0.1.10-<feature>-side-1.svg
0.1.10-<feature>-side-2.svg
0.1.10-<feature>-terminal.png
```

## CI Gate Proposal

### PR Fast

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- focused crate tests when the changed paths are known
- `scripts/release-smoke.sh --quick`

### Main Full

- full workspace tests
- deterministic TUI regression snapshots
- lane operator smoke
- app-server protocol fixture
- app-server write guard
- package smoke on the host platform

### Release Full

- `scripts/release-gate.sh --version <version>`
- supported-platform package builds
- release asset upload
- sha256 validation
- live DeepSeek development smoke with token/cost summary
- GitHub release inspection
- Homebrew tap update and fetch verification
- screenshot evidence review

## Completion Rule

Before a feature or release is reported complete, confirm:

- tests passed;
- safety checks passed when relevant;
- docs were updated or explicitly deemed unnecessary;
- screenshot evidence exists for visible behavior;
- the artifact path is included in the final report;
- remaining risks are named rather than hidden.
