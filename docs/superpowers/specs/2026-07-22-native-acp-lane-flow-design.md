# Native And ACP Lane Flow Design

Chinese version: [2026-07-22-native-acp-lane-flow-design.zh-CN.md](2026-07-22-native-acp-lane-flow-design.zh-CN.md)

## Status

Approved interaction direction for the next shared milestone:

- Core `0.3.4`;
- TUI `0.3.3`;
- GUI `0.1.0-rc.2`.

This document defines the product and contract target. It does not claim that
the behavior is implemented in the current release candidates.

## Goal

Make the shortest useful Viden workflow real in both frontends:

1. open a Git project;
2. create a Lane backed by one Viden-native primary agent;
3. send the first task to that agent through Core;
4. optionally delegate work to one or more ACP agents;
5. observe, approve, cancel, and recover the work from either TUI or GUI.

The Viden-native agent is driven by the DeepSeek or OpenAI provider selected by
Core. Codex, Claude, Kiro, and custom ACP servers are external delegated agents,
not model providers and not alternative authorities for Lane state.

## Product Model

- A Lane owns exactly one Viden-native primary agent runtime.
- The primary agent uses Core-owned provider, model, tool, permission, session,
  transcript, task, and evidence services.
- An ACP agent always belongs to an existing Lane as a delegated child session.
- A Lane may own multiple ACP child sessions.
- Creating a Lane and starting an agent are separate outcomes. A confirmed
  `StarterLaneCreated` receipt is never reverted or hidden because a later
  native or ACP start fails.
- Core remains the only business-state authority. Frontends retain only drafts,
  focus, menu state, and other presentation state.

## Shared Core Flow

### Project eligibility

Before offering creation, Core publishes whether the bound workspace is a Git
repository with a valid `HEAD`. An ineligible workspace produces a typed reason
and no starter-Lane mutation. Frontends must show that reason at the creation
entry instead of allowing a late generic failure.

### Native Lane

The canonical sequence is:

1. frontend sends the reviewed starter-Lane request;
2. Core publishes preview facts;
3. the user approves the worktree/branch mutation when required;
4. Core publishes the exact Lane receipt;
5. the frontend focuses the confirmed Lane immediately;
6. the first submitted task starts the Lane's Viden-native primary agent;
7. Core streams task, turn, tool, approval, transcript, usage, and evidence
   facts.

Provider and model are Core-owned project/session choices. Normal Lane creation
inherits those choices. Changing them is a composer or settings action and is
not part of the creation menu.

### ACP delegation

ACP delegation is allowed only from an existing Lane. The canonical sequence
is:

1. query Core's typed ACP adapter list;
2. select an adapter;
3. probe installation and agent-native authentication when needed;
4. enter the delegated task;
5. start an owner-scoped ACP session;
6. stream status, messages, tool requests, approvals, results, and evidence;
7. allow independent completion, failure, or cancellation.

An ACP failure never changes the confirmed Lane receipt or terminates the
Viden-native primary agent. Core must publish a truthful startability result;
tests may not manufacture an `auth_state=ready` value that production probing
cannot produce.

## Complete Agent Interaction Contract

This milestone covers the complete operator interaction, not only Lane and
session creation.

### Viden-native primary agent

The native interaction must support:

- provider/model health and configuration for DeepSeek and OpenAI;
- first task submission, streamed assistant output, tool progress, token/cost
  updates, and durable transcript rows;
- editable follow-up input while idle and queued follow-up input while busy;
- scoped tool approval and denial through the shared permission contract;
- exact-owner cancellation of the active turn without deleting the Lane;
- terminal completed, cancelled, provider-failed, tool-failed, and
  context-exhausted states with a typed next action;
- retry after a recoverable failure without duplicating the previous accepted
  command;
- restart/reconnect recovery through snapshot, replay, and transcript paging;
- continued conversation in the same Lane after completion or cancellation.

The frontend state machine is a projection of Core facts:

`idle -> submitting -> running -> waiting_approval -> running -> completed`.

`running` may also move to `cancelling -> cancelled`, or to `failed`. A
recoverable `failed` state may return to `submitting` only after a new typed
retry/submit command. A frontend must never infer these transitions from text.

### ACP delegated agent

