# GUI D1 Cockpit 集成检查点

英文版：[checkpoints.md](checkpoints.md)

日期：2026-07-27

本证据只描述本地候选，不是已发布、签名、公证、push、merge、tag 或 live
provider 认证的 release。

## 候选线

| 项目 | SHA / 路径 |
| --- | --- |
| 基线 `origin/main` | `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core `0.3.5` 来源 / merge | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` / `76f7f8e3a84ff38846023dda7dead0c50bfb2b68` |
| TUI `0.3.3` 来源 / merge | `6260f183d19da27e61fdf068d67a9c481c68d829` / `026736dc4c16b1d039b80e77b9fe8ff99788d51b` |
| GUI `0.1.0-rc.3` 来源 / merge | `1c44094dd29674e1cc585ff6c83302581440aeb0` / `864966d0677e9d958396fac150f4701b2d14b0a1` |
| Integration fix | `cb9baaf7ff212655d3b1ea8dd3cb4684ae40f7d0` |
| Fixture parity | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| Task 14 base | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| 分支 | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |

旧 `codex/d1-cockpit-integration` Core+GUI-only 线只保留为历史 blocked 证据；
上表才是当前候选。

## 确定性证据

| 命令 | 结果 |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS：精确 Core replay/hash、TUI render 与 GUI projection 证明 |
| `cargo test -p viden-types` | PASS，77 |
| `cargo test -p viden-runtime` | PASS，461 + 1 ignored |
| `cargo test -p viden-core` | PASS |
| `cargo test -p viden-tui` | PASS，269 + 1 API |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS，17 files / 248 tests |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | Core 0.3.5 extension count 修复后 PASS |
| `scripts/rc-tui-stability-smoke.sh` | 同一修复后 PASS |
| `cargo fmt --all -- --check` | PASS |
| `npm --prefix apps/gui test -- tests/agent_menu.spec.ts tests/d1_cockpit.spec.ts --run` | Native smoke 修复后 PASS，69 tests |
| `git diff --check` | PASS |

Canonical parity fixture 含 22 个事件，终点为
`fixture:interaction-closed-loop@22`，Core view SHA-256 为
`46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0`。
详见
[fixture-parity.zh-CN.md](../native-acp-interaction/fixture-parity.zh-CN.md)。

## App 构建证据

`npm --prefix apps/gui run tauri -- build --bundles app` 成功完成：

- App：`target/release/bundle/macos/Viden.app`
- executable：
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- `CFBundleIdentifier`：`dev.viden.gui`
- `CFBundleExecutable`：`viden-gui`
- `CFBundleShortVersionString`：`0.1.0-rc.3`
- executable size：`27,830,256` bytes
- signature：ad-hoc linker-signed，无 TeamIdentifier

未运行项目 distribution-signing 或公证门禁。

## Native Smoke 边界

状态：**PASS**。

Smoke fixture 是 `/tmp/viden-native-smoke.Hmd3ak`；它开始时是仅包含一个已提交
README 的干净 Git 仓库。创建仓库内 fixture worktree 后，这些 fixture 路径会显示
为 untracked 状态。独立 App 在 `1229x768`、fallback `test-local` 下完成了原生
操作，实际覆盖：

- 持久 Welcome 与 Open Project；
- 项目驾驶舱的零 Lane 状态；
- 默认选择 Native Viden Agent 的紧凑 New Lane 菜单；
- Core 权威 preview、应用内授权、worktree 创建与精确 Lane 创建；
- 一个 Lane 对应一个 Native execution owner；
- 可编辑 composer、保留并提交首个任务，以及确认后续输入。

Fallback provider 在 transcript 中投影 typed `Unavailable`，因此本检查点只声明
命令已确认和输入可操作，不声明有意义的 assistant answer 或 live-provider
推理。离线运行中的 ACP discovery 仅显示 unavailable。观察到的界面是英文与
`aurora/dark`；未暴露 locale 或 skin 配置入口，因此可配置性仍未验证。

原生证据保存在 [native-smoke](native-smoke/)：

| 文件 | 观察结果 |
| --- | --- |
| `01-welcome.jpeg` | 持久 Welcome shell |
| `02-zero-lane.jpeg` | 打开项目后的零 Lane 状态 |
| `03-new-lane.jpeg` | 紧凑 New Lane 菜单 |
| `04-approval-obscured-before-fix.jpeg` | 修复前缺陷证据：Lane rail 遮挡授权操作 |
| `04-approval-after-fix.jpeg` | 修复后授权全宽显示，Once、Repo paths、Deny 均无遮挡 |
| `05-lane-created-after-fix.jpeg` | 鼠标点击 Once 后创建精确 Lane/worktree 与 Native owner |
| `06-follow-up-confirmed-unavailable.jpeg` | 可编辑 follow-up 已确认；fallback response 仍为 unavailable |

原生运行暴露且当前候选修复三个闭环缺陷：New Lane overlay 被 Lane rail 裁切、
Create 持续等待而没有把控制权交给交互授权，以及 Lane 注册前无法通过精确 pending
Create owner 投影授权。对重建 App 的最终纯鼠标复验已通过：授权出现时 Lane rail
自动收起，所有授权操作无遮挡，并通过真实坐标点击 `Y · Once`，继续得到精确 Lane、
branch、worktree、Native owner 与保留的首个任务提交。
