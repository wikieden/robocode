# RoboCode 0.1.22 状态 - Provider Detail 可用性补丁

英文版： [release-0.1.22-status.md](release-0.1.22-status.md)

`0.1.22` 是建立在 `0.1.21` 之上的聚焦可用性补丁。它不扩大交互系统范围，而是继续收紧
provider detail 页面，让它更像一个紧凑的设置表单。

## 发布状态

- Workspace version：`0.1.22`
- Release commit：等待发布
- Git tag：等待 `v0.1.22`
- GitHub release：等待 `https://github.com/wikieden/robocode/releases/tag/v0.1.22`
- Release workflow：等待
- Homebrew tap commit：等待 `wikieden/homebrew-tap`
- 本地 package：`dist/robocode-v0.1.22-aarch64-apple-darwin.tar.gz`
- 本地 package sha256：
  `e4c093d141ac6e13957f84a5196ea2e76e14af3cdea2590284e35f088ee02b89`

## 已包含改动

- Provider detail 中已存在的 API key 现在显示为“开头 + `*` + 结尾”，不再只显示
  `present`。
- Provider detail 动作行现在显示当前目标值，例如 provider id 或 model name，不再在每行后面放解释性长文案。
- Provider detail 的 model action 不再重复 “save with model” 这种长说明。
- TUI preview 生成时使用确定性 fake preview key，并清除 API-base override，避免截图写入本机真实 key 片段或用户自定义 endpoint。
- README、用户指南、TUI 设计文档、modules 和 staged roadmap 已同步说明 key 脱敏行为和下一步 editable-form 方向。

## 验证

Focused 检查：

```bash
cargo fmt --check
cargo check -p robocode-cli
cargo test -p robocode-cli command_palette --quiet
cargo test -p robocode-cli preview --quiet
git diff --check
```

TUI 视觉证据：

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.22 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Release smoke：

```bash
scripts/release-smoke.sh --version 0.1.22 \
  --out-dir /tmp/robocode-0122-release-smoke-full-local
```

结果：通过，包含本地 package smoke。仅跳过需要发布后外部状态的 live DeepSeek、
GitHub release assets、GitHub Actions 和 Homebrew 检查。

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.22 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0122-postpublish-check
```

结果：等待发布。

## 截图证据

确定性的 0.1.22 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-side-2.svg`

## 剩余风险

- Provider detail 仍是可操作选择器页，还不是真正可编辑表单。下一版交互 release 应加入
  key 来源、endpoint、默认 model、连接测试、保存和取消这些字段级编辑流程。
- Live DeepSeek smoke 是 opt-in，不在默认本地 release smoke 中运行。