The ACP interaction must support:

- discovery of Codex, Claude, Kiro, and configured custom ACP descriptors;
- install, initialize, authentication, capability, and model availability;
- starting a new owner-scoped session with an explicit delegated task;
- streaming ACP messages, plan/progress updates, tool calls, and terminal
  results into the owning Lane timeline;
- routing ACP tool permission requests through Core's shared approval surface;
- sending a follow-up to, or resuming, the exact ACP session when the adapter
  advertises that capability;
- cancelling the exact running ACP session without cancelling the native agent
  or sibling ACP sessions;
- preserving completed, failed, and cancelled session records with result and
  evidence links;
- retrying a failed start as a new attempt while retaining the failed attempt;
- restoring known sessions after frontend restart from Core state.

The ACP state machine is:

`discovered -> probing -> ready -> starting -> running -> completed`.

Alternative typed states are `install_required`, `authentication_required`,
`waiting_approval`, `cancelling`, `cancelled`, `failed`, and `disconnected`.
Only `ready` may start a session. A successful initialize probe must publish an
honest startability result; `unknown` is not silently treated as `ready`.

The current contract has typed start and cancel commands, but the implementation
plan must also close the conversational follow-up/resume path. That path must be
an additive Core command/event contract bound to the exact ACP session id; TUI
or GUI may not emulate it by launching an untracked CLI process or parsing
display output.

### Shared interaction and evidence

- Native and ACP activity appears in one Lane timeline with explicit source and
  owner identity.
- Users can switch between the native conversation and each ACP child session
  without changing the owning workspace or Lane.
- Every tool request shows agent source, Lane, session, target, risk, and allowed
  scopes before approval.
- A final result links the originating task, transcript range, tool results,
  changed files, tests, usage, and evidence available from Core.
- “Completed” means Core published the matching terminal fact. An assistant
  sentence claiming completion is not sufficient.

## GUI Interaction

The D4 four-step wizard is removed from the normal creation path. The `+`
button opens one compact, Zed-style menu:

```text
NEW LANE
  Viden Agent

DELEGATE TO CURRENT LANE
  Codex
  Claude
  Kiro
  Custom ACP...
```

Behavior:

- `Viden Agent` generates the Lane id, branch, and worktree using Core defaults.
- The compact menu changes to a small Core preview/approval state only when a
  mutation confirmation is required.
- After the Lane receipt, GUI opens the Lane composer. The user's first message
  is the initial native-agent task.
- Provider/model remain in the composer footer and settings.
- Delegation entries are disabled until a Lane is selected.
- Selecting an ACP entry opens only the delegated-task composer. Installation,
  authentication, or start errors remain attached to that child session.
- The selected Lane shows one native conversation and an ACP child-session
  switcher. Each ACP item shows starting/running/waiting/completed/failed status,
  unread activity, cancel when allowed, and its result/evidence entry.
- The same composer sends native follow-ups when the native conversation is
  selected and exact-session ACP follow-ups when an ACP child is selected.
- Advanced branch, worktree, budget, and policy controls move to Lane settings
  and do not block the default path.

The primary GUI path is therefore:

`+ -> Viden Agent -> type task`.

The delegation path is:

`select Lane -> + -> ACP agent -> type delegated task`.

## TUI Interaction

TUI keeps conventional terminal interaction instead of copying the GUI menu.

- `n` or the existing new-Lane command creates a default Viden-native Lane and
  focuses its composer.
- ACP delegation is part of the system command list as `/acp`.
- Selecting `/acp` opens a keyboard-operated list of Core-published ACP
  adapters.
- After adapter selection, TUI asks for the delegated task and starts the child
  session through the shared Core command.
- The `/acp` list shows readiness and active/recent child-session status without
  reading process tables. Selecting an existing session focuses its transcript;
  selecting an adapter starts a new delegated-task flow.
- While an ACP session is focused, normal composer input is routed to the exact
  resumable session; the status/side surface retains the native agent and sibling
  sessions.
- `/acp` is visibly disabled with a reason when no Lane is active.
- Arrow keys move selection, Enter confirms, Escape cancels, and all states
  remain usable in narrow terminals.
- Provider/model remain status-line, settings, or slash-command choices; they
  are not additional new-Lane steps.

