# Design

## Source of truth

- Status: Draft
- Last refreshed: 2026-05-23
- Primary product surfaces: Viden terminal TUI, companion terminal workspaces, embedded/attached terminal panes, theme configuration, transcript/task/diagnostic panels.
- Evidence reviewed:
  - `apps/tui/src/tui.rs`: current lightweight alternate-screen TUI.
  - `PLAN.md`: V2-D richer TUI and structured views direction.
  - `docs/staged-roadmap.md`: V2 developer enhancement layer and V3 platform expansion.
  - `README.md` and `README.zh-CN.md`: current `--tui` user-facing entry point.
  - User-provided primary TUI mockup: definitive main-screen visual target with transcript timeline, right workspace/status rail, approval modal, composer, and bottom status bar.
  - `docs/previews/*.png`: generated secondary references for multi-screen, theme variants, and real workstation usage scenes.

## Visual reference mapping

- Primary concept: the user-provided single-screen Viden TUI mockup.
  - Must carry forward: top global status rail with `Viden`, version, provider, model, session, context, Git branch, and permissions mode.
  - Must carry forward: large left transcript timeline with role icons, timestamps, tool-call/result grouping, and dense readable event history.
  - Must carry forward: right workspace rail with `WORKSPACE`, `ACTIVE TASKS`, `LSP DIAGNOSTICS`, `PROVIDER HEALTH`, and `RECENT FILES` panels.
  - Must carry forward: centered approval modal with file path, action, size, preview, apply-to-all checkbox, and Deny/Approve actions.
  - Must carry forward: bottom Composer with mode selector, approval-mode controls, command chips, and a final connection/session/token/cost/time status bar.
  - Must carry forward: dark cyan technical style, thin luminous borders, compact uppercase headers, green/yellow/red status semantics, and high information density.
  - Must not carry forward literally: fake unreadable text, decorative glow that reduces contrast, or any visual element that cannot degrade gracefully in a terminal.
- Secondary multi-screen concept: `docs/previews/tui-theme-variants-v1.png`.
  - Must carry forward for companion screens: left `AGENTS` work wall, right `OPS`/terminal work wall, visible screen registry, thin luminous borders, dense but readable tables, and task/terminal lane ownership.
- Real workstation target: `docs/previews/tui-desk-32-32-27-portrait-v1.png`.
  - Must carry forward: exactly one main 32-inch 16:9 landscape display, one left 32-inch 16:9 landscape companion, and one right 27-inch portrait companion.
  - Layout implication: `MAIN` and `AGENTS` must work well in landscape; `OPS` must have a portrait-first stacked layout.
- Earlier exploration references:
  - `docs/previews/tui-concept-holodeck-v1.png` and `docs/previews/tui-multiscreen-agents-v1.png` define the original stronger sci-fi direction.
  - `docs/previews/tui-deepseek-reference-v1.png` validates the practical DeepSeek-TUI-inspired structure, but it is not the primary visual style.
  - `docs/previews/tui-desk-cyan-v1.png`, `docs/previews/tui-desk-ember-v1.png`, and `docs/previews/tui-desk-workstation-v1.png` validate real-world desk mood and theme variants.

## Brand

- Personality: local-first developer cockpit, precise, fast, calm under pressure, with a restrained high-tech feel.
- Trust signals: auditable transcript, visible mode/provider/model state, explicit permission prompts, clear task status, readable terminal-first density.
- Avoid: marketing-page UI, decorative panels without function, illegible fake data, single-color novelty themes, and visual effects that reduce terminal readability.

## Product goals

