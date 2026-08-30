# Viden TUI Cockpit Design

This document records the current TUI target so implementation stays aligned
with accepted visual references and the terminal-agent workflow.

Accepted design source: `docs/viden-design/Viden/`. The old Viden visual
plan is legacy and should not drive new TUI decisions.

Interaction flow companion: [TUI Interaction Flow Design](tui-interaction-flow-design.md).

## Visual Baseline

- Primary visual state: **no modal open**. Dialogs must inherit the same
  cockpit theme instead of introducing a separate palette.
- Viden visual sources are binding after review through
  [Viden Design Adoption](viden-design-adoption.md).
- The live baseline is `docs/viden-design/Viden/TUI/Viden - 统一原型
  (TUI).html`; component and interaction behavior comes from the TUI component
  kit and T4 interaction rules. Review snapshots are
  `docs/viden-design/reference-shots/TUI-统一原型驾驶舱.png` and
  `docs/viden-design/reference-shots/TUI-组件库.png`.
- Layout target: dense terminal cockpit, not a landing page. The first screen
  should be useful immediately for coding, reviewing, approval, and agent lane
  supervision.
- Color direction: dark cockpit as the primary theme, cyan as the primary
  interaction focus, gold for human decision or permission-needed states, green
  for success, red for denial/error, and blue for progress. Avoid large
  unrelated black bands or default terminal background leaks.
- Visual tokens must come from an accepted in-repo token source before they
  become implementation requirements; TUI implementation also needs truecolor
  and ANSI 256 fallback mapping.

## TUI Target

- Use a canonical component vocabulary instead of each screen inventing its own
  terminal frame, status bar, lane row, approval gate, and overlay.
- Status bar keeps project/lane identity fixed separately from actionable
  permission/approval/gate/error facts. The center ticker is ambient-only and
  may contain activity, event, token, or provider-health facts, never actions.
- Right rail uses Env / Lane / More groups only when matching Core facts exist.
  It is optional and closed by default; the transcript fills the available
  space while it is closed.
- Lane rows can expand to show subagents under the current lane.
- Composer behavior follows the canonical T1c component: multiline editing,
  bracketed paste, bounded growth, then internal scroll. This document does not
  define a second row-count limit.
- Welcome screen uses the Viden identity and command selector. `/setup` opens
  Core-backed onboarding, `/lanes` opens the Core lane board, and normal input
  opens the unified cockpit.
- Pending approvals stay pinned without owning composer input. `Ctrl-G` opens
  Decisions; choosing a concrete request creates explicit approval focus.
- The local supervision loop projects tasks/DAGs, active tools, queued input,
  lane output/conflicts/recovery, merge gates, evidence decisions, context
  pressure, cost visibility, and audit IDs from typed Core facts. A command
  receipt is shown only as `pending Core fact`, never as inferred success.
- The same projection also carries compact review-request, handoff, contract,
  dependency, conflict-bounce, and revert rows, plus their inbox counts. The
  status bar and right rail render the gate count with the registered gold `⏸`
  glyph from the TUI status vocabulary and add a supervision count only when
  Core has published such records; an empty supervision inbox renders no badge.
- A supervision decision is settled by the confirm-on-fact primitive: sending a
  command records only the pending command id, `CommandAccepted` keeps it
  pending, `CommandRejected` surfaces Core's reason, and confirmation requires
  the exact business fact (`MergeGateUpdated` with the requested gate id and
  status, `ReviewRequestUpdated` with the requested review id and status,
  `MergeConflictBounced` or `RevertRecorded` for the gate). Only one supervision
  command may be in flight; a second is refused locally instead of sent.
- A lane whose route is cost-blind (terminal, tmux) never shows an inferred
  token or dollar figure. Its inspector shows an explicit blind marker plus the
  four bounded run facts Core measured — run count, wall time, applied diff
  bytes, last exit code — and shows no run facts at all when Core has published
  none. A missing exit code renders as unknown, never as success. Wall time is
  rendered at the scale an operator reads — milliseconds below a second, seconds
  with one decimal below a minute, minutes plus seconds above it.

## Supervision Decision Center

