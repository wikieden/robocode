use std::time::Duration;

use viden_types::{
    CoreHandshake, EventCursorOrder, GapRecovery, ReplayRequest, RuntimeCommandEnvelope,
    RuntimeEventEnvelope, RuntimeSnapshotEnvelope, RuntimeViewState, RuntimeWireEvent,
    TranscriptPage, TranscriptPageRequest,
};

use crate::compatibility::{validate_capabilities, validate_handshake, validate_schema_version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreClientError {
    Compatibility(String),
    Protocol(String),
    Transport(String),
    SnapshotRequired { reason_code: String },
    MissingSnapshot,
    HandshakeRequired,
}

impl std::fmt::Display for CoreClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compatibility(message) => {
                write!(formatter, "core compatibility error: {message}")
            }
            Self::Protocol(message) => write!(formatter, "core protocol error: {message}"),
            Self::Transport(message) => write!(formatter, "core transport error: {message}"),
            Self::SnapshotRequired { reason_code } => {
                write!(formatter, "runtime snapshot required: {reason_code}")
            }
            Self::MissingSnapshot => write!(formatter, "runtime snapshot has not been loaded"),
            Self::HandshakeRequired => write!(formatter, "core handshake has not been confirmed"),
        }
    }
}

impl std::error::Error for CoreClientError {}

impl From<GapRecovery> for CoreClientError {
    fn from(recovery: GapRecovery) -> Self {
        match recovery {
            GapRecovery::Replay(_) => {
                Self::Transport("unexpected nested replay recovery request".to_string())
            }
            GapRecovery::SnapshotRequired { reason_code } => Self::SnapshotRequired { reason_code },
        }
    }
}

pub trait CoreTransport: Send {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError>;
    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError>;
    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError>;
    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError>;
    fn replay(
        &mut self,
        request: ReplayRequest,
    ) -> Result<viden_types::ReplayBatch, CoreClientError>;
    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError>;
}

pub trait CoreClient: Send {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError>;
    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError>;
    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError>;
    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError>;
    fn replay(
        &mut self,
        request: ReplayRequest,
    ) -> Result<viden_types::ReplayBatch, CoreClientError>;
    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError>;
}

pub struct StatefulCoreClient<T> {
    transport: T,
    handshake: Option<CoreHandshake>,
    confirmed: Option<RuntimeSnapshotEnvelope>,
}

