use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum D6ConnectionState {
    Disconnected,
    Connecting,
    Live,
    Recovering,
    Incompatible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum D6State {
    Live,
    Empty,
    Connecting,
    Disconnected,
    ProviderError,
    AgentStopped,
    ContextOverflow,
    GateQueueClear,
    IncompatibleSchema,
    MissingFeatureCapability,
    EventGap,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct D6ActionProjection {
    pub kind: &'static str,
    pub available: bool,
    pub code: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D6RecoveryProjection {
    pub connection: D6ConnectionState,
    pub state: D6State,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub recoverable: bool,
    pub business_success_blocked: bool,
    pub used_tokens: Option<u64>,
    pub hard_token_limit: Option<u64>,
    pub missing_capabilities: Vec<String>,
    pub actions: Vec<D6ActionProjection>,
}
