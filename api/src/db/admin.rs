use polodb_core::bson::doc;
use polodb_core::CollectionT;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::Db;

const ADMIN_COLLECTION: &str = "admins";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdminDoc {
    pub email: String,
    pub hash: String,
}

impl Db {
    /// Returns true if at least one admin exists in the database.
    pub fn has_admin(&self) -> Result<bool, ApiError> {
        let collection = self.0.collection::<AdminDoc>(ADMIN_COLLECTION);
        let admin = collection.find_one(doc! {})?;
        Ok(admin.is_some())
    }

    /// Find admin by email.
    pub fn find_admin_by_email(&self, email: &str) -> Result<Option<AdminDoc>, ApiError> {
        let collection = self.0.collection::<AdminDoc>(ADMIN_COLLECTION);
        let admin = collection.find_one(doc! { "email": email })?;
        Ok(admin)
    }

    /// Create a new admin. Fails if any admin already exists.
    pub fn create_admin(&self, email: String, hash: String) -> Result<(), ApiError> {
        if self.has_admin()? {
            return Err(ApiError::AdminExists);
        }
        let collection = self.0.collection::<AdminDoc>(ADMIN_COLLECTION);
        collection.insert_one(AdminDoc { email, hash })?;
        Ok(())
    }
}
