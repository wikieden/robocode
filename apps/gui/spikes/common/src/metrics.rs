/// Deterministic counters collected by the framework-neutral gate harness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateMetrics {
    pub confirmed_event_count: u64,
    pub confirmed_sync_count: u64,
    pub gap_recoveries: u64,
    pub replay_batches_observed: u64,
    pub snapshot_replacements: u64,
}
