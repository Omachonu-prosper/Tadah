//! Tadah core. Dependencies point inward: `infrastructure` -> `application` -> `domain`.
//! `cli` and `composition` sit outside, alongside the GUI in the binary.

pub mod application;
pub mod cli;
pub mod composition;
pub mod domain;
pub mod infrastructure;
pub mod timefmt;
