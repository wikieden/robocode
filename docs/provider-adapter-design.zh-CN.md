# Provider 适配层设计

RoboCode 不能把所有模型供应商都当成“一个 OpenAI-compatible URL”。很多供应商的
请求形状相似，但模型命名、工具调用回放、reasoning 字段、鉴权方式、请求体限制、
流式事件格式都不同。因此 provider 应该由“能力描述 + 适配器”共同驱动。

## 当前结论

- DeepSeek 提供 OpenAI-compatible chat 接口，覆盖 `deepseek-chat`、
  `deepseek-reasoner`、`deepseek-v4-flash`、`deepseek-v4-pro` 等模型；
  但工具调用回放涉及 reasoning content 兼容处理。同时 DeepSeek 也有
  Anthropic-compatible endpoint，所以 provider 身份和协议族不能混为一谈。
- OpenAI 后续应按 Responses/Chat 能力建模，同时支持 API key 和未来网页登录。
  工具调用、流式输出、结构化输出、reasoning 参数都应显式声明为能力。
- Anthropic 使用 Messages API 语义；system prompt、tool use/tool result block、
  max token、extended thinking 都和 OpenAI 不同。
- OpenRouter、Groq、Mistral、Together、Kimi、Qwen、智谱、火山引擎通常是
  OpenAI-compatible，但仍然需要各自的模型目录、base URL、可选 headers、工具支持
  flags 和错误映射。
- DashScope Token Plan 和 Coding Plan 不是普通 Qwen 别名；它们是独立套餐入口，
  有自己的模型白名单，也分别提供 OpenAI-compatible 和 Anthropic-compatible endpoint。
- Ollama 保持本地 provider，不要求 key，能力声明更保守。

## 适配器契约

每个 provider descriptor 后续应声明：

- 身份：provider id、展示名、协议族、默认 base URL；
- 鉴权：API key 环境变量、网页登录支持、本地免 key 模式；
- 模型目录：已知模型、默认模型、套餐内激活模型、收藏/最近顺序、是否支持动态拉取；
- 请求封装：endpoint path、message 渲染、system prompt 策略、max output tokens、
  temperature 支持、工具 schema 支持、自定义 provider 字段；
- 工具回放：assistant tool-call content 是否可为 null、是否必须回放 reasoning content、
  tool result block 如何编码；
- reasoning：reasoning effort、thinking mode 或不支持的 reasoning 参数；
- 流式输出：文本 delta、工具调用 delta、usage chunk 的解析方式；
- 限制：请求体软预算、上下文窗口提示、最大输出 token、重试/压缩策略；
- 错误：鉴权、模型不存在、请求体过大、上下文溢出、限流、兼容性、临时网络错误。

## 实施计划

1. **Provider 请求视图压缩**
   从 durable transcript 构建有预算的临时 provider request view。保留最新用户输入和
   当前工具调用配对的结构，旧历史折叠成摘要。这样可以避免 provider-side 413，同时
   本地 JSONL transcript 不丢。

2. **能力驱动的请求渲染**
   把请求渲染决策放到 provider capability profile 后面。OpenAI、Anthropic、
   DeepSeek、DashScope 套餐 endpoint、本地 Ollama 不应共享一条隐式路径。

3. **模型目录**
   静态内置目录作为离线基线；支持模型列表 API 的 provider 再逐步加入动态发现。
   TUI `/models` 应优先展示已配置 provider，并按 provider 分组，收藏和最近记录去重。

4. **Provider 设置**
   `/connect` 应在真实面板里配置 provider credential 和 endpoint。配置完成后，再进入
   该 provider 的默认/激活模型选择。配置 overlay 不应被视为开始会话。

5. **错误恢复**
   provider 错误要映射到可执行分类。例如 HTTP 413 是 `request_too_large`，不是泛化
   模型错误；下一步应提示压缩上下文或切换 provider/model。

6. **兼容性 smoke**
   增加确定性的 provider-render 测试，以及可选 live smoke：DeepSeek、DashScope
   Token Plan/Coding Plan、OpenRouter、OpenAI、Anthropic、Ollama。

## 官方参考

- DeepSeek API：<https://api-docs.deepseek.com/zh-cn/>
- OpenAI API：<https://platform.openai.com/docs/api-reference>
- Anthropic Messages API：<https://docs.anthropic.com/en/api/messages>
- OpenRouter API：<https://openrouter.ai/docs/api-reference/overview>
- Groq API 文档：<https://console.groq.com/docs/overview>
- Mistral API 文档：<https://docs.mistral.ai/api/>
- Together API 文档：<https://docs.together.ai/docs/introduction>
- 阿里云百炼 Token Plan：<https://help.aliyun.com/zh/model-studio/token-plan-overview>
- 阿里云百炼 Coding Plan：<https://help.aliyun.com/zh/model-studio/coding-plan>
- Ollama API：<https://github.com/ollama/ollama/blob/main/docs/api.md>
