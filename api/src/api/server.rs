use axum::{extract::State, routing::get, Router};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::{admin, info};
use crate::db::Db;
use crate::error::ApiError;

pub struct ApiServer;

impl ApiServer {
    fn router(db: Db) -> Router {
        let mut app = Router::new()
            .route("/api/hello", get(hello_world))
            .merge(admin::routes())
            .merge(info::routes())
            .layer(TraceLayer::new_for_http())
            .with_state(db);

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

async fn hello_world(State(_db): State<Db>) -> &'static str {
    "hello world"
}
