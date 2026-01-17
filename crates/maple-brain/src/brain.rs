//! MapleBrain implementation using OpenSecret SDK.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use brain_core::{async_trait, Brain, BrainError, InboundAttachment, InboundMessage, OutboundMessage};
use futures::StreamExt;
use opensecret::{OpenSecretClient, types::{ChatCompletionRequest, ChatMessage}};
use tokio::fs;
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

    /// Build a multimodal message with text and images for vision models.
    async fn build_vision_message(&self, user_text: &str, attachments: &[InboundAttachment]) -> Result<serde_json::Value, BrainError> {
        let mut content_parts = Vec::new();

        // Add text part
        let text = if user_text.is_empty() {
            "What do you see in this image?".to_string()
        } else {
            user_text.to_string()
        };
        content_parts.push(serde_json::json!({
            "type": "text",
            "text": text
        }));

        // Add image parts
        for attachment in attachments {
            if !attachment.is_image() {
                continue;
            }

            if let Some(ref file_path) = attachment.file_path {
                match self.load_image_as_base64(file_path, &attachment.content_type).await {
                    Ok(data_url) => {
                        content_parts.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": data_url
                            }
                        }));
                        debug!("Added image from {}", file_path);
                    }
                    Err(e) => {
                        warn!("Failed to load image {}: {}", file_path, e);
                    }
                }
            }
        }

        Ok(serde_json::Value::Array(content_parts))
    }

    /// Load an image file and encode it as a data URL.
    async fn load_image_as_base64(&self, file_path: &str, content_type: &str) -> Result<String, BrainError> {
        let bytes = fs::read(file_path).await
            .map_err(|e| BrainError::ProcessingFailed(format!("Failed to read image file: {}", e)))?;

        let base64_data = BASE64.encode(&bytes);
        let data_url = format!("data:{};base64,{}", content_type, base64_data);

        Ok(data_url)
    }
}

#[async_trait]
impl Brain for MapleBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        let sender = &message.sender;
        let user_text = &message.text;
        let has_images = message.has_images();

        debug!("Processing message from {}: {} (images: {})", sender, user_text, has_images);

        // Choose model and build messages based on whether we have images
        let (model, messages) = if has_images {
            // Use vision model for messages with images
            info!("Using vision model for message with {} image(s)",
                  message.attachments.iter().filter(|a| a.is_image()).count());

            let vision_content = self.build_vision_message(user_text, &message.attachments).await?;

            // For vision, we don't include history (images make context complex)
            // Just add system prompt if present and the vision message
            let mut msgs = Vec::new();

            if let Some(ref system_prompt) = self.config.system_prompt {
                msgs.push(ChatMessage {
                    role: "system".to_string(),
                    content: serde_json::Value::String(system_prompt.clone()),
                    tool_calls: None,
                });
            }

            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: vision_content,
                tool_calls: None,
            });

            (self.config.vision_model.clone(), msgs)
        } else {
            // Use text model with conversation history
            let msgs = self.build_messages(sender, user_text).await;
            (self.config.model.clone(), msgs)
        };

        // Create the chat completion request (streaming is required by the server)
        let request = ChatCompletionRequest {
            model,
            messages,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens.map(|t| t as i32),
            stream: Some(true),
            stream_options: None,
            tools: None,
            tool_choice: None,
        };

        // Call the OpenSecret API with streaming
        let mut stream = self.client.create_chat_completion_stream(request).await
            .map_err(|e| {
                warn!("OpenSecret API error: {}", e);
                BrainError::Network(format!("OpenSecret API error: {}", e))
            })?;

        // Collect the streamed response
        let mut response_text = String::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    if !chunk.choices.is_empty() {
                        if let Some(serde_json::Value::String(s)) = &chunk.choices[0].delta.content {
                            response_text.push_str(s);
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream error: {}", e);
                    // Continue processing - partial response is better than none
                }
            }
        }

        if response_text.is_empty() {
            warn!("No response content from OpenSecret API");
            response_text = "I'm sorry, I couldn't generate a response.".to_string();
        }

        info!("Generated response for {}: {} chars", sender, response_text.len());

        // Add to conversation history (for text messages only)
        if !has_images {
            self.history.add_exchange(sender, user_text, &response_text).await;
        }

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