- Goals:
  - Make long agent sessions easier to monitor than a plain scrolling terminal.
  - Keep the main coding conversation central while exposing plan, tasks, diagnostics, tools, Git, and provider state.
  - Support one main screen plus up to two dynamically opened companion terminal workspaces.
  - Make companion screens useful work surfaces, not decorative dashboards: they should host sub-agent lanes, terminal panes, and external coding CLIs such as `claude`, `codex`, or other configured tools.
  - Let the user launch, attach, detach, stop, and inspect companion work without leaving the main TUI.
  - Match the approved visual direction: the user-provided single-screen Viden mockup is the primary target; multi-screen images extend it, not replace it.
  - Preserve all existing `SessionEngine`, permission, transcript, and provider invariants.
  - Let users switch among built-in and custom themes without recompiling.
- Non-goals:
  - Do not replace the plain CLI output path.
  - Do not introduce a GUI app, web app, or remote screen protocol in the first TUI pass.
  - Do not fake true multi-agent orchestration inside the TUI before the core coordinator exists; until then, represent external CLI/task lanes as supervised terminal tasks with explicit ownership and logs.
  - Do not bypass transcript JSONL or permission checks for visual convenience.
- Success signals:
  - A user can run `--tui`, send prompts, approve tools, inspect state, and leave safely.
  - A user can open and close companion workspaces while the main session continues.
  - A user can start a bounded task in a side pane, for example "run codex on test fixes" or "open claude in this worktree", watch it progress, and attach to its terminal when needed.
  - The interface remains usable at common terminal sizes and on the target three-monitor setup: 32-inch landscape main, 32-inch landscape companion, 27-inch portrait companion.

## Personas and jobs

- Primary personas:
  - Solo developer using Viden as a local coding agent.
  - Power user running DeepSeek/OpenAI-compatible providers and checking provider/runtime behavior.
  - Future multi-agent user who wants visible child-task progress without losing the main thread.
- User jobs:
  - Give instructions, inspect assistant/tool output, and approve sensitive actions.
  - Track plan steps, active tasks, diagnostics, Git diff, and model/provider health.
  - Move secondary work-heavy surfaces to another physical monitor.
  - Spawn external coding tools or sub-agent-like task lanes for parallel implementation, review, testing, or research work.
  - Attach to a running terminal pane, intervene, then detach without killing the task.
  - Switch visual theme based on lighting, taste, and readability.
- Key contexts of use:
  - Local terminal sessions on macOS first, cross-platform terminal behavior later.
  - Long coding sessions where transcript noise, tool status, and task continuity matter.
  - Real workstation usage with two landscape monitors and one portrait monitor.

## Information architecture

- Primary navigation:
  - Main screen is always the command center.
  - Companion screens are optional workspaces: they can be read-only dashboards, supervised terminal panes, or sub-agent/task lane boards.
  - Screen registry shows `MAIN`, `AGENTS`, and `OPS` with open/closed state, plus open and close controls where the terminal can represent them.
- Core screens:
  - Main: top status rail, transcript timeline, workspace/status right rail, approval modal, Composer, approval-mode controls, and bottom status bar.
  - Agents companion: `AGENTS` work wall with sub-agent/task matrix, queue, blocked work, recent completions, approvals, tool timeline, and attachable task terminals.
  - Ops companion: `OPS` work wall with diagnostics, tests, Git diff, files changed, provider/tool telemetry, command history, memory notes, and attachable terminal panes for external tools.
- Terminal lane model:
  - A terminal lane represents one supervised process or shell session, such as `codex`, `claude`, `cargo test`, `rg`, or a project-specific command.
  - Each lane has an id, title, cwd/worktree, command, owner, status, started time, last activity, transcript/log path, and optional task link.
  - Lanes support `start`, `attach`, `detach`, `stop`, `restart`, and `archive`.
  - Lanes are not allowed to silently mutate project state outside the same permission/worktree policy as Viden.
