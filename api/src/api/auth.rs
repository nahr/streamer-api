//! Auth0 JWT validation and user sync.
//!
//! Requires: AUTH0_DOMAIN, and either AUTH0_AUDIENCE or AUTH0_CLIENT_ID.
//! - AUTH0_AUDIENCE: for access tokens (SPA requests with audience)
//! - AUTH0_CLIENT_ID: for ID tokens (when VITE_AUTH0_SKIP_AUDIENCE=true to avoid 403)

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, Request, State},
    http::{header, request::Parts},
    routing::get,
    Json,
};

use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::api::AppState;
use crate::db::Db;
use crate::error::ApiError;

/// Response from Auth0 /userinfo endpoint.
#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    given_name: Option<String>,
    #[serde(default)]
    family_name: Option<String>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    picture: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Auth0Claims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    given_name: Option<String>,
    #[serde(default)]
    family_name: Option<String>,
    #[serde(default)]
    picture: Option<String>,
    #[serde(default)]
    aud: serde_json::Value,
    exp: u64,
    iat: Option<u64>,
    iss: String,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    sid: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
}

#[derive(Serialize)]
pub struct AuthMeResponse {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub is_admin: bool,
}

/// JWKS cache - fetches from Auth0 and refreshes on miss.
pub struct JwksCache {
    domain: String,
    jwks: RwLock<Option<JwkSet>>,
}

impl JwksCache {
    pub fn new(domain: &str) -> Self {
        let domain = domain.trim_end_matches('/').to_string();
        if !domain.starts_with("http") {
            return Self {
                domain: format!("https://{}", domain),
                jwks: RwLock::new(None),
            };
        }
        Self {
            domain,
            jwks: RwLock::new(None),
        }
    }

    async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, ApiError> {
        let mut guard = self.jwks.write().await;
        if guard.is_none() {
            let url = format!("{}/.well-known/jwks.json", self.domain);
            let jwks: JwkSet = reqwest::get(&url)
                .await
                .map_err(|e| ApiError::Auth0ClientError(format!("Failed to fetch JWKS: {}", e)))?
                .json()
                .await
                .map_err(|e| ApiError::Auth0ClientError(format!("Failed to parse JWKS: {}", e)))?;
            *guard = Some(jwks);
        }
        let jwks = guard.as_ref().unwrap();
        let jwk = jwks
            .find(kid)
            .ok_or_else(|| ApiError::Auth0ClientError("JWK not found for kid".to_string()))?;
        DecodingKey::from_jwk(jwk)
            .map_err(|e| ApiError::Auth0ClientError(format!("Invalid JWK: {}", e)))
    }

    /// Invalidate cache (e.g. on 401 from Auth0) - allows retry with fresh JWKS.
    #[allow(dead_code)]
    async fn invalidate(&self) {
        *self.jwks.write().await = None;
    }
}

fn auth0_config() -> Result<(String, Vec<String>), ApiError> {
    let cfg = crate::config::config();
    let domain = cfg
        .auth0_domain
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("auth0.domain must be set".to_string()))?;
    let mut audiences: Vec<String> = Vec::new();
    if let Some(a) = cfg.auth0_audience.as_ref().filter(|s| !s.is_empty()) {
        audiences.push(a.clone());
    }
    if let Some(c) = cfg.auth0_client_id.as_ref().filter(|s| !s.is_empty()) {
        audiences.push(c.clone());
    }
    if audiences.is_empty() {
        return Err(ApiError::BadRequest(
            "auth0.audience or auth0.client_id must be set for Auth0 login".to_string(),
        ));
    }
    Ok((domain, audiences))
}

