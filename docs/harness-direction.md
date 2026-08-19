# Agent Harness Direction

## Purpose

This document records the accepted follow-up plan derived from the 2026-08-16
DeepSeek Harness v0.1 benchmark study. DeepSeek Harness independently validates
three Viden architecture decisions — append-only session facts as the source of
truth, a single Core contract serving multiple frontends, and permission checks
before mutation. The items below turn the remaining insights into Viden
direction. Each item carries an explicit status; only items labeled
**Implemented** describe verified behavior.

Chinese counterpart: [harness-direction.zh-CN.md](harness-direction.zh-CN.md).

## 1. Model-Visible Means Logged (Implemented: contract tests)

Invariant: every byte that reaches a provider request must be reconstructible
from the append-only JSONL transcript alone.

- Status: contract tests landed in
  `crates/runtime/src/tests/transcript_contract_tests.rs`. They run a live
  turn (tool call, denial path included), rebuild the session from the JSONL
  transcript, and require the model-visible projection (role, content,
  tool name, tool call id) of both histories to be identical.
- Open extension: the provider request also merges a context-bundle projection
  built at request time. Extending the invariant to cover the context
  projection (log or deterministically re-derive it) is a Core contract
  candidate, tracked as a follow-up below.

## 2. OS-Level Capability Seam In viden-tools (Implemented: first slice)

DeepSeek Harness swaps one filesystem/subprocess provider and Bash, PTY, and
LSP move to a remote sandbox together, with no tool changes. Viden's
`ToolExecutionContext` currently exposes `cwd` plus a semantic (LSP) provider,
and tools call `std::fs` and process spawn directly.

Status:

- `FilesystemCapability` and `ProcessCapability` traits with `LocalFilesystem`
  / `LocalProcess` defaults live in `crates/tools/src/capability.rs`;
  `ToolExecutionContext::local()` wires them in and the runtime call site uses
  it.
- File tools (`read_file`, `write_file`, `edit_file`) and `shell` consume the
  seam; capability tests in `crates/tools/src/tests/capability_tests.rs` prove
  an in-memory filesystem and a scripted process runner fully detach those
  tools from the OS.
- Remaining consumers (search, git, patch, web, process lanes, LSP runtime
  spawning) still call `std::fs`/`std::process` directly and migrate
  incrementally. Sandboxed or remote execution becomes a provider swap once
  migration completes.

## 3. Tool Execution Pre/Post Seam (Implemented)

Status: `ToolRegistry` in `crates/tools/src/lib.rs` now carries a
`ToolExecutionInterceptor` seam — `before_execute` hooks run in registration
order (the first `Reject` stops the call before the tool runs) and
`after_execute` hooks unwind in reverse; a registry without interceptors
behaves exactly as before. The runtime is the first consumer:
`crates/runtime/src/permission_gate.rs` writes the decide -> ask ->
apply_approval sequence once (`handle_tool_call`, context retrieval, the lane
approval re-check, workflow and agent write gates, and the ACP fs/terminal
bridges all resolve through it) and registers a `PermissionBackstopInterceptor`
that re-checks the pure `decide()` before any mutating tool executes, so
permission-before-mutation is structural rather than call-site discipline.
ACP client-requested file and terminal effects also moved onto the item 2
capability seam (`FilesystemCapability`/`ProcessCapability`, including a new
interactive-process spawn surface). Lane mutations keep their queued approval
flow with the gate-shaped re-check inside `resume_approval`; cost metering and
evidence capture remain future interceptor candidates.

## 4. Minimal Tool Preset For Reproducible Evaluation (Proposal)

A registry preset exposing only `shell` and the file editing tools, mirroring
the academic harness baseline DeepSeek ships as its Minimal mode. Purpose:
comparable SWE-bench / Terminal-Bench style runs and cheap A/B isolation of
context-engine effects. Low cost because `ToolRegistry` is registration-based.
Owner: Core; becomes a Core-owned configuration surface, not a frontend flag.

## 5. Code-Orchestration Mode (Proposal, long-term)

DeepSeek Harness Code mode generates a typed SDK so the model writes one
program instead of many round-trip tool calls. For Viden this is a future Core
contract candidate (a new tool family plus permission story), not near-term
work. Record only; do not start before the V3 contract freeze completes.

## 6. Interrupted-Turn Recovery (Implemented)

A crash or kill between persisting an assistant tool-call message and
persisting its result leaves a durable call that no tool result answers, and
several providers reject that request shape outright.

Status: `hydrate()` in `crates/runtime/src/session_lifecycle.rs` now closes
those calls while rebuilding the session. Each unanswered assistant tool-call
message receives a synthesized `Role::Tool` message with the matching
`tool_call_id`, inserted directly after the call it answers, and one
`Role::System` note reports how many calls were closed. Closures are
synthesized in memory only: loads stay read-only, the JSONL keeps exactly the
facts that were durable at crash time, and because the synthesis is a pure
function of the replayed entries the item 1 invariant still holds — every
later load of the same transcript rebuilds the identical history. Coverage
lives in `crates/runtime/src/tests/interrupted_turn_recovery_tests.rs`
(tail and mid-history dangling calls, multiple calls under one note,
determinism across loads, and no synthesis for answered calls).

## Strategic Stance

The "everything is a plugin" complexity tax (untyped cross-plugin injection,
load-order conflicts) is the documented community criticism of DeepSeek
Harness. Viden's differentiation is the same composability judgment with
compile-time-checked seams and a bounded plugin surface. Keep seams few, typed,
and owned by Core; do not chase dynamic plugin parity.

## Sequencing

1. Item 1 extension (context projection coverage) — next Core contract
   discussion.
2. Item 2 — schedule with the next `crates/tools` change set.
3. Item 3 — implemented; future interceptors (cost metering, evidence
   capture) attach to the existing seam as needed.
4. Items 4 and 5 — revisit after the `frontend-contract-v1` checkpoint ships.
5. Item 6 — implemented; extend it only if a future write path can persist a
   tool call without an assistant message.
