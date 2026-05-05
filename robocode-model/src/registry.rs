use crate::{adapters::builtin_provider_descriptors, descriptor::ProviderDescriptor};

#[derive(Debug, Default, Clone)]
pub struct ProviderRegistry {
    descriptors: Vec<ProviderDescriptor>,
}

impl ProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            descriptors: builtin_provider_descriptors(),
        }
    }

    pub fn descriptors(&self) -> &[ProviderDescriptor] {
        &self.descriptors
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.descriptors
            .iter()
            .map(|item| item.provider_id.clone())
            .collect()
    }
}
