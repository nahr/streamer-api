use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub mod camera;
pub mod pool_match;
pub mod settings;
pub mod user;

use crate::error::ApiError;

/// Document ID type. Stored as string in SQLite.
pub type Id = String;

fn new_id() -> Id {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Application database handle. Wraps SQLite connection for sharing across handlers.
#[derive(Clone)]
pub struct Db(pub std::sync::Arc<Mutex<Connection>>);

impl Db {
    /// Execute a function with exclusive access to the database connection.
    /// The lock is always released when the closure returns (or panics).
    pub fn execute<T, F>(&self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(&Connection) -> Result<T, ApiError>,
    {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        f(&*conn)
    }
    /// Open the database at the given path. Creates parent dirs and file if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApiError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;",
        )?;
        let db = Self(std::sync::Arc::new(Mutex::new(conn)));
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), ApiError> {
        self.execute(|conn| {
            conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS cameras (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                camera_type TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pool_matches (
                id TEXT PRIMARY KEY,
                player_one TEXT NOT NULL,
                player_two TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT,
                camera_id TEXT,
                started_by_sub TEXT,
                started_by_name TEXT,
                description TEXT,
                score_history TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE IF NOT EXISTS users (
                auth0_sub TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS settings (
                id TEXT PRIMARY KEY,
                location_name TEXT NOT NULL DEFAULT ''
            );
            INSERT OR IGNORE INTO settings (id, location_name) VALUES ('system', '');
            ",
            )?;
            // Migration: add score_history to pool_matches if missing (older DBs created before this column)
            let has_score_history: bool = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('pool_matches') WHERE name='score_history'",
                [],
                |row| row.get::<_, i64>(0),
            )? > 0;
            if !has_score_history {
                conn.execute("ALTER TABLE pool_matches ADD COLUMN score_history TEXT NOT NULL DEFAULT '[]'", [])?;
            }
            Ok(())
        })
    }

    /// Open the database using `SQLITE_PATH` (or `POLODB_PATH` for compatibility) env var, or default to `data/table-tv.db`.
    pub fn open_default() -> Result<Self, ApiError> {
        let path = std::env::var("SQLITE_PATH")
            .or_else(|_| std::env::var("POLODB_PATH"))
            .unwrap_or_else(|_| "data/table-tv.db".to_string());
        Self::open(path)
    }
}
