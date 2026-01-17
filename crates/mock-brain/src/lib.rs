//! Mock brain implementations for testing Signal bot message processing.
//!
//! This crate provides a `Brain` trait and mock implementations for testing
//! the message-listener and bot worker without requiring a real AI backend.
//!
//! # Example
//!
//! ```rust
//! use mock_brain::{Brain, EchoBrain, InboundMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mock_brain::BrainError> {
//!     let brain = EchoBrain::new();
//!
//!     let message = InboundMessage {
//!         sender: "+15551234567".to_string(),
//!         text: "Hello!".to_string(),
//!         timestamp: 1234567890,
//!         group_id: None,
//!     };
//!
//!     let response = brain.process(message).await?;
//!     println!("Response: {}", response.text);
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - `signal-daemon`: Enable integration with signal-daemon types for
//!   converting between Envelope and InboundMessage.

mod error;
mod message;
mod trait_def;

// Mock implementations
mod echo;
mod prefix;
mod delayed;

// Optional signal-daemon integration
#[cfg(feature = "signal-daemon")]
pub mod signal_integration;

pub use error::BrainError;
pub use message::{InboundMessage, OutboundMessage};
pub use trait_def::Brain;

// Export mock implementations
pub use echo::EchoBrain;
pub use prefix::PrefixBrain;
pub use delayed::DelayedBrain;

// Re-export signal integration types at crate root when feature is enabled
#[cfg(feature = "signal-daemon")]
pub use signal_integration::{EnvelopeExt, OutboundMessageExt, ProcessError, process_and_respond, send_response};
