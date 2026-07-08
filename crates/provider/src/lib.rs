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

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use viden_types::{ModelEvent, ModelRequest};

pub use config::{ProviderConfig, ProviderKind};
pub use descriptor::{
    ProtocolFamily, ProviderAuthMode, ProviderCapabilities, ProviderCompatibility,
    ProviderDescriptor, ProviderEnvMappings,
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

    fn next_events_with_control(
        &mut self,
        request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        control.check_cancelled()?;
        let events = self.next_events(request)?;
        control.check_cancelled()?;
        Ok(events)
    }
}

#[derive(Clone)]
pub struct ModelRequestControl {
    cancelled: Arc<AtomicBool>,
    prefer_streaming: bool,
    stream_sink: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl ModelRequestControl {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            prefer_streaming: false,
            stream_sink: None,
        }
    }

    pub fn with_streaming_preference(prefer_streaming: bool) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            prefer_streaming,
            stream_sink: None,
        }
    }

    pub fn with_streaming_sink<F>(prefer_streaming: bool, stream_sink: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            prefer_streaming,
            stream_sink: Some(Arc::new(stream_sink)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn prefer_streaming(&self) -> bool {
        self.prefer_streaming
    }

    pub fn emit_stream_delta(&self, delta: String) {
        if let Some(sink) = &self.stream_sink
            && !delta.is_empty()
        {
            sink(delta);
        }
    }

    pub fn has_stream_sink(&self) -> bool {
        self.stream_sink.is_some()
    }

    pub fn check_cancelled(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err("Model request cancelled".to_string())
        } else {
            Ok(())
        }
    }
}

impl Default for ModelRequestControl {
    fn default() -> Self {
        Self::new()
    }
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
