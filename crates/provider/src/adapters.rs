use crate::config::ProviderKind;
use crate::descriptor::{
    ProtocolFamily, ProviderAuthMode, ProviderCapabilities, ProviderCompatibility,
    ProviderDescriptor, ProviderEnvMappings,
};
use crate::http::{
    ANTHROPIC_API_BASE, DASHSCOPE_CODING_PLAN_ANTHROPIC_API_BASE, DASHSCOPE_CODING_PLAN_API_BASE,
    DASHSCOPE_TOKENPLAN_ANTHROPIC_API_BASE, DASHSCOPE_TOKENPLAN_API_BASE,
    DEEPSEEK_ANTHROPIC_API_BASE, DEEPSEEK_API_BASE, GROQ_API_BASE, KIMI_API_BASE, MISTRAL_API_BASE,
    OLLAMA_API_BASE, OPENAI_API_BASE, OPENROUTER_API_BASE, QWEN_API_BASE, TOGETHER_API_BASE,
    VOLCENGINE_API_BASE, ZHIPU_API_BASE,
};

#[derive(Debug, Clone, Copy)]
struct BuiltinProviderMetadata {
    kind: ProviderKind,
    provider_id: &'static str,
    display_name: &'static str,
    protocol_family: ProtocolFamily,
    default_api_base: Option<&'static str>,
    default_model: &'static str,
    known_models: &'static [&'static str],
    api_key_env: Option<&'static str>,
    api_base_env: Option<&'static str>,
    supports_streaming: bool,
    supports_native_tool_calling: bool,
    auth_modes: &'static [ProviderAuthMode],
}

