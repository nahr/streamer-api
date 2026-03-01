//! RTSP camera streaming via FFmpeg. Reads from rtsp:// URL and outputs MJPEG.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

use crate::video::CameraSource;

/// Shared state for an RTSP camera stream.
pub struct RtspCameraState {
    pub tx: broadcast::Sender<bytes::Bytes>,
}

impl CameraSource for RtspCameraState {
    fn subscribe(&self) -> broadcast::Receiver<bytes::Bytes> {
        self.tx.subscribe()
    }
}

/// Parse MJPEG stream from FFmpeg stdout into individual JPEG frames.
fn extract_jpeg_frames(mut reader: ChildStdout, tx: broadcast::Sender<bytes::Bytes>) {
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
                            let _ = tx.send(bytes::Bytes::from(jpeg));
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

/// Spawn FFmpeg to read from RTSP URL and output MJPEG to stdout.
fn spawn_rtsp_ffmpeg(rtsp_url: &str) -> Option<(Child, broadcast::Sender<bytes::Bytes>)> {
    let child = Command::new("ffmpeg")
        .args([
            "-y",
            "-rtsp_transport", "tcp",
            "-i", rtsp_url,
            "-f", "mjpeg",
            "-q:v", "5",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    match child {
        Ok(mut c) => {
            if let Some(stdout) = c.stdout.take() {
                let (tx, _) = broadcast::channel(16);
                let tx_clone = tx.clone();
                std::thread::spawn(move || extract_jpeg_frames(stdout, tx_clone));
                Some((c, tx))
            } else {
                None
            }
        }
        Err(e) => {
            tracing::warn!(url = %rtsp_url, "FFmpeg RTSP capture failed: {}", e);
            None
        }
    }
}

/// Global registry of active RTSP streams. Key: camera_id.
static RTSP_STREAMS: std::sync::LazyLock<RwLock<HashMap<String, Arc<RtspCameraState>>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Get or create RTSP stream for the given camera. Returns the broadcast sender's state.
pub fn get_or_start_rtsp_stream(camera_id: &str, rtsp_url: &str) -> Option<Arc<RtspCameraState>> {
    {
        let guard = RTSP_STREAMS.read().unwrap();
        if let Some(state) = guard.get(camera_id) {
            return Some(Arc::clone(state));
        }
    }

    let (mut child, tx) = spawn_rtsp_ffmpeg(rtsp_url)?;
    let state = Arc::new(RtspCameraState { tx: tx.clone() });
    let camera_id = camera_id.to_string();

    {
        let mut guard = RTSP_STREAMS.write().unwrap();
        guard.insert(camera_id.clone(), Arc::clone(&state));
    }

    // Spawn a task to remove from registry when FFmpeg exits
    std::thread::spawn(move || {
        let _ = child.wait();
        RTSP_STREAMS.write().unwrap().remove(&camera_id);
        tracing::debug!(camera_id = %camera_id, "RTSP stream ended");
    });

    Some(state)
}
