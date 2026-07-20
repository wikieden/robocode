# TUI 交互流程设计

English version: [tui-interaction-flow-design.md](tui-interaction-flow-design.md)

最后更新：2026-07-20

## 目的

本文定义 Viden 0.1.x 稳定线的目标 TUI 交互流程。目标是做成一个 coding
cockpit，让用户随时能回答：

- 我现在在哪？
- Viden 正在做什么？
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
- Provider 名是基础设施细节。主状态应该说 `Viden`、内部角色或具体 operation。
- Provider thinking 不显示假进度百分比。只有 lane/tool 真实进度才允许显示 percent。
- Permission 控制统一遵循 [Permission Mode 设计](permission-mode-design.zh-CN.md)，
  不再把这个 surface 标为 `Approval Mode`。
- Popup 和 panel 是直接操作界面，不是 command completion 页面。用户在 panel 里选择
  或编辑后应直接生效。
- Streaming 时 transcript history 必须仍可浏览。新输出只更新 badge，不把用户强行拉回
  底部。
- 输入包含三个明确模式：Normal 负责 cockpit 导航，Insert 负责 composer 编辑，Overlay
  负责 selector、panel 与审批。`Esc` 每次只退出一层。只有拿到精确 Core owner 时，
  `Ctrl-C` 才发送 owner-scoped cancellation；只有 lane ID 的场景必须显示 cancel
  unavailable，并提出缺失的 Core contract request。
- `Ctrl-P` 打开 selector-first 全局跳转。它只投影 Core 的 typed lane、其
  `active_session_ids`、merge gate、pending approval，以及受控的导航/补全命令注册表。
  `:`、`@`、`#`、`>`、`~` 分别限定 lane、会话、闸/问询、命令和文件；方向键或
  `j`/`k` 移动，Enter 选择，Esc 精确恢复之前的 overlay 所有权与 composer 上下文。
  Core 目前没有 typed 文件清单能力，所以 File 会保留可见但禁用，并显示具体原因；
  TUI 绝不扫描文件系统或 Git。

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
    ActiveTurn --> ActiveTurn: permission request pinned
    ActiveTurn --> Decisions: ctrl-g
    Decisions --> ApprovalFocus: select concrete request
    Decisions --> ActiveTurn: esc
    ApprovalFocus --> ActiveTurn: y approve once / n deny / esc close（仍 pinned）
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

## 输入路由

Input router 决定一次按键到底是编辑文本、导航 panel、处理 approval、滚动历史，还是控制
side screen。这能避免 Plan 模式和 provider turn 抢走键盘。

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
    Q -->|yes| R["打开 Decisions<br/>选择具体 request"]
    J --> K{"Enter while active turn?"}
    K -->|yes| L["Queue follow-up<br/>clear composer"]
    K -->|no| M["Start new turn"]
    J --> N{"Slash command?"}
    N -->|yes| O["Open palette or run immediate command"]
    N -->|no| P["Edit input"]
```

## Welcome 与配置流程

Welcome 不是会话。TUI 0.3.0 会先协商 Core 0.3.1 onboarding capability 并请求
`ProbeProject`，但在操作者打开 Setup/Lanes 或开始真实任务之前仍停留在 Welcome。
event cursor 不是 session id。

```mermaid
flowchart TD
    A["Launch Viden"] --> B["协商 Core capabilities<br/>ProbeProject"]
    B --> C{"已选择 Core lane/session<br/>或提交真实任务?"}
    C -->|no| D["Welcome surface<br/>logo + composer + context row"]
    C -->|yes| E["Unified cockpit"]

    D --> F{"User action"}
    F -->|/setup| G["Setup selector<br/>编辑 D11 draft · PreviewProjectConfig"]
    F -->|/lanes| H["Core lane board"]
    F -->|real prompt| E

    G --> I{"Core event?"}
    I -->|ProjectConfigPreviewed| G
    I -->|ProjectConfigConfirmed| J["Setup complete"]
    J --> D
    H --> K["选择 lane"]
    K --> L["多个 session 时选择 Core active_session_id"]
    L --> E
