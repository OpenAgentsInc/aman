//! OpenSecret-based brain implementation.
//!
//! This crate provides a brain implementation that uses the OpenSecret SDK
//! to process messages through an AI model with end-to-end encryption.

mod config;
mod brain;
mod history;

pub use brain::MapleBrain;
pub use config::MapleBrainConfig;

// Re-export brain-core types for convenience
pub use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
