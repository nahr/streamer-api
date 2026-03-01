use axum::{
    extract::{Path, State},
    routing::get,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::db::camera::CameraType;
use crate::error::ApiError;
use crate::video;

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CameraResponse {
    pub id: String,
    pub name: String,
    pub camera_type: CameraType,
}

impl CameraResponse {
    fn from_doc(doc: crate::db::camera::CameraDoc) -> Option<Self> {
        doc.id.map(|id| Self {
            id,
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
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
) -> Result<Json<Vec<CameraResponse>>, ApiError> {
    let cameras = app.db.list_cameras()?;
    let responses: Vec<CameraResponse> = cameras
        .into_iter()
        .filter_map(CameraResponse::from_doc)
        .collect();
    Ok(Json(responses))
}

/// GET /api/cameras/:id - Get a camera by id.
pub async fn cameras_get(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CameraResponse>, ApiError> {
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }
    let camera = app.db
        .find_camera_by_id(&id)?
        .ok_or(ApiError::CameraNotFound)?;
    CameraResponse::from_doc(camera).ok_or(ApiError::CameraNotFound).map(Json)
}

/// POST /api/cameras - Create a new camera.
pub async fn cameras_create(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Json(req): Json<CameraCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.name.is_empty() {
        return Err(ApiError::BadRequest("name is required".to_string()));
    }
    let id = app.db.create_camera(req.name, req.camera_type)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

/// PUT /api/cameras/:id - Update a camera.
pub async fn cameras_update(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CameraUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.name.is_empty() {
        return Err(ApiError::BadRequest("name is required".to_string()));
    }
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }
    app.db.update_camera(&id, req.name, req.camera_type)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/cameras/:id - Delete a camera.
pub async fn cameras_delete(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }
    let deleted = app.db.delete_camera(&id)?;
    if !deleted {
        return Err(ApiError::CameraNotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/cameras", get(cameras_list).post(cameras_create))
        .route("/api/cameras/:id/stream", get(video::camera_stream))
        .route(
            "/api/cameras/:id/stream/rtmp",
            axum::routing::post(video::camera_stream_rtmp_start),
        )
        .route(
            "/api/cameras/:id/stream/rtmp/stop",
            axum::routing::post(video::camera_stream_rtmp_stop),
        )
        .route(
            "/api/cameras/:id/stream/rtmp/status",
            axum::routing::get(video::camera_stream_rtmp_status),
        )
        .route("/api/cameras/:id", get(cameras_get).put(cameras_update).delete(cameras_delete))
}
