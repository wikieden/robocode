use std::sync::LazyLock;

use crate::config::ProviderKind;
use crate::descriptor::{
    ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings,
};
use crate::http::{ANTHROPIC_API_BASE, OLLAMA_API_BASE, OPENAI_API_BASE};

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

static BUILTIN_PROVIDER_IDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    BUILTIN_PROVIDER_METADATA
        .iter()
        .map(|metadata| metadata.provider_id)
        .collect()
});

fn builtin_provider_metadata(kind: ProviderKind) -> &'static BuiltinProviderMetadata {
    BUILTIN_PROVIDER_METADATA
        .iter()
        .find(|metadata| metadata.kind == kind)
        .expect("builtin provider metadata should exist for every ProviderKind")
}

pub(crate) fn builtin_provider_ids() -> &'static [&'static str] {
    BUILTIN_PROVIDER_IDS.as_slice()
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
    BUILTIN_PROVIDER_METADATA
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
        .collect()
}

pub(crate) fn builtin_provider_id(kind: ProviderKind) -> &'static str {
    builtin_provider_metadata(kind).provider_id
}
