# RoboCode Product Requirements

## Purpose

This document defines the complete product target for RoboCode as a Rust-based,
local-first agentic developer CLI derived from the behavioral model of
`.ref/claude-code-main`.

RoboCode is not a file-by-file port. It aims for high behavioral similarity in
the user-facing runtime model, command surface, and subsystem boundaries while
allowing a Rust-native internal architecture.

## Product Definition

### Positioning

RoboCode is a local-first, extensible developer agent that runs in the
terminal, understands a working directory, executes tools through a permission
gate, persists sessions, and can later expand into integrations, remote
operation, and coordinated multi-agent workflows.

### Primary Users

- Individual developers using AI assistance inside a local repository
- Repository maintainers who need auditable tool execution and resumable work
- Teams that want a path from local CLI usage to richer integrations over time

### Core User Jobs

- Read, search, edit, and generate code inside a repository
- Run shell and Git workflows with approval-aware execution
- Search the web and fetch supporting context into the session
- Resume prior sessions without losing tool or approval context
- Work in analysis-only or approval-heavy modes when risk is higher
- Grow into MCP, LSP, remote, and multi-agent usage without changing products

### Product Goals

- Match the reference project on core runtime behavior and subsystem shape
- Preserve strong auditability around tools, approvals, and session history
- Support cross-platform local development from the first stable release
- Keep the engine extensible enough to host integrations and advanced workflows

### Product Non-Goals

- Reproducing Bun, React, or Ink implementation details
- Copying every reference command verbatim
- Shipping the entire platform in the first release
- Reproducing product analytics and growth tooling before core workflows mature

## Core Runtime Model

### Startup and Configuration

RoboCode must start from a deterministic configuration merge model:

1. CLI flags
2. Environment variables
3. Project-local config
4. Global config
5. Built-in defaults

Configuration must cover at minimum:

- provider family and model
- API base and credentials
- permission mode
- session storage location
- request timeout and retry behavior
- additional working directories
- future integration toggles where required

### Session Model

A session is the durable unit of interaction. It owns:

- message history
- tool-call history
- permission events
- command events
- session metadata and summary fields
- working directory and scope metadata

The transcript is the durable source of truth. Any derived index must be
rebuildable from transcript files.

### Message and Tool Loop

RoboCode must preserve the reference system's central behavior:

- user input enters a shared engine
- slash commands are resolved through the same runtime domain, not a detached UI
- provider responses can emit assistant text, tool calls, and turn completion
- tool calls are normalized before execution
- every tool call flows through one shared runtime path
- tool results are reintroduced into the conversation and transcript
- the loop continues until the provider completes the turn

Required invariants:

- tool execution is never a side channel
- permissions are checked before execution, not after
- assistant tool-call intent is represented in session state
- transcript order is sufficient to reconstruct a session

### Permission Model

Permissions are a domain concept, not a purely interactive UI concept.

RoboCode must support named permission modes equivalent in intent to the
reference project:

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

The permission subsystem must support:

- allow, deny, and ask outcomes
- per-session rules
- persisted rules
- tool-scoped rules
- path-scoped rules where relevant
- additional working directories
- special handling for workflows that legitimately cross repo boundaries, such
  as worktrees and remote resources

### Session Persistence and Resume

The session layer must provide:

- append-only transcript storage
- rebuildable secondary indexing
- project-scoped session discovery
- session selectors such as latest, numeric list index, and id prefix
- enough metadata for summaries, sorting, and quick resume decisions

### Slash Commands

Slash commands are a first-class interface layer. RoboCode does not need to
copy every reference command name, but it must define complete command families
that cover the same behavioral categories over time.

Required command families:

- runtime control: help, model/provider selection, permissions, plan mode
- session control: sessions, resume, diff, share/export in later phases
- repository workflows: Git status, branch, diff, add, commit, restore, stash,
  worktree, and related flows