/// Validate JWT and return claims. Accepts tokens with any of the given audiences.
pub async fn validate_token(
    jwks: &JwksCache,
    token: &str,
    audiences: &[String],
    issuer: &str,
) -> Result<Auth0Claims, ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::Auth0ClientError(format!(
            "Invalid token: expected 3 parts (header.payload.signature), got {}",
            parts.len()
        )));
    }
    tracing::trace!(token_len = token.len(), "validate token");
    let header = decode_header(token)
        .map_err(|e| ApiError::Auth0ClientError(format!("Invalid token header: {}", e)))?;
    let kid = header
        .kid
        .ok_or_else(|| ApiError::Auth0ClientError("Token missing kid".to_string()))?;
    let key = jwks.get_decoding_key(&kid).await?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    if crate::config::config().auth0_skip_audience {
        validation.validate_aud = false;
    } else {
        validation.set_audience(audiences);
    }
    validation.set_issuer(&[issuer]);

    let token_data = decode::<serde_json::Value>(token, &key, &validation)
        .map_err(|e| ApiError::Auth0ClientError(format!("Invalid token: {}", e)))?;
    let v = &token_data.claims;
    let sub = v
        .get("sub")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let claims = Auth0Claims {
        sub: sub.clone(),
        email: v.get("email").and_then(|x| x.as_str()).map(String::from),
        name: v.get("name").and_then(|x| x.as_str()).map(String::from),
        nickname: v.get("nickname").and_then(|x| x.as_str()).map(String::from),
        given_name: v
            .get("given_name")
            .and_then(|x| x.as_str())
            .map(String::from),
        family_name: v
            .get("family_name")
            .and_then(|x| x.as_str())
            .map(String::from),
        picture: v.get("picture").and_then(|x| x.as_str()).map(String::from),
        aud: v.get("aud").cloned().unwrap_or(serde_json::Value::Null),
        exp: v.get("exp").and_then(|x| x.as_u64()).unwrap_or(0),
        iat: v.get("iat").and_then(|x| x.as_u64()),
        iss: v
            .get("iss")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        updated_at: v
            .get("updated_at")
            .and_then(|x| x.as_str())
            .map(String::from),
        email_verified: v.get("email_verified").and_then(|x| x.as_bool()),
        sid: v.get("sid").and_then(|x| x.as_str()).map(String::from),
        nonce: v.get("nonce").and_then(|x| x.as_str()).map(String::from),
    };
    tracing::trace!(name = ?claims.name, email = ?claims.email, "JWT claims");
    Ok(claims)
}

/// Extract email from claims.
fn email_from_claims(claims: &Auth0Claims) -> String {
    claims
        .email
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}@auth0.local", claims.sub))
}

/// Extract display name from claims. Prefers name, then given_name+family_name, then nickname.
fn name_from_claims(claims: &Auth0Claims) -> String {
    claims
        .name
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let given = claims.given_name.clone().filter(|s| !s.is_empty());
            let family = claims.family_name.clone().filter(|s| !s.is_empty());
            match (given, family) {
                (Some(g), Some(f)) => Some(format!("{} {}", g, f)),
                (Some(g), None) => Some(g),
                (None, Some(f)) => Some(f),
                (None, None) => None,
            }
        })
        .or_else(|| claims.nickname.clone().filter(|s| !s.is_empty()))
        .or_else(|| {
            let email = email_from_claims(claims);
            if email.ends_with("@auth0.local") {
                None
            } else {
                Some(email)
            }
        })
        .unwrap_or_else(|| {
            // Fallback for social login when name/email not in token: show provider
            let provider = claims.sub.split('|').next().unwrap_or(&claims.sub);
            match provider {
                "facebook" => "Facebook User",
                "google-oauth2" | "google" => "Google User",
                "auth0" => "User",
                _ => provider,
            }
            .to_string()
        })
}

/// True when the name is a generic fallback (e.g. "Facebook User") - treat as broken, retry userinfo.
fn is_fallback_name(name: &str, sub: &str) -> bool {
    let n = name.trim();
    if n.is_empty() {
        return true;
    }
    if matches!(n, "Facebook User" | "Google User" | "User") {
        return true;
    }
    let provider = sub.split('|').next().unwrap_or(sub);
    n == provider
}

/// True when JWT has no profile data (name, email, etc.) - used to decide whether to fetch userinfo.
fn used_profile_fallback(claims: &Auth0Claims) -> bool {
    let has_name = claims.name.as_ref().map_or(false, |s| !s.is_empty());
    let has_given = claims.given_name.as_ref().map_or(false, |s| !s.is_empty());
    let has_family = claims.family_name.as_ref().map_or(false, |s| !s.is_empty());
    let has_nickname = claims.nickname.as_ref().map_or(false, |s| !s.is_empty());
    let email = email_from_claims(claims);
    let has_real_email = !email.ends_with("@auth0.local");
    !has_name && !has_given && !has_family && !has_nickname && !has_real_email
}

