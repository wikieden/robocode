# Viden 0.1.28 Plan - Delegated Lane Visibility Cleanup

Chinese version: [release-0.1.28-plan.zh-CN.md](release-0.1.28-plan.zh-CN.md)

`0.1.28` closes the delegated-lane visibility gap left after the daily coding
loop hardening release. This release does not add a new agent backend. It makes
existing lane state, evidence, and next actions trustworthy enough for daily
operator use.

## Goals

- Show lane next actions that match the real operator lifecycle:
  complete -> accept or revise, accepted -> apply, applied -> cleanup.
- Keep side-screen lane state truthful for closed lanes so `applied`,
  `discarded`, `detached`, and `stopped` no longer look like active thinking
  work.
- Add stable background counts for delegated work: active, review, blocked, and
  done.
- Keep lane evidence visible through task records, side screens, lane artifacts,
  and release smoke output.
- Preserve the non-blocking daily loop from `0.1.27`: provider turns, approvals,
  plan mode, and lane work must not lock composer input.

## Implementation Scope

- Update `agent_tasks` lane next-action records so isolated completed lanes ask
  for explicit operator acceptance before apply.
- Add side-status lane buckets for background delegated work.
- Normalize closed operator lane states in side-1 details.
- Extend deterministic TUI regression tests for the lane lifecycle and side
  status output.
- Keep docs and release status paired in English and Chinese.

## Release Gate

Before publishing `0.1.28`, run:

```bash
cargo test -p viden-cli agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions -- --nocapture
cargo test -p viden-cli side_status_rows_summarize_lane_background_counts -- --nocapture
cargo test -p viden-cli side_lane_rows_render_closed_operator_states_as_done -- --nocapture
scripts/smoke-lane-operator-loop.sh
scripts/tui-turn-controller-smoke.sh
scripts/release-gate.sh --version 0.1.28 --phase prepublish --out-dir /tmp/viden-0128-release-gate
```

The prepublish gate must include the live DeepSeek development scenario and
record token, elapsed-time, estimated-cost, and failure-class evidence.

After publishing:

```bash
scripts/release-gate.sh --version 0.1.28 --phase postpublish --out-dir /tmp/viden-0128-release-gate
```

`0.1.28` is complete only when:

- deterministic lane and daily-loop regression tests pass;
- the prepublish gate passes with live DeepSeek smoke evidence;
- GitHub Release `v0.1.28` is published with assets and checksums;
- `wikieden/homebrew-tap` points to `0.1.28`;
- postpublish validation passes.
