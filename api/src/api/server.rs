use axum::{extract::State, routing::get, Router};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::api::{auth, camera, config, facebook, info, pool_match, settings, upgrade, user};
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
    /// Per-camera connection status from MediaMTX (camera_id -> ready).
    pub camera_connection_status: Arc<RwLock<std::collections::HashMap<String, bool>>>,
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
        let cfg = crate::config::config();
        let auth0_ready = cfg.auth0_domain.as_ref().map_or(false, |s| !s.is_empty())
            && (cfg.auth0_audience.as_ref().map_or(false, |s| !s.is_empty())
                || cfg
                    .auth0_client_id
                    .as_ref()
                    .map_or(false, |s| !s.is_empty()));
        let jwks = auth0_ready.then(|| {
            Arc::new(auth::JwksCache::new(
                cfg.auth0_domain.as_deref().unwrap_or(""),
            ))
        });

        let stream_token = cfg.stream_token.clone().unwrap_or_else(|| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            std::time::SystemTime::now().hash(&mut hasher);
            format!("{:x}", hasher.finish())
        });

        let camera_connection_status = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let camera_connection_status_clone = camera_connection_status.clone();
        let db_for_sync = db.clone();
        // MediaMTX sync runs in background; only retries while sync is failing
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("sync runtime");
            rt.block_on(async {
                const RETRY_DELAY_SECS: u64 = 10;
                loop {
                    match crate::video::sync_all_paths(&db_for_sync).await {
                        true => {
                            tracing::info!("MediaMTX paths synced");
                            return;
                        }
                        false => {
                            tracing::debug!(
                                "MediaMTX sync failed, retrying in {}s",
                                RETRY_DELAY_SECS
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_DELAY_SECS))
                                .await;
                        }
                    }
                }
            });
        });
        // Poll MediaMTX for camera connection status
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("status runtime");
            rt.block_on(async {
                const POLL_INTERVAL_SECS: u64 = 15;
                loop {
                    if let Ok(status) = crate::video::fetch_camera_connection_status().await {
                        if let Ok(mut guard) = camera_connection_status_clone.write() {
                            *guard = status;
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
            });
        });

        let app_state = AppState {
            db: db.clone(),
            overlay: overlay.clone(),
            facebook_tokens: facebook::FacebookTokenCache::new(),
            rtmp_processes,
            jwks,
            stream_token,
            camera_connection_status,
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let mut app = Router::new()
            .route("/api/hello", get(hello_world))
            .merge(config::routes())
            .merge(auth::routes())
            .merge(camera::routes())
            .merge(pool_match::routes())
            .merge(facebook::routes())
            .merge(info::routes())
            .merge(settings::routes())
            .merge(upgrade::routes())
            .merge(user::routes())
            // .layer(TraceLayer::new_for_http())
            .with_state(app_state)
            .layer(cors);

        let ui_dist = cfg
            .ui_dist_path
            .clone()
            .filter(|p| p.exists())
            .or_else(|| {
                ["ui-dist", "ui/dist", "../ui/dist"]
                    .iter()
                    .map(Path::new)
                    .find(|p| p.exists())
                    .map(Path::to_path_buf)
            })
            .or_else(|| {
                // When running release binary from target/release/, ui-dist is next to the exe
                std::env::current_exe().ok().and_then(|exe| {
                    let candidate = exe.parent()?.join("ui-dist");
                    candidate.exists().then_some(candidate)
                })
            });
        if let Some(ref ui_dist) = ui_dist {
            let path = ui_dist.to_string_lossy();
            tracing::info!(path = %path, "serving UI from");
            let serve_dir = ServeDir::new(ui_dist)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(ui_dist.join("index.html")));
            // Use fallback so API routes (/api/*) are matched first; static files only for unmatched paths
            app = app.fallback_service(serve_dir);
        }

        app
    }

    pub async fn serve(db: Db) -> Result<(), ApiError> {
        let app = Self::router(db);
        let port = crate::config::config().port;
        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        tracing::info!(addr = %addr, "starting api server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn hello_world(State(_app): State<AppState>) -> &'static str {
    "hello world"
}
