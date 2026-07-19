use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_core::{RuntimeEventEnvelope, RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent};

const D1_FIXTURE: &str = include_str!(
    "../../../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

#[derive(Deserialize)]
struct Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
    expected_view_sha256: String,
}

/// Identity and digest derived from the canonical D1 fixture through Core's reducer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct D1FixtureProjection {
    pub project_id: String,
    pub lane_id: String,
    pub session_id: String,
    pub task_id: String,
    pub view_hash: String,
}

impl D1FixtureProjection {
    pub fn from_committed_fixture() -> Result<Self, String> {
        let fixture: Fixture = serde_json::from_str(D1_FIXTURE)
            .map_err(|error| format!("parse canonical D1 fixture: {error}"))?;
        let owner = fixture
            .events
            .first()
            .ok_or_else(|| "canonical D1 fixture has no events".to_string())?
            .owner
            .clone();
        let mut view = RuntimeViewState::new(fixture.initial_snapshot);
        for envelope in fixture.events {
            match &envelope.event {
                RuntimeWireEvent::Known(event) => view.apply_event(event),
                RuntimeWireEvent::Unknown { event_type, .. } => {
                    return Err(format!(
                        "canonical D1 fixture contains unknown event `{event_type}`"
                    ));
                }
            }
        }
        let view_hash = canonical_view_sha256(&view)?;
        if view_hash != fixture.expected_view_sha256 {
            return Err(format!(
                "canonical D1 projection digest mismatch: expected {}, received {view_hash}",
                fixture.expected_view_sha256
            ));
        }
        let lane_id = view
            .lanes
            .first()
            .ok_or_else(|| "canonical D1 projection has no lane".to_string())?
            .id
            .clone();
        let task_id = view
            .tasks
            .first()
            .ok_or_else(|| "canonical D1 projection has no task".to_string())?
            .id
            .clone();
        let session_id = owner
            .session_id
            .ok_or_else(|| "canonical D1 fixture owner has no session".to_string())?;
        Ok(Self {
            project_id: owner.project_id,
            lane_id,
            session_id,
            task_id,
            view_hash,
        })
    }
}

fn canonical_view_sha256(view: &RuntimeViewState) -> Result<String, String> {
    let value = serde_json::to_value(view)
        .map_err(|error| format!("serialize canonical D1 projection: {error}"))?;
    let bytes = serde_json::to_vec(&sort_json(value))
        .map_err(|error| format!("encode canonical D1 projection: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::D1FixtureProjection;

    #[test]
    fn derives_identity_and_digest_through_the_core_runtime_view() {
        let projection = D1FixtureProjection::from_committed_fixture().unwrap();

        assert_eq!(projection.project_id, "project_viden");
        assert_eq!(projection.lane_id, "lane_d1_core");
        assert_eq!(projection.session_id, "session_d1-vertical-slice");
        assert_eq!(projection.task_id, "task_d1_core");
        assert_eq!(
            projection.view_hash,
            "7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804"
        );
    }
}
