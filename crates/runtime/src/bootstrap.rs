use std::path::{Path, PathBuf};

use viden_config::{CliOverrides, ResolvedConfig, load_config};
use viden_provider::{
    ModelProvider, ProviderConfig, ProviderHost, ProviderPluginError, ProviderRegistry,
};
use viden_types::{PermissionLevel, RuntimeSnapshot, WorkMode};

use crate::SessionEngine;

#[derive(Clone)]
pub struct RuntimeBootstrapRequest {
    pub cwd: PathBuf,
    pub cli_overrides: CliOverrides,
    pub startup_overrides: Vec<String>,
}

impl RuntimeBootstrapRequest {
    pub fn new(cwd: impl Into<PathBuf>, cli_overrides: CliOverrides) -> Self {
        Self {
            cwd: cwd.into(),
            cli_overrides,
            startup_overrides: Vec::new(),
        }
    }

    pub fn with_startup_overrides(mut self, startup_overrides: Vec<String>) -> Self {
        self.startup_overrides = startup_overrides;
        self
    }
}

pub struct RuntimeBootstrap {
    pub engine: SessionEngine,
    pub provider_summary: String,
    pub resolved_config: ResolvedConfig,
}

/// Builds the production runtime stack for every local frontend entrypoint.
///
/// Frontends select a workspace and pass CLI/config overrides here; provider
/// loading, config resolution, session storage, permission state, and runtime
/// snapshot construction stay on the shared Core/runtime path.
pub fn bootstrap_runtime(request: RuntimeBootstrapRequest) -> Result<RuntimeBootstrap, String> {
    let resolved_config = load_config(&request.cwd, &request.cli_overrides)?;
    bootstrap_runtime_with_resolved_config(&request.cwd, resolved_config, request.startup_overrides)
}

