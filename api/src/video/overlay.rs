//! Match overlay rendering and PNG export for RTMP streams.

use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::sync::{Arc, RwLock};

use crate::db::pool_match::{MatchPlayer, Rating};
use crate::db::Db;
use crate::video::rtmp;

/// Overlay PNG directory.
const OVERLAY_PNG_DIR: &str = "data";

/// Overlay path for a camera. Sanitizes camera name for use in filename.
pub fn overlay_path_for_camera(camera_name: &str) -> std::path::PathBuf {
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

/// Max stream dimensions for overlay PNG.
const MAX_STREAM_WIDTH: u32 = 1280;

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
pub fn clear_overlay_png(path: &std::path::Path) {
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

/// Restart active RTMP stream for camera so ffmpeg picks up overlay changes.
fn restart_rtmp_for_camera(db: &Db, camera_name: &str, rtmp_processes: &rtmp::RtmpState) {
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

    if rtmp::spawn_rtmp_pipeline(
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

/// Restore overlay from any active match in the database. Call at server startup.
pub fn restore_overlay_from_db(db: &Db, overlay_state: &OverlayState, rtmp_processes: &rtmp::RtmpState) {
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

/// Spawn a background task that periodically syncs overlay PNG with DB. Uses update_overlay
/// so it skips write/restart when nothing has changed.
pub fn spawn_overlay_refresh_task(db: Db, overlay_state: OverlayState, rtmp_processes: rtmp::RtmpState) {
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
    rtmp_processes: &rtmp::RtmpState,
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
    rtmp_processes: &rtmp::RtmpState,
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
