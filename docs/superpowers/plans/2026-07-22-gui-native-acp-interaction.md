# GUI Native and ACP Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship GUI `0.1.0-rc.2` with a Zed-like compact `+` menu for native Lane creation and ACP delegation, plus complete Core-owned conversation controls inside D1.

**Architecture:** Start from the same immutable Core `0.3.4` SHA as TUI. D1 remains the application shell; a small menu and task prompt replace D4 as the normal entry, while the Tauri adapter translates only typed Core commands and projections. Existing D4 remains a compatibility route and is not the primary workflow.

**Tech Stack:** TypeScript, DOM, CSS tokens, Tauri 2, Rust `viden-core` adapter, Vitest, Cargo tests.

## Global Constraints

- Target version is exactly GUI `0.1.0-rc.2`, based on the immutable Core `0.3.4` checkpoint.
- The compact menu groups `Viden Agent` under `NEW LANE` and Core adapters under `DELEGATE TO CURRENT LANE`.
- `Open project` only selects a folder; it never opens D11 or asks for a model or Lane.
- ACP choices are disabled until a Lane is selected and Core reports `AgentStartability::Ready`.
- Lane receipt is independent from native/ACP start success.
- All mutations pass through the Tauri `viden-core` client and wait for typed events.
- No white outer application frame, mock runtime, GUI-private recent list, readiness inference, or process spawning.
- Support keyboard navigation, visible focus, CJK IME, screen-reader labels, locale, theme, density, font scale, and reduced motion.

---

### Task 1: Extend the Tauri adapter with additive Core intents

**Files:**
- Modify: `apps/gui/src-tauri/src/d1.rs`
- Modify: `apps/gui/src-tauri/src/adapter.rs`
- Modify: `apps/gui/src-tauri/src/projection.rs`
- Test: `apps/gui/tests/d1_cockpit.rs`

**Interfaces:**
- Consumes: Core `WorkspaceEligibility`, `AgentStartability`, default Lane preview, ACP input/retry commands.
- Produces: D1 intents `preview_default_lane`, `send_agent_session_input`, `retry_agent_session`, and projections `workspaceEligibility`, `startability`.

- [ ] **Step 1: Write failing Rust adapter tests**

```rust
#[test]
fn d1_default_lane_intent_sends_core_generated_preview_command() {
    let command = command_for(D1Intent::PreviewDefaultLane { preset: "coder".into() });
    assert_eq!(command, RuntimeCommand::PreviewDefaultStarterLane {
        preset: StarterLanePreset::Coder,
    });
}

#[test]
fn d1_acp_follow_up_preserves_exact_session_id() {
    let command = command_for(D1Intent::SendAgentSessionInput {
        session_id: "acp-1".into(), content: "continue".into(),
    });
    assert!(matches!(command, RuntimeCommand::SendAgentSessionInput { input }
        if input.session_id.as_str() == "acp-1"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p viden-gui d1_default_lane_intent_sends_core_generated_preview_command d1_acp_follow_up_preserves_exact_session_id`

Expected: compilation fails because the D1 intents do not exist.

- [ ] **Step 3: Implement typed translations and projection fields**

Add Serde-tagged D1 intents and map them directly to Core commands. Project Core fields without recomputing `canStart`; expose the serialized `startability` string and classified diagnostic.

- [ ] **Step 4: Run GUI Rust adapter tests**

Run: `cargo test -p viden-gui --test d1_cockpit`

Expected: command identity, pending correlation, rejection, sequence gap, and snapshot replay tests pass.

- [ ] **Step 5: Commit adapter support**

```bash
git add apps/gui/src-tauri/src/d1.rs apps/gui/src-tauri/src/adapter.rs apps/gui/src-tauri/src/projection.rs apps/gui/tests/d1_cockpit.rs
git commit -m "feat(gui): expose native and ACP D1 intents"
```

### Task 2: Add the compact Zed-like agent menu

**Files:**
- Create: `apps/gui/src/components/agent_menu.ts`
- Create: `apps/gui/src/components/agent_menu.css`
- Modify: `apps/gui/src/screens/d1_cockpit.ts`
- Modify: `apps/gui/src/screens/d1_cockpit.css`
- Test: `apps/gui/tests/agent_menu.spec.ts`

**Interfaces:**
- Consumes: selected Lane id and Core adapter projections.
- Produces: `AgentMenuSelection = { kind: "native" } | { kind: "acp"; agentId: string }`.
- Produces: `renderAgentMenu(anchor, model, onSelect, onClose) -> AgentMenuController`.

- [ ] **Step 1: Write failing DOM and keyboard tests**

