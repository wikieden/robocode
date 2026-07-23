# Native and ACP Integration Certification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Certify one workspace candidate in which Core `0.3.4`, TUI `0.3.3`, and GUI `0.1.0-rc.2` complete the same native and ACP interaction loops.

**Architecture:** Integrate immutable checkpoints in the fixed order Core -> TUI -> GUI. Use one canonical ordered-event fixture for deterministic parity, then collect presence-safe live DeepSeek and ACP evidence without recording credentials.

**Tech Stack:** Git worktrees, Cargo, shell gates, deterministic JSON fixtures, TUI previews, Tauri build, live provider/ACP smoke scripts.

## Global Constraints

- Integration order is Core `0.3.4` -> TUI `0.3.3` -> GUI `0.1.0-rc.2`.
- Frontend branches must share the exact Core checkpoint SHA.
- Native acceptance covers configure, start, converse, control, observe, finish, and recover.
- ACP acceptance covers discover/install/auth, start, converse/resume, approve/control, result/evidence, retry, cancel, and restart recovery.
- Live evidence records provider/model/adapter identifiers and success states only; never key values or raw authentication output.
- No merge or push to `main` without separate explicit authorization.

---

### Task 1: Record immutable checkpoints and integrate in fixed order

**Files:**
- Create: `docs/release-evidence/native-acp-interaction/checkpoints.md`
- Modify: `docs/parallel-development-plan.md`
- Modify: `docs/parallel-development-plan.zh-CN.md`

**Interfaces:**
- Consumes: Core, TUI, and GUI terminal commits.
- Produces: a table containing branch, worktree, version, base Core SHA, terminal SHA, and verification status.

- [ ] **Step 1: Verify branch ancestry before integration**

Run:

```bash
git merge-base --is-ancestor codex/v3-core-runtime codex/v3-tui-client
git merge-base --is-ancestor codex/v3-core-runtime codex/v3-gui-client
```

Expected: both commands exit 0.

- [ ] **Step 2: Create an integration worktree from the Core checkpoint**

```bash
git worktree add .worktrees/native-acp-integration -b codex/native-acp-integration codex/v3-core-runtime
```

Expected: the new worktree is clean and points at Core `0.3.4`.

- [ ] **Step 3: Merge TUI then GUI with explicit merge commits**

```bash
git merge --no-ff codex/v3-tui-client -m "merge: integrate TUI 0.3.3"
git merge --no-ff codex/v3-gui-client -m "merge: integrate GUI 0.1.0-rc.2"
```

Expected: no ownership-scope conflicts; shared manifest conflicts preserve all three independent versions.

- [ ] **Step 4: Record exact SHAs and commit evidence metadata**

```bash
git rev-parse HEAD
git status --short
git add docs/release-evidence/native-acp-interaction/checkpoints.md docs/parallel-development-plan.md docs/parallel-development-plan.zh-CN.md
git commit -m "docs: record native and ACP integration checkpoints"
```

### Task 2: Prove deterministic Core/TUI/GUI parity

**Files:**
- Create: `scripts/native-acp-fixture-parity.sh`
- Create: `docs/release-evidence/native-acp-interaction/fixture-parity.md`
- Test: `crates/core/tests/fixtures/native-acp-interaction-v1.jsonl`

**Interfaces:**
- Consumes: the Core canonical fixture.
- Produces: normalized Core, TUI, and GUI business projections whose SHA-256 digests match.

