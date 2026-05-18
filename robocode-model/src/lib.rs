mod adapters;
mod config;
mod descriptor;
mod fallback;
mod host;
mod http;
mod parse;
mod plugin;
mod providers;
mod registry;
mod render;
mod transport;

use robocode_types::{ModelEvent, ModelRequest};

pub use config::{ProviderConfig, ProviderKind};
pub use descriptor::{
    ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings,
};
pub use host::ProviderHost;
pub use plugin::{ProviderPluginError, ProviderPluginErrorKind};
pub use providers::AnthropicProvider;
pub use registry::ProviderRegistry;

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";

pub trait ModelProvider: Send {
    fn provider_name(&self) -> &str;
    fn model(&self) -> &str;
    fn set_model(&mut self, model: String);
    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String>;
}

pub fn create_provider(config: ProviderConfig) -> Box<dyn ModelProvider> {
    ProviderHost::with_builtins()
        .create(config)
        .expect("builtin provider construction should succeed")
}

pub fn list_supported_provider_strings() -> Vec<String> {
    ProviderRegistry::with_builtins().creatable_provider_ids()
}

#[cfg(test)]
mod tests;
