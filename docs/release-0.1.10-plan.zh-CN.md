# RoboCode 0.1.10 计划

英文版： [release-0.1.10-plan.md](release-0.1.10-plan.md)

最后更新：2026-05-27

## 目标

`0.1.10` 是 Programming Cockpit Feedback 版本。重点不是继续堆面板，而是把用户输入、
provider 正在工作、子 agent lane、视觉证据之间的反馈链路打通。

这个版本要让操作者随时知道三件事：RoboCode 现在到底在做什么、这个状态来自哪条真实证据、
下一步可以怎么操作。

## 范围

- TUI 提交 provider 请求后，立即生成一条真实 `AgentTask`，不再只靠 transcript 里的
  最近 user entry 推断。
- 主屏 operation center、右栏、副屏和 ops screen 继续共用同一个标准化任务模型。
- 刷新 `0.1.10` 命名的确定性 TUI 截图。
- plugin、skill、MCP、ACP 先作为明确设计方向保留；除非已经接入统一 runtime 和权限路径，
  否则不在用户说明里宣称可写可执行能力。
- 继承 0.1.9 的 release gate：format、clippy、测试、TUI regression、package smoke、
  可选 DeepSeek smoke、GitHub release asset 验证和 Homebrew 验证。

## 验收标准

- 在 TUI 提交 provider 请求后，界面立刻出现 `thinking` 状态，并带 provider、model、
  workspace 证据。
- provider 返回后，pending task 自动清除，由 transcript 中真实的 tool、approval、test、
  diff 或 assistant task 接管状态。
- 主屏 operation center 在 provider 完成前也能显示 live task summary 和 evidence。
- 右栏和副屏读取同一套 `AgentTask` 投影。
- README 和用户指南引用 `0.1.10` 截图与安装资产。
- 发布完成前，状态文档记录本地验证和发布后验证证据。

## 验证

至少运行：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.10 --deepseek --out-dir /tmp/robocode-0110-release-smoke-full
```

发布后运行：

```bash
scripts/release-smoke.sh --version 0.1.10 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-0110-postpublish-check
```

## 延后

- 完整 ACP host，用于接入更多第三方 coding agent。
- 通过统一权限路径执行的 mutation-capable MCP 工具。
- 用户可安装 plugin/skill 的完整生命周期命令；当前只保留可见性和规划说明。