The Decision Center (`Ctrl-G`, `/decisions`) lists the decisions Core is waiting
on, in one fixed order: pending tool approvals, merge gates, review requests
still awaiting a verdict, and conflict bounces still awaiting revalidation.
Settled reviews and resolved bounces are history and are not listed as
decisions. Rows use the registered TUI glyphs `⏸` (gate), `◌` (awaiting), and
`⚠` (conflict); the record-kind token (`GATE`, `REVIEW`, `CONFLICT`) is stable
across locales so a Core identifier is always findable by the same label.

Selecting an approval row opens the pinned Approval overlay, unchanged.
Selecting a gate, review, or conflict row opens the supervision decision
overlay:

- Actions are derived from the full Core record re-read on each frame, never
  from the compact row. An open gate offers accept and reject; a gate with a
  pending conflict offers revalidation in place of accept until the origin Lane
  clears it; a merged gate offers only revert, with an explicit irreversibility
  hint; a pending review offers both verdicts. An action that can never apply to
  the record's current status is not listed at all.
- An action whose Core precondition merely looks unsatisfiable locally is still
  sent. Core is the only authority for whether a decision is allowed.
- Reject, revert, and bounce require a reason; review feedback is optional for
  both verdicts, matching the Core command contract. An empty required reason or
  text longer than Core's 500-character trust-text limit is refused locally with
  a message, and nothing is sent — the client never truncates and sends half a
  reason.
- Actors and reviewed-evidence bindings are replayed from Core's own records
  using the same derivation the GUI integration-gate adapter uses. When no owner
  or canonical evidence can be replayed, the action is refused locally because
  there is no valid payload to send.
- Arrows or number keys select an action and `Enter` confirms it. `Esc` unwinds
  in order: first the reason line, then the overlay. Closing the overlay never
  cancels an in-flight Core command; the pending badge keeps showing it. The
  overlay has no text filter, so printable characters that are not an action
  number keep editing the composer during a streaming turn.
- The outcome echo renders inside the overlay and in the status bar: `◌`
  pending with the command id, `✓` confirmed, `✗` rejected with Core's own
  reason verbatim. A settled outcome resets to idle when the next supervision
  action is initiated or when the overlay is closed; a pending outcome is never
  auto-reset.
- A lost or never published receipt would otherwise strand the single in-flight
  slot, so both the overlay and the Decision Center footer offer a dismiss
  action. Dismissing stops local attribution only: it settles nothing and does
  not cancel the Core command.

## Audit Timeline

The audit timeline overlay is the read side of the supervision loop: it answers
"what already happened here", where the Decision Center answers "what is Core
waiting on". It consumes the Core audit contract
(`RuntimeCommand::QueryAudit` -> `RuntimeEventKind::AuditPageLoaded`) and is
read-only in this version — browsing carries no per-row action.

Two entry points open it, and it has no global chord of its own:

- the supervision decision overlay's `Audit trail` row, scoped to the record on
  screen (a gate or conflict target audits `merge_gate:<gate id>`, a review
  target audits `review_request:<review id>`, using the contract's own object
  kind keys);
- the Decision Center's `Audit timeline (project)` footer pick, which opens the
  project timeline unscoped. Core's audit store is already scoped to the
  project's own workflow directory, so the query sets no `project_id` or
  `lane_id` filter rather than inventing one.

Behavior:

- Opening always dispatches a fresh first query (`limit` 100) and the panel is
  dropped when the overlay closes, so a reopened timeline is never a page of
  unknown age. `QueryAudit` mutates nothing and prompts for no permission, so
  the timeline stays readable in Plan mode.
- Records render newest first as `{time} {action} {outcome} {objects} {args}`.
  `time` is `HH:MM:SS` UTC — an audit record is evidence compared across
  machines, so a locale-shifted clock would make two readers disagree about one
  fact. `action` is Core's raw dotted key (`gate.decided`, `handoff.created`)
  and is deliberately never localized; the contract keeps it free of prose so a
  reader can diff two timelines. Outcomes use the registered `✓` (success) and
  `✗` (denied or failed) glyphs; an outcome this build does not know renders
  literal ASCII `?` rather than borrowing a status it never had. Objects render
  as comma-joined `kind:id` pairs and arguments as space-joined `k=v` pairs,
  both truncated by display width.
