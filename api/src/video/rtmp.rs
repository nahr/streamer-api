//! RTMP streaming pipeline. Spawns ffmpeg to read directly from RTSP URL and push to RTMP.
//! When running in container, adds drawtext overlay: location (top left), camera name (underneath), time (top right).

use std::path::Path;
use std::sync::Arc;

/// Resolve overlay path for ffmpeg input. Creates parent dir and returns canonical path string.
fn resolve_overlay_path(overlay_path: &Path) -> Result<String, String> {
    std::fs::create_dir_all(overlay_path.parent().unwrap_or(Path::new(".")))
        .map_err(|e| format!("Failed to create data dir: {}", e))?;

    let path = overlay_path
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(overlay_path)))
        .map_err(|e| format!("Overlay path error: {}", e))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Gracefully terminate ffmpeg so it can close the RTMP stream properly.
/// Sends SIGINT (allows ffmpeg to flush and close the RTMP connection),
/// waits up to 10 seconds, then SIGKILL if still running.
#[cfg(unix)]
fn terminate_ffmpeg(child: &mut std::process::Child, id: &str) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let pid = child.id();
    if kill(Pid::from_raw(pid as i32), Signal::SIGINT).is_ok() {
        tracing::info!(camera_id = %id, "RTMP: sent SIGINT for graceful shutdown");
    }

    for _ in 0..100 {
        if child.try_wait().ok().flatten().is_some() {
            tracing::info!(camera_id = %id, "RTMP: ffmpeg exited gracefully");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let _ = child.kill();
    let _ = child.wait();
    tracing::warn!(camera_id = %id, "RTMP: ffmpeg did not exit gracefully, force-killed");
}

#[cfg(not(unix))]
fn terminate_ffmpeg(child: &mut std::process::Child, _id: &str) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Spawn threads to capture stderr and monitor ffmpeg until stop signal or exit; then cleanup from rtmp registry.
/// When stop is received, sends 'q' to stdin first (most graceful - lets ffmpeg flush FLV metadata),
/// then falls back to SIGINT if needed.
fn spawn_ffmpeg_monitor(
    mut child: std::process::Child,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
) {
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
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

        // Try graceful shutdown via stdin 'q' first - allows ffmpeg to flush and close FLV properly.
        // This is critical for correct saved videos on Facebook (duration, metadata).
        if let Some(mut stdin) = stdin {
            if let Err(e) = std::io::Write::write_all(&mut stdin, b"q") {
                tracing::debug!(camera_id = %id, error = %e, "RTMP: could not send 'q' to stdin");
            } else {
                tracing::info!(camera_id = %id, "RTMP: sent 'q' for graceful shutdown");
            }
            drop(stdin);
            for _ in 0..100 {
                if child.try_wait().ok().flatten().is_some() {
                    tracing::info!(camera_id = %id, "RTMP: ffmpeg exited gracefully");
                    rtmp.write().unwrap().remove(&id);
                    tracing::info!(camera_id = %id, "RTMP: ffmpeg pipeline ended");
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        terminate_ffmpeg(&mut child, &id);
        rtmp.write().unwrap().remove(&id);
        tracing::info!(camera_id = %id, "RTMP: ffmpeg pipeline ended");
    });
}

/// Escape text for ffmpeg drawtext filter (handles ', \, :).
fn escape_drawtext(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
}

/// Build filter_complex: overlay below video (vstack) + optional drawtext when in container.
/// Overlay PNG is placed BELOW the video, increasing output height by 80px.
/// Input 0: RTSP (video+audio), Input 1: anullsrc, Input 2: overlay.
fn build_filter_complex(location_name: &str, camera_name: &str) -> String {
    // Scale to 960x540 (qHD) for lighter encoding; overlay is 1280 wide so scale overlay to match.
    // fps=30:round=near for smoother frame timing from variable-rate RTSP
    // loop=-1:1:0 makes overlay infinite (image2 reports 1-frame duration, which would limit vstack)
    let base_filter = "[0:v]fps=30:round=near,scale=960:540[main];[2:v]loop=-1:1:0,scale=960:80,format=yuv420p[overlay];[main][overlay]vstack=inputs=2,format=yuv420p[out]";
    if !std::path::Path::new("/.dockerenv").exists() {
        return base_filter.to_string();
    }
    let font = "fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
    let base = "fontsize=20:fontcolor=white";
    let mut parts: Vec<String> = vec![];
    if !location_name.is_empty() {
        parts.push(format!(
            "drawtext=text='{}':x=10:y=10:{}:{}",
            escape_drawtext(location_name), base, font
        ));
    }
    if !camera_name.is_empty() {
        let y = if location_name.is_empty() { 10 } else { 35 };
        parts.push(format!(
            "drawtext=text='{}':x=10:y={}:{}:{}",
            escape_drawtext(camera_name), y, base, font
        ));
    }
    parts.push(format!(
        "drawtext=text='%{{localtime\\:%H\\:%M\\:%S}}':x=w-text_w-10:y=10:{}:{}",
        base, font
    ));
    let drawtext = parts.join(",");
    // Drawtext on main before vstack, then stack overlay below.
    format!(
        "[0:v]fps=30:round=near,scale=960:540,{},format=yuv420p[main];[2:v]loop=-1:1:0,scale=960:80,format=yuv420p[overlay];[main][overlay]vstack=inputs=2,format=yuv420p[out]",
        drawtext
    )
}

/// Shared FFmpeg args for RTMP output. Input 0: RTSP (video+audio), Input 1: anullsrc, Input 2: overlay.
/// Stream selection: -map 0:a uses RTSP audio; anullsrc is fallback when RTSP has no audio.
fn build_rtmp_args(
    video_input: &[&str],
    overlay_path: &str,
    filter_complex: &str,
    encoder: &str,
    encoder_extra: &[&str],
    rtmp_url: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec!["-y".into()];
    args.extend(video_input.iter().map(|s| s.to_string()));
    args.extend([
        "-f".into(), "lavfi".into(),
        "-i".into(), "anullsrc=channel_layout=stereo:sample_rate=48000".into(),
        "-f".into(), "image2".into(),
        "-loop".into(), "1".into(),
        "-r".into(), "30".into(),  // Match video fps so vstack gets continuous overlay frames
        "-i".into(), overlay_path.to_string(),
        "-filter_complex".into(), filter_complex.to_string(),
        "-map".into(), "[out]".into(),
        "-map".into(), "0:a".into(),  // RTSP audio (camera mic); anullsrc fallback when absent
        "-c:v".into(), encoder.to_string(),
    ]);
    args.extend(encoder_extra.iter().map(|s| s.to_string()));
    args.extend([
        "-b:v".into(), "2500k".into(),
        "-profile:v".into(), "high".into(),
        "-level".into(), "4.1".into(),
        "-g".into(), "60".into(),
        "-keyint_min".into(), "60".into(),
        "-c:a".into(), "aac".into(),
        "-b:a".into(), "128k".into(),
        "-ar".into(), "48000".into(),
        "-ac".into(), "2".into(),
        "-muxpreload".into(), "0".into(),
        "-muxdelay".into(), "0".into(),
        "-avoid_negative_ts".into(), "make_zero".into(),
        "-flvflags".into(), "no_duration_filesize".into(),
        "-f".into(), "flv".into(),
        rtmp_url.to_string(),
    ]);
    args
}

/// Active RTMP streams: camera_id -> (stop sender, rtmp_url). Send to stop; url used for restart.
pub type RtmpState =
    Arc<std::sync::RwLock<std::collections::HashMap<String, (std::sync::mpsc::Sender<()>, String)>>>;

pub fn rtmp_state_new() -> RtmpState {
    Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()))
}

