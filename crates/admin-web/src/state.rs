//! Application state shared across handlers.

use broadcaster::Broadcaster;
use database::Database;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Database connection.
    pub db: Database,
    /// Signal broadcaster.
    pub broadcaster: Broadcaster,
}

impl AppState {
    /// Create new application state.
    pub fn new(db: Database, broadcaster: Broadcaster) -> Self {
        Self { db, broadcaster }
    }
}
