# ContextBundle And Token Efficiency Design

Chinese version: [context-bundle-token-efficiency.zh-CN.md](context-bundle-token-efficiency.zh-CN.md)

Last updated: 2026-05-27

## Goal

RoboCode's multi-agent orchestration should not depend on sending the full
transcript to every agent. `ContextBundle` is the shared context model reserved
in 0.1.11 for the 0.2.0 token-efficiency engine.

Its goals are:

- give each agent only the context needed for its current task
- let the main TUI explain context pressure, token budget, and context sources
- let agents collaborate through structured facts, artifacts, and evidence
  instead of copying long conversations to each other

## ContextBundle Fields

Minimum fields:

- `task`: current user goal or delegated subtask
- `selected_files`: explicitly selected or semantically retrieved files
- `diff`: workspace diff summary and key hunk summaries
- `diagnostics`: LSP, compiler, or test diagnostics
- `test_results`: command, exit code, duration, failure summary, and output tail
- `facts`: user constraints, design decisions, project conventions, and reusable memory
- `lane_summaries`: status summaries and artifacts from Codex, Claude, DeepSeek, shell, and other lanes
- `permissions`: allowed actions, actions requiring approval, and boundary constraints
- `budget`: token budget, model routing, cost ceiling, and context pressure for the agent

## Tool Output Compaction

The raw transcript remains the audit source of truth, but model input and TUI
display should compact tool output:

- long logs keep the failure summary, command, exit code, and final N tail lines
- repeated output is deduplicated by hash or adjacent repeated blocks
- test failures prioritize failing file, line, error message, and rerun command
- large diffs keep file summary, risky files, and key hunks first, expanding by
  file only when needed
- lane output enters `lane_summaries` unless the user explicitly inspects a lane

## Per-Agent Token Budget

Each agent lane should have its own budget:

- `planner`: small context, focused on decomposition and constraints
- `coder`: medium to high context, prioritizing files, diff, and diagnostics
- `reviewer`: medium context, prioritizing diff, tests, risk, and requirements
- `tester`: small context, prioritizing command, failures, and rerun evidence
- `researcher`: isolated budget so research does not pollute coding context

When context pressure is high, keep this priority order:

1. current task and user constraints
2. current diff and failure evidence
3. relevant file slices
4. recent lane summaries
5. historical transcript summaries

## TUI Surface

In 0.1.11, the TUI should at least show:

- current context window
- provider-reported token usage
- evidence source for `NOW WORKING`
- context pressure and configuration source in the side-2 MCP/context area

In 0.2.0, `ContextBundle` becomes a concrete runtime object and every agent turn
should be able to report:

- `bundle_id`
- included sources
- estimated tokens
- compaction decisions
- budget remaining
