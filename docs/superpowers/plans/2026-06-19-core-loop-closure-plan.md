# Core Programming Loop Closure Plan (2026-06-19)

Completion plan for the seven closure gaps in RoboCode's core agent loop
(`SessionEngine::process_input_with_approval_and_control`,
`robocode-core/src/runtime_loop.rs`). Maps each gap to phased, code-level work
with TDD and release gates, sequenced into the existing `0.2.x` roadmap.

This plan is behavior-level; it does not port `.ref/claude-code-main`.

## Problem — the seven gaps (evidence)

| # | Gap | Evidence |
|---|-----|----------|
| 1 | Multi-step autonomy not closed: fixed 8-iteration cap + break-on-text heuristic | `runtime_loop.rs:79` (`for _ in 0..8`), `:181` (`if !observed_tool_call \|\| observed_text { break }`) |
| 2 | Token/cost not in the loop: char-based budget; `ContextBundleRecord` token fields are display-only; usage is telemetry-only; no tool-result dedup | `runtime_loop.rs:20`, `:647` `fit_provider_request_budget`; `robocode-types/src/lib.rs:294-318`; `runtime_loop.rs:461` |
| 3 | No verification closure: only post-edit LSP diagnostics; no auto build/test/lint, no failure classification, no done-gate | `runtime_loop.rs:373-387`, `:185` (Done set unconditionally), `:286` (failure becomes a plain Tool message) |
| 4 | Single monolithic coder loop; no supervised planner/coder/reviewer/tester/doc roles | absence; lanes are delegated terminals, not role participants |
| 5 | Event/state not unified: synchronous `Vec<EngineEvent>` + parallel `upsert_agent_task`; provider events fully collected (no true stream) | `runtime_loop.rs:42`, `:100` `next_events_with_control` returns `Vec`, `:84/:161` upserts |
| 6 | Plan mode is a permission gate, not a plan→approve→execute loop construct | `robocode-permissions` plan-mode tests; no plan artifact produced/queued by the loop |
| 7 | Synchronous tool execution blocks the loop; single-shot request-too-large retry | `runtime_loop.rs:276` (sync `tools.execute`), `:78/:113-135` (`retried_request_too_large` bool) |

Already closed (do not rebuild): permission-before-mutation, tool-result
feedback into transcript, post-edit LSP diagnostics injection, single 413
compaction retry, append-only JSONL audit + resume.

## Invariants to preserve (every phase)

- All model tool calls and command effects flow through the shared runtime path.
- Permission checks happen before mutation.
- JSONL transcripts stay canonical and append-only; SQLite derived/rebuildable.
- Plan mode blocks mutating workflow/file/shell/Git/memory/task changes.
- Assistant-suggested memory needs explicit confirmation.
- TDD for behavior changes; bilingual docs updated in the same change set.

## Phases

Dependency order: **A → (B, C) → D → E**. A unblocks all. Mapped to roadmap:
A,D ⊂ `0.2.0`; B ⊂ `0.2.1`; C,E ⊂ `0.2.2`; gating ⊂ `0.2.3`.

### Phase A — Close the iteration loop + event seam  (gap #1, seam for #5)

Smallest change, largest unlock; precondition for C/D/E.

**Changes**
- A1. Replace termination. Delete the `observed_text` break. New rule: continue
  while the just-processed provider turn contained ≥1 `ModelEvent::ToolCall`;
  stop when a turn yields no tool calls (pure text / `Done`). Assistant text and
  tool calls may coexist in one turn (Anthropic interleaves) — text no longer ends
  the turn.
- A2. Replace the fixed `0..8` with a `TurnBudget { max_tool_iterations,
  soft_token_budget, wall_clock }` (config-resolved, `robocode-config`). On
  exhaustion, emit a `TurnEvent::BudgetExhausted { reason }` and pause for an
  explicit continue, instead of silently breaking. Default `max_tool_iterations`
  ≈ 25; token guard wired in Phase B.
- A3. Event seam: introduce `TurnEvent` (superset of `EngineEvent`) pushed to one
  sink; make `AgentTask` upserts derive from that sink. Full unification in D3.

**Tests (TDD, `robocode-core`)**
- `loop_continues_past_eight_tool_iterations`
- `assistant_text_with_tool_call_does_not_end_turn`
- `turn_stops_when_no_tool_call_emitted`
- `budget_exhaustion_emits_event_not_silent_break`

