# RoboCode 0.1.24 Status - Provider Setup And Non-Blocking Operator Loop

Chinese version: [release-0.1.24-status.zh-CN.md](release-0.1.24-status.zh-CN.md)

`0.1.24` is the provider setup and non-blocking operator-loop release. It keeps
the opencode-style `/connect` and `/models` setup work from `0.1.23`, then moves
live provider turns behind a TUI runtime worker so the main event loop can keep
redrawing, accepting input, handling approval, and preserving queued prompts.

## Release State

- Workspace version: `0.1.24`
- Git tag: pending
- GitHub release: pending
- Release workflow: pending
- Homebrew tap commit: pending
- Prepublish evidence: `/tmp/robocode-0124-release-gate/prepublish`
- Local package: `dist/robocode-v0.1.24-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `5a9bd29040f071a0a4f623a9b9c9795ab8229025b13d04857461a2a9bd952a1b`
- Post-publish evidence: pending GitHub assets and Homebrew validation

## Included Changes

- Provider turns now dispatch through a `TuiRuntime` worker instead of taking
  over the TUI input loop.
- The main TUI loop receives streaming deltas, approval prompts, cancel signals,
  finish events, and provider errors through one controller event path.
- Plan mode and live provider turns no longer leave the composer locked after a
  turn finishes or fails.
- Active-turn queued input stays visible through the shared `AgentTask`
  projection, including queued prompt count and next action text.
- Provider approvals are handled by the main event loop, with the old nested
  active-turn approval loop removed.
- Failed active turns keep the TUI open, restore the first queued draft, and
  keep remaining queued prompts attached to runtime-visible state.
- `/connect` remains a provider connection picker; `/models` remains a
  provider-grouped active-model picker.
- Deterministic TUI regression previews were regenerated with `0.1.24`
  screenshot names.

## Validation

Focused checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-turn-controller-smoke.sh
scripts/plan-mode-smoke.sh /tmp/robocode-0124-plan-mode-smoke
scripts/daily-loop-smoke.sh /tmp/robocode-0124-daily-loop-smoke
scripts/tui-regression.sh docs/previews/generated
```

Release gate:

```bash
scripts/release-gate.sh --version 0.1.24
```

Result: passed prepublish on 2026-06-08. Evidence:
`/tmp/robocode-0124-release-gate`.

Prepublish smoke result:

- `cargo-fmt`: passed
- `tdd-testing-contract-smoke`: passed
- `tui-turn-controller-smoke`: passed
- `cargo-clippy`: passed
- `robocode-cli-tests`: passed
- `workspace-tests`: passed
- `tui-regression`: passed
- `fallback-cli-smoke`: passed
- `plan-mode-smoke`: passed
- `daily-loop-smoke`: passed
- `codex-app-server-protocol-fixture`: passed
- `codex-app-server-write-guard`: passed
- `lane-operator-loop-smoke`: passed
- `package-smoke`: passed
- `deepseek-dev-scenario-smoke`: passed

DeepSeek live development scenario:

- Provider/model: `deepseek` / `deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok, `0` errors
- Tokens: input `11021`, output `427`, total `11448`
- Estimated cost: `¥0.011875 CNY`
- Evidence: `/tmp/robocode-0124-release-gate/prepublish/deepseek-dev-scenario`

Post-publish verification:

```bash
scripts/release-gate.sh --version 0.1.24 --phase postpublish
```

## Screenshot Evidence

Deterministic 0.1.24 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-side-2.svg`

Structured TUI evidence:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/tui-regression-evidence.json`

## Remaining Risks

- Durable active-turn queue ownership still lives in the TUI layer. The
  `AgentTask` projection makes queue state visible in `0.1.24`, but a
  UI-agnostic core/runtime queue remains future work.
- Live provider behavior still depends on account, model, and upstream provider
  availability. The prepublish gate passed a real DeepSeek development smoke,
  but published users can still hit provider-specific account or quota errors.
- GitHub release assets and Homebrew formula validation must both pass before
  this release can be considered complete.
