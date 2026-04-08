# Reference Analysis

This document captures the parts of `.ref/claude-code-main` that matter for the
Rust reimplementation.

## What The Reference Project Is

- A large TypeScript terminal agent platform built around a shared query loop.
- Tool execution, permissions, session persistence, and slash commands are all
  first-class runtime concepts.
- Many parts of the reference snapshot are product-specific or operationally
  heavy, including remote bridge flows, MCP, voice, cron, LSP, and feature
  flags.

## Architectural Spine To Preserve

### `main.tsx`

- Startup orchestration.
- Bootstraps configuration, tool registry, command registry, session state, and
  runtime environment.

### `QueryEngine.ts`

- Owns the conversation loop.
- Receives user input, calls the model, executes tools, writes transcript
  entries, and continues until the assistant finishes the turn.

### `Tool.ts` and `tools.ts`

- Define the tool contract and the shared registry.
- Every tool invocation flows through a common permission-aware runtime path.

### `types/permissions.ts`

- Permission modes are part of the domain model, not just UI state.
- Rules can come from multiple sources and can affect tool behavior differently.

### `utils/sessionStorage.ts`

- JSONL transcript is the durable source of truth.
- Session indexing, resume behavior, and transcript-derived metadata are layered
  on top of the canonical transcript.

## Carry Over In V1

- Shared session engine.
- Shared tool execution pipeline.
- Multi-mode permissions.
- Append-only JSONL transcripts.
- Slash commands for a core set of runtime controls.
- Resume support.
- Provider abstraction that can target multiple API families instead of locking
  the runtime to a single vendor.
- Built-in web lookup tools for search and fetch, implemented through the same
  permission-aware runtime path as local tools.

## Postpone

- MCP and remote server resources.
- Bridge and remote control.
- LSP integration.
- Cron and automations.
- Voice.
- Team agents and swarms.
- Rich terminal UI overlays.

## Drop Or Simplify

- Bun-specific feature flags.
- Product analytics and growth gates.
- Complex environment-specific launch logic.
- Highly specialized internal commands.
- Large UI component tree.

## Rust Translation Strategy

- Treat the reference project as a behavioral spec, not as a line-by-line port.
- Keep the core loop explicit and strongly typed.
- Favor small crates with clear domain boundaries.
- Prefer debuggable, file-backed state over hidden global process state.
- Build around portability so the same engine can support POSIX and PowerShell
  execution adapters.
