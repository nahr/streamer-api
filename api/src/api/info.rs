use axum::{extract::State, routing::get};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::ApiError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiServerInfo {
    pub initialized: bool,
}

/// GET /api/info - Returns server info. `initialized` is false if no admin exists yet.
pub async fn info(State(db): State<Db>) -> Result<axum::Json<ApiServerInfo>, ApiError> {
    let initialized = db.has_admin()?;
    Ok(axum::Json(ApiServerInfo { initialized }))
}

pub fn routes() -> axum::Router<Db> {
    axum::Router::new().route("/api/info", get(info))
}
