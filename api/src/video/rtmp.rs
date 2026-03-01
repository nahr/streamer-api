//! RTMP streaming pipeline. Spawns ffmpeg to read MJPEG from stream URL and push to RTMP.
//! On macOS, uses FFmpeg direct capture (avfoundation + videotoolbox) for better framerate.
//! When running in container, adds drawtext overlay: location (top left), camera name (underneath), time (top right).

use std::sync::Arc;

/// Escape text for ffmpeg drawtext filter (handles ', \, :).
fn escape_drawtext(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
}

/// Active RTMP streams: camera_id -> (stop sender, rtmp_url). Send to stop; url used for restart.
pub type RtmpState =
    Arc<std::sync::RwLock<std::collections::HashMap<String, (std::sync::mpsc::Sender<()>, String)>>>;

pub fn rtmp_state_new() -> RtmpState {
    Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()))
}

/// Spawn RTMP pipeline. On macOS uses FFmpeg direct capture (avfoundation + videotoolbox) for
/// internal cameras. Use `use_mjpeg_input: true` for RTSP cameras (reads from stream URL on all platforms).
/// When running in container, draws location (top left), camera name (underneath), and time (top right).
pub fn spawn_rtmp_pipeline(
    stream_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
    camera_index: u32,
    use_mjpeg_input: bool,
    location_name: &str,
    camera_name: &str,
) -> Result<(), String> {
    if use_mjpeg_input {
        spawn_rtmp_pipeline_mjpeg(
            stream_url,
            rtmp_url,
            stop_rx,
            rtmp,
            id,
            overlay_path,
            location_name,
            camera_name,
        )
    } else {
        #[cfg(target_os = "macos")]
        {
            let _ = stream_url;
            spawn_rtmp_pipeline_direct(
                rtmp_url,
                stop_rx,
                rtmp,
                id,
                overlay_path,
                camera_index,
                location_name,
                camera_name,
            )
        }

        #[cfg(not(target_os = "macos"))]
        {
            spawn_rtmp_pipeline_mjpeg(
                stream_url,
                rtmp_url,
                stop_rx,
                rtmp,
                id,
                overlay_path,
                location_name,
                camera_name,
            )
        }
    }
}

/// FFmpeg direct capture via avfoundation + videotoolbox (macOS).
#[cfg(target_os = "macos")]
fn spawn_rtmp_pipeline_direct(
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
    camera_index: u32,
    _location_name: &str,
    _camera_name: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(overlay_path.parent().unwrap_or(std::path::Path::new(".")))
        .map_err(|e| format!("Failed to create data dir: {}", e))?;

    let overlay_path_str = overlay_path
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(overlay_path)))
        .map_err(|e| format!("Overlay path error: {}", e))?
        .to_string_lossy()
        .into_owned();

    tracing::info!(
        overlay_path = %overlay_path_str,
        camera_index = camera_index,
        "RTMP: ffmpeg avfoundation + videotoolbox (direct capture)"
    );

    // avfoundation: video only (:none = no audio). videotoolbox: hardware H.264 encode.
    let mut child = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            // ========== AUDIO SOURCE ==========
            "-f",
            "lavfi",
            "-i",
            "anullsrc=channel_layout=stereo:sample_rate=48000",
            // ========== VIDEO INPUT (avfoundation direct) ==========
            "-f",
            "avfoundation",
            "-framerate",
            "30",
            "-video_size",
            "1280x720",
            "-pixel_format",
            "uyvy422",
            "-video_device_index",
            &camera_index.to_string(),
            "-i",
            "0:none",
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
            // ========== VIDEO ENCODER (videotoolbox hardware) ==========
            "-c:v",
            "h264_videotoolbox",
            "-b:v",
            "2000k",
            "-profile:v",
            "high",
            "-level",
            "4.1",
            "-g",
            "60",
            "-keyint_min",
            "60",
            // ========== AUDIO ENCODER ==========
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ar",
            "48000",
            "-ac",
            "2",
            // ========== MUX (flvflags helps RTMPS/Facebook) ==========
            "-flvflags",
            "no_duration_filesize",
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

