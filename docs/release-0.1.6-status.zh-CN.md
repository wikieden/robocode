# Viden 0.1.6 发布状态

英文版： [release-0.1.6-status.md](release-0.1.6-status.md)

最后更新：2026-05-26

## 目标

`0.1.6` 是 live-cockpit 和 extension-foundation 版本。目标是让 Viden 在真实
编程过程中更可操作：主屏显示 live activity，副屏显示 lane 和 ops evidence，
agent / extension diagnostics 可发现，并且 ACP 方向有最小协议 proof，而不只是
路线图描述。

## 阶段映射

1. Live cockpit visibility：本地实现已落地。
2. Agent 和 extension visibility：本地实现已落地。
3. side-1 和 side-2 evidence screens：本地实现已落地。
4. ACP readiness 和 protocol probe：本地实现已落地。
5. Release packaging：local smoke 已通过。
6. 外部发布：GitHub release 和 Homebrew tap validation 已通过。

## Candidate 证据

- workspace package version 从 `0.1.5` 升到 `0.1.6`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.6`。
- GitHub release workflow 默认 tag 改为 `v0.1.6`。
- README 安装示例改为 `v0.1.6`。
- README 系统截图保留人工整理过的版式，可见版本号更新为 `0.1.6`。
- side-2 preview validation 现在检查真实 ops panels：
  `TESTS / LSP`、`MCP / CONTEXT`、`EXTENSIONS` 和 `RECENT EVIDENCE`。
- `/agent list` 和 `/agent doctor acp` 会展示实验 ACP adapter 以及
  `VIDEN_AGENT_ACP_COMMAND` setup 状态。
- `/agent doctor acp` 会执行最小 JSON-RPC `initialize` handshake probe，把
  JSONL evidence 记录到 `.viden/agents/`，并报告 protocol、agent
  name/version、timeout 或失败详情。
- 带 DeepSeek 真实 provider validation 的完整本地 release smoke 已通过：
  `scripts/release-smoke.sh --version 0.1.6 --deepseek --out-dir /tmp/viden-016-release-smoke-deepseek-local`。
- Evidence 目录：
  `/tmp/viden-016-release-smoke-deepseek-local`。
- smoke matrix 已通过 `cargo-fmt`、`viden-cli-tests`、
  `workspace-tests`、`tui-previews`、`fallback-cli-smoke`、
  `shell-lane-smoke`、`tmux-lane-smoke`、`package-smoke` 和
  `deepseek-cli-smoke`。
- DeepSeek V4 Flash live smoke 已通过；transcript 中包含
  `viden-deepseek-smoke-ok`。
- `aarch64-apple-darwin` host package smoke 已通过；解压后的二进制输出
  `viden-cli 0.1.6`。
- macOS arm64 archive SHA-256：
  `22413a9d94fc0fc950ba47e232f9025ac218eb35cd788c13b2b3d44231cadab1`。
- GitHub Actions release artifact validation 已以 `upload_to_release=false`
  跑通全部配置目标：`aarch64-apple-darwin`、`x86_64-apple-darwin`、
  `x86_64-unknown-linux-gnu` 和 `x86_64-pc-windows-msvc`。
  Run: https://github.com/wikieden/viden/actions/runs/26440197730。
- 最终 GitHub release workflow 已以 `upload_to_release=true` 通过，并上传全部
  配置 artifacts。
  Run: https://github.com/wikieden/viden/actions/runs/26440351407。
- Homebrew tap `wikieden/homebrew-tap` 中的 Viden formula URL 和 SHA-256
  已指向 `v0.1.6`。
  Commit: https://github.com/wikieden/homebrew-tap/commit/b8c94da。
- Homebrew fetch smoke 已通过：
  `brew fetch --force wikieden/tap/viden` 输出
  `Formula viden (0.1.6)`。

## 验证门禁

`0.1.6` 计划内验证门禁已全部通过：

- 带 package 和 DeepSeek 真实 provider validation 的本地 release smoke；
- `upload_to_release=false` 的 GitHub Actions artifact validation；
- `upload_to_release=true` 的最终 GitHub release artifact upload；
- Homebrew tap update 和 fetch smoke。

## 当前发现

### P0

- `0.1.6` release 无剩余 P0。

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

`v0.1.6` 已发布：

- https://github.com/wikieden/viden/releases/tag/v0.1.6

release 包含：

- `viden-v0.1.6-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.6-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.6-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.6-x86_64-pc-windows-msvc.tar.gz`
- 每个 archive 对应的 `.sha256` 文件。

GitHub 返回的 asset digests：

- `aarch64-apple-darwin`：
  `sha256:5c2783b86574edf95a66af7b176ea6e3c24680782f53817d00661661397faac3`
- `x86_64-apple-darwin`：
  `sha256:7229d9a2dcdd796735ccbde6dcfccac8d35a66454b76e42cca561242e3789c6a`
- `x86_64-unknown-linux-gnu`：
  `sha256:b47d2648de98a72e2d9e0b8afef1a92090bb4325374d6f69086b7b790f9da77e`
- `x86_64-pc-windows-msvc`：
  `sha256:4b94e1f645b8b383a1b131f24e5eebfacff6f3d1ac684c05fbfe4a372b4ce386`

Homebrew 安装路径：

```bash
brew tap wikieden/tap
brew install viden
```
