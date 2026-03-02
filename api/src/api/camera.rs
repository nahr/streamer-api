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
    /// Whether the camera is connected and ready (from MediaMTX). None if not RTSP or status unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_status: Option<bool>,
}

impl CameraResponse {
    fn from_doc(
        doc: crate::db::camera::CameraDoc,
        connection_status: Option<bool>,
    ) -> Option<Self> {
        doc.id.map(|id| Self {
            id,
            name: doc.name,
            camera_type: doc.camera_type.clone(),
            connection_status: if doc.camera_type.is_rtsp() {
                Some(connection_status.unwrap_or(false))
            } else {
                None
            },
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
    let status = app
        .camera_connection_status
        .read()
        .map(|g| (*g).clone())
        .unwrap_or_default();
    let responses: Vec<CameraResponse> = cameras
        .into_iter()
        .filter_map(|doc| {
            let conn = doc.id.as_ref().and_then(|id| status.get(id).copied());
            CameraResponse::from_doc(doc, conn)
        })
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
    let conn = app.camera_connection_status.read().ok().and_then(|g| g.get(&id).copied());
    CameraResponse::from_doc(camera, conn).ok_or(ApiError::CameraNotFound).map(Json)
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
    let is_rtsp = req.camera_type.is_rtsp();
    let id = app.db.create_camera(req.name, req.camera_type)?;
    let id_clone = id.clone();
    // Sync to MediaMTX in background if it's an RTSP camera (retries when camera is powered off)
    if is_rtsp {
        let db = app.db.clone();
        tokio::spawn(async move {
            if let Ok(Some(camera)) = db.find_camera_by_id(&id_clone) {
                let settings = db.get_settings().unwrap_or_default();
                if let Err(e) = video::sync_camera_path(&camera, &settings).await {
                    tracing::warn!(camera_id = %id_clone, error = %e, "MediaMTX sync failed");
                }
            }
        });
    }
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
    app.db.update_camera(&id, req.name, req.camera_type.clone())?;
    if req.camera_type.is_rtsp() {
        let db = app.db.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            if let Ok(Some(camera)) = db.find_camera_by_id(&id_clone) {
                let settings = db.get_settings().unwrap_or_default();
                if let Err(e) = video::sync_camera_path(&camera, &settings).await {
                    tracing::warn!(camera_id = %id_clone, error = %e, "MediaMTX sync failed");
                }
            }
        });
    } else {
        let id_clone = id.clone();
        tokio::spawn(async move {
            let _ = video::delete_camera_path(&id_clone).await;
        });
    }
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
    let id_clone = id.clone();
    tokio::spawn(async move {
        let _ = video::delete_camera_path(&id_clone).await;
    });
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
        .route(
            "/api/cameras/:id/recordings/download",
            axum::routing::get(video::recording_download),
        )
        .route("/api/cameras/:id", get(cameras_get).put(cameras_update).delete(cameras_delete))
}
