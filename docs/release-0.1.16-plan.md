# Viden 0.1.16 Plan - TUI Interaction Reliability

Chinese version: [release-0.1.16-plan.zh-CN.md](release-0.1.16-plan.zh-CN.md)

## Summary

`0.1.16` is inserted before the lightweight spec/steering workflow because the
current cockpit still has interaction debt that directly affects programming
confidence. The release goal is:

> After the user submits input, Viden must always show what is happening,
> keep the terminal responsive, and make every visible action executable.

This is an operator-loop reliability release, not a visual redesign. It keeps
the current no-modal visual theme and improves the parts that felt unreliable
in real use: pending provider work, focus, input, command suggestions, approval
modal control, mouse hit testing, resize redraw, and footer affordances.

## Product Goals

- Make remote/provider work observable while it is running.
- Keep the TUI repainting during long provider/tool turns.
- Make focus explicit so keyboard and mouse actions route predictably.
- Ensure every advertised footer action has a real behavior.
- Reduce modal friction: approval should be keyboard-first, mouse-capable, and
  dismiss itself after a decision.
- Preserve CJK/IME input stability and a visible caret in common terminals.
- Add deterministic and manual interaction tests before moving into spec or
  broader orchestration features.

## P0 Scope

### 1. Non-Blocking Provider Turn Shell

Problem:
Provider requests currently run synchronously through the main TUI path. The
screen can show `pending_turn` before the call begins, but elapsed time,
`NOW WORKING`, cancellation affordances, and streaming-like status cannot
repaint while the provider call blocks.

Requirements:

- Move provider turns behind an event/channel boundary so the TUI event loop can
  continue ticking and redrawing.
- Show a central working-state row/card with:
  - current operation, for example `Thinking`, `Calling tool`, `Waiting for approval`, or `Running shell`.
  - elapsed time.
  - provider/model.
  - active task or lane id when known.
  - next safe action, for example `wait`, `approve`, `deny`, `inspect`, or `cancel`.
- Bridge permission approval requests from the worker back to the UI without
  bypassing the existing permission path.
- Record worker state into the shared runtime snapshot instead of inventing a
  TUI-only status store.
- If true cancellation is too invasive for this release, show cancellation as
  unavailable instead of advertising a fake action.

Acceptance:

- A long fallback/provider turn does not freeze clock/status repaint.
- Approval requests still require the same permission decision.
- The main cockpit always answers "what is happening now?" while a turn is
  active.

### 2. Interaction Router And Focus Model

Problem:
Input handling is still scattered across composer, command palette, approvals,
lanes, side screens, and modal paths. This causes surprises such as shortcuts
working in one state but not another.

Requirements:

- Introduce an explicit `TuiFocus` model for:
  - `composer`
  - `command_palette`
  - `approval`
  - `lane_detail`
  - `right_rail`
  - `side_screen`
- Route keyboard and mouse events through one interaction dispatcher.
- Keep current composer shortcuts:
  - `Enter` / `Ctrl-J`: send.
  - `Ctrl-K`: clear.
  - `Ctrl-R`: reload latest user prompt.
  - `Ctrl-N`: start `/task add ...`.
  - `?`: help only when composer is empty.
- Document focus transitions in code comments where the state machine is not
  obvious.

Acceptance:

- Unit tests cover focus transitions and shortcut behavior by focus target.
- Footer actions only appear when valid for the current focus.

### 3. Command Palette Parity

Problem:
The slash-command palette is improving, but long lists and mouse behavior still
do not feel complete.

Requirements:

- Add a visible scroll window for command suggestions longer than the rendered
  rows.
- Keep selected item visible when moving by keyboard.
- Support mouse left-click selection/completion for visible suggestions.
- Prevent palette rows from rendering over the composer or terminal IME region.
- Keep command descriptions concise enough for narrow terminals.

Acceptance:

- Deterministic screenshot: command palette with enough commands to prove scroll
  behavior.
- Unit tests for visible-window math and mouse hit testing.

### 4. Approval Modal Control

Problem:
Approval dialogs are central to trust, but the current modal can feel stuck:
focus is unclear, `Diff` does not pay off enough, and mouse/keyboard control is
incomplete.

Requirements:

- Default selected action remains `Approve`.
- Keyboard:
  - `y`: approve.
  - `n`: deny.
  - `d`: open or focus a real diff/evidence view.
  - `Tab` / arrows: move action focus.
  - `Esc`: close only when safe; otherwise show why a decision is required.
