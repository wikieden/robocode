# RoboCode 0.1.23 状态 - Provider 与 Model 设置补丁

英文版： [release-0.1.23-status.md](release-0.1.23-status.md)

`0.1.23` 是 provider/model 设置体验的聚焦可用性版本。它把 provider 连接和 model
选择分开，让 `/connect` 与 `/models` 更接近 opencode 的 picker 模式，并让 provider
认证方式变得可见，而不是把所有 provider 都当成 API key 流程。

## 发布状态

- Workspace version：`0.1.23`
- Release commit：`ec608e62d94bde511f2c25b6a1322baa873c7b76`
- Git tag：`v0.1.23`
- GitHub release：
  `https://github.com/wikieden/robocode/releases/tag/v0.1.23`
- Release workflow：
  `https://github.com/wikieden/robocode/actions/runs/26711635516`
- Homebrew tap commit：`wikieden/homebrew-tap@708cef1`
- 本地 package：`dist/robocode-v0.1.23-aarch64-apple-darwin.tar.gz`
- 本地 package sha256：
  `5c20394e27a68187ebfe095d9a5afb6e80e562c9fa7f7d577aa125b41f77ed61`

## 已包含改动

- `/connect` 是 provider 连接选择器。一级列表只显示供应商，而不是 provider/model 组合。
- `/connect <provider>` 打开 provider scoped 详情页，展示脱敏 key 状态、endpoint 来源、
  默认模型、active model 列表、favorite model 动作和 provider doctor 入口。
- Provider descriptor 现在暴露 auth modes。OpenAI 可以声明网页登录或 API key，gateway
  provider 声明 API key，本地 provider 声明 local auth。
- `/models` 先显示 Favorites，再显示 Recent，再显示按 provider 分组的 active model 行。
- Model favorites 是 provider/model 组合。收藏行不会在后面的 provider 分组里重复显示，
  并且可以用 `Ctrl-F` 收藏当前选中的 model 行。
- Provider scoped config 写入支持持久化 active models 和 favorite models，同时不保存明文
  API key。
- README、用户指南、模块索引、阶段路线图和确定性 TUI preview evidence 已同步到 0.1.23 设置流程。

## 验证

Focused 检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
git diff --check
```

TUI 视觉证据：

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.23 \
scripts/tui-regression.sh docs/previews/generated
```

Release smoke：

```bash
scripts/release-smoke.sh --version 0.1.23 \
  --out-dir /tmp/robocode-0123-release-smoke-full-local
```

结果：通过，包含本地 package smoke。仅跳过需要发布后外部状态的 live DeepSeek、
GitHub release assets、GitHub Actions 和 Homebrew 检查。

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.23 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0123-postpublish-check
```

结果：通过。该 smoke 已验证发布后的 GitHub release assets 和 Homebrew formula，
证据目录为 `/tmp/robocode-0123-postpublish-check`。

Homebrew tap 检查：

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

结果：通过，并已推送 `wikieden/homebrew-tap@708cef1`。

## 截图证据

确定性的 0.1.23 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-side-2.svg`

## 剩余风险

- OpenAI web-login 在本版本只是 descriptor 可见；真正的网页登录 runtime 流程仍是后续工作。
- Provider detail 仍是 selector/detail surface，还不是带 save/cancel 事务语义的完整字段编辑器。
- Live provider 可用性依赖模型和账号状态；本地 fallback、确定性 TUI 证据、release assets 和
  Homebrew smoke 仍是必须通过的发布门禁。
