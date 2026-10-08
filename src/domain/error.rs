use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Validation(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Validation(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for DomainError {}
