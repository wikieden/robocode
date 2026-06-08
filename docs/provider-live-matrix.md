# Provider Live Compatibility Matrix

## Purpose

This matrix tracks the OpenAI-compatible gateway providers that RoboCode can
create from built-in descriptors. It separates descriptor coverage from live API
verification so the repository does not imply a provider was tested without
credentials and network access.

## Current Status

Descriptor coverage is implemented and covered by offline tests. Live API
verification is intentionally manual because it requires real provider accounts,
models, credentials, and network access.

The built-in model catalog is a static picker snapshot, not a live verification
claim. The DashScope Coding Plan and Token Plan entries follow Alibaba Cloud's
separate model allowlists and dedicated base URLs rather than the standard
DashScope OpenAI-compatible endpoint.

| Provider | API base | API key env | API base env | Descriptor default model | Picker model snapshot | Streaming | Native tools | Live API status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `openrouter` | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | `OPENROUTER_API_BASE` | none; pass a model | dynamic gateway examples only | yes | yes | not recorded |
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

## Verification Commands

Use the ignored CLI smoke test to verify one provider at a time. Prefer setting
`ROBOCODE_LIVE_API_KEY` from the provider-specific key env so the command shape
stays the same across providers.

```bash
ROBOCODE_LIVE_PROVIDER=openrouter \
ROBOCODE_LIVE_MODEL='<provider-model>' \
ROBOCODE_LIVE_API_KEY="$OPENROUTER_API_KEY" \
cargo test -p robocode-cli selected_live_provider_generates_python_hello_world_from_natural_language -- --ignored
```

For providers with a descriptor default model, `ROBOCODE_LIVE_MODEL` should
still be set explicitly when recording live verification so the result names the
exact model that was tested. For providers without a descriptor default model,
the explicit model is required.

If the provider needs a non-default endpoint, add:

```bash
ROBOCODE_LIVE_API_BASE='<provider-api-base>'
```

## Recording Results

When a live run succeeds or fails, update the `Live API status` cell with:

- the exact date in `YYYY-MM-DD` format
- the model used
- the checked surfaces, such as `tool_call`, `streaming`, or `text`
- the failure mode if the provider was reachable but incompatible

Do not mark a provider as verified from offline tests alone.
