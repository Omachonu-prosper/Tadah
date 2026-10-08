//! Enterprise rules. No dependencies on anything else in the crate.

mod error;
mod list;
mod subtask;
mod task;

pub use error::DomainError;
pub use list::{TodoList, MAX_TITLE_LEN};
pub use subtask::Subtask;
pub use task::{normalize_text, Status, Task, MAX_TEXT_LEN};
