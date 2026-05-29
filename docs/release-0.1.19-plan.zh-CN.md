# RoboCode 0.1.19 计划 - Delegated Lane Usefulness

英文版： [release-0.1.19-plan.md](release-0.1.19-plan.md)

## 摘要

`0.1.19` 要把 delegated lane 做到真正有用。目标不是增加更多 agent 名字，
而是让一条委派审查闭环可靠：

> 让 RoboCode 委派一个聚焦的 review 或 shell/template 任务 -> 观察 lane
> 工作 -> 查看证据 -> accept、apply、discard、retry 或 cleanup，全程不用猜系统
> 到底处于什么状态。

`0.1.20` 仍是 usability beta gate。`0.1.19` 是这个 gate 前最后一个功能切片，
所以要优先完成少数完整闭环，而不是铺开很多半成品入口。

## 产品目标

到 `0.1.19` 结束，开发者应该可以把 RoboCode 当成一个 operator cockpit，
真实使用至少一条 delegated coding-assistant workflow：

- 从 TUI 启动 delegated lane
- 在主屏和副屏看到 lane 正在做什么
- 查看日志、产物、变更文件和下一步动作
- 安全 apply 或 discard 结果
- 保留足够证据，方便恢复或排查 lane

## 当前基线

main 上已经具备：

- provider/tool/shell/runtime projection 的共享 `AgentTask` records
- `/lane codex`、`/lane codex-review`、`/lane run`、`/lane ask`、
  `/lane inspect`、`/lane timeline`、`/lane diff`、`/lane artifacts`、
  `/lane accept`、`/lane apply`、`/lane discard`、`/lane retry`、
  `/lane stop`、`/lane cleanup` 等 lane commands
- `/agent review codex`、`/agent run codex`、`/agent status`、
  `/agent result`、`/agent cancel` 等 Codex external-agent commands
- side-1 和 side-2 lane panels、`.robocode/lanes/` 产物、delegated work 的
  ContextBundle/envelope records
- 0.1.18 之后锁定的 selector-first 交互标准

目前差距是“可用性”：用户不应该需要知道内部文件名、猜 lane id，或自己判断 lane
是不是还在工作。

## P0 范围

### 1. 一条可靠的 delegated review loop

先完成一条可靠 happy path，再扩展集成：

1. 以 deterministic shell/template lane 作为 CI baseline。
2. 当 Codex CLI 已安装且已登录时，支持真实 Codex read-only review lane。
3. Claude parity 先作为 capability/probe 路径，不阻塞本版本发布。

验收：

- `/lane run <command>` 可以 dispatch、stream/tail evidence、退出，并进入可 review 的终态。
- `/lane codex-review <task>` 或 `/agent review codex <task>` 生成可追踪 job/lane，
  有 result artifacts 和明确 next actions。
- Lane 状态流一致：
  `queued -> running -> reviewing -> accepted/applied/discarded/failed/blocked`。
- TUI 的 `NOW WORKING`、right rail、side-1、side-2 和 lane detail 显示同一个
  active delegated task。

### 2. Lane 和 Agent 决策 selector-first

Lane/agent 操作必须遵守 0.1.18 后新增的交互规则：只要用户需要选择 id 或动作，
就应该展示 selector，而不是只输出文字。

验收：

- `/lane` root 打开可执行的 command picker。
- `/lane inspect`、`/lane timeline`、`/lane diff`、`/lane artifacts`、
  `/lane accept`、`/lane apply`、`/lane discard`、`/lane retry`、
  `/lane stop`、`/lane cleanup`、`/lane archive` 能推荐/选择 live lane id，
  并展示 status、age 和 next action。
- `/agent status`、`/agent result`、`/agent cancel` 能推荐/选择 tracked
  external-agent job id。
- 鼠标点击、方向键、Enter 和 Esc 都可用。

### 3. Operator 状态可见

用户提交任务后，RoboCode 必须说清楚现在正在做什么。

验收：

- 主屏中间显示最高优先级 active task：provider thinking、tool call、shell/test、
  approval、lane running、lane reviewing、apply conflict 或 model/setup blocker。
