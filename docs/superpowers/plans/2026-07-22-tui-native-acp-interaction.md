# TUI Native and ACP Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship TUI `0.3.3` with a conventional native Lane flow and a `/acp` system command that selects, starts, resumes, focuses, and cancels Core-owned ACP sessions.

**Architecture:** Start from the immutable Core `0.3.4` SHA. Keep adapter/session selection as TUI-local overlay state, but derive all startability, ownership, progress, transcript, approval, result, and recovery facts from `RuntimeViewState`.

**Tech Stack:** Rust, crossterm, existing Viden canvas/widgets, `viden-core`, JSON i18n catalogs, deterministic TUI previews.

## Global Constraints

- Target version is exactly TUI `0.3.3`, based on the recorded Core `0.3.4` checkpoint SHA.
- `n` creates a Viden-native Lane; `/acp` delegates into the currently selected Lane.
- `/acp` is disabled with an explanatory row when no Lane is selected.
- ACP list shows Core-discovered adapters plus active/recent sessions; Enter chooses, arrows move, Esc closes.
- The composer remains editable during streaming; busy submission queues through Core.
- `Ctrl-C` cancels the exact focused native turn or ACP session.
- No TUI-private persistence, adapter readiness inference, process spawn, or Lane reducer.
- All new copy is present in `en.json` and `zh-CN.json`.

---

### Task 1: Register `/acp` and separate it from native Lane creation

**Files:**
- Modify: `apps/tui/src/tui/command_palette.rs`
- Modify: `apps/tui/src/tui/app.rs`
- Modify: `apps/tui/src/tui/state.rs`
- Test: inline tests in the same modules

**Interfaces:**
- Consumes: Core `QueryAgentAdapters` and existing Lane selection.
- Produces: `InteractionPanel::AcpPicker { selection: usize, phase: AcpPickerPhase }`.

- [ ] **Step 1: Write failing command-routing tests**

```rust
#[test]
fn acp_command_opens_picker_only_for_selected_lane() {
    let mut state = state_with_lane("lane-1");
    submit_command(&mut state, "/acp");
    assert!(matches!(state.ui.interaction_panel, Some(InteractionPanel::AcpPicker { .. })));
}

#[test]
fn native_new_lane_does_not_open_acp_picker() {
    let mut state = TuiState::default();
    press_key(&mut state, KeyCode::Char('n'));
    assert!(matches!(state.ui.interaction_panel, Some(InteractionPanel::NewLaneTask { .. })));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p viden-tui acp_command_opens_picker_only_for_selected_lane native_new_lane_does_not_open_acp_picker`

Expected: compilation fails because the new panel variants do not exist.

- [ ] **Step 3: Implement the minimal routing**

