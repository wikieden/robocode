# Viden 0.1.29 计划 - RC TUI 稳定性

English version: [release-0.1.29-plan.md](release-0.1.29-plan.md)

`0.1.29` 是 0.1.x 的 release-candidate 稳定性收口版本。本版本不增加新的 UI
surface，而是把剩余 P0/P1 TUI 稳定性预期收敛成可审计的发布 gate，为最终
zero-bug exit 做准备。

## 目标

- 冻结 0.1.x 功能扩张，只修 release-blocking TUI 稳定性问题。
- 增加 RC TUI stability smoke，证明已知 P0/P1 guardrails：fake-slow provider
  非阻塞、approval 非阻塞、streaming 不抢 scrollback、focus/paste repaint
  policy、composer terminal residue 过滤、provider/model setup picker 行为、
  LIVE WORK preview contract、synthetic planning 清理、deterministic TUI previews。
- 在 release status 中记录 P0/P1 TUI backlog 摘要。
- 显式处理 macOS Terminal/iTerm2 人工截图证据：release status 必须链接真实截图，
  或在最终 `0.1.30` zero-bug gate 前记录剩余人工证据风险。

## 发布 Gate

发布 `0.1.29` 前必须运行：

```bash
scripts/rc-tui-stability-contract-smoke.sh
scripts/rc-tui-stability-smoke.sh /tmp/viden-0129-rc-tui-stability
scripts/release-gate.sh --version 0.1.29 --phase prepublish --out-dir /tmp/viden-0129-release-gate
```

prepublish gate 必须包含 live DeepSeek 开发场景，并记录 token、耗时、预估费用和失败分类证据。

发布 GitHub assets 并同步 Homebrew 后：

```bash
scripts/release-gate.sh --version 0.1.29 --phase postpublish --out-dir /tmp/viden-0129-release-gate
```

`0.1.29` 只有在以下条件全部满足后才算完成：

- RC TUI stability smoke 通过，并记录 P0/P1 backlog 状态；
- deterministic TUI regression、plan-mode smoke、daily-loop smoke、lane
  operator smoke 通过；
- prepublish gate 通过，并包含 live DeepSeek smoke 证据；
- GitHub Release `v0.1.29` 已发布并包含 assets 和 checksums；
- `wikieden/homebrew-tap` 指向 `0.1.29`；
- postpublish validation 通过。
