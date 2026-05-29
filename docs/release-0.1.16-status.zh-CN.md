# RoboCode 0.1.16 状态 - TUI 交互可靠性

英文版： [release-0.1.16-status.md](release-0.1.16-status.md)

## 状态

`0.1.16` 是插入的 TUI 交互可靠性本地 release candidate。它被放在轻量
spec/steering workflow 之前，目的是先让 cockpit 在真实使用时保持可信和可操作，
再继续增加更大的 workflow surface。

GitHub Release assets 和 Homebrew tap 更新尚未发布。

## 已落地范围

- workspace version 已提升到 `0.1.16`。
- Provider turn 已移到 worker/channel 边界后面。provider worker 运行时，TUI 会继续刷新
  `NOW WORKING`、elapsed time、状态栏、lane snapshot 和 approval prompt。
- Approval prompt 会从 provider worker 桥接回既有 permission path；默认焦点仍是
  `Approve`。
- Active-turn composer 快捷键都有真实行为：`Ctrl-J` 发送，`Ctrl-K` 清空，
  `Ctrl-R` 重新载入最近用户输入，`Ctrl-N` 开始 `/task add ...`，`?` 只在输入区为空时打开帮助。
- Command suggestion 长列表现在会窗口化展示，并保持选中行可见。鼠标 hit testing
  会从可见行映射回完整 suggestion index。
- Approval `Diff` 焦点会在可用时展示 prompt 携带的真实 evidence / preview lines，
  不再只是装饰性 affordance。
- 已更新 TUI interaction audit、用户指南、cockpit 设计、roadmap、README 和截图引用。

## 验证

2026-05-29 本地通过：

```bash
cargo fmt --check
git diff --check
cargo clippy -p robocode-types -p robocode-core -p robocode-cli --all-targets -- -D warnings
cargo test -p robocode-cli tui::render::tests::render_frame_overlays_approval_modal --quiet
cargo test --workspace --quiet
scripts/release-smoke.sh --version 0.1.16 --quick --out-dir /tmp/robocode-0116-release-smoke-local
```

quick release smoke 通过：

- `cargo-fmt`
- `cargo-clippy`
- `robocode-cli-terminal-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`

Smoke evidence 目录：

```text
/tmp/robocode-0116-release-smoke-local
```

## 视觉证据

0.1.16 确定性 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-side-2.svg`

结构化截图证据：

```text
/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/tui-regression-evidence.json
```

## 剩余风险

- 取消仍是 best-effort。`Ctrl-C` 可以请求取消，但已经发出的 provider request
  可能会在 worker 看到取消状态前正常返回。
- 本版本解决的是 provider work 运行期间 TUI 不冻结，还不是完整 token-by-token
  provider streaming。
- 鼠标支持仍偏窄：approval 和 command suggestions 已覆盖，右栏选择、副屏滚动、
  lane modal controls、transcript links 和 mouse wheel 仍是后续工作。
- 光标闪烁和 IME candidate window 位置仍部分取决于宿主终端。
- GitHub Release packaging、多平台 assets、发布后验证和 Homebrew tap 更新不属于本地 RC 闭环。

## 下一步

`0.1.17` 可以回到轻量 spec/steering workflow，但要把本轮剩余交互 backlog 一起带进去：
鼠标覆盖、支持真实取消的路径、streaming 展示，以及更多 Terminal / iTerm2 手工验收截图。
