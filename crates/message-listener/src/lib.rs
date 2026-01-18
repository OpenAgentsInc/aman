//! Message listener utilities for Aman.
//!
//! This crate provides a high-level interface for receiving Signal messages
//! and processing them through a Brain implementation.
//!
//! # Basic Usage
//!
//! ```no_run
//! use message_listener::MessageListener;
//! use signal_daemon::DaemonConfig;
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), message_listener::Error> {
//! let config = DaemonConfig::default();
//! let listener = MessageListener::connect(config).await?;
//!
//! // Subscribe to incoming messages
//! let mut stream = listener.subscribe()?;
//!
//! while let Some(result) = stream.next().await {
//!     match result {
//!         Ok(envelope) => {
//!             if let Some(msg) = &envelope.data_message {
//!                 if let Some(text) = &msg.message {
//!                     println!("From {}: {}", envelope.source, text);
//!                 }
//!             }
//!         }
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Using MessageProcessor with a Brain
//!
//! ```no_run
//! use message_listener::{MessageProcessor, ProcessorConfig, EchoBrain};
//! use signal_daemon::{DaemonConfig, SignalClient};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = SignalClient::connect(DaemonConfig::default()).await?;
//! let brain = EchoBrain::with_prefix("Echo: ");
//! let config = ProcessorConfig::with_bot_number("+15551234567");
//!
//! let processor = MessageProcessor::new(client, brain, config);
//! processor.run().await?;
//! # Ok(())
//! # }
//! ```

mod processor;

use signal_daemon::{DaemonConfig, DaemonError, MessageStream, SignalClient};
use thiserror::Error;
use tracing::info;

// Re-export processor types
pub use processor::{MessageProcessor, ProcessorConfig, ProcessorError, ProcessResult};

// Re-export brain-core types for convenience
pub use brain_core::{Brain, BrainError, InboundAttachment, InboundMessage, OutboundMessage};

// Re-export mock brain implementations
pub use mock_brain::{EchoBrain, PrefixBrain, DelayedBrain};

// Re-export maple-brain when feature is enabled
#[cfg(feature = "maple")]
pub use maple_brain::{MapleBrain, MapleBrainConfig};

// Re-export Envelope and ReconnectConfig for users
pub use signal_daemon::{Envelope, ReconnectConfig};

/// Errors that can occur during message listening.
#[derive(Debug, Error)]
pub enum Error {
    /// Daemon communication error.
    #[error("Daemon error: {0}")]
    Daemon(#[from] DaemonError),
}

/// A listener for incoming Signal messages.
#[derive(Clone)]
pub struct MessageListener {
    client: SignalClient,
}

impl MessageListener {
    /// Connect to the signal-cli daemon and create a listener.
    pub async fn connect(config: DaemonConfig) -> Result<Self, Error> {
        let client = SignalClient::connect(config).await?;
        info!("MessageListener connected to daemon");
        Ok(Self { client })
    }

    /// Subscribe to incoming messages.
    ///
    /// Returns a stream of message envelopes. The stream will continue
    /// indefinitely until an error occurs or the connection is closed.
    ///
    /// # Errors
    ///
    /// Returns an error if the SSE connection cannot be established.
    pub fn subscribe(&self) -> Result<MessageStream, DaemonError> {
        signal_daemon::subscribe(&self.client)
    }

    /// Subscribe to incoming messages with custom reconnection configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the SSE connection cannot be established.
    pub fn subscribe_with_reconnect(
        &self,
        reconnect_config: ReconnectConfig,
    ) -> Result<MessageStream, DaemonError> {
        signal_daemon::subscribe_with_reconnect(&self.client, reconnect_config)
    }

    /// Get the underlying SignalClient.
    pub fn client(&self) -> &SignalClient {
        &self.client
    }

    /// Check if the listener is connected to the daemon.
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }
}

/// Re-export commonly used types for convenience.
pub use signal_daemon::{
    Attachment, DataMessage, GroupInfo, Mention, Quote, Reaction, ReceiptMessage, SyncMessage,
    TypingMessage,
};

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
