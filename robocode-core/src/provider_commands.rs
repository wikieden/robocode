use crate::SessionEngine;
use robocode_model::{ProviderConfig, ProviderDescriptor, ProviderPluginError};
use robocode_types::PermissionMode;

impl SessionEngine {
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

    pub(super) fn handle_settings_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None | Some("show") | Some("help") => Ok(self.render_settings()),
            Some("provider") | Some("use") => self.use_provider_and_maybe_save(&args[1..], true),
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
            .ok_or_else(|| format!("Unknown permission mode `{mode}`"))?;
        self.set_permission_mode(parsed)?;
        self.runtime_snapshot.permission_mode = parsed;
        Ok(format!(
            "Permission mode set to {}\nCurrent settings: provider {} / model {} / permissions {}.",
            parsed.cli_name(),
            self.provider.provider_name(),
            self.provider.model(),
            parsed.cli_name()
        ))
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
        let config_path = robocode_config::default_user_config_path()
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
            "  /setup provider       open provider choices".to_string(),
            "  /models               open model choices for current provider".to_string(),
            "  /settings permissions open approval mode choices".to_string(),
            "  /settings theme       open TUI theme choices".to_string(),
            "  /provider deepseek deepseek-v4-flash".to_string(),
            "  /model deepseek-v4-flash".to_string(),
            "  Set DEEPSEEK_API_KEY or ROBOCODE_DEEPSEEK_API_KEY before the first live turn."
                .to_string(),
            "".to_string(),
            "Offline/test path:".to_string(),
            "  /provider fallback test-local".to_string(),
            "".to_string(),
            "How to operate in the TUI:".to_string(),
            "  Type `/setup provider` to see provider choices.".to_string(),
            "  Type `/models` or `/model` to see model choices.".to_string(),
            "  Use `/provider doctor <id>` to check env vars and compatibility.".to_string(),
        ];
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            lines.push("".to_string());
            lines.push("Provider choices:".to_string());
            lines.extend(descriptors.iter().map(|descriptor| {
                let model = descriptor.default_model.as_deref().unwrap_or("<model>");
                format!(
                    "  - {} ({}) -> /setup provider {} {} | key={}",
                    descriptor.provider_id,
                    descriptor.display_name,
                    descriptor.provider_id,
                    model,
                    descriptor_key_status(descriptor)
                )
            }));
        }
        lines.join("\n")
    }

    fn render_provider_picker(&self, prefix: &str) -> String {
        let mut lines = vec![
            "Choose a provider:".to_string(),
            format!(
                "  Current: {} / {}",
                self.provider.provider_name(),
                self.provider.model()
            ),
            "  Enter one command below. It switches immediately and writes user config."
                .to_string(),
            "  Tip: in TUI, type provider letters after the space and use Tab/Enter.".to_string(),
            "".to_string(),
        ];
        if let Some(host) = self.provider_host.as_ref() {
            let mut descriptors = host.registry().descriptors().to_vec();
            descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
            for descriptor in descriptors {
                let model = descriptor.default_model.as_deref().unwrap_or("<model>");
                lines.push(format!(
                    "  - {:<18} {:<18} key={:<24} command: {prefix} {} {}",
                    descriptor.display_name,
                    descriptor.provider_id,
                    descriptor_key_status(&descriptor),
                    descriptor.provider_id,
                    model
                ));
            }
        } else {
            lines.push(
                "  Runtime registry unavailable; start RoboCode through the CLI.".to_string(),
            );
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
        let mut models = compatible_model_candidates(provider_id, current_model, current_model);
        if let Some(host) = self.provider_host.as_ref()
            && let Some(descriptor) = host.registry().descriptor(provider_id)
            && let Some(default_model) = descriptor.default_model.as_deref()
        {
            push_unique(&mut models, default_model.to_string());
        }
        for model in models {
            let marker = if model == current_model { "*" } else { " " };
            lines.push(format!("  {marker} {model:<30} command: {prefix} {model}"));
        }
        lines.push("".to_string());
        lines.push("Provider picker: /setup provider".to_string());
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
            robocode_config::save_user_provider_model_defaults_at(
                path,
                self.provider.provider_name(),
                self.provider.model(),
            )?;
            path.clone()
        } else {
            robocode_config::save_user_provider_model_defaults(
                self.provider.provider_name(),
                self.provider.model(),
            )?
        };
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

    pub(super) fn provider_model_recovery_prompt(&self, error: &str) -> Option<String> {
        if !looks_like_provider_model_failure(error) {
            return None;
        }
        let provider_id = self.provider.provider_name();
        let current_model = self.provider.model();
        let default_model = self
            .provider_host
            .as_ref()
            .and_then(|host| {
                let registry = host.registry();
                registry
                    .descriptor(provider_id)
                    .and_then(|descriptor| descriptor.default_model.clone())
            })
            .unwrap_or_else(|| current_model.to_string());
        let candidates = compatible_model_candidates(provider_id, &default_model, current_model);
        let candidate_text = candidates.join(", ");
        Some(
            [
                "Provider/model recovery:".to_string(),
                format!("  current: {provider_id} / {current_model}"),
                "  The current model may be unavailable, unauthorized, or incompatible."
                    .to_string(),
                format!("  candidates: {candidate_text}"),
                format!("  try: /settings model {default_model}"),
                format!("  try: /settings provider {provider_id} {default_model}"),
                format!("  diagnose: /provider doctor {provider_id}"),
                "  offline fallback: /settings provider fallback test-local".to_string(),
            ]
            .join("\n"),
        )
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

fn render_permission_picker(current: PermissionMode) -> String {
    [
        "Choose permission mode:".to_string(),
        format!("  Current: {}", current.cli_name()),
        "  Enter one command below. It switches immediately.".to_string(),
        "".to_string(),
        "  - Suggest before mutations   command: /settings permissions default".to_string(),
        "  - Auto-accept file edits     command: /settings permissions acceptEdits".to_string(),
        "  - Plan/read-only             command: /settings permissions plan".to_string(),
        "  - YOLO trusted workspace     command: /settings permissions bypassPermissions"
            .to_string(),
        "  - Deny instead of asking     command: /settings permissions dontAsk".to_string(),
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

fn descriptor_key_status(descriptor: &ProviderDescriptor) -> String {
    match descriptor.env_mappings.api_key_env.as_deref() {
        Some(name) if std::env::var_os(name).is_some() => format!("{name}:present"),
        Some(name) => format!("{name}:missing"),
        None if descriptor.provider_id == "fallback" => "not required".to_string(),
        None => "unknown".to_string(),
    }
}

fn looks_like_provider_model_failure(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    if lower.contains("cancelled") || lower.contains("canceled") {
        return false;
    }
    [
        "model",
        "not found",
        "does not exist",
        "unavailable",
        "unsupported",
        "permission",
        "unauthorized",
        "forbidden",
        "context_length",
        "maximum context",
        "api error (400)",
        "api error (401)",
        "api error (403)",
        "api error (404)",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
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
        "  /provider          Choose a provider and show switch commands",
        "  /provider <id> [model]",
        "                     Switch provider/model and save defaults",
        "  /provider list     List registered providers",
        "  /provider doctor [id]",
        "                     Show provider registry diagnostics",
        "  /provider reload   Reload provider plugin registry",
        "  /provider use <id> [model]",
        "                     Legacy session-only switch; use /provider <id> to save",
    ]
    .join("\n")
}

fn settings_help() -> String {
    [
        "Settings commands:",
        "  /settings                  Show provider/model setup status",
        "  /provider <id> [model]     Switch provider/model and save defaults",
        "  /model <model>             Switch model and save defaults",
        "  /models                    Show model choices for the current provider",
        "  /settings provider <id> [model]",
        "                             Switch provider/model and save defaults",
        "  /settings model <model>    Switch current model and save defaults",
        "  /settings save             Save current provider/model as defaults",
        "  /setup                     Interactive provider/model setup guide",
        "  /setup provider <id> [model]",
        "                             Switch provider/model and save defaults",
    ]
    .join("\n")
}
