//! xAI Grok-based brain implementation.
//!
//! This crate provides a brain implementation that uses the xAI Grok API
//! to process messages with optional real-time search capabilities.
//!
//! # Features
//!
//! - Uses xAI's Grok 4.1 Fast model for quick responses
//! - Per-sender conversation history
//! - Optional X Search for real-time Twitter/X data
//! - Optional Web Search for current web information
//! - Configurable via environment variables
//!
//! # Example
//!
//! ```rust,no_run
//! use grok_brain::GrokBrain;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let brain = GrokBrain::from_env().await?;
//!     // Use the brain...
//!     Ok(())
//! }
//! ```

mod api_types;
mod brain;
mod config;
mod history;

pub use brain::GrokBrain;
pub use config::GrokBrainConfig;

// Re-export brain-core types for convenience
pub use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
