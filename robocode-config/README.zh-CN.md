# robocode-config

## 目的

`robocode-config` 负责确定性的运行时配置解析。

## 不负责

- Provider 执行。
- 除携带已选 mode 外，不做权限策略评估。
- Session 或 workflow 持久化。

## 公共接口

- `CliOverrides`
- `ResolvedConfig`
- `load_config`

## 不变量

- 优先级为 `CLI > environment > project config > global config > defaults`。
- 配置加载只读文件/env，不产生执行副作用。
- 摘要不能泄露原始 API key。

## `.ref` 对齐

对齐 `.ref` 的分层 settings 行为，但不引入 managed settings 或 analytics。

## 测试

```bash
cargo test -p robocode-config
```
