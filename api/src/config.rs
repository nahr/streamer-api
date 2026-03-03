//! Application configuration loaded from config.toml.

use serde::Deserialize;

/// Extract host from URL (e.g. "http://127.0.0.1:9997" -> "127.0.0.1").
fn host_from_url(url: &str) -> String {
    let s = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    s.split([':', '/']).next().unwrap_or("127.0.0.1").to_string()
}
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Application configuration. Load from config.toml via `load()`.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub sqlite_path: String,
    pub ui_dist_path: Option<PathBuf>,
    pub stream_token: Option<String>,

    pub auth0_domain: Option<String>,
    pub auth0_client_id: Option<String>,
    pub auth0_audience: Option<String>,
    pub auth0_skip_audience: bool,
    pub auth0_connection: Option<String>,

    pub facebook_app_id: Option<String>,
    pub facebook_app_secret: Option<String>,

    pub mediamtx_api_url: String,
    /// Derived from mediamtx_api_url (port 9997 -> 9996).
    pub mediamtx_playback_url: String,
    /// Derived from mediamtx_api_url host.
    pub mediamtx_rtsp_host: String,
    pub mediamtx_rtsp_port: String,

    pub use_stunnel_for_rtmps: bool,
    pub stunnel_host: String,
}

#[derive(Deserialize)]
struct ConfigFile {
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    sqlite_path: Option<String>,
    #[serde(default)]
    ui_dist_path: Option<PathBuf>,
    #[serde(default)]
    stream_token: Option<String>,
    #[serde(default)]
    use_stunnel_for_rtmps: Option<bool>,

    #[serde(default, rename = "auth0")]
    auth0: Option<Auth0Section>,
    #[serde(default, rename = "facebook")]
    facebook: Option<FacebookSection>,
    #[serde(default, rename = "mediamtx")]
    mediamtx: Option<MediamtxSection>,
    #[serde(default, rename = "stunnel")]
    stunnel: Option<StunnelSection>,
}

#[derive(Deserialize)]
struct Auth0Section {
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    audience: Option<String>,
    #[serde(default)]
    skip_audience: Option<bool>,
    #[serde(default)]
    connection: Option<String>,
}

#[derive(Deserialize)]
struct FacebookSection {
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    app_secret: Option<String>,
}

#[derive(Deserialize)]
struct MediamtxSection {
    #[serde(default)]
    api_url: Option<String>,
    #[serde(default)]
    rtsp_port: Option<String>,
}

#[derive(Deserialize)]
struct StunnelSection {
    #[serde(default)]
    host: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            sqlite_path: "data/table-tv.db".to_string(),
            ui_dist_path: None,
            stream_token: None,
            auth0_domain: None,
            auth0_client_id: None,
            auth0_audience: None,
            auth0_skip_audience: false,
            auth0_connection: None,
            facebook_app_id: None,
            facebook_app_secret: None,
            mediamtx_api_url: "http://127.0.0.1:9997".to_string(),
            mediamtx_playback_url: "http://127.0.0.1:9996".to_string(),
            mediamtx_rtsp_host: "127.0.0.1".to_string(),
            mediamtx_rtsp_port: "8554".to_string(),
            use_stunnel_for_rtmps: false,
            stunnel_host: "127.0.0.1".to_string(),
        }
    }
}

impl From<ConfigFile> for AppConfig {
    fn from(f: ConfigFile) -> Self {
        let mut config = AppConfig::default();
        if let Some(p) = f.port {
            config.port = p;
        }
        if let Some(p) = f.sqlite_path.filter(|s| !s.is_empty()) {
            config.sqlite_path = p;
        }
        config.ui_dist_path = f.ui_dist_path;
        config.stream_token = f.stream_token.filter(|s| !s.is_empty());
        config.use_stunnel_for_rtmps = f.use_stunnel_for_rtmps.unwrap_or(false);

        if let Some(a) = f.auth0 {
            config.auth0_domain = a.domain.filter(|s| !s.is_empty());
            config.auth0_client_id = a.client_id.filter(|s| !s.is_empty());
            config.auth0_audience = a.audience.filter(|s| !s.is_empty());
            config.auth0_skip_audience = a.skip_audience.unwrap_or(false);
            config.auth0_connection = a.connection.filter(|s| !s.is_empty());
        }
        if let Some(fb) = f.facebook {
            config.facebook_app_id = fb.app_id.filter(|s| !s.is_empty());
            config.facebook_app_secret = fb.app_secret.filter(|s| !s.is_empty());
        }
        if let Some(m) = f.mediamtx {
            if let Some(u) = m.api_url.filter(|s| !s.is_empty()) {
                config.mediamtx_api_url = u.clone();
                config.mediamtx_playback_url = u.replace("9997", "9996");
                config.mediamtx_rtsp_host = host_from_url(&u);
            }
            if let Some(p) = m.rtsp_port.filter(|s| !s.is_empty()) {
                config.mediamtx_rtsp_port = p;
            }
        }
        if let Some(s) = f.stunnel {
            if let Some(h) = s.host.filter(|s| !s.is_empty()) {
                config.stunnel_host = h;
            }
        }
        config
    }
}

/// Load config from file. Search order: /etc/table-tv/config.toml, ./config.toml, ../config.toml.
pub fn load() -> AppConfig {
    let candidates: Vec<std::path::PathBuf> = [
        std::path::Path::new("/etc/table-tv/config.toml"),
        std::path::Path::new("./config.toml"),
        std::path::Path::new("../config.toml"),
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect();

    for path in &candidates {
        if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Ok(file) = toml::from_str::<ConfigFile>(&contents) {
                    tracing::info!(path = %path.display(), "loaded config");
                    return file.into();
                }
            }
        }
    }
    AppConfig::default()
}

/// Initialize config (call once at startup). Returns the config.
pub fn init() -> &'static AppConfig {
    CONFIG.get_or_init(load)
}

/// Get the loaded config. Panics if `init()` was not called.
pub fn config() -> &'static AppConfig {
    CONFIG.get().expect("config not initialized; call config::init() first")
}
