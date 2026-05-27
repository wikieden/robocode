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
- Live activity: a fixed strip inside the transcript area that answers what
  RoboCode is doing right now, such as `Thinking...`, `Editing src/render.rs`,
  waiting for approval, or supervising active lanes.
- Right rail: workspace, active tasks, diagnostics, provider health, recent
  files.
- Composer: always visible at the bottom, with a taller three-row input well,
  native blinking bar cursor placed inside the input row, action hints, and
  approval-mode chips.
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
  - active tasks: pending approval plus the unified `AgentTask` view of running
    or queued terminal lanes and delegated Codex jobs.
  - diagnostics: `WorkspaceSnapshot.diagnostics`, populated from the persisted
    `.robocode/diagnostics.txt` cache after background LSP checks, real
    `/lsp diagnostics <path>`, or post-edit LSP output; empty means
    unavailable/0.
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
  default model, `/git diff`, `/git add`, `/git restore`, and `/git stash
  push` suggest workspace file paths, `/agent`, `/extensions`, `/mcp`, and
  `/skills` suggest read-only visibility and diagnostics commands, `/git switch`
  suggests local branches, `/git push` suggests local branches, remotes, and known remote branch
  targets, `/git stash pop/drop` suggests stash refs, `/git worktree remove`
  suggests worktree paths, and `/lsp` suggests workspace file paths.

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

## Agent Bridge Product Contract

The cockpit's next core experience is a host-delegate agent bridge, modeled
after the Claude Code Codex plugin pattern:

- RoboCode is the host. It receives the user's primary goal and keeps the
  operation center, composer, approvals, and side screens coherent.
- Codex is the first delegate. It runs as a tracked job/lane with a readiness
  doctor, launch path, job record, cancel/result commands, and evidence output.
- Claude Code, DeepSeek TUI, shell, tmux/PTY, plugin, skill, MCP, and ACP agents
  should eventually plug into the same lifecycle instead of getting custom
  one-off panels.
- The UI must show "what is the delegate doing now?" in the main screen, not
  only inside side screens. Side screens add depth; the main screen remains the
  operator's truth surface.
- Delegate and terminal work must flow through one visible `AgentTask` model
  in the TUI. Codex jobs, Claude/DeepSeek/shell lanes, tmux sessions, PTY
  bridges, and future ACP agents should share id, agent, transport, status,
  activity, progress, evidence, pid, and result concepts before they get richer
  agent-specific controls.
- Delegate results must become evidence: changed files, commands, test output,
  final summaries, errors, and resume/thread handles.
- Write-capable delegate work must be explicit and permission-gated; review and
  diagnostics can stay read-only.

## AgentTask Runtime Model

`AgentTask` is the TUI's unified observation model for answering "who is
working, what phase are they in, and where is the evidence?" It is not a new
source of truth. It is a normalized runtime view derived from transcript
events, pending approvals, provider telemetry, test evidence, lane artifacts,
Codex job records, tmux/PTY logs, and future ACP events.

Minimum fields:

- `id` / `parent_id`: link the primary reply to child-agent and tool work.
- `agent` / `kind` / `transport`: distinguish `robocode`, `codex`, `claude`,
  `deepseek`, `shell`, `mcp`, `skill`, `acp`, plus `provider`, `tool`, `lane`,
  `job`, `test`, and `approval`.
- `status`: use one set: `queued`, `thinking`, `streaming`, `editing`,
  `running_tool`, `testing`, `waiting_approval`, `needs_input`, `blocked`,
  `done`, `failed`, `cancelled`, and `archived`.
- `activity` / `summary` / `progress`: feed the main-screen operation center
  and side-screen rows.
- `workspace` / `evidence` / `permissions` / `decision` / `result` /
  `resume_handle`: connect audit evidence, permission boundaries, and follow-up
  action. Evidence rows should prefer actionable facts: command, failure,
  conflict, path, changed files, patch artifact, and review/apply result before
  generic transcript labels.

The primary reply status must also become an `AgentTask`: after submit it shows
`thinking/streaming`, during tool calls it shows `running_tool`, during approval
blocks it shows `waiting_approval`, and after completion it shows `done` plus
the final summary. The main operation center, right-rail `ACTIVE TASKS`, side-1
lane list, side-2 evidence, `/agent status`, and `/lane inspect` must consume
the same normalized view instead of each stitching together its own status
model.

## Current Implementation Notes

- Resize events trigger redraw for the main and side TUI screens.
- Row-diff rendering avoids full-screen flicker during typing.
- The composer uses display-width aware text handling for CJK input, keeps a
  native blinking bar cursor visible in the input row, and reserves a taller
  input well so the prompt remains easy to find during long sessions.
