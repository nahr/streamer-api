use axum::{routing::get, Router};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() {
    let mut app = Router::new().route("/api/hello", get(hello_world));

    if Path::new("ui-dist").exists() {
        let serve_dir = ServeDir::new("ui-dist")
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new("ui-dist/index.html"));
        app = app.nest_service("/", serve_dir);
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn hello_world() -> &'static str {
    "hello world"
}
