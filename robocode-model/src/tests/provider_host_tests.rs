use super::provider_plugin_fixtures::{
    compile_invalid_provider_plugin, compile_provider_plugin, compile_raw_provider_plugin,
    compile_runtime_provider_plugin, temp_dir,
};
use super::*;
use crate::plugin::{ProviderPluginErrorKind, dynamic_library_suffixes};

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
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
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
fn provider_host_uses_dynamic_provider_api_base_env_mapping() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "env-openai".to_string(),
        display_name: "Env OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: Some("env-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: None,
            api_base_env: Some("ROBOCODE_TEST_ENV_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);

    unsafe {
        std::env::set_var(
            "ROBOCODE_TEST_ENV_OPENAI_API_BASE",
            "https://env.example.com",
        );
    }
    let provider = host.create_registered("env-openai", None, None, None, 90, 1);
    unsafe {
        std::env::remove_var("ROBOCODE_TEST_ENV_OPENAI_API_BASE");
    }
    let provider = provider.unwrap();

    assert_eq!(provider.provider_name(), "env-openai");
    assert_eq!(provider.model(), "env-model");
}

#[test]
fn provider_host_treats_blank_dynamic_provider_api_key_env_as_missing() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "blank-key-openai".to_string(),
        display_name: "Blank Key OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("http://127.0.0.1:9".to_string()),
        default_model: Some("blank-key-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: Some("ROBOCODE_TEST_BLANK_OPENAI_API_KEY".to_string()),
            api_base_env: None,
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);

    unsafe {
        std::env::set_var("ROBOCODE_TEST_BLANK_OPENAI_API_KEY", "   ");
    }
    let mut provider = host
        .create_registered("blank-key-openai", None, None, None, 1, 0)
        .unwrap();
    let events = provider.next_events(&ModelRequest {
        session_id: "session_blank_key".to_string(),
        model: "blank-key-model".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        tools: Vec::new(),
        permission_mode: PermissionMode::Default,
    });
    unsafe {
        std::env::remove_var("ROBOCODE_TEST_BLANK_OPENAI_API_KEY");
    }
    let events = events.unwrap();

    assert!(matches!(
        &events[0],
        ModelEvent::AssistantText { content }
            if content.contains("blank-key-openai provider is running in local fallback mode")
    ));
}

