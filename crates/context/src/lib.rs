pub mod store;

use std::path::Path;

pub use store::{ContextError, ContextPutRequest, ContextStore, StoredContext};

use viden_types::{ContextHandleRecord, ContextScope};

pub struct ContextEngine {
    store: ContextStore,
}

impl ContextEngine {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ContextError> {
        Ok(Self {
            store: ContextStore::open(root)?,
        })
    }

    pub fn store(&mut self, request: ContextPutRequest<'_>) -> Result<StoredContext, ContextError> {
        self.store.put(request)
    }

    pub fn retrieve(
        &self,
        handle: &ContextHandleRecord,
        scope: &ContextScope,
    ) -> Result<Vec<u8>, ContextError> {
        self.store.retrieve(handle, scope)
    }
}
