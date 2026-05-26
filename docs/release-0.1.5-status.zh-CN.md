# RoboCode 0.1.5 发布状态

最后更新：2026-05-26

## 目标

`0.1.5` 是编程体验版本。目标是让 TUI cockpit 足够适合日常编程闭环：
理解任务、审批 mutation、检查 diff 和 test evidence、监督 lanes，并产出可安装的
release artifacts。

## 阶段映射

1. TUI 交互稳定性：local smoke 已通过。
2. 编程闭环 evidence：local smoke 已通过。
3. lane operator workflow：shell 和 tmux lane gates 的 local smoke 已通过。
4. provider compatibility：DeepSeek V4 Flash live smoke 已通过。
5. release artifact validation：GitHub Actions artifact run 已通过。

## Candidate 证据

- workspace package version 从 `0.1.4` 升到 `0.1.5`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.5`。
- GitHub release workflow 默认 tag 改为 `v0.1.5`。
- README 安装示例改为 `v0.1.5`。
- README 系统截图保留人工整理过的版式，可见版本号更新为 `0.1.5`。
- 本地 release smoke 通过 `scripts/release-smoke.sh` 脚本化；脚本会在一个
  evidence 目录中收集 logs、生成的 TUI previews、fallback CLI smoke、lane smoke
  和 host package smoke。
- 带 DeepSeek 真实 provider validation 的完整本地 release smoke 已通过：
  `scripts/release-smoke.sh --version 0.1.5 --deepseek --out-dir /tmp/robocode-015-release-smoke-deepseek-local`。
- Evidence 目录：
  `/tmp/robocode-015-release-smoke-deepseek-local`。
- DeepSeek V4 Flash live smoke 已通过；transcript 中包含
  `robocode-deepseek-smoke-ok`。
- `aarch64-apple-darwin` host package smoke 已通过；解压后的二进制输出
  `robocode-cli 0.1.5`。
- macOS arm64 archive SHA-256：
  `734fe4a266178946b871e10a847ec8ac1f50642e270f708d8446fe5a81315e78`。
- GitHub Actions release artifact validation 已以 `upload_to_release=false`
  跑通全部配置目标：`aarch64-apple-darwin`、`x86_64-apple-darwin`、
  `x86_64-unknown-linux-gnu` 和 `x86_64-pc-windows-msvc`。
  Run: https://github.com/wikieden/robocode/actions/runs/26430970204。

## 验证门禁

把 version bump 推到 `main` 后，发布 `v0.1.5` 前运行：

```bash
scripts/release-smoke.sh --version 0.1.5 --skip-package --deepseek --github-actions
```

最终状态更新需要记录：

- 上传 artifacts 的最终 release workflow run URL；
- 已发布 release URL 和 artifact 列表。

## 当前发现

### P0

- 继续关注 right rail 和 side screens 的视觉对齐回归；frame glyphs 现在已经使用稳定
  frame 色，不再继承行内语义高亮颜色。

### P1

- `/lane` 仍是 TUI/runtime 命令面；普通 REPL 仍会把 `/lane` 视为 unknown command。
  这在 `0.1.5` 中是有意选择，release notes 需要明确说明。

### P2

- 完整 cursor-addressed terminal replay 继续后置。
- inline conflict editor 继续后置。
- 更多外部 coding-tool templates 继续按真实需求推进。

## Release 结果

`v0.1.5` 尚未发布。最终 release workflow 上传 artifacts 后，需要更新本页。
