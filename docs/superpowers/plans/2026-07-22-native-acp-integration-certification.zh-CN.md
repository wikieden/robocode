# 原生与 ACP 集成验收实施计划

> **面向智能体执行者：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 验证一个统一候选版本，其中 Core `0.3.4`、TUI `0.3.3`、GUI `0.1.0-rc.2` 完成相同的原生和 ACP 交互闭环。

**架构：** 按 Core → TUI → GUI 固定顺序整合不可变检查点。先用一个权威有序事件夹具做确定性 parity，再收集不会泄露凭据的 DeepSeek 与 ACP 实时证据。

**技术栈：** Git worktree、Cargo、Shell gate、确定性 JSON fixture、TUI preview、Tauri build、实时 Provider/ACP smoke。

## 全局约束

- 集成顺序严格为 Core `0.3.4` → TUI `0.3.3` → GUI `0.1.0-rc.2`。
- 两个前端分支必须共享完全相同的 Core SHA。
- 原生验收覆盖配置、启动、对话、控制、观察、完成、恢复。
- ACP 验收覆盖发现/安装/认证、启动、续聊/恢复、审批/控制、结果/证据、重试、取消、重启恢复。
- 实时证据只记录 Provider/模型/adapter 标识和成功状态，不记录 key 值或原始认证输出。
- 未获得额外明确授权前，不合并或推送 `main`。

---

### 任务 1：记录不可变检查点并按固定顺序集成

**文件：** 新建 `docs/release-evidence/native-acp-interaction/checkpoints.md`；更新中英文并发开发计划。

- [ ] **步骤 1：分别运行 `git merge-base --is-ancestor codex/v3-core-runtime codex/v3-tui-client` 和 `git merge-base --is-ancestor codex/v3-core-runtime codex/v3-gui-client`，两条都必须退出 0**
- [ ] **步骤 2：运行 `git worktree add .worktrees/native-acp-integration -b codex/native-acp-integration codex/v3-core-runtime`**
- [ ] **步骤 3：依次运行 `git merge --no-ff codex/v3-tui-client -m "merge: integrate TUI 0.3.3"` 与 `git merge --no-ff codex/v3-gui-client -m "merge: integrate GUI 0.1.0-rc.2"`；共享 manifest 保留三个独立版本**
- [ ] **步骤 4：记录 branch、worktree、版本、Core base SHA、terminal SHA 和验证状态，提交 `docs: record native and ACP integration checkpoints`**

### 任务 2：证明 Core/TUI/GUI 确定性 parity

**文件：** 新建 `scripts/native-acp-fixture-parity.sh` 与 `fixture-parity.md`。

- [ ] **步骤 1：脚本使用 `set -euo pipefail`，依次运行 Core replay、TUI render、GUI projection 三个明确命名测试**
- [ ] **步骤 2：运行 `bash scripts/native-acp-fixture-parity.sh`；若任一前端未接入权威夹具必须失败，接入后必须退出 0**
- [ ] **步骤 3：报告 Lane receipt、原生 turn、ACP 会话、审批、工具结果、成本、证据、终态、retry attempt、replay cursor 的归一化数量与 id**
- [ ] **步骤 4：提交 `test: certify native and ACP frontend parity`**

### 任务 3：收集 DeepSeek 原生 Agent 实时证据

**文件：** 新建 `scripts/live-native-agent-smoke.sh` 与 `live-native-agent.md`。

- [ ] **步骤 1：脚本只检查 `DEEPSEEK_API_KEY` 是否存在；未设置以 77 退出。存在时执行只读仓库任务、流式期间排队一次 follow-up、等待完成、重连并验证对话与成本恢复**
- [ ] **步骤 2：运行 `bash scripts/live-native-agent-smoke.sh`，成功输出 `LIVE_NATIVE_AGENT_PASS`，不得打印 key 或 env dump**
- [ ] **步骤 3：只记录 Lane id、Provider id、模型 id、有序状态、工具/证据 id、token/cost 和恢复结果，提交 `test: record live DeepSeek native agent evidence`**

### 任务 4：收集 ACP 发现、对话、控制和恢复实时证据

**文件：** 新建 `scripts/live-acp-agent-smoke.sh` 与 `live-acp-agent.md`。

- [ ] **步骤 1：查询并 probe adapter，只选择 Core 报告 `Ready` 的项，优先 Codex→Claude→Kiro→Custom ACP；无 Ready adapter 时以 77 分类退出**
- [ ] **步骤 2：在已有 Lane 中启动只读任务，发送一次精确续聊；建立第二 attempt 并取消；重启 client host，验证两个会话恢复**
- [ ] **步骤 3：运行 `bash scripts/live-acp-agent-smoke.sh`，成功输出 `LIVE_ACP_AGENT_PASS`，不得暴露 stderr、认证资料或命令环境**
- [ ] **步骤 4：记录 adapter/capability/model/session/input/approval/evidence/cancel/restore id，提交 `test: record live ACP interaction evidence`**

### 任务 5：运行 TUI 与 GUI 用户体验 gate

**文件：** 新建 `tui-experience.md` 与 `gui-experience.md`。

- [ ] **步骤 1：运行 `cargo test -p viden-tui && scripts/tui-turn-controller-smoke.sh && scripts/rc-tui-stability-smoke.sh && scripts/tui-regression.sh && scripts/tui-previews.sh`，覆盖 `n`、`/acp`、picker、续聊、审批、取消、错误和恢复**
- [ ] **步骤 2：运行 `cargo test -p viden-gui && npm --prefix apps/gui test && npm --prefix apps/gui run build && npm --prefix apps/gui run tauri build`，确认 app bundle 存在**
- [ ] **步骤 3：人工验证 GUI 两条主路径：Welcome 打开 Git 文件夹后 `+ -> Viden Agent -> task`；选中 Lane 后 `+ -> ACP -> task`。两者都留在 D1，ACP 不创建 Lane，不进入 D4/D11，无白色外框且可全键盘操作**
- [ ] **步骤 4：提交 `docs: record native and ACP client experience`**

### 任务 6：运行工作区最终认证 gate

**文件：** 新建 `docs/release-evidence/native-acp-interaction/certification.md`。

- [ ] **步骤 1：运行 `cargo fmt --check && scripts/check-dependency-boundaries.sh && bash scripts/native-acp-fixture-parity.sh && cargo test --workspace --quiet && git diff --check`，全部退出 0**
- [ ] **步骤 2：逐项记录原生与 ACP 的配置/发现、启动、对话、控制、观察、完成、恢复 PASS/FAIL 和证据路径；实时项跳过即阻塞认证，不得算 PASS**
- [ ] **步骤 3：提交 `docs: certify native and ACP interaction milestone`**
- [ ] **步骤 4：报告集成分支、worktree、terminal SHA、三个版本、全部检查、实时证据、跳过项和阻塞；停止在 main/tag/push/release 之前**
