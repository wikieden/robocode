# RoboCode 0.1.25 Status - TUI Display Cleanup And Idle Stability

Chinese version: [release-0.1.25-status.zh-CN.md](release-0.1.25-status.zh-CN.md)

`0.1.25` is the TUI display cleanup release. It hardens the `0.1.24`
non-blocking operator loop against long-idle repaint drift, terminal
focus/paste gaps, composer protocol residue, and welcome/modal clearing bugs.

## Release State

- Workspace version: `0.1.25`
- Git tag: pending `v0.1.25`
- Release commit: pending
- GitHub release: pending
- Release workflow: pending
- Homebrew tap commit: pending
- Prepublish evidence: `/tmp/robocode-0125-release-gate/prepublish`
- Local package: `dist/robocode-v0.1.25-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `19a7423158910cb9fbab8823bf45122613593057c2e4bb6628c416e94b23a5e6`
- Post-publish evidence: pending
- Distribution state: pending GitHub Release assets and Homebrew validation

## Included Changes

- Terminal drawing now forces periodic full redraws so the dirty-row cache does
  not assume the emulator retained all alternate-screen content after long
  idle, focus, or sleep/wake periods.
- Focus and paste events trigger TUI repaint without becoming composer input.
- Composer input discards terminal SGR residue ending in `m` or `M`, covering
  common mouse/color protocol tails such as `2;28;95;132m`.
- Welcome-screen interaction modals clear across the full frame because the
  welcome layout has no right rail.
- `/connect` provider selector previews no longer leak underlying
  `commands /connect` hint text behind the modal.
- TUI zero-bug gate docs record the long-idle/focus/paste regression and its
  required guardrails.
- `0.1.25` plan/spec/status docs are versioned and included in the TDD release
  contract smoke.

## Validation

Focused checks already used during implementation:

```bash
cargo fmt --check
cargo test -p robocode-cli tui::app::tests -- --nocapture
cargo test -p robocode-cli tui::render::tests -- --nocapture
cargo test -p robocode-cli tui::terminal::tests -- --nocapture
cargo test -p robocode-cli --quiet
cargo clippy -p robocode-cli --all-targets -- -D warnings
scripts/tui-regression.sh /tmp/robocode-tui-regression-0125-idle-fix
```

Release gate:

```bash
scripts/release-gate.sh --version 0.1.25
```

Result: passed prepublish on 2026-06-09. Evidence:
`/tmp/robocode-0125-release-gate`.

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

Post-publish verification:

```bash
scripts/release-gate.sh --version 0.1.25 --phase postpublish
```

Result: pending.

## DeepSeek Live Development Scenario

- Provider/model: `deepseek` / `deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok, `0` errors
- Tokens: input `11269`, output `455`, total `11724`
- Estimated cost: `¥0.012179 CNY`
- Evidence: `/tmp/robocode-0125-release-gate/prepublish/deepseek-dev-scenario`

## Screenshot Evidence

Generated with:

```bash
scripts/tui-regression.sh docs/previews/generated
```

Result: passed. Structured evidence:
`docs/previews/generated/tui-regression-evidence.json`.

Deterministic screenshot set:

- `docs/previews/generated/screenshots/0.1.25-tui-main.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-main-idle.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-live-turn.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-main-resize.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-cjk-input.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-command-palette.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-setup-wizard.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-provider-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-provider-detail.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-model-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-lane-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-lane-detail.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-side-1.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-side-2.svg`

## Remaining Risks

- Real terminal focus/sleep behavior can vary by emulator. The automated guard
  covers redraw policy and deterministic previews; final 0.1.x should still add
  manual Terminal/iTerm2 acceptance evidence.
- Provider doctor/probe and durable active-turn queue ownership remain future
  architecture work; they are not new commitments in this display cleanup
  patch.