/// Spawn an ffmpeg process that reads MJPEG from stream_url and pushes to rtmp_url.
/// Used for RTSP cameras (all platforms) and internal camera on Linux.
fn spawn_rtmp_pipeline_mjpeg(
    stream_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
    location_name: &str,
    camera_name: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(overlay_path.parent().unwrap_or(std::path::Path::new(".")))
        .map_err(|e| format!("Failed to create data dir: {}", e))?;

    let overlay_path_str = overlay_path
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(overlay_path)))
        .map_err(|e| format!("Overlay path error: {}", e))?
        .to_string_lossy()
        .into_owned();

    let encoder = if cfg!(target_os = "macos") {
        "h264_videotoolbox"
    } else {
        "libx264"
    };
    tracing::info!(overlay_path = %overlay_path_str, encoder = %encoder, "RTMP: ffmpeg PNG overlay");

    // Build filter: overlay + optional drawtext when in container
    let in_container = std::path::Path::new("/.dockerenv").exists();
    let filter_complex = if in_container {
        let font = "fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        let base = "fontsize=20:fontcolor=white";
        let mut parts: Vec<String> = vec![];
        if !location_name.is_empty() {
            parts.push(format!(
                "drawtext=text='{}':x=10:y=10:{}:{}",
                escape_drawtext(location_name),
                base,
                font
            ));
        }
        if !camera_name.is_empty() {
            let y = if location_name.is_empty() { 10 } else { 35 };
            parts.push(format!(
                "drawtext=text='{}':x=10:y={}:{}:{}",
                escape_drawtext(camera_name),
                y,
                base,
                font
            ));
        }
        // Time at top right (always shown in container)
        parts.push(format!(
            "drawtext=text='%{{localtime\\:%H\\:%M\\:%S}}':x=w-text_w-10:y=10:{}:{}",
            base, font
        ));
        let drawtext = parts.join(",");
        format!(
            "[1:v]fps=30[main];[main][2:v]overlay=0:H-80,{},format=yuv420p[out]",
            drawtext
        )
    } else {
        "[1:v]fps=30[main];[main][2:v]overlay=0:H-80,format=yuv420p[out]".into()
    };

    // Common args: inputs and filter
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-f".into(), "lavfi".into(),
        "-i".into(), "anullsrc=channel_layout=stereo:sample_rate=48000".into(),
        "-f".into(), "mjpeg".into(),
        "-r".into(), "30".into(),
        "-i".into(), stream_url.to_string(),
        "-f".into(), "image2".into(),
        "-loop".into(), "1".into(),
        "-i".into(), overlay_path_str.clone(),
        "-filter_complex".into(),
        filter_complex,
        "-map".into(), "[out]".into(),
        "-map".into(), "0:a".into(),
        "-c:v".into(), encoder.into(),
        "-b:v".into(), "2000k".into(),
        "-profile:v".into(), "high".into(),
        "-level".into(), "4.1".into(),
        "-g".into(), "60".into(),
        "-keyint_min".into(), "60".into(),
        "-c:a".into(), "aac".into(),
        "-b:a".into(), "128k".into(),
        "-ar".into(), "48000".into(),
        "-ac".into(), "2".into(),
        "-flvflags".into(), "no_duration_filesize".into(),
        "-f".into(), "flv".into(),
        rtmp_url.to_string(),
    ];

    // Platform-specific encoder options
    if cfg!(target_os = "macos") {
        // videotoolbox: minimal options, hardware handles the rest
    } else {
        // libx264: ultrafast for lowest CPU, zerolatency for live
        let insert_idx = args.iter().position(|a| a == "-keyint_min").unwrap() + 2;
        args.splice(insert_idx..insert_idx, [
            "-preset".into(), "ultrafast".into(),
            "-tune".into(), "zerolatency".into(),
            "-pix_fmt".into(), "yuv420p".into(),
            "-maxrate".into(), "2000k".into(),
            "-bufsize".into(), "4000k".into(),
            "-x264-params".into(), "scenecut=0:open_gop=0:min-keyint=60".into(),
            "-bf".into(), "2".into(),
            "-fps_mode".into(), "cfr".into(),
        ]);
    }

    let mut child = std::process::Command::new("ffmpeg")
        .args(args)
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
