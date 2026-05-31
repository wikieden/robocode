use super::*;
use std::collections::BTreeMap;

fn default_config_path_for_test(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join("AppData")
            .join("Roaming")
            .join("robocode")
            .join("config.toml")
    } else if cfg!(target_os = "macos") {
        root.join("Library")
            .join("Application Support")
            .join("robocode")
            .join("config.toml")
    } else {
        root.join(".config").join("robocode").join("config.toml")
    }
}

fn map_env(values: &[(&str, &str)]) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn default_config_uses_deepseek_as_online_provider() {
    let cwd =
        std::env::temp_dir().join(format!("robocode_default_deepseek_{}", std::process::id()));
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(&cwd).unwrap();

    let config = load_config_with_env(&cwd, &CliOverrides::default(), &|_| None).unwrap();

    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.model, None);
}

#[test]
fn project_file_overrides_global_file_and_env_overrides_files() {
    let root = std::env::temp_dir().join(format!("robocode_config_{}", std::process::id()));
    let global_config_path = default_config_path_for_test(&root);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
    fs::write(
        global_config_path,
        "provider = 'anthropic'\nmodel = 'global-model'\npermission_mode = 'default'\n",
    )
    .unwrap();
    fs::write(
        root.join("project").join(".robocode").join("config.toml"),
        "model = 'project-model'\npermission_mode = 'plan'\nrequest_timeout_secs = 45\n",
    )
    .unwrap();
    let env_map = map_env(&[
        ("HOME", root.to_string_lossy().as_ref()),
        ("ROBOCODE_MODEL", "env-model"),
    ]);
    let cli = CliOverrides::default();
    let config = load_config_with_env(&root.join("project"), &cli, &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model.as_deref(), Some("env-model"));
    assert_eq!(config.permission_mode, PermissionMode::Plan);
    assert_eq!(config.request_timeout_secs, 45);
    assert_eq!(config.loaded_files.len(), 2);
}

