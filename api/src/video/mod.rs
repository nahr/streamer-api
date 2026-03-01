//! Internal camera MJPEG streaming with match overlay.

use ab_glyph::FontRef;
use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use image::{imageops::FilterType, load_from_memory, RgbImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType},
    Camera,
};
use polodb_core::bson::oid::ObjectId;
use std::sync::{Arc, OnceLock, RwLock};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::db::pool_match::{MatchPlayer, Rating};
use crate::db::Db;
use crate::error::ApiError;

const MJPEG_BOUNDARY: &str = "frame";

/// Overlay PNG directory.
const OVERLAY_PNG_DIR: &str = "data";

/// Overlay path for a camera. Sanitizes camera name for use in filename.
fn overlay_path_for_camera(camera_name: &str) -> std::path::PathBuf {
    let sanitized: String = camera_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = if sanitized.is_empty() {
        "internal".to_string()
    } else {
        sanitized
    };
    std::path::Path::new(OVERLAY_PNG_DIR).join(format!("rtmp-overlay-{}.png", name))
}

/// Height of the overlay bar in pixels (matches MAX_STREAM_WIDTH for aspect).
const OVERLAY_BAR_HEIGHT: u32 = 80;

/// Max stream dimensions. Downscaling reduces CPU load for decode/encode and improves responsiveness.
const MAX_STREAM_WIDTH: u32 = 1280;
const MAX_STREAM_HEIGHT: u32 = 720;
const JPEG_QUALITY: u8 = 65;

/// Overlay data for an active match. Displayed at bottom of stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOverlay {
    pub player_one: OverlayPlayer,
    pub player_two: OverlayPlayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayPlayer {
    pub name: String,
    pub rating: Option<String>,
    pub games_won: u8,
    pub race_to: u8,
}

impl OverlayPlayer {
    pub fn from_match_player(p: &MatchPlayer) -> Self {
        let rating = p.rating.as_ref().map(|r| match r {
            Rating::Apa(v) => format!("APA {}", v),
            Rating::Fargo(v) => format!("Fargo {}", v),
        });
        Self {
            name: p.name.clone(),
            rating,
            games_won: p.games_won,
            race_to: p.race_to,
        }
    }
}

/// Shared overlay state. Updated by pool_match handlers when match changes.
pub type OverlayState = Arc<RwLock<Option<MatchOverlay>>>;

/// Active RTMP streams: camera_id -> (stop sender, rtmp_url). Send to stop; url used for restart.
pub type RtmpState =
    Arc<RwLock<std::collections::HashMap<String, (std::sync::mpsc::Sender<()>, String)>>>;

pub fn rtmp_state_new() -> RtmpState {
    Arc::new(RwLock::new(std::collections::HashMap::new()))
}

static INTERNAL_CAMERA: OnceLock<Arc<InternalCameraState>> = OnceLock::new();

/// Pre-initialize the internal camera capture loop at startup. Ensures the stream is ready
/// before any requests (e.g. when user starts match first, then goes live via OAuth).
pub fn ensure_internal_camera_ready(overlay: OverlayState) {
    let _ = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        spawn_camera_capture(tx.clone(), overlay);
        Arc::new(InternalCameraState::new(tx))
    });
}

/// Restore overlay from any active match in the database. Call at server startup.
pub fn restore_overlay_from_db(db: &Db, overlay_state: &OverlayState, rtmp_processes: &RtmpState) {
    let cameras = db.list_cameras().ok().unwrap_or_default();
    for camera in cameras {
        if camera.camera_type.is_internal()
            && db
                .find_active_pool_match_by_camera_name(&camera.name)
                .ok()
                .flatten()
                .is_some()
        {
            update_overlay(db, overlay_state, &camera.name, rtmp_processes, None);
            break;
        }
    }
}

/// Shared state for the internal camera stream.
pub struct InternalCameraState {
    tx: broadcast::Sender<Bytes>,
}

impl InternalCameraState {
    fn new(tx: broadcast::Sender<Bytes>) -> Self {
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.tx.subscribe()
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

fn load_font() -> Option<FontRef<'static>> {
    let font_bytes = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
    FontRef::try_from_slice(font_bytes).ok()
}

/// Draw the match overlay onto an RGBA image (for PNG export).
/// Layout: left player name+rating | [circle] race to x/y [circle] | right player name+rating.
fn draw_overlay_to_rgba(img: &mut RgbaImage, overlay: &MatchOverlay, font: &FontRef) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    if w < 1 || h < 1 {
        return;
    }
    // Match UI: background rgba(0,0,0,0.9), px: 2 (16px), py: 1.25, gap: 2 (16px)
    let scale = (h as f32 / 80.0 * 20.0).max(14.0).min(36.0);
    let line_h = (scale * 1.1) as i32;
    let px = 16; // px: 2 = 16px
    let section_gap = 16; // gap: 2 between left/center/right

