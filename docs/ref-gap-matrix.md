# RoboCode vs `.ref` Gap Matrix

This matrix compares the reference project in `.ref/claude-code-main` against
the current RoboCode repository and the intended target state.

| Subsystem | `.ref` capability summary | RoboCode current state | Target state | Gap | Phase | Notes |
|---|---|---|---|---|---|---|
| Core session engine | Shared query loop with tool-call continuation and transcript-driven runtime | Implemented with shared engine and unified tool loop | Match reference behavior for all major runtime paths | Low | V1 | High similarity target |
| Configuration | Startup orchestration plus complex environment/bootstrap logic | Deterministic config merge is implemented; advanced managed settings are absent | Stable local/global config plus room for managed settings | Medium | V1 / Long-term | Rust simplification is acceptable for bootstrap internals |
| Provider system | Anthropic-centric runtime with deep product integration | Multi-provider abstraction, native tool-calling, provider host/runtime registry, and DeepSeek as an independent provider family are implemented on main; dynamic loading and streaming/cancellation still need hardening | Mature multi-provider layer with stronger compatibility, dynamic provider loading, and streaming | Medium | V1 / V2 | Keep vendor-agnostic core and separate provider identity from protocol family |
| Tool runtime | Broad tool registry with shared permission-aware execution | Unified tool registry exists for local tools | Preserve shared runtime while expanding tool families | Medium | V1 / V2 / V3 | High similarity target |
| Permissions | First-class modes, rules, prompts, and edge-case handling | Core modes and rules exist; policy depth is still lighter | Mature rule system spanning local, remote, and integration flows | Medium | V1 / V2 / V3 | High similarity target |
| Session storage and resume | JSONL source of truth with resume and metadata | JSONL plus SQLite index implemented; browsing depth is basic | Richer summaries, selectors, and management | Medium | V1 / V2 | High similarity target |
| Slash commands | Large command surface across runtime, config, auth, tasks, integrations, UI | Core command families exist for runtime, sessions, git, and web | Broader command families covering config, diagnostics, integrations, and workflows | High | V1 / V2 / V3 | Do not copy every name verbatim |
| File and search tools | Read, write, edit, glob, grep | Implemented | Maintain and harden | Low | V1 | Already on target family-wise |
| Git workflows | Commit-oriented commands plus broader workflow helpers | Status, diff, switch, add, commit, push, restore, stash, worktree exist | Deeper review and workflow support | Medium | V1 / V2 | High similarity target for core flows |
| Web tools | Search and fetch built into tool system | Implemented | Improve quality and source handling | Low | V1 / V2 | Already on target family-wise |
| MCP | Server management and MCP-backed tool invocation | Not started | Full MCP lifecycle, discovery, invocation, and admin surface | High | V3 | High similarity target |
| LSP | Language server integration and recommendations | Implemented on main at a partial depth with real semantic queries, session reuse, document sync, and normalized output | Semantic code intelligence integrated with local workflows | Medium | V2 | Landed on main; still lighter than the reference platform |
| Skills | Reusable workflow system | Not started | Local skill discovery and execution model | High | V3 | Similar behavior, Rust-native implementation |
| Plugins | Built-in and third-party plugin loading | General plugin system not started; provider plugins are now part of the provider-system target | Plugin loading and management with clear trust boundaries | High | V2 / V3 | Distinguish provider plugins from the broader skills/plugin platform |
| Multi-agent / teams | Agent tool, coordinator, team workflows, inter-agent messaging | Not started | Coordinated delegated workflows under shared runtime guarantees | High | V3 | High similarity target |
| Bridge / remote | IDE bridge, remote session manager, server-oriented flows | Not started | Reusable remote and bridge layer with permission callbacks | High | V3 | High similarity target |
| Memory | Persistent memory support | Implemented on main at a partial depth with project and session memory, suggestion confirmation, and event logs | Explicit memory model tied to long-lived workflows | Medium | V2 | Landed on main; still lighter than the reference platform |
| Tasks | Task creation and workflow management | Implemented on main at a partial depth with lifecycle reducer, blockers, archive/restore, and resume context | Task lifecycle integrated into sessions and later agents | Medium | V2 | Landed on main; agent integration remains future work |
| Automation / cron | Scheduled and durable automation flows | Not started | Session and durable automation support | High | V3 | Keep behind core workflow maturity |
| Voice | Voice input and state management | Not started | Voice-assisted workflow layer | High | Long-term | Lower priority despite reference support |
| TUI / screens | Rich Ink UI, screens, structured diff, specialized views | Plain REPL plus structured terminal sections for LSP, sessions, tasks, memory, diff, and permission denials | Richer drilldowns and eventual lightweight TUI for integrations | Medium | V2 | Similar UX intent, not necessarily same framework |
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

The largest remaining gaps are platform-level subsystems and maturity gaps
rather than core local CLI behavior:

- MCP
- deeper LSP platform maturity
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
