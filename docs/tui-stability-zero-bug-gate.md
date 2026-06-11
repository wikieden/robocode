# TUI Stability Zero-Bug Gate

Chinese version: [tui-stability-zero-bug-gate.zh-CN.md](tui-stability-zero-bug-gate.zh-CN.md)

Last updated: 2026-06-09

## Purpose

The final 0.1.x phase must treat TUI stability as the highest priority.
RoboCode must not enter 0.2.x with known display drift, locked input, modal
residue, stale state, or scrollback bugs.

"Zero bug" means:

- zero known P0/P1 TUI display and interaction bugs;
- every user-visible TUI feature has screenshot or deterministic preview
  evidence;
- every release-blocking TUI regression has a repeatable test;
- P2 visual issues are documented, clearly downgraded, and do not affect the
  daily coding loop.

## Bug Severity

### P0: Blocks Release Immediately

- Input locks and the user cannot keep typing, exit, or cancel.
- Provider turns, Plan mode, approval, doctor, lanes, context building, or tool
  jobs block the main UI.
- Approval modal cannot be operated, cannot disappear, or leaves the wrong
  accepted/denied state.
- Resize makes the layout unusable or misaligns borders enough to make content
  unreadable.
- Streaming steals scrollback so history cannot be reviewed.
- Status bar, right rail, or side screens show fake data or stale blocking
  state.
- A completed task still appears as running, thinking, or waiting for approval.

### P1: Must Be Zero Before 0.1.x Final

- Caret position is wrong, or IME candidate windows are visibly detached from
  the input area.
- Command palette, provider/model picker, settings/setup modal, or completion
  popup covers input or detaches from the composer.
- Main screen and side panels show different status for the same task.
- Borders, vertical lines, separators, or colors visibly drift at common
  terminal sizes.
- Welcome/config flow transitions incorrectly: provider/model setup starts a
  session, or real work fails to leave welcome.
- Error hints are too intrusive, cover primary content, or lack a concrete
  recovery action.

### P2: Can Ship With Explicit Notes

- Minor spacing differences that do not affect readability.
- Non-blocking visual deviations in uncommon terminal/font combinations.
- Low-priority copy polish that does not affect workflow decisions.

## 0.1.x Final Exit Criteria

Before the final 0.1.x release is declared complete:

- P0/P1 TUI bug backlog is zero.
- `scripts/tui-regression.sh docs/previews/generated` passes.
- `scripts/rc-tui-stability-smoke.sh` passes and records the P0/P1 backlog
  summary.
- `scripts/plan-mode-smoke.sh` passes.
- `scripts/daily-loop-smoke.sh` passes.
- `scripts/final-zero-bug-smoke.sh` passes with
  `ROBOCODE_TUI_MANUAL_EVIDENCE_DIR` pointing at real macOS Terminal and iTerm2
  screenshots.
- fake slow provider non-blocking TUI smoke passes.
- deterministic approval non-blocking smoke passes.
- streaming scrollback smoke passes.
- provider/model setup smoke passes.
- manual screenshot acceptance covers at least macOS Terminal and iTerm2.
- every core TUI state has screenshot evidence: welcome, main idle,
  thinking/streaming, approval, provider setup, model picker, command palette,
  side-1, side-2, error recovery, and post-resize layout.
- release status lists the TUI bug backlog, screenshot paths, failing cases,
  known P2 issues, and remaining risks.

## Suggested Version Cadence

- `0.1.24`: Start the non-blocking main-loop gate. Fix root causes for Plan
  mode, approval, streaming, provider turn input lock.
- `0.1.25`: TUI display cleanup. Focus on borders, vertical lines, colors, IME,
  cursor, modal position, right rail drift, and popup placement.
- `0.1.26`: TUI regression pack. Convert historical display bugs into
  deterministic previews, terminal smoke, or manual screenshot checklists.
- `0.1.27`: Daily coding loop hardening. Validate input, approval, tests, diff,
  error recovery, scrollback, and provider setup with real development tasks.
- `0.1.28`: Delegated lane visibility cleanup. Ensure side screens, lane
  evidence, and Codex/Claude/shell job status are consistent and not fake.
- `0.1.29`: 0.1.x RC stabilization. Stop expanding new UI surfaces and fix only
  P0/P1 TUI bugs.
- `0.1.30`: 0.1.x final zero-bug gate. Enter 0.2.x only after P0/P1 are zero.

## Execution Rules

- Late 0.1.x must not sacrifice TUI stability for new agent surface area.
- Every TUI bug needs reproduction steps, severity, screenshot or transcript,
  fix PR/commit, and verification evidence.
- Every bug fix should follow TDD first: add the failing test or deterministic
  preview before implementation.
- The 0.1.x RC gate is `scripts/rc-tui-stability-smoke.sh`; do not remove it
  from release smoke while TUI stability is the release blocker.
- The 0.1.x final gate is `scripts/final-zero-bug-smoke.sh`; `0.1.30`
  prepublish release-gate runs it automatically and treats missing manual
  Terminal/iTerm2 evidence as a release blocker.
- If the bug cannot be automated, add a manual checklist and real terminal
  screenshot.
- Known display errors are not "polish" when they affect judgment, input,
  approval, scrolling, or status understanding; they are P0/P1.

## Active Regression Notes

- 2026-06-08: Long-running coding sessions can expose terminal repaint drift
  after sleep/focus/idle: the dirty-row cache may believe the full screen is
  still present while the emulator has lost rows, and terminal protocol tails
  such as `2;28;95;132m` can appear in the composer. Guardrail: force periodic
  full redraws during TUI operation and filter ANSI/mouse residue before it is
  rendered as user input. Verification should include focused terminal/app
  tests plus TUI regression output.
- 2026-06-09: Focus, paste, and SGR mouse events must not be silent from the
  renderer's point of view. Guardrail: focus/paste events trigger a repaint
  without becoming composer text, SGR mouse residues ending in `m` or `M` are
  discarded, and welcome-screen interaction modals clear the full frame because
  the welcome layout has no right rail. Verification should include app event
  policy tests, composer residue tests, render modal tests, and preview output.
- 2026-06-09: Synthetic inline activity must not outlive the transcript event
  that created it. Guardrail: `latest user turn` can render as planning only
  while the user message is still the latest transcript entry, or when a real
  pending/streaming/runtime task exists. A following tool result, system event,
  or assistant entry must clear the synthetic planning row. Verification should
  include a render test for `user -> tool-result(exit status 1)`.
- 2026-06-09: Active thinking indicators must be obvious without blocking
  input. Guardrail: active work renders as a `LIVE WORK` strip under the latest
  visible conversation, with phase, signal, and next-action guidance; provider
  thinking does not show fake progress percentages. Verification should cover
  provider turns, lane/tool activity, diff review actions, conflict blockers,
  and preview output.
