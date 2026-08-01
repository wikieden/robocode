# GUI 0.1.0-rc.3 D1 Cockpit 集成状态

英文版：[release-gui-0.1.0-rc.3-status.md](release-gui-0.1.0-rc.3-status.md)

日期：2026-07-28

这是本地、未发布的集成候选。它不是 tag、push、main merge、签名或公证构建、
Homebrew 更新、release 或 live provider 认证。

## 当前候选

| 字段 | 值 |
| --- | --- |
| 分支 | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |
| 基线 | `origin/main` at `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core | `0.3.5`，来源 `f7fe1b31`，merge `76f7f8e3` |
| TUI | `0.3.3`，来源 `6260f183`，merge `026736dc` |
| GUI | `0.1.0-rc.3`，来源 `1c44094d`，merge `864966d0` |
| Fixture parity | `interaction-closed-loop`，22 个有序事件，PASS |

当前候选按 Core -> TUI -> GUI 的固定顺序从最新 `origin/main` 重建。原
`codex/d1-cockpit-integration` Core+GUI-only 检查点是历史 blocked 证据，不是
当前候选；它的 TUI 编译 drift、缺失 parity script 与 native Lane creation 结果
不得归到当前重建线。

## 确定性门禁

| 门禁 | 结果 |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS，Core、TUI、GUI 各一项精确证明 |
| `cargo test -p viden-types` | PASS，78 passed |
| `cargo test -p viden-runtime` | PASS，461 passed、1 ignored |
| `cargo test -p viden-core` | PASS，3 个手动 fixture refresh 测试 ignored |
| `cargo test -p viden-tui` | PASS，269 个 library 与 1 个 API test |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS，17 个文件、249 个测试 |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | 将过期 extension capability 断言从 15 对齐 Core 0.3.5 的 16 后 PASS |
| `scripts/rc-tui-stability-smoke.sh` | 同一 scoped gate 修复后 PASS |
| `cargo fmt --all -- --check` | PASS |

Parity 详情见
[fixture-parity.zh-CN.md](release-evidence/native-acp-interaction/fixture-parity.zh-CN.md)。

## 独立 macOS 构建

`npm --prefix apps/gui run tauri -- build --bundles app` 通过。

- bundle：`target/release/bundle/macos/Viden.app`
- executable：
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- bundle identifier：`dev.viden.gui`
- version：`0.1.0-rc.3`
- executable size：`27,832,064` bytes
- signature：ad-hoc linker-signed，无 TeamIdentifier

这不是 distribution-signed 候选；未运行项目签名和公证。

## Native App Smoke

状态：**限定本地闭环 PASS**。

独立 App 已通过 Computer Use 在 `1229x768` 下操作
`/tmp/viden-native-smoke.Hmd3ak`。该临时 Git 仓库开始时是干净状态，只有一个
已提交 README。纯鼠标路径覆盖 Welcome、Open Project、zero-Lane、选择内置
Viden Agent 的 New Lane、Core 权威 preview 与应用内授权、worktree/Lane 创建、
单一 Native execution owner、保留的首个任务提交，以及可编辑 follow-up 提交。
Lane rail 会在授权出现时自动收起，授权操作全部直接可见，并通过真实屏幕坐标点击
`Y · Once`。

Fallback `test-local` 已确认 Native 提交，但 typed user/assistant transcript
rows 仍明确显示为 `Unavailable`，因此不声称有意义的 Native assistant answer。
第二个干净 fixture `/tmp/viden-codex-acp-final.wYycYE` 使用本机已登录的 Codex
ACP adapter 完成实测：App 发现 Codex，创建并授权一个 Lane，把临时 Lane owner
提升为精确 ACP session owner，界面显示 `Route ACP`，并完成 session
`agent-session_1785242592994456000`；结果为 `end_turn`、无 tool call，精确回答
`GUI ACP OWNER OK`。这只认证当前本机 ChatGPT 登录下的 Codex ACP 路径，不认证
可移植凭证、Claude/Kiro 登录或 OpenAI provider release。

可观察到英文界面和 `aurora/dark`，但未暴露 locale 或 skin 配置入口，因此
可配置性仍是后续门禁。截图与精确边界记录在
[GUI D1 cockpit 检查点](release-evidence/gui-d1-cockpit/checkpoints.zh-CN.md)。

## 决策

确定性集成、standalone app build 与限定 native closed-loop 门禁通过。当前仍是
本地候选，而不是 distribution 或 live-provider 认证：fallback transcript
可见性、locale/skin 配置、DeepSeek/OpenAI provider 行为和 Claude/Kiro ACP
authentication 尚未认证；Codex ACP 仅认证当前本机 ChatGPT 登录。未执行
credential creation、push、merge、tag、签名、公证、Homebrew mutation、release
或 publication。
