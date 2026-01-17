//! Health check endpoint.

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct Health {
    pub status: String,
}

/// Health check endpoint.
pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
    })
}
