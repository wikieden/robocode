pub mod reducer;
pub mod store;

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

pub use reducer::{
    LineRange, ReductionEstimate, ReductionOmission, ReductionPolicy, ReductionResult, reduce,
};
pub use store::{ContextPutRequest, ContextStore, StoredContext};

use viden_types::{ContextHandleRecord, ContextScope};

#[derive(Debug)]
pub enum ContextError {
    Store(store::ContextError),
    QualityFailed {
        missing_markers: Vec<String>,
        quality: Box<viden_types::ContextQualityRecord>,
    },
    InvalidReductionPolicy {
        field: &'static str,
        reason: &'static str,
    },
    ReductionInputTooLarge {
        byte_count: usize,
        max_input_bytes: usize,
    },
}

impl Display for ContextError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(err) => write!(formatter, "{err}"),
            Self::QualityFailed {
                missing_markers, ..
            } => {
                write!(
                    formatter,
                    "context quality failed: missing required markers {missing_markers:?}"
                )
            }
            Self::InvalidReductionPolicy { field, reason } => {
                write!(formatter, "invalid reduction policy: {field} {reason}")
            }
            Self::ReductionInputTooLarge {
                byte_count,
                max_input_bytes,
            } => write!(
                formatter,
                "reduction input too large: byte_count={byte_count}, max_input_bytes={max_input_bytes}"
            ),
        }
    }
}

impl Error for ContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(err) => Some(err),
            Self::QualityFailed { .. }
            | Self::InvalidReductionPolicy { .. }
            | Self::ReductionInputTooLarge { .. } => None,
        }
    }
}

impl From<store::ContextError> for ContextError {
    fn from(err: store::ContextError) -> Self {
        Self::Store(err)
    }
}

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
        Ok(self.store.put(request)?)
    }

    pub fn retrieve(
        &self,
        handle: &ContextHandleRecord,
        scope: &ContextScope,
    ) -> Result<Vec<u8>, ContextError> {
        Ok(self.store.retrieve(handle, scope)?)
    }
}