Add `/acp` to `COMMANDS`; route exact `/acp` to `QueryAgentAdapters` and the picker. Route `n` to a task-entry panel that later sends `PreviewDefaultStarterLane { Coder }`; do not reuse the ACP picker.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p viden-tui command_palette acp_command native_new_lane`

Expected: all selected tests pass.

- [ ] **Step 5: Commit routing separation**

```bash
git add apps/tui/src/tui/command_palette.rs apps/tui/src/tui/app.rs apps/tui/src/tui/state.rs
git commit -m "feat(tui): separate native Lane and ACP commands"
```

### Task 2: Render a keyboard-first ACP adapter and session picker

**Files:**
- Modify: `apps/tui/src/tui/modal.rs`
- Modify: `apps/tui/src/tui/input.rs`
- Modify: `apps/tui/src/tui/state.rs`
- Modify: `apps/tui/src/tui/i18n/en.json`
- Modify: `apps/tui/src/tui/i18n/zh-CN.json`
- Test: inline tests in `modal.rs` and `input.rs`

**Interfaces:**
- Consumes: `AgentAdapterView.startability`, `RuntimeViewState.agent_sessions`.
- Produces: stable row ids `adapter:<agent_id>` and `session:<session_id>`.

- [ ] **Step 1: Write failing row-order and keyboard tests**

```rust
#[test]
fn acp_picker_lists_sessions_before_adapters_with_truthful_status() {
    let state = state_with_acp_session_and_adapters();
    let rows = acp_picker_rows(&state);
    assert_eq!(rows[0].id, "session:acp-1");
    assert!(rows.iter().any(|row| row.label.contains("Authentication required")));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p viden-tui acp_picker_lists_sessions_before_adapters`

Expected: compilation fails because `acp_picker_rows` does not exist.

- [ ] **Step 3: Implement rows and overlay controls**

Render sections `ACTIVE / RECENT SESSIONS` and `AVAILABLE ACP AGENTS`. Enter on a session focuses it; Enter on `Ready` requests delegated task text; Enter on `ProbeRequired` sends `ProbeAgentAdapter`; install/auth/unavailable rows remain selectable only to show Core diagnostics. Esc unwinds task entry to picker, then closes.

- [ ] **Step 4: Run modal and input tests**

Run: `cargo test -p viden-tui acp_picker modal input`

Expected: selection, resize, CJK labels, Enter, and Esc tests pass.

- [ ] **Step 5: Commit the picker**

```bash
git add apps/tui/src/tui/modal.rs apps/tui/src/tui/input.rs apps/tui/src/tui/state.rs apps/tui/src/tui/i18n
git commit -m "feat(tui): add ACP session picker"
```

### Task 3: Complete native Lane creation and first task

**Files:**
- Modify: `apps/tui/src/tui/app.rs`
- Modify: `apps/tui/src/tui/modal.rs`
- Modify: `apps/tui/src/tui/projection.rs`
- Test: inline tests in `app.rs`

**Interfaces:**
- Consumes: `WorkspaceEligibility`, `PreviewDefaultStarterLane`, `CreateStarterLane`, `SubmitUserInput`.
- Produces: `PendingNativeLane { preview_id, content_sha256, task }` local correlation only.

- [ ] **Step 1: Write failing command-sequence tests**

```rust
#[test]
fn native_lane_task_waits_for_receipt_before_submitting() {
    let commands = drive_native_lane_creation("fix the parser");
    assert!(matches!(commands[0], RuntimeCommand::PreviewDefaultStarterLane { .. }));
    assert!(!commands.iter().any(|command| matches!(command, RuntimeCommand::SubmitUserInput { .. })));
    let commands = apply_starter_lane_receipt_and_continue();
    assert!(matches!(commands.last().unwrap(), RuntimeCommand::SubmitUserInput { content } if content == "fix the parser"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p viden-tui native_lane_task_waits_for_receipt_before_submitting`

Expected: current flow does not issue the new default preview sequence.

- [ ] **Step 3: Implement event-confirmed creation**

Show Core eligibility diagnostics before task entry. After `StarterLanePreviewed`, send `CreateStarterLane` with exact hashes; after `StarterLaneCreated`, focus the Lane and send its first native `SubmitUserInput`. A later provider failure stays inside the created Lane.

- [ ] **Step 4: Run native flow tests**

Run: `cargo test -p viden-tui native_lane starter_lane`

Expected: receipt ordering, failure independence, and zero-Lane cases pass.

- [ ] **Step 5: Commit native Lane flow**

```bash
git add apps/tui/src/tui/app.rs apps/tui/src/tui/modal.rs apps/tui/src/tui/projection.rs
git commit -m "feat(tui): complete native Lane first task"
```

### Task 4: Route ACP task, follow-up, focus, retry, and exact cancellation

**Files:**
- Modify: `apps/tui/src/tui/app.rs`
- Modify: `apps/tui/src/tui/composer.rs`
- Modify: `apps/tui/src/tui/state.rs`
- Test: inline tests in `app.rs`

**Interfaces:**
- Consumes: `StartAgentSession`, `SendAgentSessionInput`, `RetryAgentSession`, `CancelAgentSession`.
- Produces: `FocusedConversation::{NativeLane(AgentLaneId), AcpSession(SessionId)}` as presentation state.

- [ ] **Step 1: Write failing routing tests**

```rust
#[test]
fn focused_acp_composer_sends_follow_up_to_exact_session() {
    let mut state = state_focused_on_acp("lane-1", "acp-1");
    submit_text(&mut state, "continue");
    assert_eq!(take_command(), RuntimeCommand::SendAgentSessionInput {
        input: AgentSessionInput { session_id: SessionId("acp-1".into()), content: "continue".into() },
    });
}

#[test]
fn ctrl_c_targets_focused_acp_session() {
    let mut state = state_focused_on_acp("lane-1", "acp-1");
    press_ctrl_c(&mut state);
    assert_eq!(take_command(), RuntimeCommand::CancelAgentSession { session_id: "acp-1".into() });
}
```

- [ ] **Step 2: Verify failures**

Run: `cargo test -p viden-tui focused_acp_composer ctrl_c_targets_focused_acp_session`

Expected: current composer routes to the native turn.

- [ ] **Step 3: Implement focus-aware routing**

On adapter selection collect task text, then send `StartAgentSession` for the selected Lane. On session selection switch transcript lens without starting a process. Route follow-up/retry/cancel by exact focused conversation; keep the input editable while status is `Running` or `WaitingApproval`.

- [ ] **Step 4: Run conversation controls**

Run: `cargo test -p viden-tui agent_session composer cancel queue`

Expected: native and ACP routing, busy queue, retry, approval, and cancel tests pass.

- [ ] **Step 5: Commit ACP conversation controls**

```bash
git add apps/tui/src/tui/app.rs apps/tui/src/tui/composer.rs apps/tui/src/tui/state.rs
git commit -m "feat(tui): control focused ACP conversations"
```

### Task 5: Render complete statuses, evidence, and recovery

**Files:**
- Modify: `apps/tui/src/tui/render.rs`
- Modify: `apps/tui/src/tui/side_screen.rs`
- Modify: `apps/tui/src/tui/projection.rs`
- Modify: `apps/tui/src/tui/i18n/en.json`
- Modify: `apps/tui/src/tui/i18n/zh-CN.json`
- Test: inline render tests

**Interfaces:**
- Consumes: ordered native/ACP fixture from Core Task 5.
- Produces: visible starting/running/approval/completed/cancelled/failed/disconnected/replayed states.

- [ ] **Step 1: Add failing fixture render assertions**

Assert the deterministic canvas contains the focused agent name, session status, tool progress, approval action, result evidence, provider error hint, and recovered marker without exposing diagnostic secrets.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p viden-tui native_acp_fixture_render`

Expected: missing session/evidence/recovery labels fail the assertion.

- [ ] **Step 3: Implement bounded status rendering**

Use existing panels and glyphs. Keep actions in the approval panel, cap diagnostic rows, render cancelled separately from failed, and show replay recovery from Core cursor state.

- [ ] **Step 4: Run TUI tests and generate previews**

Run: `cargo test -p viden-tui && scripts/tui-previews.sh`

Expected: tests pass and previews show native and ACP states at supported terminal sizes.

- [ ] **Step 5: Commit rendering**

```bash
git add apps/tui/src/tui/render.rs apps/tui/src/tui/side_screen.rs apps/tui/src/tui/projection.rs apps/tui/src/tui/i18n
git commit -m "feat(tui): render native and ACP lifecycle"
```

### Task 6: Release TUI 0.3.3 evidence

**Files:**
- Modify: `apps/tui/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `docs/tui-version-functional-design.md`
- Modify: `docs/tui-version-functional-design.zh-CN.md`

- [ ] **Step 1: Set version and document `/acp`, `n`, focus, retry, and cancel**

- [ ] **Step 2: Run all TUI gates**

Run: `cargo test -p viden-tui && scripts/tui-turn-controller-smoke.sh && scripts/rc-tui-stability-smoke.sh && scripts/tui-regression.sh && cargo test --workspace --quiet && git diff --check`

Expected: every command exits 0.

- [ ] **Step 3: Commit the release candidate**

```bash
git add apps/tui/Cargo.toml Cargo.lock docs/tui-version-functional-design.md docs/tui-version-functional-design.zh-CN.md
git commit -m "chore(tui): release native and ACP interaction 0.3.3"
```
