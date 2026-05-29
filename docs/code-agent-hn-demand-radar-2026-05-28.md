# Code Agent HN Demand Radar - 2026-05-28

Chinese version: [code-agent-hn-demand-radar-2026-05-28.zh-CN.md](code-agent-hn-demand-radar-2026-05-28.zh-CN.md)

## Scope

This note refreshes RoboCode's competitor read with Hacker News discussion
signals. It is not a popularity ranking. It is a demand radar: what developers
complain about, what they praise, and what RoboCode is still missing.

Sources:

- HN: Ask HN: Why are AI coding agents not working for me?
- HN: Ask HN: How Do You Actually Use Claude Code Effectively?
- HN: Ask HN: Senior software engineers, how do you use Claude Code?
- HN: Show HN: Real-time dashboard for Claude Code agent teams
- HN: Parallel agents in Zed
- HN: Emacs agent-shell (powered by ACP)
- HN: Tell HN: Anthropic no longer allowing Claude Code subscriptions to use
  OpenClaw
- OpenAI Codex CLI docs
- Claude Code user FAQ
- Zed external agents docs
- Kiro docs
- Kilo product docs

## HN-Derived Demand Themes

### 1. Context Management Is A Core UX, Not An Internal Detail

HN users repeatedly describe agent quality degrading as context grows. Strong
users treat context windows as a scarce resource: summarize, commit, compact,
clear, and split tasks before the agent gets confused.

RoboCode implication:

- ContextBundle must become a visible operator surface, not only provider
  plumbing.
- The product should show included sources, omitted sources, estimated tokens,
  and compaction decisions before long tasks start.
- The next version should turn ContextBundle v1 into a budget policy with source
  priority and reason codes.

Gap:

- `0.1.13` shows pressure and injects context, but does not yet let the user
  actively curate, pin, omit, or split context.

### 2. The Winning Workflow Is Plan -> Spec -> Execute -> Review

HN comments from experienced Claude Code users converge on planning with the
agent, documenting decisions, breaking work down, then implementing in smaller
chunks. Kiro makes this explicit with steering files and specs: requirements,
design, and task phases.

RoboCode implication:

- RoboCode should add a lightweight spec/steering loop before broad autonomous
  work.
- Task envelopes should include requirements, constraints, design decisions,
  expected tests, and acceptance criteria.
- A future `/spec` or `/plan task` surface should produce files that lanes can
  consume.

Gap:

- RoboCode has tasks, memory, and release plans, but not an in-product
  spec-driven workflow that turns a user request into requirements/design/tasks
  before delegation.

### 3. Multi-Agent Visibility Is The Pain, Not Merely Spawning Agents

HN feedback around agent dashboards and parallel agents is clear: people need a
live timeline of what each agent actually did, not sanitized summaries. The hard
question is also quality: agents can be running fine while producing bad
outputs.

RoboCode implication:

- Side screens should show event timelines: prompts, tool calls, file changes,
  test commands, approvals, failures, retries, and final evidence.
- Reviewers/testers should be separate evidence lanes, not hidden subcalls.
- Side-2 should answer "why should I trust this result?"

Gap:

- Current lane evidence exists, but the operator timeline is still too coarse.
  RoboCode needs an audit replay surface per lane.

### 4. Parallel Agents Need Isolation Beyond Git Worktrees

Zed's parallel-agent thread produced a practical blocker: git worktrees are not
enough when tests share databases, migration state, caches, or services. Users
also want cleanup hooks so worktrees and test environments do not pile up.

RoboCode implication:

- A lane must declare not only `worktree`, but also test data scope, service
  ports, env vars, cache dirs, database schema, and cleanup command.
- Add lane preflight and teardown hooks.
- Surface isolation risk before launching parallel lanes.

Gap:

- RoboCode has per-lane worktree direction and review/apply safety, but no
  structured test-data or service isolation model.

### 5. ACP And Native Config Reuse Are Real User Needs

HN discussion around ACP frames it like LSP for agents: users do not want every
editor/tool to implement a separate Claude, Codex, Gemini, Aider, Goose, and
custom wrapper. Another pain point is repeated config: MCPs, credentials,
project/user config, and agent-native settings are already fragmented.

RoboCode implication:

- ACP should be treated as a serious adapter boundary after the Codex/Claude
  happy paths are stable.
- Adapter doctor should report which config is RoboCode-owned versus
  agent-native.
- Do not copy secrets or duplicate MCP config unless there is a strong reason.

Gap:

- RoboCode currently plans ACP probes, but not a concrete compatibility target
  such as "run one ACP server and map events into lane evidence."

### 6. Cost, Rate Limits, And Provider Economics Are Product Requirements

