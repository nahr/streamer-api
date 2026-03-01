//! Match overlay rendering and PNG export for RTMP streams.
//! PNG contains only the match bar (player names, score). No location or timestamp.

use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::sync::{Arc, RwLock};

use polodb_core::bson::oid::ObjectId;

use crate::db::pool_match::{MatchPlayer, Rating};
use crate::db::Db;
use crate::video::rtmp;

/// Overlay PNG directory.
const OVERLAY_PNG_DIR: &str = "data";

const OVERLAY_WIDTH: u32 = 1280;
const OVERLAY_HEIGHT: u32 = 80;

fn overlay_name_for_camera(camera_name: &str) -> String {
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
    if sanitized.is_empty() {
        "internal".to_string()
    } else {
        sanitized
    }
}

/// Overlay path for a camera. Sanitizes camera name for use in filename.
pub fn overlay_path_for_camera(camera_name: &str) -> std::path::PathBuf {
    let name = overlay_name_for_camera(camera_name);
    std::path::Path::new(OVERLAY_PNG_DIR).join(format!("rtmp-overlay-{}.png", name))
}

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

/// Draw match overlay in the 80px bar.
fn draw_match_to_rgba(img: &mut RgbaImage, overlay: &MatchOverlay, font: &FontRef) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let bar_rect = Rect::at(0, 0).of_size(w as u32, h as u32);
    draw_filled_rect_mut(img, bar_rect, Rgba([0u8, 0, 0, 230]));

    let scale = 20.0f32;
    let scale_sm = 13.0f32;
    let white = Rgba([255u8, 255, 255, 255]);
    let gray = Rgba([204u8, 204, 204, 255]);
    let bar_color = Rgba([0u8, 0, 0, 230]);
    let px = 16i32;
    let center_y = h / 2;
    let line_h = 24i32;

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
        draw_text_mut(img, gray, px, p1_rating_y, ab_glyph::PxScale::from(scale_sm), font, r);
    }

    let score_scale = 22.0f32;
    let s1 = overlay.player_one.games_won.to_string();
    let s2 = overlay.player_two.games_won.to_string();
    let race_line2 = format!("{}/{}", overlay.player_one.race_to, overlay.player_two.race_to);

    let (s1_w, s1_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s1);
    let (race2_w, race1_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, &race_line2);
    let race_line1 = "race to";
    let (race1_w, _) = imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, race_line1);
    let (s2_w, s2_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s2);

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

    draw_filled_circle_mut(img, (s1_cx, center_y), circle_r, white);
    draw_filled_circle_mut(img, (s1_cx, center_y), circle_r - 2, bar_color);
    draw_filled_circle_mut(img, (s2_cx, center_y), circle_r, white);
    draw_filled_circle_mut(img, (s2_cx, center_y), circle_r - 2, bar_color);

    let score_y = center_y - s1_h as i32 / 2 - 3;
    let s1_x = s1_cx - s1_w as i32 / 2;
    let s2_x = s2_cx - s2_w as i32 / 2;
    draw_text_mut(img, white, s1_x, score_y, ab_glyph::PxScale::from(score_scale), font, &s1);
    draw_text_mut(img, white, s2_x, center_y - s2_h as i32 / 2 - 3, ab_glyph::PxScale::from(score_scale), font, &s2);

    let race_gap = 2i32;
    let race_block_h = race1_h as i32 + race_gap + race1_h as i32;
    let race_y1 = center_y - race_block_h / 2;
    let race_y2 = race_y1 + race1_h as i32 + race_gap;
    let race_line1_x = race_x + (race_w as i32 - race1_w as i32) / 2;
    let race_line2_x = race_x + (race_w as i32 - race2_w as i32) / 2;
    draw_text_mut(img, gray, race_line1_x, race_y1, ab_glyph::PxScale::from(scale_sm), font, race_line1);
    draw_text_mut(img, gray, race_line2_x, race_y2, ab_glyph::PxScale::from(scale_sm), font, &race_line2);

    let (p2_w, _) = imageproc::drawing::text_size(ab_glyph::PxScale::from(scale), font, &overlay.player_two.name);
    let p2_x = (w - p2_w as i32 - px).max(center_end + 16);
    draw_text_mut(img, white, p2_x, p1_name_y, ab_glyph::PxScale::from(scale), font, &overlay.player_two.name);
    if let Some(ref r) = overlay.player_two.rating {
        draw_text_mut(img, gray, p2_x, p1_rating_y, ab_glyph::PxScale::from(scale_sm), font, r);
    }
}

