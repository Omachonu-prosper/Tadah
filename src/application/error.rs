use std::fmt;

use crate::domain::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Validation(String),
    NotFound(String),
    /// The storage adapter failed; the message is for display only.
    Storage(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(m) | AppError::Storage(m) => f.write_str(m),
            AppError::NotFound(what) => write!(f, "{what} not found"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::Validation(m) => AppError::Validation(m),
        }
    }
}