- External tool orchestration model:
  - Viden talks to external terminal coding tools through tool adapters, not through hidden assumptions about their internals.
  - A tool adapter turns a Viden task envelope into the safest launch method for that tool: command arguments, stdin prompt, temporary prompt file, or interactive PTY input.
  - The lane runtime captures stdout, stderr, PTY transcript, exit code, file changes, and optional structured markers from the external tool.
  - Viden decides next actions from observable evidence: process state, logs, changed files, test output, Git diff, and user approval.
  - The external tool remains a supervised collaborator; Viden remains the session owner, audit owner, and approval gate.
- Content hierarchy:
  - Highest priority: current conversation, composer, permission prompts.
  - Second priority: active work lanes, plan/task state, approvals, and tool execution.
  - Third priority: diagnostics, Git, provider health, logs, and historical details.

## Design principles

- Principle 1: terminal-first, cockpit-like second. Every visual flourish must protect readability.
- Principle 2: one source of truth. Panels are projections of transcript, workflow state, runtime snapshot, and tool events.
- Principle 3: progressive power. The first screen works alone; companion screens add visibility but are never required.
- Principle 4: reversible control. Dynamic screen open/close and theme changes should be safe runtime actions.
- Principle 5: side work must be real work. A companion screen earns its space by running or supervising useful tasks, not just mirroring status.
- Principle 6: terminal panes are first-class task surfaces. External CLIs should feel attached to the Viden session through lane metadata, logs, and task links, even when the process is independent.
- Tradeoffs:
  - Dense panels are preferred over empty cinematic space, but the composer and approvals must stay unmistakable.
  - First implementation may use external terminal windows or tmux-like process supervision; a richer embedded PTY manager can follow once state contracts are stable.
  - External tools can run in side lanes before they are deeply integrated, but their cwd, command, lifecycle, and logs must be explicit.

## Visual language

- Color:
  - Built-in themes should include `aurora-cyan`, `ember-gold`, `plasma-violet`, and `monochrome-ice`.
  - Theme tokens should cover background, panel border, active border, text, muted text, accent, success, warning, error, selection, and dim overlay.
  - Defaults should favor dark backgrounds and high-contrast foreground text.
  - The default theme should be `aurora-cyan`, because it best matches the approved preview direction.
- Typography:
  - Monospaced terminal text only; do not require custom fonts.
  - Use compact labels and stable status text.
- Spacing/layout rhythm:
  - Thin borders, stable grid, one-line headers, fixed composer/status heights.
  - Avoid nested cards; use panels and split regions.
  - Main landscape layout should reserve a stable right rail for workspace, active tasks, diagnostics, provider health, and recent files.
  - Approval should render as a centered modal over the transcript/right rail, with transcript context still visible behind it.
  - Portrait `OPS` layout should stack smaller panels vertically with fixed headers.
- Shape/radius/elevation:
  - TUI shape is expressed through box borders, separators, and active highlight state.
  - Rounded-card metaphors are only conceptual in generated images, not a terminal requirement.
- Motion:
  - Minimal. Use live refresh, spinner/progress markers, and subtle active-state changes.
  - Reduced-motion equivalent is the default terminal mode.
- Imagery/iconography:
  - No required image assets in terminal runtime.
  - Preview images live under `docs/previews/` as implementation inspiration only.

## Components

- Existing components to reuse:
  - `SessionEngine` for all input processing.
  - `EngineEvent` for rendered session output.
  - `PermissionPrompt` and `ApprovalResponse` for approvals.
  - Existing structured command views from `viden-runtime`.
  - JSONL transcript storage and workflow task state.
- New/changed components:
  - TUI layout model: screen, panel, region, focus, scroll, and render buffer.
  - Theme token model and TOML loading.
  - Screen registry for main plus up to two companions.
  - Companion workspace manager for dashboard views, task lanes, and terminal panes.
  - Terminal lane manager for launching, tracking, attaching, detaching, stopping, and logging external commands.
  - Panel adapters for transcript timeline, workspace summary, active tasks, diagnostics, provider health, recent files, approvals, agents, Git, files, provider, and tools.
  - Screen launcher abstraction so macOS Terminal support does not leak into the cross-platform registry model.
