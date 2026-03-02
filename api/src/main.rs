use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod api;
pub mod db;
pub mod error;
pub mod video;

/// On macOS, some TLS backends may not find system certs.
/// Set SSL_CERT_FILE for HTTPS clients (e.g. reqwest, ffmpeg RTMPS).
#[cfg(target_os = "macos")]
fn init_ssl_certs_macos() {
    if std::env::var("SSL_CERT_FILE").is_ok() {
        return;
    }
    let mut candidates: Vec<std::path::PathBuf> = vec![
        "/opt/homebrew/etc/gnutls/cert.pem",
        "/usr/local/etc/gnutls/cert.pem",
        "/opt/homebrew/etc/openssl@3/cert.pem",
        "/opt/homebrew/etc/openssl@1.1/cert.pem",
        "/usr/local/etc/openssl@3/cert.pem",
        "/opt/homebrew/opt/ca-certificates/share/ca-certificates/cacert.pem",
        "/etc/ssl/cert.pem",
        "/etc/ssl/certs/ca-certificates.crt",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Ok(home) = std::env::var("HOME") {
        candidates.insert(
            0,
            PathBuf::from(format!("{}/.homebrew/etc/gnutls/cert.pem", home)),
        );
        candidates.insert(
            1,
            PathBuf::from(format!(
                "{}/.homebrew/opt/ca-certificates/share/ca-certificates/cacert.pem",
                home
            )),
        );
    }
    for path in &candidates {
        if path.exists() {
            let s = path.to_string_lossy();
            std::env::set_var("SSL_CERT_FILE", s.as_ref());
            tracing::info!(path = %s, "SSL_CERT_FILE set for TLS");
            return;
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn init_ssl_certs_macos() {}

#[tokio::main]
async fn main() -> Result<(), crate::error::ApiError> {
    println!("[table-tv] main: starting");
    init_ssl_certs_macos();
    dotenvy::dotenv().ok();
    // When running from api/, also try parent .env (workspace root)
    if std::env::var("AUTH0_DOMAIN").is_err() {
        let _ = dotenvy::from_path(std::path::Path::new("../.env"));
    }
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug,tower_http=debug")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("[table-tv] main: opening db");
    let db = db::Db::open_default()?;
    tracing::info!("[table-tv] main: db ok, starting server");
    api::ApiServer::serve(db).await?;
    tracing::info!("[table-tv] exiting");
    Ok(())
}
