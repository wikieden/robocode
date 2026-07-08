use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
    Archived,
}

impl TaskStatus {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "todo" => Some(Self::Todo),
            "in_progress" | "in-progress" | "inprogress" => Some(Self::InProgress),
            "blocked" => Some(Self::Blocked),
            "done" => Some(Self::Done),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }
}

impl Display for TaskStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskPriority {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl Display for TaskPriority {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Project,
    Session,
}

impl MemoryScope {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "project" => Some(Self::Project),
            "session" => Some(Self::Session),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Session => "session",
        }
    }
}

impl Display for MemoryScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Constraint,
    Decision,
    Convention,
}

impl MemoryKind {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "fact" => Some(Self::Fact),
            "preference" => Some(Self::Preference),
            "constraint" => Some(Self::Constraint),
            "decision" => Some(Self::Decision),
            "convention" => Some(Self::Convention),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Constraint => "constraint",
            Self::Decision => "decision",
            Self::Convention => "convention",
        }
    }
}

impl Display for MemoryKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    User,
    AssistantSuggestion,
    Command,
    Imported,
}

impl MemorySource {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "user" => Some(Self::User),
            "assistant_suggestion" | "assistant-suggestion" => Some(Self::AssistantSuggestion),
            "command" => Some(Self::Command),
            "imported" => Some(Self::Imported),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::AssistantSuggestion => "assistant_suggestion",
            Self::Command => "command",
            Self::Imported => "imported",
        }
    }
}

impl Display for MemorySource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Suggested,
    Active,
    Superseded,
    Pruned,
    Rejected,
}

impl MemoryStatus {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "suggested" => Some(Self::Suggested),
            "active" => Some(Self::Active),
            "superseded" => Some(Self::Superseded),
            "pruned" => Some(Self::Pruned),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Pruned => "pruned",
            Self::Rejected => "rejected",
        }
    }
}

impl Display for MemoryStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: crate::TaskId,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub labels: Vec<String>,
    pub assignee_hint: Option<String>,
    pub parent_task_id: Option<crate::TaskId>,
    pub dependency_ids: Vec<crate::TaskId>,
    pub blocked_by: Option<String>,
    pub notes: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_session_id: Option<crate::SessionId>,
    pub last_seen_at: Option<u64>,
    pub archived_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub memory_id: crate::MemoryId,
    pub scope: MemoryScope,
    pub session_id: Option<crate::SessionId>,
    pub kind: MemoryKind,
    pub content: String,
    pub source: MemorySource,
    pub status: MemoryStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub related_task_ids: Vec<crate::TaskId>,
    pub confidence_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeContextSnapshot {
    pub active_tasks: Vec<TaskRecord>,
    pub blocked_tasks: Vec<TaskRecord>,
    pub recently_completed_tasks: Vec<TaskRecord>,
    pub relevant_project_memory: Vec<MemoryEntry>,
    pub recent_session_memory: Vec<MemoryEntry>,
    pub suggested_next_steps: Vec<String>,
    pub suggested_session_memory: Vec<String>,
}
