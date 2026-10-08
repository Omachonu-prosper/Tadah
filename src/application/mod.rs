//! Application rules: use cases and the ports they need. Depends only on `domain`.

mod error;
pub mod notes;
pub mod ports;
mod use_cases;
mod views;

pub use error::AppError;
pub use use_cases::UseCases;
pub use views::{Board, CompletedItem, CompletedKind, ListView, SubtaskView, TaskView};
