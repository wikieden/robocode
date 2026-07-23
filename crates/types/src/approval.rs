use crate::SessionId;

/// Stable risk buckets are semantic facts, not localized frontend labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalTarget {
    pub kind: String,
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    Once,
    Session { session_id: SessionId },
    RepoAllowlist { paths: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow { scope: ApprovalScope },
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDefaultAction {
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ApprovalResponse {
    pub decision: ApprovalDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
}

impl ApprovalResponse {
    pub fn allow_once(feedback: Option<String>) -> Self {
        Self {
            decision: ApprovalDecision::Allow {
                scope: ApprovalScope::Once,
            },
            feedback,
        }
    }

    pub fn deny(feedback: Option<String>) -> Self {
        Self {
            decision: ApprovalDecision::Deny,
            feedback,
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self.decision, ApprovalDecision::Allow { .. })
    }
}

impl<'de> serde::Deserialize<'de> for ApprovalResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Structured {
            decision: ApprovalDecision,
            #[serde(default)]
            feedback: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct Legacy {
            approved: bool,
            #[serde(default)]
            feedback: Option<String>,
        }

        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Structured(Structured),
            Legacy(Legacy),
        }

        match Wire::deserialize(deserializer)? {
            Wire::Structured(value) => Ok(Self {
                decision: value.decision,
                feedback: value.feedback,
            }),
            Wire::Legacy(value) if value.approved => Ok(Self::allow_once(value.feedback)),
            Wire::Legacy(value) => Ok(Self::deny(value.feedback)),
        }
    }
}
