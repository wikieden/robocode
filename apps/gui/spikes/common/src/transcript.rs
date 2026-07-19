use std::collections::VecDeque;

use viden_core::{TranscriptPage, TranscriptPageRequest, TranscriptRow, TranscriptRowId};

const MAX_NAVIGATION_HISTORY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageLocation {
    request: TranscriptPageRequest,
    anchor: Option<TranscriptRowId>,
}

/// A bounded materialized window backed exclusively by Core transcript pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptViewport {
    capacity: usize,
    rows: Vec<TranscriptRow>,
    anchor: Option<TranscriptRowId>,
    current_request: Option<TranscriptPageRequest>,
    older_request: Option<TranscriptPageRequest>,
    // Core pages backward with `before`; retain only bounded request/anchor
    // breadcrumbs so forward navigation never caches transcript row pages.
    newer_history: VecDeque<PageLocation>,
}

impl TranscriptViewport {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, 500),
            rows: Vec::new(),
            anchor: None,
            current_request: None,
            older_request: None,
            newer_history: VecDeque::new(),
        }
    }

    pub(crate) fn bounded_request(
        &self,
        mut request: TranscriptPageRequest,
    ) -> TranscriptPageRequest {
        request.limit = request.limit.clamp(1, self.capacity as u16);
        request
    }

    pub(crate) fn open_page(
        &mut self,
        request: TranscriptPageRequest,
        page: TranscriptPage,
        preferred_anchor: Option<&TranscriptRowId>,
    ) {
        self.newer_history.clear();
        self.current_request = Some(request.clone());
        self.older_request = page.older.clone().map(|before| TranscriptPageRequest {
            session_id: request.session_id,
            before: Some(before),
            limit: request.limit,
        });
        self.materialize(page.rows, preferred_anchor);
    }

    pub(crate) fn older_request(&self) -> Option<TranscriptPageRequest> {
        self.older_request.clone()
    }

    pub(crate) fn commit_older_page(
        &mut self,
        request: TranscriptPageRequest,
        page: TranscriptPage,
    ) {
        if let Some(current_request) = self.current_request.clone() {
            if self.newer_history.len() == MAX_NAVIGATION_HISTORY {
                self.newer_history.pop_front();
            }
            self.newer_history.push_back(PageLocation {
                request: current_request,
                anchor: self.anchor.clone(),
            });
        }
        self.current_request = Some(request.clone());
        self.older_request = page.older.clone().map(|before| TranscriptPageRequest {
            session_id: request.session_id,
            before: Some(before),
            limit: request.limit,
        });
        self.materialize(page.rows, None);
    }

    pub(crate) fn newer_request(&self) -> Option<TranscriptPageRequest> {
        self.newer_history
            .back()
            .map(|location| location.request.clone())
    }

    pub(crate) fn commit_newer_page(&mut self, page: TranscriptPage) {
        let Some(location) = self.newer_history.pop_back() else {
            return;
        };
        let request = location.request;
        self.current_request = Some(request.clone());
        self.older_request = page.older.clone().map(|before| TranscriptPageRequest {
            session_id: request.session_id,
            before: Some(before),
            limit: request.limit,
        });
        self.materialize(page.rows, location.anchor.as_ref());
    }

    fn materialize(
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
