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
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
struct Auth0Claims {
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
    let domain = std::env::var("AUTH0_DOMAIN")
        .map_err(|_| ApiError::BadRequest("AUTH0_DOMAIN must be set".to_string()))?;
    let mut audiences: Vec<String> = Vec::new();
    if let Ok(a) = std::env::var("AUTH0_AUDIENCE") {
        if !a.is_empty() {
            audiences.push(a);
        }
    }
    if let Ok(c) = std::env::var("AUTH0_CLIENT_ID") {
        if !c.is_empty() {
            audiences.push(c);
        }
    }
    if audiences.is_empty() {
        return Err(ApiError::BadRequest(
            "AUTH0_AUDIENCE or AUTH0_CLIENT_ID must be set for Auth0 login".to_string(),
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
    tracing::debug!(token_len = token.len(), "validate token");
    let header = decode_header(token)
        .map_err(|e| ApiError::Auth0ClientError(format!("Invalid token header: {}", e)))?;
    let kid = header
        .kid
        .ok_or_else(|| ApiError::Auth0ClientError("Token missing kid".to_string()))?;
    let key = jwks.get_decoding_key(&kid).await?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    let skip_audience = std::env::var("AUTH0_SKIP_AUDIENCE").as_deref() == Ok("true");
    if skip_audience {
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
    tracing::debug!(name = ?claims.name, email = ?claims.email, "JWT claims");
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
        let email = email_from_claims(&claims);
        let name = name_from_claims(&claims);
        let picture = claims.picture.clone().filter(|s| !s.is_empty());
        let user = app_state.db.upsert_user(claims.sub.clone(), email)?;

        Ok(AuthenticatedUser {
            sub: user.auth0_sub,
            email: user.email,
            name,
            picture,
            is_admin: user.is_admin,
        })
    }
}

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

    let claims = validate_token(jwks, token, &audiences, &issuer).await?;
    let email = email_from_claims(&claims);
    let name = name_from_claims(&claims);
    let picture = claims.picture.clone().filter(|s| !s.is_empty());
    let user = app.db.upsert_user(claims.sub.clone(), email)?;

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