fn overlay_tmp_path(path: &std::path::Path) -> std::path::PathBuf {
    path.with_file_name("rtmp-overlay.tmp.png")
}

/// Render overlay PNG (match bar only) and write to path.
pub fn render_overlay_png(path: &std::path::Path, overlay: Option<&MatchOverlay>) {
    if let Some(font) = load_font() {
        let mut img = RgbaImage::from_pixel(OVERLAY_WIDTH, OVERLAY_HEIGHT, Rgba([0, 0, 0, 0]));
        if let Some(o) = overlay {
            draw_match_to_rgba(&mut img, o, &font);
        }
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
        }
    } else {
        tracing::warn!("No font for overlay PNG");
    }
}

/// Restore overlay from any active match in the database. Call at server startup.
pub fn restore_overlay_from_db(db: &Db, overlay_state: &OverlayState, rtmp_processes: &rtmp::RtmpState) {
    let cameras = db.list_cameras().ok().unwrap_or_default();
    for camera in cameras {
        if let Some(ref id) = camera.id {
            let has_active_match = db
                .find_active_pool_match_by_camera_id(id)
                .ok()
                .flatten()
                .is_some();
            if (camera.camera_type.is_internal() || camera.camera_type.is_rtsp()) && has_active_match {
                update_overlay(db, overlay_state, id, rtmp_processes, None);
                break;
            }
        }
    }
}

/// Spawn a background task that periodically syncs overlay PNG with DB.
pub fn spawn_overlay_refresh_task(db: Db, overlay_state: OverlayState, _rtmp_processes: rtmp::RtmpState) {
    std::thread::spawn(move || {
        let interval = std::time::Duration::from_secs(2);
        loop {
            std::thread::sleep(interval);
            let cameras = match db.list_cameras() {
                Ok(c) => c,
                Err(_) => continue,
            };
            for camera in cameras {
                if camera.camera_type.is_internal() || camera.camera_type.is_rtsp() {
                    let overlay = overlay_state.read().ok().and_then(|g| (*g).clone());
                    let path = overlay_path_for_camera(&camera.name);
                    render_overlay_png(&path, overlay.as_ref());
                }
            }
        }
    });
}

/// Update the overlay for the camera. Call when match is created/updated.
/// Syncs overlay state and renders PNG. No RTMP restart needed (ffmpeg -reload picks up changes).
pub fn update_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_id: &ObjectId,
    _rtmp_processes: &rtmp::RtmpState,
    overlay_from_match: Option<MatchOverlay>,
) {
    let camera = match db.find_camera_by_id(camera_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(camera_id = %camera_id, "Camera not found by id");
            return;
        }
        Err(e) => {
            tracing::warn!(camera_id = %camera_id, error = %e, "Failed to find camera");
            return;
        }
    };

    if !camera.camera_type.is_internal() && !camera.camera_type.is_rtsp() {
        tracing::debug!(camera_id = %camera_id, "Overlay only applies to internal and RTSP cameras");
        return;
    }

    let overlay = overlay_from_match.or_else(|| {
        db.find_active_pool_match_by_camera_id(camera_id)
            .ok()
            .flatten()
            .filter(|m| m.end_time.is_none())
            .map(|m| MatchOverlay {
                player_one: OverlayPlayer::from_match_player(&m.player_one),
                player_two: OverlayPlayer::from_match_player(&m.player_two),
            })
    });

    if let Ok(mut guard) = overlay_state.write() {
        *guard = overlay.clone();
    }

    let path = overlay_path_for_camera(&camera.name);
    render_overlay_png(&path, overlay.as_ref());
}

/// Clear the overlay (e.g. when match ends).
pub fn clear_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_id: &ObjectId,
    _rtmp_processes: &rtmp::RtmpState,
) {
    let camera = match db.find_camera_by_id(camera_id) {
        Ok(Some(c)) if c.camera_type.is_internal() || c.camera_type.is_rtsp() => c,
        _ => return,
    };
    let current = overlay_state.read().ok().and_then(|g| (*g).clone());
    if current.is_none() {
        tracing::debug!(camera_id = %camera_id, "Overlay already cleared, skipping");
        return;
    }
    if let Ok(mut guard) = overlay_state.write() {
        *guard = None;
    }
    let path = overlay_path_for_camera(&camera.name);
    render_overlay_png(&path, None);
}
