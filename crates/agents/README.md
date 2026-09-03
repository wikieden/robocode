# viden-agents

## Purpose

`viden-agents` owns the external agent adapters: every way Viden drives an
agent that runs in another process. It holds one strategy per external CLI —
the generic ACP JSON-RPC client, the Codex CLI and app-server client — plus the
spawn, probe, and output-capture infrastructure they share, and the session
glue that projects tracked agent jobs into typed records.

It sits below `viden-runtime` and knows nothing about sessions, lanes,
providers, frontends, or the runtime's trust loop.

## Does Not Own

- Session, lane, provider, or frontend state.
- The permission decision sequence itself; it calls the shared gate in
  `viden-permissions` and the caller supplies the engine and the approver.
- Where a runtime event is recorded; the runtime supplies a `RuntimeEventSink`.
- Transcript recording and the `/agent` command surface, which stay in
  `viden-runtime` (`agent_dispatch.rs`).
- Direct operating-system access. Every process, file, and terminal effect goes
  through `viden-tools` capabilities.

## Public Surface

- Typed agent sessions: `start_typed_agent_session`,
  `resume_typed_agent_session`, `retry_typed_agent_session`,
  `cancel_typed_agent_session`, `mark_typed_agent_session_status`,
  `validate_typed_agent_session_request`, and
  `typed_agent_session_request_from_compat_input`.
- Adapter discovery: `typed_agent_adapter_views` and
  `probe_typed_agent_adapter`.
- Tracked job projections: `tracked_agent_job_tasks`,
  `tracked_agent_job_sessions`, and `tracked_agent_job_runtime_events`.
- Injected runtime policy: `RuntimeEventSink` and `AgentSessionApprover`.
- The ACP band's command entry points: `handle_agent_probe_command`,
  `handle_agent_auth_command`, `handle_acp_agent_run_command`,
  `run_acp_smoke_gate`, `parse_acp_run_args`, and `AcpRunArgs`.
- The Codex band's command entry points: `handle_codex_review_command`,
  `handle_codex_challenge_command`, `start_codex_job`,
  `start_codex_app_server_job`, `render_codex_job_status`,
  `render_codex_job_result`, `cancel_codex_job`, `ensure_codex_target`,
  `parse_codex_run_args`, `ParsedCodexRunArgs`, `CodexJobKind`,
  `codex_command`, and `codex_run_command_args`.
- `/agent` presentation helpers: `render_agent_list`, `render_agent_doctor`,
  and `render_agent_logs_help`.
- `shutdown_resident_acp_sessions`, so no agent process outlives its project.

## Invariants

- Permission checks precede effects. Every reverse-RPC filesystem and terminal
  mutation an agent requests resolves through
  `viden_permissions::resolve_permission` — the shared decide -> ask ->
  apply_approval gate — before the `viden-tools` capability is called.
- Runtime-owned policy is injected, never imported. Permission contexts,
  approvers, and event sinks arrive as parameters, which keeps the dependency
  edge one-directional and is enforced by
  `scripts/check-dependency-boundaries.sh`.
- Frontends reach these adapters only through Core. `viden-agents` must not
  appear in a TUI or GUI manifest.
- A resident ACP session is cached per project and torn down with it.

## Test

```bash
cargo test -p viden-agents
```

Agent behavior is additionally covered end to end by the runtime suite, which
drives adapters through `RuntimeSupervisor` and the `/agent` command surface:

```bash
cargo test -p viden-runtime
```