- The main transcript reserves a fixed `OPERATION CENTER` band at the top. It
  derives status from pending approvals, the unified `AgentTask` view, the
  latest user turn, the latest tool call, or the latest transcript entry, so
  the main screen can show `DeepSeek is thinking`, `Approval needed: ...`,
  `Supervising 2 agents: ...`, compact edit summaries, delegated-agent
  progress, and evidence source without inventing runtime data.
- Approval overlays and `waiting_approval` tasks must be treated as live only
  until a later approval resolution, tool result, assistant reply, or `/test`
  command result closes them. Closed approvals must not keep blocking the
  operation center or modal layer.
- Failed tests and lane conflicts should surface as operator actions, not just
  logs: show the failing command or conflict summary first, then the next action
  (`open failure, patch, rerun tests` or `inspect conflict and revise/apply`).
- `/diff` and `/git diff` output is also an `AgentTask`: non-empty diffs use
  `kind=diff`, `status=needs_input`, files/additions/deletions/path evidence,
  and a next action to review the diff before testing or committing.
- Transcript-derived tasks keep the latest representative diff, test, tool, and
  provider entries separately so side-2 can compare the current review surface
  with recent verification and edit evidence.
- The slash palette is local UI state; model calls are not involved. It now
  supports nested suggestions for `/lane`, `/agent`, `/extensions`, `/mcp`,
  `/skills`, `/screen`, `/provider`, `/lsp`, `/task`, `/memory`, and `/git`,
  with dynamic IDs or recent files where the
  current TUI state can provide them. Provider and model suggestions use the
  current runtime provider registry descriptors, so `/provider use` can suggest
  registered provider IDs and known descriptor default models while `/model`
  suggests the active provider's default model. Memory actions use the workflow
  memory snapshot, so `/memory confirm`, `/memory reject`, and `/memory prune`
  suggest relevant memory IDs instead of requiring the operator to copy them
  manually. Agent and extension commands expose `/agent list`,
  `/agent doctor`, `/agent review codex`, `/agent challenge codex`,
  `/agent run codex`, `/agent run codex --write`, `/agent status`,
  `/agent result`, `/agent cancel`,
  `/extensions list`, `/extensions doctor`, `/mcp list`, `/mcp doctor`, and
  `/skills list` suggestions before runtime extension execution moves beyond
  the shared permission path. Git and LSP path suggestions use the workspace file snapshot
  collected for the right rail instead of scanning the filesystem while the
  operator types. Git switch and push suggestions use the local branch snapshot
  from the current workspace; git push also uses `git remote` and
  `git branch -r` snapshots for remote and remote-branch target suggestions.
  Stash pop/drop suggestions use the current `git stash list` snapshot;
  worktree remove suggestions use the current `git worktree list --porcelain`
  snapshot.
- `/agent list` and `/agent doctor` expose template, tmux, PTY,
  custom-template, Codex, and experimental ACP adapters. Codex readiness checks
  the local `codex` binary, app-server support, auth, config sources, and job
  store path. `/agent review codex`, `/agent challenge codex`, and
  `/agent run codex [--write]` create tracked jobs under `.robocode/agents/`;
  `/agent status`, `/agent result <id>`, and `/agent cancel <id>` inspect and
  control those jobs. `--write` is explicit and runs through RoboCode's
  mutating permission prompt before Codex starts in `workspace-write` sandbox.
  Codex jobs keep a start-time Git status baseline and extract
  resume/session hints plus touched-file evidence from result/log output, so
  status/result views can show `codex resume ...` and related files when
  available. The TUI also reads app-server result/log artifacts for thread,
  turn, status, approval, resume, command-output, file-change, patch, diff,
  filesystem, error, and final-message evidence. App-server result summaries
  persist final `agentMessage` text as `message:` so `/agent result`, side-2,
  and `AgentTask` agree on the delegate answer. The main `OPERATION CENTER`
  band and right-rail `ACTIVE TASKS` panel read the same job records, so active
  Codex work is visible while the operator keeps typing. ACP readiness is configured through
  `ROBOCODE_AGENT_ACP_COMMAND`; `/agent doctor acp` can run a minimal JSON-RPC
  `initialize` handshake and writes `.robocode/agents/acp-doctor-*.jsonl`
  evidence. Full `/lane acp` execution is still follow-up work.
