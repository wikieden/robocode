# Core Native and ACP Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Core `0.3.4` as the sole authority for native Lane creation and exact-session ACP discovery, start, conversation, cancellation, persistence, and recovery.

**Architecture:** Extend the frozen frontend contract additively: Core publishes workspace eligibility and ACP startability, generates default Lane identities, and accepts ACP follow-up input against an exact session owner. The runtime supervisor performs every effect and journals ordered events; TUI and GUI only reduce `RuntimeViewState` and send commands.

**Tech Stack:** Rust, Serde tagged protocols, JSONL event journals, `viden-types`, `viden-runtime`, `viden-core`, Cargo tests.

## Global Constraints

- Target version is exactly Core `0.3.4`.
- Preserve `RuntimeCommand -> ordered RuntimeEvent -> RuntimeViewState` and `frontend-contract-v1` compatibility.
- Each Lane has one Viden-native primary agent; ACP sessions are delegated children and never create a second Lane authority.
- Lane creation succeeds independently from subsequent native-provider or ACP-session failure.
- Permission checks precede effects; cancel and follow-up commands must match the exact `RuntimeOwner.session_id` and `lane_id`.
- JSONL is canonical; SQLite remains rebuildable.
- Never serialize credentials, ACP stderr, or environment values.
- Update English and Chinese contract documentation together.

---

### Task 1: Add additive frontend contract types

**Files:**
- Modify: `crates/types/src/agent.rs`
- Modify: `crates/types/src/project.rs`
- Modify: `crates/types/src/runtime.rs`
- Modify: `crates/types/src/lib.rs`
- Test: `crates/types/src/tests.rs`
- Test: `crates/core/tests/frontend_contract_v1.rs`

**Interfaces:**
- Produces: `AgentStartability::{Ready, ProbeRequired, InstallRequired, AuthenticationRequired, Unavailable}`.
- Produces: `AgentSessionInput { session_id: SessionId, content: String }`.
- Produces: `WorkspaceEligibility { is_git_repository, has_head, can_create_lane, diagnostic }`.
- Produces: `RuntimeCommand::{PreviewDefaultStarterLane { preset }, SendAgentSessionInput { input }, RetryAgentSession { session_id }}`.
- Produces: `RuntimeEventKind::{WorkspaceEligibilityUpdated, AgentSessionInputAccepted}` and matching reduced view fields.

- [ ] **Step 1: Write failing serialization and reducer tests**

```rust
#[test]
fn additive_agent_session_input_round_trips_and_reduces() {
    let input = AgentSessionInput {
        session_id: SessionId("acp-7".into()),
        content: "continue with the failing test".into(),
    };
    let command = RuntimeCommand::SendAgentSessionInput { input: input.clone() };
    assert_eq!(serde_json::from_value(serde_json::to_value(&command).unwrap()).unwrap(), command);

    let event = RuntimeEvent::new(1, RuntimeEventKind::AgentSessionInputAccepted {
        session_id: input.session_id,
        input_id: "input-1".into(),
    });
    let mut view = RuntimeViewState::default();
    view.apply(&event);
    assert_eq!(view.agent_session_inputs.last().unwrap().input_id, "input-1");
}
```

- [ ] **Step 2: Verify the new contract test fails**

Run: `cargo test -p viden-types additive_agent_session_input_round_trips_and_reduces`

Expected: compilation fails because the new contract types and variants do not exist.

