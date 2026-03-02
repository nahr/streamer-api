use axum::{extract::State, routing::get, Router};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::{auth, camera, facebook, info, pool_match, settings, user};
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
    pub jwks: Option<Arc<auth::JwksCache>>,
    /// Token for server-side stream access (RTMP pipeline). Env STREAM_TOKEN or random at startup.
    pub stream_token: String,
    /// MediaMTX is available and paths are synced. Stream/RTMP use MediaMTX URL when true.
    pub mediamtx_available: Arc<RwLock<bool>>,
}

impl ApiServer {
    fn router(db: Db) -> Router {
        tracing::info!("building router");
        let overlay: OverlayState = Arc::new(RwLock::new(None));
        let rtmp_processes = crate::video::rtmp_state_new();
        if db
            .list_cameras()
            .map_or(false, |cams| cams.iter().any(|c| c.camera_type.is_rtsp()))
        {
            tracing::info!("restoring overlay from db");
            video::restore_overlay_from_db(&db, &overlay, &rtmp_processes);
            video::spawn_overlay_refresh_task(db.clone(), overlay.clone(), rtmp_processes.clone());
        }
        tracing::info!("router: overlay done");
        let auth0_ready = std::env::var("AUTH0_DOMAIN").is_ok()
            && (std::env::var("AUTH0_AUDIENCE").is_ok()
                || std::env::var("AUTH0_CLIENT_ID").is_ok());
        let jwks = auth0_ready.then(|| {
            Arc::new(auth::JwksCache::new(
                &std::env::var("AUTH0_DOMAIN").unwrap_or_default(),
            ))
        });

        let stream_token = std::env::var("STREAM_TOKEN").unwrap_or_else(|_| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            std::time::SystemTime::now().hash(&mut hasher);
            format!("{:x}", hasher.finish())
        });

        let mediamtx_available = Arc::new(RwLock::new(false));
        let mediamtx_available_clone = mediamtx_available.clone();
        let db_for_sync = db.clone();
        tokio::spawn(async move {
            const MAX_RETRIES: u32 = 15;
            const RETRY_DELAY_SECS: u64 = 2;
            for attempt in 1..=MAX_RETRIES {
                if crate::video::sync_all_paths(&db_for_sync).await {
                    if let Ok(mut guard) = mediamtx_available_clone.write() {
                        *guard = true;
                        tracing::info!("MediaMTX paths synced, streams will use MediaMTX proxy");
                    }
                    return;
                }
                if attempt < MAX_RETRIES {
                    tracing::debug!(
                        attempt,
                        "MediaMTX not ready, retrying in {}s",
                        RETRY_DELAY_SECS
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
            tracing::info!("MediaMTX unavailable after {} attempts, using direct camera URLs", MAX_RETRIES);
        });

        let app_state = AppState {
            db: db.clone(),
            overlay: overlay.clone(),
            facebook_tokens: facebook::FacebookTokenCache::new(),
            rtmp_processes,
            jwks,
            stream_token,
            mediamtx_available,
        };

        let mut app = Router::new()
            .route("/api/hello", get(hello_world))
            .merge(auth::routes())
            .merge(camera::routes())
            .merge(pool_match::routes())
            .merge(facebook::routes())
            .merge(info::routes())
            .merge(settings::routes())
            .merge(user::routes())
            // .layer(TraceLayer::new_for_http())
            .with_state(app_state);

        if Path::new("ui-dist").exists() {
            let serve_dir = ServeDir::new("ui-dist")
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new("ui-dist/index.html"));
            // Use fallback so API routes (/api/*) are matched first; static files only for unmatched paths
            app = app.fallback_service(serve_dir);
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
