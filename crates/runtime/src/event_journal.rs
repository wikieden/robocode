use std::collections::VecDeque;

use viden_types::{
    EventCursor, FRONTEND_SCHEMA_V1, GapRecovery, ReplayBatch, ReplayRequest, RuntimeEventEnvelope,
    RuntimeWireEvent, fresh_id,
};

pub(crate) const DEFAULT_RUNTIME_EVENT_JOURNAL_CAPACITY: usize = 10_000;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeEventJournal {
    stream_id: String,
    capacity: usize,
    events: VecDeque<RuntimeEventEnvelope>,
}

impl RuntimeEventJournal {
    pub(crate) fn new(stream_id: impl Into<String>, capacity: usize) -> Self {
        let stream_id = stream_id.into();
        let stream_id = if stream_id.is_empty() {
            fresh_id("runtime-stream")
        } else {
            stream_id
        };
        let capacity = capacity.max(1);
        Self {
            stream_id,
            capacity,
            events: VecDeque::with_capacity(capacity),
        }
    }

    pub(crate) fn default_with_stream(stream_id: impl Into<String>) -> Self {
        Self::new(stream_id, DEFAULT_RUNTIME_EVENT_JOURNAL_CAPACITY)
    }

    pub(crate) fn initial_cursor(&self) -> EventCursor {
        EventCursor {
            stream_id: self.stream_id.clone(),
            sequence: 0,
        }
    }

    pub(crate) fn current_cursor(&self) -> EventCursor {
        self.events
            .back()
            .map(|envelope| envelope.cursor.clone())
            .unwrap_or_else(|| self.initial_cursor())
    }

