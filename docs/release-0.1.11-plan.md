# Viden 0.1.11 Plan

Chinese version: [release-0.1.11-plan.zh-CN.md](release-0.1.11-plan.zh-CN.md)

Last updated: 2026-05-27

## Release Positioning

`0.1.11` is the TUI Cockpit Reliability + Orchestration Foundation release.

This release does not try to ship the full `0.2.0` multi-agent orchestration
system. It first establishes two foundations:

- the TUI cockpit is stable, trustworthy, and comfortable in real terminals
- the state model needed for multi-agent orchestration and token efficiency
  starts to become concrete

`0.2.0` remains the Agent Orchestration Runtime v1 release.

## Core Goals

### 1. Real TUI Reliability

Fix and verify the interaction issues already seen during manual use:

- resize, drag, and zoom redraw automatically without stale borders or shifted
  regions
- right-rail borders, colors, titles, and content do not drift
- composer height is more comfortable, with a visible blinking cursor
- CJK IME candidate windows stay as close to the input line as the terminal
  allows
- idle cockpit, approval modal, and command palette share one theme
- approval modal defaults to approve, supports keyboard shortcuts and mouse
  clicks, and closes immediately after a decision
- `/quit` and `/exit` exit reliably

### 2. Main-Screen Now Working State

The center of the main screen must clearly show what Viden is doing right
now instead of making the user guess whether a remote provider or external lane
is still working.

State sources flow through the shared `AgentTask` projection:

- provider thinking / streaming
- tool calls waiting for approval
- shell or test commands running
- external lanes running
- the most recent failure, blocker, or decision needed from the user

The display should include:

- current action
- responsible agent, provider, or lane
- elapsed time
- evidence source
- next available action

### 3. AgentTask / AgentLane Foundation

Prepare the data model for `0.2.0` multi-agent orchestration.

This release should complete:

- a shared `AgentTask` lifecycle: pending, thinking, running, reviewing,
  blocked, completed, failed, cancelled
- an `AgentLane` concept for main, side-1, side-2, shell, codex, claude, and
  deepseek
- evidence on each task/lane: transcript event, tool call, diff, test,
  artifact, and last output
- one state projection consumed by the right rail, side screens, lane detail,
  and Now Working panel

### 4. Token-Efficiency Design Foundation

This release does not implement the full token optimizer, but it should define
the interface and visible product surface.

Required design work:

- `ContextBundle` fields: task, selected files, diff, diagnostics, test
  results, facts, and lane summaries
- tool-output compaction rules: long-log tailing, repeated-output deduplication,
  and failure-summary priority
- per-agent token budget concept
- cost and context-pressure placement in the TUI

If code lands, prefer the smallest useful slice:

- current-turn context summary
- more truthful token/context pressure display
- compact rendering of long tool output without losing the raw transcript audit
  trail

### 5. Real Screenshot Acceptance

Keep the 0.1.10 rule: every user-visible feature needs a real-use screenshot or
deterministic TUI visual artifact before completion.

Required artifacts:

- idle cockpit
- live provider thinking
- Now Working active
- approval modal
- command palette
- main screen after resize
- side-1 lanes
- side-2 ops
- lane detail
- CJK input scenario

## Non-Goals

- no full ACP host claim in 0.1.11
- no full mutation-capable MCP runtime in 0.1.11
- no full plugin/skill marketplace
- no web, desktop, or IDE surface as the primary entrypoint
- no decorative panels without real backing state

## Acceptance Criteria

- A user can type, approve, resize, open the command palette, and run provider
  turns in a real terminal without obvious layout drift or stale regions.
- The main screen continuously displays current work from real `AgentTask` /
  `AgentLane` state.
- Right rail, side screens, and Now Working do not contradict each other.
- Approval can be completed by default Enter, shortcuts, and mouse clicks, then
  the modal disappears.
- Token/context display is no longer decorative; it at least explains current
  context sources and pressure.
- Docs make it clear that `0.2.0` carries the multi-agent orchestration runtime,
  while 0.1.11 is the foundation release.

## Verification

Run at minimum:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.11 --deepseek --out-dir /tmp/viden-0111-release-smoke-full
```

Manual verification:

- Run the TUI in macOS Terminal and iTerm2 at least once.
- Resize, drag, fullscreen, and leave fullscreen.
- Enter one short Chinese prompt with an IME.
- Confirm an approval modal by keyboard and mouse.
- Complete one small DeepSeek write-file plus command-run task.

## Next Step

After `0.1.11`, move into `0.2.0`:

- Agent Orchestration Runtime v1
- default planner -> worker -> reviewer -> tester workflow
- formal Codex / Claude Code / DeepSeek / shell lane orchestration
- ContextBundle builder and token-efficiency engine v1
