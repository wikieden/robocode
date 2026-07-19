use viden_core::{TranscriptRow, TranscriptRowId};

/// A bounded materialized window around one stable Core transcript row id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptViewport {
    capacity: usize,
    rows: Vec<TranscriptRow>,
    anchor: Option<TranscriptRowId>,
}

impl TranscriptViewport {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            rows: Vec::new(),
            anchor: None,
        }
    }

    pub fn replace_rows(
        &mut self,
        rows: Vec<TranscriptRow>,
        preferred_anchor: Option<&TranscriptRowId>,
    ) {
        if rows.is_empty() {
            self.rows.clear();
            self.anchor = None;
            return;
        }

        let anchor_index = preferred_anchor
            .and_then(|anchor| rows.iter().position(|row| &row.id == anchor))
            .unwrap_or(rows.len() - 1);
        let end = (anchor_index + (self.capacity - self.capacity / 2)).min(rows.len());
        let start = end.saturating_sub(self.capacity);
        self.rows = rows[start..end].to_vec();
        let materialized_anchor = anchor_index.saturating_sub(start);
        self.anchor = self.rows.get(materialized_anchor).map(|row| row.id.clone());
    }

    pub fn rows(&self) -> &[TranscriptRow] {
        &self.rows
    }

    pub fn anchor(&self) -> Option<&TranscriptRowId> {
        self.anchor.as_ref()
    }

    pub fn anchor_offset(&self) -> Option<usize> {
        let anchor = self.anchor.as_ref()?;
        self.rows.iter().position(|row| &row.id == anchor)
    }
}
