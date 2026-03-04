use serde::{Deserialize, Serialize};

use super::Db;
use crate::error::ApiError;

const SETTINGS_DOC_ID: &str = "system";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SettingsDoc {
    pub id: Option<String>,
    #[serde(default)]
    pub location_name: String,
    /// MediaMTX recording path (e.g. /app/data/recordings or ./recordings). Empty = default.
    #[serde(default)]
    pub record_path: String,
    /// Segment duration (e.g. 30m, 1h). Rolling storage segment length.
    #[serde(default)]
    pub record_segment_duration: String,
    /// Delete recordings after (e.g. 24h, 7d). 0 or empty = keep forever.
    #[serde(default)]
    pub record_delete_after: String,
}

impl Db {
    /// Get system settings. Returns default if none exist.
    pub fn get_settings(&self) -> Result<SettingsDoc, ApiError> {
        tracing::trace!("getting settings lock");
        self.execute(|conn| {
            tracing::trace!("settings lock acquired");
            let mut stmt = conn.prepare(
                "SELECT id, location_name, record_path, record_segment_duration, record_delete_after FROM settings WHERE id = ?1",
            )?;
            let mut rows = stmt.query([SETTINGS_DOC_ID])?;
            if let Some(row) = rows.next()? {
                Ok(SettingsDoc {
                    id: Some(row.get(0)?),
                    location_name: row.get(1)?,
                    record_path: row.get(2).unwrap_or_default(),
                    record_segment_duration: row.get(3).unwrap_or_else(|_| "1m".to_string()),
                    record_delete_after: row.get(4).unwrap_or_else(|_| "24h".to_string()),
                })
            } else {
                Ok(SettingsDoc::default())
            }
        })
    }

    /// Update system settings. Inserts if none exist, otherwise updates.
    pub fn set_settings(&self, settings: SettingsDoc) -> Result<(), ApiError> {
        self.execute(|conn| {
            conn.execute(
                "INSERT INTO settings (id, location_name, record_path, record_segment_duration, record_delete_after) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO UPDATE SET location_name = ?2, record_path = ?3, record_segment_duration = ?4, record_delete_after = ?5",
                rusqlite::params![
                    SETTINGS_DOC_ID,
                    settings.location_name,
                    settings.record_path,
                    settings.record_segment_duration,
                    settings.record_delete_after,
                ],
            )?;
            Ok(())
        })
    }
}
