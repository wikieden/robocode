# Code Agent Benchmark

Last refreshed: 2026-05-23

## Scope

This benchmark covers representative GitHub/open-source coding agents and major commercial coding-agent products. It is not literally every repository that calls itself an agent; the useful comparison set is the tools that shape developer expectations for terminal, IDE, GitHub, and multi-agent workflows.

Sources reviewed include official GitHub repositories and product documentation for OpenAI Codex CLI, Anthropic Claude Code, Google Gemini CLI, Aider, OpenHands, OpenCode, Goose, Cline, Continue, JetBrains Junie, GitHub Copilot cloud agent, Windsurf Cascade, Sourcegraph Cody/Amp direction, and Amazon Q/Kiro CLI.

## Executive Summary

Viden should not compete by being "one more coding CLI". The stronger positioning is:

- local-first Rust agent runtime with provider/plugin openness;
- strong permission, transcript, and workflow audit invariants;
- a TUI that acts as a command center;
- companion screens that run real side work through supervised terminal lanes;
- adapter-based orchestration of other coding tools such as `codex`, `claude`, `junie`, or user-defined commands.

The clear market pattern is that successful agents are becoming multi-surface and multi-session:

- Claude Code has terminal, IDE, desktop, browser, background agents, agent teams, and Agent SDK.
- GitHub Copilot cloud agent can be started from GitHub, Issues, IDEs, GitHub CLI, MCP hosts, Raycast, mobile, Jira, Slack, and CI failure contexts.
- Junie targets terminal, IDE, CI/CD, and GitHub Action triggers.
- Windsurf Cascade emphasizes checkpoints, queued messages, tool calling, terminal access, linter integration, and simultaneous Cascades.
- OpenHands separates SDK, CLI, and local GUI, with a path toward many agents in cloud.

Viden's differentiator should be not just "we can run code", but "we can supervise a small bench of coding agents/tools from one auditable terminal cockpit".

## Representative Project Snapshot

GitHub metadata was checked with `gh repo view` on 2026-05-23.

| Project | Surface | Language | Stars | License | Useful lessons for Viden |
| --- | --- | ---: | ---: | --- | --- |
| OpenAI Codex CLI | terminal, IDE, desktop/cloud ecosystem | Rust | 84.8k | Apache-2.0 | Rust terminal agent, local execution, strong brand expectation for CLI polish. |
| Google Gemini CLI | terminal, automation, MCP | TypeScript | 104.5k | Apache-2.0 | Built-in tools, MCP, non-interactive script mode, large context expectations. |
| Aider | terminal pair programming | Python | 45.2k | Apache-2.0 | Git-native editing, repo map, broad model support, simple durable UX. |
| OpenHands | SDK, CLI, local GUI, cloud | Python | 74.6k | Mixed/core MIT | Agent platform mindset: SDK first, multiple clients, scalable agent runs. |
| Goose | desktop, CLI, API | Rust | 45.7k | Apache-2.0 | Extensible local agent with desktop/CLI/API surfaces; close to Viden's Rust/local-first instincts. |
| Cline | IDE, SDK, CLI assistant | TypeScript | 62.2k | Apache-2.0 | Explicit approvals, editor/browser/terminal tool use, SDK extraction trend. |
| Continue | IDE, CLI, CI checks | TypeScript | 33.3k | Apache-2.0 | Config/rules as source-controlled assets; useful pattern for team policy. |
| OpenCode | terminal TUI | Go | 12.7k | MIT | TUI, permission dialogs, logs page, sub-task agent tool, LSP integration. |
| Amazon Q Developer CLI | terminal, now Kiro CLI direction | Rust | 2.0k | Apache-2.0/MIT | Rust CLI precedent, but open-source project is no longer the active product line. |
| JetBrains Junie | terminal, IDE, CI/CD, GitHub Action | Shell wrapper/product CLI | 259 | Proprietary service terms | Strong multi-surface task delegation model; BYOK and GitHub Action triggers matter. |

## Commercial / Big-Company Patterns

### OpenAI Codex

Codex CLI is a local terminal coding agent, with IDE and desktop/cloud surfaces around it. The repo is Rust-heavy and Apache-licensed. For Viden, this validates Rust as a credible implementation choice and sets the bar for terminal ergonomics, install story, and local trust.

Alignment:

- keep Viden Rust-native;
- treat terminal UX as product, not debugging output;
- support external `codex` lanes as a first-class adapter instead of pretending to replace it immediately.

### Anthropic Claude Code

Claude Code is a terminal-first agent that reads code, edits files, runs commands, and integrates with dev tools. Its newer direction includes multiple agents, background agents, and Agent SDK style orchestration.

Alignment:

