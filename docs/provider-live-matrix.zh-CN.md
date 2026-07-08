# Provider 真实兼容性矩阵

## 目的

这个矩阵跟踪 Viden 可通过内置 descriptor 创建的 OpenAI-compatible gateway
providers。它把 descriptor 覆盖和真实 API 验证分开记录，避免在没有凭据和网络验证时误称某个 provider 已经通过真实兼容性测试。

## 当前状态

Descriptor 覆盖已经实现，并由离线测试覆盖。真实 API 验证需要 provider 账号、模型、凭据和网络访问，因此刻意保持为手工执行。

内置模型目录是静态 picker 快照，不代表真实 API 已验证。DashScope Coding Plan
和 Token Plan 条目分别按阿里云对应模型白名单维护，并使用各自专用 Base URL，而不是普通 DashScope OpenAI-compatible endpoint。

| Provider | API base | API key env | API base env | Descriptor default model | Picker model snapshot | Streaming | Native tools | Live API status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `openrouter` | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | `OPENROUTER_API_BASE` | 无；需要显式传 model | dynamic gateway examples only | yes | yes | not recorded |
| `groq` | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` | `GROQ_API_BASE` | `openai/gpt-oss-20b` | `openai/gpt-oss-120b`, `openai/gpt-oss-20b`, `qwen/qwen3-32b` | yes | yes | not recorded |
| `mistral` | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` | `MISTRAL_API_BASE` | `mistral-medium-latest` | `mistral-medium-latest`, `mistral-large-latest`, `codestral-latest`, `devstral-*` | yes | yes | not recorded |
| `together` | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` | `TOGETHER_API_BASE` | `openai/gpt-oss-20b` | `openai/gpt-oss-*`, `Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8` | yes | yes | not recorded |
| `kimi` | `https://api.moonshot.ai/v1` | `MOONSHOT_API_KEY` | `MOONSHOT_API_BASE` | `kimi-k2.5` | `kimi-k2.5`, `kimi-k2.6`, `kimi-latest`, `moonshot-v1-128k` | yes | yes | not recorded |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `DASHSCOPE_API_KEY` | `DASHSCOPE_API_BASE` | `qwen3.6-plus` | `qwen3.6-plus`, `qwen3.6-flash`, `qwen3-coder-*`, `qwen-plus-latest`, `qwen-flash`, `qwq-plus` | yes | yes | not recorded |
| `dashscope-coding-plan` | `https://coding.dashscope.aliyuncs.com/v1` | `DASHSCOPE_CODING_PLAN_API_KEY` | `DASHSCOPE_CODING_PLAN_API_BASE` | `qwen3.6-plus` | `qwen3.6-plus`, `qwen3.5-plus`, `qwen3-max-2026-01-23`, `qwen3-coder-next`, `qwen3-coder-plus`, `kimi-k2.5`, `glm-5`, `glm-4.7`, `MiniMax-M2.5` | yes | yes | not recorded |
| `dashscope-coding-plan-anthropic` | `https://coding.dashscope.aliyuncs.com/apps/anthropic` | `DASHSCOPE_CODING_PLAN_API_KEY` | `DASHSCOPE_CODING_PLAN_ANTHROPIC_API_BASE` | `qwen3.6-plus` | `qwen3.6-plus`, `qwen3.5-plus`, `qwen3-max-2026-01-23`, `qwen3-coder-next`, `qwen3-coder-plus`, `kimi-k2.5`, `glm-5`, `glm-4.7`, `MiniMax-M2.5` | yes | yes | not recorded |
| `dashscope-tokenplan` | `https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1` | `DASHSCOPE_API_KEY` | `DASHSCOPE_TOKENPLAN_API_BASE` | `qwen3.6-plus` | `qwen3.7-max`, `qwen3.6-plus`, `qwen3.6-flash`, `qwen-image-2.0*`, `wan2.7-image*`, `deepseek-v4-*`, `deepseek-v3.2`, `kimi-k2.6`, `kimi-k2.5`, `glm-5.1`, `glm-5`, `MiniMax-M2.5` | yes | yes | not recorded |
| `dashscope-tokenplan-anthropic` | `https://token-plan.cn-beijing.maas.aliyuncs.com/apps/anthropic` | `DASHSCOPE_API_KEY` | `DASHSCOPE_TOKENPLAN_ANTHROPIC_API_BASE` | `deepseek-v4-flash` | `qwen3.7-max`, `qwen3.6-plus`, `qwen3.6-flash`, `deepseek-v4-*`, `deepseek-v3.2`, `kimi-k2.6`, `kimi-k2.5`, `glm-5.1`, `glm-5`, `MiniMax-M2.5` | yes | yes | not recorded |
| `zhipu` | `https://open.bigmodel.cn/api/paas/v4` | `ZHIPU_API_KEY` | `ZHIPU_API_BASE` | `glm-4.6` | `glm-5`, `glm-4.7`, `glm-4.6`, `glm-4.5` | yes | yes | not recorded |
| `volcengine` | `https://ark.cn-beijing.volces.com/api/v3` | `ARK_API_KEY` | `ARK_API_BASE` | `doubao-seed-2.0-code` | `doubao-seed-2.0-code`, `doubao-seed-2.0`, `doubao-seed-1.6`, `deepseek-v3.2`, `ark-code-latest` | yes | yes | not recorded |

## 验证命令

使用 ignored CLI smoke test 逐个验证 provider。建议把 provider-specific key env
传给 `VIDEN_LIVE_API_KEY`，这样不同 provider 的命令形态保持一致。

```bash
VIDEN_LIVE_PROVIDER=openrouter \
VIDEN_LIVE_MODEL='<provider-model>' \
VIDEN_LIVE_API_KEY="$OPENROUTER_API_KEY" \
cargo test -p viden-cli selected_live_provider_generates_python_hello_world_from_natural_language -- --ignored
```

即使 provider 有 descriptor default model，记录真实验证时也应该显式设置
`VIDEN_LIVE_MODEL`，这样结果会明确写出实际测试的模型。没有 descriptor default
model 的 provider 必须显式传 model。

如果 provider 需要非默认 endpoint，额外加入：

```bash
VIDEN_LIVE_API_BASE='<provider-api-base>'
```

## 记录结果

真实运行成功或失败后，更新 `Live API status` 单元格，写明：

- `YYYY-MM-DD` 格式的具体日期
- 使用的准确模型
- 已检查的表面，例如 `tool_call`、`streaming` 或 `text`
- provider 可访问但不兼容时的失败模式

不要只凭离线测试把 provider 标为 verified。
