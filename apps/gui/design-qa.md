# D1 Persistent Shell Design QA

## Scope

- Implementation: `apps/gui/src/screens/d1_cockpit.ts`,
  `apps/gui/src/screens/d1_cockpit.css`, and the shell presentation components
  under `apps/gui/src/components/`.
- Design source package: `docs/viden-design/Viden/GUI/`, including
  `gui-icons.jsx` and `gui-kit.css`.
- Exact accepted-worktree source truth capture:
  `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-acceptance/apps/gui/evidence/design-qa/d1-target-dark-cockpit.png`.
- State under review: an active Lane with streaming work, a pending high-risk
  permission request, a composer, a status bar, and a typed Context Dock.
- This pass judges the persistent shell only. Transcript-card fidelity and the
  expanded Context Dock data surface remain owned by the later D1 work-surface
  and context tasks.

## Capture normalization

The source truth capture is 5140 by 2650 pixels at 144 DPI. The implementation
source capture is `d1-shell-2570x1325-css.png`, captured by the browser at 2570
by 1325 CSS pixels. `d1-shell-5140x2650.png` is the normalized implementation:
the browser capture scaled at 2x to 5140 by 2650 for comparison. This preserves
the source truth's effective device scale without changing implementation
layout proportions.

Evidence:

- `evidence/task-5-shell/d1-shell-5140x2650.png`
- `evidence/task-5-shell/d1-shell-2570x1325-css.png`
- `evidence/task-5-shell/d1-shell-1440x900.png`
- `evidence/task-5-shell/d1-shell-1280x800.png`
- `evidence/task-5-shell/d1-shell-960x640.png`
- `evidence/task-5-shell/d1-shell-960x640-drawer-open.png`
- `evidence/task-5-shell/d1-shell-1440x900-lane-rail-keyboard-open.png`
- `evidence/task-5-shell/d1-shell-target-vs-implementation.png`
- `evidence/task-5-shell/d1-shell-focus-topbar-activity.png`
- `evidence/task-5-shell/d1-shell-focus-work-surface.png`
- `evidence/task-5-shell/d1-shell-focus-context-dock.png`
- `evidence/task-5-shell/d1-shell-focus-permission-composer-status.png`

## Visual and interaction review

- Geometry: the production shell uses the canonical 46 px title bar, 52 px
  activity rail, 218 px floating Lane rail, 300 px Context Dock, and 28 px
  status bar. The 2570 by 1325 capture has no document overflow.
- Responsive behavior: at 1440 and 1280 pixels the Context Dock remains a
  persistent third column. At 960 pixels it becomes a right-side drawer while
  preserving the work surface.
- Activity rail: the seven destinations use the exact registered path data
  from `GUI/gui-icons.jsx`; no emoji, generated artwork, or prototype runtime
  is loaded by production.
- Lane rail: the default is the accepted floating treatment, not a permanently
  pinned column. Hover and keyboard focus reveal the rail without changing Core
  facts.
- Assets and styling: the title bar uses the canonical Viden brand asset and
  GUI token package. The shell is dark from the native window edge with no
  browser-like white frame.
- Accessibility: the shell regions have stable landmarks and labels. The
  narrow Context Dock drawer is opened by both pointer click and keyboard
  Enter, reports `aria-expanded`, and receives focus. The floating Lane rail
  has a real `aria-controls` / `aria-expanded` activity button: Tab reaches it,
  Enter or Space opens the rail and focuses `+ New Lane`, and Escape closes the
  rail and restores focus to the activity button.
- Runtime truth: shell text and dock facts come from the typed D1 projection.
  Unavailable features remain explicitly unavailable; the QA harness does not
  add a production fake-data route.
- Browser console: the Browser-controlled Chrome run recorded Vite connection
  debug messages only, with no warning or error messages.

## Iterations

1. The first combined comparison found two P2 shell defects: the Lane rail was
   pinned instead of floating, and the activity rail exposed only three of the
   seven canonical destinations. It also found one-pixel box-model drift in the
   shell rails and status bar.
