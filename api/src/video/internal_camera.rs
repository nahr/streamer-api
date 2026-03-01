//! Internal camera MJPEG capture and streaming via nokhwa.

use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use image::{imageops::FilterType, load_from_memory, RgbImage};
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType},
    Camera,
};
use polodb_core::bson::oid::ObjectId;
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::api::AppState;
use crate::error::ApiError;
use crate::video::{overlay, rtmp, CameraSource};

const MJPEG_BOUNDARY: &str = "frame";

/// Max stream dimensions. Downscaling reduces CPU load for decode/encode and improves responsiveness.
const MAX_STREAM_WIDTH: u32 = 1280;
const MAX_STREAM_HEIGHT: u32 = 720;
const JPEG_QUALITY: u8 = 65;

static INTERNAL_CAMERA: OnceLock<Arc<InternalCameraState>> = OnceLock::new();

/// Shared state for the internal camera stream.
pub struct InternalCameraState {
    tx: broadcast::Sender<Bytes>,
}

impl CameraSource for InternalCameraState {
    fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.tx.subscribe()
    }
}

impl InternalCameraState {
    fn new(tx: broadcast::Sender<Bytes>) -> Self {
        Self { tx }
    }
}

/// Downscale image to fit within max dimensions. Reduces CPU load for overlay and JPEG encode.
fn maybe_downscale(img: RgbImage, max_w: u32, max_h: u32) -> RgbImage {
    if img.width() <= max_w && img.height() <= max_h {
        return img;
    }
    let (w, h) = (img.width() as f32, img.height() as f32);
    let scale = (max_w as f32 / w).min(max_h as f32 / h).min(1.0);
    let new_w = (w * scale).round() as u32;
    let new_h = (h * scale).round() as u32;
    let new_w = new_w.max(1);
    let new_h = new_h.max(1);
    image::imageops::resize(&img, new_w, new_h, FilterType::Triangle)
}

/// Spawn the camera capture task. Emits raw MJPEG; overlay is applied by ffmpeg from data/rtmp-overlay.png.
fn spawn_camera_capture(tx: broadcast::Sender<Bytes>, _overlay_state: overlay::OverlayState) {
    std::thread::spawn(move || {
        let cam_idx = std::env::var("CAMERA_INDEX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let index = CameraIndex::Index(cam_idx);
        tracing::info!(camera_index = cam_idx, "Using camera");
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let mut camera = match Camera::new(index, requested) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Internal camera not available (expected in Docker): {}", e);
                return;
            }
        };

        if let Err(e) = camera.open_stream() {
            tracing::warn!("Failed to start camera stream: {}", e);
            return;
        }

        tracing::info!("Internal camera stream started");

        loop {
            match camera.frame() {
                Ok(buffer) => {
                    let jpeg_bytes = if buffer.source_frame_format() == FrameFormat::MJPEG {
                        let raw = buffer.buffer();
                        match load_from_memory(raw) {
                            Ok(dyn_img) => {
                                let rgb = maybe_downscale(
                                    dyn_img.to_rgb8(),
                                    MAX_STREAM_WIDTH,
                                    MAX_STREAM_HEIGHT,
                                );
                                let mut jpeg = Vec::new();
                                match image::codecs::jpeg::JpegEncoder::new_with_quality(
                                    &mut jpeg,
                                    JPEG_QUALITY,
                                )
                                .encode_image(&rgb)
                                {
                                    Ok(_) => Bytes::from(jpeg),
                                    Err(_) => Bytes::copy_from_slice(raw),
                                }
                            }
                            Err(_) => Bytes::copy_from_slice(raw),
                        }
                    } else {
                        match buffer.decode_image::<RgbFormat>() {
                            Ok(rgb) => {
                                let rgb = maybe_downscale(rgb, MAX_STREAM_WIDTH, MAX_STREAM_HEIGHT);
                                let mut jpeg = Vec::new();
                                if let Err(e) = image::codecs::jpeg::JpegEncoder::new_with_quality(
                                    &mut jpeg,
                                    JPEG_QUALITY,
                                )
                                .encode_image(&rgb)
                                {
                                    tracing::warn!("JPEG encode error: {}", e);
                                    continue;
                                }
                                Bytes::from(jpeg)
                            }
                            Err(e) => {
                                tracing::warn!("Frame decode error: {}", e);
                                continue;
                            }
                        }
                    };

                    if tx.send(jpeg_bytes).is_err() {
                        // No subscribers (e.g. browser + FFmpeg both disconnected).
                        // Keep running so camera stays ready for new connections.
                        tracing::debug!("No stream subscribers, continuing capture");
                    }
                }
                Err(e) => {
                    tracing::warn!("Frame capture error: {}", e);
                }
            }
        }
    });
}

