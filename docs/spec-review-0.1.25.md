# Viden 0.1.25 Spec Review

Chinese version: [spec-review-0.1.25.zh-CN.md](spec-review-0.1.25.zh-CN.md)

Last updated: 2026-06-09

## Purpose

This spec review checks the `0.1.25` display-stability release against current
code, docs, and release gates. It uses the same spec-first rule as `0.1.24`:
current behavior must map to implementation, tests, screenshots, or smoke
evidence. Future behavior must be marked as future work.

This release keeps the non-blocking operator loop from `0.1.24` and focuses on
visible TUI correctness:

- long-idle and focus/sleep repaint stability;
- terminal protocol residue in the composer;
- welcome modal clearing and popup placement;
- transcript scrollback and live activity copy;
- release screenshot evidence and TDD coverage.

The TDD testing contract smoke for this release is
`scripts/tdd-testing-contract-smoke.sh`.

## Solid Ground

- Provider turns are dispatched through `TuiRuntime`, leaving the main loop
  available for keyboard, mouse, resize, scroll, approval, streaming, and
  cancellation.
- Streaming deltas no longer steal scrollback while the user is viewing
  history; the transcript label marks `history N · new output`.
- The terminal renderer now has a periodic full-redraw policy so the dirty-row
  cache does not assume the emulator retained all alternate-screen rows.
- Focus and paste events are renderer-visible: they trigger a repaint without
  becoming composer input.
- Composer input filters terminal SGR residue ending in `m` or `M`, covering
  common mouse/color protocol tails.
- Welcome-screen interaction modals clear the full frame because the welcome
  layout has no right rail.

## P0 Gaps

| Priority | Gap | Code Area | Impact | Spec Target |
| --- | --- | --- | --- | --- |
| P0 | Manual long-idle terminal acceptance still needs real Terminal/iTerm2 evidence | `viden-cli/src/tui/terminal.rs`, manual acceptance | Automated tests cover redraw policy, focus/paste repaint policy, and preview output, but real terminal sleep/focus behavior differs by emulator | Capture or record manual acceptance for macOS Terminal and iTerm2 before final 0.1.x zero-bug gate |
| P0 | Active-turn queue ownership remains TUI-local | `viden-cli/src/tui/state.rs`, `viden-cli/src/tui/app.rs` | Queued prompts are visible and preserved, but no-TUI/core queue ownership is not yet formal | Keep UI behavior stable in 0.1.25; move durable runtime queue ownership in a later architecture slice |

## P1 Gaps

| Priority | Gap | Code Area | Impact | Spec Target |
| --- | --- | --- | --- | --- |
| P1 | Provider doctor/probe still has synchronous command paths | `viden-cli/src/tui/app.rs`, `viden-core/src/provider_commands.rs` | A future real network doctor can still freeze if run synchronously | Convert doctor/probe to background jobs with status, tail, evidence, and cancel |
| P1 | Provider capability differences still need a complete adapter matrix | `viden-model/src/providers.rs`, `viden-model/src/adapters.rs` | DeepSeek, DashScope, OpenRouter, Anthropic, and OpenAI-compatible differences can leak into UI and recovery behavior | Provider descriptors declare auth, endpoints, model catalogs, stream fields, tool semantics, context limits, retry policy, and error mapping |
| P1 | Recent/favorite model management is still light | `viden-cli/src/tui/app.rs` model picker | Global `/models` no longer shows unconfigured providers, but richer recent persistence and favorite editing are still thin | Persist recent model choices and expose direct favorite toggling without duplicates |

## P2 Gaps

- Historical docs and screenshots may still mention older `DeepSeek is
  thinking` copy. New TUI copy should use Viden/internal-role wording such
  as `Viden is planning`.
- The release preview set is deterministic, but the final 0.1.x zero-bug gate
  should still add real terminal screenshot acceptance.

## Acceptance Gates

- `cargo fmt --check`
- `scripts/tdd-testing-contract-smoke.sh`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-turn-controller-smoke.sh`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/plan-mode-smoke.sh /tmp/viden-0125-plan-mode-smoke`
- `scripts/daily-loop-smoke.sh /tmp/viden-0125-daily-loop-smoke`
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash`
- `scripts/release-gate.sh --version 0.1.25`
- `scripts/release-gate.sh --version 0.1.25 --phase postpublish`

## Release Decision Rule

`0.1.25` can ship only when all automated release gates pass, deterministic TUI
screenshots are regenerated with `0.1.25` names, the release status records
DeepSeek token/cost evidence, and GitHub Release plus Homebrew validation both
pass.