- 状态包含 elapsed time、transport、task owner 和短 phase label，例如
  `Codex review running`、`waiting for approval`、`lane output ready for review`。
- 后台 lane 数量可见，但不抢占前台 turn 的注意力。
- 空闲状态要安静，不要看起来像坏了。

### 4. Evidence、Apply 和 Cleanup

用户必须能在 delegated 输出影响主工作区前建立信任。

验收：

- side-1 显示 lane console state、tail、attach command、pid/session 和 transport health。
- side-2 显示 artifacts、changed files、context pressure、decision file、
  diff/test evidence 和 apply/conflict state。
- `/lane accept <id>` 记录显式 decision artifact。
- `/lane apply <id>` 只在用户 accept 后，把 isolated worktree 中的 lane changes
  应用到主工作区。
- 冲突会进入可见 blocked state，并给出 `/lane resolve <id>` 和
  `/lane discard <id>` next actions。
- `/lane cleanup <id>` 安全、显式，并保留 release evidence 所需 audit trail。

### 5. Delegated Tool Capability Doctor

产品应该解释 delegated lane 为什么跑不起来。

验收：

- `/agent doctor` 或 `/lane doctor` 报告 Codex、Claude、tmux、shell template、
  worktree、Git、auth/config capability status。
- 缺少 binary 或 CLI 未登录时，给出修复提示。
- CI 中 Codex/Claude live execution 可选，但状态流必须由 deterministic probe 和
  fixtures 覆盖。

### 6. 真实使用截图集

每个 P0 功能都需要视觉证据：

- lane command selector
- `NOW WORKING` 中的 active delegated lane
- side-1 lane console/tail
- side-2 evidence/artifacts/context pressure
- Codex review result 或 deterministic shell/template review result
- accept/apply/discard decision state
- 可行时记录 conflict 或 blocked state
- final cleaned-up state

## P1 范围

- Codex/shell loop 稳定后补 Claude lane happy-path parity。
- 改善 tmux/PTY attach 和 lane input forwarding。
- ACP 只做 descriptor/probe mapping，不接入 mutating runtime。
- 改善 delegated lanes 的 provider-side ContextBundle pressure。
- 工作流可靠后，再继续提升 side-screen 密度和视觉精度。

## 非目标

- 不默认让 Codex 或 Claude 具备写权限。
- 本版本不发布大范围 ACP runtime。
- 不新增 plugin marketplace 或 mutating MCP/skill runtime。
- 不在 TUI 内做完整 terminal emulator。
- 不用截图替代 smoke tests。

## 测试计划

Focused：

- lane lifecycle reducer 与 status mapping tests
- provider/tool/shell/lane/external-agent job 的 `AgentTask` projection tests
- lane id 和 agent job id selector tests
- side-1 与 side-2 shared lane evidence render tests
- delegated lane sources、token estimate、long-output summary plus tail 的
  ContextBundle/envelope tests
- accept/apply/discard/retry/stop/cleanup command tests
- external tools 存在/缺失的 capability doctor tests

Regression：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.19 --quick --out-dir /tmp/robocode-0119-release-smoke-local
```

新增 smoke：

```bash
scripts/delegated-lane-smoke.sh --out-dir /tmp/robocode-0119-delegated-lane-smoke
```

Manual：

- macOS Terminal 和 iTerm2 各跑一次 TUI。
- lane active 时测试 resize、中文输入、command selector 和鼠标选择。
- deterministic shell/template lane 全闭环。
- Codex CLI 和 auth 可用时，跑真实 Codex read-only review lane。
- apply/discard/cleanup 每个用户可见状态都留截图。

## 发布标准

`0.1.19` 完成条件：

- delegated lane smoke 通过
- 截图生成并在 docs 中引用
- README/user guide 描述 delegated lane workflow，但不夸大 Claude/ACP 成熟度
- release status 记录 local RC、GitHub release、Homebrew tap update 和 post-publish smoke
- GitHub release assets 与 Homebrew formula 都发布到 `0.1.19`

