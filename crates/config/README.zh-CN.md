# viden-config

## 目的

`viden-config` 负责确定性的运行时配置解析。

## 不负责

- Provider 执行。
- 除携带已选 mode 外，不做权限策略评估。
- Session 或 workflow 持久化。

## 公共接口

- `CliOverrides`
- `ResolvedConfig`
- `load_config`

## Provider Plugin 目录

Provider plugin 目录现在进入与 provider/model/API 设置相同的确定性配置链路：

- 配置文件中使用 `provider_plugin_dirs = ["./plugins"]`。
- 环境变量使用 `ROBOCODE_PROVIDER_PLUGIN_DIRS`，格式为平台 path-list。
- CLI 使用可重复的 `--provider-plugin-dir <dir>`。

解析后的值保存在 `ResolvedConfig::provider_plugin_dirs`，CLI 会用它构造当前
`ProviderHost`，使动态 provider 来源可见、可审计。

## 不变量

- 优先级为 `CLI > environment > project config > global config > defaults`。
- 配置加载只读文件/env，不产生执行副作用。
- 摘要不能泄露原始 API key。
- Provider plugin 目录解析只产生数据；真正 plugin loading 属于
  `viden-provider`。

## `.ref` 对齐

对齐 `.ref` 的分层 settings 行为，但不引入 managed settings 或 analytics。

## 测试

```bash
cargo test -p viden-config
```
