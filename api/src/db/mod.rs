use polodb_core::Database;
use std::path::Path;

pub mod admin;
pub mod camera;
pub mod pool_match;

use crate::error::ApiError;

/// Application database handle. PoloDB is clone-friendly for sharing across handlers.
#[derive(Clone)]
pub struct Db(pub Database);

impl Db {
    /// Open the database at the given path. Creates parent dirs and file if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApiError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::open_path(path)?;
        Ok(Self(db))
    }

    /// Open the database using `POLODB_PATH` env var, or default to `data/table-tv.db`.
    pub fn open_default() -> Result<Self, ApiError> {
        let path = std::env::var("POLODB_PATH").unwrap_or_else(|_| "data/table-tv.db".to_string());
        Self::open(path)
    }
}
