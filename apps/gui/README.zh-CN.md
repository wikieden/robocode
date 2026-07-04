# viden-gui

`viden-gui` 是未来桌面 GUI 的应用边界。当前先作为最小 contract replay
脚手架存在，让 GUI 和 TUI 在真正开发界面之前先共享同一套 runtime facts。

## 目的

- 通过 `viden-core` 消费 `RuntimeSnapshot`、`RuntimeEvent`、
  `RuntimeCommand` 和 `RuntimeViewState`。
- 在视觉实现前提供 GUI fixture replay 测试。
- 不让 GUI 直接拥有 provider loop、tool execution、permission check、
  transcript state、workflow state 或 lane orchestration。

## 不负责

- Provider/model 执行。
- Tool 或 shell 执行。
- 权限决策。
- Transcript/session/workflow 持久化。
- Runtime 业务状态。

## 测试

```bash
cargo test -p viden-gui
```
