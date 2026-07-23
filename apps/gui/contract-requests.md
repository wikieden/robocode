# GUI Core Contract Requests

Chinese version: [contract-requests.zh-CN.md](contract-requests.zh-CN.md)

## GUI-CORE-008: Selected-Lane context scope

Core `0.3.5` exposes `RuntimeViewState.context_budgets`, but the
frontend-neutral `viden-core` facade does not re-export `ContextBudgetRecord`
and `ContextScope`. The GUI therefore cannot prove that a budget belongs to
the selected Lane's task without reconstructing a private serialization
schema.

Until Core exports the typed scope/budget contract through `viden-core`, D1
projects `contextDock.context` as `null`. The GUI must not select an arbitrary
budget, deserialize a guessed scope shape, or infer usage from display text.

Close this request when the facade exports the required frontend-neutral types
and the canonical D1 fixture covers two Lanes with distinct task-scoped
budgets.