**Exit gate**: a fixture task (read→edit→shell→edit→text-done, >8 steps) runs to
completion; `cargo test -p robocode-core`; workspace test green.

### Phase B — Real token + cost accounting  (gap #2)

**Changes**
- B1. `TokenCounter` trait in `robocode-model` (per-provider; heuristic fallback).
  Populate `ContextBundleRecord.estimated_tokens` from real counts.
- B2. Gate `fit_provider_request_budget` on the bundle's `soft_token_budget` /
  `hard_token_limit`; the 48k char constant becomes a fallback only. One budget
  source, not two.
- B3. Tool-result dedup + semantic compaction: collapse repeated identical reads
  by content hash; replace pure middle-char truncation with a short structured
  summary of dropped spans.
- B4. Cost: cumulative `ModelUsage` → `RuntimeSnapshot` → visible cost panel;
  feed soft-budget pressure into the Phase A budget guard.

**Tests**
- `estimated_tokens_within_tolerance_on_fixtures`
- `request_gating_uses_bundle_token_budget`
- `duplicate_file_reads_collapse_in_request`
- `cumulative_cost_surfaces_in_snapshot`

**Exit gate**: DeepSeek 413 reproduction no longer trips at the prior threshold;
focused + workspace tests green.

### Phase C — Verification closure + plan loop  (gaps #3, #6)

**Changes**
- C1. Generalize `post_edit_diagnostics_message` → `post_action_verification`:
  after mutating tools settle in a turn, optionally run a configured/detected
  verify set (`cargo test`/`clippy`/`fmt`, project-detected), permission-gated,
  evidence-attached, result fed back as the next turn input.
- C2. `FailureClass` (compile / test-fail / denied / not-found / timeout) on
  `AgentTask` + ToolResult; attach a next-action hint message so the loop lets the
  model recover instead of stalling.
- C3. Done-gate: if verification is configured and last run failed, finish the turn
  as `NeedsAttention`, not `Done` (`runtime_loop.rs:185` becomes conditional).
- C4. Plan loop: in plan mode the loop produces a `PlanArtifact` (ordered steps),
  emits + holds it; on approval, executes queued steps through the normal path.
  Plan mode stops being deny-only.

**Tests**
- `failed_test_feeds_failure_class_and_loop_continues`
- `done_gate_flips_to_needs_attention_on_failing_verify`
- `plan_artifact_produced_queued_and_executed_on_approval`
- `plan_mode_still_blocks_mutations_until_approved`

**Exit gate**: deterministic daily-loop smoke (request→edit→approve→verify→diff→
evidence) closes automatically; plan-mode smoke passes.

### Phase D — Async tool jobs + full event closure  (gaps #7, #5-complete)

**Changes**
- D1. Long tools (shell, web, test) move to a job model: spawn, stream tail events,
  non-blocking loop; in-flight interrupt via `ToolJobControl` (mirror
  `ModelRequestControl`).
- D2. Replace the single retry bool with a bounded `RetryPolicy` (413→compact,
  transient→backoff, rate-limit→wait), classified and capped.
- D3. Finalize one `RuntimeSnapshot` event stream; TUI subscribes; remove the dual
  channel. True streamed `AssistantText` tokens where the provider supports it.

**Tests**
- `long_shell_emits_tail_events_and_is_cancelable`
- `retry_policy_respects_caps_and_classes`
- `snapshot_stream_drives_headless_consumer`

**Exit gate**: a 30s shell tool stays non-blocking + cancelable; streaming/
scrollback smoke green; TUI previews regenerated (`scripts/tui-previews.sh`).

### Phase E — Supervised roles  (gap #4)

**Changes**
- E1. `Role` abstraction (planner/coder/reviewer/tester/doc-writer) as supervised
  loop participants with typed inputs/outputs/evidence/failure-class/next-action —
  built on A (loop), C2 (classification), D3 (stream).
- E2. Orchestrator routes a task through roles; lanes (external Codex/Claude/shell)
  become a role backend, not a separate path. Reviewer rejection loops back to coder.

**Tests**
- `task_flows_plan_code_test_review_with_per_role_evidence`
- `reviewer_rejection_loops_back_to_coder`
- `external_lane_serves_as_reviewer_role_backend`

**Exit gate**: one task completes plan→code→test→review with evidence per role on
the shared runtime; real-development-scenario gate (`0.2.3`) updated.

## Sequencing into releases

- `0.2.0` — Phase A (loop closure + event seam) and Phase D (async jobs, full
  event stream). Runtime layering / event closure as the roadmap states.