- Candidate commands:
  - `/screen open agents` opens or focuses the left companion workspace.
  - `/screen open ops` opens or focuses the right companion workspace.
  - `/lane run <command>` starts a supervised terminal lane in the current workspace.
  - `/lane codex <task>` starts a configured `codex` lane for a bounded task.
  - `/lane claude <task>` starts a configured `claude` lane for a bounded task.
  - `/lane ask <tool> <task>` starts any configured external coding tool lane.
  - `/lane attach <id>` attaches the focused TUI pane to a running lane.
  - `/lane detach` returns from an attached lane to the Viden TUI.
  - `/lane inspect <id>` summarizes lane logs, changed files, tests, and next-action recommendation.
  - `/lane accept <id>` records that the lane result is accepted and can be integrated into the main task.
  - `/lane revise <id> <feedback>` sends follow-up instructions to a still-running or restarted lane when the adapter supports input.
  - `/lane stop <id>` stops a lane after confirmation when it is still running.
- Task envelope:
  - Every external coding tool lane receives a bounded task envelope rather than an unstructured chat fragment.
  - Required fields: lane id, objective, cwd/worktree, constraints, allowed mutation scope, expected output, verification command, and handoff format.
  - Optional fields: linked Viden task id, related files, current plan excerpt, recent diagnostics, relevant transcript excerpt, branch/worktree name, timeout, and stop conditions.
  - Default handoff format asks the tool to end with a concise summary, files changed, tests run, remaining risks, and suggested next step.
  - Task envelopes are written to durable lane files so the exact instruction sent to `codex`, `claude`, or another tool can be audited.
- Tool adapter contract:
  - Adapter fields: tool id, display name, binary path or command template, supported input mode, supports interactive follow-up, supports non-interactive execution, default timeout, environment policy, and result parser.
  - Input modes:
    - `argv`: pass the task through command-line arguments when a tool supports safe non-interactive prompts.
    - `stdin`: pipe the task envelope into the process.
    - `prompt-file`: write the task envelope to a file and pass the file path.
    - `pty`: open an interactive terminal session and paste/type the task envelope.
    - `manual`: open a terminal with prepared context and let the user submit.
  - Result parsers should be conservative: prefer explicit exit code, captured logs, Git diff, and verification output over model-written success claims.
  - Adapters for `codex` and `claude` should be presets, not hard dependencies; users can define additional tools in config.
- Observation and decision loop:
  - `queued`: lane exists but has not launched.
  - `starting`: process or terminal is being created.
  - `running`: output is streaming and status panels show last activity.
  - `needs-input`: adapter detected an interactive prompt or the user attached and paused automation.
  - `completed`: process exited successfully or the tool produced a complete handoff.
  - `failed`: process exited non-zero, timed out, or violated constraints.
  - `reviewing`: Viden is summarizing logs, diff, and verification evidence.
  - `accepted`: user or Viden accepted the lane result for the active plan.
  - `revising`: follow-up feedback has been sent to the same tool or a restarted lane.
  - `archived`: lane is closed but logs and metadata remain available.
- Result policy:
  - A lane result is never trusted solely because the external tool says it is done.
  - Viden should inspect Git diff and relevant logs before recommending acceptance.
  - For file-mutating lanes, verification commands should run inside the lane worktree when available.
  - Main-session integration should present a clear choice: accept, revise, inspect manually, or discard.
  - Discard should preserve logs and worktree state unless the user explicitly requests cleanup.
- Variants and states:
  - Main screen: normal, approval-pending, tool-running, error, compact width.
  - Agents screen: idle, running, blocked, completed, stale/offline, terminal-attached.
  - Ops screen: clean, warning, failing tests, provider degraded, no data, terminal-attached.
  - Terminal lane: starting, running, waiting-for-input, detached, attached, succeeded, failed, stopped, archived.
  - Theme states: active, missing token fallback, invalid custom theme.
