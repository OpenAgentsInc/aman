//! # proton-proxy
//!
//! SMTP client for sending end-to-end encrypted email via Proton Mail Bridge.
//!
//! ## Example
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

mod client;
mod config;
mod error;
mod types;

pub use client::ProtonClient;
pub use config::ProtonConfig;
pub use error::ProtonError;
pub use types::{Attachment, Email};
