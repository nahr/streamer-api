//! Internal camera capture and streaming via FFmpeg (avfoundation on macOS, v4l2 on Linux).

use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use polodb_core::bson::oid::ObjectId;
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::error::ApiError;
use crate::video::{overlay, rtmp, CameraSource};

const MJPEG_BOUNDARY: &str = "frame";

static INTERNAL_CAMERA: RwLock<Option<Arc<InternalCameraState>>> = RwLock::new(None);

/// Shared state for the internal camera stream.
pub struct InternalCameraState {
    tx: broadcast::Sender<Bytes>,
}

impl CameraSource for InternalCameraState {
    fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.tx.subscribe()
    }
}

/// Parse MJPEG stream from FFmpeg stdout into individual JPEG frames.
/// JPEG frames start with FF D8 and end with FF D9.
fn extract_jpeg_frames(mut reader: ChildStdout, tx: broadcast::Sender<Bytes>) {
    let mut buf = [0u8; 65536];
    let mut frame = Vec::new();
    let mut in_frame = false;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                let mut i = 0;
                while i < chunk.len() {
                    if !in_frame {
                        if i + 1 < chunk.len() && chunk[i] == 0xFF && chunk[i + 1] == 0xD8 {
                            in_frame = true;
                            frame.clear();
                            frame.extend_from_slice(&chunk[i..]);
                            i = chunk.len();
                        } else {
                            i += 1;
                        }
                    } else {
                        frame.extend_from_slice(&chunk[i..]);
                        if let Some(pos) = frame.windows(2).rposition(|w| w[0] == 0xFF && w[1] == 0xD9) {
                            let end = pos + 2;
                            let jpeg = frame.drain(..end).collect::<Vec<_>>();
                            let _ = tx.send(Bytes::from(jpeg));
                            in_frame = false;
                            i = chunk.len();
                        } else {
                            i = chunk.len();
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}

/// Spawn FFmpeg to capture camera and output MJPEG to stdout.
/// macOS: avfoundation. Linux: v4l2.
fn spawn_preview_ffmpeg(camera_index: u32) -> Option<Child> {
    let cam_idx = std::env::var("CAMERA_INDEX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(camera_index);

    let child = if cfg!(target_os = "macos") {
        Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "avfoundation",
                "-framerate", "30",
                "-video_device_index", &cam_idx.to_string(),
                "-i", "0:none",
                "-f", "mjpeg",
                "-q:v", "5",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    } else {
        let device = std::env::var("VIDEO_DEVICE").unwrap_or_else(|_| "/dev/video0".to_string());
        Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "v4l2",
                "-input_format", "mjpeg",
                "-framerate", "30",
                "-i", &device,
                "-f", "mjpeg",
                "-q:v", "5",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    };

    match child {
        Ok(mut c) => {
            if let Some(stdout) = c.stdout.take() {
                let (tx, _) = broadcast::channel(16);
                let state = Arc::new(InternalCameraState { tx: tx.clone() });
                *INTERNAL_CAMERA.write().unwrap() = Some(state);
                std::thread::spawn(move || extract_jpeg_frames(stdout, tx));
            }
            Some(c)
        }
        Err(e) => {
            tracing::warn!("FFmpeg camera capture not available (expected in Docker): {}", e);
            None
        }
    }
}

/// Stop the preview FFmpeg so RTMP can use the camera. Returns after process exits.
fn stop_preview_ffmpeg(handle: &PreviewFfmpegHandle) {
    if let Some(mut child) = handle.write().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
        *INTERNAL_CAMERA.write().unwrap() = None;
        tracing::info!("Preview FFmpeg stopped for RTMP");
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Handle to the preview FFmpeg process. Stored in app state for coordination with RTMP.
pub type PreviewFfmpegHandle = Arc<RwLock<Option<Child>>>;

fn get_or_init_camera(_overlay: overlay::OverlayState, preview_handle: PreviewFfmpegHandle) -> Option<Arc<InternalCameraState>> {
    {
        let guard = INTERNAL_CAMERA.read().unwrap();
        if let Some(ref state) = *guard {
            return Some(Arc::clone(state));
        }
    }
    let cam_idx = std::env::var("CAMERA_INDEX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if let Some(child) = spawn_preview_ffmpeg(cam_idx) {
        *preview_handle.write().unwrap() = Some(child);
    }
    INTERNAL_CAMERA.read().unwrap().clone()
}

/// Pre-initialize the internal camera at startup.
pub fn ensure_internal_camera_ready(overlay: overlay::OverlayState, preview_handle: PreviewFfmpegHandle) {
    let _ = get_or_init_camera(overlay, preview_handle);
}

/// GET /api/cameras/:id/stream - MJPEG stream for internal cameras.
pub async fn camera_stream(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = app
        .db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        return Err(ApiError::BadRequest(
            "Stream only available for internal cameras".to_string(),
        ));
    }

    let state = match get_or_init_camera(app.overlay.clone(), app.preview_ffmpeg.clone()) {
        Some(s) => s,
        None => {
            return Err(ApiError::BadRequest(
                "Internal camera not available. Ensure ffmpeg is installed.".to_string(),
            ));
        }
    };

    let rx = state.subscribe();
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
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = app
        .db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        return Err(ApiError::BadRequest(
            "RTMP export only available for internal cameras".to_string(),
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

    overlay::update_overlay(
        &app.db,
        &app.overlay,
        &oid,
        &app.rtmp_processes,
        None,
    );

    let cam_idx = std::env::var("CAMERA_INDEX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Stop preview FFmpeg so RTMP can use the camera (macOS avfoundation, Linux v4l2)
    stop_preview_ffmpeg(&app.preview_ffmpeg);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, id);
    let overlay_path = overlay::overlay_path_for_camera(&camera.name);
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let rtmp = app.rtmp_processes.clone();
    let id_clone = id.clone();
    let rtmp_url = req.url.clone();
    let preview_handle = app.preview_ffmpeg.clone();

    match rtmp::spawn_rtmp_pipeline(
        &stream_url,
        &rtmp_url,
        stop_rx,
        rtmp.clone(),
        id_clone,
        &overlay_path,
        cam_idx,
    ) {
        Ok(()) => {
            rtmp.write().unwrap().insert(id.clone(), (stop_tx, rtmp_url));
            tracing::info!("RTMP start: ffmpeg pipeline started successfully");
            Ok(axum::Json(
                serde_json::json!({ "ok": true, "message": "RTMP stream started" }),
            ))
        }
        Err(e) => {
            // Restart preview on failure
            let _ = get_or_init_camera(app.overlay.clone(), preview_handle);
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

    // Restart preview FFmpeg
    let _ = get_or_init_camera(app.overlay.clone(), app.preview_ffmpeg.clone());
    tracing::info!(camera_id = %id, "RTMP stop: stream stopped, preview restarted");

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
