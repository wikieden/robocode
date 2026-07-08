# Viden 0.1.14 Plan

Chinese version: [release-0.1.14-plan.zh-CN.md](release-0.1.14-plan.zh-CN.md)

Last updated: 2026-05-28

## Positioning

`0.1.14` is the **Delegated Agent Trust Loop** release.

`0.1.13` made the cockpit the default entry point, added first-run
provider/model setup, hardened the operator loop, injected ContextBundle into
main provider turns, and completed the GitHub/Homebrew release loop. The next
release should make the multi-agent promise more concrete:

> Viden can delegate a bounded programming subtask to Codex, Claude, or a
> shell/template lane, show what that lane is doing, capture what it saw and
> changed, preserve evidence, and return the operator to an explicit
> review/apply/discard/retry/stop decision.

This remains a `0.1.x` release. The target is a reliable TUI-led orchestration
loop, not a full `0.2.0` runtime claim.

The product question for this release is not "how many agents can Viden
launch?" It is:

> Why should the operator trust this delegated result?

## Summary Of 0.1.13

Landed:

- `viden` opens TUI by default; `--no-tui` keeps scriptable REPL behavior.
- `/settings` and `/setup` support provider/model setup and saved defaults.
- Approval, command palette, CJK input, resize, and modal reliability are
  covered by tests and deterministic screenshots.
- `/lane diff <id>` and `/lane artifacts <id>` make lane review evidence easier
  to inspect.
- Main provider turns receive an ephemeral ContextBundle while raw transcript
  audit data stays preserved.
- GitHub Release assets, Homebrew tap, post-publish smoke, and DeepSeek live
  smoke are complete for `v0.1.13`.

Remaining gaps:

- Codex and Claude are mapped into lane concepts, but their repeatable
  real-world happy paths need to become release-grade.
- External-agent status, tail, result, touched files, artifacts, and next action
  need to normalize into the same `AgentTask` / lane evidence model.
- Review/apply conflict recovery needs clearer preflight, failure reasons, and
  retry lineage.
- ContextBundle needs explicit token-budget policy, omitted-source records, and
  compaction reason codes.
- Plugin, skill, MCP, and ACP concepts need descriptor/probe boundaries before
  mutating runtime support expands.

## Principles

- One operator loop: provider, shell, Codex, Claude, DeepSeek, tmux, and future
  ACP agents share task, evidence, permission, context, and decision models.
- Real before broad: one reliable Codex path and one reliable Claude path beat
  many decorative adapters.
- Evidence over trust: every delegated lane exposes command, status, tail,
  artifacts, changed files, diff, test result, decision, and next action.
- Token efficiency is product behavior: budget pressure, source selection, and
  compaction decisions must be visible and testable.
- Screenshots are release evidence for every user-visible TUI state.

## Release Cutline

`0.1.14` is complete when:

1. Codex read-only review can run as a delegated lane on a configured machine,
   or fail with actionable doctor output when unavailable.
2. Claude Code can run through a documented template/tmux lane path, or fail
   with actionable doctor output when unavailable.
3. Shell/template, Codex, and Claude results map into the same lane review and
   `AgentTask` evidence model.
4. The operator can inspect, accept, apply, discard, retry, or stop delegated
   work with visible evidence and no silent mutation.
5. ContextBundle v1 records source priority, budget pressure, omitted sources,
   and compaction decisions.
6. Plugin/skill/MCP/ACP remain read-only descriptor/probe surfaces unless routed
   through shared permission and evidence paths.

## P0 Cutline

P0 is intentionally narrow. The release should ship only when these three real
flows are demonstrable:

1. **Shell/template lane trust loop**: launch a bounded command, observe live
   tail/status, inspect timeline evidence, and stop or retry without losing
   prior evidence.
2. **Codex read-only review trust loop**: delegate a review task to Codex,
   collect review evidence and next action, and finish without workspace
   mutation.
3. **Claude template/tmux trust loop**: launch Claude through a documented
   template/tmux path, observe tail/status, inspect final output, and stop or
   retry through the same lane model.

Every P0 flow must have:

- shared `AgentTask` / lane evidence
- visible `NOW WORKING`, side-1, and side-2 state
- timeline or inspect output that explains the result
- explicit next action
- deterministic screenshot evidence or real-terminal notes

## Explicit Non-Goals

Do not spend `0.1.14` scope on:

- full ACP runtime
- plugin marketplace or install UX
- default write-capable Codex or Claude lanes
- automatic multi-agent task splitting
- cloud, web, team, or desktop surfaces
- broad mutating MCP/plugin/skill execution
- additional agent integrations that do not improve the three P0 trust loops

## Scope

## Implementation Order