/// Fetch user profile from Auth0 userinfo endpoint. Used when JWT lacks profile claims (e.g. Facebook).
async fn fetch_userinfo(domain: &str, token: &str) -> Result<UserInfoResponse, ApiError> {
    let base = domain.trim_end_matches('/');
    let url = if base.starts_with("http") {
        format!("{}/userinfo", base)
    } else {
        format!("https://{}/userinfo", base)
    };
    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| ApiError::Auth0ClientError(format!("Failed to fetch userinfo: {}", e)))?;
    let status = res.status();
    if !status.is_success() {
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let headers: Vec<_> = res
                .headers()
                .iter()
                .map(|(name, value)| {
                    format!(
                        "{}: {}",
                        name.as_str(),
                        value.to_str().unwrap_or("(invalid)")
                    )
                })
                .collect();
            tracing::warn!(headers = ?headers, "userinfo 429: response headers");
        }
        let body = res.text().await.unwrap_or_default();
        return Err(ApiError::Auth0ClientError(format!(
            "Userinfo returned {}: {}",
            status,
            if body.is_empty() {
                "(no body)".to_string()
            } else {
                body.chars().take(200).collect::<String>()
            }
        )));
    }
    res.json()
        .await
        .map_err(|e| ApiError::Auth0ClientError(format!("Failed to parse userinfo: {}", e)))
}

/// Extract Bearer token from Authorization header.
fn bearer_token_from_request(req: &Request<axum::body::Body>) -> impl Iterator<Item = &str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .into_iter()
        .filter_map(|s| s.strip_prefix("Bearer "))
}

/// Extractor for routes that require authentication. Validates Bearer token and syncs user to DB.
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub is_admin: bool,
}

fn token_from_parts(parts: &Parts) -> Option<String> {
    let from_header = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(String::from));
    if from_header.is_some() {
        return from_header;
    }
    parts.uri.query().and_then(|q| {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == "access_token" {
                    return urlencoding::decode(v).ok().map(|s| s.to_string());
                }
            }
        }
        None
    })
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let jwks = app_state
            .jwks
            .as_ref()
            .ok_or(ApiError::BadRequest("Auth0 not configured".to_string()))?;
        let (domain, audiences) = auth0_config()?;
        let domain_clean = domain.trim_start_matches("https://").trim_end_matches('/');
        let issuer = format!("https://{}/", domain_clean);

        let token = token_from_parts(parts).ok_or(ApiError::InvalidCredentials)?;

        let claims = validate_token(jwks, &token, &audiences, &issuer).await?;
        let access_token = parts
            .headers
            .get(X_AUTH0_ACCESS_TOKEN)
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty());
        let (name, email, picture) =
            resolve_profile(&app_state.db, &claims, &token, access_token, &domain, false).await;
        let user = app_state
            .db
            .upsert_user(claims.sub.clone(), email, Some(name.clone()), picture.clone())?;

        Ok(AuthenticatedUser {
            sub: user.auth0_sub,
            email: user.email,
            name,
            picture,
            is_admin: user.is_admin,
        })
    }
}

/// Extractor for stream endpoint: accepts either AuthenticatedUser or valid ?stream_token= for server-side RTMP pipeline.
#[derive(Clone, Debug)]
pub struct StreamAuth;

