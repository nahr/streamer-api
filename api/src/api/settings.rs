use axum::{extract::State, routing::get, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::db::settings::SettingsDoc;
use crate::error::ApiError;

/// GET /api/settings - Returns system settings.
pub async fn get_settings(
    State(app): State<AppState>,
) -> Result<Json<SettingsDoc>, ApiError> {
    let settings = app.db.get_settings()?;
    Ok(Json(settings))
}

#[derive(Deserialize)]
pub struct SettingsUpdateRequest {
    pub location_name: Option<String>,
}

/// PUT /api/settings - Update system settings.
pub async fn put_settings(
    State(app): State<AppState>,
    Json(req): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsDoc>, ApiError> {
    let mut current = app.db.get_settings()?;
    if let Some(location_name) = req.location_name {
        current.location_name = location_name;
    }
    app.db.set_settings(current.clone())?;
    Ok(Json(current))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/settings", get(get_settings).put(put_settings))
}
