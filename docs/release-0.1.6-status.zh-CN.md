# RoboCode 0.1.6 发布状态

英文版： [release-0.1.6-status.md](release-0.1.6-status.md)

最后更新：2026-05-26

## 目标

`0.1.6` 是 live-cockpit 和 extension-foundation 版本。目标是让 RoboCode 在真实
编程过程中更可操作：主屏显示 live activity，副屏显示 lane 和 ops evidence，
agent / extension diagnostics 可发现，并且 ACP 方向有最小协议 proof，而不只是
路线图描述。

## 阶段映射

1. Live cockpit visibility：本地实现已落地。
2. Agent 和 extension visibility：本地实现已落地。
3. side-1 和 side-2 evidence screens：本地实现已落地。
4. ACP readiness 和 protocol probe：本地实现已落地。
5. Release packaging：local smoke 已通过。
6. 外部发布：等待 GitHub release 和 Homebrew tap validation。

## Candidate 证据

- workspace package version 从 `0.1.5` 升到 `0.1.6`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.6`。
- GitHub release workflow 默认 tag 改为 `v0.1.6`。
- README 安装示例改为 `v0.1.6`。
- README 系统截图保留人工整理过的版式，可见版本号更新为 `0.1.6`。
- side-2 preview validation 现在检查真实 ops panels：
  `TESTS / LSP`、`MCP / CONTEXT`、`EXTENSIONS` 和 `RECENT EVIDENCE`。
- `/agent list` 和 `/agent doctor acp` 会展示实验 ACP adapter 以及
  `ROBOCODE_AGENT_ACP_COMMAND` setup 状态。
- `/agent doctor acp` 会执行最小 JSON-RPC `initialize` handshake probe，把
  JSONL evidence 记录到 `.robocode/agents/`，并报告 protocol、agent
  name/version、timeout 或失败详情。
- 带 DeepSeek 真实 provider validation 的完整本地 release smoke 已通过：
  `scripts/release-smoke.sh --version 0.1.6 --deepseek --out-dir /tmp/robocode-016-release-smoke-deepseek-local`。
- Evidence 目录：
  `/tmp/robocode-016-release-smoke-deepseek-local`。
- smoke matrix 已通过 `cargo-fmt`、`robocode-cli-tests`、
  `workspace-tests`、`tui-previews`、`fallback-cli-smoke`、
  `shell-lane-smoke`、`tmux-lane-smoke`、`package-smoke` 和
  `deepseek-cli-smoke`。
- DeepSeek V4 Flash live smoke 已通过；transcript 中包含
  `robocode-deepseek-smoke-ok`。
- `aarch64-apple-darwin` host package smoke 已通过；解压后的二进制输出
  `robocode-cli 0.1.6`。
- macOS arm64 archive SHA-256：
  `22413a9d94fc0fc950ba47e232f9025ac218eb35cd788c13b2b3d44231cadab1`。

## 验证门禁

把 version bump 推到 `main` 后，发布 `v0.1.6` 前运行：

```bash
scripts/release-smoke.sh --version 0.1.6 --skip-package --deepseek --github-actions
```

最终状态更新需要记录：

- GitHub Actions validation run URL；
- 已发布 release URL 和 artifact 列表；
- Homebrew tap commit 和 fetch/install smoke 结果。

## 当前发现

### P0

- `0.1.6` version bump 推到 `main` 后运行 GitHub Actions artifact
  validation，并发布最终 release。

### P1

- 完整 `/lane acp <agent> <task>` execution 仍是后续工作。`0.1.6` 证明的是
  process boundary 和 handshake/evidence path，不是完整 edit loop。
- Extension invocation 继续保守推进：先让 diagnostics 和 visibility 可用，再启用
  更宽的 plugin execution。

### P2

- 自动任务拆分继续后置。
- 完整 cursor-addressed terminal replay 继续后置。
- 更多外部 coding-agent templates 继续按真实需求推进。

## Release 结果

只有当 release workflow 上传全部配置 artifacts，并且 Homebrew tap 更新完成后，
`v0.1.6` 才标记为已发布。