```ts
it("groups native creation separately from ACP delegation", () => {
  const menu = renderMenu({ selectedLaneId: "lane-1", adapters: readyAdapters });
  expect(menu.textContent).toContain("NEW LANE");
  expect(menu.textContent).toContain("Viden Agent");
  expect(menu.textContent).toContain("DELEGATE TO CURRENT LANE");
  expect(menu.textContent).toContain("Codex");
});

it("disables delegation without a selected Lane", () => {
  const menu = renderMenu({ selectedLaneId: null, adapters: readyAdapters });
  expect(menu.querySelector('[data-agent-id="codex"]')?.getAttribute("aria-disabled")).toBe("true");
});
```

- [ ] **Step 2: Verify failure**

Run: `npm --prefix apps/gui test -- agent_menu.spec.ts`

Expected: module-not-found failure for `agent_menu`.

- [ ] **Step 3: Implement the menu with shared tokens**

Use a button-triggered anchored menu with `role="menu"`, section labels, roving tabindex, Up/Down/Home/End/Enter/Escape, outside-click close, and focus restoration. Render Core status text beside unavailable adapters; do not add a model selector.

- [ ] **Step 4: Run DOM tests**

Run: `npm --prefix apps/gui test -- agent_menu.spec.ts`

Expected: grouping, disabled state, keyboard, focus restoration, and locale tests pass.

- [ ] **Step 5: Commit the menu**

```bash
git add apps/gui/src/components/agent_menu.ts apps/gui/src/components/agent_menu.css apps/gui/src/screens/d1_cockpit.ts apps/gui/src/screens/d1_cockpit.css apps/gui/tests/agent_menu.spec.ts
git commit -m "feat(gui): add compact agent menu"
```

### Task 3: Complete native Lane creation inside D1

**Files:**
- Create: `apps/gui/src/components/lane_task_prompt.ts`
- Create: `apps/gui/src/components/lane_task_prompt.css`
- Modify: `apps/gui/src/screens/d1_cockpit.ts`
- Modify: `apps/gui/src/main.ts`
- Test: `apps/gui/tests/d1_cockpit.spec.ts`

**Interfaces:**
- Consumes: Task 1 D1 intents and Task 2 `{ kind: "native" }` selection.
- Produces: task prompt result `{ task: string }`; no Lane id, branch, worktree, provider, or model field.

- [ ] **Step 1: Write failing event-order test**

```ts
it("waits for the Core Lane receipt before submitting the native task", async () => {
  await chooseVidenAgentAndSubmit("fix the parser");
  expect(sentIntents[0]).toEqual({ type: "preview_default_lane", preset: "coder" });
  expect(sentIntents).not.toContainEqual({ type: "submit_user_input", content: "fix the parser" });
  await emitStarterLaneCreated("lane-7");
  expect(sentIntents.at(-1)).toEqual({ type: "submit_user_input", content: "fix the parser" });
});
```

- [ ] **Step 2: Verify failure**

Run: `npm --prefix apps/gui test -- d1_cockpit.spec.ts -t "waits for the Core Lane receipt"`

Expected: the D1 quick-create sequence is absent.

- [ ] **Step 3: Implement the two-confirmation flow**

Show workspace eligibility before enabling submit. Send default preview, then exact preview create, then focus the receipt Lane and submit the task. If provider start fails, retain the Lane and show typed recovery in D1. Remove normal D1 navigation into D4; keep D4 callable only by its compatibility route.

- [ ] **Step 4: Run D1 flow tests**

Run: `npm --prefix apps/gui test -- d1_cockpit.spec.ts`

Expected: zero-Lane, valid project, non-Git rejection, receipt ordering, and provider failure independence pass.

- [ ] **Step 5: Commit quick native creation**

```bash
git add apps/gui/src/components/lane_task_prompt.ts apps/gui/src/components/lane_task_prompt.css apps/gui/src/screens/d1_cockpit.ts apps/gui/src/main.ts apps/gui/tests/d1_cockpit.spec.ts
git commit -m "feat(gui): create native Lanes from D1"
```

### Task 4: Delegate and converse with ACP sessions inside a Lane

**Files:**
- Create: `apps/gui/src/components/agent_session_switcher.ts`
- Modify: `apps/gui/src/screens/d1_cockpit.ts`
- Modify: `apps/gui/src/screens/d1_cockpit.css`
- Test: `apps/gui/tests/d1_cockpit.spec.ts`

**Interfaces:**
- Consumes: ACP selection, D1 session intents, Core `agentSessions` projection.
- Produces: presentation-only `focusedConversation = { kind: "native"; laneId } | { kind: "acp"; laneId; sessionId }`.

- [ ] **Step 1: Write failing delegation and follow-up tests**

