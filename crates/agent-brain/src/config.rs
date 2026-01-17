//! Configuration for AgentBrain.

use std::env;

use brain_core::BrainError;

/// Configuration for AgentBrain.
#[derive(Debug, Clone)]
pub struct AgentBrainConfig {
    /// SQLite URL for the Aman state database.
    pub sqlite_url: String,
    /// Default language to store for new users.
    pub default_language: String,
}

impl AgentBrainConfig {
    /// Create a new config from a SQLite path or URL.
    pub fn from_sqlite_path(path: impl Into<String>) -> Self {
        let sqlite_path = path.into();
        let sqlite_url = sqlite_url_from_path(&sqlite_path);
        Self {
            sqlite_url,
            default_language: "English".to_string(),
        }
    }

    /// Load configuration from environment variables.
    ///
    /// Required env vars:
    /// - `SQLITE_PATH` (path or sqlite URL)
    ///
    /// Optional env vars:
    /// - `AMAN_DEFAULT_LANGUAGE` (default: English)
    pub fn from_env() -> Result<Self, BrainError> {
        let sqlite_path = env::var("SQLITE_PATH")
            .unwrap_or_else(|_| "./data/aman.db".to_string());
        let sqlite_url = sqlite_url_from_path(&sqlite_path);
        let default_language = env::var("AMAN_DEFAULT_LANGUAGE")
            .unwrap_or_else(|_| "English".to_string());

        Ok(Self {
            sqlite_url,
            default_language,
        })
    }
}

fn sqlite_url_from_path(path: &str) -> String {
    if path.starts_with("sqlite:") {
        path.to_string()
    } else {
        format!("sqlite:{}?mode=rwc", path)
    }
}
