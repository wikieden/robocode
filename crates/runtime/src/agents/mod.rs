//! External agent adapters.
//!
//! This tree holds everything the runtime needs to drive an agent that runs in
//! another process: the ACP JSON-RPC client, the Codex CLI and app-server
//! client, and the spawn/probe infrastructure both share. It was previously one
//! `agent_commands.rs` file; the split follows the bands that file already had.
//!
//! Module ownership, in dependency order:
//!
//! - [`infra`]: process spawning, shell command planning, output capture, and
//!   the static adapter descriptor table. Knows nothing about a protocol.
//! - [`acp`]: the ACP wire protocol — framing, handshake, prompt turns, update
//!   and patch parsing, and the reverse-RPC filesystem/terminal callbacks the
//!   agent makes back into us.
//! - [`codex`]: the Codex band — CLI command handlers, job records and their
//!   lifecycle, diagnostics, and the app-server JSON-RPC client.
//! - [`glue`]: protocol-independent session glue — the resident ACP session
//!   cache, the typed-session entrypoints, job/task/adapter-view projections,
//!   and runtime-event persistence.
//! - [`render`]: presentation helpers for the `/agent` command surface.
//! - [`dispatch`]: the `SessionEngine` command surface. This is the piece that
//!   stays behind in `viden-runtime` if the bands above are ever extracted into
//!   their own crate, so it deliberately calls into the bands rather than
//!   reaching into their internals.
//!
//! Nothing in this tree calls back up into `crate::trust_loop`; merge-gate
//! decisions are built through `viden_types::MergeGateDecision::decided_now`.

mod acp;
mod codex;
mod dispatch;
mod glue;
mod infra;
mod render;

pub(crate) use glue::{
    AgentSessionApprover, cancel_typed_agent_session, mark_typed_agent_session_status,
    probe_typed_agent_adapter, resume_typed_agent_session, retry_typed_agent_session,
    shutdown_resident_acp_sessions, start_typed_agent_session, tracked_agent_job_runtime_events,
    tracked_agent_job_sessions, tracked_agent_job_tasks, typed_agent_adapter_views,
    typed_agent_session_request_from_compat_input, validate_typed_agent_session_request,
};

#[cfg(test)]
mod streaming_tests;
#[cfg(test)]
mod tests;
