//! GET /api/config - Returns public config (Auth0) for the UI. No auth required.
//! The UI fetches this at runtime so it works when built without access to table-tv.config
//! (e.g. Debian package built on one machine, installed on another with /etc/table-tv/table-tv.config).

use axum::{extract::State, routing::get};
use serde::Serialize;

use crate::api::AppState;
use crate::config;

#[derive(Serialize)]
pub struct ConfigResponse {
    pub auth0_domain: Option<String>,
    pub auth0_client_id: Option<String>,
    pub auth0_audience: Option<String>,
    pub auth0_skip_audience: bool,
    pub auth0_connection: Option<String>,
}

pub async fn config_handler(State(_app): State<AppState>) -> axum::Json<ConfigResponse> {
    let cfg = config::config();
    axum::Json(ConfigResponse {
        auth0_domain: cfg.auth0_domain.clone(),
        auth0_client_id: cfg.auth0_client_id.clone(),
        auth0_audience: cfg.auth0_audience.clone(),
        auth0_skip_audience: cfg.auth0_skip_audience,
        auth0_connection: cfg.auth0_connection.clone(),
    })
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/config", get(config_handler))
}
