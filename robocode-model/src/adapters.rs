use crate::config::ProviderKind;
use crate::descriptor::{
    ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings,
};
use crate::http::{
    ANTHROPIC_API_BASE, DEEPSEEK_ANTHROPIC_API_BASE, DEEPSEEK_API_BASE, GROQ_API_BASE,
    KIMI_API_BASE, MISTRAL_API_BASE, OLLAMA_API_BASE, OPENAI_API_BASE, OPENROUTER_API_BASE,
    QWEN_API_BASE, TOGETHER_API_BASE, VOLCENGINE_API_BASE, ZHIPU_API_BASE,
};

#[derive(Debug, Clone, Copy)]
struct BuiltinProviderMetadata {
    kind: ProviderKind,
    provider_id: &'static str,
    display_name: &'static str,
    protocol_family: ProtocolFamily,
    default_api_base: Option<&'static str>,
    default_model: &'static str,
    api_key_env: Option<&'static str>,
    api_base_env: Option<&'static str>,
    supports_streaming: bool,
    supports_native_tool_calling: bool,
}

const BUILTIN_PROVIDER_METADATA: &[BuiltinProviderMetadata] = &[
    BuiltinProviderMetadata {
        kind: ProviderKind::Anthropic,
        provider_id: "anthropic",
        display_name: "Anthropic",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some(ANTHROPIC_API_BASE),
        default_model: "claude-sonnet-4-6",
        api_key_env: Some("ANTHROPIC_API_KEY"),
        api_base_env: Some("ROBOCODE_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::OpenAi,
        provider_id: "openai",
        display_name: "OpenAI",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OPENAI_API_BASE),
        default_model: "gpt-5.2",
        api_key_env: Some("OPENAI_API_KEY"),
        api_base_env: Some("ROBOCODE_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::OpenAiCompatible,
        provider_id: "openai-compatible",
        display_name: "OpenAI-Compatible",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OPENAI_API_BASE),
        default_model: "gpt-4o-mini",
        api_key_env: Some("OPENAI_API_KEY"),
        api_base_env: Some("ROBOCODE_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::DeepSeek,
        provider_id: "deepseek",
        display_name: "DeepSeek",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(DEEPSEEK_API_BASE),
        default_model: "deepseek-v4-flash",
        api_key_env: Some("DEEPSEEK_API_KEY"),
        api_base_env: Some("DEEPSEEK_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::DeepSeekAnthropic,
        provider_id: "deepseek-anthropic",
        display_name: "DeepSeek Anthropic-Compatible",
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some(DEEPSEEK_ANTHROPIC_API_BASE),
        default_model: "deepseek-v4-flash",
        api_key_env: Some("DEEPSEEK_API_KEY"),
        api_base_env: Some("DEEPSEEK_API_BASE"),
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::Ollama,
        provider_id: "ollama",
        display_name: "Ollama",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some(OLLAMA_API_BASE),
        default_model: "llama3.1",
        api_key_env: None,
        api_base_env: Some("ROBOCODE_API_BASE"),
        supports_streaming: false,
        supports_native_tool_calling: false,
    },
    BuiltinProviderMetadata {
        kind: ProviderKind::Fallback,
        provider_id: "fallback",
        display_name: "Fallback",
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: "fallback-local",
        api_key_env: None,
        api_base_env: None,
        supports_streaming: false,
        supports_native_tool_calling: false,
    },
];

#[derive(Debug, Clone, Copy)]
struct BuiltinGatewayMetadata {
    provider_id: &'static str,
    display_name: &'static str,
    default_api_base: &'static str,
    default_model: Option<&'static str>,
    api_key_env: &'static str,
    api_base_env: &'static str,
    supports_streaming: bool,
    supports_native_tool_calling: bool,
}

const BUILTIN_GATEWAY_METADATA: &[BuiltinGatewayMetadata] = &[
    BuiltinGatewayMetadata {
        provider_id: "openrouter",
        display_name: "OpenRouter",
        default_api_base: OPENROUTER_API_BASE,
        default_model: None,
        api_key_env: "OPENROUTER_API_KEY",
        api_base_env: "OPENROUTER_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "groq",
        display_name: "Groq",
        default_api_base: GROQ_API_BASE,
        default_model: Some("openai/gpt-oss-20b"),
        api_key_env: "GROQ_API_KEY",
        api_base_env: "GROQ_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "mistral",
        display_name: "Mistral AI",
        default_api_base: MISTRAL_API_BASE,
        default_model: Some("mistral-medium-latest"),
        api_key_env: "MISTRAL_API_KEY",
        api_base_env: "MISTRAL_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "together",
        display_name: "Together AI",
        default_api_base: TOGETHER_API_BASE,
        default_model: Some("openai/gpt-oss-20b"),
        api_key_env: "TOGETHER_API_KEY",
        api_base_env: "TOGETHER_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "kimi",
        display_name: "Kimi",
        default_api_base: KIMI_API_BASE,
        default_model: None,
        api_key_env: "MOONSHOT_API_KEY",
        api_base_env: "MOONSHOT_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "qwen",
        display_name: "Qwen",
        default_api_base: QWEN_API_BASE,
        default_model: Some("qwen-plus"),
        api_key_env: "DASHSCOPE_API_KEY",
        api_base_env: "DASHSCOPE_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "zhipu",
        display_name: "Zhipu GLM",
        default_api_base: ZHIPU_API_BASE,
        default_model: Some("glm-4.6"),
        api_key_env: "ZHIPU_API_KEY",
        api_base_env: "ZHIPU_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    BuiltinGatewayMetadata {
        provider_id: "volcengine",
        display_name: "Volcengine Ark",
        default_api_base: VOLCENGINE_API_BASE,
        default_model: None,
        api_key_env: "ARK_API_KEY",
        api_base_env: "ARK_API_BASE",
        supports_streaming: true,
        supports_native_tool_calling: true,
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
            env_mappings: ProviderEnvMappings {
                api_key_env: metadata.api_key_env.map(ToString::to_string),
                api_base_env: metadata.api_base_env.map(ToString::to_string),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: metadata.supports_streaming,
                supports_native_tool_calling: metadata.supports_native_tool_calling,
            },
            config_schema_version: 1,
        })
        .collect::<Vec<_>>();
    descriptors.extend(
        BUILTIN_GATEWAY_METADATA
            .iter()
            .map(|metadata| ProviderDescriptor {
                provider_id: metadata.provider_id.to_string(),
                display_name: metadata.display_name.to_string(),
                version: "builtin".to_string(),
                protocol_family: ProtocolFamily::OpenAi,
                default_api_base: Some(metadata.default_api_base.to_string()),
                default_model: metadata.default_model.map(ToString::to_string),
                env_mappings: ProviderEnvMappings {
                    api_key_env: Some(metadata.api_key_env.to_string()),
                    api_base_env: Some(metadata.api_base_env.to_string()),
                },
                capabilities: ProviderCapabilities {
                    supports_streaming: metadata.supports_streaming,
                    supports_native_tool_calling: metadata.supports_native_tool_calling,
                },
                config_schema_version: 1,
            }),
    );
    descriptors
}

pub(crate) fn builtin_provider_id(kind: ProviderKind) -> &'static str {
    builtin_provider_metadata(kind).provider_id
}
