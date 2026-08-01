# Task 12 Independent Review Fix Brief

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Baseline: `4c3cccf11fd21b9f632b51d0517a634a224ce5c8`
Commit target: `fix(core): enforce merge trust invariants`

## Scope

Task 13 remains paused. This follow-up is limited to the independent Task 12
review block: reviewer authority/evidence binding, artifact-policy parity,
origin-lane conflict revalidation, dynamic dependency aggregation, pure
supervisor preflight, assistant-summary non-canonicalization, and restart-safe
durable merge recovery. TUI, GUI, push, release, and live providers remain out
of scope.

## Acceptance matrix

- H1: `AcceptMergeGate` carries an owner and reviewed evidence receipts. The
  supervisor envelope owner, command owner, and independent validator owner are
  equal. Acceptance binds only the reviewer's id/hash receipts captured by the
  review request; evidence drift cannot be blessed from the gate's current
  list.
- H2: `AcceptAgentArtifact` cannot bypass canonical gate policy, independent
  review, validator timestamp, or a pending conflict. `MergeAgentPatch` accepts
  only a fully typed `Accepted` decision whose authority/evidence still match.
- H3: a pending conflict is revalidated only by new canonical evidence from the
  original lane, explicitly linked to the bounce, with a changed receipt/hash.
  Immediate accept and unrelated/unchanged evidence remain rejected.
- H4: dynamic dependencies require existing tasks, reject self/cyclic edges,
  block `StartAgentTask`, and aggregate all static/dynamic blockers before
  returning a task to `Queued`.
- H5: supervisor preparation is a pure validation phase. Invalid/stale/unknown
  canonical hash, policy, conflict, or owner facts produce `CommandRejected`
  without `ApprovalRequested` and without state changes. Approved execution is
  the first point where a real apply conflict may become durable.
- H6: assistant/task summary text never becomes canonical content or a passing
  receipt. Canonical proof requires real artifact/effect bytes, a verified
  hash, and a permission snapshot.
- Important: merge preimages live in a permission-restricted, workflow-owned
  recovery store. Runtime events contain only a safe snapshot id/hash. After
  restart/replay, revert verifies the manifest and blob hashes before restoring
  bytes. Raw/secret bytes never enter transcript or audit JSONL, and failed
  audit/apply paths compensate bytes and state.

## TDD log

Record every RED and GREEN command in
`.superpowers/sdd/task-12-review-fix-report.md` before commit.
