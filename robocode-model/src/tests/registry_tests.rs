use super::*;

#[test]
fn registry_lists_builtin_provider_ids() {
    let registry = ProviderRegistry::with_builtins();
    let ids = registry.provider_ids();
    assert!(ids.contains(&"anthropic".to_string()));
    assert!(ids.contains(&"deepseek".to_string()));
    assert!(ids.contains(&"openrouter".to_string()));
    assert!(ids.contains(&"groq".to_string()));
    assert!(ids.contains(&"mistral".to_string()));
    assert!(ids.contains(&"together".to_string()));
    assert!(ids.contains(&"kimi".to_string()));
    assert!(ids.contains(&"qwen".to_string()));
    assert!(ids.contains(&"zhipu".to_string()));
    assert!(ids.contains(&"volcengine".to_string()));
    assert!(ids.contains(&"openai".to_string()));
    assert!(ids.contains(&"fallback".to_string()));
}

#[test]
fn provider_kind_parse_roundtrips_builtin_provider_ids() {
    for provider_id in [
        "anthropic",
        "deepseek",
        "deepseek-anthropic",
        "openai",
        "openai-compatible",
        "ollama",
        "fallback",
    ] {
        let kind = ProviderKind::parse(&provider_id)
            .expect("every builtin provider id should parse through shared metadata");
        assert_eq!(builtin_provider_id(kind), provider_id.to_string());
    }
}

#[test]
fn supported_provider_strings_match_builtin_registry_ids() {
    let registry = ProviderRegistry::with_builtins();
    let mut ids = registry.provider_ids();
    let mut supported = list_supported_provider_strings();
    ids.sort();
    supported.sort();
    assert_eq!(supported, ids);
}

#[test]
fn builtin_openai_compatible_gateway_descriptors_expose_capability_matrix() {
    let registry = ProviderRegistry::with_builtins();

    let openrouter = registry.descriptor("openrouter").unwrap();
    assert_eq!(openrouter.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(
        openrouter.default_api_base.as_deref(),
        Some("https://openrouter.ai/api/v1")
    );
    assert_eq!(
        openrouter.env_mappings.api_key_env.as_deref(),
        Some("OPENROUTER_API_KEY")
    );
    assert!(openrouter.capabilities.supports_streaming);
    assert!(openrouter.capabilities.supports_native_tool_calling);

    let volcengine = registry.descriptor("volcengine").unwrap();
    assert_eq!(volcengine.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(
        volcengine.default_api_base.as_deref(),
        Some("https://ark.cn-beijing.volces.com/api/v3")
    );
    assert_eq!(
        volcengine.env_mappings.api_key_env.as_deref(),
        Some("ARK_API_KEY")
    );
    assert_eq!(volcengine.default_model, None);
}

#[test]
fn descriptor_keeps_provider_identity_separate_from_protocol_family() {
    let descriptor = ProviderDescriptor {
        provider_id: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://api.deepseek.com".to_string()),
        default_model: Some("deepseek-v4-flash".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}

#[test]
fn provider_descriptor_validation_rejects_invalid_plugin_identity() {
    let descriptor = ProviderDescriptor {
        provider_id: "../bad".to_string(),
        display_name: "Bad Plugin".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("bad-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("provider_id"), "{err}");
}

#[test]
fn provider_descriptor_validation_rejects_unsupported_schema_version() {
    let descriptor = ProviderDescriptor {
        provider_id: "future-provider".to_string(),
        display_name: "Future Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("future-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 99,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("config_schema_version"), "{err}");
}

#[test]
fn provider_descriptor_validation_rejects_empty_default_model() {
    let descriptor = ProviderDescriptor {
        provider_id: "empty-model-provider".to_string(),
        display_name: "Empty Model Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("   ".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("default_model"), "{err}");
}

#[test]
fn provider_descriptor_validation_rejects_invalid_api_base() {
    let descriptor = ProviderDescriptor {
        provider_id: "invalid-base-provider".to_string(),
        display_name: "Invalid Base Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("ftp://example.com".to_string()),
        default_model: Some("model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("default_api_base"), "{err}");
}

#[test]
fn provider_descriptor_validation_rejects_invalid_env_mapping() {
    let descriptor = ProviderDescriptor {
        provider_id: "invalid-env-provider".to_string(),
        display_name: "Invalid Env Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: Some("1BAD_ENV".to_string()),
            api_base_env: None,
        },
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = descriptor::validate_provider_descriptor(&descriptor).unwrap_err();
    assert!(err.contains("api_key_env"), "{err}");
}

#[test]
fn provider_descriptor_validation_accepts_builtin_deepseek_shape() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek").unwrap();
    descriptor::validate_provider_descriptor(descriptor).unwrap();
}

#[test]
fn registry_rejects_plugin_descriptor_that_conflicts_with_builtin_provider_id() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "openai".to_string(),
        display_name: "Conflicting OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://example.com".to_string()),
        default_model: Some("conflict-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap_err();
    assert!(err.contains("conflicts"), "{err}");
}

#[test]
fn registry_rejects_duplicate_plugin_provider_ids() {
    let first = ProviderDescriptor {
        provider_id: "duplicate-provider".to_string(),
        display_name: "Duplicate Provider A".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://a.example.com".to_string()),
        default_model: Some("model-a".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };
    let second = ProviderDescriptor {
        provider_id: "duplicate-provider".to_string(),
        display_name: "Duplicate Provider B".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://b.example.com".to_string()),
        default_model: Some("model-b".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    let err = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![first, second],
    )
    .unwrap_err();
    assert!(err.contains("duplicate-provider"), "{err}");
    assert!(err.contains("conflicts"), "{err}");
}

#[test]
fn registry_accepts_valid_non_builtin_plugin_descriptor() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "custom-openai".to_string(),
        display_name: "Custom OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://models.example.com".to_string()),
        default_model: Some("custom-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: Some("CUSTOM_OPENAI_API_KEY".to_string()),
            api_base_env: Some("CUSTOM_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        config_schema_version: 1,
    };

    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    assert!(registry.descriptor("custom-openai").is_some());
    assert!(
        registry
            .provider_ids()
            .contains(&"custom-openai".to_string())
    );
}

#[test]
fn builtin_openai_descriptor_matches_runtime_api_base_behavior() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry
        .descriptors()
        .iter()
        .find(|descriptor| descriptor.provider_id == "openai")
        .expect("openai descriptor should exist");

    assert_eq!(
        descriptor.default_api_base.as_deref(),
        builtin_default_api_base(ProviderKind::OpenAi)
    );
    assert_eq!(
        descriptor.env_mappings.api_base_env.as_deref(),
        Some("ROBOCODE_API_BASE")
    );
}

#[test]
fn registry_exposes_deepseek_as_independent_provider_id() {
    let registry = ProviderRegistry::with_builtins();
    assert!(registry.provider_ids().contains(&"deepseek".to_string()));
}

#[test]
fn deepseek_provider_uses_openai_protocol_family() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek").unwrap();
    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}

#[test]
fn deepseek_anthropic_provider_uses_official_anthropic_endpoint() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek-anthropic").unwrap();
    assert_eq!(descriptor.provider_id, "deepseek-anthropic");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::Anthropic);
    assert_eq!(
        descriptor.default_api_base.as_deref(),
        Some("https://api.deepseek.com/anthropic")
    );
    assert_eq!(
        descriptor.default_model.as_deref(),
        Some("deepseek-v4-flash")
    );
}
