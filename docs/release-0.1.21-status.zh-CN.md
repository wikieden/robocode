# RoboCode 0.1.21 状态 - 交互系统收口

英文版： [release-0.1.21-status.md](release-0.1.21-status.md)

`0.1.21` 是 Interaction System Completion release。这个版本继续收紧
provider/settings 体验：所有入口尽量变成可操作 picker/form，provider 配置和
model 选择保持分离，并新增“供应商一级列表”和“provider 详情配置表单”的截图证据。

## 发布状态

- Workspace version：`0.1.21`
- Release commit：等待发布
- Git tag：等待 `v0.1.21`
- GitHub release：等待 `https://github.com/wikieden/robocode/releases/tag/v0.1.21`
- Release workflow：等待
- Homebrew tap commit：等待 `wikieden/homebrew-tap`
- 本地 package：`dist/robocode-v0.1.21-aarch64-apple-darwin.tar.gz`
- 本地 package sha256：
  `1f83e4dbf3f347d0dcbb4c67407f9bff4026f637e626a0782292260bb9505e55`

## 已包含改动

- `/provider` 一级现在只显示供应商 id，例如 `deepseek`、`openrouter`、
  `anthropic`、`openai`，不再把 API key、endpoint、model 或说明文本混在一级行里。
- 选择供应商后进入 `PROVIDER CONFIG`，显示 key env 状态、endpoint、候选模型、
  设为默认、当前会话切换、doctor，以及跳转 `/models` 的动作。
- TUI preview 和 regression matrix 增加 provider detail 截图，产品 review 可以
  分别检查“选择供应商”和“配置供应商”两个状态。
- README 和用户指南已指向 0.1.21 截图证据集，包括新的 provider detail form。
- 本版本仍聚焦交互体验。ACP/MCP/plugin/skill 的 mutating runtime 仍不进入范围。

## 验证

Focused 与 workspace 检查：

```bash
cargo fmt --check
git diff --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p robocode-cli preview --quiet
cargo test -p robocode-cli command_palette --quiet
cargo test --workspace --quiet
scripts/daily-loop-smoke.sh /tmp/robocode-0121-daily-loop-smoke
```

TUI 视觉证据：

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.21 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

本地 release smoke：

```bash
scripts/release-smoke.sh --version 0.1.21 \
  --out-dir /tmp/robocode-0121-release-smoke-full-local
```

结果：通过，包含本地 package smoke。仅跳过需要发布后外部状态的 live DeepSeek、
GitHub release assets、GitHub Actions 和 Homebrew 检查。

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.21 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0121-postpublish-check
```

结果：等待发布。

## 截图证据

确定性的 0.1.21 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-side-2.svg`

功能映射：

- Provider 选择：`0.1.21-tui-provider-selector.svg`
- Provider 详情配置：`0.1.21-tui-provider-detail.svg`
- Model 选择：`0.1.21-tui-model-selector.svg`
- 首次 setup：`0.1.21-tui-setup-wizard.svg`
- 当前工作可视化：`0.1.21-tui-main.svg`、`0.1.21-tui-live-turn.svg`
- Composer/中文输入/resize 可靠性：`0.1.21-tui-cjk-input.svg`、
  `0.1.21-tui-main-resize.svg`
- Lane 与副屏：`0.1.21-tui-lane-selector.svg`、
  `0.1.21-tui-lane-detail.svg`、`0.1.21-tui-side-1.svg`、
  `0.1.21-tui-side-2.svg`

## 剩余风险

- Provider detail 已经可发现、可操作，但 endpoint/key 编辑仍主要依赖 command/env。
  后续 settings-form 版本应加入真正的文本编辑字段。
- Mouse/focus 已覆盖 selector row 和 modal preview，但更完整的 pane scrolling 与
  right-rail click routing 仍是后续工作。
- 本地 RC 这轮没有跑 live DeepSeek smoke；fallback、daily-loop、lane operator-loop、
  package smoke 和确定性 TUI 证据已通过。