impl<T> StatefulCoreClient<T>
where
    T: CoreTransport,
{
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            handshake: None,
            confirmed: None,
        }
    }

    pub fn confirmed_handshake(&self) -> Option<&CoreHandshake> {
        self.handshake.as_ref()
    }

    pub fn confirmed_view(&self) -> Option<&RuntimeViewState> {
        self.confirmed.as_ref().map(|snapshot| &snapshot.view)
    }

    pub fn confirmed_cursor(&self) -> Option<&viden_types::EventCursor> {
        self.confirmed.as_ref().map(|snapshot| &snapshot.cursor)
    }

    fn ensure_handshake(&self) -> Result<(), CoreClientError> {
        if self.handshake.is_some() {
            Ok(())
        } else {
            Err(CoreClientError::HandshakeRequired)
        }
    }

    fn apply_envelope_to(
        confirmed: &mut RuntimeSnapshotEnvelope,
        envelope: &RuntimeEventEnvelope,
    ) -> Result<(), CoreClientError> {
        Self::validate_event_envelope(envelope)?;
        match &envelope.event {
            RuntimeWireEvent::Known(event) => confirmed.view.apply_event(event),
            RuntimeWireEvent::Unknown { .. } => {}
        }
        confirmed.cursor = envelope.cursor.clone();
        Ok(())
    }

    fn validate_event_envelope(envelope: &RuntimeEventEnvelope) -> Result<(), CoreClientError> {
        validate_schema_version(envelope.schema_version).map_err(CoreClientError::Compatibility)?;
        if let RuntimeWireEvent::Known(event) = &envelope.event
            && event.sequence != envelope.cursor.sequence
        {
            return Err(CoreClientError::Protocol(format!(
                "runtime event sequence {} does not match cursor sequence {}",
                event.sequence, envelope.cursor.sequence
            )));
        }
        Ok(())
    }

    fn validate_snapshot_envelope(
        snapshot: &RuntimeSnapshotEnvelope,
    ) -> Result<(), CoreClientError> {
        validate_schema_version(snapshot.schema_version).map_err(CoreClientError::Compatibility)?;
        validate_capabilities(&snapshot.capabilities).map_err(CoreClientError::Compatibility)?;
        if snapshot.cursor.stream_id.is_empty() {
            return Err(CoreClientError::Protocol(
                "runtime snapshot stream id must not be empty".to_string(),
            ));
        }
        if snapshot.snapshot != snapshot.view.snapshot {
            return Err(CoreClientError::Protocol(
                "runtime snapshot and projected view disagree".to_string(),
            ));
        }
        Ok(())
    }

    fn apply_replay_batch_to(
        staged: &mut RuntimeSnapshotEnvelope,
        batch: &viden_types::ReplayBatch,
    ) -> Result<(), CoreClientError> {
        Self::validate_replay_batch(&staged.cursor, batch)?;
        for envelope in &batch.events {
            Self::apply_envelope_to(staged, envelope)?;
        }
        Ok(())
    }

    fn validate_replay_batch(
        after: &viden_types::EventCursor,
        batch: &viden_types::ReplayBatch,
    ) -> Result<(), CoreClientError> {
        let mut cursor = after.clone();
        for envelope in &batch.events {
            Self::validate_event_envelope(envelope)?;
            match cursor.classify_incoming(&envelope.cursor) {
                EventCursorOrder::Next => cursor = envelope.cursor.clone(),
                order => {
                    return Err(CoreClientError::Protocol(format!(
                        "non-contiguous replay after {}:{}: received {}:{} ({order:?})",
                        after.stream_id,
                        after.sequence,
                        envelope.cursor.stream_id,
                        envelope.cursor.sequence
                    )));
                }
            }
        }
        if batch.next != cursor {
            return Err(CoreClientError::Protocol(format!(
                "replay next cursor {}:{} does not match applied cursor {}:{}",
                batch.next.stream_id, batch.next.sequence, cursor.stream_id, cursor.sequence
            )));
        }
        Ok(())
    }

    fn apply_envelope(&mut self, envelope: &RuntimeEventEnvelope) -> Result<(), CoreClientError> {
        let Some(confirmed) = self.confirmed.as_mut() else {
            return Err(CoreClientError::MissingSnapshot);
        };
        Self::apply_envelope_to(confirmed, envelope)
    }

    fn load_snapshot_recovery(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        let snapshot = self.transport.snapshot()?;
        Self::validate_snapshot_envelope(&snapshot)?;
        self.confirmed = Some(snapshot.clone());
        Ok(snapshot)
    }

    fn recover_gap(
        &mut self,
        incoming: RuntimeEventEnvelope,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let after = self
            .confirmed_cursor()
            .cloned()
            .ok_or(CoreClientError::MissingSnapshot)?;
        // Recovery is staged on a clone and published only after every replay
        // batch is contiguous, compatible, complete, and includes the gap event.
        let mut staged = self
            .confirmed
            .clone()
            .ok_or(CoreClientError::MissingSnapshot)?;
        let mut request = ReplayRequest { after, limit: 500 };
        let mut delivered = None;
        loop {
            let batch = match self.transport.replay(request.clone()) {
                Ok(batch) => batch,
                Err(CoreClientError::SnapshotRequired { .. }) => {
                    self.load_snapshot_recovery()?;
                    return Ok(None);
                }
                Err(err) => return Err(err),
            };
            if batch.events.is_empty() && !batch.complete {
                return Err(CoreClientError::Protocol(
                    "runtime replay made no progress".to_string(),
                ));
            }
            Self::apply_replay_batch_to(&mut staged, &batch)?;
            for envelope in &batch.events {
                if envelope.cursor == incoming.cursor {
                    delivered = Some(envelope.clone());
                }
            }
            if batch.complete {
                break;
            }
            request = ReplayRequest {
                after: batch.next,
                limit: 500,
            };
        }
        if staged.cursor.sequence < incoming.cursor.sequence || delivered.is_none() {
            return Err(CoreClientError::Protocol(format!(
                "complete replay ended at {}:{} before incoming cursor {}:{}",
                staged.cursor.stream_id,
                staged.cursor.sequence,
                incoming.cursor.stream_id,
                incoming.cursor.sequence
            )));
        }
        self.confirmed = Some(staged);
        Ok(delivered)
    }
}

impl<T> CoreClient for StatefulCoreClient<T>
where
    T: CoreTransport,
{
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        let handshake = self.transport.discover()?;
        validate_handshake(&handshake).map_err(CoreClientError::Compatibility)?;
        self.handshake = Some(handshake.clone());
        Ok(handshake)
    }

    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.ensure_handshake()?;
        validate_schema_version(command.schema_version).map_err(CoreClientError::Compatibility)?;
        self.transport.send(command)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        self.ensure_handshake()?;
        let Some(incoming) = self.transport.recv(timeout)? else {
            return Ok(None);
        };
        // Validate before any recovery I/O so an incompatible envelope cannot
        // replace the last confirmed client state as a side effect.
        Self::validate_event_envelope(&incoming)?;

        if self.confirmed.is_none() {
            self.load_snapshot_recovery()?;
        }
        let cursor = self
            .confirmed_cursor()
            .cloned()
            .ok_or(CoreClientError::MissingSnapshot)?;
        match cursor.classify_incoming(&incoming.cursor) {
            EventCursorOrder::DuplicateOrOld => Ok(None),
            EventCursorOrder::Next => {
                self.apply_envelope(&incoming)?;
                Ok(Some(incoming))
            }
            EventCursorOrder::Gap => self.recover_gap(incoming),
            EventCursorOrder::StreamMismatch => {
                self.load_snapshot_recovery()?;
                Ok(None)
            }
        }
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        self.ensure_handshake()?;
        self.load_snapshot_recovery()
    }

    fn replay(
        &mut self,
        request: ReplayRequest,
    ) -> Result<viden_types::ReplayBatch, CoreClientError> {
        self.ensure_handshake()?;
        let after = request.after.clone();
        let batch = self.transport.replay(request)?;
        Self::validate_replay_batch(&after, &batch)?;
        Ok(batch)
    }

    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        self.ensure_handshake()?;
        self.transport.transcript_page(request)
    }
}
