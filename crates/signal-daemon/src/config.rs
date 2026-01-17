//! Configuration types for signal-daemon.

use std::path::PathBuf;

/// Configuration for connecting to the signal-cli daemon.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Base URL of the daemon HTTP server (e.g., "http://localhost:8080").
    pub base_url: String,
    /// Account phone number for multi-account mode.
    /// If None, assumes single-account mode.
    pub account: Option<String>,
    /// Path to signal-cli data directory.
    /// Defaults to `~/.local/share/signal-cli` on Linux.
    pub data_dir: PathBuf,
}

impl DaemonConfig {
    /// Create a new configuration with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            account: None,
            data_dir: default_data_dir(),
        }
    }

    /// Create configuration with a specific account for multi-account mode.
    pub fn with_account(base_url: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            account: Some(account.into()),
            data_dir: default_data_dir(),
        }
    }

    /// Set a custom data directory.
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = dir.into();
        self
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

    /// Get the path to the attachments directory.
    pub fn attachments_dir(&self) -> PathBuf {
        self.data_dir.join("attachments")
    }

    /// Get the full path to an attachment by its ID.
    ///
    /// The attachment ID is typically just the filename (e.g., "xKRB...jpeg").
    /// This method returns the full path to the file.
    pub fn attachment_path(&self, attachment_id: &str) -> PathBuf {
        self.attachments_dir().join(attachment_id)
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}

/// Get the default signal-cli data directory.
///
/// Uses `$XDG_DATA_HOME/signal-cli` or `$HOME/.local/share/signal-cli`.
fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("signal-cli")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share/signal-cli")
    } else {
        // Fallback
        PathBuf::from(".local/share/signal-cli")
    }
}
