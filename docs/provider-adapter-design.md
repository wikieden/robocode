# Provider Adapter Design

RoboCode treats every model provider as a capability profile plus an adapter, not
as a plain OpenAI-compatible URL. Many providers accept a Chat Completions-shaped
request, but they differ in model naming, tool-call replay, reasoning fields,
authentication, request-size limits, and streaming event formats.

## Current Findings

- DeepSeek uses an OpenAI-compatible chat surface for `deepseek-chat`,
  `deepseek-reasoner`, `deepseek-v4-flash`, and `deepseek-v4-pro`, but its tool
  call replay can require provider-specific reasoning content handling. It also
  exposes Anthropic-compatible endpoints, so the adapter must keep protocol
  family separate from provider identity.
- OpenAI should be modeled as a Responses/Chat-capable provider with both API
  key and future web-login modes. Tool calling, streaming, structured outputs,
  and reasoning controls should be declared as capabilities instead of inferred
  from provider id strings.
- Anthropic uses Messages API semantics: system prompt placement, tool use/tool
  result blocks, max token handling, and extended thinking differ from OpenAI.
- OpenRouter, Groq, Mistral, Together, Kimi, Qwen, Zhipu, and Volcengine are
  mostly OpenAI-compatible at the wire level, but each needs its own model
  catalog, base URL, optional headers, tool support flags, and error mapping.
- DashScope Token Plan and Coding Plan are not generic Qwen aliases. They are
  separate package surfaces with their own model white lists and OpenAI- or
  Anthropic-compatible endpoints.
- Local Ollama should remain a local provider with no key requirement and a more
  conservative feature profile.

## Adapter Contract

Each provider descriptor should eventually declare:

- identity: provider id, display name, protocol family, default base URL;
- auth: API key env var, web-login support, local/no-key mode;
- model catalog: known models, default model, package-specific active models,
  favorite/recent order, and whether models can be fetched dynamically;
- request envelope: endpoint path, message rendering, system-prompt policy,
  max output tokens, temperature support, tool schema support, and custom
  provider fields;
- tool replay: whether assistant tool-call content may be null, whether
  reasoning content must be replayed, and how tool result blocks are encoded;
- reasoning: provider-specific fields such as reasoning effort, thinking mode,
  or unsupported reasoning flags;
- streaming: event parsing, partial-text deltas, tool-call deltas, and usage
  chunk extraction;
- limits: soft request body budget, context window hints, max output tokens, and
  retry/compaction behavior;
- errors: auth, missing model, request too large, context overflow, rate limit,
  compatibility, and transient network failures.

## Implementation Plan

1. **Request view compaction**
   Build a budgeted provider request view from the durable transcript. Keep the
   latest user turn and current tool-call pair structured, but summarize older
   history. This prevents provider-side 413 failures while preserving the full
   JSONL transcript locally.

2. **Capability-driven rendering**
   Move request rendering decisions behind a provider capability profile:
   OpenAI, Anthropic, DeepSeek, DashScope package endpoints, and local Ollama
   should not share one implicit code path.

3. **Model catalogs**
   Keep static built-in catalogs as a safe offline baseline, then add optional
   provider-specific discovery for providers that expose model-list APIs. The
   TUI `/models` picker should show configured providers first, grouped by
   provider, with favorites and recent choices deduplicated.

4. **Provider setup**
   `/connect` configures provider credentials and endpoint settings in a real
   panel. After the provider is configured, the next step is choosing the
   default/active model for that provider. Configuration overlays must not start
   a session.

5. **Error recovery**
   Provider errors should map to actionable classes. For example, HTTP 413 is
   `request_too_large`, not a generic model failure; the next action is context
   compaction or a provider/model switch.

6. **Compatibility smoke**
   Add deterministic provider-render tests plus opt-in live smoke scripts for
   DeepSeek, DashScope Token Plan/Coding Plan, OpenRouter, OpenAI, Anthropic,
   and Ollama.

## Official Reference Links

- DeepSeek API: <https://api-docs.deepseek.com/zh-cn/>
- OpenAI API: <https://platform.openai.com/docs/api-reference>
- Anthropic Messages API: <https://docs.anthropic.com/en/api/messages>
- OpenRouter API: <https://openrouter.ai/docs/api-reference/overview>
- Groq API docs: <https://console.groq.com/docs/overview>
- Mistral API docs: <https://docs.mistral.ai/api/>
- Together API docs: <https://docs.together.ai/docs/introduction>
- Alibaba Cloud Model Studio Token Plan: <https://help.aliyun.com/zh/model-studio/token-plan-overview>
- Alibaba Cloud Model Studio Coding Plan: <https://help.aliyun.com/zh/model-studio/coding-plan>
- Ollama API: <https://github.com/ollama/ollama/blob/main/docs/api.md>
