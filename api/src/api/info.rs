use axum::{extract::State, routing::get};
use serde::{Deserialize, Serialize};

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiServerInfo {
    pub initialized: bool,
    #[serde(default)]
    pub location_name: String,
}

/// GET /api/info - Returns server info. `initialized` is false if no admin exists yet.
pub async fn info(State(app): State<AppState>) -> Result<axum::Json<ApiServerInfo>, ApiError> {
    let initialized = app.db.has_admin()?;
    let settings = app.db.get_settings().unwrap_or_default();
    Ok(axum::Json(ApiServerInfo {
        initialized,
        location_name: settings.location_name,
    }))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/info", get(info))
}
