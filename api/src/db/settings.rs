use serde::{Deserialize, Serialize};

use super::Db;
use crate::error::ApiError;

const SETTINGS_DOC_ID: &str = "system";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SettingsDoc {
    pub id: Option<String>,
    #[serde(default)]
    pub location_name: String,
}

impl Db {
    /// Get system settings. Returns default if none exist.
    pub fn get_settings(&self) -> Result<SettingsDoc, ApiError> {
        tracing::debug!("getting settings lock");
        self.execute(|conn| {
            tracing::debug!("settings lock acquired");
            let mut stmt = conn.prepare("SELECT id, location_name FROM settings WHERE id = ?1")?;
            let mut rows = stmt.query([SETTINGS_DOC_ID])?;
            if let Some(row) = rows.next()? {
                Ok(SettingsDoc {
                    id: Some(row.get(0)?),
                    location_name: row.get(1)?,
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
                "INSERT INTO settings (id, location_name) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET location_name = ?2",
                rusqlite::params![SETTINGS_DOC_ID, settings.location_name],
            )?;
            Ok(())
        })
    }
}
