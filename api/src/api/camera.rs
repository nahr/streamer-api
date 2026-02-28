use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json,
};
use polodb_core::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::db::{camera::CameraType, Db};
use crate::error::ApiError;

#[derive(Serialize, Deserialize, Debug)]
pub struct CameraResponse {
    pub id: String,
    pub name: String,
    pub camera_type: CameraType,
}

impl CameraResponse {
    fn from_doc(doc: crate::db::camera::CameraDoc) -> Option<Self> {
        doc.id.map(|id| Self {
            id: id.to_hex(),
            name: doc.name,
            camera_type: doc.camera_type,
        })
    }
}

#[derive(Deserialize)]
pub struct CameraCreateRequest {
    pub name: String,
    pub camera_type: CameraType,
}

#[derive(Deserialize)]
pub struct CameraUpdateRequest {
    pub name: String,
    pub camera_type: CameraType,
}

/// GET /api/cameras - List all cameras.
pub async fn cameras_list(
    State(db): State<Db>,
) -> Result<Json<Vec<CameraResponse>>, ApiError> {
    let cameras = db.list_cameras()?;
    let responses: Vec<CameraResponse> = cameras
        .into_iter()
        .filter_map(CameraResponse::from_doc)
        .collect();
    Ok(Json(responses))
}

/// GET /api/cameras/:id - Get a camera by id.
pub async fn cameras_get(
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Result<Json<CameraResponse>, ApiError> {
    let oid = ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;
    let camera = db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;
    CameraResponse::from_doc(camera).ok_or(ApiError::CameraNotFound).map(Json)
}

/// POST /api/cameras - Create a new camera.
pub async fn cameras_create(
    State(db): State<Db>,
    Json(req): Json<CameraCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.name.is_empty() {
        return Err(ApiError::BadRequest("name is required".to_string()));
    }
    let id = db.create_camera(req.name, req.camera_type)?;
    Ok(Json(serde_json::json!({ "id": id.to_hex() })))
}

/// PUT /api/cameras/:id - Update a camera.
pub async fn cameras_update(
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<CameraUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.name.is_empty() {
        return Err(ApiError::BadRequest("name is required".to_string()));
    }
    let oid = ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;
    db.update_camera(&oid, req.name, req.camera_type)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/cameras/:id - Delete a camera.
pub async fn cameras_delete(
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let oid = ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;
    let deleted = db.delete_camera(&oid)?;
    if !deleted {
        return Err(ApiError::CameraNotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn routes() -> axum::Router<Db> {
    axum::Router::new()
        .route("/api/cameras", get(cameras_list).post(cameras_create))
        .route("/api/cameras/:id", get(cameras_get).put(cameras_update).delete(cameras_delete))
}