The primary TUI path is:

`new Lane -> type task`.

The delegation path is:

`/acp -> select agent -> type delegated task`.

## Error And Recovery Semantics

- Non-Git or missing-HEAD workspace: block before preview and offer a direct
  explanation.
- Branch/worktree collision: preserve the creation entry and offer a regenerated
  default or explicit retry.
- Provider unavailable or unauthenticated: keep the Lane and show native-agent
  start failure with a settings action.
- ACP missing: classify as install required.
- ACP logged out: classify as authentication required and show agent-owned login
  guidance.
- ACP probe or session failure: retain the child session record and allow retry
  or cancellation.
- ACP follow-up unsupported: keep the completed result visible and require a new
  delegated session; never pretend a new process is the same session.
- Native or ACP tool approval timeout/deny: preserve the conversation and show
  the exact denied/expired action and permitted next step.
- Native context exhaustion: keep the Lane and transcript, expose Core-owned
  compact/switch-model/retry actions, and do not resend automatically.
- Event gap or reconnect: recover through snapshot/replay before enabling more
  mutations.

Errors must be rendered from typed Core facts. Display strings must not be
parsed to infer success, ownership, authentication, or retryability.

## Contract And Ownership Impact

Core owns:

- Git workspace eligibility and generated Lane defaults;
- Lane preview, approval, receipt, and primary-agent task lifecycle;
- active provider/model and provider health;
- ACP discovery, probe, startability, session lifecycle, cancellation, and
  evidence;
- snapshot/replay recovery and owner relationships.

TUI owns terminal command discovery, `/acp` selection presentation, drafts, and
focus. GUI owns the compact `+` menu, delegated-task presentation, drafts, and
focus. Neither frontend persists a second adapter registry, recent list,
provider choice, Lane record, or ACP session record.

## Verification

The shared fixture corpus must cover:

1. valid Git project -> Lane receipt -> native DeepSeek task;
2. valid Git project -> Lane receipt -> native OpenAI task;
3. non-Git and missing-HEAD preflight rejection;
4. Lane success followed by provider start failure;
5. Codex, Claude, and Kiro discovery/probe classifications;
6. custom ACP discovery from configuration;
7. ACP success, authentication-required, install-required, failure, and cancel;
8. simultaneous ACP children under one Lane;
9. event gap and snapshot/replay recovery;
10. native busy follow-up queue, scoped approval, exact-owner cancel, retry, and
    continued conversation;
11. ACP streamed progress, scoped approval, follow-up/resume, exact-session
    cancel, retry, and result/evidence recovery;
12. switching between native and multiple ACP conversations without owner
    leakage;
13. parity of Core facts consumed by TUI and GUI.

Tests must include real LocalCoreHost/runtime integration. Fixture-only tests
that inject impossible production readiness states are insufficient. Release
evidence includes a live DeepSeek native turn and at least one live ACP session;
OpenAI live execution is required only when credentials are explicitly
available for the release gate.

## Out Of Scope

- Multiple Viden-native primary agents inside one Lane.
- ACP sessions without an owning Lane.
- Provider or model selection inside the new-Lane menu.
- Mandatory custom branch/worktree/budget setup before creation.
- Frontend-owned agent registries, authentication state, or session persistence.

## Acceptance

The milestone is complete when a user can perform the native and ACP paths in
both TUI and GUI against the same Core build, and every visible success is backed
by an ordered Core receipt or lifecycle fact. Lane creation must remain visibly
successful when a subsequent native or ACP start fails.

Completion requires all rows below; entry-point-only demonstrations do not pass:

| Surface | Native agent | ACP agent |
| --- | --- | --- |
| Configure | DeepSeek/OpenAI health and model are visible | install/auth/capabilities are visible |
| Start | first Lane task starts through Core | delegated task starts under the exact Lane |
| Converse | stream, queue, follow-up, and continue | stream and supported follow-up/resume |
| Control | approve/deny, cancel turn, retry | approve/deny, cancel exact session, retry attempt |
| Observe | transcript, tools, usage, status | source-tagged transcript, progress, status |
| Finish | terminal fact and evidence | terminal fact, result, and evidence |
| Recover | snapshot/replay/transcript restore | session list and result restore |
