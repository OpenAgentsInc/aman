//! Admin web interface for Aman Signal bot.
//!
//! Provides a dashboard via HTMX + server-rendered HTML.

mod config;
mod error;
mod routes;
mod state;

use database::Database;
use tower_http::services::ServeDir;
use tracing::info;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::from_env()?;
    info!(addr = %config.addr, "Starting admin web server");

    // Connect to database
    let db = Database::connect(&config.database_url).await?;
    db.migrate().await?;

    // Build application state
    let state = AppState::new(db, config.proton);

    // Build router
    let app = routes::router()
        .nest_service("/static", ServeDir::new("crates/admin-web/static"))
        .with_state(state);

    // Start server
    info!(addr = %config.addr, "Admin web server listening");
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
