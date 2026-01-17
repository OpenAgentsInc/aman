//! Mock brain implementations for Signal bot message processing.
//!
//! This crate provides mock implementations of the `Brain` trait for testing:
//! - `EchoBrain` - Echoes messages back
//! - `PrefixBrain` - Adds prefix/suffix to messages
//! - `DelayedBrain` - Wraps another brain with artificial delay
//!
//! For production AI processing, use the `maple-brain` crate instead.
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
//!     let message = InboundMessage::direct("+15551234567", "Hello!", 1234567890);
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

// Mock implementations
mod echo;
mod prefix;
mod delayed;

// Optional signal-daemon integration
#[cfg(feature = "signal-daemon")]
pub mod signal_integration;

// Re-export brain-core types for convenience
pub use brain_core::{async_trait, Brain, BrainError, InboundAttachment, InboundMessage, OutboundMessage};

// Export mock implementations
pub use echo::EchoBrain;
pub use prefix::PrefixBrain;
pub use delayed::DelayedBrain;

// Re-export signal integration types at crate root when feature is enabled
#[cfg(feature = "signal-daemon")]
pub use signal_integration::{EnvelopeExt, OutboundMessageExt, ProcessError, process_and_respond, send_response};
