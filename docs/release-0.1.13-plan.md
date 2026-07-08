# Viden 0.1.13 Plan

Chinese version: [release-0.1.13-plan.zh-CN.md](release-0.1.13-plan.zh-CN.md)

Last updated: 2026-05-27

## Positioning

`0.1.13` is the **Operator Loop Hardening** release.

`0.1.12` made the first supervised operator loop real: shared runtime
`AgentTask` records, deterministic shell/template lanes, lane ContextBundle v0,
release smoke, screenshots, GitHub release, and Homebrew delivery. The next
release should not broaden into a full `0.2.0` runtime yet. It should make the
loop reliable enough for daily programming and turn Codex/Claude from
"mapped surfaces" into reproducible delegated workflows.

The release cutline is:

> A developer can give Viden a small task, watch the main cockpit explain
> what is happening, delegate or inspect one lane, review evidence, and accept,
> apply, retry, stop, or discard without losing context.

## Decision

Ship `0.1.13` before `0.2.0`.

Why:

- `0.1.12` proves the shape, but remaining risk still sits in real terminal
  behavior, review/apply recovery, and external-agent happy paths.
- The long-term roadmap says TUI and shared runtime must stabilize before API,
  IDE, web, desktop, and broad ACP/plugin expansion.
- `0.2.0` should mean Agent Orchestration Runtime v1, not only one more
  foundation pass.

## Principles

- No fake orchestration: panels must read real runtime facts, artifacts, logs,
  diffs, tests, and decisions.
- Operator confidence first: the main screen must always answer "what is
  running, what changed, what evidence exists, and what can I do next?"
- One lane model: shell/template, Codex, Claude, tmux/PTY, DeepSeek, and future
  ACP adapters must continue sharing `AgentTask`, evidence, permission, and
  context-budget boundaries.
- Token efficiency is product behavior, not only telemetry.
- Real terminal evidence matters for every visible interaction.

## Scope

### P0: Default TUI And First-Run Setup

Goals:

- Make the cockpit the default product entry point.
- Let users choose provider and model from settings instead of editing config by
  hand for the common path.
- Guide first-time users through the minimum setup needed to run a real turn.

Work:

- Change the default `viden` launch behavior to enter TUI mode when no
  explicit non-interactive command is requested.
- Preserve explicit escape hatches: `--no-tui`, existing preview flags,
  `--version`, `--help`, and future scripting commands must remain
  non-interactive.
- Add a settings surface in the TUI for provider and model selection, backed by
  the existing layered config model.
- Show installed/built-in providers, configured API key status, default model,
  and provider health in the settings flow.
- Add a first-run setup guide when no usable provider/model is configured:
  choose provider, choose model, explain API-key source, run a doctor/probe, and
  save the choice.
- Keep config writes explicit, auditable, and reversible; never print or persist
  secret values into transcripts or screenshots.

Acceptance:

- Running `viden` with no args opens the main TUI.
- Running `viden --help`, `viden --version`, and preview/smoke commands
  remains non-interactive.
- A first-time user can select provider/model, see whether the key is present,
  run a probe/doctor, and save the default.
- Existing config files remain valid, and CLI flags still override saved
  defaults for that invocation.

Implementation status:

- Done locally on 2026-05-27: default launch now enters the main TUI, `--no-tui`
  preserves the legacy line REPL, `/settings` and `/setup` show setup state,
  `/settings provider <id> [model]`, `/settings model <model>`, and
  `/settings save` persist provider/model defaults, and slash suggestions cover
  the settings flow.

### P0: Daily Operator Loop Reliability

Goals:

- Make the main TUI feel dependable while a provider, tool, test, or lane is
  running.
- Remove interaction traps around exit, command entry, approval, focus, and
  modal lifetime.
- Preserve the `0.1.12` visual style while fixing alignment/color regressions.

Work:

- Harden `/quit`, `/exit`, Esc, and Ctrl-C behavior across main, side-1, side-2,
  command palette, and modal states.
- Add regression coverage for slash-command input so `/quit` and `/exit` are not
  swallowed by command-palette state.
- Make approval modals dismiss immediately after approve/deny and keep their
  selected/default action obvious.
- Add mouse and keyboard focus tests for approval buttons, side-screen command
  targets, and command palette selection.
- Fix border/color consistency so a panel edge cannot render mixed theme colors
  inside one line or word.
- Keep input area height, cursor visibility, CJK preview, and resize redraw in
  the screenshot set.

Acceptance:

- A user can exit from idle, command-palette, modal, and running-safe states.
- Approval modal default is approve, shortcuts work, and the modal disappears
  after a decision.
- Screenshot evidence exists for idle, running, approval, command palette, CJK
  input, resize, side-1, side-2, and lane review.
- Manual notes or screenshots cover macOS Terminal; iTerm2 is covered when the
  app is installed on the test machine.

Implementation status:

- Started locally on 2026-05-28: command-palette regression coverage now locks
  exact `/quit` and `/exit` so Enter submits instead of completing, while
  partial `/q` and `/ex` still complete. Approval keyboard tests now cover the
  default approve focus, focus movement to diff/deny, Enter activation, and
  direct y/n/Ctrl-C shortcuts.

### P0: Evidence-Backed Review / Apply / Retry

Goals:

- Make deterministic shell/template lane review/apply feel like a real coding
  loop rather than a smoke-test-only path.
- Ensure every lane result has enough evidence to decide safely.

Work:

- Promote changed files, diff summary, test result, exit code, artifacts, and
  log tail into one lane review record.
- Add conflict-aware apply preflight: dirty workspace check, touched-file
  overlap, patch/apply failure reason, and recovery next action.
- Add `/lane diff <id>` and `/lane artifacts <id>` if current inspect output is
  too dense.
- Make `/lane retry <id>` preserve the previous objective, context sources,
  changed-file evidence, and failure reason.
- Persist lane decision events into transcript/workflow evidence so later
  resume can explain what happened.

Acceptance:

- A deterministic lane can run, produce an artifact/diff/test, enter review,
  apply cleanly, and record the final decision.
- A failing/conflicting lane can be retried or discarded with visible reason and
  no silent workspace mutation.
- Side-1 and side-2 show the same lane status and decision state as `NOW
  WORKING`.

Implementation status:

- Started locally on 2026-05-28: `/lane diff <id>` writes and displays a focused
  `L*.diff.patch` artifact, `/lane artifacts <id>` lists persisted lane files,
  and both commands are available through slash suggestions. Focused tests cover
  command-palette routing plus artifact/diff output.

### P0: ContextBundle v0.5 For Main Provider Turns

Goals:

- Move ContextBundle from lane-only metadata into the main provider path without
  destabilizing provider compatibility.
- Make token pressure actionable.

Work:

- Build a main-turn ContextBundle from user task, selected files, latest diff,
  diagnostics, recent test/lane summaries, memory/task summaries, and recent
  transcript summary.
- Use summary + tail compaction for long tool/lane/test output while preserving
  raw audit data.
- Add provider-side context pressure rows: sources, estimated tokens, largest
  contributors, and compaction notes.
- Add soft/hard budget behavior: warn at soft budget, trim lowest-priority
  sources at hard budget, and record what was omitted.
- Keep provider prompt changes behind a conservative helper so OpenAI,
  Anthropic-style, DeepSeek, fallback, and descriptor-backed providers can share
  the same bundle.

Acceptance:

- One real provider turn uses a ContextBundle-generated prompt input.
- Tests prove raw transcript/tool output remains preserved even when provider
  prompt input is compacted.
- TUI shows context pressure consistently in main status and side-2.

Implementation status:

- Started locally on 2026-05-28: main provider turns now build a conservative
  ContextBundle and append it as an ephemeral system context message to the
  `ModelRequest` without storing that generated message in the raw transcript.
  Runtime task evidence and `/status` expose context pressure, source count,
  largest sources, and compaction notes. Focused tests cover provider request
  injection and runtime evidence.

### P1: Codex / Claude Reproducible Happy Paths

Goals:

- Turn Codex and Claude adapters into repeatable, evidence-producing lanes.
- Prefer protocol/app-server events when available, but keep template/tmux
  fallback practical.

Work:

- Add or harden `/agent doctor codex`, `/agent doctor claude`, and template
  readiness diagnostics.
- Add reproducible read-only review smoke for Codex and a template/tmux smoke
  for Claude when installed.
- Map status, tail, result, touched files, final output, and suggested next
  action into shared `AgentTask` and lane review records.
- Keep write-capable external-agent work behind explicit permission, isolated
  worktree, and apply/review boundaries.
- Document unsupported or credential-gated cases clearly in docs and doctor
  output.

Acceptance:

- On a machine with Codex configured, Viden can start a read-only Codex
  review and show result/evidence in TUI.
- On a machine with Claude Code configured, Viden can run a template/tmux
  lane and show tail/result/evidence in TUI.
- Missing tools fail with actionable doctor output rather than silent empty
  panels.

### P1: Real Terminal Acceptance Harness