- Viden should support `claude` as a terminal lane preset;
- build `task envelope -> adapter -> lane -> inspect/accept/revise` before deep native multi-agent;
- eventual native subagents should copy the lead/worker visibility pattern, not just hidden parallel calls.

### GitHub Copilot Cloud Agent

Copilot is becoming a task delegation layer around repositories. It can start work from GitHub Issues, dashboards, IDE chat, GitHub.com `/task`, GitHub CLI `gh agent-task create`, MCP server tools, mobile, Raycast, Jira, Slack, new repo creation, and failing workflow runs.

Alignment:

- Viden needs a task intake model that is not only chat input;
- GitHub issue/PR/CI failure should eventually become task envelopes;
- lane logs should be followable in real time, similar to cloud-agent session logs;
- PR creation/review integration should be a later milestone after local lane acceptance works.

### Google Gemini CLI / Antigravity Direction

Gemini CLI is open-source, terminal-first, MCP-enabled, has built-in file/shell/web/search tools, and supports non-interactive automation. Google is also moving parts of the experience toward richer agent platforms.

Alignment:

- MCP support is table stakes for V3;
- non-interactive lane execution is important before full PTY attach;
- custom commands/extensions should be config-backed, not hardcoded.

### Cursor / Windsurf

Cursor and Windsurf are IDE-native agent environments. Windsurf Cascade shows several important UX primitives: queued messages, tool calling, terminal integration, linter integration, checkpoints/reverts, real-time awareness, simultaneous agents, and ignore rules.

Alignment:

- TUI should support queued lane follow-ups;
- named checkpoints/reverts map naturally to git/worktree snapshots;
- ignore/context policy must be visible;
- diagnostics/linter panels should feed directly into lane task envelopes.

### JetBrains Junie

Junie is a coding agent across terminal, IDE, CI/CD, and GitHub Actions. It supports BYOK and issue/PR trigger workflows via `@junie-agent` patterns.

Alignment:

- support BYOK/provider openness as a product principle;
- design GitHub Action / CI mode eventually around the same task envelope format;
- lane presets should be user-configurable and not tied to one vendor.

### Sourcegraph Cody / Amp Direction

Sourcegraph's strength is context retrieval over large codebases. Cody's agentic context fetching uses search, codebase files, terminal, web, MCP, and context reflection before answering.

Alignment:

- Viden should make context gathering a visible phase in task envelopes;
- LSP/search/git/doc context should be selected, not blindly dumping transcript;
- for large repos, context quality is a bigger differentiator than model choice.

### Amazon Q / Kiro

Amazon Q Developer CLI was a Rust terminal agent, but the open-source project now points users to Kiro CLI as the maintained product path. The security incident around malicious prompts in coding assistants is also a warning.

Alignment:

- keep prompt/context supply chain auditable;
- never hide destructive permissions;
- external tool lanes must be killable and preferably isolated.

## Open-Source Agent Lessons

### Aider

Aider wins by being Git-native and simple. Its repo map idea is valuable because it turns codebase context into a compact, durable structure rather than repeatedly scanning everything.

Viden alignment:

- add a compact project map panel/adapter later;
- make diffs and changed files central to lane acceptance.

### OpenHands

OpenHands is a platform, not only a CLI. The SDK/CLI/GUI split is useful: core agent runtime should not be fused to one UI.

Viden alignment:

- keep `viden-core` as the agent/session engine;
- keep TUI as one client over shared state;
- future automation/GitHub mode should reuse the same task/lane records.

### OpenCode

OpenCode has a terminal TUI, permission dialogs, log pages, LSP integration, and an `agent` tool for subtasks.

Viden alignment:

- implement permission modal and logs page early;
- make `/lane inspect` feel like a first-class log/review page;
- sub-agent lanes should be explicit task records, not hidden model calls.

### Goose / Cline / Continue

These validate extensibility and policy:

- Goose: local agent with desktop/CLI/API surfaces.
- Cline: explicit approvals for file/browser/terminal actions, strong IDE workflow.
- Continue: source-controlled checks/config and team policy.

Viden alignment:

- external tool adapters should be config-driven;
- project-local policy/rules should be first-class;
- approval mode must remain visible in the top rail and composer.

## Capability Matrix

