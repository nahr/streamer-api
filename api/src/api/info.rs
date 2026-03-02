use axum::{extract::State, routing::get};
use serde::{Deserialize, Serialize};

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiServerInfo {
    pub initialized: bool,
    #[serde(default)]
    pub location_name: String,
    /// True if at least one user has registered (signed in).
    #[serde(default)]
    pub has_users: bool,
    /// True if at least one camera is configured.
    #[serde(default)]
    pub cameras_configured: bool,
    /// Recording retention (e.g. "24h", "7d"). Empty or "0" = keep forever.
    #[serde(default)]
    pub record_delete_after: String,
}

/// GET /api/info - Returns server info. `initialized` is true when Auth0 is configured (no registration gate).
pub async fn info(State(app): State<AppState>) -> Result<axum::Json<ApiServerInfo>, ApiError> {
    let initialized = app.jwks.is_some();
    let settings = app.db.get_settings().unwrap_or_default();
    let has_users = app.db.has_admin().unwrap_or(false);
    let cameras_configured = app.db.cameras_configured().unwrap_or(false);
    Ok(axum::Json(ApiServerInfo {
        initialized,
        location_name: settings.location_name,
        has_users,
        cameras_configured,
        record_delete_after: settings.record_delete_after,
    }))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/info", get(info))
}
