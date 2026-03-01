use polodb_core::Database;
use std::path::Path;

pub mod admin;
pub mod camera;
pub mod pool_match;
pub mod settings;

use crate::error::ApiError;

/// Application database handle. PoloDB is clone-friendly for sharing across handlers.
#[derive(Clone)]
pub struct Db(pub Database);

fn is_corruption_error(e: &polodb_core::Error) -> bool {
    let s = e.to_string();
    s.contains("Corruption") || s.contains("corrupted")
}

impl Db {
    /// Open the database at the given path. Creates parent dirs and file if needed.
    /// On corruption error, removes the database directory and retries with a fresh one.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApiError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match Database::open_path(path) {
            Ok(db) => Ok(Self(db)),
            Err(e) if is_corruption_error(&e) => {
                tracing::warn!(path = %path.display(), "Database corrupted, removing and recreating: {}", e);
                let _ = std::fs::remove_dir_all(path);
                let db = Database::open_path(path)?;
                Ok(Self(db))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Open the database using `POLODB_PATH` env var, or default to `data/table-tv.db`.
    pub fn open_default() -> Result<Self, ApiError> {
        let path = std::env::var("POLODB_PATH").unwrap_or_else(|_| "data/table-tv.db".to_string());
        Self::open(path)
    }
}
