use viden_core::RuntimeViewState;

/// Stable selector groups. The index only projects typed Core facts; it never
/// discovers lanes, sessions, gates, or files from the local workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum JumpKind {
    Gate,
    Ask,
    Lane,
    Session,
    Command,
    File,
}

impl JumpKind {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Gate => "GATES",
            Self::Ask => "ASKS",
            Self::Lane => "LANES",
            Self::Session => "SESSIONS",
            Self::Command => "COMMANDS",
            Self::File => "FILES",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JumpItem {
    pub(super) kind: JumpKind,
    pub(super) id: String,
    pub(super) title: String,
    pub(super) context: String,
    pub(super) keywords: String,
    pub(super) parent_id: Option<String>,
    pub(super) enabled: bool,
    pub(super) disabled_reason: Option<String>,
}

impl JumpItem {
    #[cfg(test)]
    fn enabled(
        kind: JumpKind,
        id: impl Into<String>,
        title: impl Into<String>,
        context: impl Into<String>,
        keywords: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            title: title.into(),
            context: context.into(),
            keywords: keywords.into(),
            parent_id: None,
            enabled: true,
            disabled_reason: None,
        }
    }

    fn disabled(
        kind: JumpKind,
        id: impl Into<String>,
        title: impl Into<String>,
        context: impl Into<String>,
        keywords: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            title: title.into(),
            context: context.into(),
            keywords: keywords.into(),
            parent_id: None,
            enabled: false,
            disabled_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JumpQuery {
    text: String,
    kinds: Option<Vec<JumpKind>>,
}

impl JumpQuery {
    pub(super) fn parse(value: &str) -> Self {
        let (kinds, text) = match value.chars().next() {
            Some(':') => (Some(vec![JumpKind::Lane]), &value[1..]),
            Some('@') => (Some(vec![JumpKind::Session]), &value[1..]),
            Some('#') => (Some(vec![JumpKind::Gate, JumpKind::Ask]), &value[1..]),
            Some('>') => (Some(vec![JumpKind::Command]), &value[1..]),
            Some('~') => (Some(vec![JumpKind::File]), &value[1..]),
            _ => (None, value),
        };
        Self {
            text: text.trim().to_string(),
            kinds,
        }
    }

    #[cfg(test)]
    pub(super) fn kinds(&self) -> &[JumpKind] {
        self.kinds.as_deref().unwrap_or(&[])
    }

    fn accepts(&self, kind: JumpKind) -> bool {
        self.kinds
            .as_ref()
            .is_none_or(|kinds| kinds.contains(&kind))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JumpIndex {
    items: Vec<JumpItem>,
}

impl JumpIndex {
    pub(super) fn from_view(view: &RuntimeViewState) -> Self {
        let mut items = Vec::new();
        items.extend(view.merge_gates.iter().map(|gate| JumpItem {
            kind: JumpKind::Gate,
            id: gate.gate_id.to_string(),
            title: format!("Merge gate {}", gate.gate_id),
            context: format!("{} · {:?}", gate.task_id, gate.status),
            keywords: gate.required_evidence.join(" "),
            parent_id: None,
            enabled: true,
            disabled_reason: None,
        }));
        items.extend(view.pending_approvals.iter().map(|approval| JumpItem {
            kind: JumpKind::Ask,
            id: approval.id.clone(),
            title: approval.title.clone(),
            context: approval.tool_name.clone(),
            keywords: format!(
                "{} {} {}",
                approval.message,
                approval.input_preview,
                approval.reason.as_deref().unwrap_or_default()
            ),
            parent_id: None,
            enabled: true,
            disabled_reason: None,
        }));
        items.extend(view.lanes.iter().map(|lane| JumpItem {
            kind: JumpKind::Lane,
            id: lane.id.clone(),
            title: lane.id.clone(),
            context: format!("{} · {:?}", lane.role, lane.status),
            keywords: format!("{} {}", lane.summary, lane.evidence.join(" ")),
            parent_id: None,
            enabled: true,
            disabled_reason: None,
        }));
        items.extend(view.lanes.iter().flat_map(|lane| {
            lane.active_session_ids
                .iter()
                .map(move |session_id| JumpItem {
                    kind: JumpKind::Session,
                    id: session_id.clone(),
                    title: session_id.clone(),
                    context: lane.id.clone(),
                    keywords: format!("{} {}", lane.role, lane.summary),
                    parent_id: Some(lane.id.clone()),
                    enabled: true,
                    disabled_reason: None,
                })
        }));
        items.extend(
            super::command_palette::command_registry()
                .iter()
                .map(|command| JumpItem {
                    kind: JumpKind::Command,
                    id: command.command.to_string(),
                    title: command.command.to_string(),
                    context: command.summary.to_string(),
                    keywords: command.keywords.to_string(),
                    parent_id: None,
                    enabled: true,
                    disabled_reason: None,
                }),
        );
        // Core has no typed file inventory capability at frontend-contract-v1.
        // Keep this row visible so operators get an actionable contract reason.
        items.push(JumpItem::disabled(
            JumpKind::File,
            "core-file-inventory-unavailable",
            "Files unavailable",
            "Core capability",
            "file inventory",
            "Core file inventory is unavailable.",
        ));
        Self::new(items)
    }

    #[cfg(test)]
    fn from_items(items: Vec<JumpItem>) -> Self {
        Self::new(items)
    }

    fn new(mut items: Vec<JumpItem>) -> Self {
        items.sort_by_key(|item| item.kind);
        Self { items }
    }

    #[cfg(test)]
    pub(super) fn items(&self) -> &[JumpItem] {
        &self.items
    }

    pub(super) fn search(&self, filter: &str) -> Vec<&JumpItem> {
        let query = JumpQuery::parse(filter);
        self.items
            .iter()
            .filter(|item| query.accepts(item.kind))
            .filter(|item| {
                query.text.is_empty()
                    || fuzzy_subsequence_score(&query.text, &item.title).is_some()
                    || fuzzy_subsequence_score(&query.text, &item.context).is_some()
                    || fuzzy_subsequence_score(&query.text, &item.keywords).is_some()
            })
            .collect()
    }
}

/// Returns a subsequence match score. Callers currently use only presence or
/// absence, preserving stable source and group ordering rather than ranking.
pub(super) fn fuzzy_subsequence_score(query: &str, candidate: &str) -> Option<usize> {
    let mut query = query.chars().flat_map(char::to_lowercase);
    let mut wanted = query.next()?;
    let mut score = 0;
    let mut previous = None;
    for (index, character) in candidate.chars().flat_map(char::to_lowercase).enumerate() {
        if character == wanted {
            score += index;
            if previous == Some(index.saturating_sub(1)) {
                score = score.saturating_sub(2);
            }
            previous = Some(index);
            match query.next() {
                Some(next) => wanted = next,
                None => return Some(score),
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::TuiState;

    #[test]
    fn index_groups_typed_runtime_facts_in_stable_order_and_keeps_file_unavailable() {
        let state = TuiState::default();
        let index = JumpIndex::from_view(&state.runtime);

        assert_eq!(
            index
                .items()
                .iter()
                .map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::Command,
                JumpKind::File,
            ],
        );
        let file = index
            .items()
            .iter()
            .find(|item| item.kind == JumpKind::File)
            .expect("file capability row");
        assert!(!file.enabled);
        assert_eq!(
            file.disabled_reason.as_deref(),
            Some("Core file inventory is unavailable."),
        );
    }

    #[test]
    fn group_order_is_gate_ask_lane_session_command_file() {
        let index = JumpIndex::from_items(vec![
            JumpItem::enabled(JumpKind::File, "file", "File", "", ""),
            JumpItem::enabled(JumpKind::Command, "command", "Command", "", ""),
            JumpItem::enabled(JumpKind::Session, "session", "Session", "", ""),
            JumpItem::enabled(JumpKind::Lane, "lane", "Lane", "", ""),
            JumpItem::enabled(JumpKind::Ask, "ask", "Ask", "", ""),
            JumpItem::enabled(JumpKind::Gate, "gate", "Gate", "", ""),
        ]);

        assert_eq!(
            index
                .items()
                .iter()
                .map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                JumpKind::Gate,
                JumpKind::Ask,
                JumpKind::Lane,
                JumpKind::Session,
                JumpKind::Command,
                JumpKind::File,
            ],
        );
    }

    #[test]
    fn fuzzy_matching_checks_title_context_and_keywords() {
        let index = JumpIndex::from_items(vec![
            JumpItem::enabled(JumpKind::Lane, "lane-1", "Compiler", "Build lane", "rust"),
            JumpItem::enabled(
                JumpKind::Session,
                "session-1",
                "Notes",
                "Review context",
                "audit",
            ),
        ]);

        assert_eq!(index.search("cmplr").len(), 1);
        assert_eq!(index.search("rvw").len(), 1);
        assert_eq!(index.search("adt").len(), 1);
    }

    #[test]
    fn query_sigils_scope_every_supported_kind() {
        assert_eq!(JumpQuery::parse(":lane").kinds(), &[JumpKind::Lane]);
        assert_eq!(JumpQuery::parse("@session").kinds(), &[JumpKind::Session]);
        assert_eq!(
            JumpQuery::parse("#gate").kinds(),
            &[JumpKind::Gate, JumpKind::Ask]
        );
        assert_eq!(JumpQuery::parse(">help").kinds(), &[JumpKind::Command]);
        assert_eq!(JumpQuery::parse("~src").kinds(), &[JumpKind::File]);
    }

    #[test]
    fn empty_and_no_match_queries_are_explicit() {
        let index = JumpIndex::from_items(vec![JumpItem::enabled(
            JumpKind::Lane,
            "lane-1",
            "Compiler",
            "Build lane",
            "rust",
        )]);

        assert_eq!(index.search("").len(), 1);
        assert!(index.search("missing").is_empty());
    }

    #[test]
    fn fuzzy_subsequence_returns_no_score_when_characters_are_missing() {
        assert!(fuzzy_subsequence_score("cmp", "Compiler").is_some());
        assert!(fuzzy_subsequence_score("xyz", "Compiler").is_none());
    }
}
