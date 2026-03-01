use polodb_core::bson::doc;
use polodb_core::CollectionT;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::Db;

const SETTINGS_COLLECTION: &str = "settings";
const SETTINGS_DOC_ID: &str = "system";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SettingsDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub location_name: String,
}

impl Db {
    /// Get system settings. Returns default if none exist.
    pub fn get_settings(&self) -> Result<SettingsDoc, ApiError> {
        let collection = self.0.collection::<SettingsDoc>(SETTINGS_COLLECTION);
        let doc = collection.find_one(doc! { "_id": SETTINGS_DOC_ID })?;
        Ok(doc.unwrap_or_default())
    }

    /// Update system settings. Inserts if none exist, otherwise updates.
    pub fn set_settings(&self, settings: SettingsDoc) -> Result<(), ApiError> {
        let collection = self.0.collection::<SettingsDoc>(SETTINGS_COLLECTION);
        let existing = collection.find_one(doc! { "_id": SETTINGS_DOC_ID })?;
        if let Some(_) = existing {
            let update_doc = doc! { "$set": { "location_name": settings.location_name } };
            collection.update_many(doc! { "_id": SETTINGS_DOC_ID }, update_doc)?;
        } else {
            let doc_with_id = SettingsDoc {
                id: Some(SETTINGS_DOC_ID.to_string()),
                location_name: settings.location_name,
            };
            collection.insert_one(doc_with_id)?;
        }
        Ok(())
    }
}
