//! Auth0 Management API client for user and role management.
//!
//! Requires env vars: AUTH0_DOMAIN, AUTH0_MGMT_CLIENT_ID, AUTH0_MGMT_CLIENT_SECRET
//! Optional for roles: AUTH0_ROLE_VIEWER_ID, AUTH0_ROLE_ADMIN_ID

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;

#[derive(Debug, Serialize)]
struct Auth0TokenRequest {
    client_id: String,
    client_secret: String,
    audience: String,
    grant_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TokenType {
    Bearer,
}

#[derive(Deserialize)]
struct Auth0TokenResponse {
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub _token_type: TokenType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub email: String,
    pub last_login: Option<DateTime<Utc>>,
    pub last_ip: Option<String>,
    pub logins_count: Option<i32>,
    pub user_id: String,

    #[serde(skip_deserializing)]
    pub roles: Vec<Role>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg(test)]
pub struct CreateUserRequest {
    pub connection: String,
    pub password: String,
    pub email: String,
    pub email_verified: bool,
    pub verify_email: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum RoleType {
    Viewer,
    Admin,
}

impl RoleType {
    /// Returns the Auth0 role ID from env, or None if not configured.
    fn role_id_from_env(self) -> Option<String> {
        let var = match self {
            RoleType::Viewer => "AUTH0_ROLE_VIEWER_ID",
            RoleType::Admin => "AUTH0_ROLE_ADMIN_ID",
        };
        std::env::var(var).ok().filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Role {
    pub name: String,
}

#[derive(Debug, Serialize)]
struct RoleUpdateRequest {
    pub roles: Vec<String>,
}

#[derive(Clone)]
pub struct Auth0Client {
    client: Client,
    audience: String,
    token: String,
}

impl Auth0Client {
    pub async fn new() -> Result<Self, ApiError> {
        let domain = std::env::var("AUTH0_DOMAIN")
            .map_err(|_| ApiError::BadRequest("AUTH0_DOMAIN must be set for Auth0".to_string()))?;
        let client_id = std::env::var("AUTH0_CLIENT_ID").map_err(|_| {
            ApiError::BadRequest("AUTH0_CLIENT_ID must be set for Auth0".to_string())
        })?;
        let client_secret = std::env::var("AUTH0_CLIENT_SECRET").map_err(|_| {
            ApiError::BadRequest("AUTH0_CLIENT_SECRET must be set for Auth0".to_string())
        })?;

        let base_url = domain.trim().trim_end_matches('/');
        let base_url = if base_url.starts_with("http") {
            base_url.to_string()
        } else {
            format!("https://{}", base_url)
        };
        let audience = format!("{}/api/v2/", base_url.trim_end_matches('/'));
        let token_request = Auth0TokenRequest {
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            audience: audience.clone(),
            grant_type: "client_credentials".to_owned(),
        };

        let client = Client::new();
        let response: Auth0TokenResponse = client
            .post(format!("{}/oauth/token", base_url))
            .json(&token_request)
            .send()
            .await
            .map_err(|e| {
                ApiError::Auth0ClientError(format!("Failed to post token request: {}", e))
            })?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Token request failed: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                ApiError::Auth0ClientError(format!("Failed to deserialize token response: {}", e))
            })?;

        tracing::info!("Auth0 Management API client connected");
        Ok(Self {
            client,
            audience,
            token: response.access_token,
        })
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.audience, path.trim_start_matches('/'))
    }

