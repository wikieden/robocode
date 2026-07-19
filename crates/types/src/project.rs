#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectConfigState {
    Missing,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectProbe {
    pub root: String,
    pub is_git_repository: bool,
    pub git_root: Option<String>,
    pub config_path: String,
    pub config_state: ProjectConfigState,
    pub project_name: Option<String>,
    pub pack: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectConfigPreview {
    pub preview_id: String,
    pub relative_path: String,
    pub content_sha256: String,
    pub byte_len: u64,
    /// Exact UTF-8 bytes rendered for review. Invalid, potentially
    /// secret-bearing candidates omit this field and cannot be confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_contents: Option<String>,
    pub base_content_sha256: Option<String>,
    pub project_name: Option<String>,
    pub pack: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

impl ProjectConfigPreview {
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Available,
    Missing,
    Locked,
    Error,
}

/// Safe credential metadata. Secret bytes belong exclusively to the injected
/// credential backend and are intentionally absent from this serialized fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CredentialHandle {
    pub provider_id: String,
    pub backend_id: String,
    pub status: CredentialStatus,
}

/// Opaque, one-use credential staging handle.
///
/// The request id is safe to serialize in runtime commands; secret bytes stay
/// behind the trusted local host boundary and are never part of this DTO.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CredentialRequestId {
    id: String,
}

impl CredentialRequestId {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EventCursor, FRONTEND_SCHEMA_V1, RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind,
        RuntimeOwner, RuntimeWireEvent,
    };

    #[test]
    fn project_runtime_events_round_trip_as_known_schema_one_events() {
        let probe = ProjectProbe {
            root: "/project".to_string(),
            is_git_repository: true,
            git_root: Some("/project".to_string()),
            config_path: "/project/viden.toml".to_string(),
            config_state: ProjectConfigState::Valid,
            project_name: Some("demo".to_string()),
            pack: Some("robot-pack".to_string()),
            diagnostics: Vec::new(),
        };
        let preview = ProjectConfigPreview {
            preview_id: "preview-1".to_string(),
            relative_path: "viden.toml".to_string(),
            content_sha256: "a".repeat(64),
            byte_len: 48,
            exact_contents: Some("[project]\nname = \"demo\"\npack = \"robot-pack\"\n".to_string()),
            base_content_sha256: None,
            project_name: Some("demo".to_string()),
            pack: Some("robot-pack".to_string()),
            diagnostics: Vec::new(),
        };
        let handle = CredentialHandle {
            provider_id: "provider".to_string(),
            backend_id: "keychain:item".to_string(),
            status: CredentialStatus::Available,
        };
        let request_id = CredentialRequestId::new("crq_1");
        assert_eq!(
            serde_json::from_value::<CredentialRequestId>(
                serde_json::to_value(&request_id).unwrap()
            )
            .unwrap(),
            request_id
        );
        assert_eq!(request_id.id(), "crq_1");
        let kinds = [
            RuntimeEventKind::ProjectProbed { probe },
            RuntimeEventKind::ProjectConfigPreviewed {
                preview: preview.clone(),
            },
            RuntimeEventKind::ProjectConfigConfirmed { preview },
            RuntimeEventKind::CredentialHandleStored { handle },
        ];
        for (index, kind) in kinds.into_iter().enumerate() {
            let sequence = index as u64 + 1;
            let envelope = RuntimeEventEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                owner: RuntimeOwner::default(),
                cursor: EventCursor {
                    stream_id: "project-events".to_string(),
                    sequence,
                },
                event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(sequence, None, kind)),
            };
            let decoded: RuntimeEventEnvelope =
                serde_json::from_value(serde_json::to_value(envelope).unwrap()).unwrap();
            assert!(matches!(decoded.event, RuntimeWireEvent::Known(_)));
        }
    }
}
