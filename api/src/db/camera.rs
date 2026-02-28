use polodb_core::bson::{doc, oid::ObjectId};
use polodb_core::CollectionT;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use super::Db;

const CAMERAS_COLLECTION: &str = "cameras";

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CameraDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub camera_type: CameraType,
}

impl Db {
    /// List all cameras.
    pub fn list_cameras(&self) -> Result<Vec<CameraDoc>, ApiError> {
        let collection = self.0.collection::<CameraDoc>(CAMERAS_COLLECTION);
        let cursor = collection.find(doc! {}).run()?;
        Ok(cursor.collect::<Result<Vec<_>, polodb_core::Error>>()?)
    }

    /// Find camera by id.
    pub fn find_camera_by_id(&self, id: &ObjectId) -> Result<Option<CameraDoc>, ApiError> {
        let collection = self.0.collection::<CameraDoc>(CAMERAS_COLLECTION);
        Ok(collection.find_one(doc! { "_id": id })?)
    }

    /// Create a new camera. Fails if creating Internal and one already exists.
    pub fn create_camera(&self, name: String, camera_type: CameraType) -> Result<ObjectId, ApiError> {
        if camera_type.is_internal() {
            let existing = self.0
                .collection::<CameraDoc>(CAMERAS_COLLECTION)
                .find_one(doc! { "camera_type": { "Internal": null } })?;
            if existing.is_some() {
                return Err(ApiError::InternalCameraExists);
            }
        }
        let collection = self.0.collection::<CameraDoc>(CAMERAS_COLLECTION);
        let doc = CameraDoc {
            id: None,
            name,
            camera_type,
        };
        let result = collection.insert_one(doc)?;
        result
            .inserted_id
            .as_object_id()
            .ok_or_else(|| ApiError::BadRequest("Failed to get inserted id".to_string()))
    }

    /// Update a camera. Fails if updating to Internal and another Internal already exists.
    pub fn update_camera(
        &self,
        id: &ObjectId,
        name: String,
        camera_type: CameraType,
    ) -> Result<(), ApiError> {
        if camera_type.is_internal() {
            let existing = self.0
                .collection::<CameraDoc>(CAMERAS_COLLECTION)
                .find_one(doc! { "camera_type": { "Internal": null }, "_id": { "$ne": id } })?;
            if existing.is_some() {
                return Err(ApiError::InternalCameraExists);
            }
        }
        let collection = self.0.collection::<CameraDoc>(CAMERAS_COLLECTION);
        let camera_type_bson = polodb_core::bson::to_bson(&camera_type)
            .map_err(|e| ApiError::Unknown(e.to_string()))?;
        let camera_type_doc = match camera_type_bson {
            polodb_core::bson::Bson::Document(d) => d,
            _ => return Err(ApiError::BadRequest("Invalid camera type".to_string())),
        };
        let update_doc = doc! {
            "$set": {
                "name": name,
                "camera_type": camera_type_doc
            }
        };
        let result = collection.update_one(doc! { "_id": id }, update_doc)?;
        if result.matched_count == 0 {
            return Err(ApiError::CameraNotFound);
        }
        Ok(())
    }

    /// Delete a camera.
    pub fn delete_camera(&self, id: &ObjectId) -> Result<bool, ApiError> {
        let collection = self.0.collection::<CameraDoc>(CAMERAS_COLLECTION);
        let result = collection.delete_one(doc! { "_id": id })?;
        Ok(result.deleted_count > 0)
    }
}
