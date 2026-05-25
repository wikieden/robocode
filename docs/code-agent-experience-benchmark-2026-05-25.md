# Code Agent Experience Benchmark - 2026-05-25

This focused refresh compares the current programming experience of Codex,
Claude Code, DeepSeek-TUI / CodeWhale, and Zed. It narrows the broader
`docs/code-agent-benchmark.md` into concrete lessons for RoboCode `0.1.5`.

Sources checked on 2026-05-25:

- [OpenAI Codex overview](https://platform.openai.com/docs/codex/overview)
  and [OpenAI Codex product page](https://openai.com/codex/)
- [Claude Code overview](https://code.claude.com/docs/en/overview),
  [Claude Code subagents](https://code.claude.com/docs/en/sub-agents), and
  [Claude Code hooks](https://code.claude.com/docs/en/agent-sdk/hooks)
- [DeepSeek-TUI site](https://deepseek-tui.com/en) and
  [DeepSeek-TUI GitHub repository](https://github.com/Hmbown/DeepSeek-TUI)
- [Zed AI overview](https://zed.dev/docs/ai/overview),
  [Zed Agent Panel](https://zed.dev/docs/ai/agent-panel),
  [Zed Parallel Agents](https://zed.dev/docs/ai/parallel-agents.html), and
  [Zed External Agents](https://zed.dev/docs/ai/external-agents.html)

## Positioning Read

RoboCode should not try to beat each product on its home field:

- Codex owns high-trust OpenAI-backed task execution across cloud, desktop, and
  terminal workflows.
- Claude Code owns a mature terminal coding loop with permissions, hooks,
  subagents, MCP, and enterprise workflow surfaces.
- DeepSeek-TUI / CodeWhale owns a dense terminal-native DeepSeek V4 experience,
  including mode switching, long context expectations, and fast iteration around
  TUI details.
- Zed owns editor-native agent ergonomics: agent panel, thread sidebar, inline
  code context, parallel threads, external agents, and worktree-oriented
  isolation.

RoboCode's strongest lane is narrower and sharper:

> A local-first terminal cockpit that lets one main agent supervise code edits,
> approvals, tests, diagnostics, and external coding tools such as `codex`,
> `claude`, shell jobs, and DeepSeek-backed lanes.

## Experience Comparison

| Product | Experience strength | What RoboCode should borrow | What RoboCode should avoid copying directly |
| --- | --- | --- | --- |
| Codex | Delegated task completion, isolated environments, central diff/review expectations, multi-surface continuity | Treat diff/test evidence as a first-class result; keep task runs isolated and reviewable; make install/release friction low | Do not make cloud delegation the center of `0.1.5`; RoboCode's immediate advantage is local TUI supervision |
| Claude Code | Terminal-native action loop, conservative permissions, hooks, subagents, MCP, clear workflow automation | Build a smooth approve/test/fix loop; add light permission profiles and hooks after approval UX is stable; keep subagent state visible | Do not hide work in invisible subagents; do not expand MCP before the core coding loop is steady |
| DeepSeek-TUI / CodeWhale | Dense terminal UI, DeepSeek V4 family focus, long-context/cost/provider awareness, fast TUI iteration | Keep DeepSeek V4 Flash a real smoke target; make provider health and context pressure visible; use compact side panels | Do not chase every visual flourish; avoid adding panels that are not backed by real state |
| Zed | Editor-native agent panel, threads, parallel agents, external agents via ACP, inline context selection, worktree isolation | Make lanes feel like terminal threads; side screens should show agent lanes, tests, diagnostics, and next actions; keep external agents configurable | Do not try to recreate a full editor; RoboCode should integrate with editors, not replace them |

## RoboCode 0.1.5 Product Delta

The gap is not model capability first. The gap is operator confidence:

- The user must always know where input focus is.
- The user must see what is pending, what changed, what was tested, and what is
  safe to approve.
- External agents must be supervised lanes with state, logs, changed files, and
  a next action, not opaque subprocesses.
- Side screens must carry work evidence, not decorative dashboards.

## Recommended 0.1.5 Improvements

### 1. Stabilize the hands-on coding surface

Ship this before adding larger workflows:

- taller composer with a visible blinking cursor;
- correct IME candidate placement near the input line;
- resize-safe layout with no stale borders or right-rail drift;
- unified no-modal and modal theme colors;
- approval modal with default focus on approve, keyboard shortcuts, mouse
  targets, immediate dismissal after decision, and a visible focused action.

Why: Codex, Claude Code, DeepSeek-TUI, and Zed all make the user feel the active
interaction point. RoboCode still loses trust when the input cursor, modal, or
panel alignment feels uncertain.

### 2. Make diff and test evidence the center of the loop

Add a compact coding evidence model:

- changed files with added/modified/deleted status;
- short diff summary before approval and after mutation;
- `/test` or `/run test` command that stores command, exit code, duration, tail,
  failing file, and recommended next action;
- right rail and `/status` both surface the latest diff/test state.

Why: Codex and Zed set the expectation that code changes are reviewed centrally.
Claude Code sets the expectation that the tool can edit and verify in one loop.
RoboCode should make "what changed and did it pass" impossible to miss.

### 3. Promote lanes from logs to supervised work threads

For each lane, render:

- lane id, tool, command/template, workspace/worktree, pid/session, status, and
  elapsed time;
- last meaningful output after ANSI/OSC/prompt-noise sanitization;
- changed files and artifacts;
- last test result or verification result;
- recommended next action: inspect, send, attach, test, accept, revise, or
  cleanup.

Why: Zed's parallel threads and external agents are the closest mental model.
RoboCode can provide the terminal version: less editor-native, more operations
cockpit.

### 4. Add minimal permission automation, not full policy sprawl

For `0.1.5`, use a small profile layer:

- always allow low-risk read-only commands;
- optionally auto-allow configured test commands;
- always confirm file writes, deletes, network calls, and shell commands outside
  the workspace unless the user chooses a stronger mode;
- record every permission decision in the transcript.

Why: Claude Code's permissions and hooks are useful because they reduce
interruptions without hiding risk. RoboCode should copy that principle, not the
full surface area yet.

### 5. Keep Zed-like context explicit but terminal-native

Add `/context` as a visible context bundle:

- current task;
- selected dirty files;
- recent diagnostics;
- latest test result;
- relevant lanes;
- provider/model/context pressure;
- explicit files that will be sent to the next model turn.

Why: Zed lets users attach editor selections and thread context. RoboCode needs
the terminal equivalent so users can reason about what the agent is seeing.

## 0.1.5 Execution Order

1. Composer, cursor, IME, resize, and modal focus.
2. Slash command palette and approval modal mouse/keyboard handling.
3. Diff/test evidence model and `/status` integration.
4. Lane inspect page and side-screen lane/test/diagnostic panels.
5. Light permission profiles and `/context`.

Do not move MCP, remote automation, or a full terminal emulator into `0.1.5`
unless they unblock one of the above items.

## Acceptance Evidence

The release should not be judged only by screenshots. It needs proof that the
programming loop works:

- fallback-provider TUI smoke: edit file, approve, show diff, run test, exit;
- DeepSeek V4 Flash TUI smoke: complete a small coding task with tool calls and
  verification;
- shell lane smoke: create, inspect, capture output, cleanup;
- tmux/external lane smoke: attach command, capture meaningful output, inspect
  lane state;
- snapshot/previews for no-modal, modal, command palette, lane inspect, side-1,
  side-2, compact, normal, and wide layouts.

