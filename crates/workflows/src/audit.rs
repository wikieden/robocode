//! Append-only audit timeline persistence and paging.
//!
//! `audit.jsonl` is the canonical store. Unlike the workflow projections,
//! which are fail-soft (a bad line is skipped so the session survives), the
//! audit timeline is fail-hard: a malformed line aborts the query. An audit
//! answer that silently omits records is worse than no answer, because an
//! operator would read the gap as "nothing happened".
//!
//! Querying scans the JSONL directly. SQLite is explicitly deferred: it can
//! only ever be an optimization here, and deleting the derived index must not
//! change a single audit result.

use viden_types::{AuditPage, AuditQuery, AuditRecord};

/// Re-validates a record against the [`AuditRecord::sanitized`] bounds.
///
/// Emission sites build records through the constructor, but a record can also
/// arrive hand-built or deserialized. Appending re-runs the same bounds so an
/// unsanitized record can never reach the durable log.
pub(crate) fn revalidate_audit_record(record: &AuditRecord) -> Result<(), String> {
    let sanitized = AuditRecord::sanitized(
        record.audit_id.clone(),
        record.timestamp,
        record.owner.clone(),
        record.actor.clone(),
        record.action.clone(),
        record.objects.clone(),
        record.outcome,
        record.args.clone(),
    )?;
    if &sanitized != record {
        return Err("audit record does not match its sanitized form".to_string());
    }
    Ok(())
}

/// Applies the query filters, newest-first ordering, and page bounds.
///
/// Ordering is `(timestamp, audit_id)` descending. The id tiebreak is what
/// makes pagination stable when several records share a timestamp: without it
/// a `before` cursor could re-emit or skip a record.
pub(crate) fn audit_page(records: Vec<AuditRecord>, query: &AuditQuery) -> AuditPage {
    let mut matching = records
        .into_iter()
        .filter(|record| matches_query(record, query))
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| {
        right
            .timestamp
            .cmp(&left.timestamp)
            .then_with(|| right.audit_id.cmp(&left.audit_id))
    });
    if let Some(before) = query.before.as_ref() {
        // Exclusive upper bound: the cursor names the last record already
        // delivered, so the next page starts strictly older than it.
        matching.retain(|record| record.cursor() < *before);
    }

    let limit = query.clamped_limit();
    let complete = matching.len() <= limit;
    matching.truncate(limit);
    let next_before = if complete {
        None
    } else {
        matching.last().map(AuditRecord::cursor)
    };
    AuditPage {
        records: matching,
        next_before,
        complete,
    }
}

fn matches_query(record: &AuditRecord, query: &AuditQuery) -> bool {
    if let Some(project_id) = query.project_id.as_deref()
        && record.owner.project_id != project_id
    {
        return false;
    }
    if let Some(lane_id) = query.lane_id.as_deref() {
        // A record belongs to a lane through its owner scope OR through an
        // explicitly linked lane object, so cross-lane facts such as a handoff
        // stay visible from both lanes.
        let owned = record.owner.lane_id.as_deref() == Some(lane_id);
        let linked = record.objects.iter().any(|object| {
            object.kind == viden_types::AuditObjectRef::KIND_LANE && object.id == lane_id
        });
        if !owned && !linked {
            return false;
        }
    }
    if let Some(object) = query.object.as_ref()
        && !record.objects.contains(object)
    {
        return false;
    }
    true
}