| Capability | Market baseline | Viden current | Viden target |
| --- | --- | --- | --- |
| Terminal agent | Codex, Claude, Gemini, Aider, OpenCode, Goose, Junie | Lightweight TUI shell exists | Full approved main-screen TUI |
| Permission UX | Claude/OpenCode/Cline/Windsurf all foreground approvals | Basic TUI approval prompt | Modal approval with apply-to-all and policy visibility |
| Multi-session / background work | Claude background agents, GitHub sessions, Cursor/Windsurf multi-agent, Junie CI/GitHub | Not yet | Screen registry + terminal lanes |
| External tool orchestration | Emerging through SDKs, MCP, GitHub custom agents | Design only | Adapter presets for `codex`, `claude`, `junie`, user commands |
| Task delegation envelope | GitHub/Jira/Issue/PR prompts; Claude agent teams | Tasks/memory exist | Durable task envelope shared by lanes |
| Result observation | GitHub session logs, PRs; Windsurf checkpoints; OpenCode logs | Transcript only | Lane logs + diff + tests + accept/revise/discard |
| Isolation | GitHub branches, Devin workspaces, worktrees/checkpoints | Worktree practice in agent workflow | Per-lane worktree default for mutating external tools |
| Context system | Sourcegraph context, Aider repo map, MCP | LSP/search/tools exist | Context bundle builder for task envelopes |
| MCP/plugins | Claude/Gemini/Windsurf/Sourcegraph/GitHub | Provider plugins only | V3 MCP/tool ecosystem |
| CI/GitHub agent | Copilot, Junie, Devin, Codex cloud | Not yet | Later: issue/PR/CI task envelopes |

## Strategic Gaps for Viden

1. Main TUI is still too thin.
   - The approved single-screen design must land before feature sprawl.

2. Companion screens must be productive.
   - The core unit is a supervised lane, not a decorative panel.

3. External agent interoperability should be a feature.
   - Viden can become the cockpit that launches `codex`, `claude`, `junie`, `gemini`, or any configured CLI in bounded lanes.

4. Acceptance workflow is more important than raw generation.
   - "What changed? Did tests pass? Is it safe to integrate?" should be the primary lane review path.

5. Isolation needs to be opinionated.
   - Default file-mutating external lanes should run in per-lane worktrees once practical.

6. Context policy should be explicit.
   - Avoid full-transcript dumps; send task objective, selected files, diagnostics, plan excerpt, and constraints.

## Recommended Viden Roadmap Alignment

### R1: Main TUI Parity

Implement the approved main screen:

- top global status rail;
- transcript timeline;
- workspace/active tasks/LSP/provider/recent files right rail;
- centered approval modal;
- composer and bottom status bar.

Reason: this gives Viden a recognizable product surface and fixes current TUI shallowness.

### R2: Lane Runtime MVP

Implement lane records and `/lane run <command>`:

- durable lane metadata;
- command log capture;
- status transitions;
- stop/archive;
- `/lane inspect`.

Reason: this proves companion screens can supervise real work before integrating other AI agents.

### R3: External Tool Adapters

Implement `codex`, `claude`, and generic `ask` adapters:

- task envelope rendering;
- input modes: `stdin`, `prompt-file`, and `manual` first;
- output/log capture;
- changed-file and diff detection;
- `/lane accept`, `/lane revise`, `/lane discard`.

Reason: direct vendor replacement is less valuable than orchestrating the tools users already trust.

### R4: Companion Workspaces

Implement `AGENTS` and `OPS` workspaces:

- `AGENTS`: lane board, subtask queue, current work, blockers, approvals.
- `OPS`: tests, diagnostics, diff, provider/tool telemetry, terminal/log pane.

Reason: matches the user's multi-monitor workflow and makes side screens useful.

### R5: Isolation and Review

Add per-lane worktree support:

- create lane branch/worktree;
- run external tool there;
- inspect diff;
- merge/cherry-pick/apply only after acceptance.

Reason: this is the safety line against uncontrolled external agent edits.

### R6: GitHub/CI Intake

Turn issues, PR comments, CI failures, and local diagnostics into task envelopes.

Reason: aligns with Copilot/Junie/GitHub's delegation pattern without sacrificing Viden's local-first core.

## Design Implications for the TUI

- The right rail on the main screen should show "active work lanes" in addition to active tasks.
- The bottom composer should accept lane commands naturally.
- Approval modal must cover both Viden-native tools and lane actions.
- Companion screens should make it obvious whether a pane is only observing, running a process, or attached interactively.
- A lane card should always show: id, tool, cwd/worktree, task, status, last output, changed files, tests, and next action.
- The strongest UX is not "many agents"; it is "many agents with visible state, narrow scope, and clean handoff".

## Immediate Decision

Build the next Viden TUI slice around this hierarchy:

1. Main TUI visual parity.
2. Lane data model and `/lane run`.
3. `/lane inspect` with logs/diff/tests.
4. `codex` and `claude` adapters through task envelopes.
5. Companion workspaces that show and attach lanes.

Do not spend more time on secondary visual-only dashboards until lane runtime exists.