    let bar_rect = Rect::at(0, 0).of_size(w as u32, h as u32);
    draw_filled_rect_mut(img, bar_rect, Rgba([0u8, 0, 0, 230])); // rgba(0,0,0,0.9)

    let white = Rgba([255u8, 255, 255, 255]);
    let gray = Rgba([204u8, 204, 204, 255]); // rgba(255,255,255,0.8)
    let scale_sm = scale * 0.65;

    // All content vertically centered (alignItems: 'center')
    let center_y = h / 2;

    // Left: player1 name + rating, flex-start aligned
    let p1_name_y = center_y - line_h / 2;
    let p1_rating_y = center_y + line_h / 2;
    draw_text_mut(
        img,
        white,
        px,
        p1_name_y,
        ab_glyph::PxScale::from(scale),
        font,
        &overlay.player_one.name,
    );
    if let Some(ref r) = overlay.player_one.rating {
        draw_text_mut(
            img,
            gray,
            px,
            p1_rating_y,
            ab_glyph::PxScale::from(scale_sm),
            font,
            r,
        );
    }

    // Center: 32px circles + "race to" (center-aligned) + x/y
    let bar_color = Rgba([0u8, 0, 0, 230]);
    let score_scale = scale * 1.1;
    let s1 = overlay.player_one.games_won.to_string();
    let s2 = overlay.player_two.games_won.to_string();
    let race_line1 = "race to";
    let race_line2 = format!(
        "{}/{}",
        overlay.player_one.race_to, overlay.player_two.race_to
    );

    let (s1_w, s1_h) =
        imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s1);
    let (race1_w, race1_h) =
        imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, race_line1);
    let (race2_w, _) =
        imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, &race_line2);
    let (s2_w, s2_h) =
        imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s2);

    let circle_d = 32i32;
    let circle_r = circle_d / 2;
    let center_gap = 12i32;
    let race_w = race1_w.max(race2_w);
    let total_w = circle_d + center_gap + race_w as i32 + center_gap + circle_d;
    let center_start = (w - total_w) / 2;
    let center_end = center_start + total_w;

    let s1_cx = center_start + circle_r;
    let race_x = center_start + circle_d + center_gap;
    let s2_cx = center_start + circle_d + center_gap + race_w as i32 + center_gap + circle_r;
    let s1_x = s1_cx - s1_w as i32 / 2;
    let s2_x = s2_cx - s2_w as i32 / 2;

    // 2px white border circles
    draw_filled_circle_mut(img, (s1_cx, center_y), circle_r, white);
    draw_filled_circle_mut(img, (s1_cx, center_y), circle_r - 2, bar_color);
    draw_filled_circle_mut(img, (s2_cx, center_y), circle_r, white);
    draw_filled_circle_mut(img, (s2_cx, center_y), circle_r - 2, bar_color);

    let score_y = center_y - s1_h as i32 / 2 - 3; // nudge number up within circle
    draw_text_mut(
        img,
        white,
        s1_x,
        score_y,
        ab_glyph::PxScale::from(score_scale),
        font,
        &s1,
    );
    draw_text_mut(
        img,
        white,
        s2_x,
        center_y - s2_h as i32 / 2 - 3,
        ab_glyph::PxScale::from(score_scale),
        font,
        &s2,
    );

    // "race to" and "x/y" - center aligned (horizontal and vertical)
    let race_gap = 2i32;
    let race_block_h = race1_h as i32 + race_gap + race1_h as i32;
    let race_y1 = center_y - race_block_h / 2;
    let race_y2 = race_y1 + race1_h as i32 + race_gap;
    let race_line1_x = race_x + (race_w as i32 - race1_w as i32) / 2;
    let race_line2_x = race_x + (race_w as i32 - race2_w as i32) / 2;
    draw_text_mut(
        img,
        gray,
        race_line1_x,
        race_y1,
        ab_glyph::PxScale::from(scale_sm),
        font,
        race_line1,
    );
    draw_text_mut(
        img,
        gray,
        race_line2_x,
        race_y2,
        ab_glyph::PxScale::from(scale_sm),
        font,
        &race_line2,
    );

    // Right: player2 name + rating, flex-end aligned
    let (p2_w, _) = imageproc::drawing::text_size(
        ab_glyph::PxScale::from(scale),
        font,
        &overlay.player_two.name,
    );
    let p2_x = (w - p2_w as i32 - px).max(center_end + section_gap);
    draw_text_mut(
        img,
        white,
        p2_x,
        p1_name_y,
        ab_glyph::PxScale::from(scale),
        font,
        &overlay.player_two.name,
    );
    if let Some(ref r) = overlay.player_two.rating {
        draw_text_mut(
            img,
            gray,
            p2_x,
            p1_rating_y,
            ab_glyph::PxScale::from(scale_sm),
            font,
            r,
        );
    }
}