const BUILTIN_PROVIDER_METADATA: &[BuiltinProviderMetadata] = &[
    BuiltinProviderMetadata {
        kind: ProviderKind::Anthropic,
        provider_id: "anthropic",
        display_name: "Anthropic",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some(ANTHROPIC_API_BASE),
        default_model: "claude-sonnet-4-5",
        known_models: &[
            "claude-opus-4-5",
            "claude-sonnet-4-5",
            "claude-haiku-4-5",
            "claude-sonnet-4",
        ],
        api_key_env: Some("ANTHROPIC_API_KEY"),
        api_base_env: Some("VIDEN_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::OpenAi,
        provider_id: "openai",
        display_name: "OpenAI",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OPENAI_API_BASE),
        default_model: "gpt-5.2",
        known_models: &["gpt-5.2", "gpt-5.2-codex", "gpt-5.1", "gpt-5", "gpt-4.1"],
        api_key_env: Some("OPENAI_API_KEY"),
        api_base_env: Some("VIDEN_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::WebLogin, ProviderAuthMode::ApiKey],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::OpenAiCompatible,
        provider_id: "openai-compatible",
        display_name: "OpenAI-Compatible",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OPENAI_API_BASE),
        default_model: "gpt-4o-mini",
        known_models: &["gpt-4o-mini", "gpt-4.1-mini", "gpt-5-mini"],
        api_key_env: Some("OPENAI_API_KEY"),
        api_base_env: Some("VIDEN_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::DeepSeek,
        provider_id: "deepseek",
        display_name: "DeepSeek",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(DEEPSEEK_API_BASE),
        default_model: "deepseek-v4-flash",
        known_models: &[
            "deepseek-v4-flash",
            "deepseek-v4-pro",
            "deepseek-chat",
            "deepseek-reasoner",
        ],
        api_key_env: Some("DEEPSEEK_API_KEY"),
        api_base_env: Some("DEEPSEEK_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::DeepSeekAnthropic,
        provider_id: "deepseek-anthropic",
        display_name: "DeepSeek Anthropic-Compatible",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some(DEEPSEEK_ANTHROPIC_API_BASE),
        default_model: "deepseek-v4-flash",
        known_models: &["deepseek-v4-flash", "deepseek-v4-pro"],
        api_key_env: Some("DEEPSEEK_API_KEY"),
        api_base_env: Some("DEEPSEEK_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::Ollama,
        provider_id: "ollama",
        display_name: "Ollama",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OLLAMA_API_BASE),
        default_model: "llama3.1",
        known_models: &["llama3.1", "llama3.2", "qwen2.5-coder", "deepseek-r1"],
        api_key_env: None,
        api_base_env: Some("VIDEN_API_BASE"),
        supports_streaming: false,
        supports_native_tool_calling: false,
        auth_modes: &[ProviderAuthMode::Local],
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::Fallback,
        provider_id: "fallback",
        display_name: "Fallback",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: "fallback-local",
        known_models: &["fallback-local", "test-local"],
        api_key_env: None,
        api_base_env: None,
        supports_streaming: false,
        supports_native_tool_calling: false,
        auth_modes: &[ProviderAuthMode::Local],
    },
];

#[derive(Debug, Clone, Copy)]
struct BuiltinGatewayMetadata {
    provider_id: &'static str,
    display_name: &'static str,
    protocol_family: ProtocolFamily,
    default_api_base: &'static str,
    default_model: Option<&'static str>,
    known_models: &'static [&'static str],
    api_key_env: &'static str,
    api_base_env: &'static str,
    supports_streaming: bool,
    supports_native_tool_calling: bool,
    auth_modes: &'static [ProviderAuthMode],
}

const BUILTIN_GATEWAY_METADATA: &[BuiltinGatewayMetadata] = &[
    BuiltinGatewayMetadata {
        provider_id: "openrouter",
        display_name: "OpenRouter",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: OPENROUTER_API_BASE,
        default_model: None,
        known_models: &[
            "openai/gpt-5.2",
            "openai/gpt-5.2-codex",
            "anthropic/claude-sonnet-4.5",
            "deepseek/deepseek-chat",
            "qwen/qwen3-coder-plus",
        ],
        api_key_env: "OPENROUTER_API_KEY",
        api_base_env: "OPENROUTER_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "groq",
        display_name: "Groq",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: GROQ_API_BASE,
        default_model: Some("openai/gpt-oss-20b"),
        known_models: &[
            "openai/gpt-oss-120b",
            "openai/gpt-oss-20b",
            "qwen/qwen3-32b",
            "deepseek-r1-distill-llama-70b",
        ],
        api_key_env: "GROQ_API_KEY",
        api_base_env: "GROQ_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "mistral",
        display_name: "Mistral AI",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: MISTRAL_API_BASE,
        default_model: Some("mistral-medium-latest"),
        known_models: &[
            "mistral-medium-latest",
            "mistral-large-latest",
            "codestral-latest",
            "devstral-medium-latest",
            "devstral-small-latest",
        ],
        api_key_env: "MISTRAL_API_KEY",
        api_base_env: "MISTRAL_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "together",
        display_name: "Together AI",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: TOGETHER_API_BASE,
        default_model: Some("openai/gpt-oss-20b"),
        known_models: &[
            "openai/gpt-oss-120b",
            "openai/gpt-oss-20b",
            "Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8",
            "deepseek-ai/DeepSeek-R1",
        ],
        api_key_env: "TOGETHER_API_KEY",
        api_base_env: "TOGETHER_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "kimi",
        display_name: "Kimi",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: KIMI_API_BASE,
        default_model: Some("kimi-k2.5"),
        known_models: &["kimi-k2.5", "kimi-k2.6", "kimi-latest", "moonshot-v1-128k"],
        api_key_env: "MOONSHOT_API_KEY",
        api_base_env: "MOONSHOT_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "qwen",
        display_name: "Qwen",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: QWEN_API_BASE,
        default_model: Some("qwen3.6-plus"),
        known_models: &[
            "qwen3.6-plus",
            "qwen3.6-flash",
            "qwen3-coder-plus",
            "qwen3-coder-flash",
            "qwen-plus-latest",
            "qwen-flash",
            "qwq-plus",
        ],
        api_key_env: "DASHSCOPE_API_KEY",
        api_base_env: "DASHSCOPE_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "dashscope-coding-plan",
        display_name: "DashScope Coding Plan",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: DASHSCOPE_CODING_PLAN_API_BASE,
        default_model: Some("qwen3.6-plus"),
        known_models: &[
            "qwen3.6-plus",
            "qwen3.5-plus",
            "qwen3-max-2026-01-23",
            "qwen3-coder-next",
            "qwen3-coder-plus",
            "kimi-k2.5",
            "glm-5",
            "glm-4.7",
            "MiniMax-M2.5",
        ],
        api_key_env: "DASHSCOPE_CODING_PLAN_API_KEY",
        api_base_env: "DASHSCOPE_CODING_PLAN_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "dashscope-coding-plan-anthropic",
        display_name: "DashScope Coding Plan Anthropic-Compatible",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: DASHSCOPE_CODING_PLAN_ANTHROPIC_API_BASE,
        default_model: Some("qwen3.6-plus"),
        known_models: &[
            "qwen3.6-plus",
            "qwen3.5-plus",
            "qwen3-max-2026-01-23",
            "qwen3-coder-next",
            "qwen3-coder-plus",
            "kimi-k2.5",
            "glm-5",
            "glm-4.7",
            "MiniMax-M2.5",
        ],
        api_key_env: "DASHSCOPE_CODING_PLAN_API_KEY",
        api_base_env: "DASHSCOPE_CODING_PLAN_ANTHROPIC_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "dashscope-tokenplan",
        display_name: "DashScope TokenPlan",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: DASHSCOPE_TOKENPLAN_API_BASE,
        default_model: Some("qwen3.6-plus"),
        known_models: &[
            "qwen3.7-max",
            "qwen3.6-plus",
            "qwen3.6-flash",
            "qwen-image-2.0",
            "qwen-image-2.0-pro",
            "wan2.7-image",
            "wan2.7-image-pro",
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "deepseek-v3.2",
            "kimi-k2.6",
            "kimi-k2.5",
            "glm-5.1",
            "glm-5",
            "MiniMax-M2.5",
        ],
        api_key_env: "DASHSCOPE_API_KEY",
        api_base_env: "DASHSCOPE_TOKENPLAN_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "dashscope-tokenplan-anthropic",
        display_name: "DashScope TokenPlan Anthropic-Compatible",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: DASHSCOPE_TOKENPLAN_ANTHROPIC_API_BASE,
        default_model: Some("deepseek-v4-flash"),
        known_models: &[
            "qwen3.7-max",
            "qwen3.6-plus",
            "qwen3.6-flash",
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "deepseek-v3.2",
            "kimi-k2.6",
            "kimi-k2.5",
            "glm-5.1",
            "glm-5",
            "MiniMax-M2.5",
        ],
        api_key_env: "DASHSCOPE_API_KEY",
        api_base_env: "DASHSCOPE_TOKENPLAN_ANTHROPIC_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "zhipu",
        display_name: "Zhipu GLM",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: ZHIPU_API_BASE,
        default_model: Some("glm-4.6"),
        known_models: &["glm-5", "glm-4.7", "glm-4.6", "glm-4.5"],
        api_key_env: "ZHIPU_API_KEY",
        api_base_env: "ZHIPU_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
    BuiltinGatewayMetadata {
        provider_id: "volcengine",
        display_name: "Volcengine Ark",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: VOLCENGINE_API_BASE,
        default_model: Some("doubao-seed-2.0-code"),
        known_models: &[
            "doubao-seed-2.0-code",
            "doubao-seed-2.0",
            "doubao-seed-1.6",
            "deepseek-v3.2",
            "ark-code-latest",
        ],
        api_key_env: "ARK_API_KEY",
        api_base_env: "ARK_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
        auth_modes: &[ProviderAuthMode::ApiKey],
    },
];

fn builtin_provider_metadata(kind: ProviderKind) -> &'static BuiltinProviderMetadata {
    BUILTIN_PROVIDER_METADATA
        .iter()
        .find(|metadata| metadata.kind == kind)
        .expect("builtin provider metadata should exist for every ProviderKind")
}

pub(crate) fn builtin_provider_kind(provider_id: &str) -> Option<ProviderKind> {
    BUILTIN_PROVIDER_METADATA
        .iter()
        .find(|metadata| metadata.provider_id == provider_id)
        .map(|metadata| metadata.kind)
}

pub(crate) fn builtin_default_model(kind: ProviderKind) -> &'static str {
    builtin_provider_metadata(kind).default_model
}

