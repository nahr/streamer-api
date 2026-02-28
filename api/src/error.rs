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
    Db(#[from] polodb_core::Error),

    #[error("Admin already exists")]
    AdminExists,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::AdminExists => (StatusCode::CONFLICT, "Admin already exists"),
            ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "Bad request"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        (status, message.to_string()).into_response()
    }
}
