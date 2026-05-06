use std::sync::{Arc, LazyLock, RwLock};

use crate::{ModelProvider, ProviderConfig, ProviderRegistry, load_builtin_provider};

static BUILTIN_PROVIDER_REGISTRY: LazyLock<RwLock<Arc<ProviderRegistry>>> =
    LazyLock::new(|| {
        RwLock::new(Arc::new(ProviderRegistry::with_builtins()))
    });

pub struct ProviderHost {
    registry: &'static RwLock<Arc<ProviderRegistry>>,
}

impl ProviderHost {
    pub fn load_default() -> Result<Self, String> {
        let registry = Arc::new(ProviderRegistry::load_default()?);
        {
            let mut global = BUILTIN_PROVIDER_REGISTRY
                .write()
                .map_err(|_| "builtin registry write lock poisoned".to_string())?;
            *global = Arc::clone(&registry);
        }
        Ok(Self {
            registry: &BUILTIN_PROVIDER_REGISTRY,
        })
    }

    pub fn with_builtins() -> Self {
        Self {
            registry: &BUILTIN_PROVIDER_REGISTRY,
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

    pub fn create(&self, config: ProviderConfig) -> Result<Box<dyn ModelProvider>, String> {
        let registry = self.registry();
        load_builtin_provider(registry.as_ref(), config)
    }
}
