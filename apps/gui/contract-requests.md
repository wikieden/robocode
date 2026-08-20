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

## GUI-CORE-011: Review decision command

`frontend-contract-v1` publishes `ReviewRequestRecord` with a `Pending` status
and exposes `RuntimeCommand::RequestReview`, but no command records a review
decision. D2 therefore lists pending reviews with their Core evidence and
renders the accept/reject actions disabled with this code; it must not reuse
`AcceptLaneOutput` or an approval response as a substitute review verdict.

Close this request when Core publishes a review-decision command carrying the
review id, verdict, optional feedback, and audit id, and the canonical fixture
proves the resulting `ReviewRequestStatus` transition.

## GUI-CORE-012: Structured decision context for an approval

`ApprovalRequestView` carries only `input_preview`, an opaque display string.
The D2 design shows line-level diff rows for the pending mutation. D2 renders
the preview verbatim and declares the diff unavailable rather than parsing
display text into diff rows.

Close this request when Core publishes a typed decision context for an
approval — ordered hunks with file path, line numbers, and change kind, or an
immutable content reference the client can resolve — and the canonical
approval fixture covers a multi-file mutation.

## GUI-CORE-013: Pending contract-confirmation fact

`ContractRecord.decision` has only `Confirmed` and `Rejected`, so every
published contract is already decided. The D2 design shows a contract
confirmation queue awaiting a human. D2 lists contract records as decided
history and marks the group with this code; it must not treat a decided record
as a backlog item.

Close this request when Core publishes a pending contract-confirmation fact
with its proposer, target contract version, subscribers, and audit id, and the
canonical fixture proves that a pending contract becomes decided through
`ConfirmContract`.

## GUI-CORE-014: Ordered event log in the view state

`RuntimeViewState` publishes current facts but no ordered event log. The D10
design shows a scribe-compiled event stream across projects, and D14 needs the
same ordered history. D10 renders no ticker and declares the gap with this
code; it must not rebuild a timeline by diffing successive snapshots.

Close this request when Core publishes a bounded, ordered, owner-scoped event
log — or a replay cursor the client can page — with a stable event id, kind,
owner, and timestamp, and the canonical fixture proves ordering across two
projects.

## GUI-CORE-015: Structured merge-conflict content

`MergeGateRecord` and `ConflictBounce` name the gate, the origin Lane, and the
reason, but carry no conflict content. The D12 design shows both Lanes' hunks
side by side with conflict markers. D12 renders the Core reason text and
declares the hunk unavailable; it must not read the worktree or parse the
reason string into diff rows.

Close this request when Core publishes structured conflict content for a
bounced gate — file path, ours/theirs hunks with line numbers, and the
baseline the conflict was computed against — and the canonical merge-gate
fixture covers a two-Lane conflict on one file.

## GUI-CORE-016: Streaming Agent message chunks

The ACP adapter already receives `agent_message_chunk` updates, but
`crates/runtime/src/agent_commands.rs` accumulates them into a local string
and publishes one `AgentConversationMessageView` at turn end. No ordered event
carries a partial message, so the GUI cannot render a reply as it is produced;
it can only show the completed paragraph.

D1 therefore renders whole messages and shows the work-status strip for
liveness. It must not fake a typewriter effect over a completed message.

Close this request when Core publishes ordered chunk events carrying the
session id, the message id the chunk belongs to, the appended text, and a
terminal marker, and the canonical fixture proves that replaying the chunks
reconstructs exactly the final message.

Status: implemented in Core, fixture outstanding. `AssistantDelta` now carries
an optional session id, the ACP adapter keeps one message id for a whole
prompt turn, and the reducer grows a single owner-scoped message. Crate tests
prove that replaying the chunks reconstructs the reply exactly and that the
completion event does not duplicate it. This request stays open until the
canonical `frontend-contract-v1` fixture covers a streamed turn.

## GUI-CORE-017: Non-text Agent message content

`AgentConversationMessageView.content` is a single `String`, and
`acp_message_chunk_text` extracts only `content.type == "text"`. An ACP agent
that returns an image block therefore reaches the client as prose claiming an
image exists, with no image fact behind it — exactly what an operator sees as
"the agent says it drew something and nothing is there".

D1 renders the text Core published and does not synthesize an attachment.

Close this request when a conversation message carries typed content parts —
text plus image or file parts with a media type and an immutable content
reference the client can resolve — and the canonical fixture covers an ACP
turn that returns an image part alongside text.

Status: implemented in Core, fixture outstanding. A conversation message now
carries typed content parts, `AgentMessagePart` attaches a part to the message
it belongs to, and inline Agent bytes are persisted under
`.viden/agents/parts/` with the content digest as their file name, so the
reference is immutable and the bytes are also published as evidence. A part
kind Core does not model round-trips losslessly rather than being dropped. The
desktop shell resolves a workspace reference through the `agent_content`
command, because a webview cannot open a workspace path; it refuses any
reference outside the parts directory. This request stays open until the
canonical `frontend-contract-v1` fixture covers an ACP turn returning an image
part alongside text.

## GUI-CORE-018: Checkpoint capture and restore

D6 renders a "Restore checkpoint" recovery action, but schema 1 models no
checkpoint at all: no `RuntimeCommand` captures or restores one, no checkpoint
record exists in `RuntimeViewState`, and no event reports a restore outcome.
The other D6 actions now reach real Core commands — `restart` sends
`RetryAgentSession` and `close_lane` sends `StopLane` — which leaves
`checkpoint` as the one action with nothing behind it.

The GUI projects it as `available: false` with code `GUI-CORE-003` and never
attaches a handler. It must not simulate a restore from replay, re-open a
session at an earlier cursor, or present a snapshot re-read as a checkpoint.

Close this request when Core publishes a typed checkpoint record with a stable
id and the owner it belongs to, a command that restores one, an event carrying
the restore outcome, and a canonical `frontend-contract-v1` fixture covering a
capture followed by a restore.

## GUI-CORE-019: Always approval scope and Edit decision

`ApprovalScope` models `Once`, `Session`, and `RepoAllowlist`. The permission
dock's design also offers "Always" and "Edit": Always is a standing decision
across sessions and repositories, Edit returns a modified command for approval
instead of accepting or refusing the one proposed. Neither exists in schema 1,
so both render as fail-closed `GUI-CORE-003` placeholders and
`PermissionChoice::Always` / `PermissionChoice::Edit` are refused before any
command is built.

This also holds a keyboard divergence open. The design assigns `Shift+A` to
"Always"; the GUI keeps `Shift+A` on `repo_allowlist`, the widest scope Core
accepts, rather than binding a live chord to a dead action.

Close this request when Core models a persistent Always scope with the
revocation path that makes it safe to grant, models an Edit decision that
carries the revised command back through the same approval gate, and the
canonical fixture covers both. The GUI will then restore the design's
`Shift+A` binding.
