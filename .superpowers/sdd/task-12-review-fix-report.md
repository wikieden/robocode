# Task 12 Independent Review Fix Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
Review baseline: `4c3cccf11fd21b9f632b51d0517a634a224ce5c8`

## Outcome

The independent-review block is closed in the Core-owned trust loop:

- H1: acceptance is authorized by the assigned validator and binds the exact
  current evidence id/hash set plus the pending review request.
- H2: `AcceptAgentArtifact` cannot bypass independent review; merge requires a
  typed `Accepted` decision, completed review when required, current canonical
  bytes, and the exact reviewed bindings.
- H3: conflict revalidation is an explicit `RevalidateMergeConflict` command.
  It requires the originating Lane, exact bounce id, changed canonical receipt,
  and fresh acceptance after revalidation.
- H4: dynamic dependency edges reject missing tasks, self-edges, and cycles
  before permission. Static and dynamic blockers are aggregated for scheduling;
  clearing one edge cannot hide another active blocker.
- H5: supervisor trust mutations complete pure owner and semantic preflight
  before `ApprovalRequested`; failed preflight emits no permission callback and
  mutates neither runtime state nor workspace bytes.
- H6: provider/assistant summaries remain display-only `task_summary` evidence.
  Canonical evidence derives from real ContextStore bytes/hash and replaces
  caller claims with a Core-issued permission receipt and Core verification.

Merge recovery is now restart-safe. Preimages are content-addressed in a
workflow-owned private store. The append-only workflow event exposes only a
safe snapshot id and manifest SHA-256; it never stores raw recovery bytes.
Manifest and blob hashes, relative paths, duplicate paths, symlinks, and private
permissions are validated. Revert reloads the immutable snapshot and verifies
the current postimage before requesting permission.

## TDD Evidence

Focused RED checkpoints were captured before each implementation slice. The
corresponding GREEN contracts are:

- `trust_loop_accept_requires_validator_actor_and_exact_reviewed_hashes`;
- `trust_loop_artifact_shortcut_cannot_bypass_independent_review_policy`;
- `trust_loop_merge_rejects_stale_canonical_bytes_before_permission`;
- `trust_loop_conflict_revalidation_requires_origin_lane_and_changed_receipt`;
- `trust_loop_dynamic_dependencies_reject_invalid_edges_before_permission`;
- `trust_loop_dynamic_dependencies_block_start_and_unblock_in_aggregate`;
- `assistant_summary_never_becomes_canonical_evidence_across_resume`;
- `trust_loop_canonical_artifact_uses_core_permission_receipt_not_claimed_status`;
- `recovery_snapshot_is_restricted_content_addressed_and_tamper_evident`;
- `recovery_snapshot_rejects_unsafe_or_duplicate_paths`;
- `recovery_snapshot_rejects_symlinked_private_store_paths`; and
- `trust_loop_restart_revert_uses_durable_recovery_snapshot`.

Legacy schema-1 decision serialization and the frozen Core fixture remain
unchanged. The additive capability manifest now reports all seven trust-loop
commands.

## Documentation And Comments

The English and Chinese compatibility, frontend-integration, and multi-agent
orchestration documents were updated together. They now state the exact
validator/evidence binding, Core-issued receipt boundary, origin-Lane
revalidation, pure preflight, and private restart-safe recovery contract.
Comments are limited to the
non-obvious transaction/replay and compatibility invariants. No visual evidence,
migration, or frozen-fixture refresh is required.

## Verification

Passed:

- `cargo fmt --all -- --check`;
- `cargo test -p viden-types --quiet`: 52 passed;
- `cargo test -p viden-session --quiet`: 19 passed;
- `cargo test -p viden-workflows --quiet`: 27 passed;
- `cargo test -p viden-runtime --quiet`: 370 passed, 1 ignored live-provider test;
- `cargo test -p viden-core --quiet`: unit, 12 CoreClient, 3 frontend-contract,
  and 5 workspace-identity tests passed; 1 manual fixture refresh ignored;
- focused runtime contract, supervisor, and trust-loop suites: 64, 57, and 13
  passed respectively;
- `scripts/check-dependency-boundaries.sh`;
- Clippy with `-D warnings` for types, workflows, tools, runtime, and Core;
- bilingual pair checks, changed-document link checks, and `git diff --check`.

`cargo test --workspace --quiet` remains blocked by the separately owned
legacy TUI/Core migration mismatch (104 compile errors, led by removed Core
re-exports and legacy string/field forms). No TUI or GUI file is in this change
set.

## Second Review Follow-up

The second review found additional trust-loop hardening gaps. This follow-up
closes them without changing the frozen schema:

- `RequestReview` is now authorized by the requesting gate owner lane. Core
  derives the independent validator owner from `reviewer_lane_id`, so a reviewer
  cannot submit a self-authorizing review request.
- Supervisor and direct command preflight now reject `dependency_id` rebinding
  across different `(task_id, depends_on_task_id)` endpoints before permission.
- Manual `BounceMergeConflict` now requires the gate owner origin lane and a
  verified canonical baseline before permission. Automatic patch-conflict
  bounce uses the same validation path and no longer falls back to an empty
  baseline on canonical evidence errors.
- `RejectAgentArtifact` preflight now matches apply semantics: the evidence id
  must exist and be bound to the selected gate before approval is requested.
- Recovery snapshots reuse identical content-addressed preimage blobs across
  multiple files, and Unix private-file opens use no-follow semantics so a
  symlinked `recovery.lock` is rejected without chmoding the external target.

Additional RED/GREEN coverage:

- `trust_loop_request_review_requires_gate_owner_as_requester`;
- `trust_loop_dependency_id_cannot_be_rebound_to_different_endpoints`;
- `trust_loop_bounce_requires_gate_owner_and_valid_canonical_baseline`;
- `trust_loop_reject_agent_artifact_requires_gate_bound_evidence_before_permission`;
- `recovery_snapshot_reuses_identical_preimage_blobs_for_multiple_paths`; and
- `recovery_snapshot_rejects_symlinked_lock_without_chmoding_target`.

Second follow-up verification passed:

- `cargo test -p viden-runtime trust_loop_ --quiet`: 17 passed;
- `cargo test -p viden-workflows recovery_snapshot_ --quiet`: 5 passed;
- `cargo fmt --all -- --check`;
- `cargo test -p viden-types --quiet`: 52 passed;
- `cargo test -p viden-session --quiet`: 19 passed;
- `cargo test -p viden-workflows --quiet`: 29 passed;
- `cargo test -p viden-runtime --quiet`: 374 passed, 1 ignored live-provider
  test;
- `cargo test -p viden-core --quiet`: unit, 12 CoreClient,
  3 frontend-contract, and 5 workspace-identity tests passed; 1 manual fixture
  refresh ignored;
- `cargo clippy -p viden-types -p viden-workflows -p viden-tools -p viden-runtime -p viden-core --all-targets -- -D warnings`;
- `scripts/check-dependency-boundaries.sh`; and
- `git diff --check`.

Attempted after the second follow-up:

- `cargo test --workspace --quiet` still fails compiling the separately owned
  `viden-tui` crate with the known 104 Core API drift errors, including removed
  root `viden_core::{SessionEngine, ModelRequestControl, ProviderAuthMode}`
  re-exports and legacy string fields on typed `AgentTaskRecord` /
  `AgentLaneRecord`. No TUI or GUI files were changed.