Execute `0.1.14` in this order:

1. **Trust-loop foundation**: define or extend shared lane timeline, isolation
   declaration, capability, and evidence records before changing UI behavior.
2. **Shell/template baseline**: make the deterministic local lane prove the
   timeline, inspect, stop, retry, and evidence model without external-agent
   variability.
3. **Adapter doctor**: expose capability readiness for shell/template, Codex,
   Claude, tmux, PTY, and future ACP before launching richer delegated work.
4. **Codex read-only review**: add the first non-mutating external-agent trust
   loop.
5. **Claude template/tmux lane**: add the first terminal-template external-agent
   trust loop.
6. **Review/apply/retry safety**: harden conflict preflight and retry lineage
   once the lane evidence model is stable.
7. **TUI evidence screens**: wire `NOW WORKING`, side-1, side-2, command
   palette, and screenshots to the same shared snapshot.
8. **Docs and release evidence**: update user docs, screenshots, release status,
   and smoke scripts only after the P0 flows work.

Do not start P1 implementation until the three P0 trust loops are demonstrable.

### P0: Adapter Doctor And Capability Registry

- Add shared capability records for `shell/template`, `codex`, `claude`,
  `deepseek`, `tmux`, `pty`, and future `acp`.
- Add `/agent list`, `/agent doctor`, and `/agent doctor <id>` or equivalent
  commands if the current surface is insufficient.
- Report binary presence, version, auth/setup hint, input mode, mutation mode,
  evidence mode, and known limits.
- Show readiness in side-1, side-2, and command palette suggestions.
- Keep doctor/probe read-only.

Acceptance:

- Missing Codex or Claude produces actionable setup output.
- Configured tools show ready status and supported lane modes.
- Tests cover descriptor parsing, missing binary, configured template, and
  command rendering.

### P0: Codex Read-Only Review Lane

- Implement a Codex review lane that accepts task, cwd/worktree, ContextBundle,
  and allowed scope.
- Prefer app-server/protocol evidence when available; keep terminal fallback.
- Capture status, tail, final output, touched files, command executions,
  artifacts, and suggested next action into lane evidence.
- Keep write-capable Codex work out of P0 unless isolated and explicitly gated.
- Add deterministic fixture coverage and live/manual instructions.

Acceptance:

- A configured Codex lane can run a read-only review and return evidence.
- Main screen, side-1, side-2, `/lane inspect`, and `/status` agree.
- Result can be accepted, discarded, retried, or archived without mutation.

### P0: Claude Template/Tmux Lane

- Harden `VIDEN_LANE_CLAUDE_TEMPLATE` and tmux launch docs.
- Check `claude`, template variables, cwd/worktree, and log capture readiness.
- Normalize Claude status, tail, final output, touched files, artifacts, and
  next action into lane review.
- Preserve evidence across attach/send/stop/retry.

Acceptance:

- A configured Claude template lane can launch, be observed, inspected, stopped,
  and retried.
- Missing template or binary reports exact setup steps.
- TUI evidence matches `/lane inspect`, `/lane artifacts`, and `/lane diff`.

### P0: Review / Apply / Retry Safety

- Add preflight for dirty workspace, touched-file overlap, patch applicability,
  deleted/moved files, and untracked output.
- Make apply failures produce structured reason and next action.
- Add retry lineage: original objective, previous failure, omitted context,
  changed files, and operator decision.
- Persist accept/apply/discard/retry/stop events into transcript and lane
  evidence.

Acceptance:

- Clean apply succeeds with recorded diff/test evidence.
- Conflict blocks mutation and explains blocking files.
- Retry creates a linked task/lane instead of overwriting prior evidence.

### P0: TUI Evidence Screens

- Add deterministic screenshots for Codex-ready, Codex-reviewing,
  Claude-template-ready, lane-conflict, lane-retry, and adapter-doctor states.
- Preserve the no-modal visual style across modal and non-modal states.
- Keep input height, visible cursor, CJK behavior, resize redraw, and aligned
  borders protected.
- Add real terminal notes or screenshots for macOS Terminal and iTerm2 when
  possible.

Acceptance:

- Every new visible state has a deterministic SVG or real-terminal evidence.
- Side panels and `NOW WORKING` read the same shared task snapshot.
- No mixed-color borders or right-rail drift appear in regression output.

### P0: Lane Event Timeline And Isolation Preflight

Goals:

- Make delegated work explainable while it is running and reviewable after it
  completes.
- Prevent parallel lanes from corrupting shared test data, caches, services, or
  workspaces.

Work:

- Add a per-lane event timeline that records prompt/envelope creation, adapter
  launch, tool/command events, file changes, approvals, tests, failures,
  retries, final output, and operator decisions.
