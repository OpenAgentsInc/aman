//! Configuration types for signal-daemon.

/// Configuration for connecting to the signal-cli daemon.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Base URL of the daemon HTTP server (e.g., "http://localhost:8080").
    pub base_url: String,
    /// Account phone number for multi-account mode.
    /// If None, assumes single-account mode.
    pub account: Option<String>,
}

impl DaemonConfig {
    /// Create a new configuration with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            account: None,
        }
    }

    /// Create configuration with a specific account for multi-account mode.
    pub fn with_account(base_url: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            account: Some(account.into()),
        }
    }

    /// Get the RPC endpoint URL.
    pub fn rpc_url(&self) -> String {
        format!("{}/api/v1/rpc", self.base_url)
    }

    /// Get the events endpoint URL (with account query param if set).
    pub fn events_url(&self) -> String {
        match &self.account {
            Some(account) => {
                let encoded = urlencoding::encode(account);
                format!("{}/api/v1/events?account={}", self.base_url, encoded)
            }
            None => format!("{}/api/v1/events", self.base_url),
        }
    }

    /// Get the health check endpoint URL.
    pub fn check_url(&self) -> String {
        format!("{}/api/v1/check", self.base_url)
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}
