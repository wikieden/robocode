# TUI Interaction Flow Design

Chinese version: [tui-interaction-flow-design.zh-CN.md](tui-interaction-flow-design.zh-CN.md)

Last updated: 2026-07-20

## Purpose

This document defines the target TUI interaction flow for Viden's 0.1.x
stability line. The goal is a coding cockpit where users can always answer:

- Where am I?
- What is Viden doing?
- Can I still type?
- What is blocked?
- What should I do next?

The TUI must be one product surface over shared runtime state. It must not own
hidden business logic, start nested input loops, or invent status that the core
runtime cannot explain.

## Product Rules

- The composer is always recoverable. A provider turn, Plan mode, approval,
  setup panel, side screen, or error must not permanently lock input.
- Every active operation is represented as state: `pending_turn`,
  `runtime_task`, `interaction_panel`, `streaming_assistant`, `queued_input`, or
  `error_recovery`.
- Active work appears as a `LIVE WORK` strip under the latest visible
  transcript content. It shows phase, signal, and next action.
- Provider names are infrastructure details. Main status should say
  `Viden`, an internal role, or a concrete operation.
- No fake progress percentages for provider thinking. Percent is allowed only
  when backed by real lane/tool progress.
- Permission controls use the unified
  [Permission Mode Design](permission-mode-design.md). Do not label this surface
  as `Approval Mode`.
- Popups and panels are direct manipulation surfaces, not command-completion
  pages. Selecting or editing inside the panel applies the change.
- Transcript history must remain browsable while streaming continues. New output
  marks the badge; it does not yank the user back to the bottom.
- Input has three explicit modes: Normal for cockpit navigation, Insert for
  composer editing, and Overlay for selectors, panels, and approvals. `Esc`
  unwinds one layer at a time. `Ctrl-C` sends owner-scoped cancellation only
  when the exact Core owner is available; lanes that expose only an ID render
  cancellation as unavailable and request the missing Core contract.
- `Ctrl-P` opens selector-first Global Jump. It projects only typed Core lanes,
  their active session IDs, merge gates, pending approvals, and a controlled
  navigation/completion command registry. `:`, `@`, `#`, `>`, and `~` scope
  lane, session, gate/ask, command, and file results; arrows or `j`/`k` move,
  Enter selects, and Esc restores the prior overlay owner and composer context.
  Core does not yet expose a typed file inventory, so File remains visible but
  disabled with that concrete reason; the TUI never scans the filesystem or Git.

## Top-Level State Model

```mermaid
stateDiagram-v2
    [*] --> Welcome
    Welcome --> WelcomeConfig: /connect /setup /models /settings
    WelcomeConfig --> Welcome: save/cancel provider or model config
    Welcome --> Cockpit: submit real prompt / resume session

    Cockpit --> CommandPalette: type /
    CommandPalette --> Cockpit: enter command / esc

    Cockpit --> InteractionPanel: /connect /models /provider /settings
    InteractionPanel --> Cockpit: apply / save / cancel / esc

    Cockpit --> ActiveTurn: submit prompt
    ActiveTurn --> ActiveTurn: stream delta / tool event / queued follow-up
    ActiveTurn --> ActiveTurn: permission request pinned
    ActiveTurn --> Decisions: ctrl-g
    Decisions --> ApprovalFocus: select concrete request
    Decisions --> ActiveTurn: esc
    ApprovalFocus --> ActiveTurn: y approve once / n deny / esc close (still pinned)
    ApprovalFocus --> EvidenceFocus: d diff/evidence
    EvidenceFocus --> ApprovalFocus: close / return
    ActiveTurn --> ErrorRecovery: provider/tool failure
    ErrorRecovery --> Cockpit: retry / switch model / run doctor / continue
    ActiveTurn --> Cockpit: assistant result / cancelled

    Cockpit --> HistoryBrowse: page up / wheel / ctrl-home
    HistoryBrowse --> Cockpit: ctrl-end / scroll to live tail

    Cockpit --> SideScreen: open lane/evidence/ops screen
    SideScreen --> Cockpit: close / focus main
```