- Add `/lane timeline <id>` or fold the timeline into `/lane inspect <id>` if
  that keeps the command surface simpler.
- Extend lane envelopes with isolation declarations: worktree, env vars, cache
  dirs, database/schema scope, service ports, setup command, verification
  command, and cleanup command.
- Add isolation preflight warnings before starting multiple mutating lanes.

Acceptance:

- The operator can reconstruct why a lane reached a result without trusting only
  the final summary.
- A lane with missing cleanup or shared test-data risk shows a warning before
  launch.
- Timeline rows and isolation warnings are visible in side-1/side-2 and stored
  as evidence.

### P1: ContextBundle v1 Policy

- Add source priority, soft budget, hard budget, omitted-source records, and
  compaction reason codes.
- Apply summary + tail compaction to long lane/test/tool output before provider
  or external-agent input.
- Keep raw logs and transcript entries auditable.
- Show budget pressure and largest sources in `/status`, side-2, and lane
  inspect output.

Acceptance:

- Tests prove compacted model input does not delete raw audit data.
- High context pressure shows what was omitted and why.
- Codex, Claude, and shell lanes receive visible ContextBundle envelopes.

### P1: Cost / Rate / Runtime Budget Ledger

Goals:

- Make provider and lane economics visible before automation expands.
- Stop long-running loops from silently burning quota.

Work:

- Add per-lane ceilings for max turns, max estimated tokens, max cost, max
  wall-clock time, and max retries.
- Show budget burn rate and remaining budget in side-2, `/status`, and lane
  inspect output.
- Record provider rate-limit signals and budget stops as evidence rows.

Acceptance:

- A delegated lane can be stopped by a budget ceiling with a structured reason.
- The operator can see which lane or provider is consuming the most budget.

### P1: Lightweight Steering, Hooks, And Credential Boundaries

Goals:

- Bring in the strongest HN/Kiro/Claude signals without building a full plugin
  marketplace.
- Keep automation observable and safe.

Work:

- Define project steering files for conventions, architecture, workflows, and
  protected paths.
- Add a lightweight spec envelope shape: requirements, design notes, tasks,
  tests, and acceptance criteria.
- Design read-only hook probes for pre-tool, post-tool, notification, and stop
  events; hook outputs become evidence rows.
- Define secret handles for future MCP/plugin/agent calls so model context can
  request a capability without seeing raw credential values.

Acceptance:

- A delegated task envelope can reference steering/spec files.
- Hook probe output is visible and testable without mutating external systems.
- Docs explain the difference between capability use and secret exposure.

### P1: Extension Boundary Docs And Probes

- Define descriptor fields for provider plugins, agent adapters, skills, MCP
  servers, and ACP agents.
- Add read-only `doctor` / `probe` / `capabilities` output for each category.
- Document that future mutating extension calls must pass through shared
  permission, transcript, evidence, and token-budget boundaries.

## Verification Plan

Focused tests:

- adapter descriptor and doctor output
- Codex fixture event mapping into `AgentTask` and lane evidence
- Claude template readiness and lane envelope generation
- dirty workspace and conflict preflight
- retry lineage and decision persistence
- ContextBundle v1 source priority, omission, compaction, and raw-log
  preservation
- lane timeline and isolation preflight
- budget ceiling and budget-stop evidence
- steering/spec envelope references
- hook probe output and credential-handle rendering
- command palette entries for agent doctor and delegated lane commands

Smoke/regression:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/smoke-lane-operator-loop.sh`
- Codex read-only fixture smoke
- Claude template/tmux smoke when available
- `scripts/release-smoke.sh --version 0.1.14 --out-dir /tmp/viden-0114-release-smoke-full-local`

Manual acceptance:

- macOS Terminal and iTerm2 TUI launch
- side-1/side-2 delegated lane observation
- Codex configured read-only review, if available
- Claude configured template/tmux lane, if available
- resize, CJK input, approval modal, mouse/keyboard focus
- screenshots for every new user-visible state

Publish validation:

- tag `v0.1.14`
- GitHub Release workflow uploads macOS arm64, macOS x86_64, Linux x86_64,
  Windows x86_64 archives plus sha256 files
- update `wikieden/homebrew-tap`
- post-publish smoke validates GitHub assets and Homebrew install path

## Out Of Scope

- Full `0.2.0` orchestration runtime claim.
- Write-capable Codex/Claude autonomous mutation without isolated review/apply.
- Marketplace-style plugin or skill installation.
- Mutating MCP/ACP runtime beyond descriptor/probe/capability mapping.
- IDE/web/desktop/API surfaces that bypass the TUI-led runtime.
