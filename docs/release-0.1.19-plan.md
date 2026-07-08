# Viden 0.1.19 Plan - Delegated Lane Usefulness

Chinese version: [release-0.1.19-plan.zh-CN.md](release-0.1.19-plan.zh-CN.md)

## Summary

`0.1.19` should make delegated lanes useful enough for real development work.
The goal is not to add more agent names. The goal is to make one delegated
review loop dependable:

> ask Viden to delegate a focused review or shell/template task -> watch the
> lane work -> inspect evidence -> accept, apply, discard, retry, or clean up
> without guessing what state the system is in.

This release keeps `0.1.20` as the usability beta gate. `0.1.19` is the last
feature slice before that gate, so the work must prefer fewer complete loops
over broader but ambiguous surfaces.

## Product Goal

By the end of `0.1.19`, a developer should be able to use Viden as an
operator cockpit for at least one real delegated coding-assistant workflow:

- start a delegated lane from the TUI
- see what the lane is doing in the main screen and side screens
- review logs, artifacts, changed files, and next actions
- apply or discard the result safely
- keep enough evidence to resume or debug the lane later

## Current Baseline

Already available on main:

- shared `AgentTask` records for provider/tool/shell/runtime projection
- lane commands such as `/lane codex`, `/lane codex-review`, `/lane run`,
  `/lane ask`, `/lane inspect`, `/lane timeline`, `/lane diff`,
  `/lane artifacts`, `/lane accept`, `/lane apply`, `/lane discard`,
  `/lane retry`, `/lane stop`, `/lane cleanup`
- Codex external-agent commands such as `/agent review codex`,
  `/agent run codex`, `/agent status`, `/agent result`, and `/agent cancel`
- side-1 and side-2 lane panels, lane artifacts under `.viden/lanes/`, and
  ContextBundle/envelope records for delegated work
- selector-first interaction standard for future decision surfaces

The gap is usefulness: the operator should not need to know internal file names,
guess which id to type, or infer whether a lane is still working.

## P0 Scope

### 1. One Reliable Delegated Review Loop

Ship one dependable happy path before broadening integrations:

1. Start with a deterministic shell/template lane as the CI baseline.
2. Support a real Codex read-only review lane when the Codex CLI is installed
   and authenticated.
3. Keep Claude parity as a capability/probe path, not a release blocker.

Acceptance:

- `/lane run <command>` can dispatch, stream/tail evidence, exit, and land in a
  reviewable terminal state.
- `/lane codex-review <task>` or `/agent review codex <task>` produces a
  tracked job/lane with result artifacts and clear next actions.
- Lane status follows a consistent flow:
  `queued -> running -> reviewing -> accepted/applied/discarded/failed/blocked`.
- The TUI shows the same active delegated task in `NOW WORKING`, the right
  rail, side-1, side-2, and lane detail.

### 2. Selector-First Lane And Agent Decisions

Lane and agent operations must follow the interaction rule introduced after
`0.1.18`: if the user must choose an id or action, show a selector instead of
only printing text.

Acceptance:

- `/provider` is a provider configuration selector: key env status, endpoint
  source, provider doctor, and known model candidates. It must not mix provider
  configuration with cross-provider model selection.
- `/models` is the grouped cross-provider model selector. Selecting a row can
  switch provider plus model; `/model <model>` remains current-provider only.
- `/lane` root opens an actionable command picker.
- `/lane inspect`, `/lane timeline`, `/lane diff`, `/lane artifacts`,
  `/lane accept`, `/lane apply`, `/lane discard`, `/lane retry`,
  `/lane stop`, `/lane cleanup`, and `/lane archive` suggest/select live lane
  ids with status, age, and next action.
- `/agent status`, `/agent result`, and `/agent cancel` suggest/select tracked
  external-agent job ids.
- Mouse click, arrow keys, Enter, and Esc all work on the selector.

### 3. Operator State Visibility

When the user submits a task, Viden must say what is happening now.

Acceptance:

