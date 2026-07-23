# Task 7 Context Dock QA

Date: 2026-07-24
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-gui`
Branch: `codex/d1-cockpit-gui`
Base: `6877c77b3db4524d837f22a6d0dc21dd7641b396`
Accepted target:
`/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-acceptance/apps/gui/evidence/design-qa/d1-target-dark-cockpit.png`

## Automated Contract Evidence

- RED: `npm --prefix apps/gui test -- --run tests/d1_cockpit.spec.ts -t "one-Agent Context Dock"` failed because the current dock exposed no `data-context-section` order.
- RED amendment: `npm --prefix apps/gui test -- --run tests/d1_cockpit.spec.ts -t "localizes non-empty zh-CN Context Dock"` failed because non-empty zh-CN Context Dock facts still exposed hardcoded `Lane`, `Ahead`, `Behind`, `Dirty`, `Budget`, `Remaining`, and raw checklist status copy.
- GREEN focused: `npm --prefix apps/gui test -- --run tests/d1_cockpit.spec.ts tests/d11_intake.spec.ts tests/i18n_parity.spec.ts` passed, 73 tests.
- Full GUI web: `npm --prefix apps/gui test -- --run` passed, 137 tests.
- Build: `npm --prefix apps/gui run build` passed.
- Rust GUI: `cargo test -p viden-gui` passed.
- Hygiene: `cargo fmt --all -- --check` and `git diff --check` passed.
- Source sweep: `rg -n "Subagents|subagents|agent_session_switcher|GUI-CORE-" apps/gui/src` reports only `apps/gui/src/preferences.ts:82`, a structured diagnostic key for `GUI-CORE-005`.

## Browser Visual QA

The parent task owns the four-size in-app Browser visual QA after this amended
build. This amendment did not capture new screenshots locally.

## Workspace Classification

`cargo test --workspace --quiet` failed before any GUI failure, in `apps/tui/**`, with inherited Core/TUI contract drift:

- unresolved `viden_core` exports such as `SessionEngine`, `ModelRequestControl`, `ProviderAuthMode`, `EngineEvent`, and provider descriptors;
- TUI references to removed or changed `AgentTaskRecord` / `AgentLaneRecord` fields such as `agent`, `screen`, and `transport`;
- TUI string comparisons against typed `AgentTaskKind`, `AgentTaskStatus`, and `LaneStatus` enums;
- obsolete `ApprovalResponse { approved }` field usage.

No Core/TUI files were changed for this GUI-owned task.
