# Viden GUI Track Agent Guide

## Scope

This file applies to `apps/gui/**`. Read the repository-root `AGENTS.md`,
`docs/parallel-development-plan.md`, `docs/gui-version-functional-design.md`,
the GPUI feasibility research, and the current design sources under
`docs/viden-design/Viden/GUI/` before implementation.

## Start Gate

For V3 production implementation, the task must name the immutable
`frontend-contract-v1` Core checkpoint SHA and the intended GUI version,
starting with `gui-v0.1.0` for the local cockpit vertical slice.

- Create `codex/v3-gui-client` in `.worktrees/v3-gui-client` from that exact
  checkpoint.
- If no checkpoint SHA is available, remain in interaction/reference review,
  framework-spike planning, and Core contract-gap analysis.
- Confirm GUI interactions and reference screens with the user before building
  the production client.
- Read design in this order: `docs/viden-design/Viden/index.html`, then the GUI
  design index, then `Viden - 桌面驾驶舱 (GUI).html`, then the GUI component
  library. D11, D4, D2, and D6 pages refine flows after the cockpit is
  understood.

## Framework Gate

Keep the branch and product contract framework-neutral until Tauri and GPUI
run the same Core fixture and the same vertical slice:

- theme and density;
- composer and CJK IME;
- streaming and tool rows;
- approval, queue, and cancel;
- history scroll and bounded transcript virtualization.

Use the quantitative and platform criteria in
`docs/parallel-development-plan.md`. A failure in IME, accessibility,
three-platform packaging, bounded transcript behavior, or maintainable
framework integration is a GPUI no-go and selects the Tauri baseline. Record
the decision and update paired GUI documents before creating substantial
framework-specific surface area.

## Ownership And Client Boundary

- Own `apps/gui/**`, GUI adapters/components/screens, platform tests,
  screenshots, and GUI user documentation.
- Depend only on `viden-core` and frontend-neutral contracts.
- Send every mutation as `RuntimeCommand` and wait for `RuntimeEvent`.
- On a sequence gap, recover through snapshot/replay; never continue by
  guessing state.
- Do not access provider, tools, permissions, sessions, workflows, JSONL, or
  SQLite directly.
- Missing fields or commands are Core contract requests. Do not add a
  GUI-private reducer, gate model, cost estimate, or execution path.
- Do not edit `crates/**`, `apps/tui/**`, or shared design tokens without
  coordination with the owning track.

## Product Sequence And Acceptance

Implement the first local operator loop in this order:

1. D11 project intake and first-run setup.
2. D4 lane creation.
3. D1 cockpit.
4. D2 permission/decision slice.
5. D6 empty, disconnected, provider-error, context-overflow, and reconnect
   recovery states.

Use the latest GUI design directory as the only visual reference. Reuse shared
tokens, registered components, and brand assets; prototype Babel runtime, mock
data, and window scaffolding are not production runtime.

Language, locale, skin, mode, density, font scale, and accessibility flags come
from Core presentation preferences. The GUI exposes language and appearance as
configuration options and imports or derives from shared tokens instead of
shipping a second skin palette.

Verify fixture parity with TUI, CJK IME, keyboard-only operation, visible
focus, accessibility semantics, event-to-visible latency, bounded transcript
virtualization, crash/reconnect integrity, screenshots, and all supported
platform build/launch paths. Run `cargo test --workspace --quiet` in addition
to the framework-specific commands recorded on this branch.

The handoff must include the base Core SHA, branch, worktree, HEAD, framework
gate result, fixture parity, performance/accessibility evidence, screenshots,
contract requests, and blockers. Do not merge or push `main` without explicit
user direction.
