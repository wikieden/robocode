# RoboCode 0.1.18 状态 - 交互体验加固

英文版： [release-0.1.18-status.md](release-0.1.18-status.md)

`0.1.18` 是交互体验加固版本，用来收尾 `0.1.17` 发布后继续补齐的
provider/model 设置体验。由于 `v0.1.17` 已经公开发布，本版本不改写旧 tag，而是把
新的 settings selector 行为作为新 release 发布。

## 发布状态

- Workspace version：`0.1.18`
- Release target：`v0.1.18`
- GitHub release：https://github.com/wikieden/robocode/releases/tag/v0.1.18
- Homebrew target：`wikieden/tap/robocode` stable `0.1.18`

## 本版本内容

- `/settings` 现在打开可操作 settings selector，不再是只读状态页。
- `/provider`、`/models`、`/permissions`、`/theme` 统一使用居中的 selector 交互：
  搜索、键盘选择、鼠标选择、Enter 应用。
- `/settings permissions <mode>` 会通过共享 runtime command path 修改权限模式。
- `/settings theme <name>` 会在当前 cockpit 内即时切换 TUI 主题。
- TUI 设计契约已明确 settings surface 必须 selector-first；`/config` 和
  `/provider doctor` 这类诊断/详情命令仍然是信息展示型。
- README、用户指南、release 截图和 preview assertions 都改为用 `/settings`
  selector 作为配置入口示例。

## 验证

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-previews.sh docs/previews/generated
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.18 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

发布后还必须验证：

```bash
scripts/release-smoke.sh --version 0.1.18 --quick \
  --github-release-assets --homebrew \
  --out-dir /tmp/robocode-0118-postpublish-check
```

## 截图证据

0.1.18 确定性 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.18-tui-side-2.svg`

## 剩余风险

- 主题选择会在当前 TUI session 内即时生效。持久化主题默认值仍需要 CLI/config，后续应纳入 settings 选项。
- 虽然 selector hit testing 已有单元测试，release acceptance 仍应在 macOS Terminal/iTerm2
  中重复一次鼠标操作验证。