- Token/component ownership:
  - Built-in theme definitions belong in CLI/TUI code.
  - User themes belong in configuration, preferably `themes/*.toml` under the config home plus an optional project override.

## Accessibility

- Target standard: strong keyboard usability and high contrast; formal WCAG mapping is best-effort for terminal constraints.
- Keyboard/focus behavior:
  - Composer is the default focus.
  - Tab/Shift-Tab should move among panels once panel focus is implemented.
  - Esc should close transient overlays first; Ctrl-C exits safely.
- Contrast/readability:
  - All built-in themes must remain legible on common dark terminals.
  - Do not rely on color alone; use labels, symbols, and text state.
- Screen-reader semantics:
  - Plain terminal output remains the accessible fallback.
  - TUI must not become the only way to access commands or status.
- Reduced motion and sensory considerations:
  - Avoid blinking text and rapid flashing.
  - Refresh rate should be configurable or conservative.

## Responsive behavior

- Supported breakpoints/devices:
  - Minimum usable terminal: 80x24, compact single-column.
  - Comfortable main terminal: 120x36 and above, main plus right side panels.
  - Large landscape: transcript plus multiple side panels.
  - Portrait companion: stacked vertical panels.
- Layout adaptations:
  - Compact width hides lower-priority side panels behind tabs.
  - Landscape companion uses matrix/table layouts plus one or more terminal pane slots.
  - Portrait companion stacks panels with fixed headers and a primary terminal/log pane.
  - Target physical setup is 32-inch landscape `MAIN`, 32-inch landscape `AGENTS`, and 27-inch portrait `OPS`.
- Touch/hover differences:
  - Not applicable for terminal runtime; mouse support is optional and not required for V1 TUI polish.

## Interaction states

- Loading:
  - Show model/provider, active tool, elapsed time, and live status without blocking input rendering.
  - Terminal lanes show command, cwd, elapsed time, and last output line while starting/running.
- Empty:
  - Show concise ready state, session id, provider/model, and useful command hints.
- Error:
  - Render errors in the transcript and status bar; keep composer available where safe.
  - Failed terminal lanes keep their captured output and exit code available for review.
- Success:
  - Mark completed tool calls/tasks with clear labels and timestamps where available.
  - Completed terminal lanes can be linked to task updates, test results, or Git summaries.
- Disabled:
  - Plan mode and permission-denied states must be visibly distinct from normal editable mode.
  - Destructive lane actions such as stop/kill require explicit confirmation when a process is running.
- Offline/slow network:
  - Provider degradation belongs in the status bar and Ops companion.

## Content voice

- Tone: concise, operational, bilingual docs when user-facing docs are changed.
- Terminology:
  - Use "screen" for main/companion terminal surfaces.
  - Use "panel" for regions inside a screen.
  - Use "workspace" for a companion screen that can hold multiple panels or terminal lanes.
  - Use "lane" for one supervised task or terminal process.
  - Use "attach/detach" for entering and leaving an interactive terminal lane.
  - Use "theme" for color token sets.
  - Use "agent lane" for future sub-agent rows until real multi-agent records exist.
- Microcopy rules:
  - Prefer action/state labels over explanation.
  - Do not add instructional paragraphs inside the TUI; use command hints and docs.

## Implementation constraints

- Framework/styling system:
  - Current TUI uses `crossterm`; continue without new dependencies unless a rendering library is explicitly chosen later.
  - Keep the plain REPL path intact.
  - Embedded terminal panes may require a PTY/tmux abstraction; choose the smallest reliable layer only after a prototype proves the lane lifecycle and logging model.
- Design-token constraints:
  - Built-in themes should be available without config files.
  - Custom themes should use TOML and graceful fallback for missing keys.
- Performance constraints:
  - Rendering must avoid expensive full-state recomputation where practical.
  - Companion workspaces should poll or tail derived state conservatively.
  - Terminal lane output should be buffered and truncated for rendering while full logs remain durable.
