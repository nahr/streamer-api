use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::{Db, Id, new_id};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CameraType {
    Rtsp { url: String },
    Internal,
    Usb { device: String },
}

impl CameraType {
    pub fn is_internal(&self) -> bool {
        matches!(self, CameraType::Internal)
    }

    pub fn is_rtsp(&self) -> bool {
        matches!(self, CameraType::Rtsp { .. })
    }

    pub fn rtsp_url(&self) -> Option<&str> {
        match self {
            CameraType::Rtsp { url } => Some(url.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CameraDoc {
    pub id: Option<Id>,
    pub name: String,
    pub camera_type: CameraType,
}

impl Db {
    /// List all cameras.
    pub fn list_cameras(&self) -> Result<Vec<CameraDoc>, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, name, camera_type FROM cameras")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let camera_type: String = row.get(2)?;
            let camera_type: CameraType = serde_json::from_str(&camera_type)
                .map_err(|e| rusqlite::Error::InvalidParameterName(format!("JSON: {}", e)))?;
            Ok(CameraDoc {
                id: Some(id),
                name,
                camera_type,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Find camera by id.
    pub fn find_camera_by_id(&self, id: &str) -> Result<Option<CameraDoc>, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, name, camera_type FROM cameras WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let camera_type: String = row.get(2)?;
            let camera_type: CameraType =
                serde_json::from_str(&camera_type).map_err(|e| ApiError::Unknown(e.to_string()))?;
            Ok(Some(CameraDoc {
                id: Some(id),
                name,
                camera_type,
            }))
        } else {
            Ok(None)
        }
    }

    /// Find camera by name.
    pub fn find_camera_by_name(&self, name: &str) -> Result<Option<CameraDoc>, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT id, name, camera_type FROM cameras WHERE name = ?1")?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let camera_type: String = row.get(2)?;
            let camera_type: CameraType =
                serde_json::from_str(&camera_type).map_err(|e| ApiError::Unknown(e.to_string()))?;
            Ok(Some(CameraDoc {
                id: Some(id),
                name,
                camera_type,
            }))
        } else {
            Ok(None)
        }
    }

    /// Find the internal camera, if one exists.
    pub fn find_internal_camera(&self) -> Result<Option<CameraDoc>, ApiError> {
        let internal_json = serde_json::to_string(&CameraType::Internal)
            .map_err(|e| ApiError::Unknown(e.to_string()))?;
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let mut stmt =
            conn.prepare("SELECT id, name, camera_type FROM cameras WHERE camera_type = ?1")?;
        let mut rows = stmt.query([internal_json])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let camera_type: String = row.get(2)?;
            let camera_type: CameraType =
                serde_json::from_str(&camera_type).map_err(|e| ApiError::Unknown(e.to_string()))?;
            Ok(Some(CameraDoc {
                id: Some(id),
                name,
                camera_type,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create a new camera. Fails if creating Internal and one already exists.
    pub fn create_camera(&self, name: String, camera_type: CameraType) -> Result<Id, ApiError> {
        if camera_type.is_internal() {
            if self.find_internal_camera()?.is_some() {
                return Err(ApiError::InternalCameraExists);
            }
        }
        let id = new_id();
        let camera_type_json =
            serde_json::to_string(&camera_type).map_err(|e| ApiError::Unknown(e.to_string()))?;
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        conn.execute("INSERT INTO cameras (id, name, camera_type) VALUES (?1, ?2, ?3)", [
            &id,
            &name,
            &camera_type_json,
        ])?;
        Ok(id)
    }

    /// Update a camera. Fails if updating to Internal and another Internal already exists.
    pub fn update_camera(
        &self,
        id: &str,
        name: String,
        camera_type: CameraType,
    ) -> Result<(), ApiError> {
        if camera_type.is_internal() {
            let current = self.find_camera_by_id(id)?;
            let was_already_internal = current
                .as_ref()
                .map_or(false, |c| c.camera_type.is_internal());
            if !was_already_internal {
                if self.find_internal_camera()?.is_some() {
                    return Err(ApiError::InternalCameraExists);
                }
            }
        }
        let camera_type_json =
            serde_json::to_string(&camera_type).map_err(|e| ApiError::Unknown(e.to_string()))?;
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let changed = conn.execute(
            "UPDATE cameras SET name = ?1, camera_type = ?2 WHERE id = ?3",
            rusqlite::params![name, camera_type_json, id],
        )?;
        if changed == 0 {
            return Err(ApiError::CameraNotFound);
        }
        Ok(())
    }

    /// Delete a camera.
    pub fn delete_camera(&self, id: &str) -> Result<bool, ApiError> {
        let conn = self.0.lock().map_err(|e| ApiError::Unknown(e.to_string()))?;
        let changed = conn.execute("DELETE FROM cameras WHERE id = ?1", [id])?;
        Ok(changed > 0)
    }
}
