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

| Provider | API base | API key env | API base env | Descriptor default model | Streaming | Native tools | Live API status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `openrouter` | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | `OPENROUTER_API_BASE` | none; pass a model | yes | yes | not recorded |
| `groq` | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` | `GROQ_API_BASE` | `openai/gpt-oss-20b` | yes | yes | not recorded |
| `mistral` | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` | `MISTRAL_API_BASE` | `mistral-medium-latest` | yes | yes | not recorded |
| `together` | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` | `TOGETHER_API_BASE` | `openai/gpt-oss-20b` | yes | yes | not recorded |
| `kimi` | `https://api.moonshot.ai/v1` | `MOONSHOT_API_KEY` | `MOONSHOT_API_BASE` | none; pass a model | yes | yes | not recorded |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `DASHSCOPE_API_KEY` | `DASHSCOPE_API_BASE` | `qwen-plus` | yes | yes | not recorded |
| `zhipu` | `https://open.bigmodel.cn/api/paas/v4` | `ZHIPU_API_KEY` | `ZHIPU_API_BASE` | `glm-4.6` | yes | yes | not recorded |
| `volcengine` | `https://ark.cn-beijing.volces.com/api/v3` | `ARK_API_KEY` | `ARK_API_BASE` | none; pass a model | yes | yes | not recorded |

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