Goals:

- Make terminal UX defects harder to miss before release.
- Keep deterministic screenshots, but add real terminal evidence for the cases
  screenshots cannot prove.

Work:

- Add a documented manual acceptance checklist for macOS Terminal and iTerm2.
- Add helper scripts for launching main, side-1, and side-2 with fixed sizes and
  collecting screenshots.
- Track which interactions are deterministic, manually verified, or not
  testable on the current machine.
- Store real-use screenshots or notes under `docs/previews/manual/0.1.13/`.

Acceptance:

- Release status distinguishes deterministic SVG evidence from real-terminal
  evidence.
- If iTerm2 is not installed, release status records that exact gap and Terminal
  evidence still runs.

### P2: Extension / ACP / MCP Boundary Notes

Goals:

- Keep future platform work aligned without consuming the whole release.

Work:

- Update adapter/extension docs so plugin, skill, MCP, and ACP all point to the
  same permission/evidence/context boundaries.
- Add or refine descriptor/doctor/probe tests where cheap.
- Do not implement a mutating generalized runtime in `0.1.13`.

Acceptance:

- Docs describe what is real, experimental, and deferred.
- No extension path bypasses permissions, transcript, evidence, or token-budget
  assumptions.

## Non-Goals

- Do not claim full Agent Orchestration Runtime v1.
- Do not add broad marketplace/plugin installation flows.
- Do not expand to desktop/web/IDE/API surfaces.
- Do not build a full Zed-grade ACP host.
- Do not make Codex/Claude write-capable happy paths a blocker unless their
  permission and apply boundaries are fully auditable.

## Implementation Order

1. Default TUI and first-run setup: default launch behavior, `--no-tui`,
   provider/model settings, first-run guide, config persistence, doctor/probe.
2. Interaction hardening: `/quit`, `/exit`, command palette, approval modal
   lifetime, focus, border/color consistency.
3. Review/apply hardening: lane decision record, dirty/conflict preflight,
   retry/discard evidence, side-screen consistency.
4. Main-turn ContextBundle: builder, compaction, prompt integration, pressure UI,
   tests.
5. Codex/Claude happy paths: doctor/probe, read-only review, template/tmux
   result mapping, screenshots.
6. Real terminal harness: scripts/checklist, Terminal/iTerm2 evidence,
   0.1.13 screenshot refresh.
7. Docs/release: README, user guide, modules, roadmap, release status, GitHub
   release, Homebrew tap, post-publish smoke.

## Test Plan

Focused tests:

- default launch enters TUI while `--help`, `--version`, previews, and `--no-tui`
  remain non-interactive;
- provider/model settings list options, respect CLI override precedence, and
  persist explicit saved choices;
- first-run setup handles missing keys, probe failure, and successful
  provider/model selection without leaking secrets;
- slash-command input and exit behavior;
- command palette filtering/selection while typed commands continue to work;
- approval modal default/shortcut/mouse decision and cleanup;
- theme border/color rendering snapshots;
- lane review/apply/retry/discard state transitions;
- dirty workspace and apply-conflict preflight;
- ContextBundle provider prompt compaction and raw audit preservation;
- Codex/Claude adapter event-to-`AgentTask` mapping with fixtures.

Regression and smoke:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.13 --deepseek --out-dir /tmp/viden-0113-release-smoke-full
```

Manual acceptance:

- macOS Terminal: idle, resize, CJK input, command palette, approval modal,
  provider thinking, shell/test running, lane review/apply.
- iTerm2: same checklist when installed.
- Real Codex/Claude checks only when local authentication/tools are available;
  otherwise doctor output and fixture tests prove the failure path.

Publish validation:

```bash
scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0113-postpublish-check
```

## Release Criteria

- Workspace version is `0.1.13`.
- All P0 items pass tests and have screenshot or manual evidence.
- `viden` defaults to the TUI, and first-run provider/model setup is
  documented and verified.
- README and user guide explain current real features and experimental
  adapter limits.
- Release status lists verification, assets, Homebrew tap, and remaining risks.
- GitHub release and Homebrew tap are published and post-publish smoke passes.

## Follow-Up

After `0.1.13`:

- If the operator loop is stable in real daily use, start `0.2.0`: Agent
  Orchestration Runtime v1, default planner -> worker -> reviewer -> tester
  workflow, fuller token-efficiency engine, and stronger Codex/Claude/ACP
  adapter contracts.
- If interaction or apply/retry reliability is still weak, ship `0.1.14` as a
  second hardening release before claiming runtime v1.
