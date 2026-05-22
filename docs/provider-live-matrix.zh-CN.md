# Provider 真实兼容性矩阵

## 目的

这个矩阵跟踪 RoboCode 可通过内置 descriptor 创建的 OpenAI-compatible gateway
providers。它把 descriptor 覆盖和真实 API 验证分开记录，避免在没有凭据和网络验证时误称某个 provider 已经通过真实兼容性测试。

## 当前状态

Descriptor 覆盖已经实现，并由离线测试覆盖。真实 API 验证需要 provider 账号、模型、凭据和网络访问，因此刻意保持为手工执行。

| Provider | API base | API key env | API base env | Descriptor default model | Streaming | Native tools | Live API status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `openrouter` | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | `OPENROUTER_API_BASE` | 无；需要显式传 model | yes | yes | not recorded |
| `groq` | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` | `GROQ_API_BASE` | `openai/gpt-oss-20b` | yes | yes | not recorded |
| `mistral` | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` | `MISTRAL_API_BASE` | `mistral-medium-latest` | yes | yes | not recorded |
| `together` | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` | `TOGETHER_API_BASE` | `openai/gpt-oss-20b` | yes | yes | not recorded |
| `kimi` | `https://api.moonshot.ai/v1` | `MOONSHOT_API_KEY` | `MOONSHOT_API_BASE` | 无；需要显式传 model | yes | yes | not recorded |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `DASHSCOPE_API_KEY` | `DASHSCOPE_API_BASE` | `qwen-plus` | yes | yes | not recorded |
| `zhipu` | `https://open.bigmodel.cn/api/paas/v4` | `ZHIPU_API_KEY` | `ZHIPU_API_BASE` | `glm-4.6` | yes | yes | not recorded |
| `volcengine` | `https://ark.cn-beijing.volces.com/api/v3` | `ARK_API_KEY` | `ARK_API_BASE` | 无；需要显式传 model | yes | yes | not recorded |

## 验证命令

使用 ignored CLI smoke test 逐个验证 provider。建议把 provider-specific key env
传给 `ROBOCODE_LIVE_API_KEY`，这样不同 provider 的命令形态保持一致。

```bash
ROBOCODE_LIVE_PROVIDER=openrouter \
ROBOCODE_LIVE_MODEL='<provider-model>' \
ROBOCODE_LIVE_API_KEY="$OPENROUTER_API_KEY" \
cargo test -p robocode-cli selected_live_provider_generates_python_hello_world_from_natural_language -- --ignored
```

即使 provider 有 descriptor default model，记录真实验证时也应该显式设置
`ROBOCODE_LIVE_MODEL`，这样结果会明确写出实际测试的模型。没有 descriptor default
model 的 provider 必须显式传 model。

如果 provider 需要非默认 endpoint，额外加入：

```bash
ROBOCODE_LIVE_API_BASE='<provider-api-base>'
```

## 记录结果

真实运行成功或失败后，更新 `Live API status` 单元格，写明：

- `YYYY-MM-DD` 格式的具体日期
- 使用的准确模型
- 已检查的表面，例如 `tool_call`、`streaming` 或 `text`
- provider 可访问但不兼容时的失败模式

不要只凭离线测试把 provider 标为 verified。
