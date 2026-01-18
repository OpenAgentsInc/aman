//! Route handlers for the admin web interface.

pub mod dashboard;
pub mod health;

use axum::routing::get;
use axum::Router;

use crate::state::AppState;

/// Build the router with all routes.
pub fn router() -> Router<AppState> {
    Router::new()
        // HTML pages
        .route("/", get(dashboard::dashboard_page))
        // Health check
        .route("/health", get(health::health))
        // API endpoints
        .route("/api/stats", get(dashboard::stats_api))
}
