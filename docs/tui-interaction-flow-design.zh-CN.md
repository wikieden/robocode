# TUI 交互流程设计

English version: [tui-interaction-flow-design.md](tui-interaction-flow-design.md)

最后更新：2026-06-09

## 目的

本文定义 RoboCode 0.1.x 稳定线的目标 TUI 交互流程。目标是做成一个 coding
cockpit，让用户随时能回答：

- 我现在在哪？
- RoboCode 正在做什么？
- 我还能不能继续输入？
- 什么事情被阻塞了？
- 下一步应该做什么？

TUI 必须是共享 runtime state 上的一层产品界面。它不能拥有隐藏业务逻辑、不能启动嵌套
input loop，也不能编造 core runtime 无法解释的状态。

## 产品规则

- Composer 必须始终可恢复。Provider turn、Plan 模式、approval、setup panel、side
  screen 或 error 都不能永久锁住输入。
- 每个 active operation 都必须表示成状态：`pending_turn`、`runtime_task`、
  `interaction_panel`、`streaming_assistant`、`queued_input` 或 `error_recovery`。
- Active work 显示为最近可见 transcript 内容下面的 `LIVE WORK` strip，包含 phase、
  signal 和 next action。
- Provider 名是基础设施细节。主状态应该说 `RoboCode`、内部角色或具体 operation。
- Provider thinking 不显示假进度百分比。只有 lane/tool 真实进度才允许显示 percent。
- Permission 控制统一遵循 [Permission Mode 设计](permission-mode-design.zh-CN.md)，
  不再把这个 surface 标为 `Approval Mode`。
- Popup 和 panel 是直接操作界面，不是 command completion 页面。用户在 panel 里选择
  或编辑后应直接生效。
- Streaming 时 transcript history 必须仍可浏览。新输出只更新 badge，不把用户强行拉回
  底部。

## 顶层状态模型

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

## 单事件循环

所有输入、后台工作、重绘、approval、streaming、resize 和 panel action 都走同一个 TUI
event loop。Provider request、tool、lane、LSP 和 diagnostics 都只是事件源，不能接管终端输入。

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

## 输入路由

Input router 决定一次按键到底是编辑文本、导航 panel、处理 approval、滚动历史，还是控制
side screen。这能避免 Plan 模式和 provider turn 抢走键盘。

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

## Welcome 与配置流程

Welcome 不是会话。配置 provider/model 后不应该跳到 cockpit，除非用户开始真实任务或恢复历史。

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

## Provider Turn 与排队 Follow-Up

Active work 期间 composer 仍可编辑。按 Enter 会把草稿排队，而不是阻塞或启动嵌套请求。

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

## Approval 流程

Approval 是同一个事件循环里的 focus target，不能调用第二套阻塞式 input loop。

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

## Model 与 Provider 面板

Provider setup 和 model selection 都是直接操作面板，不应该隐藏在 command completion 语义后面。

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

## Transcript、Streaming 与 Scrollback

Streaming 要有实时感，但不能破坏历史浏览。

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

## 错误恢复流程

错误应该 inline、可操作、范围明确。除非需要用户立即操作，否则不要弹成巨大的居中阻塞警告。

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

## 渲染 View Model

Renderer 消费派生出来的 view model。它不应直接查询 runtime service，也不应在任务完成后把
旧 transcript facts 重新解释成 live work。

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

## 验收场景

- 无 session 启动：执行 `/connect`、`/models`、`/setup` 后仍停留 welcome；只有真实
  prompt 或 resume 才进入 cockpit。
- 启动 provider turn：`LIVE WORK` 出现在 transcript 最新内容下面；composer 仍可编辑；
  不显示假的 provider percent。
- Provider turn 期间输入：`Enter` 把 follow-up 入队并清空 composer。
- Plan mode turn：provider 只产出需求、架构、实现方案、测试策略和开发计划；mutating tools
  被 permission/runtime policy 拦截；composer 不锁死。
- Approval request：approval panel 处理 approve/deny/inspect，同时 resize、scroll 和
  typed follow-up 仍走主循环。
- Scrolled-up streaming：transcript badge 显示 new output；scrollback 不跳到底部。
- Provider failure：inline recovery 显示具体下一步；TUI 保持打开，composer 可用。
- `/connect`：provider 列表只显示供应商；选择后进入可编辑 key/endpoint/default-model flow。
- `/models`：只显示配置过的 provider 和 active/favorite model rows，按 provider 分组，不展示
  未配置 descriptor 噪声。
- Resize/focus/sleep：full redraw 恢复布局；composer 不出现 terminal protocol residue。

## 实现影响

- 保持一个 interaction router 和一个 event loop。
- 把 approval 提升成非阻塞 `InteractionPanel` 状态。
- 用 `TurnController` 风格 runtime 边界集中 active turn state。
- 所有 TUI 文案从稳定 view model 派生。
- Preview 和 regression tests 覆盖 welcome、active turn、queued input、approval、
  model/provider panels、streaming scrollback、error recovery 和 resize。