#[test]
#[ignore = "requires DEEPSEEK_API_KEY and live network access"]
fn deepseek_v4_accepts_replayed_tool_call_reasoning_content() {
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY is required for this ignored smoke test");
    let model =
        std::env::var("ROBOCODE_LIVE_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
    let mut provider = create_provider(ProviderConfig {
        kind: ProviderKind::DeepSeek,
        model: model.clone(),
        api_base: None,
        api_key: Some(api_key),
        request_timeout_secs: 90,
        max_retries: 1,
    });

    let events = provider
        .next_events(&ModelRequest {
            session_id: "session_live_deepseek_tool_replay".to_string(),
            model,
            messages: vec![
                Message::new(Role::User, "Create hello_world.py with the write_file tool."),
                Message {
                    id: "msg_live_tool_call".to_string(),
                    role: Role::Assistant,
                    content: format!(
                        "path=hello_world.py\tcontent=print('Hello, world!')\t{PROVIDER_REASONING_CONTENT_KEY}=Need to create the requested file."
                    ),
                    timestamp: 1,
                    tool_name: Some("write_file".to_string()),
                    tool_call_id: Some("call_live_123".to_string()),
                },
                Message {
                    id: "msg_live_tool_result".to_string(),
                    role: Role::Tool,
                    content: "Wrote hello_world.py".to_string(),
                    timestamp: 2,
                    tool_name: Some("write_file".to_string()),
                    tool_call_id: Some("call_live_123".to_string()),
                },
                Message::new(Role::User, "Reply with exactly DONE."),
            ],
            tools: vec![ToolSpec {
                name: "write_file".to_string(),
                description: "Write a file".to_string(),
                is_mutating: true,
                input_schema_hint: "path=file content=text".to_string(),
            }],
            permission_mode: PermissionMode::Default,
        })
        .expect("DeepSeek V4 Flash should accept replayed tool-call history");

    assert!(
        events
            .iter()
            .any(|event| matches!(event, ModelEvent::AssistantText { content } if content.contains("DONE"))),
        "expected DONE response, got {events:?}"
    );
}

#[test]
fn provider_host_prefers_explicit_api_base_over_dynamic_provider_env_mapping() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "explicit-openai".to_string(),
        display_name: "Explicit OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: Some("explicit-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: None,
            api_base_env: Some("ROBOCODE_TEST_EXPLICIT_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);

    unsafe {
        std::env::set_var("ROBOCODE_TEST_EXPLICIT_OPENAI_API_BASE", "not-a-valid-url");
    }
    let provider = host.create_registered(
        "explicit-openai",
        None,
        Some("https://explicit.example.com"),
        None,
        90,
        1,
    );
    unsafe {
        std::env::remove_var("ROBOCODE_TEST_EXPLICIT_OPENAI_API_BASE");
    }
    let provider = provider.unwrap();

    assert_eq!(provider.provider_name(), "explicit-openai");
    assert_eq!(provider.model(), "explicit-model");
}

#[test]
fn provider_host_rejects_invalid_dynamic_provider_api_base_env_mapping() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "invalid-env-openai".to_string(),
        display_name: "Invalid Env OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: Some("invalid-env-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: None,
            api_base_env: Some("ROBOCODE_TEST_INVALID_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);
    unsafe {
        std::env::set_var("ROBOCODE_TEST_INVALID_OPENAI_API_BASE", "not-a-valid-url");
    }
    let error = host
        .create_registered("invalid-env-openai", None, None, None, 90, 1)
        .err()
        .unwrap()
        .to_string();
    unsafe {
        std::env::remove_var("ROBOCODE_TEST_INVALID_OPENAI_API_BASE");
    }

    assert!(error.contains("ROBOCODE_TEST_INVALID_OPENAI_API_BASE"));
    assert!(error.contains("must start with http:// or https://"));
}

#[test]
fn provider_host_reports_missing_dynamic_provider_api_base_after_env_lookup() {
    let plugin_descriptor = ProviderDescriptor {
        provider_id: "missing-base-openai".to_string(),
        display_name: "Missing Base OpenAI".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: None,
        default_model: Some("missing-base-model".to_string()),
        env_mappings: ProviderEnvMappings {
            api_key_env: None,
            api_base_env: Some("ROBOCODE_TEST_MISSING_OPENAI_API_BASE".to_string()),
        },
        capabilities: ProviderCapabilities {
            supports_streaming: true,
            supports_native_tool_calling: true,
        },
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
        config_schema_version: 1,
    };
    let registry = ProviderRegistry::from_descriptor_sources(
        adapters::builtin_provider_descriptors(),
        vec![plugin_descriptor],
    )
    .unwrap();
    let host = ProviderHost::with_registry(registry);
    unsafe {
        std::env::remove_var("ROBOCODE_TEST_MISSING_OPENAI_API_BASE");
    }

    let error = host
        .create_registered("missing-base-openai", None, None, None, 90, 1)
        .err()
        .unwrap()
        .to_string();

    assert!(error.contains("does not define a default API base"));
    assert!(error.contains("pass an API base explicitly"));
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
        compatibility: ProviderCompatibility::default(),
        auth_modes: Vec::new(),
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

#[test]
fn provider_host_refresh_loads_new_dynamic_provider_from_plugin_dir() {
    let plugin_dir = temp_dir("runtime_refresh");
    let mut host = ProviderHost::load_from_dirs(vec![plugin_dir.clone()]).unwrap();
    let before_registry = host.registry();
    let mut existing = host
        .create_registered("openai-compatible", Some("agent-a"), None, None, 90, 1)
        .unwrap();

    assert!(before_registry.descriptor("runtime-provider").is_none());

    let plugin_path = compile_runtime_provider_plugin(&plugin_dir);
    host.refresh_from_dirs(vec![plugin_dir]).unwrap();
    let after_registry = host.registry();

    assert!(plugin_path.exists());
    assert!(!std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert!(after_registry.descriptor("runtime-provider").is_some());

    let loaded = host
        .create_registered("runtime-provider", None, None, None, 90, 1)
        .unwrap();
    existing.set_model("agent-a-updated".to_string());

    assert_eq!(existing.provider_name(), "openai-compatible");
    assert_eq!(existing.model(), "agent-a-updated");
    assert_eq!(loaded.provider_name(), "runtime-provider");
    assert_eq!(loaded.model(), "runtime-default");
}

#[test]
fn provider_host_refresh_from_dirs_is_instance_scoped_for_builtin_hosts() {
    let plugin_dir = temp_dir("builtin_host_instance_scoped_refresh");
    compile_runtime_provider_plugin(&plugin_dir);
    let mut first_host = ProviderHost::with_builtins();
    let second_host = ProviderHost::with_builtins();
    let second_before_registry = second_host.registry();

    first_host.refresh_from_dirs(vec![plugin_dir]).unwrap();

    let first_after_registry = first_host.registry();
    let second_after_registry = second_host.registry();
    assert!(
        first_after_registry
            .descriptor("runtime-provider")
            .is_some()
    );
    assert!(
        second_after_registry
            .descriptor("runtime-provider")
            .is_none()
    );
    assert!(std::sync::Arc::ptr_eq(
        &second_before_registry,
        &second_after_registry
    ));
}

#[test]
fn provider_host_loads_multiple_dynamic_providers_from_plugin_dirs() {
    let first_dir = temp_dir("multi_provider_first");
    let second_dir = temp_dir("multi_provider_second");
    compile_provider_plugin(
        &first_dir,
        "alpha_provider_plugin",
        "alpha-provider",
        "Alpha Provider",
        "https://alpha.example.com",
        "alpha-default",
    );
    compile_provider_plugin(
        &second_dir,
        "beta_provider_plugin",
        "beta-provider",
        "Beta Provider",
        "https://beta.example.com",
        "beta-default",
    );

    let host = ProviderHost::load_from_dirs_diagnostic(vec![second_dir, first_dir]).unwrap();
    let registry = host.registry();

    assert!(registry.descriptor("alpha-provider").is_some());
    assert!(registry.descriptor("beta-provider").is_some());
    let alpha = host
        .create_registered("alpha-provider", None, None, None, 90, 1)
        .unwrap();
    let beta = host
        .create_registered("beta-provider", None, None, None, 90, 1)
        .unwrap();
    assert_eq!(alpha.provider_name(), "alpha-provider");
    assert_eq!(alpha.model(), "alpha-default");
    assert_eq!(beta.provider_name(), "beta-provider");
    assert_eq!(beta.model(), "beta-default");
}

#[test]
fn provider_host_diagnostic_load_rejects_dynamic_plugin_conflicting_with_builtin() {
    let plugin_dir = temp_dir("builtin_conflict");
    compile_provider_plugin(
        &plugin_dir,
        "openai_conflict_provider_plugin",
        "openai",
        "Conflicting OpenAI",
        "https://conflict.example.com",
        "conflict-default",
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::Registry);
    assert!(err.message.contains("openai"), "{err}");
    assert!(err.message.contains("conflicts"), "{err}");
}

#[test]
fn provider_host_diagnostic_load_rejects_duplicate_dynamic_provider_ids() {
    let plugin_dir = temp_dir("duplicate_dynamic_provider");
    compile_provider_plugin(
        &plugin_dir,
        "duplicate_provider_a_plugin",
        "duplicate-dynamic-provider",
        "Duplicate Dynamic Provider A",
        "https://duplicate-a.example.com",
        "duplicate-a-default",
    );
    compile_provider_plugin(
        &plugin_dir,
        "duplicate_provider_b_plugin",
        "duplicate-dynamic-provider",
        "Duplicate Dynamic Provider B",
        "https://duplicate-b.example.com",
        "duplicate-b-default",
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::Registry);
    assert!(err.message.contains("duplicate-dynamic-provider"), "{err}");
    assert!(err.message.contains("conflicts"), "{err}");
}

#[test]
fn provider_host_refresh_dynamic_conflict_keeps_previous_registry_active() {
    let good_dir = temp_dir("refresh_conflict_good");
    compile_runtime_provider_plugin(&good_dir);
    let mut host = ProviderHost::load_from_dirs_diagnostic(vec![good_dir]).unwrap();
    let before_registry = host.registry();
    let mut existing = host
        .create_registered(
            "runtime-provider",
            Some("before-refresh"),
            None,
            None,
            90,
            1,
        )
        .unwrap();
    let conflict_dir = temp_dir("refresh_conflict_bad");
    compile_provider_plugin(
        &conflict_dir,
        "openai_refresh_conflict_provider_plugin",
        "openai",
        "Refresh Conflict OpenAI",
        "https://refresh-conflict.example.com",
        "refresh-conflict-default",
    );

    let err = host
        .refresh_from_dirs_diagnostic(vec![conflict_dir])
        .unwrap_err();
    let after_registry = host.registry();

    assert_eq!(err.kind, ProviderPluginErrorKind::Registry);
    assert!(err.message.contains("openai"), "{err}");
    assert!(std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert!(after_registry.descriptor("runtime-provider").is_some());
    assert!(after_registry.descriptor("openai").is_some());
    existing.set_model("still-bound".to_string());
    assert_eq!(existing.provider_name(), "runtime-provider");
    assert_eq!(existing.model(), "still-bound");
}

#[test]
fn provider_host_refresh_failure_keeps_previous_registry_active() {
    let good_dir = temp_dir("refresh_good");
    compile_runtime_provider_plugin(&good_dir);
    let mut host = ProviderHost::load_from_dirs(vec![good_dir]).unwrap();
    let before_registry = host.registry();
    let invalid_dir = temp_dir("refresh_invalid");
    compile_invalid_provider_plugin(&invalid_dir);

    let err = host.refresh_from_dirs(vec![invalid_dir]).unwrap_err();
    let after_registry = host.registry();

    assert!(err.contains("Invalid provider descriptor"), "{err}");
    assert!(std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert!(after_registry.descriptor("runtime-provider").is_some());
    assert!(after_registry.descriptor("invalid-provider").is_none());
}

#[test]
fn provider_host_diagnostic_refresh_failure_keeps_previous_registry_active() {
    let good_dir = temp_dir("diagnostic_refresh_good");
    compile_runtime_provider_plugin(&good_dir);
    let mut host = ProviderHost::load_from_dirs_diagnostic(vec![good_dir]).unwrap();
    let before_registry = host.registry();
    let invalid_dir = temp_dir("diagnostic_refresh_invalid");
    let invalid_plugin_path = compile_invalid_provider_plugin(&invalid_dir);

    let err = host
        .refresh_from_dirs_diagnostic(vec![invalid_dir])
        .unwrap_err();
    let after_registry = host.registry();

    assert_eq!(err.kind, ProviderPluginErrorKind::InvalidDescriptor);
    assert_eq!(err.path, invalid_plugin_path);
    assert!(std::sync::Arc::ptr_eq(&before_registry, &after_registry));
    assert!(after_registry.descriptor("runtime-provider").is_some());
    assert!(after_registry.descriptor("invalid-provider").is_none());
}

#[test]
fn provider_host_diagnostic_load_exposes_plugin_error_kind_and_path() {
    let plugin_dir = temp_dir("diagnostic_load_invalid");
    let plugin_path = plugin_dir.join(format!("broken.{}", dynamic_library_suffixes()[0]));
    std::fs::write(&plugin_path, b"not a real library").unwrap();

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::LoadLibrary);
    assert_eq!(err.path, plugin_path);
}

#[test]
fn provider_host_diagnostic_load_reports_missing_descriptor_symbol() {
    let plugin_dir = temp_dir("diagnostic_missing_symbol");
    let plugin_path = compile_raw_provider_plugin(
        &plugin_dir,
        "missing_symbol_provider_plugin",
        r#"
#[no_mangle]
pub extern "C" fn unrelated_symbol() -> usize {
    1
}
"#,
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::MissingDescriptorSymbol);
    assert_eq!(err.path, plugin_path);
}

#[test]
fn provider_host_diagnostic_load_reports_null_descriptor() {
    let plugin_dir = temp_dir("diagnostic_null_descriptor");
    let plugin_path = compile_raw_provider_plugin(
        &plugin_dir,
        "null_descriptor_provider_plugin",
        r#"
use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    std::ptr::null()
}
"#,
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::NullDescriptor);
    assert_eq!(err.path, plugin_path);
}

#[test]
fn provider_host_diagnostic_load_reports_non_utf8_descriptor() {
    let plugin_dir = temp_dir("diagnostic_non_utf8_descriptor");
    let plugin_path = compile_raw_provider_plugin(
        &plugin_dir,
        "non_utf8_descriptor_provider_plugin",
        r#"
use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    static BYTES: [u8; 2] = [0xff, 0x00];
    BYTES.as_ptr().cast::<c_char>()
}
"#,
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::NonUtf8Descriptor);
    assert_eq!(err.path, plugin_path);
}

#[test]
fn provider_host_diagnostic_load_reports_decode_descriptor() {
    let plugin_dir = temp_dir("diagnostic_decode_descriptor");
    let plugin_path = compile_raw_provider_plugin(
        &plugin_dir,
        "decode_descriptor_provider_plugin",
        r#"
use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    static DESCRIPTOR_JSON: &str = "not-json\0";
    DESCRIPTOR_JSON.as_ptr().cast()
}
"#,
    );

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::DecodeDescriptor);
    assert_eq!(err.path, plugin_path);
}

#[test]
fn provider_host_diagnostic_load_reports_invalid_descriptor() {
    let plugin_dir = temp_dir("diagnostic_invalid_descriptor");
    let plugin_path = compile_invalid_provider_plugin(&plugin_dir);

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::InvalidDescriptor);
    assert_eq!(err.path, plugin_path);
}
