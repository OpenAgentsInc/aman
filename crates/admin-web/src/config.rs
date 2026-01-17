//! Configuration loaded from environment variables.

use std::env;
use std::net::SocketAddr;

/// Admin web server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Server bind address.
    pub addr: SocketAddr,
    /// SQLite database URL.
    pub database_url: String,
    /// Signal daemon URL.
    pub signal_daemon_url: String,
    /// Bot phone number.
    pub aman_number: String,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// | Variable | Description | Default |
    /// |----------|-------------|---------|
    /// | `ADMIN_ADDR` | Server bind address | `127.0.0.1:8788` |
    /// | `SQLITE_PATH` | SQLite database URL | `sqlite:aman.db?mode=rwc` |
    /// | `SIGNAL_DAEMON_URL` | Signal daemon URL | `http://127.0.0.1:8080` |
    /// | `AMAN_NUMBER` | Bot phone number | (required) |
    pub fn from_env() -> Result<Self, ConfigError> {
        let addr = env::var("ADMIN_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8788".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidAddr)?;

        let database_url = env::var("SQLITE_PATH")
            .unwrap_or_else(|_| "sqlite:aman.db?mode=rwc".to_string());

        let signal_daemon_url = env::var("SIGNAL_DAEMON_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let aman_number = env::var("AMAN_NUMBER")
            .map_err(|_| ConfigError::MissingAmanNumber)?;

        Ok(Self {
            addr,
            database_url,
            signal_daemon_url,
            aman_number,
        })
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid ADMIN_ADDR format")]
    InvalidAddr,

    #[error("AMAN_NUMBER environment variable is required")]
    MissingAmanNumber,
}
