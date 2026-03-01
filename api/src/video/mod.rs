//! Video streaming: camera sources, overlays, and RTMP export.

use bytes::Bytes;
use tokio::sync::broadcast;

mod internal_camera;
mod overlay;
mod rtmp;

pub use internal_camera::{
    camera_stream, camera_stream_rtmp_start, camera_stream_rtmp_stop, camera_stream_rtmp_status,
    ensure_internal_camera_ready,
};
pub use overlay::{
    clear_overlay, overlay_path_for_camera, restore_overlay_from_db, spawn_overlay_refresh_task,
    update_overlay, MatchOverlay, OverlayPlayer, OverlayState,
};
pub use rtmp::{rtmp_state_new, RtmpStartRequest, RtmpState};

/// Trait for camera sources that provide a video stream.
/// Implementations grab frames and broadcast them to subscribers.
pub trait CameraSource: Send + Sync {
    /// Subscribe to receive MJPEG frame bytes from the stream.
    fn subscribe(&self) -> broadcast::Receiver<Bytes>;
}