    #[cfg(test)]
    pub async fn create_user(&self, u: CreateUserRequest) -> Result<User, ApiError> {
        let response = self
            .client
            .post(self.api_url("users"))
            .bearer_auth(&self.token)
            .json(&u)
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to create user: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Create user failed: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                ApiError::Auth0ClientError(format!("Failed to deserialize user: {}", e))
            })?;
        Ok(response)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), ApiError> {
        let url = self.api_url(&format!("users/{}", urlencoding::encode(user_id)));
        self.client
            .delete(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to delete user: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Delete user failed: {}", e)))?;
        Ok(())
    }

    pub async fn get_users(&self) -> Result<Vec<User>, ApiError> {
        let response: Vec<User> = self
            .client
            .get(self.api_url("users"))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to get users: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Get users failed: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                ApiError::Auth0ClientError(format!("Failed to deserialize user list: {}", e))
            })?;
        Ok(response)
    }

    pub async fn get_roles(&self, user_id: &str) -> Result<Vec<Role>, ApiError> {
        let url = self.api_url(&format!("users/{}/roles", urlencoding::encode(user_id)));
        let response: Vec<Role> = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to get roles: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Get roles failed: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                ApiError::Auth0ClientError(format!("Failed to deserialize roles: {}", e))
            })?;
        Ok(response)
    }

    pub async fn add_roles(&self, user_id: &str, roles: Vec<RoleType>) -> Result<(), ApiError> {
        let role_ids: Vec<String> = roles
            .into_iter()
            .filter_map(|r| r.role_id_from_env())
            .collect();
        if role_ids.is_empty() {
            return Err(ApiError::BadRequest(
                "No role IDs configured. Set AUTH0_ROLE_VIEWER_ID and/or AUTH0_ROLE_ADMIN_ID."
                    .to_string(),
            ));
        }

        let url = self.api_url(&format!("users/{}/roles", urlencoding::encode(user_id)));
        self.client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&RoleUpdateRequest { roles: role_ids })
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to add roles: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Add roles failed: {}", e)))?;
        Ok(())
    }

    pub async fn remove_roles(&self, user_id: &str, roles: Vec<RoleType>) -> Result<(), ApiError> {
        let role_ids: Vec<String> = roles
            .into_iter()
            .filter_map(|r| r.role_id_from_env())
            .collect();
        if role_ids.is_empty() {
            return Err(ApiError::BadRequest(
                "No role IDs configured. Set AUTH0_ROLE_VIEWER_ID and/or AUTH0_ROLE_ADMIN_ID."
                    .to_string(),
            ));
        }

        let url = self.api_url(&format!("users/{}/roles", urlencoding::encode(user_id)));
        self.client
            .delete(&url)
            .bearer_auth(&self.token)
            .json(&RoleUpdateRequest { roles: role_ids })
            .send()
            .await
            .map_err(|e| ApiError::Auth0ClientError(format!("Failed to remove roles: {}", e)))?
            .error_for_status()
            .map_err(|e| ApiError::Auth0ClientError(format!("Remove roles failed: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Auth0Client, CreateUserRequest, RoleType};

    #[tokio::test]
    #[ignore = "requires AUTH0_DOMAIN, AUTH0_MGMT_CLIENT_ID, AUTH0_MGMT_CLIENT_SECRET"]
    async fn test_create_delete_user() {
        let auth0_client = Auth0Client::new().await.unwrap();

        let new_user = auth0_client
            .create_user(CreateUserRequest {
                connection: "Username-Password-Authentication".to_owned(),
                password: "Abcd123$".to_owned(),
                email: "test-delete@bogus.email".to_owned(),
                email_verified: false,
                verify_email: false,
            })
            .await
            .unwrap();

        tracing::info!("Created test user: {:?}", new_user);

        auth0_client.delete_user(&new_user.user_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Auth0 env vars and AUTH0_ROLE_VIEWER_ID, AUTH0_ROLE_ADMIN_ID"]
    async fn test_add_remove_roles() {
        let auth0_client = Auth0Client::new().await.unwrap();

        let users = auth0_client.get_users().await.unwrap();
        let test_user = users
            .iter()
            .find(|u| u.email == "test@bogus.email")
            .expect("Create test@bogus.email in Auth0 first");

        assert_eq!(
            auth0_client
                .get_roles(&test_user.user_id)
                .await
                .unwrap()
                .len(),
            0
        );

        auth0_client
            .add_roles(&test_user.user_id, vec![RoleType::Viewer])
            .await
            .unwrap();

        assert_eq!(
            auth0_client
                .get_roles(&test_user.user_id)
                .await
                .unwrap()
                .len(),
            1
        );

        auth0_client
            .remove_roles(&test_user.user_id, vec![RoleType::Viewer])
            .await
            .unwrap();

        assert_eq!(
            auth0_client
                .get_roles(&test_user.user_id)
                .await
                .unwrap()
                .len(),
            0
        );
    }
}
