#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum ContextScope {
    Task(String),
    Dag(String),
    Workflow(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextContentKind {
    Json,
    Code,
    Diff,
    Log,
    Diagnostic,
    Transcript,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextHandleRecord {
    pub handle_id: String,
    pub item_id: String,
    pub preferred_view_id: Option<String>,
    pub content_sha256: String,
    pub scope: ContextScope,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextItemRecord {
    pub item_id: String,
    pub scope: ContextScope,
    pub kind: ContextContentKind,
    pub content_sha256: String,
    pub title: String,
    pub summary: String,
    pub token_count: u64,
    pub evidence_id: Option<String>,
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextViewRecord {
    pub view_id: String,
    pub item_id: String,
    pub kind: ContextContentKind,
    pub derivation: String,
    pub content_sha256: String,
    pub token_count: u64,
    pub quality_id: Option<String>,
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextRetrievalRecord {
    pub retrieval_id: String,
    pub handle_id: String,
    pub item_id: String,
    pub view_id: Option<String>,
    #[serde(default = "default_context_retrieval_scope")]
    pub scope: ContextScope,
    #[serde(default)]
    pub byte_count: u64,
    #[serde(default)]
    pub token_count: u64,
    #[serde(default = "default_context_retrieval_reason_category")]
    pub reason_category: String,
    pub reason: String,
    pub requester: String,
    pub retrieved_at: Option<u64>,
}

fn default_context_retrieval_scope() -> ContextScope {
    ContextScope::Task("unknown".to_string())
}

fn default_context_retrieval_reason_category() -> String {
    "retrieve".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextQualityRecord {
    pub quality_id: String,
    pub target_id: String,
    pub passed: bool,
    pub score_microunits: Option<u64>,
    pub checks: Vec<String>,
    pub failure_reason: Option<String>,
    pub checked_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextBudgetRecord {
    pub budget_id: String,
    pub scope: ContextScope,
    pub soft_token_limit: u64,
    pub hard_token_limit: u64,
    pub used_tokens: u64,
    pub remaining_tokens: u64,
    pub exceeded: bool,
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CostUsageRecord {
    pub usage_id: String,
    pub provider_id: String,
    pub model: String,
    pub scope: ContextScope,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_micro_usd: u64,
    pub actual_cost_micro_usd: Option<u64>,
    pub recorded_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextBundleSummaryRecord {
    pub bundle_id: String,
    pub scope: ContextScope,
    pub handle_ids: Vec<String>,
    pub estimated_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderCacheObservationRecord {
    pub provider_id: String,
    pub model: String,
    pub cached_input_tokens: u64,
    pub cache_hit_microunits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceCanonicalizationRecord {
    pub evidence_id: String,
    pub item_id: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct CostLedgerTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_tokens: u64,
    pub total_estimated_cost_micro_usd: u64,
    pub total_actual_cost_micro_usd: Option<u64>,
}

impl CostLedgerTotals {
    pub fn record(&mut self, cost: &CostUsageRecord) {
        self.input_tokens = self.input_tokens.saturating_add(cost.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(cost.output_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(cost.cached_input_tokens);
        self.total_tokens = self.total_tokens.saturating_add(cost.total_tokens);
        self.total_estimated_cost_micro_usd = self
            .total_estimated_cost_micro_usd
            .saturating_add(cost.estimated_cost_micro_usd);
        if let Some(actual_cost) = cost.actual_cost_micro_usd {
            self.total_actual_cost_micro_usd = Some(
                self.total_actual_cost_micro_usd
                    .unwrap_or(0)
                    .saturating_add(actual_cost),
            );
        }
    }
}
