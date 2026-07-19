use std::time::Duration;

use viden_runtime::RuntimeSupervisor;
use viden_types::{
    CoreHandshake, GapRecovery, ReplayBatch, ReplayRequest, RuntimeCommandEnvelope,
    RuntimeEventEnvelope, RuntimeSnapshotEnvelope, TranscriptPage, TranscriptPageRequest,
};

use crate::{
    client::{CoreClientError, CoreTransport},
    compatibility::local_core_handshake,
};

pub struct LocalCoreTransport {
    supervisor: RuntimeSupervisor,
}

impl LocalCoreTransport {
    pub fn new(supervisor: RuntimeSupervisor) -> Self {
        Self { supervisor }
    }
}

impl CoreTransport for LocalCoreTransport {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        Ok(local_core_handshake())
    }

    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.supervisor
            .send_command_envelope(command)
            .map_err(CoreClientError::Transport)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        Ok(self.supervisor.recv_event_envelope_timeout(timeout))
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        self.supervisor
            .snapshot_envelope()
            .map_err(CoreClientError::Transport)
    }

    fn replay(&mut self, request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
        self.supervisor
            .replay_events(request)
            .map_err(|recovery| match recovery {
                GapRecovery::SnapshotRequired { reason_code } => {
                    CoreClientError::SnapshotRequired { reason_code }
                }
                GapRecovery::Replay(_) => {
                    CoreClientError::Transport("unexpected replay recovery request".to_string())
                }
            })
    }

    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        self.supervisor
            .load_transcript_page(request)
            .map_err(CoreClientError::Transport)
    }
}
