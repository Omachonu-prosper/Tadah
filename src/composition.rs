//! Composition root shared by the executables: picks the concrete adapters.

use std::path::PathBuf;

use crate::application::UseCases;
use crate::application::ports::Result;
use crate::infrastructure::sqlite::SqliteStore;
use crate::infrastructure::system::{SystemClock, UuidGenerator};

/// `$TADAH_DB`, else `<data dir>/tadah/tadah.db` (e.g. `~/.local/share/tadah/tadah.db`).
pub fn db_path() -> PathBuf {
    if let Some(p) = std::env::var_os("TADAH_DB") {
        return p.into();
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tadah")
        .join("tadah.db")
}

pub fn open_app(path: &std::path::Path) -> Result<UseCases<SqliteStore>> {
    Ok(UseCases::new(
        SqliteStore::open(path)?,
        Box::new(SystemClock),
        Box::new(UuidGenerator),
    ))
}