- The main screen center shows the highest-priority active task:
  provider thinking, tool call, shell/test, approval, lane running, lane
  reviewing, apply conflict, or model/setup blocker.
- The status includes elapsed time, transport, task owner, and a short phase
  label such as `Codex review running`, `waiting for approval`, or
  `lane output ready for review`.
- Background lane count is visible without stealing focus from the active
  foreground turn.
- Empty or idle states stay quiet and do not look broken.

### 4. Evidence, Apply, And Cleanup

The user needs to trust delegated output before it touches the main workspace.

Acceptance:

- Side-1 shows lane console state, tail, attach command, pid/session, and
  transport health.
- Side-2 shows artifacts, changed files, context pressure, decision file,
  diff/test evidence, and apply/conflict state.
- `/lane accept <id>` records an explicit decision artifact.
- `/lane apply <id>` applies isolated lane changes only after acceptance when
  the lane used an isolated worktree.
- Conflicts produce a visible blocked state with `/lane resolve <id>` and
  `/lane discard <id>` next actions.
- `/lane cleanup <id>` is safe, explicit, and preserves the audit trail needed
  for the release evidence.

### 5. Capability Doctor For Delegated Tools

The product should explain why a delegated lane cannot run.

Acceptance:

- `/agent doctor` or `/lane doctor` reports Codex, Claude, tmux, shell template,
  worktree, Git, and auth/config capability status.
- Missing binaries and unauthenticated CLIs produce fix hints.
- Live Codex/Claude execution is optional in CI, but deterministic probes and
  fixtures must cover the state transitions.

### 6. Real-Use Screenshot Set

Every P0 feature needs visual evidence:

- lane command selector
- active delegated lane in `NOW WORKING`
- side-1 lane console/tail
- side-2 evidence/artifacts/context pressure
- Codex review result or deterministic shell/template review result
- accept/apply/discard decision state
- conflict or blocked state if feasible
- final cleaned-up state

## P1 Scope

- Claude lane happy-path parity after the Codex/shell loop is stable.
- Better tmux/PTY attach ergonomics and lane input forwarding.
- ACP descriptor/probe mapping only, without mutating runtime integration.
- Provider-side ContextBundle pressure improvements for delegated lanes.
- More visual polish for side-screen density once the workflow is reliable.

## Non-Goals

- Do not make Codex or Claude write-capable by default.
- Do not ship a broad ACP runtime in this release.
- Do not add a plugin marketplace or mutating MCP/skill runtime.
- Do not build a full terminal emulator inside the TUI.
- Do not treat screenshots as a substitute for passing smoke tests.

## Test Plan

Focused:

- lane lifecycle reducer and status mapping tests
- `AgentTask` projection tests for provider/tool/shell/lane/external-agent jobs
- selector tests for lane ids and agent job ids
- side-1 and side-2 render tests for shared lane evidence
- ContextBundle/envelope tests for delegated lane sources, token estimate, and
  long-output summary plus tail
- accept/apply/discard/retry/stop/cleanup command tests
- capability doctor tests for missing and present external tools

Regression:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.19 --quick --out-dir /tmp/viden-0119-release-smoke-local
```

New smoke:

```bash
scripts/delegated-lane-smoke.sh --out-dir /tmp/viden-0119-delegated-lane-smoke
```

Manual:

- macOS Terminal and iTerm2 TUI run.
- Resize, CJK input, command selector, and mouse selection while a lane is
  active.
- Deterministic shell/template lane full loop.
- Real Codex read-only review lane when the CLI and auth are available.
- Apply/discard/cleanup with screenshots for each user-visible state.

## Release Standard

`0.1.19` is complete only when:

- the delegated lane smoke passes
- screenshots are generated and linked from docs
- README/user guide mention the delegated lane workflow without overstating
  Claude/ACP maturity
- release status documents local RC, GitHub release, Homebrew tap update, and
  post-publish smoke
- GitHub release assets and Homebrew formula are published for `0.1.19`
