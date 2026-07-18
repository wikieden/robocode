#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum ContextScope {
    Task(String),
    Dag(String),
    Workflow(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum CostScope {
    Request(String),
    AgentTask(String),
    Dag(String),
    Workflow(String),
    SmokeRun(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub retrieval_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CostAmount {
    pub currency: String,
    pub micro_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CostEstimate {
    pub amount: CostAmount,
    pub provider_id: String,
    pub model: String,
    pub price_table_version: String,
    pub estimated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostUsageOutcome {
    Success,
    Failure,
    Cancelled,
}

impl CostUsageOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
        }
    }
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
    #[serde(default = "default_context_retrieval_permission_decision")]
    pub permission_decision: String,
    #[serde(default = "default_context_retrieval_reason_rule_category")]
    pub reason_rule_category: String,
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

fn default_context_retrieval_permission_decision() -> String {
    "unknown".to_string()
}

fn default_context_retrieval_reason_rule_category() -> String {
    "unknown".to_string()
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
    pub scopes: Vec<CostScope>,
    pub tokens: TokenUsage,
    pub estimate: Option<CostEstimate>,
    pub actual_cost: Option<CostAmount>,
    pub attempt_index: u32,
    pub outcome: CostUsageOutcome,
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
    pub retrieval_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<CostAmount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_cost: Option<CostAmount>,
    pub total_estimated_cost_micro_usd: u64,
    pub total_actual_cost_micro_usd: Option<u64>,
}

impl CostLedgerTotals {
    pub fn record(&mut self, cost: &CostUsageRecord) {
        self.input_tokens = self
            .input_tokens
            .saturating_add(cost.tokens.input_tokens.unwrap_or(0));
        self.output_tokens = self
            .output_tokens
            .saturating_add(cost.tokens.output_tokens.unwrap_or(0));
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(cost.tokens.cached_input_tokens.unwrap_or(0));
        self.retrieval_tokens = self
            .retrieval_tokens
            .saturating_add(cost.tokens.retrieval_tokens.unwrap_or(0));
        self.total_tokens = self
            .total_tokens
            .saturating_add(cost.tokens.total_tokens.unwrap_or(0));
        if let Some(estimate) = &cost.estimate {
            self.total_estimated_cost_micro_usd = self
                .total_estimated_cost_micro_usd
                .saturating_add(estimate.amount.micro_units);
            self.estimated_cost = Some(CostAmount {
                currency: estimate.amount.currency.clone(),
                micro_units: self
                    .estimated_cost
                    .as_ref()
                    .map(|amount| amount.micro_units)
                    .unwrap_or(0)
                    .saturating_add(estimate.amount.micro_units),
            });
        }
        match (&self.actual_cost, &cost.actual_cost) {
            (_, None) => {
                self.actual_cost = None;
                self.total_actual_cost_micro_usd = None;
            }
            (None, Some(actual)) if self.total_tokens == cost.tokens.total_tokens.unwrap_or(0) => {
                self.actual_cost = Some(actual.clone());
                self.total_actual_cost_micro_usd = Some(actual.micro_units);
            }
            (Some(current), Some(actual)) => {
                let micro_units = current.micro_units.saturating_add(actual.micro_units);
                self.actual_cost = Some(CostAmount {
                    currency: current.currency.clone(),
                    micro_units,
                });
                self.total_actual_cost_micro_usd = Some(micro_units);
            }
            (None, Some(_)) => {}
        }
    }
}