## Single Event Loop

All input, background work, repaint, approval, streaming, resize, and panel
actions go through one TUI event loop. Provider requests, tools, lanes, LSP, and
diagnostics are event sources. They do not own terminal input.

```mermaid
flowchart TD
    A["Main TUI Event Loop"] --> B{"Next event"}
    B -->|keyboard/mouse/paste/resize| C["Interaction Router"]
    B -->|provider/tool/lane event| D["Runtime Event Drain"]
    B -->|timer tick| E["Animation/Repaint Tick"]

    C --> F{"Focus target"}
    F -->|composer| G["ComposerAction<br/>edit/submit/queue/cancel"]
    F -->|palette| H["PaletteAction<br/>filter/select/close"]
    F -->|panel| I["PanelAction<br/>edit/select/apply/cancel"]
    F -->|explicit approval focus| J["ApprovalAction<br/>approve once/deny/diff/close"]
    F -->|transcript| K["HistoryAction<br/>scroll/follow live"]
    F -->|side screen| L["SideAction<br/>focus/select/close"]

    G --> M["Dispatch Runtime Command"]
    H --> M
    I --> M
    J --> M
    K --> N["Update View State"]
    L --> N
    D --> O["Update Runtime Snapshot"]
    E --> P["Mark Dirty Regions"]

    M --> O
    O --> Q["Derive View Model"]
    N --> Q
    P --> Q
    Q --> R["Render Frame"]
    R --> A
```

## Input Routing

The input router decides whether a key edits text, navigates a panel, resolves
approval, scrolls history, or controls a side screen. This prevents Plan mode
and provider turns from stealing the keyboard.

```mermaid
flowchart TD
    A["Input Event"] --> B{"Has explicit approval focus?"}
    B -->|yes| C["Approval keymap<br/>y/n/d/arrows/enter/esc"]
    B -->|no| D{"Interaction panel open?"}
    D -->|yes| E["Panel keymap<br/>search/edit/select/save/cancel"]
    D -->|no| F{"Command palette open?"}
    F -->|yes| G["Palette keymap<br/>filter/up/down/tab/enter/esc"]
    F -->|no| H{"Transcript history focus?"}
    H -->|yes| I["History keymap<br/>page/wheel/ctrl-end"]
    H -->|no| J["Composer keymap"]

    J --> Q{"Ctrl-G?"}
    Q -->|yes| R["Open Decisions<br/>select concrete request"]
    J --> K{"Enter while active turn?"}
    K -->|yes| L["Queue follow-up<br/>clear composer"]
    K -->|no| M["Start new turn"]
    J --> N{"Slash command?"}
    N -->|yes| O["Open palette or run immediate command"]
    N -->|no| P["Edit input"]
```

## Welcome And Configuration Flow

Welcome is not a session. TUI 0.3.0 negotiates the Core 0.3.1 onboarding
capabilities and requests `ProbeProject`, but remains on Welcome until the
operator opens Setup/Lanes or starts real work. The event cursor is not a
session id.

```mermaid
flowchart TD
    A["Launch Viden"] --> B["Negotiate Core capabilities<br/>ProbeProject"]
    B --> C{"Selected Core lane/session<br/>or real prompt?"}
    C -->|no| D["Welcome surface<br/>logo + composer + context row"]
    C -->|yes| E["Unified cockpit"]

    D --> F{"User action"}
    F -->|/setup| G["Setup selector<br/>edit D11 draft · PreviewProjectConfig"]
    F -->|/lanes| H["Core lane board"]
    F -->|real prompt| E

    G --> I{"Core event?"}
    I -->|ProjectConfigPreviewed| G
    I -->|ProjectConfigConfirmed| J["Setup complete"]
    J --> D
    H --> K["Select lane"]
    K --> L["Select Core active_session_id<br/>when multiple exist"]
    L --> E
```

