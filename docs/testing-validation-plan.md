# Testing and Validation Plan

Chinese version: [testing-validation-plan.zh-CN.md](testing-validation-plan.zh-CN.md)

Last updated: 2026-05-27

## Purpose

RoboCode should be validated like a developer tool that people will use in
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

### 3. TUI Visual Checks

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

### 4. Safety and Permission Checks

Run when touching tools, approvals, permissions, lanes, app-server integration,
plugins, MCP, skills, or workflow state:

```bash
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-lane-operator-loop.sh
```

The expected default is fail-closed. A mutating path without a valid permission
decision should not proceed.

### 5. Live Provider Checks

Use deterministic fallback tests for default CI. Run live providers only when
credentials and rate limits are available:

```bash
scripts/release-smoke.sh --quick --deepseek
```

Live checks should prove provider compatibility, not replace deterministic
fixtures.

### 6. Release Checks

Before tagging a release candidate:

```bash
scripts/release-smoke.sh --version <version>
```

When credentials are available:

```bash
scripts/release-smoke.sh --version <version> --deepseek --github-actions
```

After publishing:

```bash
gh release view v<version>
scripts/release-smoke.sh --version <version> --github-release-assets --homebrew --skip-package
```

The release status document must record the command, evidence directory,
release URL, workflow run, assets, Homebrew result, and remaining risks.

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
/tmp/robocode-<version>-*/screenshots/
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

- supported-platform package builds
- release asset upload
- sha256 validation
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
