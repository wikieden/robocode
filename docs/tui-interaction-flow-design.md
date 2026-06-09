# TUI Interaction Flow Design

Chinese version: [tui-interaction-flow-design.zh-CN.md](tui-interaction-flow-design.zh-CN.md)

Last updated: 2026-06-09

## Purpose

This document defines the target TUI interaction flow for RoboCode's 0.1.x
stability line. The goal is a coding cockpit where users can always answer:

- Where am I?
- What is RoboCode doing?
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
  `RoboCode`, an internal role, or a concrete operation.
- No fake progress percentages for provider thinking. Percent is allowed only
  when backed by real lane/tool progress.
- Permission controls use the unified
  [Permission Mode Design](permission-mode-design.md). Do not label this surface
  as `Approval Mode`.
- Popups and panels are direct manipulation surfaces, not command-completion
  pages. Selecting or editing inside the panel applies the change.
- Transcript history must remain browsable while streaming continues. New output
  marks the badge; it does not yank the user back to the bottom.

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
    ActiveTurn --> ApprovalPanel: permission request
    ApprovalPanel --> ActiveTurn: approve / deny / inspect
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
    F -->|approval| J["ApprovalAction<br/>approve/deny/inspect"]
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
    A["Input Event"] --> B{"Has modal approval?"}
    B -->|yes| C["Approval keymap<br/>y/n/d/tab/arrows/esc"]
    B -->|no| D{"Interaction panel open?"}
    D -->|yes| E["Panel keymap<br/>search/edit/select/save/cancel"]
    D -->|no| F{"Command palette open?"}
    F -->|yes| G["Palette keymap<br/>filter/up/down/tab/enter/esc"]
    F -->|no| H{"Transcript history focus?"}
    H -->|yes| I["History keymap<br/>page/wheel/ctrl-end"]
    H -->|no| J["Composer keymap"]

    J --> K{"Enter while active turn?"}
    K -->|yes| L["Queue follow-up<br/>clear composer"]
    K -->|no| M["Start new turn"]
    J --> N{"Slash command?"}
    N -->|yes| O["Open palette or run immediate command"]
    N -->|no| P["Edit input"]
```

## Welcome And Configuration Flow

Welcome is not a session. Configuration should not jump into the cockpit unless
the user starts real work or resumes history.

```mermaid
flowchart TD
    A["Launch RoboCode"] --> B{"Has visible session content?"}
    B -->|no| C["Welcome surface<br/>logo + composer + context row"]
    B -->|yes| D["Cockpit"]

    C --> E{"User action"}
    E -->|/connect| F["Provider picker"]
    E -->|/models| G["Configured model picker"]
    E -->|/setup| H["Setup checklist"]
    E -->|real prompt| I["Start session"]

    F --> J["Provider setup form<br/>auth/key/endpoint/default model"]
    J --> K{"Save?"}
    K -->|save| C
    K -->|cancel/esc| C

    G --> L["Switch active provider/model"]
    L --> C
    H --> C
    I --> D
```

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
blocking input loop.

```mermaid
flowchart TD
    A["Runtime requests mutation"] --> B["Permission layer builds ApprovalRequest"]
    B --> C["TurnController emits PendingApproval"]
    C --> D["TUI renders approval panel"]
    D --> E{"User action"}
    E -->|approve| F["resolve_approval(approve)"]
    E -->|deny| G["resolve_approval(deny)"]
    E -->|inspect diff| H["Focus evidence/diff"]
    E -->|scroll/resize/type| I["Still handled by main loop"]
    H --> D
    I --> D
    F --> J["Runtime continues"]
    G --> K["Runtime records denial"]
    J --> L["LIVE WORK updates"]
    K --> L
```

## Model And Provider Panels

Provider setup and model selection are direct panels. They should not be hidden
behind command-completion semantics.

```mermaid
flowchart TD
    A["/connect"] --> B["Provider picker<br/>providers only"]
    B --> C["Provider setup form"]
    C --> D{"Auth mode"}
    D -->|API key| E["Edit/delete masked key"]
    D -->|web login| F["Open login / confirm token"]
    D -->|local/no key| G["Show local status"]
    C --> H["Endpoint edit"]
    C --> I["Default model picker"]
    I --> J["Provider-scoped model list"]
    J --> K["Save provider config"]
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
- Approval request: approval panel handles approve/deny/inspect, while resize,
  scroll, and typed follow-up remain routed through the main loop.
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
- Promote approval into non-blocking `InteractionPanel` state.
- Centralize active turn state in a `TurnController`-style runtime boundary.
- Derive all TUI text from a stable view model.
- Make preview and regression tests cover welcome, active turn, queued input,
  approval, model/provider panels, streaming scrollback, error recovery, and
  resize.
