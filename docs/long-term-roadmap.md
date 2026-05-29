# RoboCode Long-Term Roadmap

Chinese version: [long-term-roadmap.zh-CN.md](long-term-roadmap.zh-CN.md)

Last updated: 2026-05-29

## Strategic Thesis

RoboCode should not compete by being another single-agent chat CLI. The durable
opportunity is to become the local-first operating layer for AI coding work:

> A multi-agent coding cockpit that makes agent work observable, bounded,
> reviewable, reusable, and token-efficient.

The TUI is the first product surface, not the final product boundary. It is the
right starting point because dense terminal state, approvals, logs, tests,
diagnostics, and side-agent lanes are easiest to supervise in a cockpit. Once
the runtime is reliable, the same orchestration model can power CLI automation,
IDE/ACP adapters, desktop, web, and team workflows.

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

RoboCode should become:

- a local-first AI coding operator cockpit
- a supervisor for other coding agents, not only a provider client
- a structured fact and evidence layer over messy transcripts, logs, diffs, and
  test output
- a token-efficiency engine that decides what context each agent needs and
  explains what was omitted
- a safe extension runtime for providers, MCP servers, skills, hooks, ACP
  agents, shell jobs, and future integrations

RoboCode should avoid becoming:

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

RoboCode should support ecosystem growth while keeping one permission and
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
- Every user-visible feature needs a real screenshot or deterministic visual
  artifact.
- New adapters start read-only or supervised before becoming mutating.
- New extension surfaces start as descriptor/doctor/probe before invocation.
- Do not add more automation until current automation can explain itself.
- Prefer one excellent end-to-end lane over many half-working integrations.
- TUI polish matters, but only when it improves operator confidence.

## Recommended Next Sequence

After `0.1.16`, the likely sequence is:

1. `0.1.17`: Daily Coding Loop Baseline. Prove DeepSeek-first setup,
   interactive provider/model configuration, switch-model recovery, scoped
   edit, approval, test, diff, final summary, and resume evidence in one
   deterministic workflow. Lightweight spec/steering is included only as
   task-brief support.
2. `0.1.18`: Failure Recovery And Review Gates. Make failed tests, diff
   review, apply conflict, rollback, rerun, and final readiness states obvious.
3. `0.1.19`: Delegated Lane Usefulness. Make one Codex/Claude/shell delegated
   review workflow dependable enough for real use.
4. `0.1.20`: Usability Beta. A clean install should complete the daily coding
   loop and one delegated review loop with documented screenshots and smoke
   evidence.
5. `0.2.0`: Spec-driven, evidence-driven multi-agent workflow baseline.

This sequence keeps RoboCode focused on the wedge: not maximum autonomy, but
maximum operator trust.
