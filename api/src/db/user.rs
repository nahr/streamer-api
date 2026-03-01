use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::Db;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserDoc {
    pub auth0_sub: String,
    pub email: String,
    pub is_admin: bool,
}

impl Db {
    /// Returns true if at least one admin exists.
    pub fn has_admin(&self) -> Result<bool, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT 1 FROM users WHERE is_admin = 1 LIMIT 1")?;
        let mut rows = stmt.query([])?;
        Ok(rows.next()?.is_some())
    }

    /// Find user by Auth0 sub (subject) claim.
    pub fn find_user_by_sub(&self, sub: &str) -> Result<Option<UserDoc>, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT auth0_sub, email, is_admin FROM users WHERE auth0_sub = ?1")?;
        let mut rows = stmt.query([sub])?;
        if let Some(row) = rows.next()? {
            Ok(Some(UserDoc {
                auth0_sub: row.get(0)?,
                email: row.get(1)?,
                is_admin: row.get::<_, i64>(2)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create or update a user. First user becomes admin.
    pub fn upsert_user(&self, auth0_sub: String, email: String) -> Result<UserDoc, ApiError> {
        if let Some(existing) = self.find_user_by_sub(&auth0_sub)? {
            return Ok(existing);
        }

        let is_admin = !self.has_admin()?;
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (auth0_sub, email, is_admin) VALUES (?1, ?2, ?3)",
            rusqlite::params![auth0_sub, email, is_admin as i64],
        )?;
        Ok(UserDoc {
            auth0_sub,
            email,
            is_admin,
        })
    }
}
