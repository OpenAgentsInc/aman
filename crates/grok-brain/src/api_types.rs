//! xAI API request and response types.

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

/// An xAI tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XaiTool {
    /// Tool type (always "xai_tool" for built-in tools)
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Tool specification
    pub xai_tool: XaiToolSpec,
}

/// xAI tool specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XaiToolSpec {
    /// Tool type: "x_search" or "web_search"
    #[serde(rename = "type")]
    pub spec_type: String,
}

impl XaiTool {
    /// Create an X Search tool.
    pub fn x_search() -> Self {
        Self {
            tool_type: "xai_tool".to_string(),
            xai_tool: XaiToolSpec {
                spec_type: "x_search".to_string(),
            },
        }
    }

    /// Create a Web Search tool.
    pub fn web_search() -> Self {
        Self {
            tool_type: "xai_tool".to_string(),
            xai_tool: XaiToolSpec {
                spec_type: "web_search".to_string(),
            },
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
    /// Tools to make available (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<XaiTool>>,
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