/// Pre-initialize the internal camera capture loop at startup. Ensures the stream is ready
/// before any requests (e.g. when user starts match first, then goes live via OAuth).
pub fn ensure_internal_camera_ready(overlay: overlay::OverlayState) {
    let _ = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        spawn_camera_capture(tx.clone(), overlay);
        Arc::new(InternalCameraState::new(tx))
    });
}

/// GET /api/cameras/:id/stream - MJPEG stream for internal cameras.
pub async fn camera_stream(
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

    let state = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        spawn_camera_capture(tx.clone(), app.overlay.clone());
        Arc::new(InternalCameraState::new(tx))
    });

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
/// Spawns ffmpeg to read the MJPEG stream and push to RTMP (e.g. YouTube Live, Facebook).
/// Requires ffmpeg to be installed (with libx264 and AAC support).
/// The overlay is burned into the stream.
pub async fn camera_stream_rtmp_start(
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
        tracing::warn!(camera_id = %id, "RTMP start: camera is not internal");
        return Err(ApiError::BadRequest(
            "RTMP export only available for internal cameras".to_string(),
        ));
    }

    if req.url.is_empty() || (!req.url.starts_with("rtmp://") && !req.url.starts_with("rtmps://")) {
        tracing::warn!(url = %req.url, "RTMP start: invalid URL");
        return Err(ApiError::BadRequest(
            "url must be a valid RTMP URL (e.g. rtmp://... or rtmps://...)".to_string(),
        ));
    }

    // Stop any existing stream for this camera before starting a new one.
    if let Some((stop_tx, _)) = app.rtmp_processes.write().unwrap().remove(&id) {
        tracing::info!("RTMP start: stopping existing stream first");
        let _ = stop_tx.send(());
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Ensure overlay PNG reflects current match state before ffmpeg reads it
    overlay::update_overlay(
        &app.db,
        &app.overlay,
        &camera.name,
        &app.rtmp_processes,
        None,
    );

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, id);
    tracing::info!(stream_url = %stream_url, "RTMP start: starting ffmpeg pipeline");

    let overlay_path = overlay::overlay_path_for_camera(&camera.name);
    if !overlay_path.exists() {
        overlay::clear_overlay_png(&overlay_path);
    }
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let rtmp = app.rtmp_processes.clone();
    let id_clone = id.clone();
    let rtmp_url = req.url.clone();

    match rtmp::spawn_rtmp_pipeline(
        &stream_url,
        &rtmp_url,
        stop_rx,
        rtmp.clone(),
        id_clone,
        &overlay_path,
    ) {
        Ok(()) => {
            rtmp.write().unwrap().insert(id, (stop_tx, rtmp_url));
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

    tracing::info!(camera_id = %id, "RTMP stop: stream stopped");
    Ok(axum::Json(
        serde_json::json!({ "ok": true, "message": "RTMP stream stopped" }),
    ))
}

/// GET /api/cameras/:id/stream/rtmp/status - Check if RTMP stream is active.
pub async fn camera_stream_rtmp_status(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let active = app.rtmp_processes.read().unwrap().contains_key(&id);
    Ok(axum::Json(serde_json::json!({ "active": active })))
}
