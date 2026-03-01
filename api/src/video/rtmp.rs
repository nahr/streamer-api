//! RTMP streaming pipeline. Spawns ffmpeg to read MJPEG from stream URL and push to RTMP.

use std::sync::Arc;

/// Active RTMP streams: camera_id -> (stop sender, rtmp_url). Send to stop; url used for restart.
pub type RtmpState =
    Arc<std::sync::RwLock<std::collections::HashMap<String, (std::sync::mpsc::Sender<()>, String)>>>;

pub fn rtmp_state_new() -> RtmpState {
    Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()))
}

/// Spawn an ffmpeg process that reads MJPEG from stream_url and pushes to rtmp_url.
/// Overlays the PNG at the given path (match bar only).
/// Runs in a thread; stops when stop_rx receives. Removes from rtmp_processes when done.
/// Requires ffmpeg to be installed (with libx264 and AAC support).
pub fn spawn_rtmp_pipeline(
    stream_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(overlay_path.parent().unwrap_or(std::path::Path::new(".")))
        .map_err(|e| format!("Failed to create data dir: {}", e))?;

    let overlay_path_str = overlay_path
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(overlay_path)))
        .map_err(|e| format!("Overlay path error: {}", e))?
        .to_string_lossy()
        .into_owned();

    tracing::info!(overlay_path = %overlay_path_str, "RTMP: ffmpeg PNG overlay");

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

#[derive(serde::Deserialize)]
pub struct RtmpStartRequest {
    pub url: String,
}
