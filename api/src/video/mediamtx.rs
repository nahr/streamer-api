//! MediaMTX Control API client. Syncs camera paths for rolling recording and proxy.
//! FFmpeg/stream consumers read from MediaMTX (rtsp://host:8554/camera/{id}) instead of
//! the camera directly, so the camera has a single connection and isn't overloaded.

use crate::db::camera::CameraDoc;
use crate::db::settings::SettingsDoc;
use crate::error::ApiError;

/// MediaMTX Control API base URL.
fn mediamtx_api_base() -> String {
    crate::config::config().mediamtx_api_url.clone()
}

/// RTSP URL to read from MediaMTX for a camera. Used when MediaMTX is available.
pub fn mediamtx_rtsp_url(camera_id: &str) -> String {
    let cfg = crate::config::config();
    format!(
        "rtsp://{}:{}/camera/{}",
        cfg.mediamtx_rtsp_host,
        cfg.mediamtx_rtsp_port,
        camera_id
    )
}

/// Path name in MediaMTX for a camera.
fn path_name(camera_id: &str) -> String {
    format!("camera/{}", camera_id)
}

/// PathConf for MediaMTX Control API (subset we use).
/// MediaMTX expects camelCase field names in JSON.
#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct PathConf {
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_segment_duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_delete_after: Option<String>,
}

/// Add or replace a camera path in MediaMTX.
pub async fn sync_camera_path(
    camera: &CameraDoc,
    settings: &SettingsDoc,
) -> Result<(), ApiError> {
    let rtsp_url = camera
        .camera_type
        .rtsp_url()
        .filter(|u| !u.trim().is_empty());
    let rtsp_url = match rtsp_url {
        Some(u) => u.trim().to_string(),
        None => return Ok(()),
    };

    let camera_id = camera.id.as_deref().unwrap_or("");
    if camera_id.is_empty() {
        return Ok(());
    }

    let path = path_name(camera_id);

    let mut record_path = settings.record_path.trim().to_string();
    if record_path.is_empty() {
        record_path = "./recordings/%path/%Y-%m-%d_%H-%M-%S-%f".to_string();
    } else {
        // Ensure %path is available for per-camera subdirs
        if !record_path.contains("%path") {
            record_path = format!("{}/%path/%Y-%m-%d_%H-%M-%S-%f", record_path.trim_end_matches('/'));
        }
    }

    let record_delete_after = settings.record_delete_after.trim();
    let record_delete_after = if record_delete_after.is_empty() || record_delete_after == "0" {
        "0s".to_string() // Keep forever
    } else {
        record_delete_after.to_string()
    };

    let conf = PathConf {
        source: Some(rtsp_url),
        record: Some(true),
        record_path: Some(record_path),
        record_format: Some("fmp4".to_string()),
        record_segment_duration: Some(
            settings
                .record_segment_duration
                .trim()
                .is_empty()
                .then(|| "1m".to_string())
                .unwrap_or_else(|| settings.record_segment_duration.trim().to_string()),
        ),
        record_delete_after: Some(record_delete_after),
        ..Default::default()
    };

    let base = mediamtx_api_base();
    let url = format!("{}/v3/config/paths/replace/{}", base, urlencoding::encode(&path));

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .json(&conf)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX API request failed: {}", e)))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        tracing::warn!(
            path = %path,
            status = %status,
            body = %body,
            "MediaMTX path replace failed"
        );
        return Err(ApiError::Unknown(format!(
            "MediaMTX path replace failed: {} {}",
            status, body
        )));
    }

    tracing::info!(path = %path, "MediaMTX path synced");
    Ok(())
}