/// Temp path for atomic overlay write. Must end in .png so image crate recognizes format.
fn overlay_tmp_path(path: &std::path::Path) -> std::path::PathBuf {
    path.with_file_name("rtmp-overlay.tmp.png")
}

/// Write overlay to PNG for ffmpeg to use. Atomic write so ffmpeg -reload picks up changes.
fn render_overlay_to_png(overlay: &MatchOverlay, path: &std::path::Path) {
    if let Some(font) = load_font() {
        let mut img =
            RgbaImage::from_pixel(MAX_STREAM_WIDTH, OVERLAY_BAR_HEIGHT, Rgba([0, 0, 0, 0]));
        draw_overlay_to_rgba(&mut img, overlay, &font);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = overlay_tmp_path(path);
        if let Err(e) = img.save(&tmp) {
            tracing::warn!(path = ?tmp, error = %e, "Failed to save overlay PNG");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            tracing::warn!(path = ?path, error = %e, "Failed to rename overlay PNG");
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        tracing::info!(path = %path.display(), "Overlay PNG written");
    } else {
        tracing::warn!("No font for overlay PNG");
    }
}

/// Write empty transparent PNG when overlay is cleared. Atomic write for ffmpeg -reload.
fn clear_overlay_png(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(path = ?parent, error = %e, "Failed to create data directory for overlay");
            return;
        }
    }
    let img = RgbaImage::from_pixel(MAX_STREAM_WIDTH, OVERLAY_BAR_HEIGHT, Rgba([0, 0, 0, 0]));
    let tmp = overlay_tmp_path(path);
    if let Err(e) = img.save(&tmp) {
        tracing::warn!(path = ?tmp, error = %e, "Failed to save empty overlay PNG");
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        tracing::warn!(path = ?path, error = %e, "Failed to rename overlay PNG");
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    tracing::info!(path = %path.display(), "Overlay PNG cleared (transparent)");
}

/// Spawn the camera capture task. Emits raw MJPEG; overlay is applied by ffmpeg from data/rtmp-overlay.png.
fn spawn_camera_capture(tx: broadcast::Sender<Bytes>, _overlay_state: OverlayState) {
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

/// Spawn a background task that periodically syncs overlay PNG with DB. Uses update_overlay
/// so it skips write/restart when nothing has changed.
pub fn spawn_overlay_refresh_task(db: Db, overlay_state: OverlayState, rtmp_processes: RtmpState) {
    std::thread::spawn(move || {
        let interval = std::time::Duration::from_secs(2);
        loop {
            std::thread::sleep(interval);
            let cameras = match db.list_cameras() {
                Ok(c) => c,
                Err(_) => continue,
            };
            for camera in cameras {
                if camera.camera_type.is_internal() {
                    update_overlay(&db, &overlay_state, &camera.name, &rtmp_processes, None);
                }
            }
        }
    });
}

/// Update the overlay for the camera. Call when match is created/updated.
/// Sets overlay to None when no active match.
/// Pass `overlay_from_match` when you have fresh match data (e.g. from score update) to avoid DB read.
pub fn update_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_name: &str,
    rtmp_processes: &RtmpState,
    overlay_from_match: Option<MatchOverlay>,
) {
    let camera = match db.find_camera_by_name(camera_name) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(camera = %camera_name, "Camera not found by name");
            return;
        }
        Err(e) => {
            tracing::warn!(camera = %camera_name, error = %e, "Failed to find camera");
            return;
        }
    };

    if !camera.camera_type.is_internal() {
        tracing::debug!(camera = %camera_name, "Overlay only applies to internal cameras");
        return;
    }

    let overlay = overlay_from_match.or_else(|| {
        db.find_active_pool_match_by_camera_name(camera_name)
            .ok()
            .flatten()
            .filter(|m| m.end_time.is_none())
            .map(|m| MatchOverlay {
                player_one: OverlayPlayer::from_match_player(&m.player_one),
                player_two: OverlayPlayer::from_match_player(&m.player_two),
            })
    });

    let current = overlay_state.read().ok().and_then(|g| (*g).clone());
    if current == overlay {
        tracing::debug!(camera = %camera_name, "Overlay unchanged, skipping write");
        return;
    }

    let path = overlay_path_for_camera(camera_name);

    if let Some(ref o) = overlay {
        tracing::info!(camera = %camera_name, "Overlay PNG rendered");
        render_overlay_to_png(o, &path);
    } else {
        tracing::info!(camera = %camera_name, "Overlay cleared");
        clear_overlay_png(&path);
    }

    if let Ok(mut guard) = overlay_state.write() {
        let is_set = overlay.is_some();
        *guard = overlay;
        if is_set {
            tracing::info!(camera = %camera_name, "Overlay set for active match");
        }
    }

    // Restart RTMP stream so ffmpeg picks up the new overlay (it doesn't reload the file)
    restart_rtmp_for_camera(db, camera_name, rtmp_processes);
}

