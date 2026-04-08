use std::fs;
use std::path::{Path, PathBuf};

use robocode_types::PermissionMode;
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    pub session_home: Option<PathBuf>,
    pub request_timeout_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub provider: String,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub permission_mode: PermissionMode,
    pub session_home: Option<PathBuf>,
    pub request_timeout_secs: u64,
    pub max_retries: u32,
    pub loaded_files: Vec<PathBuf>,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: None,
            api_base: None,
            api_key: None,
            permission_mode: PermissionMode::Default,
            session_home: None,
            request_timeout_secs: 90,
            max_retries: 1,
            loaded_files: Vec::new(),
        }
    }
}

impl ResolvedConfig {
    pub fn summary(&self) -> String {
        format!(
            "provider={} model={} permission_mode={} session_home={} timeout={}s retries={}",
            self.provider,
            self.model.as_deref().unwrap_or("<default>"),
            self.permission_mode.cli_name(),
            self.session_home
                .as_deref()
                .map(Path::display)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<default>".to_string()),
            self.request_timeout_secs,
            self.max_retries,
        )
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    provider: Option<String>,
    model: Option<String>,
    api_base: Option<String>,
    api_key: Option<String>,
    api_key_env: Option<String>,
    permission_mode: Option<String>,
    session_home: Option<String>,
    request_timeout_secs: Option<u64>,
    max_retries: Option<u32>,
}

pub fn load_config(cwd: &Path, cli: &CliOverrides) -> Result<ResolvedConfig, String> {
    load_config_with_env(cwd, cli, &|key| std::env::var(key).ok())
}

fn load_config_with_env<F>(
    cwd: &Path,
    cli: &CliOverrides,
    env_lookup: &F,
) -> Result<ResolvedConfig, String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut resolved = ResolvedConfig::default();
    let mut loaded_files = Vec::new();

    for path in config_paths(cwd, cli, env_lookup)? {
        if let Some(file_config) = read_config_file(&path, env_lookup)? {
            apply_file_config(&mut resolved, file_config, cwd)?;
            loaded_files.push(path);
        }
    }

    apply_env_config(&mut resolved, env_lookup)?;
    apply_cli_config(&mut resolved, cli);
    resolved.loaded_files = loaded_files;
    Ok(resolved)
}

fn config_paths<F>(cwd: &Path, cli: &CliOverrides, env_lookup: &F) -> Result<Vec<PathBuf>, String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(path) = cli
        .config_path
        .clone()
        .or_else(|| env_lookup("ROBOCODE_CONFIG").map(PathBuf::from))
    {
        return Ok(vec![path]);
    }

    let mut paths = Vec::new();
    if let Some(global) = default_config_path(env_lookup) {
        paths.push(global);
    }
    paths.push(cwd.join(".robocode").join("config.toml"));
    Ok(paths)
}

