# Viden 0.1.27 Plan - Daily Coding Loop Hardening

Chinese version: [release-0.1.27-plan.zh-CN.md](release-0.1.27-plan.zh-CN.md)

`0.1.27` finishes the interactive reliability work that still made Viden
feel fragile during real coding. The scope is intentionally narrow: make the
main coding loop truthful, responsive, and testable before adding larger agent
features.

## Goals

- Runtime state is not static decoration. Topbar, composer, and bottom status
  bar read real `RuntimeSnapshot` mode and permission values.
- `/mode plan`, `/mode build`, and `/permissions ask` visibly update the TUI in
  the same command turn.
- Provider turns, plan turns, tool execution, and approval prompts keep the
  composer available for typing, queueing, cancellation, and history scroll.
- The active work indicator clears after completion and never leaves stale
  `planning` or `thinking` rows behind.
- Release smoke includes one deterministic daily-loop test and one live
  DeepSeek development scenario with token, duration, cost, and failure
  classification evidence.

## Required Implementation

- Add a full TUI controller regression for:
  `prompt -> streaming -> approval -> write_file -> queued follow-up -> final`.
- Keep normal composer typing available while an approval request is visible;
  approval shortcuts still resolve the prompt.
- Replace static topbar status text with a real activity label such as `idle`,
  `working`, `approval`, or `check`.
- Show real work mode and permission level in the bottom status bar.
- Keep `scripts/tui-turn-controller-smoke.sh` as the fast gate for input,
  approval, queue, mode, permission, scrollback, and stale-status regressions.

## Verification Gate

Before publishing `0.1.27`, run:

```bash
scripts/release-gate.sh --version 0.1.27 --phase prepublish
```

The gate must include:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- focused TUI turn-controller smoke;
- TUI regression preview;
- plan-mode smoke;
- daily-loop smoke;
- live DeepSeek development smoke with token and cost summary.

After publishing GitHub assets and syncing Homebrew, run:

```bash
scripts/release-gate.sh --version 0.1.27 --phase postpublish
```

## Completion Criteria

`0.1.27` is complete only when:

- all prepublish gate steps pass;
- GitHub Release `v0.1.27` is published with assets and checksums;
- `wikieden/homebrew-tap` points to `0.1.27`;
- postpublish Homebrew and GitHub asset validation pass;
- the status doc records DeepSeek tokens, estimated cost, evidence paths, and
  any observed failure class.
