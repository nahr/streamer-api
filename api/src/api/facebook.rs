//! Facebook Live API integration with OAuth for user authentication.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Redirect,
    routing::get,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::error::ApiError;

const GRAPH_API_BASE: &str = "https://graph.facebook.com/v21.0";
const OAUTH_DIALOG: &str = "https://www.facebook.com/v21.0/dialog/oauth";
const SCOPES: &str = "publish_video";
const STATE_TTL_MINUTES: i64 = 10;

/// Signed OAuth state payload (self-contained, no server storage needed).
#[derive(Serialize, Deserialize)]
struct StatePayload {
    r: String, // return_to
    t: i64,    // timestamp
}

fn create_signed_state(return_to: &str, secret: &[u8]) -> String {
    let payload = StatePayload {
        r: return_to.to_string(),
        t: Utc::now().timestamp(),
    };
    let payload_json = serde_json::to_string(&payload).unwrap();
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
    mac.update(payload_b64.as_bytes());
    let sig = mac.finalize();
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig.into_bytes());
    format!("{}.{}", payload_b64, sig_b64)
}

fn verify_signed_state(state: &str, secret: &[u8]) -> Option<String> {
    let mut parts = state.splitn(2, '.');
    let payload_b64 = parts.next()?;
    let sig_b64 = parts.next()?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64.as_bytes()).ok()?;
    let payload_json = String::from_utf8(payload_bytes).ok()?;
    let payload: StatePayload = serde_json::from_str(&payload_json).ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
    mac.update(payload_b64.as_bytes());
    let sig_bytes = URL_SAFE_NO_PAD.decode(sig_b64.as_bytes()).ok()?;
    mac.verify_slice(&sig_bytes).ok()?;
    let created = chrono::DateTime::from_timestamp(payload.t, 0)?;
    let expires = Utc::now() - Duration::minutes(STATE_TTL_MINUTES);
    if created < expires {
        return None;
    }
    Some(payload.r)
}

/// Derives base URL from request headers (Host, X-Forwarded-Host, X-Forwarded-Proto).
/// Falls back to BASE_URL env var if headers don't yield a valid URL.
fn base_url_from_request(headers: &HeaderMap) -> Option<String> {
    use axum::http::header;
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("http");
    Some(format!("{}://{}", scheme, host))
}

/// Resolves base URL: request-derived first, then config base_url, else error.
fn resolve_base_url(headers: &HeaderMap, config_fallback: bool) -> Result<String, ApiError> {
    if let Some(url) = base_url_from_request(headers) {
        return Ok(url);
    }
    if config_fallback {
        crate::config::config()
            .base_url
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .ok_or_else(|| {
                ApiError::BadRequest(
                    "base_url must be set for OAuth callback (e.g. https://example.com). \
                     Or ensure requests include Host/X-Forwarded-Host."
                        .to_string(),
                )
            })
    } else {
        Err(ApiError::BadRequest(
            "Could not determine base URL from request. Set BASE_URL or ensure Host header is present.".to_string(),
        ))
    }
}

/// Short-lived cache for user tokens (auth_key -> access_token).
#[derive(Clone, Default)]
pub struct FacebookTokenCache {
    auth_key_to_token:
        std::sync::Arc<RwLock<HashMap<String, (String, chrono::DateTime<chrono::Utc>)>>>,
}

impl FacebookTokenCache {
    const TTL_MINUTES: i64 = 10;

    pub fn new() -> Self {
        Self {
            auth_key_to_token: std::sync::Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn store_token(&self, auth_key: String, token: String) {
        let expires = Utc::now() + Duration::minutes(Self::TTL_MINUTES);
        let mut m = self.auth_key_to_token.write().unwrap();
        m.insert(auth_key, (token, expires));
    }

    fn take_token(&self, auth_key: &str) -> Option<String> {
        let mut m = self.auth_key_to_token.write().unwrap();
        let (token, expires) = m.remove(auth_key)?;
        if Utc::now() > expires {
            return None;
        }
        Some(token)
    }
}

/// GET /api/facebook/auth?return_to=... - Starts OAuth flow, redirects to Facebook.
#[derive(Deserialize)]
pub struct AuthQuery {
    pub return_to: String,
}

pub async fn facebook_auth(
    State(_app): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<AuthQuery>,
) -> Result<Redirect, ApiError> {
    let app_id = crate::config::config()
        .facebook_app_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("Facebook OAuth not configured. Set facebook.app_id.".to_string()))?;
    let base_url = resolve_base_url(&headers, true)?;

    let return_to = q.return_to.trim();
    if return_to.is_empty() || !return_to.starts_with('/') {
        return Err(ApiError::BadRequest(
            "return_to must be a path starting with /.".to_string(),
        ));
    }

    let app_secret = crate::config::config()
        .facebook_app_secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("Facebook OAuth not configured. Set facebook.app_secret.".to_string()))?;
    let state = create_signed_state(return_to, app_secret.as_bytes());