- Four states stay visibly distinct: loading (a query is in flight and nothing
  has arrived), empty (Core answered with an empty page), error (`✗` with
  Core's own reason verbatim), and the loaded list. The footer always states how
  many records are loaded and whether older ones remain, so a short list is
  never mistaken for the whole timeline.
- Arrows move the selection through the rows and then onto the `Load older
  records` row, which `Enter` confirms. A complete timeline hides that row. A
  second page request while one is in flight is refused locally with a message
  and nothing is sent. Confirming a record row is a deliberate no-op.
- The read correlates independently of the supervision decision slot: a pending
  gate, review, or revert command neither blocks the timeline nor is settled by
  a page. The overlay has no text filter, so printable characters keep editing
  the composer during a streaming turn.
- `Esc` closes to the base state. The TUI keeps an overlay return path only for
  Global Jump, so the timeline unwinds exactly as the Approval overlay opened
  from the Decision Center does.

Known limitation, recorded rather than papered over: `AuditPageLoaded` carries
no command id, so a page produced by another client's concurrent query can be
attributed to this panel's in-flight query. For the single-operator loop this is
acceptable — the page is still a real Core page, it is discarded when the
overlay closes, and the next query self-corrects. Removing the ambiguity is a
Core contract change (a command id on the page), not a client-side guess.

## Main Screen

- Top bar: product, provider, model, session, context window, Git branch, work
  mode, permission level, active-lane count, and telemetry availability.
- Transcript: dominant left pane, timeline-styled entries, recent rows kept
  visible at the bottom.
- Live activity: a prominent `LIVE WORK` strip inside the transcript area,
  directly after the latest visible conversation entry, that answers what
  Viden is doing right now with phase, signal, and next-action guidance.
- Right rail: Env / Lane / More tabs, compacting workspace, active tasks,
  context, MCP, LSP, Todo, diagnostics, provider health, recent files, usage,
  and keybindings.
- Composer: always visible at the bottom, following the canonical T1c sizing
  and internal-scroll behavior, with a native blinking bar cursor, action
  hints, work mode chips, and permission level chips.
- Bottom status: connection, session, event count, active lanes, context
  pressure, and presentation/help hints. Cost is explicitly `metered`, `blind`,
  or `unavailable`; token counts never fabricate money when Core has no actual
  cost fact. Recovery and pending-command alerts remain pinned rather than
  entering the ambient ticker.

## Presentation Preferences

- Locale, skin, color mode, density, motion, font scale, and accessibility are
  shared presentation preferences owned and persisted by Core. TUI consumes
  the resolved preference projection and does not maintain a private palette
  or preference store.
- Built-in locale support starts with `en` and `zh-CN`. User-facing labels must
  remain translatable; stable command names, IDs, statuses, and audit facts are
  not inferred from localized copy.
- TUI copy lives in the key-compatible `apps/tui/i18n/en.json` and
  `apps/tui/i18n/zh-CN.json` catalogs. Both catalogs must keep exact key and
  interpolation-parameter parity. `zh-Hans-CN` system input resolves to
  `zh-CN`, an unknown system locale resolves to English, and a missing key is
  rendered visibly as the key instead of disappearing.
- Runtime rendering derives the catalog from the current Core
  `ResolvedUiPreferences` projection on every frame, including after snapshot,
  replay, or `UiPreferencesUpdated`. CLI override, stored user preference,
  system locale, and English fallback precedence is resolved by Core; the TUI
  neither repeats that resolver nor accepts project data as a personal UI
  preference source.
- Stable Settings is selector-first and exposes locale, skin, mode, density,
  motion, terminal color depth, Apply, and Reset. Persisted choices send only
  `SetUiPreferences { patch: UiPreferencePatch }` or `ResetUiPreferences` and
  remain pending through `CommandAccepted`; only the matching
  `UiPreferencesUpdated` confirms success. Rejection preserves the draft and
  renders Core's reason. Missing `ui.preference_persistence` keeps the surface
  visible but unavailable with zero command transport.
- Amber and Phosphor light choices remain visible but disabled with the
  dark-only reason. Color depth is an explicitly unsaved session preview
  because schema 1 does not persist that terminal-only axis. The TUI never
  writes a private config or preference store.
- The production TUI generates exactly eight complete semantic palettes from
  `docs/viden-design/Viden/tokens.css` at build time: Aurora, Ice, and Mono in
  dark/light plus dark-only Amber and Phosphor. Missing registered tokens fail
  the build; an invalid or partial appearance falls back atomically to Aurora
  dark/regular instead of combining axes from different profiles.
- Terminal presentation resolves `auto | truecolor | ansi256 | ansi16` against
  detected capabilities, applies compact/regular/comfy geometry, keeps reduced
  motion indicators static, and substitutes registered one-cell ASCII glyphs
  when Unicode is unavailable. A `UiPreferencesUpdated` projection refreshes
  theme, density, and motion in memory without creating a TUI preference store.
- Mouse capture remains disabled. It may be enabled only after the Core
  resolved preference contract exposes and returns an explicit true value;
  environment guesses and TUI-private persistence are not valid substitutes.
- Adding a locale or skin requires paired copy/token coverage and deterministic
  preview evidence. A frontend-only skin fork is not an accepted extension
  path.

## Lane / Session Hierarchy

The new TUI must share the GUI hierarchy:

```text
Workspace -> Project -> Lane / Session -> Subagent
```

Requirements:

- `/sessions`, `/lane`, side rail, history, and evidence use the same
  lane/session identity.
- A lane can belong to a project or live as a workspace-level global lane.
- Expanded lanes show subagents, backend type, status, progress, evidence, and
  gate count.
- Main transcript displays the current lane; switching lanes must not lose
  composer draft or pending input queue.

## Data Truth Contract

- Live TUI panels must not use demo values that look like real runtime state.
- If runtime data is not connected, render `unavailable`, `0`, or an explicit
  setup hint instead of invented health, latency, cost, task, or diagnostic
  values.
- Demo values are allowed only in explicit `--tui-preview*` fixture paths.
- Every main-cockpit and right-rail fact comes from `RuntimeViewState` through
  `CoreClient`. The TUI does not scan workspace files, read lane artifacts,
  mutate configuration, or persist runtime facts independently.
- Setup consumes `ProjectProbe`, `ProjectConfigPreview`,
  `confirmed_project_config`, active provider health, and safe credential
  handles. Raw secret ingress is unavailable through the Core 0.3.2 client and
  must not be recreated as a slash command.
- Startup requires only the frozen Core base contract. Every Core 0.3.2
  extension is feature-gated independently and requires agreement between the
  handshake and confirmed snapshot; an absent extension never blocks unrelated
  cockpit use.
- Any new cockpit metric must identify its runtime source in code or docs.

## Command Palette

The slash command palette appears above the composer for top-level
slash-prefixed tokens such as `/` or `/p`, and for supported nested command
queries such as `/lane `, `/git st`, `/task status task_`, or
`/lsp diagnostics src/`.

Interactive decision commands are modal-first after submission, but they should
not steal focus while the user is still typing. `/connect`, `/models`, and
`/settings provider` stay in the compact completion surface until `Enter`
submits them; then the dedicated modal owns search, keyboard movement, mouse
selection, and `Enter` apply semantics. Future configuration, mode-switching,
lane/agent action, and multi-choice workflows should follow the same pattern.
They must not degrade into status-only pages unless the command is explicitly a
diagnostic or details command such as `/config`, `/status`, or `/provider
doctor`.

`/setup` opens the Core-backed Setup lens and sends `ProbeProject` only when
`runtime.project_onboarding` is available. Otherwise it stays visible and
read-only with a concrete unavailable reason and sends no setup command. The selector
holds a secret-free presentation draft shaped as Core D11 `[project]` data with
required `name` and `pack` fields. Preview sends its exact contents through
`PreviewProjectConfig`; only a valid Core preview whose exact contents still
match the draft enables `ConfirmProjectConfig` with its immutable id and hash.
The UI stays pending until
`ProjectConfigConfirmed`; local command acceptance cannot mark it complete.

`/lanes` opens the Board lens from `RuntimeViewState.lanes`. Selecting a lane
uses only its Core-owned `active_session_ids`; multiple ids open a second
selector before entering the Session/Cockpit lens. Core 0.3.2 has no global
session list, so the TUI does not invent one.

`/lane` is the orchestration action selector. It lists lane launch commands and,
when lanes are active, id-specific inspect, timeline, diff, and artifacts
actions so users do not have to memorize lane ids.
The first-level object is a durable lane/session, not a disposable background job.
Existing lane rows must show project/global ownership, subagent count, pending
gate count, latest evidence, and running state.

Provider/model selectors have separate semantics:

- `/provider` and `/connect` are the supplier connection flow. The first-level
  list shows supplier names such as `DeepSeek` and `OpenRouter`; it must not
  include credential, endpoint, or model explanations on the supplier rows.
  TUI 0.3.0 never accepts credential bytes or repackages them as slash-command
  text. It displays only Core-masked handle metadata; when no trusted Core
  ingress is available, the detail surface says so and remains read-only.
- `/models` is the cross-provider model selector. Rows are grouped by provider
  with models indented underneath, and it only shows providers/models that have
  already been configured or activated. Descriptor-only defaults for
  unconfigured providers must not appear as runnable choices. Selecting a row
  applies the provider/model switch directly.
- `/model <model>` is the current-provider quick switch for activated models.
  It must not hide the fact that selecting a model from another provider
  requires switching provider as well.

Keyboard contract:

- `Up` / `Down`: move the selected command.
- `Tab`: complete the selected command into the composer.
- `Enter`: complete a partial command; submit an exact command.
- Mouse left click: select a visible suggestion on press and complete it on
  release.
- `Esc`: close the command palette for the current query. Editing the query
  reopens it.
- `/exit`, `/quit`, `exit`, and `quit`: leave the TUI.

Rendering contract:

- The palette uses the same cockpit border, title, and row style as the rest of
  the TUI.
- It floats directly above the composer and must not obscure the input cursor.
- It shows command, summary, and selected-row marker.
- Long suggestion lists render a visible window with a range hint, and keyboard
  navigation adjusts that window so the selected item always remains visible.
  Mouse hit testing maps from visible rows back to the underlying suggestion
  index before completion.
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

## Approval Gate

Approval uses an explicit-focus overlay in the same event loop, not a passive
transcript card or a nested input loop.

- A pending approval remains pinned and does not own the composer. Composer
  `y`, `n`, `d`, and `Enter` retain their normal edit/submission meaning.
- `Ctrl-G` opens Decisions. Selecting a concrete pending request opens explicit
  approval focus for that request only.
- Explicit focus renders four typed choices: `1` allow once, `2` allow for the
  Core-provided session, `3` add the exact Core-provided repository allowlist,
  and `4` deny. Unavailable scopes remain visibly unavailable. `y`/`n` retain
  allow-once/deny compatibility, `d` opens request diff/evidence, arrow keys
  move the selected action, and `Enter` activates it.
- `Esc` only closes explicit approval focus; it never denies or otherwise
  resolves the request. After overlays and local selection are unwound,
  Normal-mode `Esc`, `Ctrl-C`, and ExitConfirm send an active-work interrupt
  only when `runtime.lane_owner_projection` exposes exactly one owner for the
  selected active lane. Missing, ambiguous, mismatched, inactive, or stale
  owner state is visibly unavailable and produces zero transport commands.
- Session-scoped approval and repository allowlisting appear only when the
  request's typed `allowed_scopes` contains the exact Core payload. The TUI
  forwards that scope and the original request owner unchanged.
- Mouse input is optional; when enabled, it selects the same current actions.
- Inspecting diff/evidence never resolves the gate. The panel must render the
  request's real command, scope, risk, expiry/default action, and preview or
  evidence when present.
- After Core reports approval resolution, the pinned request and any matching
  explicit focus must disappear, and the transcript/right rail should redraw
  without style residue.
- After the deadline, the request remains visible and inert with
  `default Deny · awaiting Core ApprovalResolved`. TUI does not remove it or
  synthesize a denial locally.

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

- Viden is the host. It receives the user's primary goal and keeps the
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
- `agent` / `kind` / `transport`: distinguish `viden`, `codex`, `claude`,
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
- Provider turns now run through the `TuiRuntime` worker and feed stream,
  approval, cancel, finish, and error events back into the main TUI event loop.
  Remaining 0.1.24 work is to make queued input runtime-visible and broaden the
  slow-provider/approval/resize smoke evidence.
- The composer uses display-width aware text handling for CJK input, keeps a
  native blinking bar cursor visible in the input row, and reserves a taller
  input well so the prompt remains easy to find during long sessions.
- The main transcript keeps a compact but prominent `LIVE WORK` strip at the
  live tail, directly below the latest conversation content, instead of a
  blocking center card or a detached top strip. It derives status from pending
  approvals, the unified `AgentTask` view, the latest user turn, the latest
  tool call, or the latest transcript entry, so the main screen can show
  `Viden working`, `Approval needed: ...`, `Supervising 2 agents: ...`,
  compact edit summaries, delegated-agent progress, next actions, and a
  human-readable signal without inventing runtime data or fake provider
  progress percentages.
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
  snapshot. Long lists are windowed rather than clipped: the selected row stays
  visible, and the footer hint shows the visible range.
- `/agent list` and `/agent doctor` expose template, tmux, PTY,
  custom-template, Codex, and experimental ACP adapters. Codex readiness checks
  the local `codex` binary, app-server support, auth, config sources, and job
  store path. `/agent review codex`, `/agent challenge codex`, and
  `/agent run codex [--write]` create tracked jobs under `.viden/agents/`;
  `/agent status`, `/agent result <id>`, and `/agent cancel <id>` inspect and
  control those jobs. `--write` is explicit and runs through Viden's
  mutating permission prompt before Codex starts in `workspace-write` sandbox.
  Codex jobs keep a start-time Git status baseline and extract
  resume/session hints plus touched-file evidence from result/log output, so
  status/result views can show `codex resume ...` and related files when
  available. The TUI also reads app-server result/log artifacts for thread,
  turn, status, approval, resume, command-output, file-change, patch, diff,
  filesystem, error, and final-message evidence. App-server result summaries
  persist final `agentMessage` text as `message:` so `/agent result`, side-2,
  and `AgentTask` agree on the delegate answer. The main `NOW WORKING`
  band and right-rail `ACTIVE TASKS` panel read the same job records, so active
  Codex work is visible while the operator keeps typing. ACP readiness is configured through
  `VIDEN_AGENT_ACP_COMMAND`; `/agent doctor acp` can run a minimal JSON-RPC
  `initialize` handshake and writes `.viden/agents/acp-doctor-*.jsonl`
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
  artifact paths, timeline rows, and envelope preview. `/lane timeline <id>`
  focuses the same persisted event chronology for operator review.
- Template-launched Codex and Claude lanes run in isolated per-lane Git
  worktrees under `.viden/worktrees/` when a Git `HEAD` is available. The
  task envelope records the lane workspace, mutation scope, isolation warnings,
  and cleanup/verification hints.
- `/lane inspect <id>` also reports the relevant changed-file snapshot, using
  the lane worktree for isolated external lanes and the current workspace for
  non-isolated shell lanes. It includes lane verification evidence from
  exit/log artifacts and any explicit lane decision artifact.
- `/lane accept <id>`, `/lane revise <id>`, and `/lane discard <id>` record
  explicit operator decisions under `.viden/lanes/<lane-id>.decision.md`.
- `/lane apply <id>` applies an accepted isolated-lane worktree back to the
  current workspace through an auditable Git patch. It writes
  `.viden/lanes/<lane-id>.apply.patch` and
  `.viden/lanes/<lane-id>.apply.md`, refuses non-accepted lanes unless
  `--force` is provided, and does not commit or remove the lane worktree.
  If the patch does not apply cleanly, Viden leaves the main workspace
  untouched, marks the lane `apply_conflict`, and writes
  `.viden/lanes/<lane-id>.apply-conflict.md` with direct and three-way apply
  check output plus changed-file context.
- `/lane resolve <id>` retries an `apply_conflict` lane after the operator has
  adjusted the main workspace or lane worktree. It uses the same auditable
  patch path as `/lane apply`: the Git patch must pass `git apply --check`
  before Viden mutates the main workspace. A clean retry writes the normal
  `.apply.md`; a still-conflicting retry refreshes `.apply-conflict.md`.
- `/lane cleanup <id>` archives a lane by removing its isolated worktree only
  when the worktree is clean. Dirty worktrees require explicit
  `/lane cleanup <id> --force`, and every cleanup writes
  `.viden/lanes/<lane-id>.cleanup.md` before removal.
- `/lane archive <id>` records `.viden/lanes/<lane-id>.archive.md` and marks
  the lane archived without deleting logs, decisions, apply records, or an
  isolated worktree. Live queued/running/attached lanes must be stopped,
  completed, or detached first.
- `/lane attach <id>` opens an interactive terminal for the lane workspace and
  records `.viden/lanes/<lane-id>.attach.md`. `/lane detach <id>` clears the
  attached UI state without killing the external terminal process.
- `/lane tmux <id>` creates or reuses a named tmux session for the lane
  workspace. The side-1 lane monitor and focused lane modal surface the exact
  `tmux attach -t ...` command for attached tmux lanes, or `/lane tmux <id>` as
  the next interaction step for lanes that are not attached yet. With the
  default tmux template, pane output is piped into the standard lane `.log`, so
  side screens and `/lane inspect` can observe live tmux output.
- `/lane pty <id>` starts an embedded PTY bridge for the lane workspace. It
  creates `.viden/lanes/<lane-id>.pty.in` as the input FIFO, writes
  `.viden/lanes/<lane-id>.pty.md` as the audit record, and captures output
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
  `.viden/diagnostics.txt`, so the main screen and side screens can show the
  same evidence-backed diagnostics snapshot.
- `/screen side-1` and `/screen side-2` launch real companion TUI processes
  with the current provider, model, theme, and workspace. The main screen tracks
  up to two side screens, `/screen list` reports them, and
  `/screen close <side-1|side-2>` stops tracking and sends a terminate request
  when a pid is known.
- The screen registry is persisted in `.viden/screens.tsv`, so main and
  side-screen processes can observe the same companion-screen state.
- `VIDEN_SCREEN_SIDE_1_LAUNCH_TEMPLATE` and
  `VIDEN_SCREEN_SIDE_2_LAUNCH_TEMPLATE` can override the default
  current-binary launcher for per-screen desktop workflows, with
  `VIDEN_SCREEN_LAUNCH_TEMPLATE` as a shared fallback. Supported
  placeholders are `{screen}`, `{title}`, `{role}`, `{display}`,
  `{display_index}`, `{provider}`, `{model}`, `{theme}`, `{cwd}`, `{binary}`,
  `{args}` and their shell-quoted `{name:q}` forms. This lets an operator route
  side screens through Terminal.app, iTerm, tmux, or a display-placement script.
- `VIDEN_LANE_CODEX_TEMPLATE` and `VIDEN_LANE_CLAUDE_TEMPLATE` support
  `{tool}`, `{task}`, `{envelope}`, `{cwd}`, and `{worktree}` plus shell-quoted
  `{name:q}` forms. `{cwd}` and `{worktree}` both resolve to the actual lane
  workspace.
- `/lane ask <tool> <task>` uses `VIDEN_LANE_<TOOL>_TEMPLATE` for custom
  supervised external tools such as Gemini, Junie, or local coding CLIs. Missing
  templates leave a queued lane with a rendered task envelope instead of losing
  the request.
- `VIDEN_LANE_PTY_TEMPLATE` can override the embedded PTY bridge. It supports
  `{lane}`, `{task}`, `{tool}`, `{cwd}`, `{worktree}`, `{command}`, `{input}`,
  `{log}`, `{shell}` and shell-quoted `{name:q}` forms. The default Unix
  template uses the system `script` command plus the lane input FIFO.
- `VIDEN_LANE_ATTACH_TEMPLATE` can override the default lane attach launcher.
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
  remains hidden unless a provider reports cost data; Viden does not invent
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
  variables, but Viden still delegates OS-specific window movement to the
  configured terminal or display-placement script.
- Command palette nested suggestions cover the main command families, including
  workspace file path suggestions for the common Git and LSP path-taking
  commands.
- Visual parity still needs repeated screenshot comparison against the
  holodeck reference.
