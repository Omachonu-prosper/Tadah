use super::DomainError;

pub const MAX_TITLE_LEN: usize = 100;

/// A named group of tasks, e.g. "Afara.AI Todos". An empty title is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoList {
    pub id: String,
    pub title: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

fn normalize_title(raw: &str) -> Result<String, DomainError> {
    let title = raw.trim();
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(DomainError::Validation(format!(
            "List title is over {MAX_TITLE_LEN} characters"
        )));
    }
    Ok(title.to_string())
}

impl TodoList {
    pub fn new(id: String, title: &str, position: i64, now: i64) -> Result<TodoList, DomainError> {
        Ok(TodoList {
            id,
            title: normalize_title(title)?,
            position,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn rename(&mut self, title: &str, now: i64) -> Result<(), DomainError> {
        let title = normalize_title(title)?;
        if title != self.title {
            self.title = title;
            self.updated_at = now;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_empty_title_but_not_oversized() {
        assert!(TodoList::new("l".into(), "", 0, 1).is_ok());
        assert!(TodoList::new("l".into(), &"x".repeat(MAX_TITLE_LEN + 1), 0, 1).is_err());
    }
}