- `/test <command>` is a real runtime command, not a visual placeholder. It
  runs through shell approval, records the latest status, exit code, duration,
  command, failure summary, likely failing files, and output tail, and makes
  the compact status visible through `/status`. Failed test output is also
  normalized into `AgentTask.evidence` rows (`failure`, `failing-file`, `tail`,
  and `rerun <command>`) so side-2 and the main operation center can guide the
  patch/rerun recovery loop.
- `/extensions doctor` and `/mcp doctor` are readiness reports, not placeholders:
  they show provider plugin dirs, MCP config files and server names, skill root
  counts across project/user/legacy scopes, and the permission boundary that
  blocks extension mutation until it enters the shared tool path.
- The main screen polls lane artifacts while idle, so background `/lane run`
  completion, failure, and log-tail state appear without a keypress.
- Side screen 2 is the ops/evidence cockpit. It renders `TESTS / LSP`,
  `MCP / CONTEXT`, `EXTENSIONS`, and `RECENT EVIDENCE` panels. Test rows are
  parsed from real `/test` transcript evidence, LSP rows use
  `WorkspaceSnapshot.diagnostics`, MCP rows inspect workspace/user config file
  paths, extension rows summarize provider/catalog/lane/MCP/skill readiness,
  and `RECENT EVIDENCE` rows read the unified `AgentTask` runtime view for
  approval, tool, lane, and Codex job `id / agent / status / progress /
  activity`, with `evidence`, `decision`, `result`, and the next operator action
  as secondary rows. For failed or blocked tasks, secondary rows prioritize
  command, failure, failing-file, tail, rerun, path, lines, and changed-files
  evidence so the operator sees actionable evidence first. Completed app-server
  text turns prioritize final `message ...` evidence before lower-signal
  protocol ids.
- The right-rail `ACTIVE TASKS` panel reads the real workflow task store exposed
  by `/task` and `/tasks`, then combines those task records with pending
  approvals and active lanes.
- Live side screens read only persisted lane state; when no lane store exists
  they show an empty state instead of falling back to preview/demo lanes.
- `/lane inspect <id>` reads persisted lane artifacts: `.log` tail, `.done`
  exit code, log path, done path, envelope path, terminal attach/tmux/PTY
  artifact paths, and envelope preview.
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
- `/lane resolve <id>` retries an `apply_conflict` lane after the operator has
  adjusted the main workspace or lane worktree. It uses the same auditable
  patch path as `/lane apply`: the Git patch must pass `git apply --check`
  before RoboCode mutates the main workspace. A clean retry writes the normal
  `.apply.md`; a still-conflicting retry refreshes `.apply-conflict.md`.
- `/lane cleanup <id>` archives a lane by removing its isolated worktree only
  when the worktree is clean. Dirty worktrees require explicit
  `/lane cleanup <id> --force`, and every cleanup writes
  `.robocode/lanes/<lane-id>.cleanup.md` before removal.
- `/lane archive <id>` records `.robocode/lanes/<lane-id>.archive.md` and marks
  the lane archived without deleting logs, decisions, apply records, or an
  isolated worktree. Live queued/running/attached lanes must be stopped,
  completed, or detached first.
- `/lane attach <id>` opens an interactive terminal for the lane workspace and
  records `.robocode/lanes/<lane-id>.attach.md`. `/lane detach <id>` clears the
  attached UI state without killing the external terminal process.
- `/lane tmux <id>` creates or reuses a named tmux session for the lane
  workspace. The side-1 lane monitor and focused lane modal surface the exact
  `tmux attach -t ...` command for attached tmux lanes, or `/lane tmux <id>` as
  the next interaction step for lanes that are not attached yet. With the
  default tmux template, pane output is piped into the standard lane `.log`, so
  side screens and `/lane inspect` can observe live tmux output.
- `/lane pty <id>` starts an embedded PTY bridge for the lane workspace. It
  creates `.robocode/lanes/<lane-id>.pty.in` as the input FIFO, writes
  `.robocode/lanes/<lane-id>.pty.md` as the audit record, and captures output
  in the standard lane `.log`. `/lane inspect <id>` surfaces those PTY artifact
  paths, and `/lane send <id> <text>` writes a line to that PTY bridge from
  inside the TUI.
- Side-1 `LIVE OUTPUT` and the focused lane modal replay the latest persisted
  lane `.log` tail when available, falling back to the lane summary only when
  no captured terminal output exists yet. This gives tmux, PTY, and background
  lanes an in-cockpit screen-state slice before a full terminal emulator lands.
- Side-1 lane rows now use the shared agent vocabulary by showing transport
  (`template`, `tmux`, `pty`, or `shell`) and state (`thinking`, `editing`,
  `testing`, `needs input`, `blocked`, or `done`) next to attach and evidence
  hints.
