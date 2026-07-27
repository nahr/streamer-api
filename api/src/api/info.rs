use axum::{extract::State, routing::get};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiServerInfo {
    pub initialized: bool,
    pub version: String,
    pub candidate_version: String,
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

/// GET /api/info - Returns server info.
pub async fn info(State(app): State<AppState>) -> Result<axum::Json<ApiServerInfo>, ApiError> {
    let settings = app.db.get_settings().unwrap_or_default();
    let has_users = app.db.has_admin().unwrap_or(false);
    let cameras_configured = app.db.cameras_configured().unwrap_or(false);

    let mut version = env!("CARGO_PKG_VERSION").to_string();
    let mut candidate_version = version.clone();

    let output = Command::new("apt-cache")
        .arg("policy")
        .arg(env!("CARGO_PKG_NAME"))
        .output()
        .await;

    let pkg_version = env!("CARGO_PKG_VERSION");
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.trim().starts_with("Candidate:") {
                let v = line.split_whitespace().nth(1).unwrap_or("").to_string();
                candidate_version = if v.is_empty() || v == "(none)" { pkg_version.to_string() } else { v };
            } else if line.trim().starts_with("Installed") {
                let v = line.split_whitespace().nth(1).unwrap_or("").to_string();
                version = if v.is_empty() || v == "(none)" { pkg_version.to_string() } else { v };
            }
        }
    }

    Ok(axum::Json(ApiServerInfo {
        initialized: true,
        version,
        candidate_version,
        location_name: settings.location_name,
        has_users,
        cameras_configured,
        record_delete_after: settings.record_delete_after,
    }))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/info", get(info))
}