- environment and diagnostics: config, doctor, context, usage/cost, status
- integration management: MCP, plugins, skills, remote, auth
- collaboration and workflow: tasks, agents, teams, memory

### Provider Abstraction

The provider layer must stay vendor-agnostic.

Required capabilities:

- provider family selection
- model selection
- request timeout and retry policy
- text generation
- native tool-calling when supported
- structured error reporting
- future streaming and cancellation support across providers

The product target includes support for:

- Anthropic
- OpenAI
- OpenAI-compatible APIs
- Ollama or equivalent local model backends
- fallback or offline development mode

### Unified Tool Runtime

Tool execution must remain the single most stable interface boundary in the
system.

Every tool definition must include:

- public name and description
- mutating versus non-mutating classification
- input contract
- permission expectations
- execution handler
- serializable result shape

Minimum tool families for the complete product target:

- shell execution
- file read, write, and edit
- codebase search and globbing
- Git workflows
- web search and fetch
- MCP-backed tools
- LSP-backed actions
- agent, team, task, and remote-trigger tools in later phases

## Subsystem Requirements

### CLI / REPL / Slash Commands

Goal:
Provide the default local interaction surface for users working inside a repo.

Requirements:

- lightweight interactive REPL from the start
- progressively richer terminal UI over time
- discoverable command surface with help output
- command parsing that stays stable across providers and tools
- safe fallback behavior when advanced subsystems are unavailable

Phase priority:
- V1 core
- richer TUI in V2

### Configuration System

Goal:
Provide one predictable way to configure runtime behavior locally and globally.

Requirements:

- deterministic precedence
- explicit config schema
- compatibility-safe defaults
- environment and CLI overrides
- future migration path for config evolution

Phase priority:
- V1 core

### Provider System

Goal:
Support multiple model backends without coupling core logic to any one vendor.

Requirements:

- consistent internal provider contract
- vendor-specific protocol adapters
- native tool-calling where supported
- request retry and timeout policy
- compatibility behavior for providers with weaker protocol support

Phase priority:
- V1 core, deepened in V2

### Tool System

Goal:
Expose all actionable capabilities through a shared permission-aware runtime.

Requirements:

- single registry model
- consistent tool contract
- serializable results
- transcript visibility
- future pluggability for MCP, plugins, and agent-generated tools

Phase priority:
- V1 core, expanded continuously

### Permission System

Goal:
Make tool execution safe, auditable, and policy-aware.

Requirements:

- named modes
- explicit decisions
- rule persistence
- path scoping
- additional directories
- special-case handling for cross-root workflows
- later support for remote and integration-aware policies

Phase priority:
- V1 core, expanded in V2 and V3

### Session / Transcript / Resume

Goal:
Make the session durable, resumable, and inspectable.

Requirements:

- append-only transcript
- rebuildable index
- project-scoped session discovery
- fast resume
- better summaries and browsing in later phases

Phase priority:
- V1 core, enriched in V2

### Git Workflows

Goal:
Support local repository workflows directly inside the agent.

Requirements:

- inspect repository state
- stage and commit changes
- restore and stash workflows
- worktree support
- richer diff and branch workflows
- future PR-comment and review-oriented support where applicable

Phase priority:
- V1 core, expanded in V2

### Web Tools

Goal:
Allow the agent to retrieve external context without leaving the session loop.

Requirements:

- search and fetch
- transcript-visible results
- size and scope controls
- source-aware handling in future richer versions

Phase priority:
- V1 core, improved in V2

### MCP System

Goal:
Make remote tool ecosystems and external structured resources available through
the same runtime model as local tools.

Requirements:

- MCP server registration and lifecycle management
- MCP tool discovery and invocation
- permission-aware execution
- session-visible results
- command surface for MCP administration

Phase priority:
- V3

### LSP System

Goal:
Add semantic, language-aware code intelligence beyond shell and grep.

Requirements:

- language server management
- symbol- and reference-aware operations
- opt-in workflow integration with local tools
- graceful fallback when LSP is unavailable

Phase priority:
- V2

### Skills / Plugins

Goal:
Allow reusable workflows and third-party extensions without bloating core code.

Requirements:

- skill discovery and execution model
- plugin loading model
- clear trust boundary for local versus remote extensions
- command surface for listing and managing extensions

Phase priority:
- V3

### Multi-Agent / Team / Coordinator

Goal:
Support delegated and coordinated work beyond a single conversation thread.

Requirements:

- agent spawning
- inter-agent messaging
- team-level orchestration
- transcript-aware coordination
- permission and scope isolation between agents

Phase priority:
- V3

### Bridge / Remote / Server Mode

Goal:
Allow IDE-connected, remote, and service-oriented RoboCode usage beyond a local
terminal session.

Requirements:

- bridge protocol
- remote session transport
- permission callbacks across process boundaries
- server or daemon mode where required
- continuity with local session semantics

Phase priority:
- V3

### Memory / Tasks / Automation / Cron

Goal:
Support longer-lived workflows that outlast a single active prompt loop.

Requirements:

- persistent memory model
- task lifecycle management
- scheduled execution or reminders
- durable and session-scoped automation variants

Phase priority:
- V2 for memory and tasks
- V3 for automation and cron

### Voice

Goal:
Allow spoken interaction and voice-assisted workflows where they add value.

Requirements:

- voice capture and transcription
- voice session state
- fallback to text interaction

Phase priority:
- long-term

### UI / TUI / Visual Assist

Goal:
Move beyond a plain REPL when richer interaction improves comprehension.

Requirements:

- better diff presentation
- session browsers
- contextual permission prompts
- richer views for MCP, tasks, memory, and remote state

Phase priority:
- V2

### Operational Platform Features

Goal:
Support the long-term needs of a mature multi-environment product.

Requirements:

- analytics and usage tracking where appropriate
- feature flags
- managed settings
- policy limits and remote governance

Phase priority:
- long-term

## External Interfaces and Public Surface

### Command Surface Requirements

RoboCode must define stable command families rather than an ad hoc command pile.
The complete product target must cover:

- runtime control
- session control
- repository workflows
- diagnostics and config
- integrations
- collaboration
- platform administration

### Tool Contract Requirements

Public tool definitions must expose:

- stable name
- clear capability description
- declared mutability
- input contract
- permission expectation
- result format suitable for transcript storage

### Provider Configuration Interface

The public provider interface must allow users and integrators to choose:

- provider family
- model
- endpoint
- credentials
- timeout
- retry settings

### Permission Modes

The public permission surface must expose at least:

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

### Session Selectors

The public session interface must support:

- latest session selection
- list-based selection
- id-prefix selection
- project scoping

### Working Directory and Scope Controls

The public workspace model must support:

- primary working directory
- additional working directories
- Git worktree flows
- future remote or bridge-provided workspace scopes

### Future Integration Interfaces

MCP, remote, and multi-agent subsystems must be designed so they can plug into
the same command, permission, tool, and transcript model instead of creating
parallel runtimes.

## Non-Functional Requirements

- Cross-platform support for macOS, Linux, and Windows
- Recoverability through durable transcripts and rebuildable indexes
- Auditability for tools, permissions, and command-level actions
- Extensibility for providers, tools, plugins, and MCP integrations
- Performance suitable for interactive CLI use and long-running sessions
- Security through explicit approval and scope-aware execution
- Compatibility strategy that favors behavioral similarity over implementation
  similarity

## Acceptance Criteria

The complete RoboCode requirements set is acceptable only if it answers:

- what the finished product is
- which subsystems are in scope
- which phase each subsystem belongs to
- what "good enough" behavior means for each major subsystem
- how RoboCode should stay similar to `.ref` without becoming a literal port

