use axum::{extract::State, routing::get, Router};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::{admin, camera, facebook, info, pool_match, settings};
use crate::db::Db;
use crate::error::ApiError;
use crate::video::{self, OverlayState};

pub struct ApiServer;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub overlay: OverlayState,
    pub facebook_tokens: facebook::FacebookTokenCache,
    pub rtmp_processes: crate::video::RtmpState,
}

impl ApiServer {
    fn router(db: Db) -> Router {
        let overlay: OverlayState = Arc::new(RwLock::new(None));
        let rtmp_processes = crate::video::rtmp_state_new();
        if db.list_cameras().map_or(false, |cams| cams.iter().any(|c| c.camera_type.is_internal())) {
            video::ensure_internal_camera_ready(overlay.clone());
            video::restore_overlay_from_db(&db, &overlay, &rtmp_processes);
            video::spawn_overlay_refresh_task(db.clone(), overlay.clone(), rtmp_processes.clone());
        }
        let app_state = AppState {
            db: db.clone(),
            overlay: overlay.clone(),
            facebook_tokens: facebook::FacebookTokenCache::new(),
            rtmp_processes,
        };

        let mut app = Router::new()
            .route("/api/hello", get(hello_world))
            .merge(admin::routes())
            .merge(camera::routes())
            .merge(pool_match::routes())
            .merge(facebook::routes())
            .merge(info::routes())
            .merge(settings::routes())
            .layer(TraceLayer::new_for_http())
            .with_state(app_state);

        if Path::new("ui-dist").exists() {
            let serve_dir = ServeDir::new("ui-dist")
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new("ui-dist/index.html"));
            app = app.nest_service("/", serve_dir);
        }

        app
    }

    pub async fn serve(db: Db) -> Result<(), ApiError> {
        let app = Self::router(db);
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        tracing::info!("starting api server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn hello_world(State(_app): State<AppState>) -> &'static str {
    "hello world"
}
