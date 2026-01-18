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
    /// Proton Mail configuration (optional).
    pub proton: Option<proton_proxy::ProtonConfig>,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// | Variable | Description | Default |
    /// |----------|-------------|---------|
    /// | `ADMIN_ADDR` | Server bind address | `127.0.0.1:8788` |
    /// | `SQLITE_PATH` | SQLite database URL | `sqlite:aman.db?mode=rwc` |
    pub fn from_env() -> Result<Self, ConfigError> {
        let addr = env::var("ADMIN_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8788".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidAddr)?;

        let database_url = env::var("SQLITE_PATH")
            .unwrap_or_else(|_| "sqlite:aman.db?mode=rwc".to_string());

        // Proton config is optional - only load if credentials are set
        let proton = proton_proxy::ProtonConfig::from_env().ok();

        Ok(Self {
            addr,
            database_url,
            proton,
        })
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid ADMIN_ADDR format")]
    InvalidAddr,
}
