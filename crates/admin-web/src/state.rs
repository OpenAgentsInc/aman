//! Application state shared across handlers.

use database::Database;
use proton_proxy::ProtonConfig;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Database connection.
    pub db: Database,
    /// Proton Mail configuration (optional).
    pub proton_config: Option<ProtonConfig>,
}

impl AppState {
    /// Create new application state.
    pub fn new(db: Database, proton_config: Option<ProtonConfig>) -> Self {
        Self { db, proton_config }
    }
}