/// Finish the current recording segment and start a new one. Called when score changes during a match
/// so segment boundaries align with game events. Toggles record off then on via PATCH.
pub async fn finish_recording_segment(camera_id: &str) -> Result<(), ApiError> {
    if camera_id.is_empty() {
        return Ok(());
    }

    let path = path_name(camera_id);
    let base = mediamtx_api_base();
    let url = format!("{}/v3/config/paths/patch/{}", base, urlencoding::encode(&path));

    let client = reqwest::Client::new();

    // Stop recording to close current segment
    let patch_off = serde_json::json!({ "record": false });
    let res = client
        .patch(&url)
        .json(&patch_off)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX API request failed: {}", e)))?;
    if !res.status().is_success() && res.status().as_u16() != 404 {
        let body = res.text().await.unwrap_or_default();
        tracing::debug!(path = %path, "MediaMTX record off failed: {}", body);
        return Ok(()); // Non-fatal: path might not exist or MediaMTX unavailable
    }

    // Brief delay to let MediaMTX flush and close the segment
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Resume recording
    let patch_on = serde_json::json!({ "record": true });
    let res = client
        .patch(&url)
        .json(&patch_on)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX API request failed: {}", e)))?;
    if !res.status().is_success() && res.status().as_u16() != 404 {
        let body = res.text().await.unwrap_or_default();
        tracing::debug!(path = %path, "MediaMTX record on failed: {}", body);
    }

    tracing::debug!(path = %path, "MediaMTX segment finished");
    Ok(())
}

/// Delete a camera path from MediaMTX.
pub async fn delete_camera_path(camera_id: &str) -> Result<(), ApiError> {
    if camera_id.is_empty() {
        return Ok(());
    }

    let path = path_name(camera_id);
    let base = mediamtx_api_base();
    let url = format!("{}/v3/config/paths/delete/{}", base, urlencoding::encode(&path));

    let client = reqwest::Client::new();
    let res = client
        .delete(&url)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX API request failed: {}", e)))?;

    // 404 is ok - path might not exist
    if res.status().as_u16() == 404 {
        return Ok(());
    }
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        tracing::warn!(
            path = %path,
            status = %status,
            body = %body,
            "MediaMTX path delete failed"
        );
        return Err(ApiError::Unknown(format!(
            "MediaMTX path delete failed: {} {}",
            status, body
        )));
    }

    tracing::info!(path = %path, "MediaMTX path deleted");
    Ok(())
}

/// Check if MediaMTX API is reachable.
pub async fn is_available() -> bool {
    let base = mediamtx_api_base();
    let url = format!("{}/v3/info", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    client.get(&url).send().await.is_ok()
}

/// Path entry from MediaMTX /v3/paths/list.
#[derive(serde::Deserialize)]
struct PathItem {
    name: Option<String>,
    ready: Option<bool>,
}

/// Path list response from MediaMTX.
#[derive(serde::Deserialize)]
struct PathListResponse {
    items: Option<Vec<PathItem>>,
}

/// Fetch camera connection status from MediaMTX. Returns map of camera_id -> ready.
pub async fn fetch_camera_connection_status(
) -> Result<std::collections::HashMap<String, bool>, ApiError> {
    let base = mediamtx_api_base();
    let url = format!("{}/v3/paths/list", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| ApiError::Unknown(e.to_string()))?;

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX paths list failed: {}", e)))?;

    if !res.status().is_success() {
        return Err(ApiError::Unknown(format!(
            "MediaMTX paths list failed: {}",
            res.status()
        )));
    }

    let body: PathListResponse = res
        .json()
        .await
        .map_err(|e| ApiError::Unknown(format!("MediaMTX paths list parse failed: {}", e)))?;

    let mut status = std::collections::HashMap::new();
    for item in body.items.unwrap_or_default() {
        if let Some(name) = item.name {
            if let Some(camera_id) = name.strip_prefix("camera/") {
                status.insert(camera_id.to_string(), item.ready.unwrap_or(false));
            }
        }
    }
    Ok(status)
}

/// Sync all RTSP cameras to MediaMTX. Called on startup and when settings change.
pub async fn sync_all_paths(db: &crate::db::Db) -> bool {
    if !is_available().await {
        tracing::debug!("MediaMTX not available, skipping path sync");
        return false;
    }

    let settings = match db.get_settings() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get settings for MediaMTX sync");
            return false;
        }
    };

    let cameras = match db.list_cameras() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list cameras for MediaMTX sync");
            return false;
        }
    };

    let mut ok = true;
    for camera in &cameras {
        if !camera.camera_type.is_rtsp() {
            continue;
        }
        if let Err(e) = sync_camera_path(camera, &settings).await {
            tracing::warn!(camera_id = ?camera.id, error = %e, "MediaMTX sync failed for camera");
            ok = false;
        }
    }

    ok
}
