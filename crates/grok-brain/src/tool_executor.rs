//! ToolExecutor implementation using Grok for real-time search.
//!
//! This module provides a ToolExecutor that uses the xAI Grok API
//! to perform real-time searches. It's designed to be used with
//! MapleBrain to provide privacy-preserving real-time data access.

use brain_core::{async_trait, BrainError, ToolExecutor, ToolRequest, ToolResult};
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::api_types::{ApiError, ChatCompletionRequest, ChatMessage, SearchParameters};
use crate::config::GrokBrainConfig;

/// A ToolExecutor that uses Grok for real-time search.
///
/// This executor handles the `realtime_search` tool by forwarding
/// sanitized queries to the xAI Grok API with Live Search enabled.
///
/// # Privacy Model
///
/// This executor only receives sanitized queries crafted by MapleBrain -
/// never the user's original message. This preserves user privacy when
/// accessing real-time data.
///
/// # Example
///
/// ```rust,ignore
/// use grok_brain::GrokToolExecutor;
/// use maple_brain::{MapleBrain, MapleBrainConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let grok_executor = GrokToolExecutor::from_env()?;
///     let maple_config = MapleBrainConfig::from_env()?;
///     let brain = MapleBrain::with_tools(maple_config, grok_executor).await?;
///     Ok(())
/// }
/// ```
pub struct GrokToolExecutor {
    client: Client,
    config: GrokBrainConfig,
}

/// Default HTTP timeout for API requests (60 seconds).
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 60;

impl GrokToolExecutor {
    /// Create a new GrokToolExecutor with the given configuration.
    pub fn new(config: GrokBrainConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .build()
            .map_err(|e| {
                BrainError::Configuration(format!("Failed to create HTTP client: {}", e))
            })?;

        info!(
            "GrokToolExecutor initialized with model: {}, x_search: {}, web_search: {}",
            config.model, config.enable_x_search, config.enable_web_search
        );

        Ok(Self { client, config })
    }

    /// Create a GrokToolExecutor from environment variables.
    ///
    /// This uses the same environment variables as GrokBrain.
    /// By default, it enables both X Search and Web Search for comprehensive results.
    pub fn from_env() -> Result<Self, BrainError> {
        let mut config = GrokBrainConfig::from_env()?;

        // For tool execution, we want search enabled by default
        // The config can still override via env vars
        if !config.enable_x_search && !config.enable_web_search {
            info!("Enabling search tools by default for GrokToolExecutor");
            config.enable_x_search = true;
            config.enable_web_search = true;
        }

        Self::new(config)
    }

    /// Create a GrokToolExecutor with specific search settings.
    pub fn with_search(
        api_key: impl Into<String>,
        x_search: bool,
        web_search: bool,
    ) -> Result<Self, BrainError> {
        let config = GrokBrainConfig::builder()
            .api_key(api_key)
            .enable_x_search(x_search)
            .enable_web_search(web_search)
            .build();

        Self::new(config)
    }

    /// Build search parameters based on search type requested.
    fn build_search_parameters(&self, search_type: Option<&str>) -> SearchParameters {
        match search_type {
            Some("social") => {
                if self.config.enable_x_search {
                    SearchParameters::x_only()
                } else {
                    SearchParameters::enabled()
                }
            }
            Some("web") => {
                if self.config.enable_web_search {
                    SearchParameters::web_only()
                } else {
                    SearchParameters::enabled()
                }
            }
            Some("both") | None => {
                if self.config.enable_x_search && self.config.enable_web_search {
                    SearchParameters::all_sources()
                } else if self.config.enable_x_search {
                    SearchParameters::x_only()
                } else if self.config.enable_web_search {
                    SearchParameters::web_only()
                } else {
                    SearchParameters::enabled()
                }
            }
            _ => SearchParameters::all_sources(),
        }
    }

