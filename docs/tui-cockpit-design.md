# RoboCode TUI Cockpit Design

This document records the current TUI target so implementation stays aligned
with the generated reference visuals and the terminal-agent workflow.

## Visual Baseline

- Primary visual state: **no modal open**. Dialogs must inherit the same
  aurora-cyan cockpit theme instead of introducing a separate palette.
- Main reference image:
  `docs/previews/tui-concept-holodeck-v1.png`.
- Layout target: dense terminal cockpit, not a landing page. The first screen
  should be useful immediately for coding, reviewing, approval, and agent lane
  supervision.
- Color direction: dark blue-black surfaces, cyan borders, green success,
  yellow attention/permission, red denial/error. Avoid large unrelated black
  bands or default terminal background leaks.

## Main Screen

- Top bar: product, provider, model, session, context window, Git branch,
  permission mode, active-lane count, and telemetry availability.
- Transcript: dominant left pane, timeline-styled entries, recent rows kept
  visible at the bottom.
- Right rail: workspace, active tasks, diagnostics, provider health, recent
  files.
- Composer: always visible at the bottom, with input cursor placed inside the
  input row, action hints, and approval-mode chips.
- Bottom status: connection, session, event count, active lanes, context window,
  theme/help hints. Token, cost, and rate metrics should appear only after real
  provider telemetry is wired.

## Data Truth Contract

- Live TUI panels must not use demo values that look like real runtime state.
- If runtime data is not connected, render `unavailable`, `0`, or an explicit
  setup hint instead of invented health, latency, cost, task, or diagnostic
  values.
- Demo values are allowed only in explicit `--tui-preview*` fixture paths.
- Right rail data sources:
  - workspace: `WorkspaceSnapshot::load_current`.
  - active tasks: pending approval plus running or queued terminal lanes.
  - diagnostics: `WorkspaceSnapshot.diagnostics`, populated from the persisted
    `.robocode/diagnostics.txt` cache after real `/lsp diagnostics <path>` or
    post-edit LSP output; empty means unavailable/0.
  - provider health: `ProviderStatus` derived from `SessionEngine`
    `ProviderTelemetry`; request count, success/failure count, last/average
    latency, last event count, and last error are real values. Rate, token, and
    cost stay hidden until their runtime sources exist.
  - recent files: filesystem metadata modification time.
- Any new cockpit metric must identify its runtime source in code or docs.

## Command Palette

The slash command palette appears above the composer for top-level
slash-prefixed tokens such as `/` or `/p`, and for supported nested command
queries such as `/lane `, `/git st`, `/task status task_`, or
`/lsp diagnostics src/`.

Keyboard contract:

- `Up` / `Down`: move the selected command.
- `Tab`: complete the selected command into the composer.
- `Enter`: complete a partial command; submit an exact command.
- `Esc`: close the command palette for the current query. Editing the query
  reopens it.
- `/exit`, `/quit`, `exit`, and `quit`: leave the TUI.

Rendering contract:

- The palette uses the same cockpit border, title, and row style as the rest of
  the TUI.
- It floats directly above the composer and must not obscure the input cursor.
- It shows command, summary, and selected-row marker.
- Supported nested command families show local subcommands and known runtime
  objects. `/lane` suggests lane IDs, `/screen close` suggests tracked side
  screens, `/task` suggests task IDs and task statuses, `/memory` suggests
  operable memory IDs, `/provider use` suggests registered providers and
  descriptor default models, `/model` suggests the active provider's descriptor
  default model, `/git switch` suggests local branches, `/git push` suggests
  local branches, remotes, and known remote branch targets, `/git stash
  pop/drop` suggests stash refs, `/git worktree remove` suggests worktree
  paths, and `/lsp` suggests recent workspace files.

## Approval Modal

Approval is an interactive overlay, not a passive transcript card.

- `Tab` / `Shift-Tab` and arrow keys move focus across apply-all, deny,
  diff, and approve controls.
- Default focus is `Approve`, so `Enter` accepts the common case immediately.
- `Enter` activates the focused control.
- `Space` toggles apply-all when the checkbox is focused.
- `y` approves, `n` / `Esc` / `Ctrl-C` denies.
- Mouse clicks focus controls; releasing on deny or approve resolves the
  prompt.
- After approval or denial, the pending modal must disappear immediately and
  the transcript/right rail should redraw without style residue.

## Multi-Screen Direction

The TUI supports one main screen plus up to two side screens:

- Main screen: transcript, approvals, command entry, and high-level status.
- Side screen 1: child-agent / terminal lane monitoring.
- Side screen 2: diagnostics, build state, files, and ops context.

The core product need is not decoration; it is supervising multiple terminal
coding tools such as Codex, Claude Code, shell jobs, and DeepSeek-backed lanes.
Side screens should expose task state, latest output, artifacts, progress, and
route hints so the main agent can decide follow-up actions.

## Current Implementation Notes

- Resize events trigger redraw for the main and side TUI screens.
- Row-diff rendering avoids full-screen flicker during typing.
- The composer uses display-width aware text handling for CJK input.
- The slash palette is local UI state; model calls are not involved. It now
  supports nested suggestions for `/lane`, `/screen`, `/provider`, `/lsp`,
  `/task`, `/memory`, and `/git`, with dynamic IDs or recent files where the
  current TUI state can provide them. Provider and model suggestions use the
  current runtime provider registry descriptors, so `/provider use` can suggest
  registered provider IDs and known descriptor default models while `/model`
  suggests the active provider's default model. Memory actions use the workflow
  memory snapshot, so `/memory confirm`, `/memory reject`, and `/memory prune`
  suggest relevant memory IDs instead of requiring the operator to copy them
  manually. Git switch and push suggestions use the local branch snapshot from the
  current workspace; git push also uses `git remote` and `git branch -r`
  snapshots for remote and remote-branch target suggestions. Stash pop/drop
  suggestions use the current `git stash list` snapshot; worktree remove
  suggestions use the current `git worktree list --porcelain` snapshot.
