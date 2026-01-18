//! xAI API request and response types.

// These types mirror the xAI API structure. Some fields are not currently used
// but are kept for completeness and future use.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: "system", "user", or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
}

impl ChatMessage {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// Search source type for Live Search API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSource {
    /// Source type: "web", "news", "x", or "rss"
    #[serde(rename = "type")]
    pub source_type: String,
}

impl SearchSource {
    /// Create a web search source.
    pub fn web() -> Self {
        Self {
            source_type: "web".to_string(),
        }
    }

    /// Create a news search source.
    pub fn news() -> Self {
        Self {
            source_type: "news".to_string(),
        }
    }

    /// Create an X (Twitter) search source.
    pub fn x() -> Self {
        Self {
            source_type: "x".to_string(),
        }
    }

    /// Create an RSS search source.
    pub fn rss() -> Self {
        Self {
            source_type: "rss".to_string(),
        }
    }
}

/// Search parameters for Live Search API.
///
/// Note: The Live Search API is being deprecated by January 12, 2026 in favor
/// of the Agentic Tool Calling API. However, it still works for now.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchParameters {
    /// Search mode: "off", "auto", or "on"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Whether to return citations in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_citations: Option<bool>,

    /// Maximum number of search results to consider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_search_results: Option<u32>,

    /// Start date for search results (ISO 8601 format: YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_date: Option<String>,

    /// End date for search results (ISO 8601 format: YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<String>,

    /// Sources to search. Defaults to ["web", "news", "x"] if not specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<SearchSource>>,
}

impl SearchParameters {
    /// Create search parameters with search enabled.
    pub fn enabled() -> Self {
        Self {
            mode: Some("on".to_string()),
            return_citations: Some(true),
            ..Default::default()
        }
    }

    /// Create search parameters for web search only.
    pub fn web_only() -> Self {
        Self {
            mode: Some("on".to_string()),
            return_citations: Some(true),
            sources: Some(vec![SearchSource::web(), SearchSource::news()]),
            ..Default::default()
        }
    }

    /// Create search parameters for X (social) search only.
    pub fn x_only() -> Self {
        Self {
            mode: Some("on".to_string()),
            return_citations: Some(true),
            sources: Some(vec![SearchSource::x()]),
            ..Default::default()
        }
    }

    /// Create search parameters for all sources.
    pub fn all_sources() -> Self {
        Self {
            mode: Some("on".to_string()),
            return_citations: Some(true),
            sources: Some(vec![
                SearchSource::web(),
                SearchSource::news(),
                SearchSource::x(),
            ]),
            ..Default::default()
        }
    }
}

/// Chat completion request to xAI API.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    /// Model to use
    pub model: String,
    /// Messages in the conversation
    pub messages: Vec<ChatMessage>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature for generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Search parameters for Live Search (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_parameters: Option<SearchParameters>,
}

/// Chat completion response from xAI API.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    /// Response ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Unix timestamp
    pub created: u64,
    /// Model used
    pub model: String,
    /// Response choices
    pub choices: Vec<Choice>,
    /// Token usage
    pub usage: Option<Usage>,
}

/// A response choice.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    /// Choice index
    pub index: u32,
    /// The message
    pub message: ResponseMessage,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// Response message (may have tool info).
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    /// Role
    pub role: String,
    /// Content (may be null if tool calls)
    pub content: Option<String>,
}

/// Token usage information.
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Completion tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// API error response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    /// Error details
    pub error: ApiErrorDetails,
}

/// API error details.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorDetails {
    /// Error message
    pub message: String,
    /// Error type
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    /// Error code
    pub code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_system() {
        let msg = ChatMessage::system("You are helpful");
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "You are helpful");
    }

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("Hello!");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello!");
    }

    #[test]
    fn test_chat_message_assistant() {
        let msg = ChatMessage::assistant("Hi there!");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Hi there!");
    }

    #[test]
    fn test_search_source_web() {
        let source = SearchSource::web();
        assert_eq!(source.source_type, "web");
    }

    #[test]
    fn test_search_source_x() {
        let source = SearchSource::x();
        assert_eq!(source.source_type, "x");
    }

    #[test]
    fn test_search_parameters_enabled() {
        let params = SearchParameters::enabled();
        assert_eq!(params.mode, Some("on".to_string()));
        assert_eq!(params.return_citations, Some(true));
    }

    #[test]
    fn test_search_parameters_web_only() {
        let params = SearchParameters::web_only();
        assert_eq!(params.mode, Some("on".to_string()));
        let sources = params.sources.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].source_type, "web");
        assert_eq!(sources[1].source_type, "news");
    }

    #[test]
    fn test_search_parameters_x_only() {
        let params = SearchParameters::x_only();
        let sources = params.sources.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_type, "x");
    }

    #[test]
    fn test_chat_message_serialize() {
        let msg = ChatMessage::user("Test message");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Test message\""));
    }

    #[test]
    fn test_search_parameters_serialize() {
        let params = SearchParameters::enabled();
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"mode\":\"on\""));
        assert!(json.contains("\"return_citations\":true"));
    }

    #[test]
    fn test_chat_completion_request_serialize() {
        let request = ChatCompletionRequest {
            model: "grok-4-1-fast".to_string(),
            messages: vec![
                ChatMessage::system("Be helpful"),
                ChatMessage::user("Hello"),
            ],
            max_tokens: Some(1024),
            temperature: Some(0.7),
            search_parameters: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"grok-4-1-fast\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        // search_parameters should be omitted when None
        assert!(!json.contains("\"search_parameters\""));
    }

    #[test]
    fn test_chat_completion_request_with_search() {
        let request = ChatCompletionRequest {
            model: "grok-4-1-fast".to_string(),
            messages: vec![ChatMessage::user("Search for news")],
            max_tokens: None,
            temperature: None,
            search_parameters: Some(SearchParameters::all_sources()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"search_parameters\""));
        assert!(json.contains("\"mode\":\"on\""));
        assert!(json.contains("\"web\""));
        assert!(json.contains("\"news\""));
    }

    #[test]
    fn test_chat_completion_response_deserialize() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "grok-4-1-fast",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1234567890);
        assert_eq!(response.model, "grok-4-1-fast");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].index, 0);
        assert_eq!(response.choices[0].message.role, "assistant");
        assert_eq!(
            response.choices[0].message.content,
            Some("Hello! How can I help you?".to_string())
        );
        assert_eq!(response.choices[0].finish_reason, Some("stop".to_string()));

        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 8);
        assert_eq!(usage.total_tokens, 18);
    }

    #[test]
    fn test_api_error_deserialize() {
        let json = r#"{
            "error": {
                "message": "Invalid API key",
                "type": "authentication_error",
                "code": "invalid_api_key"
            }
        }"#;

        let error: ApiError = serde_json::from_str(json).unwrap();

        assert_eq!(error.error.message, "Invalid API key");
        assert_eq!(
            error.error.error_type,
            Some("authentication_error".to_string())
        );
        assert_eq!(error.error.code, Some("invalid_api_key".to_string()));
    }

    #[test]
    fn test_response_with_null_content() {
        let json = r#"{
            "id": "chatcmpl-456",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "grok-4-1-fast",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "tool_calls"
            }]
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();

        assert!(response.choices[0].message.content.is_none());
        assert_eq!(
            response.choices[0].finish_reason,
            Some("tool_calls".to_string())
        );
        assert!(response.usage.is_none());
    }
}