/// Clear the overlay (e.g. when match ends).
pub fn clear_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_name: &str,
    rtmp_processes: &RtmpState,
) {
    let camera = match db.find_camera_by_name(camera_name) {
        Ok(Some(c)) if c.camera_type.is_internal() => c,
        _ => return,
    };
    let current = overlay_state.read().ok().and_then(|g| (*g).clone());
    if current.is_none() {
        tracing::debug!(camera = %camera_name, "Overlay already cleared, skipping");
        return;
    }
    clear_overlay_png(&overlay_path_for_camera(&camera.name));
    if let Ok(mut guard) = overlay_state.write() {
        *guard = None;
    }
    restart_rtmp_for_camera(db, camera_name, rtmp_processes);
}

/// Restart active RTMP stream for camera so ffmpeg picks up overlay changes.
fn restart_rtmp_for_camera(db: &Db, camera_name: &str, rtmp_processes: &RtmpState) {
    let camera = match db.find_camera_by_name(camera_name) {
        Ok(Some(c)) if c.camera_type.is_internal() => c,
        _ => return,
    };
    let camera_id = match camera.id {
        Some(id) => id.to_hex(),
        None => return,
    };

    let (stop_tx, rtmp_url) = match rtmp_processes.write().unwrap().remove(&camera_id) {
        Some((tx, url)) => (tx, url),
        None => return,
    };

    let _ = stop_tx.send(());
    std::thread::sleep(std::time::Duration::from_secs(2));

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, camera_id);
    let (stop_tx_new, stop_rx) = std::sync::mpsc::channel();
    let rtmp = rtmp_processes.clone();
    let overlay_path = overlay_path_for_camera(camera_name);

    if spawn_rtmp_pipeline(
        &stream_url,
        &rtmp_url,
        stop_rx,
        rtmp.clone(),
        camera_id.clone(),
        &overlay_path,
    )
    .is_ok()
    {
        rtmp.write()
            .unwrap()
            .insert(camera_id, (stop_tx_new, rtmp_url));
        tracing::info!(camera = %camera_name, "RTMP stream restarted for overlay update");
    }
}