- `0.2.1` — Phase B (context/token/cost engine).
- `0.2.2` — Phase C (verification + plan loop) then Phase E (supervised roles).
- `0.2.3` — add to the mandatory release gate: multi-step autonomy smoke (A),
  token/cost summary (B), auto-verify daily-loop (C), async-tool/cancel smoke (D),
  role-flow smoke (E).

## Risks

- A1 termination change can run away → bounded by A2 budget + explicit continue.
- B token counts are provider-specific → heuristic fallback + tolerance tests.
- C auto-verify must stay permission-gated → no unprompted shell on deny modes.
- D async refactor touches the synchronous core → land behind the event seam (A3)
  before D3 removes the old channel.
- E is the largest; do not start before A/B/C/D contracts are stable.

## Verification & gating (per change set)

`cargo fmt` on edited files → focused crate tests → `cargo test --workspace
--quiet` for shared/release-facing changes → TUI previews for visual changes →
bilingual docs updated. State honestly what was not tested.

## Progress log

- **2026-06-19 — Phase A1 + A2 (first slice) landed.** `robocode-core`:
  - `runtime_loop.rs`: termination changed from `if !observed_tool_call ||
    observed_text` to `if !observed_tool_call` — assistant text alongside a tool
    call no longer ends the turn, so every tool result is fed back. Fixed `0..8`
    cap replaced by `SessionEngine::turn_budget.max_tool_iterations` (default 25);
    on exhaustion the loop emits an `EngineEvent::System("RoboCode turn budget
    exhausted …")` and marks the provider task paused instead of silently breaking.
  - `lib.rs`: new `TurnBudget { max_tool_iterations }` (default 25) field +
    `set_max_tool_iterations()` / `turn_budget()`.
  - Tests (`tests/runtime_loop_tests.rs`):
    `assistant_text_with_tool_call_in_same_turn_continues_loop`,
    `tool_loop_respects_iteration_budget_and_emits_event` (+ `AlwaysToolCallProvider`).
  - Verified: `cargo test -p robocode-core` 129 passed / 1 ignored (live-deepseek,
    no creds); `cargo clippy -p robocode-core` clean; `cargo fmt` clean. Workspace
    gate in progress.
  - Follow-ups still open in Phase A: config-resolved budget via `robocode-config`
    (currently default + setter only); true pause/resume "continue" turn (the slice
    emits the event + ends; it does not yet persist a resumable paused turn);
    wall-clock + token guards (token guard lands with Phase B); A3 event seam.
- **2026-06-19 — Phase C2 (first slice) landed.** `robocode-core/runtime_loop.rs`:
  - On a failed tool result, `classify_tool_failure()` derives a `ToolFailureClass`
    (not_found / directory_target / compile_error / test_failure / timeout / other)
    and a paired next-action hint, injected as an `EngineEvent::System` /
    transcript message so it is fed back to the model on the follow-up turn; the
    class is also recorded on the tool task evidence (`failure_class <x>`).
  - Test: `failed_tool_result_includes_failure_classification_and_next_action`.
    Existing `failed_tool_execution_is_returned_to_provider_without_ending_turn`
    still passes (now also gets a directory_target hint).
  - `ToolFailureClass` is a private string-based enum for now; promote to a shared
    `robocode-types` contract when Phase E roles branch on it. Verification: see
    workspace gate result below.
  - Slices A1/A2 + C2 committed on branch `core-loop-closure` (`main` untouched):
    `cargo test --workspace` 587 passed / 4 ignored.
- **2026-06-19 — Phase A2 follow-up: budget is now config-resolved.** The turn
  iteration ceiling is no longer default-only:
  - `robocode-config`: `ResolvedConfig.max_tool_iterations` (default 25), file key
    `max_tool_iterations`, env `ROBOCODE_MAX_TOOL_ITERATIONS` (both clamped >= 1).
  - `robocode-cli/main.rs`: applies `resolved_config.max_tool_iterations` to the
    engine at startup (single production site; the 7 `tui/app.rs` sites are tests).
  - Test: `max_tool_iterations_defaults_and_resolves_from_file_and_env`
    (default / file / env-overrides-file).
  - Not done: surfacing the value in `ResolvedConfig::summary()` (skipped to avoid
    churning summary-string assertions); a CLI flag (`CliOverrides`) — config file +
    env cover it for now; user-facing config doc for the new key/env.
