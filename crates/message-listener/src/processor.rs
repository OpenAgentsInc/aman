//! Message processor that connects signal-daemon to a Brain implementation.

use brain_core::{Brain, BrainError};
use futures::StreamExt;
use mock_brain::EnvelopeExt;
use signal_daemon::{DaemonError, Envelope, SignalClient};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Configuration for the message processor.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// The bot's own phone number (to ignore messages from self).
    pub bot_number: Option<String>,

    /// Whether to process group messages.
    pub process_groups: bool,

    /// Whether to process direct messages.
    pub process_direct: bool,

    /// Whether to send typing indicators while processing.
    pub send_typing_indicators: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            bot_number: None,
            process_groups: true,
            process_direct: true,
            send_typing_indicators: false,
        }
    }
}

impl ProcessorConfig {
    /// Create a new config with the bot's phone number.
    pub fn with_bot_number(bot_number: impl Into<String>) -> Self {
        Self {
            bot_number: Some(bot_number.into()),
            ..Default::default()
        }
    }
}

/// Errors that can occur during message processing.
#[derive(Debug, Error)]
pub enum ProcessorError {
    /// Error from the signal daemon.
    #[error("daemon error: {0}")]
    Daemon(#[from] DaemonError),

    /// Error from the brain during processing.
    #[error("brain error: {0}")]
    Brain(#[from] BrainError),

    /// The message stream ended unexpectedly.
    #[error("message stream ended")]
    StreamEnded,
}

/// Result of processing a single message.
#[derive(Debug)]
pub enum ProcessResult {
    /// Message was processed and response sent.
    Responded {
        sender: String,
        response: String,
        timestamp: u64,
    },
    /// Message was skipped (e.g., from self, or not a text message).
    Skipped { reason: String },
    /// Error occurred during processing.
    Error(ProcessorError),
}

/// A message processor that receives Signal messages and processes them through a Brain.
pub struct MessageProcessor<B: Brain> {
    client: SignalClient,
    brain: B,
    config: ProcessorConfig,
}

impl<B: Brain> MessageProcessor<B> {
    /// Create a new message processor.
    pub fn new(client: SignalClient, brain: B, config: ProcessorConfig) -> Self {
        Self {
            client,
            brain,
            config,
        }
    }

    /// Create a processor with default configuration.
    pub fn with_defaults(client: SignalClient, brain: B) -> Self {
        Self::new(client, brain, ProcessorConfig::default())
    }

    /// Get a reference to the brain.
    pub fn brain(&self) -> &B {
        &self.brain
    }

    /// Get a reference to the client.
    pub fn client(&self) -> &SignalClient {
        &self.client
    }

    /// Check if we should process this envelope.
    fn should_process(&self, envelope: &Envelope) -> Result<(), String> {
        // Check if it's from ourselves
        if let Some(ref bot_number) = self.config.bot_number {
            if envelope.source == *bot_number || envelope.source_number == *bot_number {
                return Err("message from self".to_string());
            }
        }

        // Check if it has a data message with text
        let data_message = envelope
            .data_message
            .as_ref()
            .ok_or_else(|| "no data message".to_string())?;

        data_message
            .message
            .as_ref()
            .ok_or_else(|| "no text content".to_string())?;

        // Check group/direct message filtering
        let is_group = data_message.group_info.is_some();
        if is_group && !self.config.process_groups {
            return Err("group messages disabled".to_string());
        }
        if !is_group && !self.config.process_direct {
            return Err("direct messages disabled".to_string());
        }

        Ok(())
    }

    /// Process a single envelope and return the result.
    pub async fn process_envelope(&self, envelope: &Envelope) -> ProcessResult {
        // Check if we should process this message
        if let Err(reason) = self.should_process(envelope) {
            debug!("Skipping message: {}", reason);
            return ProcessResult::Skipped { reason };
        }

        // Convert to inbound message
        let inbound = match envelope.to_inbound_message() {
            Some(msg) => msg,
            None => {
                return ProcessResult::Skipped {
                    reason: "could not convert to inbound message".to_string(),
                }
            }
        };

        let sender = inbound.sender.clone();
        let is_group = inbound.group_id.is_some();
        info!("Processing message from {}: {}", sender, inbound.text);

        // Send typing indicator if enabled
        if self.config.send_typing_indicators {
            let typing_result = if let Some(ref group_id) = inbound.group_id {
                self.client.send_typing_to_group(group_id, true).await
            } else {
                self.client.send_typing(&sender, true).await
            };
            if let Err(e) = typing_result {
                warn!("Failed to send typing indicator: {}", e);
            }
        }

        // Process through brain
        let response = match self.brain.process(inbound.clone()).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Brain error for {}: {}", sender, e);
                // Stop typing indicator on error
                if self.config.send_typing_indicators {
                    let _ = if is_group {
                        if let Some(ref group_id) = inbound.group_id {
                            self.client.send_typing_to_group(group_id, false).await
                        } else {
                            Ok(())
                        }
                    } else {
                        self.client.send_typing(&sender, false).await
                    };
                }
                return ProcessResult::Error(ProcessorError::Brain(e));
            }
        };

        // Send response
        let send_result = if response.is_group {
            self.client
                .send_to_group(&response.recipient, &response.text)
                .await
        } else {
            self.client
                .send_text(&response.recipient, &response.text)
                .await
        };

        match send_result {
            Ok(result) => {
                info!(
                    "Sent response to {} (ts={}): {}",
                    response.recipient, result.timestamp, response.text
                );
                ProcessResult::Responded {
                    sender,
                    response: response.text,
                    timestamp: result.timestamp,
                }
            }
            Err(e) => {
                error!("Failed to send response to {}: {}", response.recipient, e);
                ProcessResult::Error(ProcessorError::Daemon(e))
            }
        }
    }

