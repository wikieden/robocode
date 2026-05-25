# RoboCode 0.1.4 发布状态

最后更新：2026-05-25

## 目标

`0.1.4` 是稳定性优先的 TUI cockpit preview 版本。目标是把当前五阶段计划推进到
外部用户可以试用的状态：TUI 基础稳定、真实 provider/tool 流程可用、lane 操作
可观察，并且具备可安装的 release artifact。

## 阶段映射

1. baseline operator run：进行中。
2. P0 TUI 交互修复：进行中。
3. lane operator workflow hardening：等待完整人工 operator run。
4. provider compatibility pass：等待 DeepSeek live smoke 确认。
5. release candidate packaging：进行中。

## Baseline 证据

- `cargo test --workspace --quiet` 通过。
- `scripts/tui-previews.sh /tmp/robocode-014-baseline-preview` 已用于 baseline
  verification 的 TUI preview 生成。
- `robocode-cli --help` 能输出启动参数和 provider 列表。
- `robocode-cli --version` 现在输出 `robocode-cli 0.1.4`。
- fallback REPL smoke 通过，覆盖 `/status` 和 `/exit`。
- `scripts/package-release.sh 0.1.4 aarch64-apple-darwin` 打包 smoke 通过。
- 解压 `dist/robocode-v0.1.4-aarch64-apple-darwin.tar.gz` 后，包内二进制
  smoke 通过。
- macOS arm64 archive SHA-256：
  `747afc5cd066939f97d12180a1deaf6c608b088ccbadaf4f1e604f3d83c13fb3`。

## 已为 0.1.4 落地的变更

- workspace package version 从 `0.1.3` 升到 `0.1.4`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.4`。
- CLI 增加 `--version` / `-V`，用于 release smoke 和 issue 反馈。
- GitHub release workflow 默认 tag 改为 `v0.1.4`。
- README 安装示例改为 `v0.1.4`。
- README 系统截图保留人工整理过的版式，只把可见版本号更新为 `0.1.4`。

## 当前发现

### P0

- 本轮自动 baseline 未确认新的自动化 P0 blocker。
- release 前仍需要带凭证手动确认 DeepSeek live TUI smoke。

### P1

- `/lane` 仍是 TUI/runtime 命令面；普通 REPL 会返回 `Unknown command /lane`。
  这与当前架构方向一致，但 release notes 需要明确，避免用户以为 cockpit 外也
  已支持 lane 管理。
- resize、审批弹窗清理、输入法位置、鼠标交互、副屏生命周期、tmux/PTY log
  capture、lane apply/conflict recovery 仍需要完整人工 operator run。

### P2

- 完整 cursor-addressed terminal replay 继续后置。
- inline conflict editor 继续后置。
- 更多外部 coding-tool templates 继续按真实需求推进。

## 下一道 Gate

打 `v0.1.4` tag 前需要完成：

- fallback TUI 人工 smoke。
- DeepSeek V4 Flash live smoke。
- 一个 shell lane operator run。
- 支持环境下的一个 tmux 或 PTY lane operator run。
- `cargo test --workspace --quiet`。
- 为 GitHub Actions 配置的全部目标构建 release artifact。
