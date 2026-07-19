# Viden TUI Track Agent Guide

## Scope

This file applies to `apps/tui/**`. Read the repository-root `AGENTS.md`,
`docs/parallel-development-plan.md`, and the current design sources under
`docs/viden-design/Viden/TUI/` before changing TUI behavior or visuals.

## Start Gate

For V3 production implementation, the task must name the immutable
`frontend-contract-v1` Core checkpoint SHA.

- Create `codex/v3-tui-client` in `.worktrees/v3-tui-client` from that exact
  checkpoint.
- If no checkpoint SHA is available, remain in interaction/reference review
  and Core contract-gap analysis. Do not begin the production migration.
- Confirm TUI interaction decisions and reference screens with the user before
  changing the production UI.

## Ownership And Client Boundary

- Own `apps/tui/**`, TUI-specific tests, TUI previews/screenshots, and TUI
  user documentation.
- Render from `RuntimeViewState` plus TUI-local layout/input state only.
- Send `RuntimeCommand` through the Core client and wait for ordered
  `RuntimeEvent` confirmation.
- Do not own authoritative lane lifecycle, worktrees, terminal/tmux/PTY spawn,
  accept/apply, conflict recovery, persistence, provider/tool execution, or
  permission reduction.
- Do not infer success from transcript text, command output, or process exit
  copy when Core exposes an event for that fact.
- Missing fields or commands are Core contract requests. Do not create a
  TUI-private business model to bypass them.
- Do not edit `crates/**`, `apps/gui/**`, or shared design tokens without
  coordination with the owning track.

## Interaction Acceptance

Preserve the 0.1.30 zero-bug baseline while aligning with the accepted TUI
design:

- Normal, Insert, and Overlay input modes have explicit ownership.
- `Esc` unwinds overlay -> selection -> insert; `Ctrl-C` targets current work.
- The composer remains usable during streaming, tools, and approvals.
- Queue/cancel, bracketed paste, multiline input, CJK width/cursor behavior,
  scrollback, resize, focus, and selector-first navigation do not regress.
- Approval actions stay pinned and operable; ambient ticker content carries no
  actions.
- Shared Core fixtures replay to the same business facts as GUI.

## Development And Verification

- Use TDD for behavior changes and keep bilingual docs synchronized.
- Reuse the canonical TUI kit and registered glyphs; do not introduce emoji or
  private design tokens.
- Run:

```bash
cargo test -p viden-tui
scripts/tui-turn-controller-smoke.sh
scripts/rc-tui-stability-smoke.sh
scripts/tui-regression.sh
cargo test --workspace --quiet
```

The handoff must include the base Core SHA, branch, worktree, HEAD, fixture
replay result, smoke/regression evidence, screenshots, contract requests, and
blockers. Do not merge or push `main` without explicit user direction.
