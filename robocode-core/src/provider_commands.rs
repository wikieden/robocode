use crate::SessionEngine;
use robocode_model::{ProviderConfig, ProviderDescriptor, ProviderPluginError};

impl SessionEngine {
    pub(super) fn handle_provider_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None => Ok(format!(
                "Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            )),
            Some("list") => Ok(self.render_provider_list()),
            Some("doctor") => Ok(self.render_provider_doctor(args.get(1).map(String::as_str))),
            Some("reload") => self.reload_provider_registry(),
            Some("use") => self.use_provider(&args[1..]),
            Some("help") => Ok(provider_help()),
            Some(subcommand) => Ok(format!(
                "Unknown provider subcommand `{subcommand}`.\n\n{}",
                provider_help()
            )),
        }
    }

    pub(super) fn handle_settings_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None | Some("show") | Some("help") => Ok(self.render_settings()),
            Some("provider") | Some("use") => {
                let output = self.use_provider(&args[1..])?;
                let saved = self.save_current_provider_model_defaults()?;
                Ok(format!("{output}\n{saved}"))
            }
            Some("model") => {
                let Some(model) = args.get(1) else {
                    return Ok(settings_help());
                };
                self.provider.set_model(model.clone());
                self.runtime_snapshot.model_label = self.provider.model().to_string();
                self.persist_meta("model", self.provider.model())?;
                let saved = self.save_current_provider_model_defaults()?;
                Ok(format!("Model set to {}\n{saved}", self.provider.model()))
            }
            Some("save") => self.save_current_provider_model_defaults(),
            Some(subcommand) => Ok(format!(
                "Unknown settings subcommand `{subcommand}`.\n\n{}",
                settings_help()
            )),
        }
    }

    fn render_provider_list(&self) -> String {
        let Some(host) = self.provider_host.as_ref() else {
            return [
                "Provider registry:",
                "  Runtime registry: unavailable",
                "  Start RoboCode through the CLI to enable provider plugin commands.",
                "",
                &format!(
                    "Current provider: {} ({})",
                    self.provider.provider_name(),
                    self.provider.model()
                ),
            ]
            .join("\n");
        };
        let registry = host.registry();
        let mut descriptors = registry.descriptors().to_vec();
        descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        let mut lines = vec![
            "Provider registry:".to_string(),
            format!(
                "  Plugin dirs: {}",
                if self.provider_plugin_dirs.is_empty() {
                    "<default>".to_string()
                } else {
                    self.provider_plugin_dirs
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            format!(
                "  Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            ),
        ];
        lines.extend(descriptors.iter().map(render_provider_descriptor));
        lines.join("\n")
    }

    fn render_settings(&self) -> String {
        let config_path = robocode_config::default_user_config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|err| format!("<unavailable: {err}>"));
        let mut lines = vec![
            "RoboCode settings:".to_string(),
            format!(
                "  Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            ),
            format!("  API key: {}", self.current_provider_key_status()),
            format!("  User config: {config_path}"),
            "  Persist default: /settings save".to_string(),
            "  Choose provider: /settings provider <id> [model]".to_string(),
            "  Choose model: /settings model <model>".to_string(),
            "  Diagnostics: /provider doctor [id]".to_string(),
        ];
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            lines.push("".to_string());
            lines.push("Available providers:".to_string());
            lines.extend(descriptors.iter().map(|descriptor| {
                format!(
                    "  - {} default_model={} key={}",
                    descriptor.provider_id,
                    descriptor.default_model.as_deref().unwrap_or("<required>"),
                    descriptor_key_status(descriptor)
                )
            }));
        } else {
            lines.push("".to_string());
            lines.push("Available providers: runtime registry unavailable".to_string());
        }
        lines.join("\n")
    }

    fn render_provider_doctor(&self, provider_id: Option<&str>) -> String {
        let Some(host) = self.provider_host.as_ref() else {
            return [
                "Provider diagnostics:",
                "  Runtime registry: unavailable",
                "  Start RoboCode through the CLI to enable provider diagnostics.",
                "",
                &format!(
                    "Current provider: {} ({})",
                    self.provider.provider_name(),
                    self.provider.model()
                ),
            ]
            .join("\n");
        };
        let registry = host.registry();
        let mut descriptors = registry.descriptors().to_vec();
        descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        if let Some(provider_id) = provider_id {
            let Some(descriptor) = descriptors
                .iter()
                .find(|descriptor| descriptor.provider_id == provider_id)
            else {
                return format!("Provider `{provider_id}` is not registered.");
            };
            return [
                format!("Provider diagnostics: {provider_id}"),
                format!(
                    "  Current provider: {} ({})",
                    self.provider.provider_name(),
                    self.provider.model()
                ),
                render_provider_diagnostic(descriptor),
            ]
            .join("\n");
        }
        let mut lines = vec![
            "Provider diagnostics:".to_string(),
            format!("  Registry providers: {}", descriptors.len()),
            format!(
                "  Plugin dirs: {}",
                if self.provider_plugin_dirs.is_empty() {
                    "<default>".to_string()
                } else {
                    self.provider_plugin_dirs
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            format!(
                "  Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            ),
        ];
        lines.extend(descriptors.iter().map(render_provider_diagnostic));
        lines.join("\n")
    }

    fn reload_provider_registry(&mut self) -> Result<String, String> {
        let Some(host) = self.provider_host.as_mut() else {
            return Ok([
                "Provider registry reload unavailable.",
                "Start RoboCode through the CLI to enable provider plugin commands.",
            ]
            .join("\n"));
        };
        let before_count = host.registry().descriptors().len();
        let result = if self.provider_plugin_dirs.is_empty() {
            host.refresh_diagnostic()
        } else {
            host.refresh_from_dirs_diagnostic(self.provider_plugin_dirs.clone())
        };
        match result {
            Ok(()) => {
                let after_count = host.registry().descriptors().len();
                Ok(format!(
                    "Provider registry reloaded: {before_count} -> {after_count} providers.\nCurrent provider instance remains {} ({}).",
                    self.provider.provider_name(),
                    self.provider.model()
                ))
            }
            Err(err) => Ok(format!(
                "Provider registry reload failed.\n{}\nPrevious registry remains active with {before_count} providers.",
                format_provider_plugin_error(&err)
            )),
        }
    }

    fn use_provider(&mut self, args: &[String]) -> Result<String, String> {
        let Some(provider_id) = args.first().map(String::as_str) else {
            return Ok(provider_help());
        };
        let requested_model = args.get(1).map(String::as_str);
        let Some(host) = self.provider_host.as_ref() else {
            return Ok([
                "Provider switching unavailable.",
                "Start RoboCode through the CLI to enable provider runtime commands.",
            ]
            .join("\n"));
        };
        let registry = host.registry();
        let Some(descriptor) = registry.descriptor(provider_id) else {
            return Ok(format!("Provider `{provider_id}` is not registered."));
        };
        let model = requested_model
            .map(ToString::to_string)
            .or_else(|| descriptor.default_model.clone())
            .ok_or_else(|| {
                format!("Provider `{provider_id}` does not define a default model; pass a model")
            })?;
        let next_provider = self.create_provider_from_runtime(provider_id, Some(&model))?;
        self.provider = next_provider;
        self.runtime_snapshot.provider_family = provider_id.to_string();
        self.runtime_snapshot.model_label = self.provider.model().to_string();
        self.persist_meta("provider", provider_id)?;
        self.persist_meta("model", self.provider.model())?;
        Ok(format!(
            "Provider set to {} ({})",
            self.provider.provider_name(),
            self.provider.model()
        ))
    }

    fn save_current_provider_model_defaults(&self) -> Result<String, String> {
        let path = robocode_config::save_user_provider_model_defaults(
            self.provider.provider_name(),
            self.provider.model(),
        )?;
        Ok(format!(
            "Saved default provider/model to {}",
            path.display()
        ))
    }

    fn current_provider_key_status(&self) -> String {
        if self.provider.provider_name() == "fallback" {
            return "not required".to_string();
        }
        if self.provider_api_key.is_some() {
            return "present".to_string();
        }
        if let Some(host) = self.provider_host.as_ref() {
            let registry = host.registry();
            if let Some(descriptor) = registry.descriptor(self.provider.provider_name()) {
                return descriptor_key_status(descriptor);
            }
        }
        "unknown".to_string()
    }

    pub(super) fn create_provider_from_runtime(
        &self,
        provider_id: &str,
        model: Option<&str>,
    ) -> Result<Box<dyn robocode_model::ModelProvider>, String> {
        let Some(host) = self.provider_host.as_ref() else {
            return Err("Provider runtime is unavailable.".to_string());
        };
        let provider = match ProviderConfig::from_settings(
            provider_id,
            model,
            self.provider_api_base.as_deref(),
            self.provider_api_key.as_deref(),
            self.provider_request_timeout_secs,
            self.provider_max_retries,
        ) {
            Ok(config) => host.create(config)?,
            Err(_) => host.create_registered(
                provider_id,
                model,
                self.provider_api_base.as_deref(),
                self.provider_api_key.as_deref(),
                self.provider_request_timeout_secs,
                self.provider_max_retries,
            )?,
        };
        Ok(provider)
    }
}

fn render_provider_descriptor(descriptor: &ProviderDescriptor) -> String {
    format!(
        "  - {} ({}) family={:?} default_model={} streaming={} tools={} compat={}",
        descriptor.provider_id,
        descriptor.display_name,
        descriptor.protocol_family,
        descriptor.default_model.as_deref().unwrap_or("<none>"),
        descriptor.capabilities.supports_streaming,
        descriptor.capabilities.supports_native_tool_calling,
        render_provider_compatibility(descriptor),
    )
}

fn render_provider_diagnostic(descriptor: &ProviderDescriptor) -> String {
    format!(
        "  - {} family={:?} default_model={} api_base={} api_key_env={} api_base_env={} streaming={} tools={} compat={}",
        descriptor.provider_id,
        descriptor.protocol_family,
        descriptor.default_model.as_deref().unwrap_or("<none>"),
        descriptor.default_api_base.as_deref().unwrap_or("<none>"),
        render_env_status(descriptor.env_mappings.api_key_env.as_deref()),
        render_env_status(descriptor.env_mappings.api_base_env.as_deref()),
        descriptor.capabilities.supports_streaming,
        descriptor.capabilities.supports_native_tool_calling,
        render_provider_compatibility(descriptor),
    )
}

fn render_provider_compatibility(descriptor: &ProviderDescriptor) -> String {
    let compatibility = &descriptor.compatibility;
    let mut parts = Vec::new();
    if !compatibility.supports_tool_choice {
        parts.push("tool_choice=false".to_string());
    }
    if compatibility.requires_reasoning_content_for_tool_calls {
        parts.push("reasoning_content=required".to_string());
    }
    if compatibility.requires_non_null_tool_call_content {
        parts.push("tool_call_content=non-null".to_string());
    }
    if let Some(effort) = compatibility.reasoning_effort_high.as_deref() {
        parts.push(format!("effort_high={effort}"));
    }
    if let Some(effort) = compatibility.reasoning_effort_max.as_deref() {
        parts.push(format!("effort_max={effort}"));
    }
    if parts.is_empty() {
        "default".to_string()
    } else {
        parts.join(", ")
    }
}

fn render_env_status(env_name: Option<&str>) -> String {
    match env_name {
        Some(name) if std::env::var_os(name).is_some() => format!("{name}(present)"),
        Some(name) => format!("{name}(missing)"),
        None => "<none>".to_string(),
    }
}

fn descriptor_key_status(descriptor: &ProviderDescriptor) -> String {
    match descriptor.env_mappings.api_key_env.as_deref() {
        Some(name) if std::env::var_os(name).is_some() => format!("{name}:present"),
        Some(name) => format!("{name}:missing"),
        None if descriptor.provider_id == "fallback" => "not required".to_string(),
        None => "unknown".to_string(),
    }
}

fn format_provider_plugin_error(err: &ProviderPluginError) -> String {
    let path = if err.path.as_os_str().is_empty() {
        "<registry>".to_string()
    } else {
        err.path.display().to_string()
    };
    format!(
        "  kind: {:?}\n  path: {}\n  message: {}\n  detail: {}",
        err.kind, path, err.message, err
    )
}

fn provider_help() -> String {
    [
        "Provider commands:",
        "  /provider          Show current provider and model",
        "  /provider list     List registered providers",
        "  /provider doctor [id]",
        "                     Show provider registry diagnostics",
        "  /provider reload   Reload provider plugin registry",
        "  /provider use <id> [model]",
    ]
    .join("\n")
}

fn settings_help() -> String {
    [
        "Settings commands:",
        "  /settings                  Show provider/model setup status",
        "  /settings provider <id> [model]",
        "                             Switch provider/model and save defaults",
        "  /settings model <model>    Switch current model and save defaults",
        "  /settings save             Save current provider/model as defaults",
        "  /setup                     Alias for /settings",
    ]
    .join("\n")
}