- [ ] **Step 1: Write the parity script with strict failures**

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo test -p viden-core --test frontend_contract_v1 native_acp_fixture_snapshot_matches_ordered_replay
cargo test -p viden-tui native_acp_fixture_render
cargo test -p viden-gui native_acp_fixture_projection
```

- [ ] **Step 2: Run the script and verify initial missing GUI/TUI parity names fail if not wired**

Run: `bash scripts/native-acp-fixture-parity.sh`

Expected before wiring: non-zero because every named parity test must exist; after wiring: exit 0.

- [ ] **Step 3: Record normalized facts, not screenshots, in the parity report**

Record counts and ids for Lane receipt, native turn, ACP sessions, approvals, tool results, costs, evidence, terminal states, retry attempt, and replay cursor.

- [ ] **Step 4: Commit deterministic certification**

```bash
git add scripts/native-acp-fixture-parity.sh docs/release-evidence/native-acp-interaction/fixture-parity.md
git commit -m "test: certify native and ACP frontend parity"
```

### Task 3: Collect live DeepSeek native-agent evidence

**Files:**
- Create: `scripts/live-native-agent-smoke.sh`
- Create: `docs/release-evidence/native-acp-interaction/live-native-agent.md`

**Interfaces:**
- Consumes: `DEEPSEEK_API_KEY` by presence only and configured DeepSeek model.
- Produces: Lane id, provider id, model id, ordered state sequence, tool/evidence ids, token/cost totals, and recovery result.

- [ ] **Step 1: Add a presence-safe smoke script**

The script exits 77 with `SKIP: DEEPSEEK_API_KEY is not present` when unset. When present, it runs a deterministic read-only repository task, queues one follow-up during streaming, observes completion, reconnects, and verifies the transcript and cost facts return.

- [ ] **Step 2: Run the live native smoke**

Run: `bash scripts/live-native-agent-smoke.sh`

Expected: exit 0 with `LIVE_NATIVE_AGENT_PASS`; never print the key or environment dump.

- [ ] **Step 3: Record redacted evidence and commit**

```bash
git add scripts/live-native-agent-smoke.sh docs/release-evidence/native-acp-interaction/live-native-agent.md
git commit -m "test: record live DeepSeek native agent evidence"
```

### Task 4: Collect live ACP discovery, conversation, control, and recovery evidence

**Files:**
- Create: `scripts/live-acp-agent-smoke.sh`
- Create: `docs/release-evidence/native-acp-interaction/live-acp-agent.md`

**Interfaces:**
- Consumes: the first Core-probed adapter with `AgentStartability::Ready`, preferring Codex, then Claude, then Kiro, then configured custom ACP.
- Produces: adapter id, advertised capabilities/models, session id, follow-up input id, approval audit id when requested, result evidence id, cancel result, and restored terminal state.

- [ ] **Step 1: Add a readiness-driven live ACP script**

The script queries and probes adapters, selects only a Core-reported `Ready` adapter, starts a read-only delegated task in an existing Lane, sends one exact-session follow-up, starts a second attempt and cancels it, restarts the client host, and verifies both sessions restore. Exit 77 with a classified skip when no adapter is ready.

- [ ] **Step 2: Run the live ACP smoke**

Run: `bash scripts/live-acp-agent-smoke.sh`

Expected: exit 0 with `LIVE_ACP_AGENT_PASS`; no raw stderr, auth material, or command environment appears.

- [ ] **Step 3: Record redacted evidence and commit**

```bash
git add scripts/live-acp-agent-smoke.sh docs/release-evidence/native-acp-interaction/live-acp-agent.md
git commit -m "test: record live ACP interaction evidence"
```

### Task 5: Run user-facing TUI and GUI experience gates

**Files:**
- Create: `docs/release-evidence/native-acp-interaction/tui-experience.md`
- Create: `docs/release-evidence/native-acp-interaction/gui-experience.md`

- [ ] **Step 1: Run TUI gates and inspect previews**

Run: `cargo test -p viden-tui && scripts/tui-turn-controller-smoke.sh && scripts/rc-tui-stability-smoke.sh && scripts/tui-regression.sh && scripts/tui-previews.sh`

Expected: all exit 0; evidence covers `n`, `/acp`, adapter/session picker, focus, follow-up, approval, cancel, failure, and recovery.

- [ ] **Step 2: Run GUI gates and build the app**

Run: `cargo test -p viden-gui && npm --prefix apps/gui test && npm --prefix apps/gui run build && npm --prefix apps/gui run tauri build`

Expected: all exit 0 and the bundle exists at `apps/gui/src-tauri/target/release/bundle/macos/Viden.app`.

- [ ] **Step 3: Manually verify the two primary GUI paths**

Open a Git folder from Welcome, then verify `+ -> Viden Agent -> task` reaches D1 with a created Lane. Select that Lane, then verify `+ -> Codex/Claude/Kiro/Custom ACP -> task` creates a child session without opening D4 or D11. Confirm no white outer frame and keyboard-only operation.

- [ ] **Step 4: Commit experience evidence**

```bash
git add docs/release-evidence/native-acp-interaction/tui-experience.md docs/release-evidence/native-acp-interaction/gui-experience.md
git commit -m "docs: record native and ACP client experience"
```

### Task 6: Run the workspace certification gate

**Files:**
- Create: `docs/release-evidence/native-acp-interaction/certification.md`

- [ ] **Step 1: Run formatting, dependency, diff, and workspace tests**

Run: `cargo fmt --check && scripts/check-dependency-boundaries.sh && bash scripts/native-acp-fixture-parity.sh && cargo test --workspace --quiet && git diff --check`

Expected: every command exits 0.

- [ ] **Step 2: Complete the acceptance matrix**

Record PASS/FAIL plus evidence location for native and ACP Configure/Discover, Start, Converse, Control, Observe, Finish, and Recover rows. A skipped live provider or ACP row blocks certification rather than being silently treated as pass.

- [ ] **Step 3: Commit the certification record**

```bash
git add docs/release-evidence/native-acp-interaction/certification.md
git commit -m "docs: certify native and ACP interaction milestone"
```

- [ ] **Step 4: Stop before main mutation**

Report the integration branch, worktree, terminal SHA, three component versions, exact checks, live evidence, skipped gates, and blockers. Do not tag, push, merge, or release until the user authorizes that separate action.
