# robocode-core

## 目的

`robocode-core` 负责 `SessionEngine`：用户输入、slash commands、provider events、tool calls、permission checks、transcript 写入和 workflow commands 的共享运行路径。

## 不负责

- 具体模型协议。
- 工具实现细节。
- JSONL/SQLite 存储内部实现。
- Task 和 memory reducer 规则。

## 公共接口

- `SessionEngine`
- `EngineEvent`
- runtime/session/provider/Git/Web/task/memory 命令处理。
- Provider runtime commands：
  - `/provider` 显示当前 provider 实例。
  - `/provider list` 渲染当前 provider registry，并包含紧凑 provider
    compatibility flags。
  - `/provider doctor [id]` 渲染 provider diagnostics；传入 id 时会聚焦展示
    单个 provider 的 compatibility 要求。
  - `/provider reload` 重新加载 provider plugin descriptors，但不替换当前
    provider 实例。
  - `/provider use <id> [model]` 通过当前 registry 切换当前 provider 实例。

## 不变量

- Tool calls 和 mutating workflow commands 执行前必须通过 permission checks。
- Slash commands 写入 `TranscriptEntry::Command`。
- Provider reload 必须保持原子语义：失败时报告 diagnostics，并保留之前可用的
  registry。
- Provider switching 必须经过当前 `ProviderHost`，并更新 provider/model 的
  transcript metadata。
- `/task resume-context` 可以更新派生字段，但不能改变 task 业务状态。
- Transcript 审计能力必须保持。

## `.ref` 对齐

把 `.ref` 中 `main.tsx`、`commands.ts`、`Tool.ts`、permission types、task/session flows 的行为映射到 Rust 编排层。

## 测试

```bash
cargo test -p robocode-core
```
