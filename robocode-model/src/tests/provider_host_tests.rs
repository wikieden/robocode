use super::*;

#[test]
fn provider_host_can_refresh_registry_without_replacing_existing_provider_instance() {
    let mut host = ProviderHost::with_builtins();
    let before_registry = host.registry();
    let mut provider = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    host.refresh().unwrap();
    let after_registry = host.registry();

    let mut before_ids = before_registry.provider_ids();
    let mut after_ids = after_registry.provider_ids();
    before_ids.sort();
    after_ids.sort();

    assert!(!std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert_eq!(after_ids, before_ids);
    assert!(
        after_registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.provider_id == "openai-compatible")
    );
    assert_eq!(provider.provider_name(), "openai-compatible");
    provider.set_model("deepseek-v4-pro".to_string());
    assert_eq!(provider.model(), "deepseek-v4-pro");
}

#[test]
fn provider_host_creates_independent_provider_instances_per_engine() {
    let host = ProviderHost::with_builtins();
    let mut first = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let second = host
        .create(
            ProviderConfig::from_settings(
                "openai-compatible",
                Some("deepseek-chat"),
                None,
                None,
                90,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    first.set_model("deepseek-v4-pro".to_string());

    assert_eq!(first.provider_name(), "openai-compatible");
    assert_eq!(first.model(), "deepseek-v4-pro");
    assert_eq!(second.provider_name(), "openai-compatible");
    assert_eq!(second.model(), "deepseek-chat");
}

#[test]
fn provider_host_creates_dynamic_openai_provider_from_registry_descriptor() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "custom-openai".to_string(),
        display_name: "Custom OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://models.example.com".to_string()),
        default_model: Some("custom-model".to_string()),
        env_mappings: ProviderEnvMappings::default(),
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
    let host = ProviderHost::with_registry(registry);

    let provider = host
        .create_registered("custom-openai", None, None, None, 90, 1)
        .unwrap();

    assert_eq!(provider.provider_name(), "custom-openai");
    assert_eq!(provider.model(), "custom-model");
}

#[test]
fn provider_host_keeps_dynamic_provider_instances_independent() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "team-provider".to_string(),
        display_name: "Team Provider".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::Anthropic,
        default_api_base: Some("https://team.example.com".to_string()),
        default_model: Some("team-default".to_string()),
        env_mappings: ProviderEnvMappings::default(),
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
    let host = ProviderHost::with_registry(registry);
    let mut first = host
        .create_registered("team-provider", Some("agent-a-model"), None, None, 90, 1)
        .unwrap();
    let second = host
        .create_registered("team-provider", Some("agent-b-model"), None, None, 90, 1)
        .unwrap();

    first.set_model("agent-a-updated".to_string());

    assert_eq!(first.provider_name(), "team-provider");
    assert_eq!(first.model(), "agent-a-updated");
    assert_eq!(second.provider_name(), "team-provider");
    assert_eq!(second.model(), "agent-b-model");
}
