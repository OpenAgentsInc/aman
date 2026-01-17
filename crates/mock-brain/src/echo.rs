//! Echo brain implementation - echoes messages back.

use async_trait::async_trait;

use crate::error::BrainError;
use crate::message::{InboundMessage, OutboundMessage};
use crate::trait_def::Brain;

/// A simple brain that echoes messages back to the sender.
///
/// Useful for testing the message flow without any AI processing.
#[derive(Debug, Clone, Default)]
pub struct EchoBrain {
    /// Optional prefix to add before the echo.
    prefix: Option<String>,
}

impl EchoBrain {
    /// Create a new EchoBrain with no prefix.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new EchoBrain with a custom prefix.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mock_brain::EchoBrain;
    ///
    /// let brain = EchoBrain::with_prefix("Echo: ");
    /// // Will respond with "Echo: <original message>"
    /// ```
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
        }
    }
}

#[async_trait]
impl Brain for EchoBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        let response_text = match &self.prefix {
            Some(prefix) => format!("{}{}", prefix, message.text),
            None => message.text.clone(),
        };

        Ok(OutboundMessage::reply_to(&message, response_text))
    }

    fn name(&self) -> &str {
        "EchoBrain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_no_prefix() {
        let brain = EchoBrain::new();
        let msg = InboundMessage::direct("+15551234567", "Hello!", 1234567890);

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, "Hello!");
        assert_eq!(response.recipient, "+15551234567");
        assert!(!response.is_group);
    }

    #[tokio::test]
    async fn test_echo_with_prefix() {
        let brain = EchoBrain::with_prefix("Echo: ");
        let msg = InboundMessage::direct("+15551234567", "Hello!", 1234567890);

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, "Echo: Hello!");
    }

    #[tokio::test]
    async fn test_echo_group_message() {
        let brain = EchoBrain::new();
        let msg = InboundMessage::group("+15551234567", "Hello group!", 1234567890, "group123");

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, "Hello group!");
        assert_eq!(response.recipient, "group123");
        assert!(response.is_group);
    }

    #[tokio::test]
    async fn test_brain_name() {
        let brain = EchoBrain::new();
        assert_eq!(brain.name(), "EchoBrain");
    }

    #[tokio::test]
    async fn test_brain_is_ready() {
        let brain = EchoBrain::new();
        assert!(brain.is_ready().await);
    }
}
