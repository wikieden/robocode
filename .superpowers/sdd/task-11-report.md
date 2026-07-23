# Task 11 Report — D1 New Lane Popover Loop

## Status

DONE_WITH_CONCERNS

## Commit And HEAD

- Branch: `codex/d1-lane-popover-loop`
- Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-lane-popover-loop`
- Implementation commit: `407f306707148e44b62a12a3db37cb4d765339c2`
- HEAD when report was written: `407f306707148e44b62a12a3db37cb4d765339c2`

## Files Changed

- `apps/gui/src/components/agent_menu.ts`
- `apps/gui/src/components/agent_menu.css`
- `apps/gui/src/components/lane_task_prompt.ts` removed
- `apps/gui/src/components/lane_task_prompt.css` removed
- `apps/gui/src/screens/d1_cockpit.ts`
- `apps/gui/src/i18n/catalog.ts`
- `apps/gui/src/i18n/en.json`
- `apps/gui/src/i18n/zh-CN.json`
- `apps/gui/tests/agent_menu.spec.ts`
- `apps/gui/tests/d1_cockpit.spec.ts`
- `apps/gui/README.md`
- `apps/gui/README.zh-CN.md`

## Red Evidence

- `npm --prefix apps/gui test -- --run tests/agent_menu.spec.ts` failed as expected before production changes.
- Failure: `keeps Agent selection, task draft, and Create in one focused popover` reported `expected null not to be null` for `[data-lane-task]`.
- Meaning: current production rendered only the compact Agent menu and had no task textarea in the same popover, matching the two-stage gap.

## Green Evidence

- `npm --prefix apps/gui test -- --run tests/agent_menu.spec.ts tests/d1_cockpit.spec.ts tests/i18n_parity.spec.ts` -> 3 files passed, 67 tests passed.
- `npm --prefix apps/gui test -- --run` -> 17 files passed, 241 tests passed.
- `npm --prefix apps/gui run build` -> `tsc --noEmit && vite build` succeeded.
- `cargo test -p viden-gui` -> all GUI Rust unit, integration, and doc tests passed.
- `scripts/check-doc-pairs.sh apps/gui/README.md apps/gui/README.zh-CN.md` -> passed.
- `git diff --check` -> passed.

## Behavior And Contract Impact

- Replaced the Agent-menu -> task-dialog loop with one anchored New Lane popover.
- The popover contains the New Lane heading, built-in Viden Agent, discovered ACP Agents, one selected Agent, focused task textarea, disabled-until-non-empty Create Lane action, Core eligibility/probe diagnostics, and a presentation-only branch/worktree hint derived from task text.
- Agent selection stays inside the popover and updates the selected Agent instead of closing.
- Create dispatches the existing ordered flow: `preview_default_lane`, `create_starter_lane`, then native `submit` or ACP `start_agent_session` after the exact Core Lane is projected.
- Core/transport rejection preserves the task draft and exposes the existing D1 typed rejection surface.
- ACP discovery serialization remains in D1: native Viden stays selectable while probes run, and ACP rows remain probe-gated.
- CJK IME composition is preserved for Cmd/Ctrl+Enter create.
- The removed second task prompt had no remaining callers.

## Docs And Comments Decision

- Updated `apps/gui/README.md` and `apps/gui/README.zh-CN.md` because the D1 New Lane interaction changed.
- Added no new code comments; the changed control flow is covered by focused tests and existing function boundaries.
- No shared design source, Core crate, TUI, manifest, or token source was changed.

## Self-Review Findings

- The first implementation preserved draft state but remounted the popover asynchronously after ordered projection redraws; fixed by synchronous remount.
- A later self-review found rejection could leave Create disabled after a remount with `submitting=true`; fixed by re-rendering after rejected dispatch and added a regression test.
- No unrelated files were staged in the implementation commit.

## Remaining Concerns

- The required report file was written after the implementation commit so it could name the real implementation hash, then committed separately to leave the worktree clean. The behavior checkpoint remains `407f306707148e44b62a12a3db37cb4d765339c2`; the report-only commit is not part of the GUI runtime change.