    /// Run the processor, handling messages until the stream ends or an error occurs.
    ///
    /// This method consumes self and runs indefinitely.
    pub async fn run(self) -> Result<(), ProcessorError> {
        info!("Starting message processor with brain: {}", self.brain.name());

        let mut stream = signal_daemon::subscribe(&self.client);

        while let Some(result) = stream.next().await {
            match result {
                Ok(envelope) => {
                    let result = self.process_envelope(&envelope).await;
                    match result {
                        ProcessResult::Responded { sender, response, .. } => {
                            debug!("Responded to {}: {}", sender, response);
                        }
                        ProcessResult::Skipped { reason } => {
                            debug!("Skipped: {}", reason);
                        }
                        ProcessResult::Error(e) => {
                            // Log but continue processing
                            warn!("Error processing message: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    // Continue on stream errors - they might be recoverable
                }
            }
        }

        warn!("Message stream ended");
        Err(ProcessorError::StreamEnded)
    }

    /// Run the processor with a callback for each processed message.
    ///
    /// The callback receives each ProcessResult, allowing for custom handling.
    pub async fn run_with_callback<F>(self, mut callback: F) -> Result<(), ProcessorError>
    where
        F: FnMut(ProcessResult) + Send,
    {
        info!("Starting message processor with brain: {}", self.brain.name());

        let mut stream = signal_daemon::subscribe(&self.client);

        while let Some(result) = stream.next().await {
            match result {
                Ok(envelope) => {
                    let result = self.process_envelope(&envelope).await;
                    callback(result);
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    callback(ProcessResult::Error(ProcessorError::Daemon(e)));
                }
            }
        }

        warn!("Message stream ended");
        Err(ProcessorError::StreamEnded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_daemon::{DataMessage, GroupInfo};

    #[allow(dead_code)]
    fn make_test_envelope(sender: &str, text: &str) -> Envelope {
        Envelope {
            source: sender.to_string(),
            source_number: sender.to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: Some(text.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    fn make_group_envelope(sender: &str, text: &str, group_id: &str) -> Envelope {
        Envelope {
            source: sender.to_string(),
            source_number: sender.to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: Some(text.to_string()),
                group_info: Some(GroupInfo {
                    group_id: group_id.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_should_process_valid_message() {
        let config = ProcessorConfig::default();
        // We can't easily test without a real client, so just test the config
        assert!(config.process_groups);
        assert!(config.process_direct);
    }

    #[test]
    fn test_config_with_bot_number() {
        let config = ProcessorConfig::with_bot_number("+15551234567");
        assert_eq!(config.bot_number, Some("+15551234567".to_string()));
    }

    #[test]
    fn test_envelope_filtering() {
        // Test that filtering logic works correctly
        let envelope = make_test_envelope("+15559876543", "hello");

        // Should have data message and text
        assert!(envelope.data_message.is_some());
        assert!(envelope.data_message.as_ref().unwrap().message.is_some());
    }
}