    let redirect_uri = format!("{}/facebook/callback", base_url.trim_end_matches('/'));
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&scope={}&state={}",
        OAUTH_DIALOG,
        urlencoding::encode(&app_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&state),
    );

    tracing::info!(return_to = %return_to, "Facebook OAuth: redirecting to Facebook");
    Ok(Redirect::temporary(&auth_url))
}

/// POST /api/facebook/exchange-code - Exchanges code for auth_key (used when UI handles callback).
#[derive(Deserialize)]
pub struct ExchangeCodeRequest {
    pub code: String,
    pub state: String,
}

pub async fn facebook_exchange_code(
    State(app): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<ExchangeCodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    tracing::info!("Facebook exchange-code: received request");
    let cfg = crate::config::config();
    let app_secret = cfg
        .facebook_app_secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("Facebook OAuth not configured. Set facebook.app_secret.".to_string()))?;
    let return_to =
        verify_signed_state(req.state.trim(), app_secret.as_bytes()).ok_or_else(|| {
            tracing::warn!(
                state_len = req.state.len(),
                "Facebook exchange-code: invalid or expired state"
            );
            ApiError::BadRequest("Invalid or expired state. Please try again.".to_string())
        })?;

    let app_id = cfg
        .facebook_app_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("Facebook OAuth not configured. Set facebook.app_id.".to_string()))?;
    let app_secret = cfg
        .facebook_app_secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("Facebook OAuth not configured. Set facebook.app_secret.".to_string()))?;
    let base_url = resolve_base_url(&headers, true)?;

    let redirect_uri = format!("{}/facebook/callback", base_url.trim_end_matches('/'));

    let token_url = format!(
        "{}/oauth/access_token?client_id={}&client_secret={}&redirect_uri={}&code={}",
        GRAPH_API_BASE,
        urlencoding::encode(&app_id),
        urlencoding::encode(&app_secret),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(req.code.trim()),
    );

    let client = reqwest::Client::new();
    let res = client
        .get(&token_url)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("Token exchange failed: {}", e)))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| ApiError::Unknown(format!("Failed to read token response: {}", e)))?;

    if !status.is_success() {
        let err_msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|j| {
                j.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body.clone());
        tracing::error!(status = %status, body = %body, "Facebook exchange-code: token exchange failed");
        return Err(ApiError::BadRequest(format!(
            "Facebook token exchange failed: {}",
            err_msg
        )));
    }

    tracing::info!(return_to = %return_to, "Facebook exchange-code: token exchange succeeded");
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| ApiError::Unknown(format!("Invalid token response: {}", e)))?;

    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::Unknown("Facebook response missing access_token".to_string()))?;

    let auth_key = Uuid::new_v4().to_string();
    app.facebook_tokens
        .store_token(auth_key.clone(), access_token.to_string());
    tracing::info!(auth_key = %auth_key, return_to = %return_to, "Facebook exchange-code: returning auth_key");

    Ok(Json(serde_json::json!({
        "auth_key": auth_key,
        "return_to": return_to
    })))
}

