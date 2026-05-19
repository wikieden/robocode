use crate::SessionEngine;
use robocode_model::{ProviderDescriptor, ProviderPluginError};

impl SessionEngine {
    pub(super) fn handle_provider_command(&mut self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str) {
            None => Ok(format!(
                "Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            )),
            Some("list") => Ok(self.render_provider_list()),
            Some("reload") => self.reload_provider_registry(),
            Some("help") => Ok(provider_help()),
            Some(subcommand) => Ok(format!(
                "Unknown provider subcommand `{subcommand}`.\n\n{}",
                provider_help()
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
}

fn render_provider_descriptor(descriptor: &ProviderDescriptor) -> String {
    format!(
        "  - {} ({}) family={:?} default_model={}",
        descriptor.provider_id,
        descriptor.display_name,
        descriptor.protocol_family,
        descriptor.default_model.as_deref().unwrap_or("<none>")
    )
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
        "  /provider reload   Reload provider plugin registry",
    ]
    .join("\n")
}
