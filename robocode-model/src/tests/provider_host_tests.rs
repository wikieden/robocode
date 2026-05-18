use super::*;
use crate::plugin::{ProviderPluginErrorKind, dynamic_library_suffixes};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("robocode_provider_host_{name}_{nanos}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn compile_runtime_provider_plugin(plugin_dir: &Path) -> PathBuf {
    compile_provider_plugin(
        plugin_dir,
        "runtime_provider_plugin",
        "runtime-provider",
        "Runtime Provider",
        "https://runtime.example.com",
        "runtime-default",
    )
}

fn compile_invalid_provider_plugin(plugin_dir: &Path) -> PathBuf {
    compile_provider_plugin(
        plugin_dir,
        "invalid_provider_plugin",
        "invalid-provider",
        "Invalid Provider",
        "ftp://invalid.example.com",
        "invalid-default",
    )
}

fn compile_provider_plugin(
    plugin_dir: &Path,
    crate_name: &str,
    provider_id: &str,
    display_name: &str,
    api_base: &str,
    default_model: &str,
) -> PathBuf {
    let source = plugin_dir.join(format!("{crate_name}.rs"));
    let library = plugin_dir.join(format!("lib{crate_name}.{}", dynamic_library_suffixes()[0]));
    let source_text = r#"
use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    static DESCRIPTOR_JSON: &str = concat!(
        "{\"provider_id\":\"__PROVIDER_ID__\",",
        "\"display_name\":\"__DISPLAY_NAME__\",",
        "\"version\":\"1\",",
        "\"protocol_family\":\"OpenAi\",",
        "\"default_api_base\":\"__API_BASE__\",",
        "\"default_model\":\"__DEFAULT_MODEL__\",",
        "\"env_mappings\":{\"api_key_env\":\"RUNTIME_PROVIDER_API_KEY\",\"api_base_env\":null},",
        "\"capabilities\":{\"supports_streaming\":false,\"supports_native_tool_calling\":true},",
        "\"config_schema_version\":1}\0"
    );
    DESCRIPTOR_JSON.as_ptr().cast()
}
"#
    .replace("__PROVIDER_ID__", provider_id)
    .replace("__DISPLAY_NAME__", display_name)
    .replace("__API_BASE__", api_base)
    .replace("__DEFAULT_MODEL__", default_model);
    std::fs::write(&source, source_text).unwrap();
    let output = Command::new("rustc")
        .args(["--edition=2021", "--crate-type", "cdylib"])
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap_or_else(|err| panic!("failed to launch rustc for provider plugin test: {err}"));
    assert!(
        output.status.success(),
        "failed to compile provider plugin test dylib\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    library
}

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
fn provider_host_diagnostic_load_exposes_plugin_error_kind_and_path() {
    let plugin_dir = temp_dir("diagnostic_load_invalid");
    let plugin_path = plugin_dir.join(format!("broken.{}", dynamic_library_suffixes()[0]));
    std::fs::write(&plugin_path, b"not a real library").unwrap();

    let err = ProviderHost::load_from_dirs_diagnostic(vec![plugin_dir]).unwrap_err();

    assert_eq!(err.kind, ProviderPluginErrorKind::LoadLibrary);
    assert_eq!(err.path, plugin_path);
}
