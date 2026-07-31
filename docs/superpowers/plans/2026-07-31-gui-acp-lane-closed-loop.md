# GUI ACP Lane Closed-Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make explicit Codex/Viden Lane selection, ACP execution state, and the completed ACP response form one truthful GUI loop.

**Architecture:** Extend the Core-owned `AgentSessionView` with optional output populated from the ACP protocol response, then project it through the existing Tauri D1 adapter. Keep selection and recovery precedence as GUI presentation state; never read agent artifacts directly from the web client.

**Tech Stack:** Rust, serde, Viden runtime contracts, Tauri, TypeScript, Vitest/jsdom.

## Global Constraints

- One Lane has exactly one Core-owned Agent session.
- All ACP output comes through `RuntimeCommand -> RuntimeEvent -> RuntimeViewState`.
- The contract change is additive and legacy serialized records remain readable.
- GUI never reads `.viden` agent logs or result files directly.
- English and Chinese specifications stay paired.

---

### Task 1: Explicit Agent selection

**Files:**
- Modify: `apps/gui/src/components/agent_menu.ts`
- Modify: `apps/gui/src/screens/d1_cockpit.ts`
- Modify: `apps/gui/src/i18n/catalog.ts`
- Test: `apps/gui/tests/agent_menu.spec.ts`
- Test: `apps/gui/tests/d1_cockpit.spec.ts`

**Interfaces:**
- Consumes: `AgentMenuModel.adapters[].startability`
- Produces: `AgentMenuSelection | undefined` until an enabled Agent is explicitly selected

- [ ] **Step 1: Write failing component tests**

Assert that no radio is checked initially, Create is disabled during probing,
and selecting a ready Codex option enables Create only after a non-empty task.

- [ ] **Step 2: Run the focused tests and confirm the expected failure**

Run: `npm test -- --run tests/agent_menu.spec.ts`

Expected: FAIL because Viden is implicitly selected and probing does not block Create.

- [ ] **Step 3: Implement explicit selection**

Make selection optional, include it in the submit guard, reset to no selection
when the popover closes, and render the selected Agent name in the create action.

- [ ] **Step 4: Run the focused tests**

Run: `npm test -- --run tests/agent_menu.spec.ts tests/d1_cockpit.spec.ts`

Expected: PASS.

### Task 2: Core-owned ACP response

**Files:**
- Modify: `crates/types/src/agent.rs`
- Modify: `crates/types/src/tests.rs`
- Modify: `crates/runtime/src/agent_commands.rs`
- Modify: `crates/runtime/src/tests/runtime_supervisor_tests.rs`
- Modify: `crates/core/tests/frontend_contract_v1.rs`

**Interfaces:**
- Produces: `AgentSessionView.output: Option<String>`
- Consumes: `AcpSessionPromptEvidence.message`

- [ ] **Step 1: Write failing contract/runtime tests**

Assert that legacy JSON without `output` decodes as `None` and a completed ACP
session event contains the exact protocol response.

- [ ] **Step 2: Run the focused tests and confirm the expected failure**

Run: `cargo test -p viden-types agent_session`

Run: `cargo test -p viden-runtime runtime_supervisor_owns_typed_acp_session_lifecycle_snapshot_and_replay`

Expected: FAIL because `AgentSessionView` has no output fact.

- [ ] **Step 3: Add the backward-compatible field**

Add `#[serde(default, skip_serializing_if = "Option::is_none")]` and populate
the completed session from `evidence.message`, leaving non-completed sessions
and empty responses as `None`.

- [ ] **Step 4: Run the Core tests**

Run: `cargo test -p viden-types`

Run: `cargo test -p viden-runtime runtime_supervisor_owns_typed_acp_session_lifecycle_snapshot_and_replay`

Expected: PASS.

### Task 3: D1 projection and recovery precedence

**Files:**
- Modify: `apps/gui/src-tauri/src/d1.rs`
- Modify: `apps/gui/src-tauri/src/projection.rs`
- Modify: `apps/gui/src/models/workspace.ts`
- Modify: `apps/gui/src/screens/d1_cockpit.ts`
- Modify: `apps/gui/src/i18n/catalog.ts`
- Test: `apps/gui/src-tauri/src/projection.rs`
- Test: `apps/gui/tests/d1_cockpit.spec.ts`

**Interfaces:**
- Consumes: `AgentSessionView.output`
- Produces: `D1AgentSessionProjection.output` and an ACP assistant row in D1

- [ ] **Step 1: Write failing projection and rendering tests**

Construct a completed selected ACP session with output and a stale
`agent_stopped` recovery state. Assert that the output is projected and visible
and the recovery surface is absent.

- [ ] **Step 2: Run focused tests and confirm the expected failure**

Run: `cargo test -p viden-gui --lib projection`

Run: `npm test -- --run tests/d1_cockpit.spec.ts`

Expected: FAIL because output is absent and recovery always wins.

- [ ] **Step 3: Implement projection and scoped recovery**

Map optional output, render it as a typed ACP assistant row, and suppress only
stale `agent_stopped` when the selected ACP session is active or completed.
Failed and cancelled selected sessions retain recovery.

- [ ] **Step 4: Run focused GUI tests**

Run: `npm test -- --run tests/agent_menu.spec.ts tests/d1_cockpit.spec.ts`

Run: `cargo test -p viden-gui`

Expected: PASS.

### Task 4: Gates and live acceptance

**Files:**
- Review: all changed files
- Produce: live app evidence only; no generated artifacts are tracked

**Interfaces:**
- Consumes: completed Tasks 1-3
- Produces: verified Codex ACP GUI loop

- [ ] **Step 1: Run change and dependency gates**

Run: `git diff --check`

Run: `scripts/check-dependency-boundaries.sh`

Run: `cargo test --workspace --quiet`

Expected: PASS.

- [ ] **Step 2: Build the macOS app**

Run: `npm run tauri build`

Expected: `target/release/bundle/macos/Viden.app` exists.

- [ ] **Step 3: Run a real Codex ACP Lane**

Launch with `VIDEN_GUI_WORKSPACE` set to this repository, explicitly select
Codex, create a task that asks for an exact harmless response, approve once,
and wait for completion.

- [ ] **Step 4: Verify visible truth**

Assert that the Lane rail names Codex, the session reaches `completed`, the
exact response is visible in the center surface, and `AGENT STOPPED` is absent.

