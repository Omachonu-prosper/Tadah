use crate::domain::Status;

// Response models handed to the UI. Entities don't cross the boundary outward.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtaskView {
    pub id: String,
    pub text: String,
    pub done: bool,
    /// Unix seconds.
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub id: String,
    pub text: String,
    pub status: Status,
    /// Unix seconds.
    pub created_at: i64,
    pub subtasks: Vec<SubtaskView>,
}

impl TaskView {
    /// Whether the UI should offer "add subtask" (mirrors the domain rule).
    pub fn accepts_subtasks(&self) -> bool {
        self.status == Status::Partial
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListView {
    pub id: String,
    pub title: String,
    pub tasks: Vec<TaskView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletedKind {
    Task,
    Subtask,
}

/// One finished task or subtask, with when it was finished (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedItem {
    pub id: String,
    pub kind: CompletedKind,
    pub text: String,
    /// Title of the section it lives in (may be empty).
    pub section: String,
    /// For subtasks: the task they belong to.
    pub parent_task: Option<String>,
    pub completed_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Board {
    pub lists: Vec<ListView>,
}

impl Board {
    pub fn count(&self, status: Status) -> usize {
        self.lists
            .iter()
            .flat_map(|l| &l.tasks)
            .filter(|t| t.status == status)
            .count()
    }
}
