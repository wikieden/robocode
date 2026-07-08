# Viden 0.1.30 计划 - 最终 Zero-Bug TUI Gate

English version: [release-0.1.30-plan.md](release-0.1.30-plan.md)

`0.1.30` 是 0.1.x 的最终 zero-bug gate（final zero-bug gate）。它收口早先 0.1.26 范围里剩余的
Mode/Permission UI surface，并把剩余 TUI 稳定性要求变成 prepublish 的硬门槛。

## 目标

- 将 topbar、composer、status、mode、permission、provider/model 标签接到真实
  runtime evidence，而不是静态展示文字。
- provider turn、plan mode、approval、tool job 活跃时，composer 仍然可用：输入、
  排队、取消和历史滚动都不能锁死。
- Plan mode 是 planning-only 模式：允许产品需求、架构、实现方案和计划；file、shell、
  Git、workflow、memory、task mutation 必须继续阻断。
- 每次发布必须运行 live DeepSeek 开发场景 smoke，并记录 token、耗时、预估费用和失败分类。
- 发布前必须通过最终 zero-bug gate：既有 deterministic TUI 截图，也有真实 macOS
  Terminal 和 iTerm2 证据。

## 发布 Gate

发布 `0.1.30` 前必须运行：

```bash
export VIDEN_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30
scripts/final-zero-bug-contract-smoke.sh
scripts/final-zero-bug-smoke.sh /tmp/viden-0130-final-zero-bug
scripts/release-gate.sh --version 0.1.30 --phase prepublish --out-dir /tmp/viden-0130-release-gate
```

`0.1.30` 的 prepublish gate 会自动运行 `scripts/final-zero-bug-smoke.sh`。缺少
Terminal/iTerm2 人工截图时，最终 zero-bug gate 必须失败。

发布 GitHub assets 并同步 Homebrew 后：

```bash
scripts/release-gate.sh --version 0.1.30 --phase postpublish --out-dir /tmp/viden-0130-release-gate
```

`0.1.30` 只有在以下条件全部满足后才算完成：

- P0/P1 TUI backlog 为 0；
- deterministic TUI regression、final zero-bug smoke、plan-mode smoke、
  daily-loop smoke、RC TUI stability smoke 全部通过；
- prepublish gate 通过，并包含 live DeepSeek token/cost 证据；
- GitHub Release `v0.1.30` 已发布并包含 assets 和 checksums；
- `wikieden/homebrew-tap` 指向 `0.1.30`；
- postpublish validation 通过。
