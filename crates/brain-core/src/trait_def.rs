//! The Brain trait definition.

use async_trait::async_trait;

use crate::error::BrainError;
use crate::message::{InboundMessage, OutboundMessage};

/// A trait for processing inbound messages and generating responses.
///
/// Implementations can range from simple echo bots to full AI backends.
/// This trait is object-safe and can be used with `Box<dyn Brain>`.
#[async_trait]
pub trait Brain: Send + Sync {
    /// Process an inbound message and generate a response.
    ///
    /// # Arguments
    ///
    /// * `message` - The incoming message to process.
    ///
    /// # Returns
    ///
    /// An `OutboundMessage` containing the response, or an error if
    /// processing failed.
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError>;

    /// Get a human-readable name for this brain implementation.
    fn name(&self) -> &str;

    /// Check if the brain is ready to process messages.
    ///
    /// Default implementation always returns true.
    async fn is_ready(&self) -> bool {
        true
    }

    /// Gracefully shut down the brain.
    ///
    /// Default implementation does nothing.
    async fn shutdown(&self) -> Result<(), BrainError> {
        Ok(())
    }
}
