//! MapleBrain implementation using OpenSecret SDK.

use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
use opensecret::{OpenSecretClient, types::{ChatCompletionRequest, ChatMessage}};
use tracing::{debug, info, warn};

use crate::config::MapleBrainConfig;
use crate::history::ConversationHistory;

/// A brain implementation that uses OpenSecret SDK for AI processing.
///
/// MapleBrain maintains per-sender conversation history and communicates
/// with the OpenSecret API for end-to-end encrypted AI interactions.
pub struct MapleBrain {
    client: OpenSecretClient,
    config: MapleBrainConfig,
    history: ConversationHistory,
}

impl MapleBrain {
    /// Create a new MapleBrain with the given configuration.
    ///
    /// This performs the attestation handshake with the OpenSecret API,
    /// which establishes a secure encrypted session.
    pub async fn new(config: MapleBrainConfig) -> Result<Self, BrainError> {
        let client = OpenSecretClient::new_with_api_key(&config.api_url, config.api_key.clone())
            .map_err(|e| BrainError::Configuration(format!("Failed to create OpenSecret client: {}", e)))?;

        // Perform attestation handshake to establish secure session
        info!("Performing attestation handshake with OpenSecret API...");
        client.perform_attestation_handshake().await
            .map_err(|e| BrainError::Network(format!("Attestation handshake failed: {}", e)))?;
        info!("Attestation handshake complete");

        let history = ConversationHistory::new(config.max_history_turns);

        Ok(Self {
            client,
            config,
            history,
        })
    }

    /// Create a MapleBrain from environment variables.
    ///
    /// See [`MapleBrainConfig::from_env`] for required environment variables.
    pub async fn from_env() -> Result<Self, BrainError> {
        let config = MapleBrainConfig::from_env()?;
        Self::new(config).await
    }

    /// Get the configuration.
    pub fn config(&self) -> &MapleBrainConfig {
        &self.config
    }

    /// Clear conversation history for a specific sender.
    pub async fn clear_history(&self, sender: &str) {
        self.history.clear(sender).await;
    }

    /// Clear all conversation histories.
    pub async fn clear_all_history(&self) {
        self.history.clear_all().await;
    }

    /// Build the messages array for a chat completion request.
    async fn build_messages(&self, sender: &str, user_text: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // Add system prompt if configured
        if let Some(ref system_prompt) = self.config.system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: serde_json::Value::String(system_prompt.clone()),
                tool_calls: None,
            });
        }

        // Add conversation history
        let history = self.history.get(sender).await;
        for msg in history {
            messages.push(ChatMessage {
                role: msg.role,
                content: serde_json::Value::String(msg.content),
                tool_calls: None,
            });
        }

        // Add current user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: serde_json::Value::String(user_text.to_string()),
            tool_calls: None,
        });

        messages
    }
}

#[async_trait]
impl Brain for MapleBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        let sender = &message.sender;
        let user_text = &message.text;

        debug!("Processing message from {}: {}", sender, user_text);

        // Build messages with history
        let messages = self.build_messages(sender, user_text).await;

        // Create the chat completion request
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens.map(|t| t as i32),
            stream: Some(false),
            stream_options: None,
            tools: None,
            tool_choice: None,
        };

        // Call the OpenSecret API
        let response = self.client.create_chat_completion(request).await
            .map_err(|e| {
                warn!("OpenSecret API error: {}", e);
                BrainError::Network(format!("OpenSecret API error: {}", e))
            })?;

        // Extract the response text
        let response_text = response
            .choices
            .first()
            .and_then(|choice| {
                choice.message.content.as_str().map(|s| s.to_string())
            })
            .unwrap_or_else(|| {
                warn!("No response content from OpenSecret API");
                "I'm sorry, I couldn't generate a response.".to_string()
            });

        info!("Generated response for {}: {} chars", sender, response_text.len());

        // Add to conversation history
        self.history.add_exchange(sender, user_text, &response_text).await;

        // Return the outbound message
        Ok(OutboundMessage::reply_to(&message, response_text))
    }

    fn name(&self) -> &str {
        "MapleBrain"
    }

    async fn is_ready(&self) -> bool {
        // Could add a health check here in the future
        true
    }

    async fn shutdown(&self) -> Result<(), BrainError> {
        // Clear all history on shutdown
        self.history.clear_all().await;
        Ok(())
    }
}
