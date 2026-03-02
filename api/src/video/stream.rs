//! Camera stream and RTMP handlers. Supports RTSP cameras only.

use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::api::auth::{AuthenticatedUser, StreamAuth};
use crate::api::AppState;
use crate::db::camera::CameraType;
use crate::error::ApiError;
use crate::video::{overlay, rtmp, rtsp_camera, CameraSource};

const MJPEG_BOUNDARY: &str = "frame";

/// GET /api/cameras/:id/stream - MJPEG stream for RTSP cameras.
/// Accepts either Bearer token (browser) or ?stream_token= (RTMP pipeline).
pub async fn camera_stream(
    _auth: StreamAuth,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    if id.is_empty() || id.len() > 64 {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }

    let camera = app
        .db
        .find_camera_by_id(&id)?
        .ok_or(ApiError::CameraNotFound)?;

    let rx = match &camera.camera_type {
        CameraType::Rtsp { url } => {
            let url = url.trim();
            if url.is_empty() {
                return Err(ApiError::BadRequest(
                    "RTSP URL is not configured for this camera.".to_string(),
                ));
            }
            let s = match rtsp_camera::get_or_start_rtsp_stream(&id, url) {
                Some(s) => s,
                None => {
                    return Err(ApiError::BadRequest(
                        "Failed to connect to RTSP stream. Ensure ffmpeg is installed and the URL is valid.".to_string(),
                    ));
                }
            };
            s.subscribe()
        }
    };

    let stream = BroadcastStream::new(rx)
        .map(|x| match x {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                e.to_string(),
            )),
        })
        .map(|bytes: Result<Bytes, _>| {
            bytes.map(|bytes| {
                let header = format!(
                    "\r\n--{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    MJPEG_BOUNDARY,
                    bytes.len()
                );
                let out: Bytes = [header.as_bytes(), bytes.as_ref()].concat().into();
                out
            })
        });

    let body = Body::from_stream(stream);

    let response = (
        [
            (
                header::CONTENT_TYPE,
                format!("multipart/x-mixed-replace; boundary={}", MJPEG_BOUNDARY),
            ),
            (
                header::CACHE_CONTROL,
                "no-cache, no-store, must-revalidate".to_string(),
            ),
            (header::PRAGMA, "no-cache".to_string()),
            (header::EXPIRES, "0".to_string()),
        ],
        body,
    )
        .into_response();

    Ok(response)
}

/// POST /api/cameras/:id/stream/rtmp - Start RTMP push to the given URL.
pub async fn camera_stream_rtmp_start(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<rtmp::RtmpStartRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let url_safe = if req.url.len() > 60 {
        format!("{}...", &req.url[..60])
    } else {
        req.url.clone()
    };
    tracing::info!(camera_id = %id, url = %url_safe, "RTMP start: received request");
    if id.is_empty() || id.len() > 64 {
        return Err(ApiError::BadRequest("Invalid camera id".to_string()));
    }

    let camera = app
        .db
        .find_camera_by_id(&id)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_rtsp() {
        return Err(ApiError::BadRequest(
            "RTMP export only available for RTSP cameras.".to_string(),
        ));
    }

    if req.url.is_empty() || (!req.url.starts_with("rtmp://") && !req.url.starts_with("rtmps://")) {
        return Err(ApiError::BadRequest(
            "url must be a valid RTMP URL (e.g. rtmp://... or rtmps://...)".to_string(),
        ));
    }

    if let Some((stop_tx, _)) = app.rtmp_processes.write().unwrap().remove(&id) {
        let _ = stop_tx.send(());
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    overlay::update_overlay(&app.db, &app.overlay, &id, &app.rtmp_processes, None);

    let settings = app.db.get_settings().unwrap_or_default();
    let location_name = settings.location_name.as_str();
    let camera_name = camera.name.as_str();

    let rtsp_url = camera
        .camera_type
        .rtsp_url()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| ApiError::BadRequest("RTSP URL not configured for this camera.".to_string()))?;

    let overlay_path = overlay::overlay_path_for_camera(&camera.name);
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let rtmp = app.rtmp_processes.clone();
    let id_clone = id.clone();

    match rtmp::spawn_rtmp_pipeline(
        rtsp_url,
        &req.url,
        stop_rx,
        rtmp.clone(),
        id_clone,
        &overlay_path,
        location_name,
        camera_name,
    ) {
        Ok(()) => {
            rtmp.write()
                .unwrap()
                .insert(id.clone(), (stop_tx, req.url.clone()));
            tracing::info!("RTMP start: ffmpeg pipeline started successfully");
            Ok(axum::Json(
                serde_json::json!({ "ok": true, "message": "RTMP stream started" }),
            ))
        }
        Err(e) => {
            tracing::error!(error = %e, "RTMP start: failed to start ffmpeg pipeline");
            Err(ApiError::BadRequest(format!(
                "Failed to start ffmpeg pipeline: {}. Ensure ffmpeg is installed.",
                e
            )))
        }
    }
}

/// POST /api/cameras/:id/stream/rtmp/stop - Stop the RTMP stream for this camera.
pub async fn camera_stream_rtmp_stop(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let (stop_tx, _) = app
        .rtmp_processes
        .write()
        .unwrap()
        .remove(&id)
        .ok_or_else(|| {
            ApiError::BadRequest("No active RTMP stream for this camera.".to_string())
        })?;

    if stop_tx.send(()).is_err() {
        tracing::warn!(camera_id = %id, "RTMP stop: pipeline thread already ended");
    }

    std::thread::sleep(std::time::Duration::from_secs(2));

    tracing::info!(camera_id = %id, "RTMP stop: stream stopped");

    Ok(axum::Json(
        serde_json::json!({ "ok": true, "message": "RTMP stream stopped" }),
    ))
}

/// GET /api/cameras/:id/stream/rtmp/status - Check if RTMP stream is active.
pub async fn camera_stream_rtmp_status(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let active = app.rtmp_processes.read().unwrap().contains_key(&id);
    Ok(axum::Json(serde_json::json!({ "active": active })))
}
