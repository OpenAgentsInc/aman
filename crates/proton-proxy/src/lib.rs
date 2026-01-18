//! # proton-proxy
//!
//! SMTP/IMAP client for sending and receiving end-to-end encrypted email via Proton Mail Bridge.
//!
//! ## Sending Email
//!
//! ```no_run
//! use proton_proxy::{ProtonClient, ProtonConfig, Email};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), proton_proxy::ProtonError> {
//!     let config = ProtonConfig::from_env()?;
//!     let client = ProtonClient::new(config)?;
//!     
//!     let email = Email::new("recipient@proton.me", "Hello", "E2E encrypted message");
//!     client.send(&email).await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Watching a Folder
//!
//! ```no_run
//! use proton_proxy::{InboxWatcher, ProtonConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), proton_proxy::ProtonError> {
//!     let config = ProtonConfig::from_env()?;
//!     let mut watcher = InboxWatcher::new(config);
//!     
//!     watcher.watch("INBOX", |msg| async move {
//!         println!("New message: {}", msg.subject);
//!         Ok(())
//!     }).await?;
//!     
//!     Ok(())
//! }
//! ```

mod client;
mod config;
mod error;
mod imap_client;
mod types;
mod watcher;

pub use client::ProtonClient;
pub use config::ProtonConfig;
pub use error::ProtonError;
pub use imap_client::ImapClient;
pub use types::{Attachment, Email, InboxAttachment, InboxMessage};
pub use watcher::InboxWatcher;
