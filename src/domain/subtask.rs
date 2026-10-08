use super::{DomainError, normalize_text};

/// A checkbox item under a task, e.g. one remaining step of a partly done task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtask {
    pub id: String,
    pub task_id: String,
    pub text: String,
    pub done: bool,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Set exactly while the subtask is done.
    pub completed_at: Option<i64>,
}

impl Subtask {
    pub fn new(
        id: String,
        task_id: String,
        text: &str,
        done: bool,
        position: i64,
        now: i64,
    ) -> Result<Subtask, DomainError> {
        Ok(Subtask {
            id,
            task_id,
            text: normalize_text(text)?,
            done,
            position,
            created_at: now,
            updated_at: now,
            completed_at: done.then_some(now),
        })
    }

    pub fn set_done(&mut self, done: bool, now: i64) {
        if done == self.done {
            return;
        }
        self.done = done;
        self.updated_at = now;
        self.completed_at = done.then_some(now);
    }

    pub fn toggle(&mut self, now: i64) {
        self.set_done(!self.done, now);
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

    #[test]
    fn validates_text_and_toggles() {
        assert!(Subtask::new("s".into(), "t".into(), " ", false, 0, 1).is_err());
        let mut s = Subtask::new("s".into(), "t".into(), " step ", false, 0, 1).unwrap();
        assert_eq!(s.text, "step");
        s.toggle(2);
        assert!(s.done);
        assert_eq!(s.updated_at, 2);
        s.toggle(3);
        assert!(!s.done);
    }

    #[test]
    fn completed_at_is_set_exactly_while_done() {
        let mut s = Subtask::new("s".into(), "t".into(), "step", false, 0, 1).unwrap();
        assert_eq!(s.completed_at, None);
        s.set_done(true, 5);
        assert_eq!(s.completed_at, Some(5));
        s.set_done(true, 9); // no change: keeps the original completion time
        assert_eq!(s.completed_at, Some(5));
        s.set_done(false, 10);
        assert_eq!(s.completed_at, None);
        let done = Subtask::new("s".into(), "t".into(), "step", true, 0, 7).unwrap();
        assert_eq!(done.completed_at, Some(7));
    }
}
