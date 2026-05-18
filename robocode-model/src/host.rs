use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};

use crate::ProviderPluginError;
use crate::providers::{load_builtin_provider, load_registered_provider};
use crate::{ModelProvider, ProviderConfig, ProviderRegistry};

static BUILTIN_PROVIDER_REGISTRY: LazyLock<Arc<RwLock<Arc<ProviderRegistry>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(Arc::new(ProviderRegistry::with_builtins()))));

#[derive(Debug)]
pub struct ProviderHost {
    registry: Arc<RwLock<Arc<ProviderRegistry>>>,
}

impl ProviderHost {
    pub fn load_default() -> Result<Self, String> {
        Self::load_default_diagnostic().map_err(|err| err.to_string())
    }

    pub fn load_default_diagnostic() -> Result<Self, ProviderPluginError> {
        let registry = Arc::new(ProviderRegistry::load_default_diagnostic()?);
        {
            let mut global = BUILTIN_PROVIDER_REGISTRY.write().map_err(|_| {
                ProviderPluginError::registry("builtin registry write lock poisoned")
            })?;
            *global = Arc::clone(&registry);
        }
        Ok(Self {
            registry: Arc::clone(&BUILTIN_PROVIDER_REGISTRY),
        })
    }

    pub fn load_from_dirs(plugin_dirs: Vec<PathBuf>) -> Result<Self, String> {
        Self::load_from_dirs_diagnostic(plugin_dirs).map_err(|err| err.to_string())
    }

    pub fn load_from_dirs_diagnostic(
        plugin_dirs: Vec<PathBuf>,
    ) -> Result<Self, ProviderPluginError> {
        Ok(Self {
            registry: Arc::new(RwLock::new(Arc::new(
                ProviderRegistry::load_from_dirs_diagnostic(plugin_dirs)?,
            ))),
        })
    }

    pub fn with_builtins() -> Self {
        Self {
            registry: Arc::clone(&BUILTIN_PROVIDER_REGISTRY),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_registry(registry: ProviderRegistry) -> Self {
        Self {
            registry: Arc::new(RwLock::new(Arc::new(registry))),
        }
    }

    pub fn registry(&self) -> Arc<ProviderRegistry> {
        self.registry
            .read()
            .expect("builtin registry read lock should not be poisoned")
            .clone()
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        let rebuilt = Arc::new(ProviderRegistry::load_default()?);
        let mut global = self
            .registry
            .write()
            .map_err(|_| "builtin registry write lock poisoned".to_string())?;
        *global = rebuilt;
        Ok(())
    }

    pub fn refresh_from_dirs(&mut self, plugin_dirs: Vec<PathBuf>) -> Result<(), String> {
        let rebuilt = Arc::new(ProviderRegistry::load_from_dirs(plugin_dirs)?);
        let mut registry = self
            .registry
            .write()
            .map_err(|_| "provider registry write lock poisoned".to_string())?;
        *registry = rebuilt;
        Ok(())
    }

    pub fn create(&self, config: ProviderConfig) -> Result<Box<dyn ModelProvider>, String> {
        let registry = self.registry();
        load_builtin_provider(registry.as_ref(), config)
    }

    pub fn create_registered(
        &self,
        provider_id: &str,
        model: Option<&str>,
        api_base: Option<&str>,
        api_key: Option<&str>,
        request_timeout_secs: u64,
        max_retries: u32,
    ) -> Result<Box<dyn ModelProvider>, String> {
        let registry = self.registry();
        load_registered_provider(
            registry.as_ref(),
            provider_id,
            model,
            api_base,
            api_key,
            request_timeout_secs,
            max_retries,
        )
    }
}
