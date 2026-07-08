# Code Agent HN 需求雷达 - 2026-05-28

English version: [code-agent-hn-demand-radar-2026-05-28.md](code-agent-hn-demand-radar-2026-05-28.md)

## 范围

这份文档用 Hacker News 讨论刷新 Viden 的竞品认知。它不是热度榜，而是需求雷达：
开发者在抱怨什么、认可什么，以及 Viden 还缺什么。

来源：

- HN: Ask HN: Why are AI coding agents not working for me?
- HN: Ask HN: How Do You Actually Use Claude Code Effectively?
- HN: Ask HN: Senior software engineers, how do you use Claude Code?
- HN: Show HN: Real-time dashboard for Claude Code agent teams
- HN: Parallel agents in Zed
- HN: Emacs agent-shell (powered by ACP)
- HN: Tell HN: Anthropic no longer allowing Claude Code subscriptions to use
  OpenClaw
- OpenAI Codex CLI docs
- Claude Code user FAQ
- Zed external agents docs
- Kiro docs
- Kilo product docs

## HN 需求主题

### 1. Context 管理是核心 UX，不是内部细节

HN 用户反复提到：context 变大后，agent 质量会下降。高阶用户会主动 summarize、
commit、compact、clear，并把任务拆小。

Viden 含义：

- ContextBundle 必须成为可见 operator surface，而不只是 provider plumbing。
- 长任务开始前，产品应该显示 included sources、omitted sources、estimated tokens
  和 compaction decisions。
- 下一版应该把 ContextBundle v1 做成带 source priority 和 reason codes 的 budget
  policy。

缺口：

- `0.1.13` 已显示 pressure 并注入 context，但用户还不能主动 curate、pin、omit 或
  split context。

### 2. 成熟工作流是 Plan -> Spec -> Execute -> Review

有经验的 Claude Code 用户通常先和 agent 规划、记录决策、拆分任务，再小步实现。
Kiro 把这个做成 steering files 和 specs：requirements、design、tasks。

Viden 含义：

- Viden 应该增加轻量 spec/steering loop，再做更强自治。
- Task envelope 应该包含 requirements、constraints、design decisions、expected tests
  和 acceptance criteria。
- 后续 `/spec` 或 `/plan task` 应该产出 lane 可消费的文件。

缺口：

- Viden 有 tasks、memory 和 release plans，但还没有产品内 spec-driven workflow。

### 3. 多 agent 的痛点是可见性，不只是能 spawn

HN 对 agent dashboard 和 parallel agents 的反馈很清晰：用户需要看到每个 agent
实际做了什么，而不是被整理过的乐观 summary。另一个难点是质量：agent 可能运行正常，
但产出很差。

Viden 含义：

- Side screens 应该展示事件时间线：prompt、tool call、file change、test command、
  approval、failure、retry、final evidence。
- reviewer/tester 应该是独立 evidence lane，而不是隐藏 subcall。
- side-2 要回答“我为什么能相信这个结果？”

缺口：

- 当前 lane evidence 已存在，但 operator timeline 还太粗。需要 per-lane audit replay。

### 4. Parallel agents 需要的不只是 Git worktrees

Zed parallel-agent 讨论里有个实际 blocker：git worktree 不够，测试会共享数据库、
migration state、cache 或服务。用户也需要 cleanup hooks，避免 worktree 和测试环境堆积。

Viden 含义：

- lane 需要声明的不只是 `worktree`，还包括 test data scope、service ports、env vars、
  cache dirs、database schema 和 cleanup command。
- 增加 lane preflight 和 teardown hooks。
- 启动 parallel lane 前展示 isolation risk。

缺口：

- Viden 有 per-lane worktree 方向和 review/apply safety，但还没有结构化 test-data
  或 service isolation 模型。

### 5. ACP 和复用原生配置是真需求

HN 对 ACP 的讨论把它类比成“agents 的 LSP”：用户不想每个 editor/tool 都重新实现
Claude、Codex、Gemini、Aider、Goose 和 custom wrapper。另一个痛点是 MCP、credentials、
project/user config 已经很碎。

Viden 含义：

- Codex/Claude happy path 稳定后，ACP 应该成为认真推进的 adapter boundary。
- Adapter doctor 应该说明哪些配置属于 Viden，哪些属于 agent-native。
- 不要无理由复制 secret 或重复 MCP config。

缺口：

- Viden 目前只规划 ACP probes，还没有具体兼容目标，例如“跑一个 ACP server 并把
  events 映射进 lane evidence”。

### 6. 成本、rate limits、provider economics 是产品需求

HN 关于 Claude subscription、third-party harness、Cursor billing 和 OpenClaw 的讨论说明：
用户关心 cost transparency、rate-limit 行为，以及自动 agent 会不会隐形烧配额。

