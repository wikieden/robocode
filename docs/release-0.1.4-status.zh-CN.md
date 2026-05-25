# RoboCode 0.1.4 发布状态

最后更新：2026-05-25

## 目标

`0.1.4` 是稳定性优先的 TUI cockpit preview 版本。目标是把当前五阶段计划推进到
外部用户可以试用的状态：TUI 基础稳定、真实 provider/tool 流程可用、lane 操作
可观察，并且具备可安装的 release artifact。

## 阶段映射

1. baseline operator run：完成。
2. P0 TUI 交互修复：已满足 0.1.4 release gate。
3. lane operator workflow hardening：shell 和 tmux lane smoke 已完成。
4. provider compatibility pass：DeepSeek V4 Flash live smoke 已完成。
5. release candidate packaging：非上传模式 artifact build gate 已完成。

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
- fallback TUI smoke 已在 tmux 中通过，覆盖
  `/lane run printf robocode-lane-smoke`、`/lane inspect L1` 和 `/exit`；
  lane evidence 写入隔离的 `/tmp/robocode-014-tui-smoke.*` workspace。
- tmux lane smoke 已在 tmux 中通过，覆盖 `/lane tmux L1`；lane 写入
  `L1.tmux.md` 和实时 `L1.log`，路径位于隔离的
  `/tmp/robocode-014-tmux-smoke.*` workspace。
- DeepSeek V4 Flash TUI live smoke 已通过：环境中存在 `DEEPSEEK_API_KEY`，
  prompt `reply exactly ROBOSMOKE` 在 TUI pane capture 和 JSONL transcript
  中都得到 assistant response `ROBOSMOKE`。
- GitHub Actions release artifact validation 已以 `upload_to_release=false`
  跑通全部配置目标：`aarch64-apple-darwin`、`x86_64-apple-darwin`、
  `x86_64-unknown-linux-gnu` 和 `x86_64-pc-windows-msvc`。
  Run: https://github.com/wikieden/robocode/actions/runs/26401318871。

## 已为 0.1.4 落地的变更

- workspace package version 从 `0.1.3` 升到 `0.1.4`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.4`。
- CLI 增加 `--version` / `-V`，用于 release smoke 和 issue 反馈。
- GitHub release workflow 默认 tag 改为 `v0.1.4`。
- README 安装示例改为 `v0.1.4`。
- README 系统截图保留人工整理过的版式，只把可见版本号更新为 `0.1.4`。
- lane log summary 在持久化和渲染前会清理 terminal control sequence 和
  prompt-only 噪声，避免 tmux/PTY log 把 escape sequence 推进 cockpit 布局。

## 当前发现

### P0

- 当前没有已确认的自动化或 live-smoke P0 blocker。

### P1

- `/lane` 仍是 TUI/runtime 命令面；普通 REPL 会返回 `Unknown command /lane`。
  这与当前架构方向一致，但 release notes 需要明确，避免用户以为 cockpit 外也
  已支持 lane 管理。
- 0.1.4 之后仍建议继续补足审批弹窗边界、输入法位置、鼠标交互、副屏生命周期、
  lane apply/conflict recovery 的完整人工 operator 覆盖；shell 和 tmux lane
  smoke 已覆盖本次 release gate。

### P2

- 完整 cursor-addressed terminal replay 继续后置。
- inline conflict editor 继续后置。
- 更多外部 coding-tool templates 继续按真实需求推进。

## 下一道 Gate

打 `v0.1.4` tag 前还需要完成最新 lane-summary 修复后的最终本地验证：

- `cargo test --workspace --quiet`。
- 提交并推送 lane-summary sanitization 和更新后的 release status。
- 创建 `v0.1.4` Git tag，并用 `upload_to_release=true` 运行 release workflow。
