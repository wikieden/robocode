# RoboCode 0.1.20 状态 - Usability Beta Gate

英文版： [release-0.1.20-status.md](release-0.1.20-status.md)

`0.1.20` 是 Usability Beta Gate release。这个版本重点解决“不知道怎么操作”的问题：
首次启动 setup 是可操作向导，provider/model 故障会给出恢复动作，lane 操作有居中的
selector 和确定性截图证据。

## 发布状态

- Workspace version：`0.1.20`
- Release commit：`320de3318bb0e53727497ee0b23cec4e9cc40a41`
- Git tag：`v0.1.20`
- GitHub release：https://github.com/wikieden/robocode/releases/tag/v0.1.20
- Release workflow：https://github.com/wikieden/robocode/actions/runs/26686753200
- Homebrew tap commit：`wikieden/homebrew-tap@2f57fb1f8526afcea293f86377a584a212000201`
- 本地包：`dist/robocode-v0.1.20-aarch64-apple-darwin.tar.gz`
- 本地包 sha256：
  `7cc2eeb04ceebcf67926d0aeb843d538065bf0150e374c17f3a0b175ac9fa8b2`

## 本版变更

- `/setup` 现在打开独立的 `SETUP WIZARD` selector，包含 provider 配置、模型选择、
  permissions、theme、provider doctor、fallback smoke 和保存 defaults 等可操作行。
- 干净 TUI 启动时，如果当前在线 provider 缺少 API key，会预填 `/setup`；fallback
  session 仍保持离线直达。
- `/provider` 保持供应商配置语义。选择 provider 后进入 `PROVIDER CONFIG`，展示
  key/env、endpoint、模型候选、doctor、switch/save 等动作。
- `/models` 保持按 provider 分组的跨供应商模型选择器。
- Provider/model failure 被分类为 missing key、auth、rate limit、timeout、
  context overflow、compatibility、model unavailable 等恢复类型，并给出具体 next action。
- `/lane` 现在打开居中的 `LANE ACTIONS` selector，并对 tracked lane 提供 id-specific
  inspect、timeline、diff、artifacts 操作。
- TUI preview 和 regression fixture 新增 setup wizard 与 lane selector 状态，并继续覆盖
  main cockpit、resize、中文输入、provider/model selector、lane detail 和副屏状态。
- `docs/release-0.1.21-plan.zh-CN.md` 记录下一步交互系统收口计划，确保后续从同一批
  可用性问题继续推进。

## 验证

聚焦测试和 workspace 检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/daily-loop-smoke.sh /tmp/robocode-0120-daily-loop-smoke
```

TUI 视觉证据：

```bash
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-previews.sh docs/previews/generated

ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.20 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

本地 release smoke：

```bash
scripts/release-smoke.sh --version 0.1.20 \
  --out-dir /tmp/robocode-0120-release-smoke-full-local
```

结果：包含 `package-smoke` 在内通过；只跳过需要发布后状态或显式 live key 的 DeepSeek、
GitHub release 和 Homebrew 检查。

GitHub release asset 验证：

```bash
scripts/release-smoke.sh --version 0.1.20 --quick \
  --github-release-assets --skip-package \
  --out-dir /tmp/robocode-0120-github-release-check
```

结果：GitHub release assets checksum validation 通过。

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.20 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0120-postpublish-check
```

结果：在 `/tmp/robocode-0120-postpublish-check` 通过，包含 GitHub release assets
和 Homebrew validation。

Homebrew formula 验证：

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

结果：`brew fetch` 解析到 formula `robocode (0.1.20)`，`brew audit` 无错误输出。

## 截图证据

确定性 0.1.20 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-side-2.svg`

## 剩余风险

- Provider 配置已经可发现，但 endpoint/key 编辑仍主要依赖 command/env。`0.1.21` 应完成
  统一 settings/form runtime。
- Mouse/focus 对 selector 有改善，但仍需要 composer、approval、selector、lane detail、
  side screens 和 right rail 共用一套显式 focus router。
- 本轮 release 未跑 live DeepSeek smoke；fallback、确定性 daily-loop、lane operator-loop、
  GitHub assets 和 Homebrew 已验证。
- Codex/Claude/tmux lane 仍是受监督 adapter path。本版发布阻塞项仍是确定性
  shell/template lane loop。