- The main screen polls lane artifacts while idle, so background `/lane run`
  completion, failure, and log-tail state appear without a keypress.
- The right-rail `ACTIVE TASKS` panel reads the real workflow task store exposed
  by `/task` and `/tasks`, then combines those task records with pending
  approvals and active lanes.
- Live side screens read only persisted lane state; when no lane store exists
  they show an empty state instead of falling back to preview/demo lanes.
- `/lane inspect <id>` reads persisted lane artifacts: `.log` tail, `.done`
  exit code, log path, done path, envelope path, and envelope preview.
- Template-launched Codex and Claude lanes run in isolated per-lane Git
  worktrees under `.robocode/worktrees/` when a Git `HEAD` is available. The
  task envelope records the lane workspace and mutation scope.
- `/lane inspect <id>` also reports the relevant changed-file snapshot, using
  the lane worktree for isolated external lanes and the current workspace for
  non-isolated shell lanes. It includes lane verification evidence from
  exit/log artifacts and any explicit lane decision artifact.
- `/lane accept <id>`, `/lane revise <id>`, and `/lane discard <id>` record
  explicit operator decisions under `.robocode/lanes/<lane-id>.decision.md`.
- `/lane apply <id>` applies an accepted isolated-lane worktree back to the
  current workspace through an auditable Git patch. It writes
  `.robocode/lanes/<lane-id>.apply.patch` and
  `.robocode/lanes/<lane-id>.apply.md`, refuses non-accepted lanes unless
  `--force` is provided, and does not commit or remove the lane worktree.
  If the patch does not apply cleanly, RoboCode leaves the main workspace
  untouched, marks the lane `apply_conflict`, and writes
  `.robocode/lanes/<lane-id>.apply-conflict.md` with direct and three-way apply
  check output plus changed-file context.
- `/lane cleanup <id>` archives a lane by removing its isolated worktree only
  when the worktree is clean. Dirty worktrees require explicit
  `/lane cleanup <id> --force`, and every cleanup writes
  `.robocode/lanes/<lane-id>.cleanup.md` before removal.
- `/lane attach <id>` opens an interactive terminal for the lane workspace and
  records `.robocode/lanes/<lane-id>.attach.md`. `/lane detach <id>` clears the
  attached UI state without killing the external terminal process.
- Provider health now reflects measured model-request telemetry from the shared
  runtime loop: real request count, success/failure count, last and average
  latency, last event count, and last provider error.
- LSP diagnostics from real core events are parsed by the TUI and persisted to
  `.robocode/diagnostics.txt`, so the main screen and side screens can show the
  same evidence-backed diagnostics snapshot.
- `/screen side-1` and `/screen side-2` launch real companion TUI processes
  with the current provider, model, theme, and workspace. The main screen tracks
  up to two side screens, `/screen list` reports them, and
  `/screen close <side-1|side-2>` stops tracking and sends a terminate request
  when a pid is known.
- The screen registry is persisted in `.robocode/screens.tsv`, so main and
  side-screen processes can observe the same companion-screen state.
- `ROBOCODE_SCREEN_LAUNCH_TEMPLATE` can override the default current-binary
  launcher for desktop-specific workflows, for example opening a new terminal
  window or routing a side screen to another display. Supported placeholders are
  `{screen}`, `{provider}`, `{model}`, `{theme}` and their shell-quoted
  `{name:q}` forms.
- `ROBOCODE_LANE_CODEX_TEMPLATE` and `ROBOCODE_LANE_CLAUDE_TEMPLATE` support
  `{task}`, `{envelope}`, `{cwd}`, and `{worktree}` plus shell-quoted
  `{name:q}` forms. `{cwd}` and `{worktree}` both resolve to the actual lane
  workspace.
- `ROBOCODE_LANE_ATTACH_TEMPLATE` can override the default lane attach launcher.
  It supports `{lane}`, `{task}`, `{tool}`, `{cwd}`, `{worktree}`, `{log}` and
  shell-quoted `{name:q}` forms. macOS has a default Terminal.app launcher;
  other platforms should provide this template, for example a tmux or desktop
  terminal command.
- Code changes that alter cockpit behavior, commands, architecture, config, or
  UI must update the relevant docs. Comments should document non-obvious
  invariants and safety boundaries, not restate obvious code.

## Near-Term Gaps

- Embedded PTY is still future work; current lanes support non-interactive shell
  commands, template-launched Codex/Claude adapters, external-terminal attach,
  persisted envelope/log/exit-code artifacts, plus Unix process-group stop.
- Apply currently uses a conservative patch path through `/lane apply <id>` and
  records conflict reports when the patch cannot apply cleanly. Interactive
  conflict resolution is still follow-up work. Discarding a lane records the
  decision but intentionally does not delete its logs, worktree, or changes;
  cleanup requires a separate `/lane cleanup` command.
- Provider token, cost, and rate telemetry is not connected yet, so the live UI
  intentionally keeps those metrics hidden.
- Diagnostics still require an explicit LSP source, such as `/lsp diagnostics
  <path>` or post-edit diagnostics from the LSP runtime. The live TUI does not
  run an automatic background checker yet.
- Side-screen launch is real process management, but automatic OS window
  placement across physical monitors is still a launcher-template/manual
  desktop responsibility.
- Command palette nested suggestions cover the main command families. Arbitrary
  path completions are still a future refinement.
- Visual parity still needs repeated screenshot comparison against the
  holodeck reference.
