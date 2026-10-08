use super::DomainError;

pub const MAX_TEXT_LEN: usize = 500;

/// Todo -> Partial -> Done -> Todo.
///
/// Todo    nothing done yet        ("- task")
/// Partial started, work remains   ("- * task")
/// Done    finished                ("- / task")
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Todo,
    Partial,
    Done,
}

impl Status {
    pub fn next(self) -> Status {
        match self {
            Status::Todo => Status::Partial,
            Status::Partial => Status::Done,
            Status::Done => Status::Todo,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Partial => "partial",
            Status::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "todo" => Some(Status::Todo),
            "partial" => Some(Status::Partial),
            "done" => Some(Status::Done),
            _ => None,
        }
    }
}

/// Times are unix seconds; the domain doesn't care where they come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub text: String,
    pub status: Status,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Set exactly while the task is done.
    pub completed_at: Option<i64>,
}

/// Trims and validates task text.
pub fn normalize_text(raw: &str) -> Result<String, DomainError> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(DomainError::Validation("Task text cannot be empty".into()));
    }
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(DomainError::Validation(format!(
            "Task text is over {MAX_TEXT_LEN} characters"
        )));
    }
    Ok(text.to_string())
}

impl Task {
    pub fn new(
        id: String,
        list_id: String,
        text: &str,
        status: Status,
        position: i64,
        now: i64,
    ) -> Result<Task, DomainError> {
        Ok(Task {
            id,
            list_id,
            text: normalize_text(text)?,
            status,
            position,
            created_at: now,
            updated_at: now,
            completed_at: (status == Status::Done).then_some(now),
        })
    }

    pub fn set_status(&mut self, status: Status, now: i64) {
        if status == self.status {
            return;
        }
        self.status = status;
        self.updated_at = now;
        self.completed_at = (status == Status::Done).then_some(now);
    }

    /// Business rule: you break a task into steps once you've started it and work remains.
    pub fn ensure_accepts_subtasks(&self) -> Result<(), DomainError> {
        if self.status == Status::Partial {
            Ok(())
        } else {
            Err(DomainError::Validation(
                "Subtasks can only be added to partly done tasks".into(),
            ))
        }
    }

    pub fn advance(&mut self, now: i64) {
        self.set_status(self.status.next(), now);
    }

    pub fn rename(&mut self, text: &str, now: i64) -> Result<(), DomainError> {
        let text = normalize_text(text)?;
        if text != self.text {
            self.text = text;
            self.updated_at = now;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(text: &str) -> Result<Task, DomainError> {
        Task::new("t".into(), "l".into(), text, Status::Todo, 0, 100)
    }

    #[test]
    fn trims_and_validates_text() {
        assert_eq!(task("  hello ").unwrap().text, "hello");
        assert!(task("   ").is_err());
        assert!(task(&"x".repeat(MAX_TEXT_LEN + 1)).is_err());
        assert!(task(&"x".repeat(MAX_TEXT_LEN)).is_ok());
    }

    #[test]
    fn cycles_todo_partial_done_todo() {
        let mut t = task("a").unwrap();
        t.advance(1);
        assert_eq!(t.status, Status::Partial);
        t.advance(2);
        assert_eq!(t.status, Status::Done);
        t.advance(3);
        assert_eq!(t.status, Status::Todo);
    }

    #[test]
    fn completed_at_is_set_exactly_while_done() {
        let mut t = task("a").unwrap();
        assert_eq!(t.completed_at, None);
        t.set_status(Status::Done, 5);
        assert_eq!(t.completed_at, Some(5));
        t.set_status(Status::Partial, 6);
        assert_eq!(t.completed_at, None);
        let done = Task::new("t".into(), "l".into(), "a", Status::Done, 0, 9).unwrap();
        assert_eq!(done.completed_at, Some(9));
    }

    #[test]
    fn only_partial_tasks_accept_subtasks() {
        let mut t = task("a").unwrap();
        assert!(t.ensure_accepts_subtasks().is_err());
        t.set_status(Status::Partial, 1);
        assert!(t.ensure_accepts_subtasks().is_ok());
        t.set_status(Status::Done, 2);
        assert!(t.ensure_accepts_subtasks().is_err());
    }

    #[test]
    fn rename_only_touches_updated_at_on_change() {
        let mut t = task("a").unwrap();
        t.rename("a", 50).unwrap();
        assert_eq!(t.updated_at, 100);
        t.rename("b", 50).unwrap();
        assert_eq!((t.text.as_str(), t.updated_at), ("b", 50));
        assert!(t.rename("", 60).is_err());
        assert_eq!(t.text, "b");
    }
}
