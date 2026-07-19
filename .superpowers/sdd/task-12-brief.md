# Task 12 Brief: Cross-lane primitives and complete MergeGate recovery

## Execution context

- Branch: `codex/v3-core-runtime`
- Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
- Starting HEAD: `043dcdb3ae9b2034eb34e03f321d8847126fd824`
- Ownership: Core-only types, runtime trust loop/contracts/lifecycle, focused tests,
  and directly necessary bilingual Core documentation.
- Excluded: TUI, GUI, integration, Task 13+, push, release, live-provider, and
  changes outside the assigned Core ownership boundary.
- Collaboration rule: other agents may edit the shared repository; preserve
  their work and do not revert changes outside this task.

## Required deliverable

Complete the local cross-lane trust loop with typed, replayable records and
commands for handoff acceptance, review requests, contract confirmation,
dependency blocking/unblocking, canonical-evidence MergeGate decisions,
conflict bounce to the originating lane, revalidation/application, and audited
revert.

### Typed records and interfaces

- `HandoffRecord`
- `ReviewRequestRecord`
- `ContractRecord`
- `DependencyRecord`
- `MergeGateType`
- `MergeGatePolicySnapshot`
- `MergeGateValidator`
- `MergeGateDecision`
- `ConflictBounce`
- `RevertRecord`

### Runtime commands

- `CreateHandoff`
- `RequestReview`
- `ConfirmContract`
- `SetDependency`
- `BounceMergeConflict`
- `RevertAppliedChange`

### Required behavioral coverage

1. A handoff is explicitly accepted or rejected through typed state.
2. Review requests and contract confirmations are durable, typed facts.
3. Dependencies block and unblock deterministically.
4. MergeGate requires canonical evidence; summary-only evidence never passes.
5. Accept/reject decisions are typed, not strings.
6. Apply conflicts bounce to the originating lane with structured recovery.
7. A bounced change must be revalidated before merge/application.
8. Applied changes can be reverted with append-only audit evidence.

## Invariants

- Preserve owner identity across commands, events, replay, conflict bounce, and
  revert.
- Preserve schema-1 additive/unknown compatibility and frozen frontend fixture
  replay. Any capability or Core facade extension must be explicit.
- Canonical evidence is authoritative; summaries are display material only.
- The mutation order is `validate -> permission -> prepare -> workflow/audit
  precommit -> apply -> commit facts`.
- On failure, restore affected bytes/state and emit structured recovery facts.
- Frontends do not reduce business state or infer successful decisions.
- Keep all transcript/workflow facts append-only and auditable.

## TDD execution checklist

- [ ] Read and critically review the controlling Task 12 plan and current
  contracts.
- [ ] RED: typed cross-lane records and command envelope coverage.
- [ ] GREEN: minimal additive shared contracts and replay/snapshot reduction.
- [ ] RED: canonical-evidence-only typed MergeGate accept/reject behavior.
- [ ] GREEN: policy/validator/decision reduction without string decisions.
- [ ] RED: dependency blocking/unblocking and conflict bounce/revalidation.
- [ ] GREEN: transactional apply/recovery and originating-lane preservation.
- [ ] RED: audited revert and rollback-on-audit/apply failure.
- [ ] GREEN: append-only revert/recovery facts and byte restoration.
- [ ] Refresh bilingual Core/frontend contract documentation and fix the Task
  11 Chinese duplicate `序列化后的` wording nit.
- [ ] Run focused trust/merge/evidence tests, affected crate suites, Clippy,
  fmt, dependency/doc/diff gates, then workspace suite.
- [ ] Review complete diff and commit exactly
  `feat(core): complete the local merge trust loop` without pushing.

## Verification evidence

Record every RED failure, GREEN command, final gate, skipped check, and known
out-of-scope blocker in `.superpowers/sdd/task-12-report.md` before handoff.
