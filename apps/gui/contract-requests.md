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

## GUI-CORE-009: Owner-scoped typed transcript rows

The frontend contract exposes lane output as an untyped stream and exposes a
global assistant stream. It does not expose an ordered, owner-scoped user and
assistant transcript sequence. D1 therefore renders only typed lane-output
facts for the selected exact owner and declares user/assistant rows
unavailable; it must not infer roles from display text.

Close this request when Core publishes ordered transcript rows with a stable
row id, full `RuntimeOwner`, typed `user`/`assistant` role, content or an
immutable content reference, and replay/pagination cursor. The canonical D1
fixture must prove that two Lanes cannot leak rows across owners.

## GUI-CORE-010: Owner-scoped live-work facts

`AgentTaskRecord`, active tool calls, queued inputs, and evidence views do not
carry a `RuntimeOwner` in frontend-contract-v1. D1 omits these global facts
from a selected Lane rather than attributing them by timing or label.

Close this request when each live-work fact carries a full `RuntimeOwner` and
the canonical D1 fixture proves selected-Lane projection for two concurrent
owners.
