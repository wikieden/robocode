# RoboCode Program Plan Index

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Break the complete RoboCode product requirements into execution-sized implementation plans that can be worked in order without re-opening product scope decisions.

**Architecture:** RoboCode is already at an early V1 baseline, so the next planning layer should preserve the existing core and expand by subsystem. This index acts as the dependency map for detailed implementation plans instead of treating the full product target as one giant execution plan.

**Tech Stack:** Rust workspace, existing RoboCode crates, Markdown planning docs

---

## Sequencing Rules

- Execute V2 local-developer enhancements before V3 platform work.
- Preserve the shared engine, permission, and transcript invariants in every plan.
- Avoid introducing side-channel runtimes for MCP, agents, or remote flows.
- Prefer new crates or focused modules only when they materially improve boundaries.

## Plan Queue

### Plan 1: V2-A Session and Command Enhancement

Purpose:
- broaden the local command surface
- make sessions easier to inspect and resume
- expose configuration and health information inside the CLI

Primary files:
- `robocode-cli/src/main.rs`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-config/src/lib.rs`

Output:
- detailed plan saved as `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`

### Plan 2: V2-B LSP Foundation

Purpose:
- introduce semantic code intelligence without breaking the existing tool loop

Expected files:
- new `robocode-lsp` crate
- `Cargo.toml`
- `robocode-core/src/lib.rs`
- `robocode-tools/src/lib.rs`
- `robocode-types/src/lib.rs`

### Plan 3: V2-C Memory and Task Workflows

Purpose:
- add long-lived memory and task state tied to sessions

Expected files:
- new `robocode-memory` or `robocode-workflows` crate
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`

### Plan 4: V2-D Rich TUI and Structured Views

Purpose:
- improve session browsing, diff viewing, and approval ergonomics

Expected files:
- `robocode-cli/src/main.rs`
- new presentation modules or crate for TUI concerns
- `robocode-core/src/lib.rs`

### Plan 5: V3-A MCP and Plugin Runtime

Purpose:
- add external tool ecosystems and extension loading

Expected files:
- new `robocode-mcp` crate
- new `robocode-plugins` crate
- `robocode-core/src/lib.rs`
- `robocode-tools/src/lib.rs`

### Plan 6: V3-B Multi-Agent and Coordinator

Purpose:
- add delegated work, teams, and transcript-safe coordination

Expected files:
- new coordinator/agent crates
- `robocode-core/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-session/src/lib.rs`

### Plan 7: V3-C Bridge, Remote, and Server Mode

Purpose:
- support IDE-connected and remote RoboCode sessions

Expected files:
- new bridge/remote/server crates
- `robocode-core/src/lib.rs`
- `robocode-permissions/src/lib.rs`
- `robocode-session/src/lib.rs`

## Execution Order

- [ ] Execute Plan 1 first.
- [ ] After Plan 1 lands, decide whether semantic assistance or workflow continuity is more urgent and choose Plan 2 or Plan 3 next.
- [ ] Delay Plan 5 through Plan 7 until the V2 command/session surface is stable enough to host integrations safely.

## Exit Condition

This index is complete when every major subsystem from:

- `docs/product-requirements.md`
- `docs/staged-roadmap.md`
- `docs/ref-gap-matrix.md`

has a corresponding detailed implementation plan with an explicit execution order.

