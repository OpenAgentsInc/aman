//! Tool execution support for Brain implementations.
//!
//! This module provides traits and types for executing tools that brains
//! can call during message processing. The primary use case is calling external
//! services (like real-time search) while preserving user privacy by
//! only sending sanitized queries crafted by the model.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Optional metadata about the tool call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolRequestMeta {
    /// Sender identifier (phone or user ID), if available.
    pub sender: Option<String>,
    /// Group identifier for group chats, if available.
    pub group_id: Option<String>,
    /// Whether the message was in a group context.
    pub is_group: Option<bool>,
}

/// Result of a tool execution.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// The tool call ID this result corresponds to.
    pub tool_call_id: String,
    /// The result content (will be sent back to the model).
    pub content: String,
    /// Whether the tool execution succeeded.
    pub success: bool,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            success: true,
        }
    }

    /// Create a failed tool result.
    pub fn error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: format!("Error: {}", error.into()),
            success: false,
        }
    }
}

/// A request to execute a tool.
#[derive(Debug, Clone)]
pub struct ToolRequest {
    /// Unique ID for this tool call.
    pub id: String,
    /// Name of the tool to execute.
    pub name: String,
    /// Arguments as a JSON object.
    pub arguments: HashMap<String, Value>,
    /// Optional metadata about the tool call.
    pub metadata: Option<ToolRequestMeta>,
}

impl ToolRequest {
    /// Parse arguments from a JSON string.
    pub fn from_call(
        id: String,
        name: String,
        arguments_json: &str,
    ) -> Result<Self, serde_json::Error> {
        let arguments: HashMap<String, Value> = serde_json::from_str(arguments_json)?;
        Ok(Self {
            id,
            name,
            arguments,
            metadata: None,
        })
    }

    /// Parse arguments from a JSON string and include metadata.
    pub fn from_call_with_metadata(
        id: String,
        name: String,
        arguments_json: &str,
        metadata: ToolRequestMeta,
    ) -> Result<Self, serde_json::Error> {
        let mut request = Self::from_call(id, name, arguments_json)?;
        request.metadata = Some(metadata);
        Ok(request)
    }

    /// Attach metadata to an existing tool request.
    pub fn with_metadata(mut self, metadata: ToolRequestMeta) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Get a string argument by name.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.arguments.get(key).and_then(|v| v.as_str())
    }

    /// Get a required string argument, or return an error message.
    pub fn require_string(&self, key: &str) -> Result<&str, String> {
        self.get_string(key)
            .ok_or_else(|| format!("Missing required argument: {}", key))
    }
}

/// Trait for executing tools called by a Brain.
///
/// Implement this trait to provide external capabilities to a brain.
/// The primary use case is real-time data search, but this can
/// be extended to other tools.
///
/// # Privacy Model
///
/// The tool executor only receives the sanitized query that the brain
/// crafted - never the user's original message. This preserves user privacy
/// when calling external services.
///
/// # Example
///
/// ```ignore
/// use brain_core::{ToolExecutor, ToolRequest, ToolResult};
///
/// struct MySearchExecutor;
///
/// #[async_trait]
/// impl ToolExecutor for MySearchExecutor {
///     async fn execute(&self, request: ToolRequest) -> ToolResult {
///         match request.name.as_str() {
///             "realtime_search" => {
///                 let query = match request.require_string("query") {
///                     Ok(q) => q,
///                     Err(e) => return ToolResult::error(&request.id, e),
///                 };
///                 // Call your search service...
///                 ToolResult::success(&request.id, "search results here")
///             }
///             _ => ToolResult::error(&request.id, "Unknown tool"),
///         }
///     }
///
///     fn supported_tools(&self) -> Vec<&str> {
///         vec!["realtime_search"]
///     }
/// }
/// ```
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool and return the result.
    async fn execute(&self, request: ToolRequest) -> ToolResult;

    /// List the tools this executor supports.
    /// Used to validate tool calls and for documentation.
    fn supported_tools(&self) -> Vec<&str>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("call-123", "Some data");
        assert!(result.success);
        assert_eq!(result.tool_call_id, "call-123");
        assert_eq!(result.content, "Some data");
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("call-456", "Something went wrong");
        assert!(!result.success);
        assert_eq!(result.content, "Error: Something went wrong");
    }

    #[test]
    fn test_tool_request_parsing() {
        let request = ToolRequest::from_call(
            "id-1".to_string(),
            "realtime_search".to_string(),
            r#"{"query": "latest news", "search_type": "web"}"#,
        )
        .unwrap();

        assert_eq!(request.name, "realtime_search");
        assert_eq!(request.get_string("query"), Some("latest news"));
        assert_eq!(request.get_string("search_type"), Some("web"));
        assert!(request.metadata.is_none());
    }

    #[test]
    fn test_require_string_missing() {
        let request = ToolRequest::from_call(
            "id-1".to_string(),
            "test".to_string(),
            r#"{"foo": "bar"}"#,
        )
        .unwrap();

        assert!(request.require_string("missing").is_err());
    }
}