fn default_config_path<F>(env_lookup: &F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    if cfg!(windows) {
        env_lookup("APPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("robocode").join("config.toml"))
    } else if cfg!(target_os = "macos") {
        env_lookup("HOME").map(PathBuf::from).map(|base| {
            base.join("Library")
                .join("Application Support")
                .join("robocode")
                .join("config.toml")
        })
    } else {
        env_lookup("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env_lookup("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|base| base.join("robocode").join("config.toml"))
    }
}

fn read_config_file<F>(path: &Path, env_lookup: &F) -> Result<Option<FileConfig>, String>
where
    F: Fn(&str) -> Option<String>,
{
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read config {}: {err}", path.display()))?;
    let mut config: FileConfig = toml::from_str(&contents)
        .map_err(|err| format!("Failed to parse config {}: {err}", path.display()))?;
    if config.api_key.is_none() {
        if let Some(name) = config.api_key_env.as_deref() {
            config.api_key = env_lookup(name);
        }
    }
    Ok(Some(config))
}

fn apply_file_config(
    resolved: &mut ResolvedConfig,
    file: FileConfig,
    cwd: &Path,
) -> Result<(), String> {
    if let Some(provider) = file.provider {
        resolved.provider = provider;
    }
    if let Some(model) = file.model {
        resolved.model = Some(model);
    }
    if let Some(api_base) = file.api_base {
        resolved.api_base = Some(api_base);
    }
    if let Some(api_key) = file.api_key {
        resolved.api_key = Some(api_key);
    }
    if let Some(permission_mode) = file.permission_mode {
        resolved.permission_mode = PermissionMode::parse_cli(&permission_mode)
            .ok_or_else(|| format!("Unknown permission mode `{permission_mode}` in config"))?;
    }
    if let Some(session_home) = file.session_home {
        resolved.session_home = Some(resolve_path(cwd, &session_home));
    }
    if let Some(request_timeout_secs) = file.request_timeout_secs {
        resolved.request_timeout_secs = request_timeout_secs.max(1);
    }
    if let Some(max_retries) = file.max_retries {
        resolved.max_retries = max_retries;
    }
    Ok(())
}

fn apply_env_config<F>(resolved: &mut ResolvedConfig, env_lookup: &F) -> Result<(), String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(provider) = env_lookup("ROBOCODE_PROVIDER") {
        resolved.provider = provider;
    }
    if let Some(model) = env_lookup("ROBOCODE_MODEL") {
        if !model.trim().is_empty() {
            resolved.model = Some(model);
        }
    }
    if let Some(api_base) = env_lookup("ROBOCODE_API_BASE") {
        resolved.api_base = Some(api_base);
    }
    if let Some(api_key) = env_lookup("ROBOCODE_API_KEY") {
        resolved.api_key = Some(api_key);
    }
    if let Some(permission_mode) = env_lookup("ROBOCODE_PERMISSION_MODE") {
        resolved.permission_mode = PermissionMode::parse_cli(&permission_mode)
            .ok_or_else(|| format!("Unknown permission mode `{permission_mode}` in environment"))?;
    }
    if let Some(session_home) = env_lookup("ROBOCODE_SESSION_HOME") {
        resolved.session_home = Some(PathBuf::from(session_home));
    }
    if let Some(request_timeout_secs) = env_lookup("ROBOCODE_REQUEST_TIMEOUT_SECS") {
        resolved.request_timeout_secs = request_timeout_secs
            .parse::<u64>()
            .map_err(|_| "ROBOCODE_REQUEST_TIMEOUT_SECS must be an integer".to_string())?
            .max(1);
    }
    if let Some(max_retries) = env_lookup("ROBOCODE_MAX_RETRIES") {
        resolved.max_retries = max_retries
            .parse::<u32>()
            .map_err(|_| "ROBOCODE_MAX_RETRIES must be an integer".to_string())?;
    }
    Ok(())
}

fn apply_cli_config(resolved: &mut ResolvedConfig, cli: &CliOverrides) {
    if let Some(provider) = &cli.provider {
        resolved.provider = provider.clone();
    }
    if let Some(model) = &cli.model {
        resolved.model = Some(model.clone());
    }
    if let Some(api_base) = &cli.api_base {
        resolved.api_base = Some(api_base.clone());
    }
    if let Some(api_key) = &cli.api_key {
        resolved.api_key = Some(api_key.clone());
    }
    if let Some(permission_mode) = cli.permission_mode {
        resolved.permission_mode = permission_mode;
    }
    if let Some(session_home) = &cli.session_home {
        resolved.session_home = Some(session_home.clone());
    }
    if let Some(request_timeout_secs) = cli.request_timeout_secs {
        resolved.request_timeout_secs = request_timeout_secs.max(1);
    }
    if let Some(max_retries) = cli.max_retries {
        resolved.max_retries = max_retries;
    }
}

fn resolve_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn map_env(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn project_file_overrides_global_file_and_env_overrides_files() {
        let root = std::env::temp_dir().join(format!("robocode_config_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(
            root.join("Library")
                .join("Application Support")
                .join("robocode"),
        )
        .unwrap();
        fs::create_dir_all(root.join("project").join(".robocode")).unwrap();
        fs::write(
            root.join("Library")
                .join("Application Support")
                .join("robocode")
                .join("config.toml"),
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
        let cli = CliOverrides {
            provider: Some("openai".to_string()),
            model: Some("gpt-5.2".to_string()),
            permission_mode: Some(PermissionMode::AcceptEdits),
            request_timeout_secs: Some(120),
            max_retries: Some(3),
            ..CliOverrides::default()
        };
        let env_map: BTreeMap<String, String> = BTreeMap::new();
        let config = load_config_with_env(&cwd, &cli, &|key| env_map.get(key).cloned()).unwrap();
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model.as_deref(), Some("gpt-5.2"));
        assert_eq!(config.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(config.request_timeout_secs, 120);
        assert_eq!(config.max_retries, 3);
    }
}