pub fn bootstrap_runtime_with_resolved_config(
    cwd: &Path,
    resolved_config: ResolvedConfig,
    startup_overrides: Vec<String>,
) -> Result<RuntimeBootstrap, String> {
    let provider_host = load_startup_provider_host(&resolved_config)?;
    let provider_selection = create_startup_provider(&provider_host, &resolved_config)?;
    let provider_summary = format!(
        "{} | config={} | files={}",
        provider_selection.summary,
        resolved_config.summary(),
        if resolved_config.loaded_files.is_empty() {
            "<none>".to_string()
        } else {
            resolved_config
                .loaded_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    let runtime_snapshot = RuntimeSnapshot {
        cwd: cwd.to_path_buf(),
        provider_family: resolved_config.provider.clone(),
        model_label: provider_selection.model_label.clone(),
        work_mode: if resolved_config.permission_mode == viden_types::PermissionMode::Plan {
            WorkMode::Plan
        } else {
            WorkMode::Build
        },
        permission_mode: resolved_config.permission_mode,
        permission_level: PermissionLevel::from_legacy_mode(resolved_config.permission_mode),
        config_summary: resolved_config.summary(),
        loaded_config_files: resolved_config.loaded_files.clone(),
        startup_overrides,
        ui_preferences: resolved_config.ui.clone(),
    };
    let mut engine = SessionEngine::new_with_home_and_snapshot(
        cwd,
        provider_selection.provider,
        resolved_config.session_home.clone(),
        runtime_snapshot,
    )?;
    engine.set_provider_runtime(
        provider_host,
        resolved_config.provider_plugin_dirs.clone(),
        resolved_config.api_base.clone(),
        resolved_config.api_key.clone(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    );
    engine.set_permission_mode(resolved_config.permission_mode)?;

    Ok(RuntimeBootstrap {
        engine,
        provider_summary,
        resolved_config,
    })
}

fn load_startup_provider_host(
    resolved_config: &viden_config::ResolvedConfig,
) -> Result<ProviderHost, String> {
    if resolved_config.provider_plugin_dirs.is_empty() {
        ProviderHost::load_default_diagnostic().map_err(format_provider_plugin_error)
    } else {
        ProviderHost::load_from_dirs_diagnostic(resolved_config.provider_plugin_dirs.clone())
            .map_err(format_provider_plugin_error)
    }
}

fn format_provider_plugin_error(err: ProviderPluginError) -> String {
    let path = if err.path.as_os_str().is_empty() {
        "<registry>".to_string()
    } else {
        err.path.display().to_string()
    };
    format!(
        "provider plugin loading failed\n  kind: {:?}\n  path: {}\n  message: {}\n  detail: {}",
        err.kind, path, err.message, err
    )
}

struct StartupProviderSelection {
    provider: Box<dyn ModelProvider>,
    model_label: String,
    summary: String,
}

fn create_startup_provider(
    host: &ProviderHost,
    resolved_config: &viden_config::ResolvedConfig,
) -> Result<StartupProviderSelection, String> {
    match ProviderConfig::from_settings(
        &resolved_config.provider,
        resolved_config.model.as_deref(),
        resolved_config.api_base.as_deref(),
        resolved_config.api_key.as_deref(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    ) {
        Ok(provider_config) => {
            let model_label = provider_config.model.clone();
            let summary = provider_config.summary();
            let provider = host.create(provider_config)?;
            Ok(StartupProviderSelection {
                provider,
                model_label,
                summary,
            })
        }
        Err(builtin_error) => create_dynamic_startup_provider(host, resolved_config)
            .map_err(|dynamic_error| format!("{builtin_error}; {dynamic_error}")),
    }
}

fn create_dynamic_startup_provider(
    host: &ProviderHost,
    resolved_config: &viden_config::ResolvedConfig,
) -> Result<StartupProviderSelection, String> {
    let registry = host.registry();
    let descriptor = registry
        .descriptor(&resolved_config.provider)
        .ok_or_else(|| format!("Provider `{}` is not registered", resolved_config.provider))?;
    let model_label = resolved_config
        .model
        .clone()
        .or_else(|| descriptor.default_model.clone())
        .ok_or_else(|| {
            format!(
                "Provider `{}` does not define a default model; pass --model",
                resolved_config.provider
            )
        })?;
    let provider = host.create_registered(
        &resolved_config.provider,
        resolved_config.model.as_deref(),
        resolved_config.api_base.as_deref(),
        resolved_config.api_key.as_deref(),
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    )?;
    Ok(StartupProviderSelection {
        provider,
        model_label,
        summary: dynamic_provider_summary(&registry, resolved_config),
    })
}

fn dynamic_provider_summary(
    registry: &ProviderRegistry,
    resolved_config: &viden_config::ResolvedConfig,
) -> String {
    let descriptor = registry.descriptor(&resolved_config.provider);
    let model = resolved_config
        .model
        .as_deref()
        .or_else(|| descriptor.and_then(|descriptor| descriptor.default_model.as_deref()))
        .unwrap_or("<required>");
    let api_base = resolved_config
        .api_base
        .clone()
        .or_else(|| {
            descriptor
                .and_then(|descriptor| descriptor.env_mappings.api_base_env.as_deref())
                .and_then(|name| std::env::var(name).ok())
        })
        .or_else(|| descriptor.and_then(|descriptor| descriptor.default_api_base.clone()))
        .unwrap_or_else(|| "<required>".to_string());
    let key_present = resolved_config.api_key.is_some()
        || descriptor
            .and_then(|descriptor| descriptor.env_mappings.api_key_env.as_deref())
            .and_then(|name| std::env::var(name).ok())
            .is_some();
    format!(
        "provider={} model={} api_base={} key={} timeout={}s retries={}",
        resolved_config.provider,
        model,
        api_base,
        if key_present { "present" } else { "missing" },
        resolved_config.request_timeout_secs,
        resolved_config.max_retries,
    )
}
