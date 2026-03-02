use axum::{extract::State, routing::get, Json};
use serde::Deserialize;

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::db::settings::SettingsDoc;
use crate::error::ApiError;

/// GET /api/settings - Returns system settings.
pub async fn get_settings(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
) -> Result<Json<SettingsDoc>, ApiError> {
    let settings = app.db.get_settings()?;
    Ok(Json(settings))
}

#[derive(Deserialize)]
pub struct SettingsUpdateRequest {
    pub location_name: Option<String>,
    pub record_path: Option<String>,
    pub record_segment_duration: Option<String>,
    pub record_delete_after: Option<String>,
}

/// PUT /api/settings - Update system settings.
pub async fn put_settings(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Json(req): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsDoc>, ApiError> {
    let record_settings_changed = req.record_path.is_some()
        || req.record_segment_duration.is_some()
        || req.record_delete_after.is_some();

    let mut current = app.db.get_settings()?;
    if let Some(location_name) = req.location_name {
        current.location_name = location_name;
    }
    if let Some(record_path) = req.record_path {
        current.record_path = record_path;
    }
    if let Some(record_segment_duration) = req.record_segment_duration {
        current.record_segment_duration = record_segment_duration;
    }
    if let Some(record_delete_after) = req.record_delete_after {
        current.record_delete_after = record_delete_after;
    }
    app.db.set_settings(current.clone())?;

    // Re-sync MediaMTX paths in background when record settings change
    if record_settings_changed {
        let db = app.db.clone();
        tokio::spawn(async move {
            let _ = crate::video::sync_all_paths(&db).await;
        });
    }

    Ok(Json(current))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/settings", get(get_settings).put(put_settings))
}
