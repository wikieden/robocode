# TUI Interaction Audit - 2026-05-29

Chinese version: [tui-interaction-audit-2026-05-29.zh-CN.md](tui-interaction-audit-2026-05-29.zh-CN.md)

## Scope

This audit focuses on real operator friction rather than screenshot appearance:
input, command palette, mouse handling, modal behavior, redraw stability, and
the "what is working right now" signal.

## Findings

### Fixed In This Pass

- Main TUI enabled mouse capture but discarded `Event::Mouse` outside the
  approval loop. The slash command palette now supports left-click selection
  and completion for visible rows.
- The composer advertised `Ctrl-K`, `Ctrl-R`, `Ctrl-N`, and `?`, but the main
  event loop only implemented typing, arrows, tab, enter, escape, and theme
  cycling. These shortcuts now have concrete behavior:
  - `Ctrl-K`: clear composer.
  - `Ctrl-R`: reload the latest user prompt for regeneration.
  - `Ctrl-N`: start `/task add ...`.
  - `?`: open help from an empty composer.
- `Ctrl-J` is now treated as an explicit send key, matching the footer action.
- Semantic highlighting used single-letter `E` / `W` metric spans, which could
  color ordinary words letter-by-letter. Those broad single-letter spans are
  removed so labels and panel borders stay visually stable.

### Still Risky

- Mouse support is still narrow. It covers approval and command suggestions,
  but not right-rail task selection, lane modal controls, transcript links, or
  side-screen panels.
- Cursor blink still relies on terminal cursor style. There is no app-owned
  blink pulse or high-contrast caret fallback for terminals that ignore
  `SetCursorStyle::BlinkingBar`.
- Resize redraw is structurally handled by full redraw on size change, but
  there is no stress test that simulates rapid alternating sizes plus input and
  modal states.

### Updated In 0.1.16 RC

- Provider turns now run behind a worker/channel boundary. The main event loop
  keeps repainting elapsed time, pending state, lane snapshots, and approval
  prompts while the provider worker runs.
- Approval prompts are bridged back to the UI and still resolve through the
  existing permission path.
- Command suggestions now use a visible scroll window for long lists, keep the
  selected row visible, and map mouse hits through the visible window.
- Approval `Diff` focus now renders prompt evidence/preview lines when the
  approval prompt carries them.

### Remaining Interaction Backlog

- True cancellation remains best-effort. The UI can request cancellation, but
  an already in-flight provider request may still complete before the worker
  observes the request.
- Provider token streaming is still a future feature; `0.1.16` makes the shell
  responsive during a turn, not a full streaming renderer.
- Mouse coverage should expand next to right-rail task selection, lane modal
  controls, side-screen scrolling, transcript links, and wheel events.
- Cursor blink and IME placement still depend partly on terminal behavior.
  Viden should add an app-owned high-contrast caret fallback if more
  terminals fail to render the native blinking bar.

## Recommended Next Slice

This slice is now scoped as `0.1.16`:
TUI Interaction Reliability. It should land before lightweight spec/steering
work because a larger workflow surface will amplify these interaction problems
instead of hiding them.

1. Move provider turns to a background worker channel so the TUI keeps
   repainting `NOW WORKING`, elapsed time, cancellation affordances, and
   streaming status.
2. Add a central interaction router with explicit focus targets:
   `composer`, `palette`, `approval`, `lane-detail`, `right-rail`, and
   `side-screen`.
3. Expand mouse hit testing from command palette to right rail, lane controls,
   and approval diff.
4. Add terminal-interaction regression scenarios for rapid resize, mouse
   selection, shortcut keys, and pending-turn repaint cadence.
5. Replace fake-looking footer actions with either implemented actions or
   removed text. Footer promises should be executable.

Detailed release requirements live in
[Viden 0.1.16 Plan](release-0.1.16-plan.md).