- Compatibility constraints:
  - macOS Terminal spawning may be the first companion implementation; core screen and lane state should not be macOS-specific.
  - External coding CLIs are optional integrations: missing `claude`, `codex`, or other tools should produce a clear lane error, not a TUI crash.
  - Transcript JSONL remains canonical; SQLite remains derived.
  - All mutations continue through the shared runtime and permission path.
- External tool safety constraints:
  - Default to read-only or isolated worktree execution for external coding tools until the user accepts their output.
  - Prefer per-lane worktrees for `codex` and `claude` tasks that may edit files.
  - Never send secrets, API keys, or full transcripts to external tools unless the user explicitly configured that context policy.
  - Limit context by default: task objective, selected files, relevant diagnostics, recent plan excerpt, and explicit constraints.
  - Preserve a full lane audit trail: task envelope, launch command, environment summary, captured output, exit code, changed files, verification commands, and accept/revise/discard decision.
  - External lane processes must be killable without corrupting main TUI state.
- Test/screenshot expectations:
  - Unit-test layout splits, theme parsing/fallback, screen registry limits, terminal lane state transitions, and event-to-panel adapters.
  - Unit-test task envelope rendering, adapter command construction, result-state transitions, timeout handling, and lane log retention.
  - Smoke-test `--tui` with fallback provider.
  - For later visual iterations, capture terminal screenshots or snapshot render strings for key terminal sizes.

## Development plan

- Phase 1: main-screen foundation.
  - Implement the approved single-screen layout: top rail, transcript timeline, right rail, approval modal, Composer, status bar.
  - Keep all input, approvals, and tool execution inside `SessionEngine`.
  - Add render snapshot tests for compact, normal, and wide terminals.
- Phase 2: screen registry and companion workspace shell.
  - Add `MAIN`, `AGENTS`, and `OPS` registry state with max two companions.
  - Add open/focus/close lifecycle without terminal lane execution first.
  - Keep companion state serializable so external windows can follow the same session.
- Phase 3: terminal lane MVP.
  - Add supervised lane records: id, title, cwd, command, status, log path, task link, timestamps.
  - Support `/lane run <command>` for non-interactive commands first, with durable logs and status updates.
  - Show lanes in `AGENTS`/`OPS` workspaces and allow stop/archive.
- Phase 4: external coding CLI lanes.
  - Add task envelopes and configurable adapters for tools such as `codex` and `claude`.
  - Support `/lane codex <task>` and `/lane claude <task>` when binaries are available.
  - Capture output, changed files, exit code, and verification evidence.
  - Add `/lane inspect`, `/lane accept`, and `/lane revise`.
  - Prefer isolated per-lane worktrees for file-mutating tasks once the workflow is reliable.
- Phase 5: attachable interactive panes.
  - Prototype tmux or embedded PTY attach/detach.
  - Add `/lane attach <id>` and `/lane detach`.
  - Preserve full lane logs and make terminal attachment a reversible view state.
- Phase 6: visual polish and theme expansion.
  - Apply the approved cyan primary style and additional theme variants.
  - Tune dense panel rendering after the task/lane workflow is proven useful.

## Open questions

- [ ] Should the first terminal lane implementation use OS terminal windows, tmux sessions, or an embedded PTY crate?
- [ ] What default external coding CLI presets should ship first: `codex`, `claude`, both, or user-configured commands only?
- [ ] How should lane-created file changes be isolated: same worktree by default, per-lane git worktree, or user-selected?
- [ ] Should project-local themes live under `.viden/themes/*.toml`, config-home `themes/*.toml`, or both?
- [ ] Should `/screen open` spawn macOS Terminal first, or support a printed command for users who prefer iTerm/WezTerm/tmux?
- [ ] How should future real sub-agent state be represented before the V3 multi-agent coordinator exists?