```

TUI 只投影 Core onboarding facts，并且只通过 `CoreClient` 发送 typed command。
它不会自行扫描项目、写入或确认配置，也不接收原始 credential bytes。它可以用 immutable
preview id/hash 请求 Core 确认，但 Setup 只从 `ProjectConfigConfirmed` 得出完成状态；
command receipt 不代表成功。

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

Approval 是同一个事件循环里的 focus target，不能调用第二套阻塞式 input loop。Pending
approval 只保持 pinned，不接管输入。操作者用 `Ctrl-G` 打开 Decisions 并选中一条具体
request 前，composer 中的 `y`、`n`、`d`、`Enter` 都是普通草稿/提交输入。

```mermaid
flowchart TD
    A["Runtime requests mutation"] --> B["Permission layer builds ApprovalRequest"]
    B --> C["TurnController emits PendingApproval"]
    C --> D["Pin request<br/>composer 保持输入权"]
    D --> E["Ctrl-G 打开 Decisions"]
    E --> F["选择具体 request"]
    F --> G["显式 approval focus"]
    G -->|1 / y| H["转发 allow-once scope"]
    G -->|2| H2["转发精确 session scope"]
    G -->|3| H3["转发精确 repository allowlist"]
    G -->|4 / n| I["转发 deny"]
    G -->|d| J["打开 request diff/evidence"]
    G -->|方向键| K["移动选中 action"]
    K -->|Enter| L["执行选中 action"]
    G -->|Esc| M["关闭 focus<br/>request 继续 pinned"]
    J --> G
    H --> N["Runtime continues"]
    H2 --> N
    H3 --> N
    I --> O["Runtime records denial"]
    L --> P["派发选中的当前 action"]
    N --> Q["LIVE WORK updates"]
    O --> Q
    P --> Q
```

Session 级允许和 repository allowlist 是按 request 开放的 Core action。只有该 request
的 typed `allowed_scopes` 提供精确 payload 时，TUI 才暴露对应 choice，并通过原 request
owner 原样转发后等待 Core resolution。过期 request 继续 pinned 且不可操作，直到
`ApprovalResolved`；TUI 绝不本地合成拒绝或成功。

## Model 与 Provider 面板

Provider setup 和 model selection 都是直接操作面板，不应该隐藏在 command completion 语义后面。

```mermaid
flowchart TD
    A["/connect"] --> B["Provider picker<br/>providers only"]
    B --> C["Core provider health"]
    C --> D{"存在安全 credential handle?"}
    D -->|yes| E["显示脱敏 handle 元数据"]
    D -->|no| F["Trusted ingress unavailable<br/>只读"]
    C --> I["Configured model picker"]
    I --> J["Provider-scoped model list"]
    J --> K["发送 Core-owned provider/model action"]
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
- Approval request：request 保持 pinned 且不接管 composer；`Ctrl-G` 打开 Decisions，选中
  具体 request 后进入显式 focus，`y` 仅本次允许、`n` 拒绝、`d` 打开 diff/evidence、
  `Enter` 执行选中 action，`Esc` 只关闭 focus。
- Scrolled-up streaming：transcript badge 显示 new output；scrollback 不跳到底部。
- Provider failure：inline recovery 显示具体下一步；TUI 保持打开，composer 可用。
- `/connect`：provider 列表只显示供应商；选择后进入可编辑 key/endpoint/default-model flow。
- `/models`：只显示配置过的 provider 和 active/favorite model rows，按 provider 分组，不展示
  未配置 descriptor 噪声。
- Resize/focus/sleep：full redraw 恢复布局；composer 不出现 terminal protocol residue。

## 实现影响

- 保持一个 interaction router 和一个 event loop。
- Pending approval 保持为 pinned runtime fact；只有 Decisions 选中具体 request 后才创建
  非阻塞 `OverlayKind::Approval` 状态。
- 用 `TurnController` 风格 runtime 边界集中 active turn state。
- 所有 TUI 文案从稳定 view model 派生。
- Preview 和 regression tests 覆盖 welcome、active turn、queued input、approval、
  model/provider panels、streaming scrollback、error recovery 和 resize。