- [ ] **Step 3: Implement the minimal additive DTOs and reducers**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStartability {
    Ready,
    ProbeRequired,
    InstallRequired,
    AuthenticationRequired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionInput {
    pub session_id: SessionId,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEligibility {
    pub is_git_repository: bool,
    pub has_head: bool,
    pub can_create_lane: bool,
    pub diagnostic: Option<String>,
}
```

Add `startability: AgentStartability` to `AgentAdapterView`, `workspace_eligibility: Option<WorkspaceEligibility>` and `agent_session_inputs: Vec<AgentSessionInputView>` to `RuntimeViewState`, with Serde defaults.

- [ ] **Step 4: Run contract tests**

Run: `cargo test -p viden-types && cargo test -p viden-core --test frontend_contract_v1`

Expected: all tests pass and old schema-one fixtures still decode.

- [ ] **Step 5: Commit the additive contract**

```bash
git add crates/types crates/core/tests/frontend_contract_v1.rs
git commit -m "feat(core): extend native and ACP frontend contract"
```

### Task 2: Publish truthful workspace eligibility and Core-generated Lane defaults

**Files:**
- Modify: `crates/runtime/src/frontend_services.rs`
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/starter_lane.rs`
- Test: `crates/runtime/src/tests/frontend_services_tests.rs`
- Test: `crates/runtime/src/tests/runtime_contract_tests.rs`

**Interfaces:**
- Consumes: `WorkspaceEligibility` and `PreviewDefaultStarterLane` from Task 1.
- Produces: `workspace_eligibility(cwd: &Path) -> WorkspaceEligibility`.
- Produces: `default_starter_lane_request(cwd: &Path, preset: StarterLanePreset) -> Result<StarterLaneRequest, String>`.

- [ ] **Step 1: Write failing non-Git and valid-HEAD tests**

```rust
#[test]
fn default_lane_preview_rejects_non_git_before_preview() {
    let runtime = runtime_in_temp_dir_without_git();
    let events = runtime.handle(RuntimeCommand::PreviewDefaultStarterLane {
        preset: StarterLanePreset::Coder,
    }).unwrap();
    assert_rejected(&events, "workspace_not_git_repository");
    assert!(!events.iter().any(is_starter_lane_preview));
}

#[test]
fn default_lane_preview_generates_unique_core_owned_identity() {
    let runtime = runtime_in_git_repo_with_commit();
    let first = preview_default(&runtime);
    let second = preview_default(&runtime);
    assert_ne!(first.lane.id, second.lane.id);
    assert!(first.branch.starts_with("viden/lane-"));
}
```

- [ ] **Step 2: Verify failures**

Run: `cargo test -p viden-runtime default_lane_preview_ -- --nocapture`

Expected: compilation fails because the default preview command is not handled.

- [ ] **Step 3: Implement Git preflight and identity generation**

Use `git rev-parse --is-inside-work-tree` and `git rev-parse --verify HEAD` through the existing read-only command boundary. Generate `lane-<12 lowercase hex>` once in Core and derive `viden/<lane-id>` plus `.worktrees/<lane-id>`; return classified diagnostics without exposing raw stderr.

```rust
pub(crate) fn default_starter_lane_request(
    cwd: &Path,
    preset: StarterLanePreset,
) -> Result<StarterLaneRequest, String> {
    let eligibility = workspace_eligibility(cwd);
    if !eligibility.can_create_lane {
        return Err(eligibility.diagnostic.unwrap_or_else(|| "workspace_ineligible".into()));
    }
    let lane_id = next_lane_id();
    Ok(StarterLaneRequest { lane_id, preset, branch: None, worktree_path: None })
}
```

- [ ] **Step 4: Run focused runtime tests**

Run: `cargo test -p viden-runtime default_lane_preview_ workspace_eligibility_`

Expected: all selected tests pass; non-Git directories never emit a preview.

- [ ] **Step 5: Commit workspace preflight**

```bash
git add crates/runtime/src/frontend_services.rs crates/runtime/src/runtime_contract.rs crates/runtime/src/starter_lane.rs crates/runtime/src/tests
git commit -m "feat(core): publish Lane workspace eligibility"
```

### Task 3: Make ACP discovery and startability truthful

**Files:**
- Modify: `crates/runtime/src/agent_commands.rs`
- Modify: `crates/runtime/src/runtime_contract.rs`
- Test: `crates/runtime/src/tests/runtime_contract_tests.rs`
- Test: `crates/runtime/src/tests/fixtures/acp-v1/codex-acp.json`
- Test: `crates/runtime/src/tests/fixtures/acp-v1/claude-acp.json`
- Test: `crates/runtime/src/tests/fixtures/acp-v1/kiro-acp.json`

**Interfaces:**
- Consumes: `AgentStartability`.
- Produces: `classify_agent_startability(availability, auth_state, initialize_ok) -> AgentStartability`.
- Guarantees: a successful ACP initialize probe yields `Ready`; unknown auth yields `ProbeRequired`, never startable.

- [ ] **Step 1: Write failing probe classification tests**

```rust
#[test]
fn successful_initialize_probe_is_ready_to_start() {
    let adapter = probe_with_fixture("codex-acp.json").unwrap();
    assert_eq!(adapter.availability, AgentAvailability::Available);
    assert_eq!(adapter.auth_state, AgentAuthState::Ready);
    assert_eq!(adapter.startability, AgentStartability::Ready);
}

#[test]
fn unprobed_installed_adapter_requires_probe() {
    let adapter = installed_unprobed_adapter();
    assert_eq!(adapter.startability, AgentStartability::ProbeRequired);
}
```

- [ ] **Step 2: Verify the readiness test fails for the current `Unknown` state**

Run: `cargo test -p viden-runtime successful_initialize_probe_is_ready_to_start`

Expected: assertion fails because current code emits `AgentAuthState::Unknown`.

- [ ] **Step 3: Implement classified startability**

```rust
fn classify_agent_startability(
    availability: AgentAvailability,
    auth_state: AgentAuthState,
) -> AgentStartability {
    match (availability, auth_state) {
        (AgentAvailability::Available, AgentAuthState::Ready) => AgentStartability::Ready,
        (AgentAvailability::Available, AgentAuthState::Unknown) => AgentStartability::ProbeRequired,
        (AgentAvailability::NeedsInstall, _) => AgentStartability::InstallRequired,
        (AgentAvailability::NeedsAuth, _) | (_, AgentAuthState::LoggedOut) => AgentStartability::AuthenticationRequired,
        _ => AgentStartability::Unavailable,
    }
}
```

Set `Ready` only after initialize completes and its advertised capabilities are parsed. Keep raw process output in local logs only.

- [ ] **Step 4: Run ACP probe tests**

Run: `cargo test -p viden-runtime agent_adapter probe_typed_agent_adapter`

Expected: discovery, install, authentication, and ready fixture cases pass.

- [ ] **Step 5: Commit honest readiness**

```bash
git add crates/runtime/src/agent_commands.rs crates/runtime/src/runtime_contract.rs crates/runtime/src/tests
git commit -m "fix(core): publish truthful ACP startability"
```

### Task 4: Add exact-session ACP follow-up, retry, and cancellation

**Files:**
- Modify: `crates/runtime/src/agent_commands.rs`
- Modify: `crates/runtime/src/runtime_supervisor.rs`
- Modify: `crates/runtime/src/event_journal.rs`
- Test: `crates/runtime/src/tests/runtime_supervisor_tests.rs`

**Interfaces:**
- Consumes: `SendAgentSessionInput`, `RetryAgentSession`, `AgentSessionInputAccepted`.
- Produces: `resume_typed_agent_session(cwd, session, content, sink, approver) -> Result<String, String>` where the return value is the durable input id.
- Guarantees: follow-up and cancel reject a mismatched session/lane owner and reuse the stored ACP resume handle.

- [ ] **Step 1: Write failing owner and resume tests**

```rust
#[test]
fn follow_up_resumes_exact_acp_session_and_preserves_owner() {
    let mut supervisor = fixture_supervisor();
    let session = start_fixture_acp(&mut supervisor, "lane-1", "acp-1");
    let events = supervisor.send(RuntimeCommand::SendAgentSessionInput {
        input: AgentSessionInput { session_id: session.session_id.clone(), content: "continue".into() },
    }).unwrap();
    assert!(events.iter().any(|event| matches!(event.kind,
        RuntimeEventKind::AgentSessionInputAccepted { ref session_id, .. } if session_id == &session.session_id)));
}

#[test]
fn cancel_rejects_session_not_owned_by_focused_lane() {
    let error = cancel_with_owner("lane-2", "acp-1").unwrap_err();
    assert!(error.contains("agent_session_owner_mismatch"));
}
```

- [ ] **Step 2: Verify failures**

Run: `cargo test -p viden-runtime follow_up_resumes_exact_acp_session cancel_rejects_session_not_owned`

Expected: follow-up command is unhandled and owner validation test fails.

- [ ] **Step 3: Implement resume using durable session metadata**

Load the existing session record, verify `RuntimeOwner`, append an input record before spawning ACP, pass `load_session_id` to `AcpSessionOptions`, emit `AgentSessionInputAccepted`, then stream normal tool/approval/session events. `RetryAgentSession` creates a new attempt id under the same logical session and never mutates a terminal attempt in place.

```rust
fn validate_session_owner(expected: &RuntimeOwner, actual: &RuntimeOwner) -> Result<(), String> {
    if expected.lane_id != actual.lane_id || expected.session_id != actual.session_id {
        return Err("agent_session_owner_mismatch".into());
    }
    Ok(())
}
```

- [ ] **Step 4: Run supervisor and journal tests**

Run: `cargo test -p viden-runtime runtime_supervisor agent_session event_journal`

Expected: start, follow-up, retry, exact cancel, and approval event tests pass.

- [ ] **Step 5: Commit conversational ACP control**

```bash
git add crates/runtime/src/agent_commands.rs crates/runtime/src/runtime_supervisor.rs crates/runtime/src/event_journal.rs crates/runtime/src/tests/runtime_supervisor_tests.rs
git commit -m "feat(core): resume exact ACP sessions"
```

### Task 5: Recover native and ACP interaction facts after restart

**Files:**
- Modify: `crates/runtime/src/runtime_contract.rs`
- Modify: `crates/runtime/src/agent_commands.rs`
- Modify: `crates/core/tests/frontend_contract_v1.rs`
- Create: `crates/core/tests/fixtures/native-acp-interaction-v1.jsonl`
- Test: `crates/runtime/src/tests/runtime_contract_tests.rs`

**Interfaces:**
- Consumes: all Task 1-4 events.
- Produces: one deterministic fixture containing eligible workspace, Lane receipt, native stream/tool/cost/completion, ACP probe/start/follow-up/approval/result/cancel, and replay cursor.
- Guarantees: replay and snapshot yield identical business facts.

- [ ] **Step 1: Write a failing restart parity test**

```rust
#[test]
fn native_acp_fixture_snapshot_matches_ordered_replay() {
    let fixture = include_str!("fixtures/native-acp-interaction-v1.jsonl");
    let replayed = reduce_fixture(fixture);
    let restored = snapshot_then_restore(fixture);
    assert_eq!(business_projection(&restored), business_projection(&replayed));
    assert_eq!(restored.agent_sessions.len(), 2);
    assert!(restored.latest_evidence.iter().any(|item| item.label.contains("ACP")));
}
```

- [ ] **Step 2: Verify the fixture test fails**

Run: `cargo test -p viden-core --test frontend_contract_v1 native_acp_fixture_snapshot_matches_ordered_replay`

Expected: failure because the fixture and restoration assertions do not exist.

- [ ] **Step 3: Implement restoration and add the canonical fixture**

Restore terminal `Completed`, `Failed`, and `Cancelled` events without converting them to `Started`. Restore accepted ACP inputs, retry attempts, result evidence, and the exact cursor. Redact volatile timestamps before parity comparison.

- [ ] **Step 4: Run parity and focused Core gates**

Run: `cargo test -p viden-types && cargo test -p viden-runtime && cargo test -p viden-core`

Expected: all tests pass, including ordered replay after a deliberate sequence-gap recovery.

- [ ] **Step 5: Commit recovery parity**

```bash
git add crates/runtime crates/core/tests
git commit -m "test(core): certify native and ACP recovery parity"
```

### Task 6: Publish Core 0.3.4 checkpoint and bilingual contract docs

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `docs/frontend-integration-contract.md`
- Modify: `docs/frontend-integration-contract.zh-CN.md`
- Modify: `docs/parallel-development-plan.md`
- Modify: `docs/parallel-development-plan.zh-CN.md`

**Interfaces:**
- Produces: immutable Core `0.3.4` checkpoint consumed by both frontend branches.

- [ ] **Step 1: Update version and document exact commands/events/state fields**

Document `PreviewDefaultStarterLane`, `SendAgentSessionInput`, `RetryAgentSession`, `WorkspaceEligibilityUpdated`, `AgentSessionInputAccepted`, `AgentStartability`, retry semantics, exact-owner cancel, and snapshot/replay behavior in both languages.

- [ ] **Step 2: Run formatting and boundary gates**

Run: `cargo fmt --check && scripts/check-dependency-boundaries.sh && git diff --check`

Expected: all commands exit 0.

- [ ] **Step 3: Run the Core release candidate suite**

Run: `cargo test -p viden-types && cargo test -p viden-session && cargo test -p viden-workflows && cargo test -p viden-runtime && cargo test -p viden-core && cargo test --workspace --quiet`

Expected: all tests pass.

- [ ] **Step 4: Commit and record the immutable SHA**

```bash
git add crates/core/Cargo.toml Cargo.lock docs/frontend-integration-contract.md docs/frontend-integration-contract.zh-CN.md docs/parallel-development-plan.md docs/parallel-development-plan.zh-CN.md
git commit -m "chore(core): release frontend contract 0.3.4"
git rev-parse HEAD
```

Expected: the printed SHA becomes the only allowed base for TUI `0.3.3` and GUI `0.1.0-rc.2` worktrees.
