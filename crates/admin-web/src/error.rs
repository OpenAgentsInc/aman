//! Error types for the admin web interface.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use thiserror::Error;

/// Errors that can occur in the admin web interface.
#[derive(Debug, Error)]
pub enum AdminError {
    /// Database error.
    #[error("Database error: {0}")]
    Database(#[from] database::DatabaseError),

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AdminError::Database(err) => {
                tracing::error!("Database error: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            AdminError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        };

        let body = serde_json::json!({
            "error": message
        });

        (status, Json(body)).into_response()
    }
}

/// Result type for admin operations.
pub type Result<T> = std::result::Result<T, AdminError>;
