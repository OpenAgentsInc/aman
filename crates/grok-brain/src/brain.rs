//! GrokBrain implementation using xAI API.

use brain_core::{
    async_trait, hash_prompt, Brain, BrainError, ConversationHistory, InboundMessage,
    OutboundMessage,
};
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::api_types::{
    ApiError, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, SearchParameters,
};
use crate::config::GrokBrainConfig;

/// A brain implementation that uses xAI's Grok API for AI processing.
///
/// GrokBrain maintains per-sender conversation history and communicates
/// with the xAI API for AI interactions. Optionally supports real-time
/// search via the Live Search API.
pub struct GrokBrain {
    client: Client,
    config: GrokBrainConfig,
    history: ConversationHistory,
    system_prompt_hash: Option<String>,
}

impl GrokBrain {
    /// Create a new GrokBrain with the given configuration.
    pub fn new(config: GrokBrainConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .build()
            .map_err(|e| BrainError::Configuration(format!("Failed to create HTTP client: {}", e)))?;

        let history = ConversationHistory::new(config.max_history_turns);
        let system_prompt_hash = config
            .system_prompt
            .as_ref()
            .map(|prompt| hash_prompt(prompt));

        if let Some(ref hash) = system_prompt_hash {
            info!("GrokBrain system prompt fingerprint: {}", hash);
        }

        info!(
            "GrokBrain initialized with model: {}, x_search: {}, web_search: {}",
            config.model, config.enable_x_search, config.enable_web_search
        );

        Ok(Self {
            client,
            config,
            history,
            system_prompt_hash,
        })
    }

    /// Create a GrokBrain from environment variables.
    ///
    /// See [`GrokBrainConfig::from_env`] for required environment variables.
    pub async fn from_env() -> Result<Self, BrainError> {
        let config = GrokBrainConfig::from_env()?;
        Self::new(config)
    }

    /// Get the configuration.
    pub fn config(&self) -> &GrokBrainConfig {
        &self.config
    }

    /// Get the system prompt fingerprint, if configured.
    pub fn system_prompt_hash(&self) -> Option<&str> {
        self.system_prompt_hash.as_deref()
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
            messages.push(ChatMessage::system(system_prompt.clone()));
        }

        // Add conversation history
        let history = self.history.get(sender).await;
        for msg in history {
            messages.push(ChatMessage {
                role: msg.role,
                content: msg.content,
            });
        }

        // Add current user message
        messages.push(ChatMessage::user(user_text));

        messages
    }

    /// Build search parameters based on configuration.
    fn build_search_parameters(&self) -> Option<SearchParameters> {
        if !self.config.enable_x_search && !self.config.enable_web_search {
            return None;
        }

        if self.config.enable_x_search && self.config.enable_web_search {
            Some(SearchParameters::all_sources())
        } else if self.config.enable_x_search {
            Some(SearchParameters::x_only())
        } else {
            Some(SearchParameters::web_only())
        }
    }

    /// Make a chat completion request to the xAI API.
    async fn chat_completion(
        &self,
        messages: Vec<ChatMessage>,
        model_override: Option<&str>,
    ) -> Result<ChatCompletionResponse, BrainError> {
        let url = format!("{}/v1/chat/completions", self.config.api_url);
        let model = model_override.unwrap_or(&self.config.model);

        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            search_parameters: self.build_search_parameters(),
        };

        debug!("Sending request to xAI API: {:?}", request);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::Network(format!("Failed to send request: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse as API error
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                return Err(BrainError::ProcessingFailed(format!(
                    "API error ({}): {}",
                    status.as_u16(),
                    api_error.error.message
                )));
            }

            return Err(BrainError::ProcessingFailed(format!(
                "API error ({}): {}",
                status.as_u16(),
                error_text
            )));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| BrainError::ProcessingFailed(format!("Failed to parse response: {}", e)))?;

        debug!("Received response from xAI API: {:?}", completion);

        Ok(completion)
    }
}

#[async_trait]
impl Brain for GrokBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        let sender = &message.sender;
        let user_text = &message.text;
        let model_override = message
            .routing
            .as_ref()
            .and_then(|routing| routing.model_override.as_deref());
        let selected_model = select_model_for_message(&self.config, &message);

        debug!("Processing message from {}: {}", sender, user_text);
        if let Some(override_model) = model_override {
            info!("Using model override: {}", override_model);
        }

        // Build messages with history
        let messages = self.build_messages(sender, user_text).await;

        // Make API request
        let completion = self
            .chat_completion(messages, Some(selected_model.as_str()))
            .await?;

        // Extract response text
        let response_text = completion
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                warn!("No content in response, using default");
                "I apologize, but I couldn't generate a response.".to_string()
            });

        // Add to conversation history
        self.history
            .add_exchange(sender, user_text, &response_text)
            .await;

        // Log usage if available
        if let Some(usage) = completion.usage {
            debug!(
                "Token usage - prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(OutboundMessage::reply_to(&message, response_text))
    }

    fn name(&self) -> &str {
        "GrokBrain"
    }
}

fn select_model_for_message(config: &GrokBrainConfig, message: &InboundMessage) -> String {
    if let Some(override_model) = message
        .routing
        .as_ref()
        .and_then(|routing| routing.model_override.as_deref())
    {
        return override_model.to_string();
    }

    config.model.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_parameters_none() {
        let config = GrokBrainConfig::builder()
            .api_key("test-key")
            .enable_x_search(false)
            .enable_web_search(false)
            .build();

        let brain = GrokBrain::new(config).unwrap();
        assert!(brain.build_search_parameters().is_none());
    }

    #[test]
    fn test_build_search_parameters_x_only() {
        let config = GrokBrainConfig::builder()
            .api_key("test-key")
            .enable_x_search(true)
            .enable_web_search(false)
            .build();

        let brain = GrokBrain::new(config).unwrap();
        let params = brain.build_search_parameters().unwrap();
        let sources = params.sources.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_type, "x");
    }

    #[test]
    fn test_build_search_parameters_both() {
        let config = GrokBrainConfig::builder()
            .api_key("test-key")
            .enable_x_search(true)
            .enable_web_search(true)
            .build();

        let brain = GrokBrain::new(config).unwrap();
        let params = brain.build_search_parameters().unwrap();
        let sources = params.sources.unwrap();
        assert_eq!(sources.len(), 3); // web, news, x
    }

    #[test]
    fn test_brain_name() {
        let config = GrokBrainConfig::builder().api_key("test-key").build();

        let brain = GrokBrain::new(config).unwrap();
        assert_eq!(brain.name(), "GrokBrain");
    }

    #[test]
    fn test_select_model_for_message_default() {
        let config = GrokBrainConfig::builder()
            .api_key("test-key")
            .model("grok-default")
            .build();
        let message = InboundMessage::direct("+123", "hello", 0);

        let selected = select_model_for_message(&config, &message);
        assert_eq!(selected, "grok-default");
    }

    #[test]
    fn test_select_model_for_message_override() {
        let config = GrokBrainConfig::builder()
            .api_key("test-key")
            .model("grok-default")
            .build();
        let mut message = InboundMessage::direct("+123", "hello", 0);
        message.routing = Some(brain_core::RoutingInfo {
            model_override: Some("grok-override".to_string()),
            ..Default::default()
        });

        let selected = select_model_for_message(&config, &message);
        assert_eq!(selected, "grok-override");
    }
}
