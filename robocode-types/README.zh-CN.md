# robocode-types

## 目的

`robocode-types` 负责所有 crate 共享的领域契约。

## 不负责

- 运行时行为。
- 持久化实现。
- Provider/tool 执行。

## 公共接口

- Message、tool、provider、permission、transcript、session、runtime、task、memory、resume context 类型。
- 公开 enum 的 CLI-name helpers。
- 轻量 transcript encode/decode helpers。

## 不变量

- 类型保持行为中立。
- 序列化形状变更必须谨慎；JSONL logs 和 adapters 依赖它。
- modes/statuses 的公开 CLI 名称应保持稳定。

## `.ref` 对齐

收集与 `.ref` command、log、permission、task、id 类型对应的契约。

## 测试

```bash
cargo test -p robocode-types
```
