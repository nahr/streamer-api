//! GET /api/config - Returns public config for the UI.

use axum::{extract::State, routing::get, Json};
use serde::Serialize;

use crate::api::AppState;

#[derive(Serialize)]
pub struct ConfigResponse {}

pub async fn config_handler(State(_app): State<AppState>) -> Json<ConfigResponse> {
    Json(ConfigResponse {})
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/config", get(config_handler))
}