pub(crate) fn is_builtin_default_model(model: &str) -> bool {
    BUILTIN_PROVIDER_METADATA
        .iter()
        .any(|metadata| metadata.default_model == model)
}

pub(crate) fn builtin_default_api_base(kind: ProviderKind) -> Option<&'static str> {
    builtin_provider_metadata(kind).default_api_base
}

fn builtin_provider_compatibility(kind: ProviderKind) -> ProviderCompatibility {
    match kind {
        ProviderKind::DeepSeek => ProviderCompatibility {
            supports_tool_choice: false,
            requires_reasoning_content_for_tool_calls: true,
            requires_non_null_tool_call_content: true,
            reasoning_effort_high: Some("high".to_string()),
            reasoning_effort_max: Some("max".to_string()),
        },
        ProviderKind::DeepSeekAnthropic => ProviderCompatibility {
            reasoning_effort_high: Some("high".to_string()),
            reasoning_effort_max: Some("max".to_string()),
            ..ProviderCompatibility::default()
        },
        _ => ProviderCompatibility::default(),
    }
}

pub(crate) fn builtin_provider_descriptors() -> Vec<ProviderDescriptor> {
    let mut descriptors = BUILTIN_PROVIDER_METADATA
        .iter()
        .map(|metadata| ProviderDescriptor {
            provider_id: metadata.provider_id.to_string(),
            display_name: metadata.display_name.to_string(),
            version: "builtin".to_string(),
            protocol_family: metadata.protocol_family,
            default_api_base: metadata.default_api_base.map(ToString::to_string),
            default_model: Some(metadata.default_model.to_string()),
            known_models: metadata
                .known_models
                .iter()
                .map(ToString::to_string)
                .collect(),
            env_mappings: ProviderEnvMappings {
                api_key_env: metadata.api_key_env.map(ToString::to_string),
                api_base_env: metadata.api_base_env.map(ToString::to_string),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: metadata.supports_streaming,
                supports_native_tool_calling: metadata.supports_native_tool_calling,
            },
            compatibility: builtin_provider_compatibility(metadata.kind),
            auth_modes: metadata.auth_modes.to_vec(),
            config_schema_version: 1,
        })
        .collect::<Vec<_>>();
    descriptors.extend(BUILTIN_GATEWAY_METADATA.iter().map(|metadata| {
        ProviderDescriptor {
            provider_id: metadata.provider_id.to_string(),
            display_name: metadata.display_name.to_string(),
            version: "builtin".to_string(),
            protocol_family: metadata.protocol_family,
            default_api_base: Some(metadata.default_api_base.to_string()),
            default_model: metadata.default_model.map(ToString::to_string),
            known_models: metadata
                .known_models
                .iter()
                .map(ToString::to_string)
                .collect(),
            env_mappings: ProviderEnvMappings {
                api_key_env: Some(metadata.api_key_env.to_string()),
                api_base_env: Some(metadata.api_base_env.to_string()),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: metadata.supports_streaming,
                supports_native_tool_calling: metadata.supports_native_tool_calling,
            },
            compatibility: ProviderCompatibility::default(),
            auth_modes: metadata.auth_modes.to_vec(),
            config_schema_version: 1,
        }
    }));
    descriptors
}

pub(crate) fn builtin_provider_id(kind: ProviderKind) -> &'static str {
    builtin_provider_metadata(kind).provider_id
}