/// GET /api/cameras/:id/stream - MJPEG stream for internal cameras.
pub async fn camera_stream(
    State(app): State<crate::api::AppState>,
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

/// Spawn an ffmpeg process that reads MJPEG from stream_url and pushes to rtmp_url.
/// Overlays the PNG at data/rtmp-overlay.png (written when match overlay changes).
/// Runs in a thread; stops when stop_rx receives. Removes from rtmp_processes when done.
/// Requires ffmpeg to be installed (with libx264 and AAC support).
fn spawn_rtmp_pipeline(
    stream_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
) -> Result<(), String> {
    if !overlay_path.exists() {
        clear_overlay_png(overlay_path);
    }
    if !overlay_path.exists() {
        return Err(format!(
            "Overlay PNG not found at {}. Ensure data/ directory exists.",
            overlay_path.display()
        ));
    }

    let overlay_path_str = overlay_path
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(overlay_path)))
        .map_err(|e| format!("Overlay path error: {}", e))?
        .to_string_lossy()
        .into_owned();

    tracing::info!(overlay_path = %overlay_path_str, "RTMP: ffmpeg overlay PNG");

    // Facebook Live: H.264 baseline, AAC 48kHz stereo 128kbps CBR, keyframes every 2s, ~2Mbps video.
    let mut child = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            // ========== AUDIO SOURCE ==========
            "-f",
            "lavfi",
            "-i",
            "anullsrc=channel_layout=stereo:sample_rate=48000",
            // ========== VIDEO INPUT (MJPEG) ==========
            "-f",
            "mjpeg",
            "-r",
            "30",
            "-i",
            stream_url,
            // ========== OVERLAY PNG ==========
            "-f",
            "image2",
            "-loop",
            "1",
            "-i",
            &overlay_path_str,
            // ========== FILTER GRAPH ==========
            "-filter_complex",
            "[1:v]fps=30[main];[main][2:v]overlay=0:H-80,format=yuv420p[out]",
            "-map",
            "[out]",
            "-map",
            "0:a",
            // ========== VIDEO ENCODER ==========
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-profile:v",
            "high",
            "-level",
            "4.1",
            "-pix_fmt",
            "yuv420p",
            "-b:v",
            "2000k",
            "-maxrate",
            "2000k",
            "-bufsize",
            "4000k",
            "-g",
            "60", // 30fps * 2s
            "-keyint_min",
            "60",
            "-x264-params",
            "scenecut=0:open_gop=0:min-keyint=60",
            "-bf",
            "2", // stable B-frames
            "-fps_mode",
            "cfr",
            // ========== AUDIO ENCODER ==========
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ar",
            "48000",
            "-ac",
            "2",
            // ========== MUX ==========
            "-f",
            "flv",
            rtmp_url,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}. Ensure ffmpeg is installed.", e))?;

    let stderr = child.stderr.take();
    let stderr_done: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let stderr_done_clone = stderr_done.clone();
    std::thread::spawn(move || {
        if let Some(mut stderr) = stderr {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stderr, &mut buf);
            if !buf.is_empty() {
                let s = String::from_utf8_lossy(&buf).into_owned();
                *stderr_done_clone.lock().unwrap() = Some(s);
            }
        }
    });

    std::thread::spawn(move || {
        loop {
            match stop_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }

            if child.try_wait().ok().flatten().is_some() {
                if let Ok(guard) = stderr_done.lock() {
                    if let Some(ref s) = *guard {
                        tracing::error!(camera_id = %id, "RTMP: ffmpeg exited unexpectedly:\n{}", s);
                    } else {
                        tracing::warn!(camera_id = %id, "RTMP: ffmpeg process exited unexpectedly");
                    }
                }
                break;
            }
        }

        let _ = child.kill();
        let _ = child.wait();
        rtmp.write().unwrap().remove(&id);
        tracing::info!(camera_id = %id, "RTMP: ffmpeg pipeline ended");
    });

    Ok(())
}

/// POST /api/cameras/:id/stream/rtmp - Start RTMP push to the given URL.
/// Spawns ffmpeg to read the MJPEG stream and push to RTMP (e.g. YouTube Live, Facebook).
/// Requires ffmpeg to be installed (with libx264 and AAC support).
/// The overlay is burned into the stream.
pub async fn camera_stream_rtmp_start(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<RtmpStartRequest>,
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
    update_overlay(
        &app.db,
        &app.overlay,
        &camera.name,
        &app.rtmp_processes,
        None,
    );

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, id);
    tracing::info!(stream_url = %stream_url, "RTMP start: starting ffmpeg pipeline");

    let overlay_path = overlay_path_for_camera(&camera.name);
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let rtmp = app.rtmp_processes.clone();
    let id_clone = id.clone();
    let rtmp_url = req.url.clone();

    match spawn_rtmp_pipeline(
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

#[derive(serde::Deserialize)]
pub struct RtmpStartRequest {
    pub url: String,
}

/// POST /api/cameras/:id/stream/rtmp/stop - Stop the RTMP stream for this camera.
pub async fn camera_stream_rtmp_stop(
    State(app): State<crate::api::AppState>,
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
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let active = app.rtmp_processes.read().unwrap().contains_key(&id);
    Ok(axum::Json(serde_json::json!({ "active": active })))
}