#[test]
fn cli_overrides_win() {
    let cwd = std::env::temp_dir();
    let plugin_dir = cwd.join("cli-provider-plugins");
    let cli = CliOverrides {
        provider: Some("openai".to_string()),
        model: Some("gpt-5.2".to_string()),
        provider_plugin_dirs: vec![plugin_dir.clone()],
        permission_mode: Some(PermissionMode::AcceptEdits),
        request_timeout_secs: Some(120),
        max_retries: Some(3),
        ..CliOverrides::default()
    };
    let env_map: BTreeMap<String, String> = BTreeMap::new();
    let config = load_config_with_env(&cwd, &cli, &|key| env_map.get(key).cloned()).unwrap();
    assert_eq!(config.provider, "openai");
    assert_eq!(config.model.as_deref(), Some("gpt-5.2"));
    assert_eq!(config.provider_plugin_dirs, vec![plugin_dir]);
    assert_eq!(config.permission_mode, PermissionMode::AcceptEdits);
    assert_eq!(config.request_timeout_secs, 120);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn provider_plugin_dirs_resolve_from_file_env_and_cli_precedence() {
    let cwd = std::env::temp_dir().join(format!(
        "robocode_plugin_dirs_config_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join(".robocode")).unwrap();
    fs::write(
        cwd.join(".robocode").join("config.toml"),
        r#"
provider_plugin_dirs = ["relative-plugins", "/absolute-plugins"]
"#,
    )
    .unwrap();

    let file_config = load_config_with_env(&cwd, &CliOverrides::default(), &|_| None).unwrap();
    assert_eq!(
        file_config.provider_plugin_dirs,
        vec![
            cwd.join("relative-plugins"),
            PathBuf::from("/absolute-plugins")
        ]
    );

    let env_dirs = std::env::join_paths([cwd.join("env-a"), cwd.join("env-b")]).unwrap();
    let env_dirs = env_dirs.to_string_lossy().to_string();
    let env_map = map_env(&[("ROBOCODE_PROVIDER_PLUGIN_DIRS", env_dirs.as_str())]);
    let env_config = load_config_with_env(&cwd, &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();
    assert_eq!(
        env_config.provider_plugin_dirs,
        vec![cwd.join("env-a"), cwd.join("env-b")]
    );

    let cli_dir = cwd.join("cli-plugins");
    let cli = CliOverrides {
        provider_plugin_dirs: vec![cli_dir.clone()],
        ..CliOverrides::default()
    };
    let cli_config = load_config_with_env(&cwd, &cli, &|key| env_map.get(key).cloned()).unwrap();
    assert_eq!(cli_config.provider_plugin_dirs, vec![cli_dir]);
}

#[test]
fn deepseek_provider_specific_env_overrides_generic_api_fields() {
    let cwd = std::env::temp_dir().join("robocode_deepseek_config_test");
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join(".robocode")).unwrap();
    fs::write(
        cwd.join(".robocode").join("config.toml"),
        r#"
provider = "deepseek"
api_key = "generic-key"
api_base = "https://generic.example"
[providers.deepseek]
api_base = "https://provider.example"
"#,
    )
    .unwrap();

    let env_map = map_env(&[("DEEPSEEK_API_KEY", "provider-key")]);

    let config = load_config_with_env(&cwd, &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.api_key.as_deref(), Some("provider-key"));
    assert_eq!(config.api_base.as_deref(), Some("https://provider.example"));
}

#[test]
fn save_user_provider_model_defaults_updates_existing_config_without_secrets() {
    let root = std::env::temp_dir().join(format!(
        "robocode_save_provider_model_{}",
        std::process::id()
    ));
    let path = root.join("config.toml");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &path,
        r#"
permission_mode = "plan"

[providers.deepseek]
api_base = "https://api.deepseek.example"
"#,
    )
    .unwrap();

    save_user_provider_model_defaults_at(&path, "deepseek", "deepseek-v4-flash").unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains(r#"provider = "deepseek""#));
    assert!(contents.contains(r#"model = "deepseek-v4-flash""#));
    assert!(contents.contains(r#"permission_mode = "plan""#));
    assert!(contents.contains("[providers.deepseek]"));
    assert!(!contents.contains("api_key"));

    let cli = CliOverrides {
        config_path: Some(path),
        ..CliOverrides::default()
    };
    let config = load_config_with_env(&root, &cli, &|_| None).unwrap();
    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.model.as_deref(), Some("deepseek-v4-flash"));
    assert_eq!(config.permission_mode, PermissionMode::Plan);
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://api.deepseek.example")
    );
}

#[test]
fn save_user_provider_config_updates_scoped_provider_fields() {
    let root = std::env::temp_dir().join(format!(
        "robocode_save_provider_config_{}",
        std::process::id()
    ));
    let path = root.join("config.toml");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &path,
        r#"
provider = "deepseek"
permission_mode = "plan"

[providers.deepseek]
api_base = "https://old.example"
"#,
    )
    .unwrap();

    save_user_provider_config_at(
        &path,
        "deepseek",
        ProviderConfigUpdate {
            api_base: Some("https://api.deepseek.com".to_string()),
            api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
            default_model: Some("deepseek-v4-pro".to_string()),
            models: None,
            favorite_models: None,
        },
    )
    .unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains(r#"provider = "deepseek""#));
    assert!(contents.contains(r#"permission_mode = "plan""#));
    assert!(contents.contains("[providers.deepseek]"));
    assert!(contents.contains(r#"api_base = "https://api.deepseek.com""#));
    assert!(contents.contains(r#"api_key_env = "DEEPSEEK_API_KEY""#));
    assert!(contents.contains(r#"default_model = "deepseek-v4-pro""#));
    assert!(!contents.contains("api_key ="));

    let cli = CliOverrides {
        config_path: Some(path),
        provider: Some("deepseek".to_string()),
        model: None,
        ..CliOverrides::default()
    };
    let env_map = map_env(&[("DEEPSEEK_API_KEY", "deepseek-provider-key")]);
    let config = load_config_with_env(&root, &cli, &|key| env_map.get(key).cloned()).unwrap();

    assert_eq!(config.api_base.as_deref(), Some("https://api.deepseek.com"));
    assert_eq!(config.api_key.as_deref(), Some("deepseek-provider-key"));
    assert_eq!(config.model.as_deref(), Some("deepseek-v4-pro"));
}

#[test]
fn save_user_provider_config_rejects_empty_updates() {
    let root = std::env::temp_dir().join(format!(
        "robocode_save_provider_config_empty_{}",
        std::process::id()
    ));
    let path = root.join("config.toml");
    let _ = fs::remove_dir_all(&root);

    let err = save_user_provider_config_at(
        &path,
        "deepseek",
        ProviderConfigUpdate {
            api_base: Some(" ".to_string()),
            ..ProviderConfigUpdate::default()
        },
    )
    .unwrap_err();

    assert!(err.contains("API base cannot be empty"));
}

#[test]
fn add_user_provider_favorite_model_moves_unique_model_to_front() {
    let root = std::env::temp_dir().join(format!(
        "robocode_favorite_provider_model_{}",
        std::process::id()
    ));
    let path = root.join("config.toml");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &path,
        r#"
[providers.deepseek]
models = ["deepseek-v4-flash", "deepseek-v4-pro"]
favorite_models = ["deepseek-v4-flash"]
"#,
    )
    .unwrap();

    add_user_provider_favorite_model_at(&path, "deepseek", "deepseek-v4-pro").unwrap();

    let config = load_provider_ui_config_at(&path).unwrap();
    let deepseek = config.get("deepseek").unwrap();
    assert_eq!(
        deepseek.models,
        vec![
            "deepseek-v4-flash".to_string(),
            "deepseek-v4-pro".to_string()
        ]
    );
    assert_eq!(
        deepseek.favorite_models,
        vec![
            "deepseek-v4-pro".to_string(),
            "deepseek-v4-flash".to_string()
        ]
    );
}

#[test]
fn deepseek_provider_scoped_config_from_global_applies_after_project_provider_selection() {
    let root =
        std::env::temp_dir().join(format!("robocode_deepseek_global_{}", std::process::id()));
    let global_config_path = default_config_path_for_test(&root);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
    fs::write(
        &global_config_path,
        r#"
[providers.deepseek]
api_base = "https://global-provider.example"
"#,
    )
    .unwrap();
    fs::write(
        root.join("project").join(".robocode").join("config.toml"),
        r#"
provider = "deepseek"
api_base = "https://generic-project.example"
"#,
    )
    .unwrap();

    let env_map = map_env(&[("HOME", root.to_string_lossy().as_ref())]);
    let config = load_config_with_env(&root.join("project"), &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek");
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://global-provider.example")
    );
}

#[test]
fn deepseek_provider_scoped_config_applies_when_provider_is_selected_by_cli() {
    let root = std::env::temp_dir().join(format!("robocode_deepseek_cli_{}", std::process::id()));
    let global_config_path = default_config_path_for_test(&root);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
    fs::write(
        &global_config_path,
        r#"
[providers.deepseek]
api_base = "https://global-provider.example"
"#,
    )
    .unwrap();

    let env_map = map_env(&[("HOME", root.to_string_lossy().as_ref())]);
    let cli = CliOverrides {
        provider: Some("deepseek".to_string()),
        ..CliOverrides::default()
    };
    let config = load_config_with_env(&root.join("project"), &cli, &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek");
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://global-provider.example")
    );
}

#[test]
fn deepseek_anthropic_uses_deepseek_scoped_config() {
    let root = std::env::temp_dir().join(format!(
        "robocode_deepseek_anthropic_{}",
        std::process::id()
    ));
    let global_config_path = default_config_path_for_test(&root);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
    fs::write(
        &global_config_path,
        r#"
[providers.deepseek]
api_base = "https://api.deepseek.com/anthropic"
api_key_env = "DEEPSEEK_API_KEY"
"#,
    )
    .unwrap();

    let env_map = map_env(&[
        ("HOME", root.to_string_lossy().as_ref()),
        ("DEEPSEEK_API_KEY", "deepseek-provider-key"),
    ]);
    let cli = CliOverrides {
        provider: Some("deepseek-anthropic".to_string()),
        ..CliOverrides::default()
    };
    let config = load_config_with_env(&root.join("project"), &cli, &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek-anthropic");
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://api.deepseek.com/anthropic")
    );
    assert_eq!(config.api_key.as_deref(), Some("deepseek-provider-key"));
}

#[test]
fn arbitrary_provider_scoped_config_applies_to_selected_provider() {
    let root =
        std::env::temp_dir().join(format!("robocode_openrouter_scoped_{}", std::process::id()));
    let global_config_path = default_config_path_for_test(&root);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
    fs::write(
        &global_config_path,
        r#"
[providers.openrouter]
api_base = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
default_model = "openai/gpt-5.2"
"#,
    )
    .unwrap();

    let env_map = map_env(&[
        ("HOME", root.to_string_lossy().as_ref()),
        ("OPENROUTER_API_KEY", "openrouter-key"),
    ]);
    let cli = CliOverrides {
        provider: Some("openrouter".to_string()),
        ..CliOverrides::default()
    };
    let config = load_config_with_env(&root.join("project"), &cli, &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "openrouter");
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://openrouter.ai/api/v1")
    );
    assert_eq!(config.api_key.as_deref(), Some("openrouter-key"));
    assert_eq!(config.model.as_deref(), Some("openai/gpt-5.2"));
}

#[test]
fn provider_specific_env_uses_normalized_provider_name() {
    let cwd = std::env::temp_dir().join(format!("robocode_openrouter_env_{}", std::process::id()));
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join(".robocode")).unwrap();
    fs::write(
        cwd.join(".robocode").join("config.toml"),
        r#"
provider = "openrouter"
api_key = "generic-key"
api_base = "https://generic.example"
"#,
    )
    .unwrap();

    let env_map = map_env(&[
        ("OPENROUTER_API_KEY", "provider-key"),
        ("OPENROUTER_API_BASE", "https://provider.example"),
    ]);
    let config = load_config_with_env(&cwd, &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "openrouter");
    assert_eq!(config.api_key.as_deref(), Some("provider-key"));
    assert_eq!(config.api_base.as_deref(), Some("https://provider.example"));
}

#[test]
fn provider_specific_env_uses_shared_family_aliases() {
    let cwd = std::env::temp_dir().join(format!(
        "robocode_deepseek_anthropic_env_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join(".robocode")).unwrap();
    fs::write(
        cwd.join(".robocode").join("config.toml"),
        r#"
provider = "deepseek-anthropic"
"#,
    )
    .unwrap();

    let env_map = map_env(&[
        ("DEEPSEEK_API_KEY", "provider-key"),
        ("DEEPSEEK_API_BASE", "https://api.deepseek.com/anthropic"),
    ]);
    let config = load_config_with_env(&cwd, &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek-anthropic");
    assert_eq!(config.api_key.as_deref(), Some("provider-key"));
    assert_eq!(
        config.api_base.as_deref(),
        Some("https://api.deepseek.com/anthropic")
    );
}
