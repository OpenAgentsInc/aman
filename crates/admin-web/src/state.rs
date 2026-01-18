//! Application state shared across handlers.

use broadcaster::Broadcaster;
use database::Database;
use proton_proxy::ProtonConfig;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Database connection.
    pub db: Database,
    /// Signal broadcaster (optional - not needed for dashboard-only mode).
    pub broadcaster: Option<Broadcaster>,
    /// Proton Mail configuration (optional).
    pub proton_config: Option<ProtonConfig>,
}

impl AppState {
    /// Create new application state.
    pub fn new(db: Database, broadcaster: Option<Broadcaster>, proton_config: Option<ProtonConfig>) -> Self {
        Self { db, broadcaster, proton_config }
    }
}
