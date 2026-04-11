# RoboCode vs `.ref` Gap Matrix

This matrix compares the reference project in `.ref/claude-code-main` against
the current RoboCode repository and the intended target state.

| Subsystem | `.ref` capability summary | RoboCode current state | Target state | Gap | Phase | Notes |
|---|---|---|---|---|---|---|
| Core session engine | Shared query loop with tool-call continuation and transcript-driven runtime | Implemented with shared engine and unified tool loop | Match reference behavior for all major runtime paths | Low | V1 | High similarity target |
| Configuration | Startup orchestration plus complex environment/bootstrap logic | Deterministic config merge is implemented; advanced managed settings are absent | Stable local/global config plus room for managed settings | Medium | V1 / Long-term | Rust simplification is acceptable for bootstrap internals |
| Provider system | Anthropic-centric runtime with deep product integration | Multi-provider abstraction exists with native tool-calling support | Mature multi-provider layer with stronger compatibility and streaming | Medium | V1 / V2 | Keep vendor-agnostic core |
| Tool runtime | Broad tool registry with shared permission-aware execution | Unified tool registry exists for local tools | Preserve shared runtime while expanding tool families | Medium | V1 / V2 / V3 | High similarity target |
| Permissions | First-class modes, rules, prompts, and edge-case handling | Core modes and rules exist; policy depth is still lighter | Mature rule system spanning local, remote, and integration flows | Medium | V1 / V2 / V3 | High similarity target |
| Session storage and resume | JSONL source of truth with resume and metadata | JSONL plus SQLite index implemented; browsing depth is basic | Richer summaries, selectors, and management | Medium | V1 / V2 | High similarity target |
| Slash commands | Large command surface across runtime, config, auth, tasks, integrations, UI | Core command families exist for runtime, sessions, git, and web | Broader command families covering config, diagnostics, integrations, and workflows | High | V1 / V2 / V3 | Do not copy every name verbatim |
| File and search tools | Read, write, edit, glob, grep | Implemented | Maintain and harden | Low | V1 | Already on target family-wise |
| Git workflows | Commit-oriented commands plus broader workflow helpers | Status, diff, switch, add, commit, push, restore, stash, worktree exist | Deeper review and workflow support | Medium | V1 / V2 | High similarity target for core flows |
| Web tools | Search and fetch built into tool system | Implemented | Improve quality and source handling | Low | V1 / V2 | Already on target family-wise |
| MCP | Server management and MCP-backed tool invocation | Not started | Full MCP lifecycle, discovery, invocation, and admin surface | High | V3 | High similarity target |
| LSP | Language server integration and recommendations | Not started | Semantic code intelligence integrated with local workflows | High | V2 | High similarity target |
| Skills | Reusable workflow system | Not started | Local skill discovery and execution model | High | V3 | Similar behavior, Rust-native implementation |
| Plugins | Built-in and third-party plugin loading | Not started | Plugin loading and management with clear trust boundaries | High | V3 | Similar behavior, Rust-native implementation |
| Multi-agent / teams | Agent tool, coordinator, team workflows, inter-agent messaging | Not started | Coordinated delegated workflows under shared runtime guarantees | High | V3 | High similarity target |
| Bridge / remote | IDE bridge, remote session manager, server-oriented flows | Not started | Reusable remote and bridge layer with permission callbacks | High | V3 | High similarity target |
| Memory | Persistent memory support | Not started | Explicit memory model tied to long-lived workflows | High | V2 | Similar behavior, simpler first implementation |
| Tasks | Task creation and workflow management | Not started | Task lifecycle integrated into sessions and later agents | High | V2 | High-value feature before broader platform work |
| Automation / cron | Scheduled and durable automation flows | Not started | Session and durable automation support | High | V3 | Keep behind core workflow maturity |
| Voice | Voice input and state management | Not started | Voice-assisted workflow layer | High | Long-term | Lower priority despite reference support |
| TUI / screens | Rich Ink UI, screens, structured diff, specialized views | Minimal REPL and textual help | Richer TUI for diff, sessions, permissions, and integrations | High | V2 | Similar UX intent, not necessarily same framework |
| Analytics / feature flags / managed settings | Product operations, flags, policy, telemetry, managed config | Not started by design | Selective adoption only after core product maturity | High | Long-term | Do not prioritize early unless product needs demand it |

## Summary

RoboCode already covers the reference project's most important architectural
spine:

- shared session engine
- shared tool runtime
- permissions
- transcripts and resume
- provider abstraction
- high-value local developer tools

The largest remaining gaps are platform-level subsystems rather than core local
CLI behavior:

- MCP
- LSP
- skills and plugins
- multi-agent coordination
- bridge and remote operation
- memory, tasks, and automation
- richer terminal UI

The deliberate de-prioritizations are the reference project's product-scale
operational systems:

- analytics
- feature flags
- managed settings
- other product-growth infrastructure that does not improve the core developer
  workflow early
