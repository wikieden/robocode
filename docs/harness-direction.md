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

## 2. OS-Level Capability Seam In viden-tools (Accepted, planned)

DeepSeek Harness swaps one filesystem/subprocess provider and Bash, PTY, and
LSP move to a remote sandbox together, with no tool changes. Viden's
`ToolExecutionContext` currently exposes `cwd` plus a semantic (LSP) provider,
and tools call `std::fs` and process spawn directly.

Planned shape:

- define filesystem and process capability traits next to
  `SemanticToolProvider` in `crates/tools`;
- default implementations preserve current local behavior exactly;
- migrate tools to consume the seam incrementally, starting with file and
  shell tools;
- sandboxed or remote execution later becomes a provider swap, not a tool
  rewrite.

Do this while the tool surface is small; retrofitting after the registry grows
is the expensive path.

## 3. Tool Execution Pre/Post Seam (Accepted, planned)

`ToolRegistry::execute` dispatches directly today; permission checks happen at
the call site before execution. A registry-level pre-execute/post-execute seam
lets permissions, cost metering, and evidence capture attach uniformly instead
of relying on every call site. Permission-before-mutation stays a structural
guarantee rather than a discipline. Scope: additive, default behavior
unchanged; the runtime call site in `handle_tool_call` becomes the first
consumer.

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
3. Item 3 — schedule with the next tools/permissions change set.
4. Items 4 and 5 — revisit after the `frontend-contract-v1` checkpoint ships.
