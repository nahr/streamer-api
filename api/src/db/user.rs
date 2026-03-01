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
        self.execute(|conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM users WHERE is_admin = 1 LIMIT 1")?;
            let mut rows = stmt.query([])?;
            Ok(rows.next()?.is_some())
        })
    }

    /// Find user by Auth0 sub (subject) claim.
    pub fn find_user_by_sub(&self, sub: &str) -> Result<Option<UserDoc>, ApiError> {
        self.execute(|conn| {
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
        })
    }

    /// List all users.
    pub fn list_users(&self) -> Result<Vec<UserDoc>, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT auth0_sub, email, is_admin FROM users ORDER BY email")?;
        let rows = stmt.query_map([], |row| {
            Ok(UserDoc {
                auth0_sub: row.get(0)?,
                email: row.get(1)?,
                is_admin: row.get::<_, i64>(2)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(ApiError::from)
    }

    /// Set admin status for a user. Fails if trying to remove the last admin.
    pub fn set_user_admin(&self, auth0_sub: &str, is_admin: bool) -> Result<UserDoc, ApiError> {
        let existing = self
            .find_user_by_sub(auth0_sub)?
            .ok_or_else(|| ApiError::BadRequest("User not found".to_string()))?;

        self.execute(|conn| {
            if !is_admin && existing.is_admin {
                let admin_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM users WHERE is_admin = 1",
                    [],
                    |row| row.get(0),
                )?;
                if admin_count <= 1 {
                    return Err(ApiError::BadRequest(
                        "Cannot remove the last admin. Promote another user first.".to_string(),
                    ));
                }
            }

            conn.execute(
                "UPDATE users SET is_admin = ?1 WHERE auth0_sub = ?2",
                rusqlite::params![is_admin as i64, auth0_sub],
            )?;
            Ok(UserDoc {
                auth0_sub: existing.auth0_sub,
                email: existing.email,
                is_admin,
            })
        })
    }

    /// Create or update a user. First user becomes admin.
    pub fn upsert_user(&self, auth0_sub: String, email: String) -> Result<UserDoc, ApiError> {
        if let Some(existing) = self.find_user_by_sub(&auth0_sub)? {
            return Ok(existing);
        }

        let is_admin = !self.has_admin()?;
        self.execute(|conn| {
            conn.execute(
                "INSERT INTO users (auth0_sub, email, is_admin) VALUES (?1, ?2, ?3)",
                rusqlite::params![auth0_sub, email, is_admin as i64],
            )?;
            Ok(UserDoc {
                auth0_sub,
                email,
                is_admin,
            })
        })
    }
}