    /// Execute a search query using Grok.
    async fn search(&self, query: &str, search_type: Option<&str>) -> Result<String, BrainError> {
        let url = format!("{}/v1/chat/completions", self.config.api_url);

        let search_parameters = self.build_search_parameters(search_type);

        // Create a system message that encourages thorough search
        let system_message = ChatMessage::system(
            "You are a search assistant. Your job is to search for real-time information \
             and provide comprehensive, factual results. Use the search capability \
             to find the most current and relevant information. Present the results clearly \
             and cite sources when available.",
        );

        let user_message = ChatMessage::user(query);

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![system_message, user_message],
            max_tokens: self.config.max_tokens,
            temperature: Some(0.3), // Lower temperature for factual search
            search_parameters: Some(search_parameters),
        };

        debug!("Sending search request to xAI API: {:?}", request);

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

            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                return Err(BrainError::ProcessingFailed(format!(
                    "Grok API error ({}): {}",
                    status.as_u16(),
                    api_error.error.message
                )));
            }

            return Err(BrainError::ProcessingFailed(format!(
                "Grok API error ({}): {}",
                status.as_u16(),
                error_text
            )));
        }

        let completion: crate::api_types::ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| BrainError::ProcessingFailed(format!("Failed to parse response: {}", e)))?;

        // Extract response text
        let result = completion
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "No search results found.".to_string());

        // Log usage if available
        if let Some(usage) = completion.usage {
            debug!(
                "Search token usage - prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(result)
    }
}

#[async_trait]
impl ToolExecutor for GrokToolExecutor {
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        match request.name.as_str() {
            "realtime_search" => {
                let query = match request.require_string("query") {
                    Ok(q) => q,
                    Err(e) => return ToolResult::error(&request.id, e),
                };

                let search_type = request.get_string("search_type");

                info!(
                    "Executing realtime_search: query='{}', type={:?}",
                    query, search_type
                );

                match self.search(query, search_type).await {
                    Ok(result) => {
                        info!("Search completed successfully ({} chars)", result.len());
                        ToolResult::success(&request.id, result)
                    }
                    Err(e) => {
                        warn!("Search failed: {}", e);
                        ToolResult::error(&request.id, e.to_string())
                    }
                }
            }
            other => {
                warn!("Unknown tool requested: {}", other);
                ToolResult::error(&request.id, format!("Unknown tool: {}", other))
            }
        }
    }

    fn supported_tools(&self) -> Vec<&str> {
        vec!["realtime_search"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_parameters_social() {
        let config = GrokBrainConfig::builder()
            .api_key("test")
            .enable_x_search(true)
            .enable_web_search(true)
            .build();

        let executor = GrokToolExecutor::new(config).unwrap();
        let params = executor.build_search_parameters(Some("social"));
        let sources = params.sources.unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_type, "x");
    }

    #[test]
    fn test_build_search_parameters_web() {
        let config = GrokBrainConfig::builder()
            .api_key("test")
            .enable_x_search(true)
            .enable_web_search(true)
            .build();

        let executor = GrokToolExecutor::new(config).unwrap();
        let params = executor.build_search_parameters(Some("web"));
        let sources = params.sources.unwrap();

        assert_eq!(sources.len(), 2); // web + news
        assert_eq!(sources[0].source_type, "web");
    }

    #[test]
    fn test_build_search_parameters_both() {
        let config = GrokBrainConfig::builder()
            .api_key("test")
            .enable_x_search(true)
            .enable_web_search(true)
            .build();

        let executor = GrokToolExecutor::new(config).unwrap();
        let params = executor.build_search_parameters(Some("both"));
        let sources = params.sources.unwrap();

        assert_eq!(sources.len(), 3); // web, news, x
    }

    #[test]
    fn test_supported_tools() {
        let config = GrokBrainConfig::builder().api_key("test").build();
        let executor = GrokToolExecutor::new(config).unwrap();

        assert_eq!(executor.supported_tools(), vec!["realtime_search"]);
    }
}