2. The implementation switched the Lane rail to the accepted floating
   interaction, restored the seven registered activity icons in design order,
   and made shell dimensions border-box exact. The regenerated full and focused
   comparisons contain no actionable P0, P1, or P2 issue within this task's
   persistent-shell scope.
3. Review found the visually hidden Lane rail had no keyboard entry path
   because CSS `visibility: hidden` removed its controls from sequential focus.
   The canonical Lanes activity item is now an operable disclosure control.
   Browser-controlled Chrome verified the complete Tab, Enter, focused open
   state, Escape, and restored-focus sequence. The open-state capture above and
   the unchanged full comparison show no new geometry, token, or asset mismatch;
   no actionable P0, P1, or P2 remains.

The in-app browser connection was unavailable in this environment. Per the
approved fallback, the same browser QA protocol was completed through
Browser-controlled Chrome against the production screen module and CSS.

## Task 6 — D1 work and approval flow

- Target: `apps/gui/evidence/design-qa/d1-target-dark-cockpit.png` from the
  accepted D1 cockpit worktree.
- Harness: `apps/gui/evidence/task-6-work-surface/qa.html` imports the
  production D1 screen and typed fixture. It does not add a production
  fake-data route.
- Full comparison:
  `apps/gui/evidence/task-6-work-surface/d1-target-vs-implementation.png`.
- Focused comparisons:
  `apps/gui/evidence/task-6-work-surface/d1-center-target-vs-implementation.png`
  and
  `apps/gui/evidence/task-6-work-surface/d1-permission-composer-target-vs-implementation.png`.
- Responsive evidence:
  `d1-work-1440x900.png`, `d1-work-1280x800.png`, and
  `d1-work-960x640.png`. Chrome reported the same client dimensions before
  each capture.
- Density normalization: the 2570×1325 CSS-pixel capture was scaled to
  5140×2650 physical pixels before the accepted 5140×2650 target comparison.
  The raw CSS-density, normalized, and direct 5140×2650 captures are preserved.
- State fidelity: the selected Lane owns the transcript, permission request,
  Live Work, environment, composer, and cancel action. Missing Core facts remain
  typed unavailable states with paired GUI-CORE contract requests.
- Approval interaction: Browser-controlled Chrome exercised Once, Session,
  Always, Edit, and Deny against request
  `permission-shell-high-risk`. The harness recorded each exact choice.
- Keyboard and composer interaction: Tab moved focus from Once to Session;
  `d1-work-1440x900-permission-focus.png` preserves the focus state. Chrome
  submitted CJK multiline text while the approval was present, queued it for
  `lane-core`, and dispatched cancel for that same Lane. A separate idle run
  verified Enter submits immediately. Automated composer coverage verifies IME
  composition, paste, undo, multiline editing, and approval-safe editing.
- Browser console: fresh busy and idle Chrome sessions recorded Vite connection
  debug messages only, with no warning or error messages.

### Task 6 iterations

1. The first target comparison found P2 density drift in the center cards and
   Live Work, plus a missing visible Send action, flattened Live Work hierarchy,
   and approval labels that did not match the accepted interaction language.
2. The work surface was tightened to the accepted density, restored the visible
   Send action and Once/Session/Always/Edit/Deny labels, and rendered the
   three-level Live Work hierarchy with registered tokens only.
3. Independent contract review found cross-Lane permission and transcript
   projection risks, weak ACP binding cardinality checks, a broad busy-state
   inference, and a per-collection rather than shared Live Work cap. The
   implementation now fails closed on exact Lane/session ownership, resets and
   waits on Lane switches, uses the authoritative turn owner, and enforces one
   shared 24-item cap.
4. The regenerated full and focused comparisons and the 1440×900, 1280×800,
   and 960×640 captures contain no actionable P0, P1, or P2 issue in Task 6
   scope.

final result: passed