/// GET /api/facebook/status - Returns whether Facebook Live (OAuth) is configured.
pub async fn facebook_status(
    _auth: AuthenticatedUser,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cfg = crate::config::config();
    let app_id = cfg.facebook_app_id.clone();
    let app_secret = cfg.facebook_app_secret.clone();
    let base_url = base_url_from_request(&headers)
        .or_else(|| cfg.base_url.clone())
        .filter(|s| !s.is_empty());
    let configured = app_id.as_ref().map_or(false, |s| !s.is_empty())
        && app_secret.as_ref().map_or(false, |s| !s.is_empty())
        && base_url.as_ref().map_or(false, |s| !s.is_empty());
    let redirect_uri = base_url
        .as_ref()
        .map(|b| format!("{}/facebook/callback", b.trim_end_matches('/')));
    Ok(Json(serde_json::json!({
        "configured": configured,
        "redirect_uri": redirect_uri
    })))
}

/// POST /api/facebook/live-url - Creates a Facebook Live video using the user's token.
#[derive(Deserialize)]
pub struct FacebookLiveUrlRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    /// Privacy: EVERYONE, ALL_FRIENDS, FRIENDS_OF_FRIENDS, SELF. Defaults to EVERYONE.
    pub privacy: Option<String>,
    /// One-time auth key from OAuth callback (required).
    pub auth_key: Option<String>,
}

/// POST /api/facebook/live-url - Creates a Facebook Live video and returns the stream URL.
pub async fn facebook_live_url(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    axum::Json(req): axum::Json<FacebookLiveUrlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    tracing::info!(
        has_auth_key = req.auth_key.is_some(),
        "Facebook live-url: received request"
    );
    let auth_key = req
        .auth_key
        .as_ref()
        .and_then(|s| if s.trim().is_empty() { None } else { Some(s.trim()) })
        .ok_or_else(|| {
            tracing::warn!("Facebook live-url: missing auth_key");
            ApiError::BadRequest(
                "Authentication required. Click \"Go Live with Facebook\" to sign in with your Facebook account.".to_string(),
            )
        })?;

    let token = app.facebook_tokens.take_token(auth_key).ok_or_else(|| {
        tracing::warn!(auth_key = %auth_key, "Facebook live-url: auth_key not found or expired");
        ApiError::BadRequest(
            "Session expired or invalid. Please sign in again with Facebook.".to_string(),
        )
    })?;

    // User tokens always stream to "me" (user's profile)
    let target_id = "me";

    let title = req
        .title
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Table TV Live".to_string());
    let description = req.description.unwrap_or_default();
    let privacy_value = req
        .privacy
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("EVERYONE");
    let privacy = format!(r#"{{"value":"{}"}}"#, privacy_value);

    let url: String = format!(
        "{}/{}/live_videos?status=LIVE_NOW&title={}&description={}&privacy={}&access_token={}",
        GRAPH_API_BASE,
        target_id,
        urlencoding::encode(&title),
        urlencoding::encode(&description),
        urlencoding::encode(&privacy),
        token.trim()
    );

    tracing::info!(title = %title, description=%description, privacy=%privacy, "starting facebook stream");

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .send()
        .await
        .map_err(|e| ApiError::Unknown(format!("Facebook API request failed: {}", e)))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| ApiError::Unknown(format!("Failed to read Facebook response: {}", e)))?;

    if !status.is_success() {
        let err_msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|j| {
                j.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body.clone());
        tracing::error!(status = %status, body = %body, "Facebook live-url: create live_videos failed");
        return Err(ApiError::BadRequest(format!(
            "Facebook API error: {}",
            err_msg
        )));
    }

    tracing::info!("Facebook live-url: create live_videos succeeded");
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| ApiError::Unknown(format!("Invalid Facebook response: {}", e)))?;

    let stream_url = json
        .get("secure_stream_url")
        .or_else(|| json.get("stream_url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ApiError::Unknown("Facebook response missing secure_stream_url".to_string())
        })?;

    let live_video_id = json.get("id").and_then(|v| v.as_str());
    tracing::info!(live_video_id = ?live_video_id, "Facebook live-url: returning stream URL");
    Ok(Json(serde_json::json!({
        "url": stream_url,
        "live_video_id": live_video_id
    })))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/facebook/auth", get(facebook_auth))
        .route(
            "/api/facebook/exchange-code",
            axum::routing::post(facebook_exchange_code),
        )
        .route("/api/facebook/status", get(facebook_status))
        .route(
            "/api/facebook/live-url",
            axum::routing::post(facebook_live_url),
        )
}
