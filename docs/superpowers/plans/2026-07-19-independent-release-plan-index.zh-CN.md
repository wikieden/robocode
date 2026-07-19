# Core、TUI、GUI 独立版本列车实施计划索引

英文版：[2026-07-19-independent-release-plan-index.md](2026-07-19-independent-release-plan-index.md)

> **给 agentic workers：** 执行任何一条产品线前必须使用 `superpowers:executing-plans`；并发分支创建与收尾必须分别使用 `superpowers:using-git-worktrees` 和 `superpowers:finishing-a-development-branch`。

**目标：** 从同一个不可变 Core 合同 checkpoint 切出 TUI、GUI 两条独立版本线，并在 I0、I1、I2 三个集成门完成可验证的本地操作闭环。

**架构：** Core 是业务事实、权限、持久化和副作用的唯一权威；TUI、GUI 是只通过 `CoreClient` 交互的独立客户端。三条线分别使用 SemVer，前端 release manifest 固定 Core SHA、schema 与 capabilities。

**技术栈：** Rust workspace、JSONL + rebuildable SQLite、Ratatui/Crossterm、GUI framework gate 的胜出框架、Serde fixtures、双语 Markdown。

## 全局约束

- 设计检查顺序固定为全局 `index.html` → 客户端设计索引 → 组件库 → TUI 统一原型或 GUI 桌面驾驶舱。
- `tokens.css` 是视觉数值真源；`en` 与 `zh-CN`、皮肤、明暗、密度、motion 从 I0 建模并可持久化。
- Core checkpoint 完成前，TUI/GUI 只能做 spike、fixture consumer 和本地 UI 状态，不得发明业务合同。
- 分支所有权：Core `crates/**`；TUI `apps/tui/**`；GUI `apps/gui/**`。跨域缺口回到 Core 分支处理。
- 实现合入顺序固定为 Core → TUI → GUI；每一步都重新跑共享 fixture、迁移和 workspace gate。
- 历史 release 证据不改写；活跃视觉文档改用最新设计目录，旧 preview 只标记为 archive。

## 计划组

1. [Core 0.3 runtime contract](2026-07-19-core-0.3-runtime-contract.zh-CN.md)
2. [TUI 0.2 thin client](2026-07-19-tui-0.2-thin-client.zh-CN.md)
3. [GUI 0.1 desktop cockpit](2026-07-19-gui-0.1-desktop-cockpit.zh-CN.md)
4. [Independent release integration](2026-07-19-independent-release-integration.zh-CN.md)

设计规格：[Core、TUI、GUI 独立版本列车设计](../specs/2026-07-19-independent-core-tui-gui-release-train-design.zh-CN.md)

## 分支拓扑与门禁

```text
main@baseline
  └─ codex/v3-core-runtime
       ├─ I0: Core 0.3.0 / frontend-contract-v1 / immutable SHA
       ├─ codex/v3-tui-client  -> TUI 0.2.0-alpha.1 -> 0.2.0 -> 0.2.1
       └─ codex/v3-gui-client  -> GUI 0.1.0-alpha.1 -> beta.1 -> 0.1.0

integration: Core 0.3.0 -> I0 -> Core 0.3.1 + TUI 0.2.0 + GUI beta.1 -> I1
             -> Core 0.3.2 + TUI 0.2.1 + GUI 0.1.0 -> I2
```

## 完成定义

- 三个组件有独立版本与 changelog，且 TUI/GUI manifest 记录精确 Core checkpoint。
- 同一 fixture 在 Core、TUI、GUI 归约出同一业务事实；断流、gap、重连、迁移有测试。
- TUI 不再持有 engine/provider/Git/process 权威；GUI 只使用 `CoreClient`。
- `en`、`zh-CN` key parity、8 个有效皮肤/明暗组合、密度、reduced motion、CJK、键盘和可访问性门通过。
- 一条真实本地任务完成 request → work → test/review → evidence → gate → apply/recovery，并产生追加式审计证据。
- `cargo test --workspace --quiet` 通过，活跃文档及说明性注释与最终行为一致。
