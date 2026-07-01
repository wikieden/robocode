# Viden Long-Term Roadmap

Chinese version: [long-term-roadmap.zh-CN.md](long-term-roadmap.zh-CN.md)

Last updated: 2026-06-26

## Strategic Thesis

Viden should not compete by being another single-agent chat CLI. The durable
opportunity is to become the local-first operating layer for AI coding work:

> A multi-agent coding cockpit that makes agent work observable, bounded,
> reviewable, reusable, and token-efficient.

The TUI is the first product surface, not the final product boundary. It is the
right starting point because dense terminal state, approvals, logs, tests,
diagnostics, and side-agent lanes are easiest to supervise in a cockpit. Once
the runtime is reliable, the same orchestration model can power CLI automation,
IDE/ACP adapters, desktop, web, and team workflows.

GUI and desktop work should follow the runtime contract, not lead it. Viden
must first land the core structure, shared event/command model, context and
cost facts, supervised agent loop, and release gates. After the runtime/UI
contract freeze, TUI and GUI can be developed in parallel under
[Viden Parallel Development Plan](parallel-development-plan.md). The GUI
product contract is tracked in
[GUI Version Functional Design](gui-version-functional-design.md).

The accepted TUI / GUI visual source is now
`docs/viden-design/Viden/`, governed by
[Viden Design Adoption](viden-design-adoption.md). RoboCode remains a legacy
implementation and compatibility name until the rename migration is explicitly
planned.

## Market Read

Current AI coding tools are converging around several patterns:

- Claude Code is strong at terminal-native agent work, hooks, MCP, subagents,
  and workflow conventions.
- Codex is strong as a local coding agent surface and should be treated as an
  important delegated lane rather than only a competitor.
- Zed is pushing editor-native parallel agents and ACP as an external-agent
  interoperability boundary.
- Kiro is pushing spec-driven development, steering files, hooks, MCP, and
  project knowledge as product primitives.
- Aider remains a reminder that repo maps, git-native workflows, and small
  scoped changes can beat heavier agent systems.
- OpenHands, Goose, Kilo, and similar projects point toward broader platform,
  SDK, cloud, and multi-surface futures.
- Hacker News feedback repeatedly says that agent success depends less on raw
  autonomy and more on context control, scoped tasks, evidence, isolation, cost
  visibility, and human review.

The implication for RoboCode: do not chase every surface at once. Win the
operator loop first.

## Product Identity

Viden should become:

- a local-first AI coding operator cockpit
- a supervisor for other coding agents, not only a provider client
- a structured fact and evidence layer over messy transcripts, logs, diffs, and
  test output
- a token-efficiency engine that decides what context each agent needs and
  explains what was omitted
- a safe extension runtime for providers, MCP servers, skills, hooks, ACP
  agents, shell jobs, and future integrations
- a high-density operator product where TUI and GUI share runtime facts,
  lane/session semantics, decision gates, and evidence/context presentation

Viden should avoid becoming:

- a clone of Claude Code, Codex, Zed, Kiro, or Aider
- a decorative TUI with side panels that do not control real work
- a marketplace before the permission, credential, and evidence model is mature
- a cloud/team product before the local runtime is trustworthy
- a second editor before editor integration boundaries are clear

## Long-Term Pillars

### 1. Operator Loop

The core loop is:

1. clarify intent
2. build task/spec/context
3. route to one or more lanes
4. observe live status
5. collect evidence
6. review/apply/discard/retry
7. preserve decisions for future context

Every feature should strengthen this loop. If a feature adds automation without
improving observability, reviewability, or recovery, it should wait.

### 2. Shared Agent Runtime

All agent surfaces should use one shared model:

- `AgentTask`
- `AgentLane`
- `ContextBundle`
- `Evidence`
- `Artifact`
- `Decision`
- `Permission`
- `Budget`
- `CredentialHandle`

Providers, shell lanes, Codex, Claude, DeepSeek, tmux, PTY, ACP, MCP, skills,
and hooks should not create side-channel runtimes.

### 3. Context And Token Efficiency

Token efficiency is a product feature, not an implementation detail.

Long-term capabilities:

- source ranking and pin/omit controls
- repo maps and semantic summaries
- diff-aware context selection
- task-specific memory retrieval
- long-log summary plus tail preservation
- per-lane token and cost budgets
- context pressure warnings before expensive turns
- omitted-source records with reason codes
- reusable context bundles across related lanes

### 4. Evidence And Trust

The user should always be able to answer:

- What is the agent doing now?
- What did it see?
- What did it change?
- What did it run?
- What failed?
- Why does RoboCode think this is ready?
- What is the next safe action?

This requires event timelines, audit replay, changed-file evidence, test output,
diagnostics, permission history, and screenshot/smoke evidence for UI releases.

### 5. Multi-Agent Orchestration

Multi-agent does not mean "spawn more agents." It means bounded parallel work
with clear roles and recoverable state.

Target built-in roles:

- planner
- implementer
- reviewer
- tester
- researcher
- doc writer
- release/verifier

Target orchestration patterns:

- plan -> implement -> review -> test
- parallel investigation lanes
- adversarial review lane
- rescue lane for failing tests
- documentation/update lane
- release validation lane

### 6. Isolation And Safety

Parallel coding agents need more than git worktrees.

Long-term lane isolation should model:

- worktree or branch
- writable path scope
- environment variables
- caches
- test database/schema
- service ports
- background processes
- setup and teardown commands
- cleanup proof

### 7. Extensibility Without Fragmentation

Viden should support ecosystem growth while keeping one permission and
evidence model.

Extension layers should mature in this order:

1. descriptors and doctors
2. read-only probes
3. supervised invocation
4. permission-gated mutation
5. marketplace/install UX

ACP should be treated as the likely agent interoperability boundary, similar in
spirit to how LSP standardized editor/language-server integration.

### 8. Surfaces After Runtime

The TUI remains the primary surface until the runtime is proven. After that:

- CLI automation for scripts and CI
- ACP/IDE adapter for editor-native context
- API/server mode for local integrations
- desktop app for richer visual supervision
- web/team dashboard after local workflows prove repeatable
- cloud/remote only when credential and audit boundaries are mature

## Horizon Roadmap

### Horizon 1: 0.1.x - Cockpit And Delegated Lanes

Goal:
Make the TUI-led operator loop real and trustworthy.

Key outcomes:

- default TUI is stable
- first-run setup works
- provider/model setup is clear
- approval UX is reliable
- input, focus, mouse, modal, caret, and resize behavior are reliable enough
  for daily use
- active provider/tool work keeps repainting visible working state instead of
  freezing the operator cockpit
- side screens show real lane state
- shell/template lanes are useful
- Codex and Claude can run as supervised delegated lanes
- lane event timeline exists
- ContextBundle v1 is visible
- lane isolation preflight exists
- docs, screenshots, release assets, and Homebrew are routine
- before 0.1.x final, pass the [TUI Stability Zero-Bug Gate](tui-stability-zero-bug-gate.md):
  zero known P0/P1 TUI display, input, modal, scrollback, resize, and stale-state bugs

What not to do yet:

- broad write-capable external agents by default
- cloud/team dashboards
- plugin marketplace
- full ACP runtime before lane evidence is stable

### Horizon 2: 0.2.x - Spec, Context, And Evidence Runtime

Goal:
Turn RoboCode from a cockpit into a repeatable coding workflow engine.

Key outcomes:

- `/spec` or equivalent creates requirements/design/tasks
- steering files capture project conventions
- task envelopes feed lanes directly
- ContextBundle supports pin/omit/source priority/reason codes
- cost/rate/time budget ledger exists
- event timelines and audit replay are first-class
- reviewer/tester lanes become standard templates
- lane apply/discard/retry lineage is durable
- local release/test workflows are encoded as reusable flows

What this enables:

- users trust longer tasks because they can see scope, context, budget, and
  evidence before applying changes
- RoboCode becomes differentiated even when using Codex or Claude as backends

### Horizon 3: 0.3.x - External Agent And ACP Interoperability

Goal:
Make RoboCode a supervisor for heterogeneous coding agents.

Key outcomes:

- ACP probe and one real ACP compatibility target
- external agent capability registry
- Codex/Claude/DeepSeek/Gemini/custom templates share the same lane lifecycle
- adapter-native config is discovered instead of duplicated
- MCP/plugin/skill descriptors are visible in doctor and side-2
- credential handles prevent secret leakage into transcripts
- hooks are typed, blocking-capable, logged, and inspectable

What this enables:

- RoboCode can orchestrate the best available agent for each task without
  turning into a provider-specific wrapper

### Horizon 4: 0.4.x - Built-In Multi-Agent Workflows

Goal:
Ship out-of-the-box orchestration that feels useful without manual wiring.

Key outcomes:

- built-in workflow templates: plan, implement, review, test, release
- background reviewer/tester lanes
- automatic task splitting with human-visible boundaries
- rescue loops for failing tests
- context reuse across lanes
- budget-aware model routing
- project-level memory and decision records become practical, not decorative
- deterministic validation packs for TUI, CLI, provider, lane, and extension
  behavior

What this enables:

- RoboCode becomes a true multi-agent programming workbench rather than a
  cockpit around manually launched tasks

### Horizon 5: 0.5.x - Platform Surfaces

Goal:
Expose the stable runtime outside the TUI.

Key outcomes:

- scriptable CLI flows
- local API/server mode
- IDE/ACP adapter
- desktop/visual operator view exploration
- CI/release assistant mode
- team/report export surfaces
- stable extension SDK boundaries

What this enables:

- teams and advanced users can embed RoboCode into their development workflows
  without bypassing its permission, evidence, and budget model

### Horizon 6: 1.0 - Reliable Local AI Coding Operating Layer

Goal:
Make RoboCode dependable enough to recommend for real project work.

1.0 criteria:

- multi-provider setup is stable
- core TUI workflows are reliable across terminal sizes and common input modes
- delegated lanes are observable and recoverable
- context and budget behavior is visible and controllable
- permission and credential boundaries are documented and tested
- release packaging is routine across supported platforms
- common workflows have first-class docs and screenshots
- external integrations do not bypass audit, permissions, or evidence

## Version Planning Principles

- Every release should improve one of: observability, context efficiency,
  isolation, reviewability, or repeatability.
- 0.1.x final is the TUI stability exit: do not enter 0.2.x while known P0/P1
  TUI bugs remain open.
- Every user-visible feature needs a real screenshot or deterministic visual
  artifact.
- New adapters start read-only or supervised before becoming mutating.
- New extension surfaces start as descriptor/doctor/probe before invocation.
- Do not add more automation until current automation can explain itself.
- Prefer one excellent end-to-end lane over many half-working integrations.
- TUI polish matters, but only when it improves operator confidence.

## Recommended Next Sequence

After `0.1.24`, the likely sequence is:

1. `0.1.25`: TUI Display Cleanup. Focus on borders, vertical lines, colors,
   IME, cursor, modal position, right rail drift, and popup placement.
2. `0.1.26`: TUI Regression Pack. Convert historical display bugs into
   deterministic previews, terminal smoke, or manual screenshot checklists.
3. `0.1.27`: Daily Coding Loop Hardening. Validate input, approval, tests,
   diff, error recovery, scrollback, and provider setup with real development
   tasks.
4. `0.1.28`: Delegated Lane Visibility Cleanup. Ensure side screens, lane
   evidence, and Codex/Claude/shell job status are consistent and not fake.
5. `0.1.29`: 0.1.x RC Stabilization. Stop expanding new UI surfaces and fix
   only P0/P1 TUI bugs.
6. `0.1.30`: 0.1.x Final Zero-Bug Gate. Enter 0.2.x only after P0/P1 TUI
   backlog is zero, screenshot evidence is complete, quick/full release gates
   pass, and GitHub Release plus Homebrew are synchronized.
7. `0.2.0`: Runtime Layering And Event Closure. Separate core, TUI, provider,
   tool, lane, and evidence boundaries behind a unified `RuntimeSnapshot` /
   event stream. The TUI subscribes to runtime state instead of owning business
   logic.
8. `0.2.1`: Context And Token/Cost Engine. Add `ContextBundle`, semantic file
   selection, log compaction, tool-result deduplication, token budgets, and
   cost panels to reduce context blowups, DeepSeek 413 failures, and invisible
   spend.
9. `0.2.2`: Agent Execution Loop. Make planner, coder, reviewer, tester, and
   doc-writer supervised roles with task envelopes, inputs, outputs, evidence,
   failure classification, and next actions.
10. `0.2.3`: Real Development Scenario Gate. Require DeepSeek live development
    smoke, daily-loop, plan-mode, provider/model, lane operator, release gate,
    and token/cost summaries for every release.
11. `0.3.0`: Multi-frontend Contract Freeze. Freeze the UI/runtime contract
    and Viden migration plan before parallel frontend implementation.
12. `0.3.1`: Parallel TUI and GUI Implementation. Run core/runtime, TUI, and
    GUI branches concurrently, with at most three active owners and every
    frontend consuming the same runtime.

This sequence keeps RoboCode focused on the wedge: not maximum autonomy, but
maximum operator trust.
