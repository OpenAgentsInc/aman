//! Broadcast utilities for Aman.
//!
//! This crate provides a high-level interface for sending Signal messages
//! using the signal-cli daemon.
//!
//! # Example
//!
//! ```no_run
//! use broadcaster::Broadcaster;
//! use signal_daemon::DaemonConfig;
//!
//! # async fn example() -> Result<(), broadcaster::Error> {
//! let config = DaemonConfig::default();
//! let broadcaster = Broadcaster::connect(config).await?;
//!
//! // Send a text message
//! broadcaster.send_text("+1234567890", "Hello!").await?;
//!
//! // Send to a group
//! broadcaster.send_to_group("GROUP_ID", "Hello group!").await?;
//! # Ok(())
//! # }
//! ```

use signal_daemon::{DaemonConfig, DaemonError, SendParams, SendResult, SignalClient};
use thiserror::Error;
use tracing::info;

/// Errors that can occur during broadcast operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Daemon communication error.
    #[error("Daemon error: {0}")]
    Daemon(#[from] DaemonError),
}

/// A broadcaster for sending Signal messages.
#[derive(Clone)]
pub struct Broadcaster {
    client: SignalClient,
}

impl Broadcaster {
    /// Connect to the signal-cli daemon and create a broadcaster.
    pub async fn connect(config: DaemonConfig) -> Result<Self, Error> {
        let client = SignalClient::connect(config).await?;
        info!("Broadcaster connected to daemon");
        Ok(Self { client })
    }

    /// Send a text message to a recipient.
    pub async fn send_text(&self, recipient: &str, message: &str) -> Result<SendResult, Error> {
        info!(recipient = %recipient, "Sending text message");
        self.client.send_text(recipient, message).await.map_err(Error::from)
    }

    /// Send a text message to a group.
    pub async fn send_to_group(&self, group_id: &str, message: &str) -> Result<SendResult, Error> {
        info!(group_id = %group_id, "Sending group message");
        self.client.send_to_group(group_id, message).await.map_err(Error::from)
    }

    /// Send a message with full parameters.
    pub async fn send(&self, params: SendParams) -> Result<SendResult, Error> {
        self.client.send(params).await.map_err(Error::from)
    }

    /// Get the underlying SignalClient.
    pub fn client(&self) -> &SignalClient {
        &self.client
    }

    /// Check if the broadcaster is connected to the daemon.
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }
}

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
