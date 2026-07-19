use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{
    CoreClient, CoreClientError, CoreHandshake, CoreTransport, EventCursor, ReplayBatch,
    ReplayRequest, RuntimeCommandEnvelope, RuntimeEventEnvelope, RuntimeSnapshotEnvelope,
    StatefulCoreClient, TranscriptPage, TranscriptPageRequest,
};

use crate::{ConfirmedState, GateMetrics, GuiProjection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiConnectionState {
    Disconnected,
    Connecting,
    Live {
        cursor: EventCursor,
    },
    Recovering {
        expected: EventCursor,
        received: EventCursor,
    },
    Incompatible {
        reason: String,
    },
}

#[derive(Debug, Default)]
struct TransportObservation {
    last_received: Option<EventCursor>,
    replay_calls: u64,
}

struct ObservedTransport<T> {
    inner: T,
    observation: Arc<Mutex<TransportObservation>>,
}

impl<T> CoreTransport for ObservedTransport<T>
where
    T: CoreTransport,
{
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        self.inner.discover()
    }

    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.inner.send(command)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let result = self.inner.recv(timeout);
        let cursor = result
            .as_ref()
            .ok()
            .and_then(|event| event.as_ref())
            .map(|event| event.cursor.clone());
        self.observation.lock().unwrap().last_received = cursor;
        result
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        self.inner.snapshot()
    }

    fn replay(&mut self, request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
        let mut observation = self.observation.lock().unwrap();
        observation.replay_calls = observation.replay_calls.saturating_add(1);
        drop(observation);
        self.inner.replay(request)
    }

    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        self.inner.transcript_page(request)
    }
}

pub struct GuiCoreAdapter<T>
where
    T: CoreTransport,
{
    client: StatefulCoreClient<ObservedTransport<T>>,
    observation: Arc<Mutex<TransportObservation>>,
    connection: GuiConnectionState,
    projection: GuiProjection,
    metrics: GateMetrics,
}

impl<T> GuiCoreAdapter<T>
where
    T: CoreTransport,
{
    pub fn new(transport: T) -> Self {
        let observation = Arc::new(Mutex::new(TransportObservation::default()));
        let transport = ObservedTransport {
            inner: transport,
            observation: Arc::clone(&observation),
        };
        Self {
            client: StatefulCoreClient::new(transport),
            observation,
            connection: GuiConnectionState::Disconnected,
            projection: GuiProjection::default(),
            metrics: GateMetrics::default(),
        }
    }

    pub fn connect(&mut self) -> Result<(), CoreClientError> {
        self.connection = GuiConnectionState::Connecting;
        if let Err(error) = self.client.discover() {
            self.transition_error(&error, None, None, false);
            return Err(error);
        }
        if let Err(error) = self.client.snapshot() {
            self.transition_error(&error, None, None, false);
            return Err(error);
        }
        self.publish_confirmed();
        Ok(())
    }

    pub fn send_intent(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.client.send(command)
    }

    pub fn pump(&mut self, timeout: Duration) -> Result<(), CoreClientError> {
        let before = self.client.confirmed_cursor().cloned();
        let replay_before = self.observation.lock().unwrap().replay_calls;
        self.observation.lock().unwrap().last_received = None;

        let result = self.client.recv(timeout);
        let observation = self.observation.lock().unwrap();
        let received = observation.last_received.clone();
        let replay_after = observation.replay_calls;
        drop(observation);
        let replay_delta = replay_after.saturating_sub(replay_before);
        self.metrics.replay_batches_observed = self
            .metrics
            .replay_batches_observed
            .saturating_add(replay_delta);

        match result {
            Ok(_) => {
                let after = self.client.confirmed_cursor().cloned();
                self.record_confirmed_progress(before.as_ref(), after.as_ref(), replay_delta);
                self.publish_confirmed();
                Ok(())
            }
            Err(error) => {
                self.transition_error(&error, before.as_ref(), received.as_ref(), replay_delta > 0);
                Err(error)
            }
        }
    }

    pub fn recover(&mut self) -> Result<(), CoreClientError> {
        match self.client.snapshot() {
            Ok(_) => {
                self.metrics.snapshot_replacements =
                    self.metrics.snapshot_replacements.saturating_add(1);
                self.publish_confirmed();
                Ok(())
            }
            Err(error) => {
                if matches!(error, CoreClientError::Compatibility(_)) {
                    self.transition_error(&error, None, None, false);
                }
                Err(error)
            }
        }
    }

    pub fn connection_state(&self) -> &GuiConnectionState {
        &self.connection
    }

    pub fn projection(&self) -> &GuiProjection {
        &self.projection
    }

    pub fn metrics(&self) -> &GateMetrics {
        &self.metrics
    }

    fn publish_confirmed(&mut self) {
        let Some(state) = ConfirmedState::from_core(&self.client) else {
            return;
        };
        let cursor = self
            .client
            .confirmed_cursor()
            .cloned()
            .expect("confirmed state includes a cursor");
        self.projection.apply_batch([state]);
        self.metrics.confirmed_sync_count = self.metrics.confirmed_sync_count.saturating_add(1);
        self.connection = GuiConnectionState::Live { cursor };
    }

    fn record_confirmed_progress(
        &mut self,
        before: Option<&EventCursor>,
        after: Option<&EventCursor>,
        replay_delta: u64,
    ) {
        if let (Some(before), Some(after)) = (before, after) {
            if before.stream_id == after.stream_id {
                self.metrics.confirmed_event_count = self
                    .metrics
                    .confirmed_event_count
                    .saturating_add(after.sequence.saturating_sub(before.sequence));
            } else {
                self.metrics.snapshot_replacements =
                    self.metrics.snapshot_replacements.saturating_add(1);
            }
        }
        if replay_delta > 0 {
            self.metrics.gap_recoveries = self.metrics.gap_recoveries.saturating_add(1);
        }
    }

    fn transition_error(
        &mut self,
        error: &CoreClientError,
        before: Option<&EventCursor>,
        received: Option<&EventCursor>,
        replay_attempted: bool,
    ) {
        if matches!(error, CoreClientError::Compatibility(_)) {
            self.connection = GuiConnectionState::Incompatible {
                reason: error.to_string(),
            };
            return;
        }
        if replay_attempted && let (Some(before), Some(received)) = (before, received) {
            self.connection = GuiConnectionState::Recovering {
                expected: EventCursor {
                    stream_id: before.stream_id.clone(),
                    sequence: before.sequence.saturating_add(1),
                },
                received: received.clone(),
            };
            return;
        }
        self.connection = GuiConnectionState::Disconnected;
    }
}