The TUI only projects Core onboarding facts and sends typed commands through
`CoreClient`. It does not scan the project, write or confirm configuration by
itself, or accept raw credential bytes locally. It may request confirmation by
immutable preview id/hash, but completion is derived only from
`ProjectConfigConfirmed`; a command receipt is not success.

## Provider Turn And Queued Follow-Up

During active work, the composer remains editable. Enter queues the draft rather
than blocking or starting a nested request.

```mermaid
sequenceDiagram
    actor User
    participant UI as TUI Event Loop
    participant TC as Turn Controller
    participant RT as Runtime/SessionEngine
    participant P as Provider

    User->>UI: submit prompt
    UI->>TC: start_turn(prompt)
    TC->>RT: run provider turn
    RT->>P: request stream
    UI-->>User: render LIVE WORK strip

    loop while provider turn active
        P-->>RT: stream/tool/usage/error events
        RT-->>TC: normalized runtime events
        TC-->>UI: snapshot update
        UI-->>User: repaint transcript + LIVE WORK
        User->>UI: type next instruction
        User->>UI: Enter
        UI->>TC: queue_followup(draft)
        UI-->>User: clear composer, show queued count
    end

    P-->>RT: final result
    RT-->>TC: turn_completed
    TC-->>UI: append canonical transcript
    UI->>TC: start next queued prompt if present
```

## Approval Flow

Approval is a focus target in the same event loop. It must not call a separate
blocking input loop. Pending approvals are pinned but do not own input.
Composer `y`, `n`, `d`, and `Enter` remain ordinary draft/submission input until
the operator opens Decisions with `Ctrl-G` and selects a concrete request.

```mermaid
flowchart TD
    A["Runtime requests mutation"] --> B["Permission layer builds ApprovalRequest"]
    B --> C["TurnController emits PendingApproval"]
    C --> D["Pin request<br/>composer retains input"]
    D --> E["Ctrl-G opens Decisions"]
    E --> F["Select concrete request"]
    F --> G["Explicit approval focus"]
    G -->|1 / y| H["forward allow-once scope"]
    G -->|2| H2["forward exact session scope"]
    G -->|3| H3["forward exact repository allowlist"]
    G -->|4 / n| I["forward deny"]
    G -->|d| J["Focus request diff/evidence"]
    G -->|arrows| K["Move selected action"]
    K -->|Enter| L["Activate selected action"]
    G -->|Esc| M["Close focus<br/>request stays pinned"]
    J --> G
    H --> N["Runtime continues"]
    H2 --> N
    H3 --> N
    I --> O["Runtime records denial"]
    L --> P["Dispatch selected current action"]
    N --> Q["LIVE WORK updates"]
    O --> Q
    P --> Q
```

Session-scoped approval and repository allowlisting are request-gated Core
actions. The TUI exposes a choice only when that request's typed
`allowed_scopes` provides the exact payload, forwards it through the original
request owner, and waits for Core resolution. Expired requests remain pinned
and inert until `ApprovalResolved`; the TUI never synthesizes local denial or
success.

## Model And Provider Panels

Provider setup and model selection are direct panels. They should not be hidden
behind command-completion semantics.

```mermaid
flowchart TD
    A["/connect"] --> B["Provider picker<br/>providers only"]
    B --> C["Core provider health"]
    C --> D{"Safe credential handle?"}
    D -->|yes| E["Show masked handle metadata"]
    D -->|no| F["Trusted ingress unavailable<br/>read-only"]
    C --> I["Configured model picker"]
    I --> J["Provider-scoped model list"]
    J --> K["Send Core-owned provider/model action"]
    K --> L["Return to previous surface"]

    M["/models"] --> N["Configured providers only"]
    N --> O["Favorites"]
    N --> P["Recent"]
    N --> Q["Provider groups"]
    Q --> R["Indented model rows"]
    R --> S["Enter switches active model"]
    R --> T["Favorite toggles without duplicates"]
```

## Transcript, Streaming, And Scrollback

Streaming should feel live without destroying history review.

