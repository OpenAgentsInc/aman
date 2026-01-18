//! Message sender trait and implementations.

use async_trait::async_trait;

use crate::error::OrchestratorError;

/// Trait for sending messages and typing indicators.
///
/// Abstracted to support different transports (Signal, tests, etc.)
#[async_trait]
pub trait MessageSender: Send + Sync {
    /// Send a text message.
    ///
    /// # Arguments
    /// * `recipient` - Phone number or group ID
    /// * `text` - Message content
    /// * `is_group` - Whether this is a group message
    async fn send_message(
        &self,
        recipient: &str,
        text: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError>;

    /// Set typing indicator state.
    ///
    /// # Arguments
    /// * `recipient` - Phone number or group ID
    /// * `is_group` - Whether this is a group
    /// * `started` - true to start typing, false to stop
    async fn set_typing(
        &self,
        recipient: &str,
        is_group: bool,
        started: bool,
    ) -> Result<(), OrchestratorError>;
}

/// A no-op message sender for testing that discards all messages.
#[derive(Debug, Clone, Default)]
pub struct NoOpSender;

#[async_trait]
impl MessageSender for NoOpSender {
    async fn send_message(
        &self,
        _recipient: &str,
        _text: &str,
        _is_group: bool,
    ) -> Result<(), OrchestratorError> {
        Ok(())
    }

    async fn set_typing(
        &self,
        _recipient: &str,
        _is_group: bool,
        _started: bool,
    ) -> Result<(), OrchestratorError> {
        Ok(())
    }
}

/// A logging message sender for debugging that logs all operations.
#[derive(Debug, Clone, Default)]
pub struct LoggingSender;

#[async_trait]
impl MessageSender for LoggingSender {
    async fn send_message(
        &self,
        recipient: &str,
        text: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        let msg_type = if is_group { "group" } else { "direct" };
        tracing::info!("[{}] Sending {} message to {}: {}", msg_type, msg_type, recipient, text);
        Ok(())
    }

    async fn set_typing(
        &self,
        recipient: &str,
        is_group: bool,
        started: bool,
    ) -> Result<(), OrchestratorError> {
        let msg_type = if is_group { "group" } else { "direct" };
        let state = if started { "started" } else { "stopped" };
        tracing::info!("[{}] Typing {} for {}", msg_type, state, recipient);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_sender() {
        let sender = NoOpSender;

        // Should not error
        sender.send_message("+1234567890", "test", false).await.unwrap();
        sender.set_typing("+1234567890", false, true).await.unwrap();
    }

    #[tokio::test]
    async fn test_logging_sender() {
        let sender = LoggingSender;

        // Should not error
        sender.send_message("+1234567890", "test", false).await.unwrap();
        sender.send_message("group123", "test", true).await.unwrap();
        sender.set_typing("+1234567890", false, true).await.unwrap();
        sender.set_typing("+1234567890", false, false).await.unwrap();
    }
}
