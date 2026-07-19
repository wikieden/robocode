# Task 12 Cross-lane Trust Loop Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
Starting HEAD: `043dcdb3ae9b2034eb34e03f321d8847126fd824`

## Outcome

Task 12 completes the Core-local trust loop for cross-lane work and MergeGate
recovery:

- typed handoff acceptance, review request, contract decision, dependency,
  validator/policy/decision, conflict bounce, and revert records;
- seven additive runtime commands plus schema-1 events, snapshot fields, replay,
  workflow projection, and explicit `runtime.trust_loop` capability negotiation;
- canonical-reference-only evidence acceptance, with summary text retained as
  display material rather than proof;
- independent-review gates that remain pending until the assigned validator
  explicitly binds the exact reviewed evidence id/hash set and review request;
- structured apply conflicts returned to the originating lane, mandatory
  revalidation before a second accept/merge attempt, and typed conflict
  resolution;
- private content-addressed recovery snapshots plus write-ahead workflow facts
  for merge/revert, byte-and-state compensation after downstream failure, and
  restart-safe append-only audited revert facts;
- normal supervisor approval/resume semantics for every trust-loop mutation,
  preserving the submitting owner and permission generation.

The frozen Core 0.3.0 fixtures remain unchanged. Historical schema-1 string
MergeGate decisions deserialize into a read-only `Legacy` typed outcome and
serialize back to the exact string form; new runtime paths never emit that
outcome. Empty extension fields are omitted, so existing payload bytes and
digests remain stable.

## TDD evidence

The first focused `trust_loop_` compile established RED with 36 errors because
the typed records, commands, events, snapshot fields, and MergeGate extensions
did not exist. Subsequent behavioral RED checkpoints demonstrated that:

1. accepting a review did not update its `ReviewRequestRecord`;
2. a workflow append failure did not emit a structured recoverable error;
3. an accepted handoff did not project the new MergeGate owner;
4. supervised trust mutations followed the old generic deny-only path instead
   of the resumable project-mutation approval path;
5. canonical evidence could auto-accept a gate that required independent
   review; and
6. an accepted independent review did not bind the reviewed evidence ids; and
7. a stale-evidence merge preflight created a conflict bounce before a denied
   permission decision;
8. artifact acceptance could bypass an independent-review policy;
9. conflict evidence could be reused without origin-lane, changed-receipt, and
   bounce identity checks;
10. dynamic dependencies accepted missing/self/cyclic edges and unblocked a
    task while another blocker remained; and
11. in-memory rollback bytes disappeared across runtime restart.

Pure preflight now completes before any approval request or mutation. Merge
recovery is persisted in the workflow-owned private store before file effects.

The final trust-loop integration tests cover accepted and rejected
handoffs, review/contract/dependency replay, summary-only rejection,
independent review, conflict bounce to the original lane, mandatory
revalidation, merge, resolution, audited revert, final-audit failure
compensation, and real supervisor allow/deny behavior. Existing merge/evidence
tests were migrated to typed decisions without weakening their assertions.

## Permission and transaction invariants

- Trust mutations validate ids, explicit actor/envelope ownership, referenced
  gates/evidence, exact bytes/hashes, dynamic dependencies, and command-specific
  preconditions before requesting permission.
- The supervisor queues them through the existing owner-bound
  `ProjectMutation` state machine. Approval resumes the exact command and
  permission epoch; denial leaves runtime facts and local files unchanged.
- Merge executes as `pure preflight -> permission -> prepare -> private recovery
  snapshot -> durable precommit -> file effect -> typed final facts -> durable
  projection`. Revert validates the immutable snapshot and current postimage
  before permission, including after runtime restart.
- If the final projection fails, the outer command transaction restores exact
  pre-effect bytes and runtime vectors, then emits `CommandRejected` plus a
  recoverable structured `Error`. The write-ahead audit marker remains durable.
- Unknown or mismatched artifacts are rejected before approval. Provider and
  assistant summaries remain non-canonical; caller-claimed verification,
  permission, and quality are replaced by Core-derived bytes/hash, a Core-issued
  permission receipt, and Core verification facts.
- Conflict bounces retain the originating lane and owner. Revalidation requires
  that lane, the exact bounce, and a changed canonical receipt before a new
  independent acceptance can merge.
- Revert uses the recorded applied-change identity and a manifest-hashed private
  snapshot to restore exact pre-apply bytes and modes; successful revert facts
  are append-only and raw recovery bytes never enter the event log.

## Compatibility, documentation, and comments

The shared protocol extension manifest now advertises `runtime.trust_loop` and
the seven commands/new events. Types and Core facade tests cover unknown-field
tolerance, command round trips, typed decisions, legacy string migration, and
the unchanged frozen payload digest.

English and Chinese compatibility, frontend integration, and multi-agent Core
orchestration documents were updated together. They describe typed authority,
canonical evidence, independent validation, conflict/revalidation, write-ahead
apply/revert, and frontend replay responsibilities. The Task 11 Chinese
duplicate `序列化后的` wording was also corrected. Comments were added only for
the schema-1 migration boundary, transaction ordering, and the lane-supervisor
completion barrier; no migration, fixture refresh, screenshot, or visual
evidence was required.

## Verification

Passed:

- `cargo fmt --all -- --check`;
- `cargo test -p viden-types --quiet`: 52 passed;
- `cargo test -p viden-session --quiet`: 19 passed;
- `cargo test -p viden-workflows --quiet`: 27 passed;
- `cargo test -p viden-runtime --quiet`: 370 passed, 1 ignored live-provider
  test;
- `cargo test -p viden-core --quiet`: unit, 12 CoreClient, 3
  frontend-contract, and 5 workspace-identity tests passed; 1 manual fixture
  refresh ignored;
- `cargo clippy -p viden-types -p viden-workflows -p viden-tools -p viden-runtime -p viden-core --all-targets -- -D warnings`;
- `scripts/check-dependency-boundaries.sh`;
- paired-document checks for all three changed English/Chinese pairs;
- link checks for the six changed bilingual documents and three SDD reports;
- `git diff --check`;
- `failed_permission_controls_leave_lane_engine_and_epoch_unchanged` in 30
  consecutive focused reruns after 20 earlier successful reruns.

The permission-control rerun confirmed a test-observation race rather than a
production epoch failure: `ApprovalResolved` is intentionally published before
the approved effect completes, while the response command's `CommandAccepted`
is published after the LaneSupervisor completion barrier. The test already
waited for both facts; a concise comment now records why both are required. No
production permission or epoch semantics changed.

Attempted but blocked outside Task 12 ownership:

- `cargo test --workspace --quiet` reaches the pre-existing TUI/Core migration
  mismatch and fails compiling `viden-tui` with 104 errors. The representative
  failures are removed root Core re-exports (`SessionEngine`,
  `ModelRequestControl`, provider/engine types), the historical
  `ApprovalResponse.approved` field, and old string/field forms of typed
  `AgentTaskRecord`/`AgentLaneRecord`. No TUI or GUI file was changed.

## Handoff

Owned changes are limited to shared Core types/protocols, runtime trust-loop,
supervisor/session/workflow projection, Core extension compatibility tests,
paired Core/frontend/orchestration documentation, and this SDD evidence. Task
13+, frontend migration, live-provider, push, merge, tag, release, and fixture
refresh work were not performed.
