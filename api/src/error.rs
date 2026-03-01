use axum::{http::StatusCode, response::IntoResponse};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Unknown error occurred: {0}")]
    Unknown(String),

    #[error("Invalid server address: {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Server I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Internal camera already exists")]
    InternalCameraExists,

    #[error("Camera not found")]
    CameraNotFound,

    #[error("Pool match not found")]
    PoolMatchNotFound,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Auth0 error: {0}")]
    Auth0ClientError(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::InternalCameraExists => (StatusCode::CONFLICT, "Internal camera already exists".to_string()),
            ApiError::CameraNotFound => (StatusCode::NOT_FOUND, "Camera not found".to_string()),
            ApiError::PoolMatchNotFound => (StatusCode::NOT_FOUND, "Pool match not found".to_string()),
            ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()),
            ApiError::Auth0ClientError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %message, "API returned 500");
        }
        (status, message).into_response()
    }
}