    pub(crate) fn record(&mut self, mut envelope: RuntimeEventEnvelope) -> RuntimeEventEnvelope {
        if envelope.schema_version != FRONTEND_SCHEMA_V1 {
            return envelope;
        }
        let next_sequence = self.current_cursor().sequence.saturating_add(1);
        envelope.cursor = EventCursor {
            stream_id: self.stream_id.clone(),
            sequence: next_sequence,
        };
        if let RuntimeWireEvent::Known(event) = &mut envelope.event {
            // The journal owns the externally visible event order. RuntimeEngine
            // batches may carry local sequence values, so the wire event is
            // resequenced atomically with its replay cursor.
            event.sequence = next_sequence;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(envelope.clone());
        envelope
    }

    pub(crate) fn replay(&self, request: ReplayRequest) -> Result<ReplayBatch, GapRecovery> {
        let limit = request.limit.clamp(1, 500) as usize;
        if request.after.stream_id != self.stream_id {
            return Err(GapRecovery::SnapshotRequired {
                reason_code: "stream_mismatch".to_string(),
            });
        }

        let current = self.current_cursor();
        if request.after.sequence > current.sequence {
            return Err(GapRecovery::SnapshotRequired {
                reason_code: "cursor_ahead".to_string(),
            });
        }
        if request.after.sequence == current.sequence {
            return Ok(ReplayBatch {
                events: Vec::new(),
                next: current,
                complete: true,
            });
        }

        if let Some(first) = self.events.front()
            && request.after.sequence < first.cursor.sequence.saturating_sub(1)
        {
            return Err(GapRecovery::SnapshotRequired {
                reason_code: "retention_expired".to_string(),
            });
        }

        let events = self
            .events
            .iter()
            .filter(|envelope| envelope.cursor.sequence > request.after.sequence)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next = events
            .last()
            .map(|envelope| envelope.cursor.clone())
            .unwrap_or_else(|| request.after.clone());
        let complete = next.sequence == current.sequence;

        Ok(ReplayBatch {
            events,
            next,
            complete,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::{RuntimeEvent, RuntimeEventKind, RuntimeOwner, RuntimeWireEvent};

    fn envelope(sequence: u64, stream_id: &str) -> RuntimeEventEnvelope {
        RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: RuntimeOwner::default(),
            cursor: EventCursor {
                stream_id: stream_id.to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::new(
                sequence,
                RuntimeEventKind::AssistantDelta {
                    message_id: format!("msg-{sequence}"),
                    task_id: None,
                    session_id: None,
                    content: format!("e{sequence}"),
                },
            )),
        }
    }

    #[test]
    fn event_journal_sequence_starts_at_one_and_ignores_duplicate_old() {
        let mut journal = RuntimeEventJournal::new("stream", 10);
        assert_eq!(journal.current_cursor().sequence, 0);

        journal.record(envelope(42, "engine"));
        journal.record(envelope(42, "engine"));
        assert_eq!(journal.current_cursor().sequence, 2);
        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: journal.initial_cursor(),
                    limit: 10,
                })
                .unwrap()
                .events
                .iter()
                .map(|event| (
                    event.cursor.stream_id.as_str(),
                    event.cursor.sequence,
                    match &event.event {
                        RuntimeWireEvent::Known(event) => event.sequence,
                        RuntimeWireEvent::Unknown { .. } => 0,
                    },
                ))
                .collect::<Vec<_>>(),
            vec![("stream", 1, 1), ("stream", 2, 2)]
        );
        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: journal.current_cursor(),
                    limit: 10,
                })
                .unwrap()
                .events
                .len(),
            0
        );
    }

    #[test]
    fn event_journal_replay_is_ordered_bounded_and_empty_at_current() {
        let mut journal = RuntimeEventJournal::new("stream", 10);
        for sequence in 1..=4 {
            journal.record(envelope(sequence, "stream"));
        }

        let batch = journal
            .replay(ReplayRequest {
                after: EventCursor {
                    stream_id: "stream".to_string(),
                    sequence: 1,
                },
                limit: 2,
            })
            .unwrap();
        assert_eq!(
            batch
                .events
                .iter()
                .map(|event| event.cursor.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(batch.next.sequence, 3);
        assert!(!batch.complete);

        let empty = journal
            .replay(ReplayRequest {
                after: journal.current_cursor(),
                limit: 50,
            })
            .unwrap();
        assert!(empty.events.is_empty());
        assert!(empty.complete);
    }

    #[test]
    fn event_journal_retention_expiry_changed_stream_and_capacity_eviction_request_snapshot() {
        let mut journal = RuntimeEventJournal::new("stream", 2);
        journal.record(envelope(1, "stream"));
        journal.record(envelope(2, "stream"));
        journal.record(envelope(3, "stream"));

        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: EventCursor {
                        stream_id: "stream".to_string(),
                        sequence: 0,
                    },
                    limit: 10,
                })
                .unwrap_err(),
            GapRecovery::SnapshotRequired {
                reason_code: "retention_expired".to_string()
            }
        );
        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: EventCursor {
                        stream_id: "other".to_string(),
                        sequence: 2,
                    },
                    limit: 10,
                })
                .unwrap_err(),
            GapRecovery::SnapshotRequired {
                reason_code: "stream_mismatch".to_string()
            }
        );
        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: EventCursor {
                        stream_id: "stream".to_string(),
                        sequence: 1,
                    },
                    limit: 10,
                })
                .unwrap()
                .events
                .iter()
                .map(|event| event.cursor.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );

        let zero_capacity = RuntimeEventJournal::new("zero", 0);
        assert_eq!(zero_capacity.capacity, 1);
    }

    #[test]
    fn event_journal_normalizes_empty_stream_and_rejects_cursor_ahead_of_head() {
        let mut journal = RuntimeEventJournal::new("", 4);
        assert!(!journal.initial_cursor().stream_id.is_empty());
        journal.record(envelope(1, "ignored"));

        assert_eq!(
            journal
                .replay(ReplayRequest {
                    after: EventCursor {
                        stream_id: journal.current_cursor().stream_id,
                        sequence: 2,
                    },
                    limit: 10,
                })
                .unwrap_err(),
            GapRecovery::SnapshotRequired {
                reason_code: "cursor_ahead".to_string()
            }
        );
    }
}