#[async_trait]
impl<S> FromRequestParts<S> for StreamAuth
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        // Check for stream_token in query (used by RTMP pipeline)
        if let Some(query) = parts.uri.query() {
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "stream_token" {
                        if let Ok(decoded) = urlencoding::decode(v) {
                            if !decoded.is_empty() && decoded == app_state.stream_token {
                                return Ok(StreamAuth);
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Fall back to normal auth
        let _ = AuthenticatedUser::from_request_parts(parts, state).await?;
        Ok(StreamAuth)
    }
}

/// Resolve name, email, picture from claims. Uses DB cache when JWT lacks profile; only calls Auth0 userinfo on cache miss.
/// userinfo_token: use this for the userinfo call when present (Auth0 requires access token; ID tokens are rejected).
/// use_userinfo: when false, skip userinfo fetch (use cache or fallback only).
async fn resolve_profile(
    db: &Db,
    claims: &Auth0Claims,
    bearer_token: &str,
    userinfo_token: Option<&str>,
    domain: &str,
    use_userinfo: bool,
) -> (String, String, Option<String>) {
    let mut email = email_from_claims(claims);
    let mut name = name_from_claims(claims);
    let mut picture = claims.picture.clone().filter(|s| !s.is_empty());

    // Check DB cache first (avoids Auth0 userinfo rate limits)
    let cached = db.find_user_by_sub(&claims.sub).ok().flatten();
    let has_cached_profile = cached.as_ref().and_then(|c| c.name.as_ref()).map_or(false, |s| {
        !s.is_empty() && !is_fallback_name(s, &claims.sub)
    });

    if has_cached_profile {
        let c = cached.unwrap();
        name = c.name.unwrap();
        email = c.email;
        if let Some(p) = c.picture.filter(|s| !s.is_empty()) {
            picture = Some(p);
        }
        return (name, email, picture);
    }

    // Cache miss, stored values None, or fallback name (e.g. "Facebook User"): fetch from Auth0 at login when allowed
    let needs_userinfo = used_profile_fallback(claims)
        || cached.as_ref().map_or(false, |c| {
            c.name.as_ref()
                .map_or(true, |s| s.is_empty() || is_fallback_name(s, &claims.sub))
        });
    if use_userinfo && needs_userinfo {
        let has_access_token = userinfo_token.is_some();
        let token_for_userinfo = userinfo_token.unwrap_or(bearer_token);
        match fetch_userinfo(domain, token_for_userinfo).await {
            Ok(userinfo) => {
                if let Some(n) = userinfo
                    .name
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        let g = userinfo.given_name.filter(|s| !s.is_empty());
                        let f = userinfo.family_name.filter(|s| !s.is_empty());
                        match (g, f) {
                            (Some(gg), Some(ff)) => Some(format!("{} {}", gg, ff)),
                            (Some(gg), None) => Some(gg),
                            (None, Some(ff)) => Some(ff),
                            (None, None) => None,
                        }
                    })
                    .or_else(|| userinfo.nickname.filter(|s| !s.is_empty()))
                {
                    name = n;
                }
                if let Some(e) = userinfo.email.filter(|s| !s.is_empty()) {
                    email = e;
                }
                if let Some(p) = userinfo.picture.filter(|s| !s.is_empty()) {
                    picture = Some(p);
                }
            }
            Err(e) => {
                tracing::warn!(
                    sub = %claims.sub,
                    has_access_token = has_access_token,
                    error = %e,
                    "userinfo fetch failed; using provider fallback (e.g. Facebook User). \
                     If has_access_token=false, ensure client sends X-Auth0-Access-Token."
                );
            }
        }
    }

    (name, email, picture)
}

/// Header sent by client when using skipAudience: access token for Auth0 userinfo (ID tokens are rejected).
const X_AUTH0_ACCESS_TOKEN: &str = "x-auth0-access-token";

/// GET /api/auth/me - Validate Bearer token, sync user to DB, return user info.
pub async fn auth_me(
    State(app): State<AppState>,
    req: Request<axum::body::Body>,
) -> Result<Json<AuthMeResponse>, ApiError> {
    let jwks = app
        .jwks
        .as_ref()
        .ok_or(ApiError::BadRequest("Auth0 not configured".to_string()))?;
    let (domain, audiences) = auth0_config()?;
    let domain_clean = domain.trim_start_matches("https://").trim_end_matches('/');
    let issuer = format!("https://{}/", domain_clean);

    let token = bearer_token_from_request(&req)
        .next()
        .ok_or(ApiError::InvalidCredentials)?;

    let access_token = req
        .headers()
        .get(X_AUTH0_ACCESS_TOKEN)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty());

    let claims = validate_token(jwks, token, &audiences, &issuer).await?;
    let (name, email, picture) =
        resolve_profile(&app.db, &claims, token, access_token, &domain, true).await;
    let user = app
        .db
        .upsert_user(claims.sub.clone(), email.clone(), Some(name.clone()), picture.clone())?;

    Ok(Json(AuthMeResponse {
        sub: user.auth0_sub,
        email: user.email,
        name,
        picture,
        is_admin: user.is_admin,
    }))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/auth/me", get(auth_me))
}