/// Rewrite rtmps:// URL to rtmp://host:19350 when USE_STUNNEL_FOR_RTMPS=1.
/// FFmpeg's native RTMPS often fails with "Input/output error"; stunnel works around this.
/// STUNNEL_HOST defaults to 127.0.0.1 (same host/container); override if stunnel runs elsewhere.
fn maybe_rewrite_rtmps_for_stunnel(url: &str) -> String {
    if std::env::var("USE_STUNNEL_FOR_RTMPS").as_deref() != Ok("1") {
        return url.to_string();
    }
    if !url.starts_with("rtmps://") {
        return url.to_string();
    }
    let host = std::env::var("STUNNEL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    // rtmps://host:port/path -> rtmp://STUNNEL_HOST:19350/path
    if let Some(path_start) = url[8..].find('/') {
        let path = &url[8 + path_start..];
        format!("rtmp://{}:19350{}", host, path)
    } else {
        url.to_string()
    }
}

/// Spawn RTMP pipeline. Reads directly from RTSP URL and pushes to RTMP.
/// When running in container, draws location (top left), camera name (underneath), and time (top right).
pub fn spawn_rtmp_pipeline(
    rtsp_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
    overlay_path: &std::path::Path,
    location_name: &str,
    camera_name: &str,
) -> Result<(), String> {
    let overlay_path_str = resolve_overlay_path(overlay_path)?;

    let (encoder, encoder_extra) = if cfg!(target_os = "macos") {
        // Use libx264 on macOS; VideoToolbox has frame buffering that can throttle output
        (
            "libx264",
            [
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-pix_fmt", "yuv420p",
                "-b:v", "2500k",
                "-maxrate", "2500k",
                "-bufsize", "5000k",
                "-x264-params", "scenecut=0:open_gop=0:min-keyint=60",
                "-sc_threshold", "0",
                "-bf", "0",
                "-fps_mode", "cfr",
            ]
            .as_slice(),
        )
    } else {
        (
            "libx264",
            [
                "-preset", "ultrafast",  // Prioritize real-time over quality to avoid choppiness
                "-tune", "zerolatency",
                "-pix_fmt", "yuv420p",
                "-b:v", "2500k",
                "-maxrate", "2500k",
                "-bufsize", "5000k",
                "-x264-params", "scenecut=0:open_gop=0:min-keyint=60",
                "-sc_threshold", "0",
                "-bf", "0",  // No B-frames for lower latency
                "-fps_mode", "cfr",
            ]
            .as_slice(),
        )
    };
    tracing::info!(overlay_path = %overlay_path_str, encoder = %encoder, "RTMP: ffmpeg PNG overlay");

    // Read directly from RTSP. UDP often performs better than TCP for live cameras.
    let video_input = [
        "-rtsp_transport", "udp",
        "-fflags", "nobuffer",
        "-flags", "low_delay",
        "-analyzeduration", "1000000",
        "-probesize", "1000000",
        "-i", rtsp_url,
    ];
    let filter = build_filter_complex(location_name, camera_name);
    let output_url = maybe_rewrite_rtmps_for_stunnel(rtmp_url);
    if output_url != rtmp_url {
        tracing::info!("RTMP: using stunnel relay (USE_STUNNEL_FOR_RTMPS=1)");
    }
    let args = build_rtmp_args(
        &video_input,
        &overlay_path_str,
        &filter,
        encoder,
        encoder_extra,
        &output_url,
    );

    let child = std::process::Command::new("ffmpeg")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}. Ensure ffmpeg is installed.", e))?;

    spawn_ffmpeg_monitor(child, stop_rx, rtmp, id);

    Ok(())
}

#[derive(serde::Deserialize)]
pub struct RtmpStartRequest {
    pub url: String,
}