- Provider health now reflects measured model-request telemetry from the shared
  runtime loop: real request count, success/failure count, last and average
  latency, last event count, provider-reported token usage, token throughput
  when request timing permits it, and last provider error.
- Provider health metric rows render as stable label/value pairs: the row label
  carries the metric color while values such as `Configured` and `0 ok / 0 err`
  stay in the normal text color, avoiding word-level color splits in the compact
  right rail.
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
- `ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE` and
  `ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE` can override the default
  current-binary launcher for per-screen desktop workflows, with
  `ROBOCODE_SCREEN_LAUNCH_TEMPLATE` as a shared fallback. Supported
  placeholders are `{screen}`, `{title}`, `{role}`, `{display}`,
  `{display_index}`, `{provider}`, `{model}`, `{theme}`, `{cwd}`, `{binary}`,
  `{args}` and their shell-quoted `{name:q}` forms. This lets an operator route
  side screens through Terminal.app, iTerm, tmux, or a display-placement script.
- `ROBOCODE_LANE_CODEX_TEMPLATE` and `ROBOCODE_LANE_CLAUDE_TEMPLATE` support
  `{tool}`, `{task}`, `{envelope}`, `{cwd}`, and `{worktree}` plus shell-quoted
  `{name:q}` forms. `{cwd}` and `{worktree}` both resolve to the actual lane
  workspace.
- `/lane ask <tool> <task>` uses `ROBOCODE_LANE_<TOOL>_TEMPLATE` for custom
  supervised external tools such as Gemini, Junie, or local coding CLIs. Missing
  templates leave a queued lane with a rendered task envelope instead of losing
  the request.
- `ROBOCODE_LANE_PTY_TEMPLATE` can override the embedded PTY bridge. It supports
  `{lane}`, `{task}`, `{tool}`, `{cwd}`, `{worktree}`, `{command}`, `{input}`,
  `{log}`, `{shell}` and shell-quoted `{name:q}` forms. The default Unix
  template uses the system `script` command plus the lane input FIFO.
- `ROBOCODE_LANE_ATTACH_TEMPLATE` can override the default lane attach launcher.
  It supports `{lane}`, `{task}`, `{tool}`, `{cwd}`, `{worktree}`, `{log}` and
  shell-quoted `{name:q}` forms. macOS has a default Terminal.app launcher;
  other platforms should provide this template, for example a tmux or desktop
  terminal command.
- Code changes that alter cockpit behavior, commands, architecture, config, or
  UI must update the relevant docs. Comments should document non-obvious
  invariants and safety boundaries, not restate obvious code.

## Near-Term Gaps

- Embedded PTY now has a first supervised bridge through `/lane pty` and
  `/lane send`, and the cockpit replays recent persisted lane log tails in the
  side screen and focused lane modal. A richer inline terminal emulator with
  cursor-addressed screen-state replay remains follow-up work.
- Apply currently uses a conservative patch path through `/lane apply <id>` and
  records conflict reports when the patch cannot apply cleanly. `/lane resolve
  <id>` provides an operator-driven retry loop after manual conflict cleanup;
  a full inline conflict editor is still follow-up work. Discarding a lane
  records the decision but intentionally does not delete its logs, worktree, or
  changes; cleanup requires a separate `/lane cleanup` command.
- Provider token telemetry now comes from real provider `usage` payloads for
  OpenAI-compatible, Anthropic, and Ollama-style responses. Token rate is
  derived only when both usage and non-zero request timing are available. Cost
  remains hidden unless a provider reports cost data; RoboCode does not invent
  prices in the TUI.
- Diagnostics come from the shared LSP runtime through post-edit diagnostics,
  explicit `/lsp diagnostics <path>`, and a throttled live TUI background
  checker over workspace Rust files. Projects without a configured/available
  language server still show `diagnostics unavailable`.
- Main-screen rendering now uses display-cell width helpers for Chinese text,
  emoji modifiers, combining marks, transcript wrapping, topbar fitting, and
  approval preview borders so long multilingual sessions do not push the right
  rail out of alignment.
- Side-screen launch is real process management. Physical monitor placement is
  now an explicit launcher-template integration point via per-screen template
  variables, but RoboCode still delegates OS-specific window movement to the
  configured terminal or display-placement script.
- Command palette nested suggestions cover the main command families, including
  workspace file path suggestions for the common Git and LSP path-taking
  commands.
- Visual parity still needs repeated screenshot comparison against the
  holodeck reference.