```mermaid
flowchart TD
    A["Provider stream event"] --> B{"User following live tail?"}
    B -->|yes| C["Append temporary assistant row"]
    C --> D["Auto-follow latest row"]
    B -->|no| E["Append temporary assistant row"]
    E --> F["Keep scroll offset stable"]
    F --> G["Badge: history N · new output"]
    D --> H["Final provider result"]
    G --> H
    H --> I["Replace temp row with canonical transcript event"]
    I --> J{"Queued follow-up?"}
    J -->|yes| K["Start next turn"]
    J -->|no| L["Idle cockpit"]
```

## Error Recovery Flow

Errors should be inline, actionable, and scoped. They should not appear as
large blocking center warnings unless user action is required.

```mermaid
flowchart TD
    A["Runtime failure"] --> B{"Failure type"}
    B -->|provider 413/context| C["Context recovery<br/>compact/split/retry"]
    B -->|provider auth| D["Open /connect provider"]
    B -->|rate limit| E["Show wait/model alternatives"]
    B -->|tool failure| F["Show tool result + rerun/inspect"]
    B -->|permission denied| G["Show denied action + edit task"]
    B -->|render/input bug| H["Keep TUI open + record recovery hint"]

    C --> I["Inline recovery row"]
    D --> I
    E --> I
    F --> I
    G --> I
    H --> I
    I --> J["Composer remains editable"]
    J --> K{"User chooses next action"}
    K -->|retry| L["Start corrected turn"]
    K -->|configure| M["Open panel"]
    K -->|continue| N["Return to cockpit"]
```

## Render View Model

Rendering consumes a derived view model. It should not query runtime services
directly or reinterpret old transcript facts as live work after completion.

```mermaid
flowchart LR
    A["Session transcript"] --> E["ViewModel Deriver"]
    B["TurnController snapshot"] --> E
    C["Workflow/lane/task state"] --> E
    D["Provider/LSP telemetry"] --> E

    E --> F["Top bar model"]
    E --> G["Transcript rows"]
    E --> H["LIVE WORK strip"]
    E --> I["Right rail"]
    E --> J["Composer state"]
    E --> K["Panels/modals"]
    E --> L["Bottom bar"]

    F --> M["Frame renderer"]
    G --> M
    H --> M
    I --> M
    J --> M
    K --> M
    L --> M
```

## Acceptance Scenarios

- Launch with no session: welcome stays visible after `/connect`, `/models`, and
  `/setup`; it enters cockpit only after a real prompt or resume.
- Start provider turn: `LIVE WORK` appears under latest transcript content;
  composer remains editable; no fake provider percent appears.
- Type during provider turn: `Enter` queues a follow-up and clears composer.
- Plan mode turn: the provider only produces requirements, architecture,
  implementation approach, test strategy, and development plans; mutating tools
  are blocked by permission/runtime policy; composer input continues to work.
- Approval request: the request stays pinned without owning composer input;
  `Ctrl-G` opens Decisions, selecting a concrete request creates explicit focus,
  `y` approves once, `n` denies, `d` opens diff/evidence, `Enter` activates the
  selected action, and `Esc` only closes focus.
- Streaming while scrolled up: transcript badge shows new output; scrollback
  does not jump.
- Provider failure: inline recovery shows concrete next action; TUI remains
  open and composer usable.
- `/connect`: provider list shows providers only; selecting one opens editable
  key/endpoint/default-model flow.
- `/models`: shows configured providers and active/favorite model rows grouped
  by provider, with no unconfigured descriptor spam.
- Resize/focus/sleep: full redraw restores layout; no terminal protocol residue
  appears in the composer.

## Implementation Implications

- Keep one interaction router and one event loop.
- Keep pending approvals as pinned runtime facts and create non-blocking
  `OverlayKind::Approval` state only after a concrete Decisions selection.
- Centralize active turn state in a `TurnController`-style runtime boundary.
- Derive all TUI text from a stable view model.
- Make preview and regression tests cover welcome, active turn, queued input,
  approval, model/provider panels, streaming scrollback, error recovery, and
  resize.
