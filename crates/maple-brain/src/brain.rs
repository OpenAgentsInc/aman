//! MapleBrain implementation using OpenSecret SDK.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use brain_core::{async_trait, Brain, BrainError, InboundAttachment, InboundMessage, OutboundMessage};
use futures::StreamExt;
use opensecret::{
    types::{ChatCompletionRequest, ChatMessage, ToolCall},
    OpenSecretClient,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::config::MapleBrainConfig;
use crate::history::ConversationHistory;
use crate::tools::ToolDefinition;
use brain_core::{ToolExecutor, ToolRequest};

/// Maximum number of tool call rounds to prevent infinite loops.
/// Set to 2 to limit costs and latency - one search should usually be enough.
const MAX_TOOL_ROUNDS: usize = 2;

/// Status updates that can be sent during message processing.
#[derive(Debug, Clone)]
pub enum StatusUpdate {
    /// Processing has started.
    Processing,
    /// A tool is being executed (e.g., "Searching for current information...").
    ToolExecuting {
        /// Name of the tool being executed.
        tool_name: String,
        /// Human-readable description of what's happening.
        description: String,
    },
    /// Tool execution completed.
    ToolComplete {
        /// Name of the tool that completed.
        tool_name: String,
    },
}

/// Type alias for the async status callback.
pub type StatusCallback = Box<
    dyn Fn(StatusUpdate) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
>;

/// A brain implementation that uses OpenSecret SDK for AI processing.
///
/// MapleBrain maintains per-sender conversation history and communicates
/// with the OpenSecret API for end-to-end encrypted AI interactions.
///
/// # Tool Support
///
/// MapleBrain can optionally be configured with a [`ToolExecutor`] to enable
/// tool calling. When a tool executor is present, the model can request
/// external data (like real-time search results) by calling tools.
///
/// The key privacy feature is that the model crafts sanitized queries for
/// tools - the user's original message never leaves the TEE. Only the
/// model's reformulated query is sent to external services.
pub struct MapleBrain {
    client: OpenSecretClient,
    config: MapleBrainConfig,
    history: ConversationHistory,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
}

impl MapleBrain {
    /// Create a new MapleBrain with the given configuration.
    ///
    /// This performs the attestation handshake with the OpenSecret API,
    /// which establishes a secure encrypted session.
    pub async fn new(config: MapleBrainConfig) -> Result<Self, BrainError> {
        Self::new_internal(config, None).await
    }

    /// Create a new MapleBrain with tool execution support.
    ///
    /// The tool executor will be called when the model requests external
    /// data via tool calls. This enables features like real-time search
    /// while preserving user privacy.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = MapleBrainConfig::from_env()?;
    /// let grok_executor = GrokToolExecutor::from_env().await?;
    /// let brain = MapleBrain::with_tools(config, grok_executor).await?;
    /// ```
    pub async fn with_tools<E: ToolExecutor + 'static>(
        config: MapleBrainConfig,
        executor: E,
    ) -> Result<Self, BrainError> {
        Self::new_internal(config, Some(Arc::new(executor))).await
    }

    /// Create a new MapleBrain with a shared tool executor.
    pub async fn with_shared_tools(
        config: MapleBrainConfig,
        executor: Arc<dyn ToolExecutor>,
    ) -> Result<Self, BrainError> {
        Self::new_internal(config, Some(executor)).await
    }

    async fn new_internal(
        config: MapleBrainConfig,
        tool_executor: Option<Arc<dyn ToolExecutor>>,
    ) -> Result<Self, BrainError> {
        let client =
            OpenSecretClient::new_with_api_key(&config.api_url, config.api_key.clone()).map_err(
                |e| BrainError::Configuration(format!("Failed to create OpenSecret client: {}", e)),
            )?;

        // Perform attestation handshake to establish secure session
        info!("Performing attestation handshake with OpenSecret API...");
        client
            .perform_attestation_handshake()
            .await
            .map_err(|e| BrainError::Network(format!("Attestation handshake failed: {}", e)))?;
        info!("Attestation handshake complete");

        if tool_executor.is_some() {
            info!("MapleBrain initialized with tool execution support");
        }

        let history = ConversationHistory::new(config.max_history_turns);

        Ok(Self {
            client,
            config,
            history,
            tool_executor,
        })
    }

    /// Create a MapleBrain from environment variables.
    ///
    /// See [`MapleBrainConfig::from_env`] for required environment variables.
    pub async fn from_env() -> Result<Self, BrainError> {
        let config = MapleBrainConfig::from_env()?;
        Self::new(config).await
    }

    /// Check if this brain has tool execution support.
    pub fn has_tools(&self) -> bool {
        self.tool_executor.is_some()
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

    /// Process a message with status updates via callback.
    ///
    /// This is like [`Brain::process`] but allows you to receive status updates
    /// during processing, such as when a tool search is being performed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let callback = |status| {
    ///     Box::pin(async move {
    ///         match status {
    ///             StatusUpdate::ToolExecuting { description, .. } => {
    ///                 println!("Status: {}", description);
    ///             }
    ///             _ => {}
    ///         }
    ///     })
    /// };
    /// let response = brain.process_with_status(message, Box::new(callback)).await?;
    /// ```
    pub async fn process_with_status(
        &self,
        message: InboundMessage,
        status_callback: StatusCallback,
    ) -> Result<OutboundMessage, BrainError> {
        self.process_internal(message, Some(status_callback)).await
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
    async fn build_vision_message(
        &self,
        user_text: &str,
        attachments: &[InboundAttachment],
    ) -> Result<serde_json::Value, BrainError> {
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
                match self
                    .load_image_as_base64(file_path, &attachment.content_type)
                    .await
                {
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
    async fn load_image_as_base64(
        &self,
        file_path: &str,
        content_type: &str,
    ) -> Result<String, BrainError> {
        let bytes = fs::read(file_path).await.map_err(|e| {
            BrainError::ProcessingFailed(format!("Failed to read image file: {}", e))
        })?;

        let base64_data = BASE64.encode(&bytes);
        let data_url = format!("data:{};base64,{}", content_type, base64_data);

        Ok(data_url)
    }

    /// Get the tools to include in requests (if executor is present).
    fn get_tools(&self) -> Option<Vec<opensecret::types::Tool>> {
        self.tool_executor.as_ref().map(|_| {
            vec![ToolDefinition::realtime_search().to_opensecret_tool()]
        })
    }

    /// Make a streaming chat completion request and collect the response.
    async fn complete_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<(String, Option<Vec<ToolCall>>), BrainError> {
        let mut stream = self
            .client
            .create_chat_completion_stream(request)
            .await
            .map_err(|e| {
                warn!("OpenSecret API error: {}", e);
                BrainError::Network(format!("OpenSecret API error: {}", e))
            })?;

        let mut response_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut finish_reason: Option<String> = None;

        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    if !chunk.choices.is_empty() {
                        let choice = &chunk.choices[0];

                        // Collect text content
                        if let Some(serde_json::Value::String(s)) = &choice.delta.content {
                            response_text.push_str(s);
                        }

                        // Collect tool calls (they come in chunks)
                        if let Some(calls) = &choice.delta.tool_calls {
                            for call in calls {
                                // Find or create the tool call entry
                                if let Some(index) = call.index {
                                    let idx = index as usize;
                                    while tool_calls.len() <= idx {
                                        tool_calls.push(ToolCall {
                                            id: String::new(),
                                            tool_type: "function".to_string(),
                                            function: opensecret::types::FunctionCall {
                                                name: String::new(),
                                                arguments: String::new(),
                                            },
                                            index: Some(tool_calls.len() as i32),
                                        });
                                    }

                                    // Update the tool call
                                    if !call.id.is_empty() {
                                        tool_calls[idx].id = call.id.clone();
                                    }
                                    if !call.function.name.is_empty() {
                                        tool_calls[idx].function.name = call.function.name.clone();
                                    }
                                    tool_calls[idx]
                                        .function
                                        .arguments
                                        .push_str(&call.function.arguments);
                                }
                            }
                        }

                        // Track finish reason
                        if choice.finish_reason.is_some() {
                            finish_reason = choice.finish_reason.clone();
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream error: {}", e);
                    // Continue processing - partial response is better than none
                }
            }
        }

        // Log what we received for debugging
        debug!(
            "Stream complete - finish_reason: {:?}, text_len: {}, tool_calls: {}",
            finish_reason,
            response_text.len(),
            tool_calls.len()
        );

        // Return tool calls if that was the finish reason
        let has_tool_calls =
            finish_reason.as_deref() == Some("tool_calls") && !tool_calls.is_empty();

        // Also check for "tool_use" which some APIs use
        let has_tool_calls = has_tool_calls
            || (finish_reason.as_deref() == Some("tool_use") && !tool_calls.is_empty());

        if !tool_calls.is_empty() {
            debug!(
                "Tool calls collected: {:?}",
                tool_calls.iter().map(|c| &c.function.name).collect::<Vec<_>>()
            );
        }

        Ok((
            response_text,
            if has_tool_calls {
                Some(tool_calls)
            } else {
                None
            },
        ))
    }
}

impl MapleBrain {
    /// Internal process implementation that supports optional status callbacks.
    async fn process_internal(
        &self,
        message: InboundMessage,
        status_callback: Option<StatusCallback>,
    ) -> Result<OutboundMessage, BrainError> {
        let sender = &message.sender;
        let user_text = &message.text;
        let has_images = message.has_images();

        // Use group_id for group conversations, sender for direct messages
        let history_key = message
            .group_id
            .as_ref()
            .map(|g| format!("group:{}", g))
            .unwrap_or_else(|| sender.clone());

        debug!(
            "Processing message from {}: {} (images: {}, history_key: {})",
            sender, user_text, has_images, history_key
        );

        // Choose model and build messages based on whether we have images
        let (model, mut messages) = if has_images {
            // Use vision model for messages with images
            info!(
                "Using vision model for message with {} image(s)",
                message.attachments.iter().filter(|a| a.is_image()).count()
            );

            let vision_content = self
                .build_vision_message(user_text, &message.attachments)
                .await?;

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

            // No tools for vision requests (too complex with images)
            (self.config.vision_model.clone(), msgs)
        } else {
            // Use text model with conversation history
            let msgs = self.build_messages(&history_key, user_text).await;
            (self.config.model.clone(), msgs)
        };

        // Get tools if we have an executor (not for vision)
        let tools = if has_images { None } else { self.get_tools() };

        // Initial request
        let mut request = ChatCompletionRequest {
            model: model.clone(),
            messages: messages.clone(),
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens.map(|t| t as i32),
            stream: Some(true),
            stream_options: None,
            tools: tools.clone(),
            tool_choice: None,
        };

        let mut response_text = String::new();
        let mut rounds = 0;

        // Tool call loop
        loop {
            rounds += 1;
            if rounds > MAX_TOOL_ROUNDS {
                warn!("Exceeded maximum tool call rounds ({})", MAX_TOOL_ROUNDS);
                break;
            }

            let (text, tool_calls) = self.complete_chat(request.clone()).await?;

            match tool_calls {
                Some(calls) if !calls.is_empty() => {
                    info!("Model requested {} tool call(s)", calls.len());

                    // Add assistant message with tool calls to conversation
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: serde_json::Value::Null,
                        tool_calls: Some(calls.clone()),
                    });

                    // Execute tools with status callback
                    let results = self
                        .execute_tool_calls_with_status(&calls, status_callback.as_ref())
                        .await;
                    messages.extend(results);

                    // Update request for next round
                    request = ChatCompletionRequest {
                        model: model.clone(),
                        messages: messages.clone(),
                        temperature: self.config.temperature,
                        max_tokens: self.config.max_tokens.map(|t| t as i32),
                        stream: Some(true),
                        stream_options: None,
                        tools: tools.clone(),
                        tool_choice: None,
                    };

                    // Continue loop to get model's response with tool results
                }
                _ => {
                    // No tool calls - this is the final response
                    response_text = text;
                    break;
                }
            }
        }

        if response_text.is_empty() {
            warn!("No response content from OpenSecret API");
            response_text = "I'm sorry, I couldn't generate a response.".to_string();
        }

        info!(
            "Generated response for {}: {} chars (tool rounds: {})",
            sender,
            response_text.len(),
            rounds
        );

        // Add to conversation history (for text messages only)
        if !has_images {
            self.history
                .add_exchange(&history_key, user_text, &response_text)
                .await;
        }

        // Return the outbound message
        Ok(OutboundMessage::reply_to(&message, response_text))
    }

    /// Execute tool calls with optional status callback.
    async fn execute_tool_calls_with_status(
        &self,
        tool_calls: &[ToolCall],
        status_callback: Option<&StatusCallback>,
    ) -> Vec<ChatMessage> {
        let executor = match &self.tool_executor {
            Some(e) => e,
            None => return vec![],
        };

        let mut results = Vec::new();

        for call in tool_calls {
            let request = match ToolRequest::from_call(
                call.id.clone(),
                call.function.name.clone(),
                &call.function.arguments,
            ) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to parse tool arguments: {}", e);
                    results.push(ChatMessage {
                        role: "tool".to_string(),
                        content: serde_json::Value::String(format!(
                            "Error: Invalid arguments - {}",
                            e
                        )),
                        tool_calls: None,
                    });
                    continue;
                }
            };

            // Notify via callback that tool execution is starting
            if let Some(callback) = status_callback {
                let description = match request.name.as_str() {
                    "realtime_search" => "Searching for current information...".to_string(),
                    _ => format!("Executing {}...", request.name),
                };
                callback(StatusUpdate::ToolExecuting {
                    tool_name: request.name.clone(),
                    description,
                })
                .await;
            }

            info!(
                "Executing tool '{}' with sanitized query",
                request.name
            );
            debug!("Tool request: {:?}", request);

            let result = executor.execute(request.clone()).await;

            info!(
                "Tool '{}' completed (success: {})",
                call.function.name, result.success
            );

            // Notify via callback that tool execution completed
            if let Some(callback) = status_callback {
                callback(StatusUpdate::ToolComplete {
                    tool_name: request.name.clone(),
                })
                .await;
            }

            // Add tool result as a message
            results.push(ChatMessage {
                role: "tool".to_string(),
                content: serde_json::json!({
                    "tool_call_id": result.tool_call_id,
                    "result": result.content
                }),
                tool_calls: None,
            });
        }

        results
    }
}

#[async_trait]
impl Brain for MapleBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        self.process_internal(message, None).await
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
