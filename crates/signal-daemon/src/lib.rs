//! Signal-cli daemon client library.
//!
//! This crate provides a Rust client for communicating with the signal-cli daemon
//! over HTTP. It supports:
//!
//! - Sending messages to individuals and groups
//! - Receiving messages via Server-Sent Events (SSE)
//! - Health checking and connection monitoring
//!
//! # Example
//!
//! ```no_run
//! use signal_daemon::{DaemonConfig, SignalClient};
//!
//! # async fn example() -> Result<(), signal_daemon::DaemonError> {
//! // Connect to the daemon
//! let config = DaemonConfig::default();
//! let client = SignalClient::connect(config).await?;
//!
//! // Send a message
//! let result = client.send_text("+1234567890", "Hello!").await?;
//! println!("Sent at timestamp: {}", result.timestamp);
//!
//! // Subscribe to incoming messages
//! use futures::StreamExt;
//! let mut messages = signal_daemon::subscribe(&client)?;
//! while let Some(result) = messages.next().await {
//!     match result {
//!         Ok(envelope) => {
//!             if let Some(msg) = envelope.data_message {
//!                 println!("From {}: {:?}", envelope.source, msg.message);
//!             }
//!         }
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod process;
pub mod sse;
pub mod types;

pub use client::SignalClient;
pub use config::DaemonConfig;
pub use error::DaemonError;
pub use process::{spawn_and_connect, DaemonProcess, ProcessConfig, DEFAULT_JAR_PATH};
pub use sse::{subscribe, subscribe_with_reconnect, MessageStream, ReconnectConfig};
pub use types::*;

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
