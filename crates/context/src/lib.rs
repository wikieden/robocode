pub mod cost;
pub mod reducer;
pub mod store;

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

pub use cost::{CostLedger, totals_from_records};
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

#[cfg(test)]
mod cost_tests {
    use super::*;
    use viden_types::{
        CostAmount, CostEstimate, CostScope, CostUsageOutcome, CostUsageRecord, TokenUsage,
    };

    fn provider_entry(id: &str, attempt_index: u32, actual: Option<u64>) -> CostUsageRecord {
        CostUsageRecord {
            usage_id: id.to_string(),
            provider_id: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            scopes: vec![
                CostScope::Request("req-1".to_string()),
                CostScope::AgentTask("task-1".to_string()),
                CostScope::Dag("dag-1".to_string()),
                CostScope::Workflow("wf-1".to_string()),
            ],
            tokens: TokenUsage {
                input_tokens: Some(1_000),
                output_tokens: Some(250),
                cached_input_tokens: Some(400),
                retrieval_tokens: Some(0),
                total_tokens: Some(1_250),
            },
            estimate: Some(CostEstimate {
                amount: CostAmount {
                    currency: "USD".to_string(),
                    micro_units: 2_100,
                },
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                price_table_version: "test-2026-07-18".to_string(),
                estimated: true,
            }),
            actual_cost: actual.map(|micro_units| CostAmount {
                currency: "USD".to_string(),
                micro_units,
            }),
            attempt_index,
            outcome: CostUsageOutcome::Success,
            recorded_at: Some(10 + u64::from(attempt_index)),
        }
    }

    #[test]
    fn ledger_aggregates_exact_tokens_and_unknown_actual_by_scope() {
        let mut ledger = CostLedger::default();
        ledger.append(provider_entry("usage-1", 0, Some(2_300)));
        ledger.append(provider_entry("usage-2", 1, None));

        let totals = ledger.totals_for_scope(&CostScope::Workflow("wf-1".to_string()));

        assert_eq!(totals.input_tokens, 2_000);
        assert_eq!(totals.output_tokens, 500);
        assert_eq!(totals.cached_input_tokens, 800);
        assert_eq!(totals.retrieval_tokens, 0);
        assert_eq!(totals.total_tokens, 2_500);
        assert_eq!(totals.estimated_cost.as_ref().unwrap().micro_units, 4_200);
        assert_eq!(totals.actual_cost, None);
        assert_eq!(
            ledger
                .records_for_scope(&CostScope::Request("req-1".to_string()))
                .len(),
            2
        );
    }

    #[test]
    fn ledger_deduplicates_usage_id_and_saturates_overflow() {
        let mut ledger = CostLedger::default();
        let mut entry = provider_entry("usage-dup", 0, Some(u64::MAX));
        entry.tokens.input_tokens = Some(u64::MAX);
        entry.tokens.output_tokens = Some(10);
        entry.tokens.total_tokens = Some(u64::MAX);
        entry.estimate.as_mut().unwrap().amount.micro_units = u64::MAX;

        ledger.append(entry.clone());
        ledger.append(entry);

        let totals = ledger.totals_for_scope(&CostScope::Workflow("wf-1".to_string()));
        assert_eq!(ledger.records().len(), 1);
        assert_eq!(totals.input_tokens, u64::MAX);
        assert_eq!(totals.total_tokens, u64::MAX);
        assert_eq!(
            totals.estimated_cost.as_ref().unwrap().micro_units,
            u64::MAX
        );
        assert_eq!(totals.actual_cost.as_ref().unwrap().micro_units, u64::MAX);
    }
}
