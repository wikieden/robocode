# RoboCode Staged Roadmap

## Purpose

This roadmap translates the full RoboCode product requirements into delivery
stages. It is intentionally derived from the target product, not from the
history of the current repository.

## Stage Definitions

### V1: Local Core CLI

Goal:
Ship a reliable, local-first developer agent CLI with durable sessions,
permissions, and high-value local tools.

Required capabilities:

- interactive REPL
- startup configuration model
- provider abstraction
- file, search, shell, web, and Git tool families
- permission modes and approvals
- append-only transcript plus resume
- foundational slash commands

Exit criteria:

- users can run code-reading and code-editing workflows locally end to end
- tool calls, approvals, and transcript history remain auditable
- provider switching does not require core-engine changes
- sessions can be resumed reliably by project

### V2: Developer Enhancement Layer

Goal:
Turn the local CLI core into a more capable day-to-day development assistant.

Required capabilities:

- broader command surface
- better session browsing and summaries
- stronger Git and diff workflows
- LSP integration
- memory and task management
- richer TUI and interaction patterns

Exit criteria:

- users can complete more of the development workflow without dropping to ad
  hoc shell usage
- semantic code assistance exists beyond grep and file editing
- session and task continuity feel deliberate instead of incidental

### V3: Platform Expansion Layer

Goal:
Expand RoboCode from a local agent CLI into an extensible developer platform.

Required capabilities:

- MCP integration
- skills and plugins
- multi-agent coordination
- bridge and remote session support
- automation and cron-style workflows

Exit criteria:

- external tool ecosystems can plug into RoboCode through stable interfaces
- remote and integrated clients can reuse the same execution and permission
  model as local sessions
- multi-agent workflows do not bypass transcript and permission guarantees

### Long-Term Platform Features

Goal:
Add product-scale capabilities that are useful only after core workflows are
stable.

Target capabilities:

- voice interaction
- multi-device handoff
- analytics and managed settings
- feature-flag infrastructure
- reference-project-specific operational tooling where still justified

Exit criteria:

- advanced productization does not destabilize the core local developer
  workflows

## Priority Rules

- V1 behavior is the baseline contract for all later work
- V2 should deepen local developer effectiveness before broad platform sprawl
- V3 should reuse V1 and V2 execution invariants instead of introducing new
  side-channel runtimes
- long-term platform features should follow, not lead, core workflow maturity

## Current Repository Mapping

The current repository already establishes an early V1 baseline through:

- the REPL and command loop
- config resolution
- provider abstraction
- permissions
- transcripts and resume
- Git and web tools

That does not change the roadmap ordering. It means RoboCode has started V1,
not that the later stages should be pulled forward prematurely.

