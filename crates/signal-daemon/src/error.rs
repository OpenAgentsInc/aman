//! Error types for signal-daemon.

use thiserror::Error;

/// Errors that can occur when interacting with the signal-cli daemon.
#[derive(Debug, Error)]
pub enum DaemonError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// JSON-RPC error response from daemon.
    #[error("RPC error {code}: {message}")]
    Rpc { code: i32, message: String },

    /// Connection to daemon failed.
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Daemon health check failed.
    #[error("Health check failed")]
    HealthCheckFailed,

    /// SSE stream error.
    #[error("SSE error: {0}")]
    Sse(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// Message sending failed.
    #[error("Send failed: {0}")]
    SendFailed(String),
}
