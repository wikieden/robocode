# GUI 0.1.0-rc.3 D1 Cockpit 集成状态

英文版：[release-gui-0.1.0-rc.3-status.md](release-gui-0.1.0-rc.3-status.md)

日期：2026-07-24

这是本地、未发布的 D1 cockpit 集成检查点。它不是 tag、push、main merge、签名构建、公证构建、Homebrew 更新或 live provider 认证。

## 已集成输入

| 字段 | 值 |
| --- | --- |
| 分支 | `codex/d1-cockpit-integration` |
| Worktree | `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-integration` |
| 基线 | `origin/main` at `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core 来源 | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core merge commit | `6d0094a11cc64c097a2f48ee6122ec8bc95a2d23` |
| GUI 来源 | `4cb2a498c5b091f62f554ff407acdf162f96cc1e` |
| GUI merge commit | `d27dc09fd230e3c2fb3ae79fbf3d10a45f400226` |
| Core 版本 | `0.3.5` |
| GUI 版本 | `0.1.0-rc.3` |
| TUI 版本 | `0.3.3`，本任务未合并 TUI 分支 |

## 门禁结果

| 门禁 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test -p viden-types` | PASS | 77 passed |
| `cargo test -p viden-runtime` | 重新运行后 PASS | 最终重跑：461 passed, 1 ignored。首次全量运行在 `acp_async_job_can_be_cancelled_by_pid` 上失败一次；单测隔离运行和完整重跑均通过。 |
| `cargo test -p viden-core` | PASS | unit、CoreClient、frontend contract、host services 和 workspace identity 测试通过；3 个手动 fixture refresh 测试 ignored。 |
| `cargo test -p viden-gui` | PASS | 82 个 GUI Rust 测试通过，覆盖 adapter、D11、D1、D4、D6、permission、reconnect 和 virtualization。 |
| `npm --prefix apps/gui ci` | PASS | 0 vulnerabilities。 |
| `npm --prefix apps/gui test -- tests/d1_cockpit.spec.ts --run` | PASS | native-menu availability 修复后，54 个 D1 focused tests 通过。 |
| `npm --prefix apps/gui test -- --run` | PASS | native-menu availability 修复后，17 个文件、239 个测试通过。 |
| `npm --prefix apps/gui run build` | PASS | native-menu availability 修复后，TypeScript 检查和 Vite production build 通过。 |
| `bash scripts/native-acp-fixture-parity.sh` | BLOCKED | Core+GUI-only 集成中缺少该脚本；本任务未合并 TUI 分支。 |
| `bash scripts/check-dependency-boundaries.sh` | PASS | exit 0。 |
| `cargo test --workspace --quiet` | BLOCKED | `viden-tui` 不能编译到已合并 Core facade；代表性错误包括移除的 `viden_core` 根导出、移除的 `ApprovalResponse.approved` 字段，以及旧字符串形态的 `AgentTaskRecord`/`AgentLaneRecord` 字段。 |
| Manifest/hash integrity | PASS | `d1-main-cockpit.json` 和 rc.3 截图 hash 与 `apps/gui/manifests/0.1.0-rc.3.toml` 一致。 |
| `npm --prefix apps/gui run tauri build` | PASS | native-menu availability 修复后，产出 `target/release/bundle/macos/Viden.app` 与 `target/release/bundle/dmg/Viden_0.1.0-rc.3_aarch64.dmg`。 |

## Native App Smoke

已构建并启动精确的未签名 app bundle：

- app path：`target/release/bundle/macos/Viden.app`
- executable：`Contents/MacOS/viden-gui`
- bundle id：`dev.viden.gui`
- version：`0.1.0-rc.3`
- signature：ad-hoc，无 TeamIdentifier
- first launched PID：`49949`

本地进程级启动显示 PID `49949` 来自精确的集成 bundle。由于
`.worktrees/native-acp-integration` 中已有旧 Viden 进程以相同产品进程名运行，
第一次桌面控制路径无法为新 PID 区分出可见 window。

随后独立 native-app desktop control 验证了集成 bundle 的可达 UI 状态：
persistent Welcome shell、native Open Project folder dialog、选择已有安全 temp
project，以及 D1 shell retention 均通过。Lanes rail 可以打开并显示紧凑的
`+ New Lane` popup。

本检查点已应用修复：`apps/gui/src/components/agent_menu.ts` 现在只要 Core
workspace eligibility 允许 Lane creation，就保持 native `Viden Agent` 可用；ACP
选项仍由 ACP readiness 决定。修复后 focused 和 full GUI npm suite 均通过。

修复后 native smoke：New Lane menu 可以 resolve。`Viden Agent` enabled 且可选；
`Codex` Ready；`Kiro` Ready；`Claude` disabled，并显示 initialize probe failure。
选择 `Viden Agent` 会关闭菜单，但 5 秒后没有新 Lane 出现。当前 native smoke
阻塞点是 native Lane creation/Core owner binding。已有选中 Lane 也仍没有唯一
Core execution owner，因此发送保持 disabled。

## 决策

本检查点是可 review 的 Core+GUI 集成候选，但不是完整 D1 cockpit 认证。一个
scoped GUI 修复已让 native Lane creation 在 ACP probing 期间保持可用，但剩余
阻塞不属于这个本地菜单状态修复：TUI/Core workspace 编译 drift、缺失 native ACP
parity script、选择 `Viden Agent` 后的 native Lane creation/Core owner binding，
以及已有选中 Lane 没有唯一 Core execution owner。

下一步安全动作：串行处理 TUI/Core 兼容迁移，或在 native-menu 修复后授权重跑
fresh native smoke，然后重跑 workspace、parity 与 native desktop gates。
