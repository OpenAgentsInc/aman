//! Route handlers for the admin web interface.

pub mod broadcast;
pub mod dashboard;
pub mod health;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// Build the router with all routes.
pub fn router() -> Router<AppState> {
    Router::new()
        // HTML pages
        .route("/", get(dashboard::dashboard_page))
        .route("/broadcast", get(broadcast::broadcast_page))
        // Health check
        .route("/health", get(health::health))
        // API endpoints
        .route("/api/stats", get(dashboard::stats_api))
        .route("/api/topics", get(broadcast::topics_api))
        .route("/api/broadcast/preview", post(broadcast::preview_api))
        .route("/api/broadcast", post(broadcast::send_api))
}