Viden 含义：

- cockpit 应该按 provider 和 lane 展示 token/cost/rate budget。
- 长循环要有上限：max turns、max tokens、max cost、max wall-clock time。
- agent 继续昂贵步骤前应解释原因。

缺口：

- Viden 有 context pressure 和 provider health，但没有 cost ledger、quota forecast
  或 per-lane budget stop condition。

### 7. Credentials 和 agent tool access 是信任边界

HN 对 credential proxy 的讨论反映出一个强担忧：agent 需要使用工具，但不应该看到或泄露
secret。Claude Code 和 Kiro 也都强调 MCP、hooks、privacy/security surface。

Viden 含义：

- API keys 不能进入 transcript、screenshot 或 model context。
- 未来 MCP/plugin calls 需要 credential brokering 或 least-privilege capability
  boundaries。
- Permission prompt 应区分“agent 使用某 capability”和“agent 看见 secret”。

缺口：

- Viden setup 已避免保存 secret，但还没有 MCP、external API 或 agent adapters 的
  credential broker / proxy pattern。

### 8. Hooks 有用，但必须可观察、可阻断

Claude Code 和 Kiro 都提供 hooks。HN 用户认可 hooks 用于 notification、lint/test
automation 和 hard-block unsafe actions，但也抱怨 DIY hook 行为难 debug。

Viden 含义：

- Hooks 应该 typed、logged、testable，并显示在 side-2。
- PreToolUse-style hooks 应能带结构化 reason 阻断。
- Hook output 应进入 evidence rows，而不是隐藏 shell noise。

缺口：

- Viden 有 extension boundary 规划，但还没有 hook lifecycle 或 hook evidence model。

## 竞品差距矩阵

| 竞品 / pattern | 强信号 | Viden 缺口 | 产品回应 |
| --- | --- | --- | --- |
| Claude Code | 成熟 terminal loop、MCP、hooks、skills、subagents、checkpoints、non-interactive mode | Viden 有更强 cockpit 目标，但内建 automation surface 更弱 | 增加 hook lifecycle、spec/steering 和可复现 Claude lane |
| Codex | 本地 Rust CLI、强安装体验、跨 surface 方向、evidence 预期 | Viden 暂时不应替代 Codex | 把 Codex 做成 reference delegated lane backend |
| Zed | ACP external agents、editor-native threads、worktree parallelism | Viden 缺 ACP runtime 和 editor-native file context | P0 lane 稳定后做 ACP probe/event mapping；TUI 继续做 ops cockpit |
| Kiro | Specs、steering files、hooks、MCP、privacy-first framing | Viden 缺产品内 spec/steering workflow | 增加 task envelope spec phases 和 project steering files |
| Kilo / OpenClaw | 多 surface、很多模型、cloud/slack/automations | Viden 目前 TUI-first、local-first | 不急追 cloud；先做 cost/rate ledger 和 automation boundary |
| Aider | Git-native 简洁体验和 repo map | Viden context 仍可能过于 transcript-centric | 把 compact repo/project map 接入 ContextBundle |
| OpenHands / Goose | Platform/SDK 形态，CLI + GUI + API | Viden runtime 仍主要 TUI-led | 保持 core 可复用，但 API/server 等 TUI 稳定后再做 |
| DeepSeek-TUI | dense terminal-native provider experience | Viden 编排目标更强，但 terminal UX 仍需持续打磨 | 继续把确定性截图和 live provider smoke 作为 release gate |

## 对 0.1.14 的优先级修正

保持当前 `0.1.14` 方向，但加紧：

1. 新增 `P0-HN`: lane event timeline / audit replay。
   - 显示 prompt、tool call、command、file change、approval、test、failure、retry、
     final output 的时间线。
2. 新增 `P0-HN`: isolation preflight。
   - worktree 加 test DB/schema/cache/service-port declarations 和 cleanup。
3. 把 cost/rate budget 从 P2 提到 P1。
   - per-lane token/cost/time ceilings 和 visible burn rate。
4. 新增 `P1`: lightweight steering/spec files。
   - project conventions 和 requirements/design/tasks envelope。
5. 新增 `P1`: hook probe design。
   - pre/post tool hooks、blocking hooks、notifications 和 hook evidence。
6. 新增 `P1`: credential boundary design。
   - MCP/plugin/agent context 中只出现 secret handles，不出现 secret values。

## 产品判断

HN 信号说明，下一阶段的切入点不是“更多自治 agent”，而是：

> 让多 agent 编程变得可观察、有边界、可 review、成本可预测。

Viden 的 TUI-first 策略仍然成立，但前提是副屏真正成为 evidence 和 control surface，
而不是 dashboard。

