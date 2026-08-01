# Design QA — Compact New Lane Popover

## Scope

This QA covers the latest supplied target for GUI Lane creation: a compact
popover anchored to `+ New Lane`, Agent selection, task entry, isolation
preview, `Full setup…`, and one primary create action.

## Source and implementation

- Latest reference: `/var/folders/rm/hlc2td2x1rq11k_yknfsn1y80000gn/T/codex-clipboard-2dce13da-5348-45e6-90e1-bfa81acf2347.png`
- Focused reference crop: `apps/gui/evidence/0.1.0-rc.3/lane-agent-popover-reference.png`
- Browser-rendered implementation: `apps/gui/evidence/0.1.0-rc.3/lane-agent-popover-implementation.png`
- Same-input comparison: `apps/gui/evidence/0.1.0-rc.3/lane-agent-popover-comparison.png`

The two 600 × 650 comparison regions preserve aspect ratio and use padding,
not stretching. The implementation also passed native macOS accessibility
inspection and exposes a dialog, radio group, task textbox, and create action.

## Interaction checks

1. The built-in Viden Agent is selected by default; ready ACP adapters retain
   stable product ordering and real registered brand assets.
2. The task field receives initial focus, supports CJK composition, and submits
   with Cmd/Ctrl+Enter only after composition ends.
3. Create remains disabled until the selected Agent is startable and the task
   is non-empty.
4. Git projects preview `vd/<task-slug>` plus local isolation; non-Git folders
   state that the opened workspace is used directly.
5. Create uses the existing ordered Core flow: preview, create/approval, exact
   Lane projection, then native submit or ACP session start.
6. `Full setup…` routes to D4 rather than adding advanced controls to the quick
   popover.

## Findings

- P0: none.
- P1: none.
- P2: none.
- P3: the reference includes `Attach tmux…`, but the current Core contract does
  not publish a startable tmux Lane adapter. The GUI omits that action instead
  of presenting a control that cannot complete.
- P3: the reference says `Global · cross-project`; current Core state is scoped
  to the opened project, so the implementation truthfully labels it `Current
  project` until a cross-project scope contract exists.

## Final result

passed
