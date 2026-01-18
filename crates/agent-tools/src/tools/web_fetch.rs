//! WebFetch tool for fetching and processing web content.

use async_trait::async_trait;
use brain_core::InboundMessage;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Maximum content length to fetch (500KB).
const MAX_CONTENT_LENGTH: usize = 500 * 1024;

/// Maximum content length for summarization (10KB).
const MAX_SUMMARIZE_LENGTH: usize = 10 * 1024;

/// WebFetch tool for fetching URL content and optionally summarizing it.
///
/// Fetches web pages, converts HTML to plain text, and can optionally
/// use a brain to summarize the content.
///
/// # Parameters
///
/// - `url` (required): The URL to fetch.
/// - `summarize` (optional, default: false): If true and a brain is available,
///   summarize the fetched content.
/// - `prompt` (optional): Custom summarization prompt. Only used if summarize is true.
///
/// # Examples
///
/// ```json
/// {"url": "https://example.com"}
/// {"url": "https://news.site/article", "summarize": true}
/// {"url": "https://docs.site/page", "summarize": true, "prompt": "Extract the key points"}
/// ```
pub struct WebFetch {
    client: reqwest::Client,
}

impl WebFetch {
    /// Create a new WebFetch tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (compatible; AmanBot/1.0)")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch content from a URL.
    async fn fetch_url(&self, url: &str) -> Result<String, ToolError> {
        debug!("Fetching URL: {}", url);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "HTTP {} for URL: {}",
                response.status(),
                url
            )));
        }

        // Check content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("text/html");

        let is_html = content_type.contains("text/html");

        // Check content length
        let content_length = response.content_length().unwrap_or(0) as usize;
        if content_length > MAX_CONTENT_LENGTH {
            return Err(ToolError::ExecutionFailed(format!(
                "Content too large: {} bytes (max: {})",
                content_length, MAX_CONTENT_LENGTH
            )));
        }

        let body = response.text().await?;

        // Truncate if necessary
        let body = if body.len() > MAX_CONTENT_LENGTH {
            body[..MAX_CONTENT_LENGTH].to_string()
        } else {
            body
        };

        // Convert HTML to text if needed
        let text = if is_html {
            html2text::from_read(body.as_bytes(), 80)
                .map_err(|e| ToolError::ExecutionFailed(format!("HTML parsing error: {}", e)))?
        } else {
            body
        };

        Ok(text)
    }

    /// Summarize content using a brain.
    async fn summarize_with_brain(
        &self,
        content: &str,
        prompt: Option<&str>,
        args: &ToolArgs,
    ) -> Result<String, ToolError> {
        let brain = args
            .brain
            .as_ref()
            .ok_or_else(|| ToolError::BrainError("No brain available for summarization".to_string()))?;

        // Truncate content for summarization
        let truncated = if content.len() > MAX_SUMMARIZE_LENGTH {
            format!(
                "{}...\n\n[Content truncated, showing first {} characters]",
                &content[..MAX_SUMMARIZE_LENGTH],
                MAX_SUMMARIZE_LENGTH
            )
        } else {
            content.to_string()
        };

        let summarize_prompt = prompt.unwrap_or("Please summarize the following content concisely:");
        let message_text = format!("{}\n\n---\n\n{}", summarize_prompt, truncated);

        // Create an inbound message for the brain
        let message = InboundMessage::direct("tool:web_fetch", &message_text, 0);

        match brain.process(message).await {
            Ok(response) => Ok(response.text),
            Err(e) => Err(ToolError::BrainError(format!("Summarization failed: {}", e))),
        }
    }
}

impl Default for WebFetch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetches content from a URL and converts HTML to plain text. \
         Can optionally summarize the content using AI."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let url = args.get_string("url")?;
        let summarize = args.get_bool_or("summarize", false);
        let prompt = args.get_string_opt("prompt");

        debug!("WebFetch: url={}, summarize={}", url, summarize);

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::InvalidParameter {
                name: "url".to_string(),
                reason: "URL must start with http:// or https://".to_string(),
            });
        }

        // Fetch the content
        let content = match self.fetch_url(&url).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to fetch URL {}: {}", url, e);
                return Err(e);
            }
        };

        // Summarize if requested and brain is available
        if summarize {
            if args.brain.is_some() {
                match self
                    .summarize_with_brain(&content, prompt.as_deref(), &args)
                    .await
                {
                    Ok(summary) => {
                        return Ok(ToolOutput::success(format!(
                            "Summary of {}:\n\n{}",
                            url, summary
                        )));
                    }
                    Err(e) => {
                        warn!("Summarization failed, returning raw content: {}", e);
                        // Fall through to return raw content
                    }
                }
            } else {
                debug!("Summarization requested but no brain available");
            }
        }

        // Return raw content
        let preview_len = content.len().min(2000);
        let preview = if content.len() > 2000 {
            format!(
                "{}...\n\n[Showing first 2000 of {} characters]",
                &content[..preview_len],
                content.len()
            )
        } else {
            content
        };

        Ok(ToolOutput::success(format!("Content from {}:\n\n{}", url, preview)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(url: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("url".to_string(), Value::String(url.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_missing_url() {
        let fetch = WebFetch::new();
        let args = ToolArgs::new(HashMap::new());

        let result = fetch.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    #[tokio::test]
    async fn test_invalid_url_scheme() {
        let fetch = WebFetch::new();
        let args = make_args("ftp://example.com");

        let result = fetch.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    // Integration tests
    #[tokio::test]
    #[ignore]
    async fn test_fetch_example_com() {
        let fetch = WebFetch::new();
        let result = fetch.execute(make_args("https://example.com")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Example Domain"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_nonexistent() {
        let fetch = WebFetch::new();
        let result = fetch
            .execute(make_args("https://this-domain-does-not-exist-12345.com"))
            .await;
        assert!(matches!(result, Err(ToolError::HttpError(_))));
    }
}
