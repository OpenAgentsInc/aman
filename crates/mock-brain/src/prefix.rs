//! Prefix brain implementation - transforms messages with prefix/suffix.

use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};

/// A brain that transforms messages by adding prefix and/or suffix.
///
/// Useful for testing message transformation without AI.
#[derive(Debug, Clone)]
pub struct PrefixBrain {
    prefix: String,
    suffix: String,
}

impl PrefixBrain {
    /// Create a new PrefixBrain with the given prefix and suffix.
    pub fn new(prefix: impl Into<String>, suffix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            suffix: suffix.into(),
        }
    }

    /// Create a brain that wraps messages in quotes.
    pub fn quoted() -> Self {
        Self::new("\"", "\"")
    }

    /// Create a brain that formats as a bot response.
    pub fn bot_response() -> Self {
        Self::new("[Bot] ", "")
    }
}

impl Default for PrefixBrain {
    fn default() -> Self {
        Self::new("", "")
    }
}

#[async_trait]
impl Brain for PrefixBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        let response_text = format!("{}{}{}", self.prefix, message.text, self.suffix);
        Ok(OutboundMessage::reply_to(&message, response_text))
    }

    fn name(&self) -> &str {
        "PrefixBrain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prefix_suffix() {
        let brain = PrefixBrain::new(">>", "<<");
        let msg = InboundMessage::direct("+15551234567", "test", 1234567890);

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, ">>test<<");
    }

    #[tokio::test]
    async fn test_quoted() {
        let brain = PrefixBrain::quoted();
        let msg = InboundMessage::direct("+15551234567", "hello", 1234567890);

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, "\"hello\"");
    }

    #[tokio::test]
    async fn test_bot_response() {
        let brain = PrefixBrain::bot_response();
        let msg = InboundMessage::direct("+15551234567", "help", 1234567890);

        let response = brain.process(msg).await.unwrap();
        assert_eq!(response.text, "[Bot] help");
    }
}
