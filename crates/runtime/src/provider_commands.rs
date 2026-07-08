use crate::SessionEngine;
use viden_config::ProviderConfigUpdate;
use viden_provider::{ProviderAuthMode, ProviderConfig, ProviderDescriptor, ProviderPluginError};
use viden_types::PermissionMode;

impl SessionEngine {
    pub(super) fn handle_connect_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None => Ok(self.render_provider_picker("/connect")),
            Some(provider_id) if args.len() == 1 => {
                Ok(self.render_connect_provider_detail(provider_id))
            }
            Some(provider_id) => {
                self.handle_settings_provider(&args_for_provider(provider_id, &args[1..]))
            }
        }
    }

    pub(super) fn handle_provider_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None => Ok(self.render_provider_picker("/provider")),
            Some("list") => Ok(self.render_provider_list()),
            Some("doctor") => Ok(self.render_provider_doctor(args.get(1).map(String::as_str))),
            Some("reload") => self.reload_provider_registry(),
            Some("use") => self.use_provider_and_maybe_save(&args[1..], false),
            Some("help") => Ok(provider_help()),
            Some(provider_id) => {
                let rest = args
                    .get(1..)
                    .map(|items| items.to_vec())
                    .unwrap_or_default();
                let mut provider_args = Vec::with_capacity(rest.len() + 1);
                provider_args.push(provider_id.to_string());
                provider_args.extend(rest);
                self.use_provider_and_maybe_save(&provider_args, true)
            }
        }
    }

    pub(super) fn handle_model_command(&mut self, args: &[String]) -> Result<String, String> {
        let Some(model) = args.first() else {
            return Ok(self.render_model_picker("/model"));
        };
        self.provider.set_model(model.clone());
        self.runtime_snapshot.model_label = self.provider.model().to_string();
        self.persist_meta("model", self.provider.model())?;
        let saved = self.save_current_provider_model_defaults()?;
        Ok(format!(
            "Model set to {}\nCurrent provider: {} ({})\n{saved}\nNext live turn uses {} / {}.",
            self.provider.model(),
            self.provider.provider_name(),
            self.provider.model(),
            self.provider.provider_name(),
            self.provider.model()
        ))
    }

    pub(super) fn handle_models_command(&mut self, args: &[String]) -> Result<String, String> {
        match args {
            [] => Ok(self.render_models_picker()),
            [provider_id, model] => {
                self.use_provider_and_maybe_save(&[provider_id.clone(), model.clone()], true)
            }
            [_single] => Ok([
                "Model choices are grouped by provider.",
                "Use `/models` to browse, or `/settings provider <provider> <model>` to switch.",
            ]
            .join("\n")),
            _ => Ok("Usage: /models | /models <provider> <model>".to_string()),
        }
    }

    pub(super) fn handle_settings_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None | Some("show") | Some("help") => Ok(self.render_settings()),
            Some("provider") => self.handle_settings_provider(&args[1..]),
            Some("use") => self.use_provider_and_maybe_save(&args[1..], true),
            Some("model") => self.handle_model_command(&args[1..]),
            Some("permissions") | Some("permission") => {
                self.handle_settings_permissions(&args[1..])
            }
            Some("doctor") => Ok(self.render_provider_doctor(args.get(1).map(String::as_str))),
            Some("theme") => Ok(render_tui_theme_settings(args.get(1).map(String::as_str))),
            Some("save") => self.save_current_provider_model_defaults(),
            Some(subcommand) => Ok(format!(
                "Unknown settings subcommand `{subcommand}`.\n\n{}",
                settings_help()
            )),
        }
    }

    fn handle_settings_provider(&mut self, args: &[String]) -> Result<String, String> {
        let Some(provider_id) = args.first().map(String::as_str) else {
            return Ok(self.render_provider_picker("/settings provider"));
        };
        match args.get(1).map(String::as_str) {
            Some("endpoint" | "api-base" | "api_base") => {
                let value = provider_config_value(args, "endpoint")?;
                self.save_provider_config_update(
                    provider_id,
                    ProviderConfigUpdate {
                        api_base: Some(value.clone()),
                        ..ProviderConfigUpdate::default()
                    },
                    format!("endpoint {value}"),
                )
            }
            Some("key-env" | "api-key-env" | "api_key_env") => {
                let value = provider_config_value(args, "key-env")?;
                self.save_provider_config_update(
                    provider_id,
                    ProviderConfigUpdate {
                        api_key_env: Some(value.clone()),
                        ..ProviderConfigUpdate::default()
                    },
                    format!("key env {value}"),
                )
            }
            Some("default-model" | "default_model") => {
                let value = provider_config_value(args, "default-model")?;
                self.save_provider_config_update(
                    provider_id,
                    ProviderConfigUpdate {
                        default_model: Some(value.clone()),
                        ..ProviderConfigUpdate::default()
                    },
                    format!("default model {value}"),
                )
            }
            Some("enable-model" | "enable") => {
                let value = provider_config_value(args, "enable-model")?;
                self.add_provider_model(provider_id, &value)
            }
            Some("favorite-model" | "favorite") => {
                let value = provider_config_value(args, "favorite-model")?;
                self.add_provider_favorite_model(provider_id, &value)
            }
            Some("models" | "enabled-models" | "enabled_models") => {
                let values = args
                    .get(2..)
                    .unwrap_or_default()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    return Ok(format!(
                        "Usage: /settings provider {provider_id} models <model> [model...]\nThis controls which models appear in `/models` for this provider."
                    ));
                }
                self.save_provider_config_update(
                    provider_id,
                    ProviderConfigUpdate {
                        models: Some(values.clone()),
                        ..ProviderConfigUpdate::default()
                    },
                    format!("active models {}", values.join(", ")),
                )
            }
            _ => self.use_provider_and_maybe_save(args, true),
        }
    }

    pub(super) fn handle_setup_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None | Some("show") | Some("help") => Ok(self.render_setup_wizard()),
            Some("provider") if args.len() == 1 => {
                Ok(self.render_provider_picker("/setup provider"))
            }
            Some("model") if args.len() == 1 => Ok(self.render_model_picker("/setup model")),
            _ => self.handle_settings_command(args),
        }
    }

    fn handle_settings_permissions(&mut self, args: &[String]) -> Result<String, String> {
        let Some(mode) = args.first() else {
            return Ok(render_permission_picker(self.mode()));
        };
        let parsed = PermissionMode::parse_cli(mode)
            .ok_or_else(|| format!("Unknown permission level `{mode}`"))?;
        self.set_permission_mode(parsed)?;
        self.runtime_snapshot.permission_mode = parsed;
        let level = viden_types::PermissionLevel::from_legacy_mode(parsed);
        Ok(format!(
            "Permission level set to {}\nCurrent settings: provider {} / model {} / permissions {}.",
            level.cli_name(),
            self.provider.provider_name(),
            self.provider.model(),
            level.cli_name()
        ))
    }

    fn render_provider_list(&self) -> String {
        let Some(host) = self.provider_host.as_ref() else {
            return [
                "Provider registry:",
                "  Runtime registry: unavailable",
                "  Start Viden through the CLI to enable provider plugin commands.",
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
        let config_path = viden_config::default_user_config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|err| format!("<unavailable: {err}>"));
        let mut lines = vec![
            "Settings picker:".to_string(),
            format!(
                "  Current: provider {} / model {} / permissions {}",
                self.provider.provider_name(),
                self.provider.model(),
                self.mode().cli_name()
            ),
            format!("  API key: {}", self.current_provider_key_status()),
            format!("  User config: {config_path}"),
            "".to_string(),
            "Select one setting:".to_string(),
            "  - Provider       /settings provider".to_string(),
            "  - Model          /settings model".to_string(),
            "  - Permissions    /settings permissions".to_string(),
            "  - Theme          /settings theme".to_string(),
            "  - Save defaults  /settings save".to_string(),
            "  - Diagnostics    /settings doctor".to_string(),
            "  - Config details /config".to_string(),
            "".to_string(),
            "TUI usage: type `/settings`, use arrows or mouse, then Enter.".to_string(),
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

    fn render_setup_wizard(&self) -> String {
        let config_path = viden_config::default_user_config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|err| format!("<unavailable: {err}>"));
        let current_provider = self.provider.provider_name();
        let current_model = self.provider.model();
        let mut lines = vec![
            "Provider/model setup:".to_string(),
            format!("  Current: {current_provider} / {current_model}"),
            format!("  API key: {}", self.current_provider_key_status()),
            format!("  User config: {config_path}"),
            "".to_string(),
            "Fast actions:".to_string(),
            "  /setup provider       open provider configuration choices".to_string(),
            "  /models               open model choices grouped by provider".to_string(),
            "  /settings permissions open permission level choices".to_string(),
            "  /settings theme       open TUI theme choices".to_string(),
            "  /settings provider deepseek deepseek-v4-flash".to_string(),
            "  /model deepseek-v4-flash  (current provider only)".to_string(),
            "  Set DEEPSEEK_API_KEY or VIDEN_DEEPSEEK_API_KEY before the first live turn."
                .to_string(),
            "".to_string(),
            "Offline/test path:".to_string(),
            "  /provider fallback test-local".to_string(),
            "".to_string(),
            "How to operate in the TUI:".to_string(),
            "  Type `/setup provider` to see provider key/env/endpoint configuration.".to_string(),
            "  Type `/models` to choose any listed model grouped by provider.".to_string(),
            "  Use `/provider doctor <id>` to check env vars and compatibility.".to_string(),
        ];
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            lines.push("".to_string());
            lines.push("Provider choices:".to_string());
            lines.extend(descriptors.iter().map(|descriptor| {
                format!(
                    "  - {} ({}) -> /setup provider {} | models: /models | key={}",
                    descriptor.provider_id,
                    descriptor.display_name,
                    descriptor.provider_id,
                    descriptor_key_status(descriptor)
                )
            }));
        }
        lines.join("\n")
    }

    fn render_provider_picker(&self, prefix: &str) -> String {
        let mut lines = vec![
            "Provider configuration:".to_string(),
            format!(
                "  Current: {} / {}",
                self.provider.provider_name(),
                self.provider.model()
            ),
            "  Providers show API key env vars, endpoint sources, and model candidates."
                .to_string(),
            "  Use `/models` to choose a model across providers.".to_string(),
            "  Use `/settings provider <id>` to select a provider default.".to_string(),
            "".to_string(),
        ];
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            for descriptor in descriptors {
                lines.push(format!(
                    "  - {:<18} key={:<24} endpoint={} models={}",
                    descriptor.display_name,
                    descriptor_key_status(&descriptor),
                    descriptor
                        .default_api_base
                        .as_deref()
                        .unwrap_or("<configured>"),
                    descriptor_model_candidates(
                        &descriptor,
                        self.provider.provider_name(),
                        self.provider.model()
                    )
                    .join(", ")
                ));
                lines.push(format!(
                    "    inspect: /provider doctor {} | select default: {prefix} {}",
                    descriptor.provider_id, descriptor.provider_id
                ));
            }
        } else {
            lines.push("  Runtime registry unavailable; start Viden through the CLI.".to_string());
        }
        lines.join("\n")
    }

    fn render_model_picker(&self, prefix: &str) -> String {
        let provider_id = self.provider.provider_name();
        let current_model = self.provider.model();
        let mut lines = vec![
            "Choose a model:".to_string(),
            format!("  Current provider: {provider_id}"),
            format!("  Current model: {current_model}"),
            "  Enter one command below. It switches immediately and writes user config."
                .to_string(),
            "".to_string(),
        ];
        let models = if let Some(host) = self.provider_host.as_ref()
            && let Some(descriptor) = host.registry().descriptor(provider_id)
        {
            descriptor_model_candidates(descriptor, provider_id, current_model)
        } else {
            compatible_model_candidates(provider_id, current_model, current_model)
        };
        for model in models {
            let marker = if model == current_model { "*" } else { " " };
            lines.push(format!("  {marker} {model:<30} command: {prefix} {model}"));
        }
        lines.push("".to_string());
        lines.push("Provider picker: /setup provider".to_string());
        lines.join("\n")
    }

    fn render_connect_provider_detail(&self, provider_id: &str) -> String {
        let Some(host) = self.provider_host.as_ref() else {
            return [
                "Connect provider:",
                "  Runtime registry: unavailable",
                "  Start Viden through the CLI to enable provider configuration.",
            ]
            .join("\n");
        };
        let registry = host.registry();
        let Some(descriptor) = registry.descriptor(provider_id) else {
            return format!("Provider `{provider_id}` is not registered.");
        };
        let configured = self.provider_ui_config_for(provider_id);
        let active_models = configured
            .as_ref()
            .map(|config| config.models.as_slice())
            .unwrap_or(&[]);
        let favorite_models = configured
            .as_ref()
            .map(|config| config.favorite_models.as_slice())
            .unwrap_or(&[]);
        let endpoint = configured
            .as_ref()
            .and_then(|config| config.api_base.as_deref())
            .or(descriptor.default_api_base.as_deref())
            .unwrap_or("<provider built-in>");
        let key_env = configured
            .as_ref()
            .and_then(|config| config.api_key_env.as_deref())
            .or(descriptor.env_mappings.api_key_env.as_deref())
            .unwrap_or("<not required>");
        let default_model = configured
            .as_ref()
            .and_then(|config| config.default_model.as_deref())
            .or(descriptor.default_model.as_deref())
            .unwrap_or("<choose one>");
        let active_models_label = if active_models.is_empty() {
            "<none yet>".to_string()
        } else {
            active_models.join(", ")
        };
        let favorite_models_label = if favorite_models.is_empty() {
            "<none yet>".to_string()
        } else {
            favorite_models.join(", ")
        };
        let candidates = descriptor_model_candidates(
            descriptor,
            self.provider.provider_name(),
            self.provider.model(),
        );
        let mut lines = vec![
            format!(
                "Connect provider: {provider_id} / {}",
                descriptor.display_name
            ),
            format!("  auth: {}", descriptor_auth_detail(descriptor)),
            format!("  key env: {key_env} ({})", key_env_status(key_env)),
            format!("  endpoint: {endpoint}"),
            format!("  default model: {default_model}"),
            format!("  active in /models: {active_models_label}"),
            format!("  favorites: {favorite_models_label}"),
            "".to_string(),
            "Configure:".to_string(),
            format!("  /settings provider {provider_id} key-env <ENV_NAME>"),
            format!("  /settings provider {provider_id} endpoint <URL>"),
            format!("  /settings provider {provider_id} default-model <MODEL>"),
            format!("  /settings provider {provider_id} enable-model <MODEL>"),
            format!("  /settings provider {provider_id} favorite-model <MODEL>"),
            format!("  /settings provider {provider_id} models <MODEL> [MODEL...]"),
            "".to_string(),
            "Suggested models:".to_string(),
        ];
        if candidates.is_empty() {
            lines.push(format!(
                "  - Free-type one: /settings provider {provider_id} enable-model <model>"
            ));
        } else {
            lines.extend(candidates.iter().map(|model| {
                let marker = if active_models.iter().any(|active| active == model) {
                    "*"
                } else {
                    " "
                };
                format!(
                    "  {marker} {model:<32} /settings provider {provider_id} enable-model {model}"
                )
            }));
        }
        lines.push("".to_string());
        lines.push(
            "After configuration, open `/models` to pick only the active models for configured providers."
                .to_string(),
        );
        lines.join("\n")
    }

    fn render_models_picker(&self) -> String {
        let current_provider = self.provider.provider_name();
        let current_model = self.provider.model();
        let mut lines = vec![
            "Choose a model:".to_string(),
            format!("  Current: {current_provider} / {current_model}"),
            "  Models are grouped by configured provider. Selecting one switches provider and model."
                .to_string(),
            "  Use `/connect` to add provider keys, endpoints, and active models.".to_string(),
            "".to_string(),
        ];
        let mut rendered = 0usize;
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            for descriptor in descriptors {
                let configured = self.provider_ui_config_for(&descriptor.provider_id);
                let models = configured_model_candidates(
                    configured.as_ref(),
                    &descriptor,
                    current_provider,
                    current_model,
                );
                if models.is_empty() {
                    continue;
                }
                rendered += 1;
                lines.push(format!(
                    "{} ({})",
                    descriptor.display_name, descriptor.provider_id
                ));
                for model in models {
                    let marker =
                        if descriptor.provider_id == current_provider && model == current_model {
                            "*"
                        } else {
                            " "
                        };
                    lines.push(format!(
                        "  {marker} {:<34} /models {} {}",
                        model, descriptor.provider_id, model
                    ));
                }
                lines.push("".to_string());
            }
        } else {
            lines.push("Runtime registry unavailable; start Viden through the CLI.".to_string());
        }
        if rendered == 0 {
            lines.push("No configured provider models yet.".to_string());
            lines.push("Open `/connect`, choose a provider, enter the key if needed, then choose the default model.".to_string());
        }
        lines.join("\n")
    }

    fn render_provider_doctor(&self, provider_id: Option<&str>) -> String {
        let Some(host) = self.provider_host.as_ref() else {
            return [
                "Provider diagnostics:",
                "  Runtime registry: unavailable",
                "  Start Viden through the CLI to enable provider diagnostics.",
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
                render_provider_doctor_detail(descriptor),
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
                "Start Viden through the CLI to enable provider plugin commands.",
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

    fn use_provider_and_maybe_save(
        &mut self,
        args: &[String],
        save: bool,
    ) -> Result<String, String> {
        let Some(provider_id) = args.first().map(String::as_str) else {
            return Ok(self.render_provider_picker("/provider"));
        };
        let requested_model = args.get(1).map(String::as_str);
        let Some(host) = self.provider_host.as_ref() else {
            return Ok([
                "Provider switching unavailable.",
                "Start Viden through the CLI to enable provider runtime commands.",
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
        let output = format!(
            "Provider set to {} ({})",
            self.provider.provider_name(),
            self.provider.model()
        );
        if save {
            let saved = self.save_current_provider_model_defaults()?;
            Ok(format!(
                "{output}\n{saved}\nNext live turn uses {} / {}.",
                self.provider.provider_name(),
                self.provider.model()
            ))
        } else {
            Ok(format!(
                "{output}\nSession-only switch. Save as default with /settings save."
            ))
        }
    }

    fn save_current_provider_model_defaults(&self) -> Result<String, String> {
        let path = if let Some(path) = &self.user_config_path_override {
            viden_config::save_user_provider_model_defaults_at(
                path,
                self.provider.provider_name(),
                self.provider.model(),
            )?;
            path.clone()
        } else {
            viden_config::save_user_provider_model_defaults(
                self.provider.provider_name(),
                self.provider.model(),
            )?
        };
        Ok(format!(
            "Saved default provider/model to {}",
            path.display()
        ))
    }

    pub(crate) fn save_provider_config_update(
        &self,
        provider_id: &str,
        update: ProviderConfigUpdate,
        summary: String,
    ) -> Result<String, String> {
        let path = if let Some(path) = &self.user_config_path_override {
            viden_config::save_user_provider_config_at(path, provider_id, update)?;
            path.clone()
        } else {
            viden_config::save_user_provider_config(provider_id, update)?
        };
        Ok(format!(
            "Saved provider config: {provider_id} {summary}\nUser config: {}\nUse `/settings provider {provider_id}` to switch to this provider, `/models` to choose a model, or `/provider doctor {provider_id}` to verify env and endpoint readiness.",
            path.display()
        ))
    }

    pub(crate) fn add_provider_model(
        &self,
        provider_id: &str,
        model: &str,
    ) -> Result<String, String> {
        let path = if let Some(path) = &self.user_config_path_override {
            viden_config::add_user_provider_model_at(path, provider_id, model)?;
            path.clone()
        } else {
            viden_config::add_user_provider_model(provider_id, model)?
        };
        Ok(format!(
            "Enabled model for /models: {provider_id} / {model}\nUser config: {}\nNext: set default with `/settings provider {provider_id} default-model {model}` or choose it from `/models`.",
            path.display()
        ))
    }

    pub(crate) fn remove_provider_model(
        &self,
        provider_id: &str,
        model: &str,
    ) -> Result<String, String> {
        let mut config = self.provider_ui_config_for(provider_id).unwrap_or_default();
        config.models.retain(|item| item != model);
        config.favorite_models.retain(|item| item != model);
        self.save_provider_config_update(
            provider_id,
            ProviderConfigUpdate {
                models: Some(config.models),
                favorite_models: Some(config.favorite_models),
                ..ProviderConfigUpdate::default()
            },
            format!("removed model {model}"),
        )
    }

    fn add_provider_favorite_model(
        &self,
        provider_id: &str,
        model: &str,
    ) -> Result<String, String> {
        let path = if let Some(path) = &self.user_config_path_override {
            viden_config::add_user_provider_favorite_model_at(path, provider_id, model)?;
            path.clone()
        } else {
            viden_config::add_user_provider_favorite_model(provider_id, model)?
        };
        Ok(format!(
            "Favorited model for /models: {provider_id} / {model}\nUser config: {}\nThis model now appears first in the Favorites section without duplicating in provider groups.",
            path.display()
        ))
    }

    fn provider_ui_config_for(&self, provider_id: &str) -> Option<viden_config::ProviderUiConfig> {
        if let Some(path) = &self.user_config_path_override {
            return viden_config::load_provider_ui_config_at(path)
                .ok()
                .and_then(|mut configs| configs.remove(provider_id));
        }
        viden_config::load_provider_ui_configs(&self.cwd)
            .ok()
            .and_then(|mut configs| configs.remove(provider_id))
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
    ) -> Result<Box<dyn viden_provider::ModelProvider>, String> {
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

    pub(super) fn provider_model_recovery_prompt(&self, error: &str) -> Option<String> {
        let failure = classify_provider_model_failure(error)?;
        let provider_id = self.provider.provider_name();
        let current_model = self.provider.model();
        let descriptor = self.provider_host.as_ref().and_then(|host| {
            let registry = host.registry();
            registry.descriptor(provider_id).cloned()
        });
        let default_model = descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.default_model.clone())
            .unwrap_or_else(|| current_model.to_string());
        let candidates = descriptor
            .as_ref()
            .map(|descriptor| descriptor_model_candidates(descriptor, provider_id, current_model))
            .filter(|models| !models.is_empty())
            .unwrap_or_else(|| {
                compatible_model_candidates(provider_id, &default_model, current_model)
            });
        let candidate_text = candidates.join(", ");
        let primary_candidate = candidates
            .first()
            .cloned()
            .unwrap_or_else(|| default_model.clone());
        Some(
            [
                "Provider/model recovery:".to_string(),
                format!("  class: {}", failure.class),
                format!("  current: {provider_id} / {current_model}"),
                format!("  reason: {}", failure.reason),
                format!("  candidates: {candidate_text}"),
                format!("  next: {}", failure.next_action),
                format!("  try: /models {provider_id} {primary_candidate}"),
                "  picker: /models".to_string(),
                format!("  reconnect: /connect {provider_id}"),
                format!("  diagnose: /provider doctor {provider_id}"),
                format!("  live smoke: scripts/provider-live-smoke.sh --provider {provider_id} --model {primary_candidate}"),
                "  offline fallback: /settings provider fallback test-local".to_string(),
            ]
            .join("\n"),
        )
    }
}

fn args_for_provider(provider_id: &str, rest: &[String]) -> Vec<String> {
    let mut args = Vec::with_capacity(rest.len() + 1);
    args.push(provider_id.to_string());
    args.extend(rest.iter().cloned());
    args
}

fn render_provider_descriptor(descriptor: &ProviderDescriptor) -> String {
    format!(
        "  - {} ({}) family={:?} auth={} default_model={} streaming={} tools={} compat={}",
        descriptor.provider_id,
        descriptor.display_name,
        descriptor.protocol_family,
        descriptor_auth_detail(descriptor),
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

fn render_provider_doctor_detail(descriptor: &ProviderDescriptor) -> String {
    let default_model = descriptor
        .default_model
        .as_deref()
        .unwrap_or("<choose one>");
    let known_models = if descriptor.known_models.is_empty() {
        "<none declared>".to_string()
    } else {
        descriptor.known_models.join(", ")
    };
    let key_env = descriptor
        .env_mappings
        .api_key_env
        .as_deref()
        .unwrap_or("<not required>");
    let key_hint = if key_env == "<not required>" {
        "key: not required".to_string()
    } else if std::env::var_os(key_env).is_some() {
        format!("key: {key_env} present")
    } else {
        format!("key: {key_env} missing")
    };
    let endpoint = descriptor
        .default_api_base
        .as_deref()
        .unwrap_or("<provider built-in>");
    [
        "  readiness:".to_string(),
        format!("    {key_hint}"),
        format!("    endpoint: {endpoint}"),
        format!("    default model: {default_model}"),
        format!("    known models: {known_models}"),
        format!("    configure: /connect {}", descriptor.provider_id),
        format!(
            "    choose model: /models {} {default_model}",
            descriptor.provider_id
        ),
        format!(
            "    live smoke: scripts/provider-live-smoke.sh --provider {} --model {default_model}",
            descriptor.provider_id
        ),
    ]
    .join("\n")
}

fn render_permission_picker(current: PermissionMode) -> String {
    let current_level = viden_types::PermissionLevel::from_legacy_mode(current);
    [
        "Choose permission level:".to_string(),
        format!("  Current: {}", current_level.cli_name()),
        "  Enter one command below. Work mode is changed with /mode.".to_string(),
        "".to_string(),
        "  - Ask before mutations       command: /settings permissions ask".to_string(),
        "  - Auto-accept file edits     command: /settings permissions auto_edit".to_string(),
        "  - Read-only permissions      command: /settings permissions read_only".to_string(),
        "  - Full trusted workspace     command: /settings permissions full_access".to_string(),
    ]
    .join("\n")
}

fn render_tui_theme_settings(theme: Option<&str>) -> String {
    match theme {
        Some(theme) => format!(
            "TUI theme `{theme}` is handled by the live TUI. Use `/settings theme {theme}` inside the cockpit or start with `--tui-theme {theme}`."
        ),
        None => [
            "Choose TUI theme:".to_string(),
            "  TUI-only setting; it applies immediately inside the cockpit.".to_string(),
            "".to_string(),
            "  - aurora-cyan      command: /settings theme aurora-cyan".to_string(),
            "  - ember-gold       command: /settings theme ember-gold".to_string(),
            "  - plasma-violet    command: /settings theme plasma-violet".to_string(),
            "  - monochrome-ice   command: /settings theme monochrome-ice".to_string(),
        ]
        .join("\n"),
    }
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

fn key_env_status(env_name: &str) -> String {
    if env_name == "<not required>" {
        return "not required".to_string();
    }
    if std::env::var_os(env_name).is_some() {
        "present".to_string()
    } else {
        "missing".to_string()
    }
}

fn descriptor_auth_detail(descriptor: &ProviderDescriptor) -> String {
    if descriptor.auth_modes.is_empty() {
        if descriptor.env_mappings.api_key_env.is_some() {
            return "API key".to_string();
        }
        return "local / no key".to_string();
    }
    descriptor
        .auth_modes
        .iter()
        .map(|mode| match mode {
            ProviderAuthMode::ApiKey => "API key",
            ProviderAuthMode::WebLogin => "web login",
            ProviderAuthMode::Local => "local / no key",
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn descriptor_key_status(descriptor: &ProviderDescriptor) -> String {
    match descriptor.env_mappings.api_key_env.as_deref() {
        Some(name) if std::env::var_os(name).is_some() => format!("{name}:present"),
        Some(name) => format!("{name}:missing"),
        None if descriptor.provider_id == "fallback" => "not required".to_string(),
        None => "unknown".to_string(),
    }
}

struct ProviderFailureClass {
    class: &'static str,
    reason: &'static str,
    next_action: &'static str,
}

fn classify_provider_model_failure(error: &str) -> Option<ProviderFailureClass> {
    let lower = error.to_ascii_lowercase();
    if lower.contains("cancelled") || lower.contains("canceled") {
        return None;
    }
    let class = if lower.contains("api key")
        || lower.contains("missing key")
        || lower.contains("key=missing")
        || lower.contains("no api key")
    {
        ProviderFailureClass {
            class: "missing_key",
            reason: "API key is missing or not visible to this process.",
            next_action: "open provider config, export the listed key env var, then retry",
        }
    } else if lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("permission")
        || lower.contains("api error (401)")
        || lower.contains("api error (403)")
    {
        ProviderFailureClass {
            class: "auth",
            reason: "Provider rejected the credentials or account permissions.",
            next_action: "run provider doctor, verify key scope, or switch to fallback",
        }
    } else if lower.contains("rate limit") || lower.contains("too many requests") {
        ProviderFailureClass {
            class: "rate_limit",
            reason: "Provider is rate limiting the current key or model.",
            next_action: "retry later, switch model/provider, or use fallback",
        }
    } else if lower.contains("api error (413)")
        || lower.contains("http 413")
        || lower.contains("payload too large")
        || lower.contains("request entity too large")
    {
        ProviderFailureClass {
            class: "request_too_large",
            reason: "Provider rejected the serialized request body before model execution.",
            next_action: "compact provider context, retry with a smaller prompt, or switch provider",
        }
    } else if lower.contains("timeout") || lower.contains("timed out") {
        ProviderFailureClass {
            class: "timeout",
            reason: "Provider request timed out before a usable response.",
            next_action: "retry, increase request timeout, or switch provider",
        }
    } else if lower.contains("context_length")
        || lower.contains("maximum context")
        || lower.contains("context overflow")
    {
        ProviderFailureClass {
            class: "context_overflow",
            reason: "The request is larger than the provider/model context limit.",
            next_action: "compact context or switch to a larger-context model",
        }
    } else if lower.contains("tool_calls")
        || lower.contains("tool call")
        || lower.contains("tool_choice")
        || lower.contains("unsupported")
    {
        ProviderFailureClass {
            class: "compatibility",
            reason: "The model/provider response is incompatible with Viden tool calls.",
            next_action: "switch to a known-compatible model or inspect provider doctor",
        }
    } else if lower.contains("model")
        || lower.contains("not found")
        || lower.contains("does not exist")
        || lower.contains("unavailable")
        || lower.contains("api error (400)")
        || lower.contains("api error (404)")
    {
        ProviderFailureClass {
            class: "model_unavailable",
            reason: "The selected model may be missing, retired, or unavailable for this key.",
            next_action: "open /models and switch to a known model candidate",
        }
    } else {
        return None;
    };
    Some(class)
}

pub(crate) fn is_request_too_large_provider_failure(error: &str) -> bool {
    classify_provider_model_failure(error)
        .is_some_and(|failure| failure.class == "request_too_large")
}

fn compatible_model_candidates(
    provider_id: &str,
    default_model: &str,
    current_model: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    push_unique(&mut candidates, default_model.to_string());
    match provider_id {
        "deepseek" | "deepseek-anthropic" => {
            push_unique(&mut candidates, "deepseek-v4-flash".to_string());
            push_unique(&mut candidates, "deepseek-v4-pro".to_string());
        }
        "fallback" => {
            push_unique(&mut candidates, "test-local".to_string());
        }
        _ => {}
    }
    if current_model != default_model {
        push_unique(&mut candidates, current_model.to_string());
    }
    candidates
}

fn descriptor_model_candidates(
    descriptor: &ProviderDescriptor,
    current_provider: &str,
    current_model: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(default_model) = descriptor.default_model.as_deref() {
        push_unique(&mut candidates, default_model.to_string());
    }
    for model in &descriptor.known_models {
        push_unique(&mut candidates, model.to_string());
    }
    if descriptor.provider_id == current_provider {
        push_unique(&mut candidates, current_model.to_string());
    }
    candidates
}

fn configured_model_candidates(
    config: Option<&viden_config::ProviderUiConfig>,
    descriptor: &ProviderDescriptor,
    current_provider: &str,
    current_model: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let configured = config.is_some()
        || descriptor.provider_id == current_provider
        || descriptor
            .env_mappings
            .api_key_env
            .as_deref()
            .is_some_and(|env| {
                std::env::var(env)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            })
        || descriptor.env_mappings.api_key_env.is_none();
    if let Some(config) = config {
        for model in &config.favorite_models {
            push_unique(&mut candidates, model.to_string());
        }
        for model in &config.models {
            push_unique(&mut candidates, model.to_string());
        }
        if let Some(default_model) = config.default_model.as_deref() {
            push_unique(&mut candidates, default_model.to_string());
        }
    }
    if descriptor.provider_id == current_provider {
        push_unique(&mut candidates, current_model.to_string());
    }
    if configured {
        if let Some(default_model) = descriptor.default_model.as_deref() {
            push_unique(&mut candidates, default_model.to_string());
        }
        for model in &descriptor.known_models {
            push_unique(&mut candidates, model.to_string());
        }
    }
    candidates
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
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
        "  /connect           Open the provider connection picker",
        "  /provider          Show provider configuration, keys, endpoints, and models",
        "  /provider <id> [model]",
        "                     Switch provider/model and save defaults",
        "  /provider list     List registered providers",
        "  /provider doctor [id]",
        "                     Show provider registry diagnostics",
        "  /provider reload   Reload provider plugin registry",
        "  /provider use <id> [model]",
        "                     Legacy session-only switch; use /provider <id> to save",
        "  /models            Choose model across provider groups",
        "  /settings provider <id> endpoint <url>",
        "                     Save provider endpoint",
        "  /settings provider <id> key-env <ENV_NAME>",
        "                     Save provider key env source",
        "  /settings provider <id> default-model <model>",
        "                     Save provider-scoped default model",
        "  /settings provider <id> enable-model <model>",
        "                     Add a model to the /models picker for this provider",
        "  /settings provider <id> favorite-model <model>",
        "                     Pin a provider/model pair to the top of /models",
        "  /settings provider <id> models <model> [model...]",
        "                     Replace the provider's active /models list",
    ]
    .join("\n")
}

fn settings_help() -> String {
    [
        "Settings commands:",
        "  /settings                  Show provider/model setup status",
        "  /connect                   Open the provider connection picker",
        "  /provider <id> [model]     Switch provider/model and save defaults",
        "  /model <model>             Switch model and save defaults",
        "  /models                    Show model choices grouped by provider",
        "  /settings provider <id> [model]",
        "                             Switch provider/model and save defaults",
        "  /settings provider <id> endpoint <url>",
        "                             Save provider API endpoint",
        "  /settings provider <id> key-env <ENV_NAME>",
        "                             Save provider API key environment variable",
        "  /settings provider <id> default-model <model>",
        "                             Save provider-scoped default model",
        "  /settings model <model>    Switch current model and save defaults",
        "  /settings save             Save current provider/model as defaults",
        "  /setup                     Interactive provider/model setup guide",
        "  /setup provider <id> [model]",
        "                             Switch provider/model and save defaults",
    ]
    .join("\n")
}

fn provider_config_value(args: &[String], field: &str) -> Result<String, String> {
    let value = args
        .get(2)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Usage: /settings provider <id> {field} <value>"))?;
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::classify_provider_model_failure;

    #[test]
    fn classifies_common_provider_recovery_failures() {
        let cases = [
            ("missing API key for DeepSeek", "missing_key"),
            ("API error (401): unauthorized", "auth"),
            ("rate limit exceeded", "rate_limit"),
            (
                "API error (413): deepseek returned HTTP 413",
                "request_too_large",
            ),
            ("request timed out after 90s", "timeout"),
            ("maximum context length exceeded", "context_overflow"),
            (
                "assistant message with tool_calls is unsupported",
                "compatibility",
            ),
            ("model does not exist", "model_unavailable"),
        ];

        for (message, expected) in cases {
            let class = classify_provider_model_failure(message).expect(message);
            assert_eq!(class.class, expected, "{message}");
            assert!(!class.next_action.is_empty(), "{message}");
        }
    }

    #[test]
    fn cancelled_provider_request_does_not_show_recovery_prompt() {
        assert!(classify_provider_model_failure("model request cancelled").is_none());
    }
}
