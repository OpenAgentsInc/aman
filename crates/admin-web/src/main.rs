//! Admin web interface for Aman Signal bot.
//!
//! Provides a dashboard and broadcast functionality via HTMX + server-rendered HTML.

mod config;
mod error;
mod routes;
mod state;

use broadcaster::Broadcaster;
use database::Database;
use signal_daemon::DaemonConfig;
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

    // Connect to broadcaster
    let daemon_config = DaemonConfig::with_account(
        &config.signal_daemon_url,
        &config.aman_number,
    );
    let broadcaster = Broadcaster::connect(daemon_config).await?;

    // Build application state
    let state = AppState::new(db, broadcaster);

    // Build router
    let app = routes::router()
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // Start server
    info!(addr = %config.addr, "Admin web server listening");
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