```ts
it("starts ACP as a child of the selected Lane", async () => {
  await chooseAgent("codex", "review the diff");
  expect(sentIntents.at(-1)).toEqual({
    type: "start_agent_session", laneId: "lane-1", agentId: "codex", model: null, task: "review the diff",
  });
});

it("routes composer input to the focused ACP session", async () => {
  focusAcpSession("acp-1");
  await submitComposer("continue");
  expect(sentIntents.at(-1)).toEqual({ type: "send_agent_session_input", sessionId: "acp-1", content: "continue" });
});
```

- [ ] **Step 2: Verify failure**

Run: `npm --prefix apps/gui test -- d1_cockpit.spec.ts -t "ACP"`

Expected: the current D1 composer lacks ACP focus routing.

- [ ] **Step 3: Implement task prompt, session switcher, and controls**

After a ready adapter is selected, reuse the task prompt and send `start_agent_session`. Show active/recent child sessions under the selected Lane. Session selection focuses existing transcript; composer sends follow-up, retry sends `retry_agent_session`, and cancel targets the exact session. Never create a Lane during ACP delegation.

- [ ] **Step 4: Run ACP GUI tests**

Run: `npm --prefix apps/gui test -- d1_cockpit.spec.ts agent_menu.spec.ts`

Expected: start, switch, follow-up, approval, retry, exact cancel, terminal result, and restart restore tests pass.

- [ ] **Step 5: Commit ACP conversation UX**

```bash
git add apps/gui/src/components/agent_session_switcher.ts apps/gui/src/screens/d1_cockpit.ts apps/gui/src/screens/d1_cockpit.css apps/gui/tests/d1_cockpit.spec.ts
git commit -m "feat(gui): complete ACP conversations in D1"
```

### Task 5: Align visual, localization, accessibility, and recovery states

**Files:**
- Modify: `apps/gui/src/i18n/en.json`
- Modify: `apps/gui/src/i18n/zh-CN.json`
- Modify: `apps/gui/src/ui/window_chrome.css`
- Modify: `apps/gui/src/screens/d1_cockpit.css`
- Test: `apps/gui/tests/d1_cockpit.spec.ts`
- Test: `apps/gui/tests/visual_shell.spec.ts`

- [ ] **Step 1: Add failing visual/accessibility assertions**

Assert no white outer frame token is rendered, menu/task/session controls have accessible names, Chinese labels fit without clipping, focus remains visible in both skins, and reduced motion disables menu animation.

- [ ] **Step 2: Verify failure**

Run: `npm --prefix apps/gui test -- visual_shell.spec.ts d1_cockpit.spec.ts`

Expected: at least the new menu/session accessibility snapshots are missing.

- [ ] **Step 3: Implement token-only styling and complete status copy**

Use the design package tokens for background, border, accent, danger, focus, density, and typography. Render connecting, install required, authentication required, provider/tool/context error, cancelled, retrying, and replayed states from Core facts.

- [ ] **Step 4: Run frontend and build checks**

Run: `npm --prefix apps/gui test && npm --prefix apps/gui run build`

Expected: all tests and TypeScript build pass.

- [ ] **Step 5: Commit visual and accessibility alignment**

```bash
git add apps/gui/src/i18n apps/gui/src/ui/window_chrome.css apps/gui/src/screens/d1_cockpit.css apps/gui/tests
git commit -m "fix(gui): align agent interaction states"
```

### Task 6: Release GUI 0.1.0-rc.2 evidence

**Files:**
- Modify: `apps/gui/src-tauri/Cargo.toml`
- Modify: `apps/gui/src-tauri/tauri.conf.json`
- Modify: `Cargo.lock`
- Modify: `docs/gui-version-functional-design.md`
- Modify: `docs/gui-version-functional-design.zh-CN.md`

- [ ] **Step 1: Set version and document the compact menu and compatibility D4 route**

- [ ] **Step 2: Run GUI candidate gates**

Run: `cargo test -p viden-gui && npm --prefix apps/gui test && npm --prefix apps/gui run build && cargo test --workspace --quiet && git diff --check`

Expected: every command exits 0.

- [ ] **Step 3: Build and launch the macOS application bundle**

Run: `npm --prefix apps/gui run tauri build`

Expected: `apps/gui/src-tauri/target/release/bundle/macos/Viden.app` exists and opens into D1 without a white outer frame.

- [ ] **Step 4: Commit the release candidate**

```bash
git add apps/gui/src-tauri/Cargo.toml apps/gui/src-tauri/tauri.conf.json Cargo.lock docs/gui-version-functional-design.md docs/gui-version-functional-design.zh-CN.md
git commit -m "chore(gui): release native and ACP interaction rc.2"
```
