//! Video streaming: camera sources, overlays, and RTMP export.

use bytes::Bytes;
use tokio::sync::broadcast;

mod mediamtx;
mod mjpeg;
mod recording;
mod overlay;
mod rtsp_camera;
mod rtmp;
mod stream;

pub use recording::recording_download;
pub use stream::{
    camera_stream, camera_stream_rtmp_start, camera_stream_rtmp_stop, camera_stream_rtmp_status,
};
pub use overlay::{
    clear_overlay, overlay_path_for_camera, restore_overlay_from_db, spawn_overlay_refresh_task,
    update_overlay, MatchOverlay, OverlayPlayer, OverlayState,
};
pub use mediamtx::{
    delete_camera_path, fetch_camera_connection_status, finish_recording_segment, is_available,
    mediamtx_rtsp_url, sync_all_paths, sync_camera_path,
};
pub use rtmp::{rtmp_state_new, RtmpStartRequest, RtmpState};

/// Trait for camera sources that provide a video stream.
/// Implementations grab frames and broadcast them to subscribers.
pub trait CameraSource: Send + Sync {
    /// Subscribe to receive MJPEG frame bytes from the stream.
    fn subscribe(&self) -> broadcast::Receiver<Bytes>;
}