HN discussion around Claude subscriptions, third-party harnesses, Cursor
billing, and OpenClaw shows that users care about cost transparency, rate-limit
behavior, and whether automated agents burn through quota invisibly.

RoboCode implication:

- The operator cockpit should show token/cost/rate budget per provider and per
  lane.
- Long-running loops should have ceilings: max turns, max tokens, max cost, max
  wall-clock time.
- Agents should explain why they are asking for another expensive step.

Gap:

- RoboCode has context pressure and provider health, but not a cost ledger,
  quota forecast, or per-lane budget stop condition.

### 7. Credentials And Agent Tool Access Are A Trust Boundary

HN discussions around agent credential proxies reflect a strong fear: agents
need access to tools, but should not see or leak secrets. Claude Code and Kiro
also emphasize MCP, hooks, and privacy/security surfaces.

RoboCode implication:

- API keys should stay out of transcripts, screenshots, and model context.
- Future MCP/plugin calls need credential brokering or least-privilege
  capability boundaries.
- Permission prompts should distinguish "agent uses a capability" from "agent
  sees the secret."

Gap:

- RoboCode avoids storing secrets in setup, but does not yet have a credential
  broker / proxy pattern for MCP, external APIs, or agent adapters.

### 8. Hooks Are Useful Only When They Are Observable And Blocking

Claude Code and Kiro both expose hooks. HN users value hooks for notifications,
lint/test automation, and hard-blocking unsafe actions, but also complain that
DIY hook behavior is hard to debug.

RoboCode implication:

- Hooks should be typed, logged, testable, and visible in side-2.
- PreToolUse-style hooks should be able to block with a structured reason.
- Hook outputs should become evidence rows, not hidden shell noise.

Gap:

- RoboCode has extension boundary planning, but no hook lifecycle or hook
  evidence model yet.

## Competitor Gap Matrix

| Competitor / pattern | Strong signal | RoboCode gap | Product response |
| --- | --- | --- | --- |
| Claude Code | Mature terminal loop, MCP, hooks, skills, subagents, checkpoints, non-interactive mode | RoboCode has better cockpit ambition but weaker built-in automation surfaces | Add hook lifecycle, spec/steering, and reproducible Claude lane |
| Codex | Local Rust CLI, strong install story, cross-surface direction, evidence expectations | RoboCode should not try to replace Codex yet | Make Codex the reference delegated lane backend |
| Zed | ACP external agents, editor-native threads, worktree parallelism | RoboCode lacks ACP runtime and editor-native file context | Build ACP probe/event mapping after P0 lanes; keep TUI as ops cockpit |
| Kiro | Specs, steering files, hooks, MCP, privacy-first framing | RoboCode lacks in-product spec/steering workflow | Add task envelope spec phases and project steering files |
| Kilo / OpenClaw | Multi-surface agent use, many models, cloud/slack/automations | RoboCode is TUI-first and local-first only | Do not chase cloud yet; add cost/rate ledger and future automation boundary |
| Aider | Git-native simplicity and repo map | RoboCode context can still be too transcript-centric | Add compact repo/project map into ContextBundle |
| OpenHands / Goose | Platform/SDK shape, CLI + GUI + API | RoboCode's runtime is still TUI-led | Keep core reusable, but defer API/server until TUI loop stabilizes |
| DeepSeek-TUI | Dense terminal-native provider experience | RoboCode has stronger orchestration goal but still rougher terminal UX | Keep deterministic screenshots and live provider smoke as release gates |

## Priority Changes For 0.1.14

Keep the current `0.1.14` direction, but tighten it:

1. Add `P0-HN`: lane event timeline / audit replay.
   - Show prompt, tool call, command, file change, approval, test, failure,
     retry, and final output chronology.
2. Add `P0-HN`: isolation preflight.
   - Worktree plus test DB/schema/cache/service-port declarations and cleanup.
3. Promote cost/rate budget from P2 to P1.
   - Per-lane token/cost/time ceilings and visible burn rate.
4. Add `P1`: lightweight steering/spec files.
   - Project conventions and requirements/design/tasks envelope.
5. Add `P1`: hook probe design.
   - Pre/post tool hooks, blocking hooks, notifications, and hook evidence.
6. Add `P1`: credential boundary design.
   - Secret handles, not secret values, in MCP/plugin/agent context.

## Product Bet

The HN signal says the next wedge is not "more autonomous agents." The wedge is:

> Make multi-agent coding observable, bounded, reviewable, and economically
> predictable.

RoboCode's TUI-first strategy is still sound, but only if the side screens
become evidence and control surfaces rather than dashboards.