- Mouse:
  - left-click `Approve`, `Deny`, `Diff`, and checkbox regions.
  - after a final decision, remove the modal immediately.
- Diff:
  - show either a real inline diff/evidence view or remove the fake affordance.

Acceptance:

- Modal screenshot evidence for default-approve, diff/evidence, and post-action
  cleared state.
- Tests cover keyboard and mouse action mapping.

### 5. Resize, Caret, And IME Stability

Problem:
The cockpit is visually dense. Misaligned borders, stale redraw artifacts,
invisible caret, and IME candidate windows far from the input area immediately
hurt confidence.

Requirements:

- Add a deterministic rapid-resize regression scenario that exercises:
  - idle main screen.
  - command palette.
  - approval modal.
  - active provider turn.
  - CJK input.
- Keep composer height large enough for readable input and IME placement.
- Add a high-contrast caret fallback or app-owned pulse when terminal cursor
  blinking is not reliable.
- Keep border colors consistent on the right rail and side screens.
- Avoid semantic highlighter spans that color letters inside ordinary words.

Acceptance:

- Terminal and iTerm2 manual screenshots show readable input/caret placement.
- Regression artifacts prove borders stay aligned after resize.

### 6. Footer Promise Audit

Problem:
Footer actions that look clickable or keyboard-addressable but do nothing make
the whole TUI feel fake.

Requirements:

- Audit every footer label in main, side-1, side-2, lane detail, command
  palette, and approval states.
- Implement the action, hide the action, or mark it as unavailable with a clear
  reason.
- Add tests for all implemented global shortcuts.

Acceptance:

- Documentation and tests list the supported controls.
- No visible footer action is knowingly fake.

## P1 Scope

- Mouse selection for right-rail tasks, recent files, diagnostics, and provider
  health rows.
- Mouse wheel support for transcript, palette, and side panels.
- Real provider streaming display where the provider supports it.
- App-level cancel/interrupt that can stop a running provider/tool turn when the
  underlying operation supports cancellation.
- Focus breadcrumbs for multi-screen operation.

## Explicit Non-Goals

- Do not implement the lightweight spec/steering workflow in this version; move
  it to `0.1.17`.
- Do not add new external-agent adapter breadth.
- Do not introduce a plugin marketplace or mutating MCP/ACP runtime.
- Do not redesign the no-modal visual theme; keep visual changes tied to
  interaction clarity.

## Test Plan

Focused automated checks:

- `cargo test -p viden-cli tui::app --quiet`
- `cargo test -p viden-cli tui::command_palette --quiet`
- `cargo test -p viden-cli tui::terminal --quiet`
- new focus-router tests for key and mouse routing
- new provider-worker tests for pending, approval, completion, and failure
  events
- new palette scroll-window tests

Regression checks:

- `cargo fmt --check`
- `cargo clippy -p viden-cli --all-targets -- -D warnings`
- `cargo test -p viden-cli --quiet -- --test-threads=1`
- `cargo test --workspace --quiet`
- `scripts/tui-regression.sh docs/previews/generated`
- new or expanded interaction smoke for rapid resize, pending turn repaint,
  modal action mapping, and command-palette mouse behavior

Manual acceptance:

- macOS Terminal and iTerm2:
  - normal input.
  - CJK/IME input.
  - command palette mouse and keyboard.
  - approval modal keyboard and mouse.
  - resize while a turn is active.
  - side-1 and side-2 visual consistency.
- Provide real screenshots for each user-visible acceptance state before
  marking the version complete.

## Release Evidence

Required screenshots or deterministic visual artifacts:

- `0.1.16-tui-working-state`
- `0.1.16-tui-command-palette-scroll`
- `0.1.16-tui-approval-default-approve`
- `0.1.16-tui-approval-diff`
- `0.1.16-tui-cjk-caret`
- `0.1.16-tui-resize-active-turn`
- `0.1.16-tui-side-rail-consistency`

## Documentation Updates

- Update README controls after implementation.
- Update user guide controls and troubleshooting.
- Update TUI cockpit design with focus and mouse rules.
- Update staged and long-term roadmaps.
- Add `release-0.1.16-status` when the local RC exists.

## Open Risks

- Moving provider turns off the main TUI path is the highest-risk change because
  approval decisions must remain synchronous from the tool loop's perspective.
- True cancellation may require deeper provider/tool runtime changes and can be
  deferred if the UI does not advertise it as working.
- IME candidate-window placement depends partly on terminal behavior; Viden
  can improve composer geometry and caret placement, but cannot fully control
  every terminal's native IME UI.
