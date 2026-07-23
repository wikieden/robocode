use std::collections::VecDeque;
use std::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptRow {
    pub id: String,
    pub kind: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptViewport {
    anchor: Option<String>,
    capacity: usize,
    follow_latest: bool,
    new_output_count: usize,
    rows: VecDeque<TranscriptRow>,
}

impl Default for TranscriptViewport {
    fn default() -> Self {
        Self {
            anchor: None,
            capacity: 240,
            follow_latest: true,
            new_output_count: 0,
            rows: VecDeque::new(),
        }
    }
}

impl TranscriptViewport {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            ..Self::default()
        }
    }

    pub fn append(&mut self, row: TranscriptRow) {
        let id = row.id.clone();
        self.rows.push_back(row);
        while self.rows.len() > self.capacity {
            self.rows.pop_front();
        }
        if self.follow_latest {
            self.anchor = Some(id);
            self.new_output_count = 0;
        } else {
            self.new_output_count = self.new_output_count.saturating_add(1);
        }
    }

    pub fn set_follow_latest(&mut self, follow_latest: bool, anchor: Option<String>) {
        self.follow_latest = follow_latest;
        if follow_latest {
            self.anchor = self.rows.back().map(|row| row.id.clone());
            self.new_output_count = 0;
        } else if let Some(anchor) = anchor {
            self.anchor = Some(anchor);
        }
    }

    pub fn rows(&self) -> &VecDeque<TranscriptRow> {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    pub fn follow_latest(&self) -> bool {
        self.follow_latest
    }

    pub fn new_output_count(&self) -> usize {
        self.new_output_count
    }

    pub fn visible_range(&self, viewport_height: usize, row_height: usize) -> Range<usize> {
        let visible = viewport_height
            .div_ceil(row_height.max(1))
            .saturating_add(2)
            .min(self.rows.len());
        let end = if self.follow_latest {
            self.rows.len()
        } else {
            self.anchor
                .as_deref()
                .and_then(|anchor| self.rows.iter().position(|row| row.id == anchor))
                .map_or(self.rows.len(), |index| index + 1)
        };
        end.saturating_sub(visible)..end
    }
}
