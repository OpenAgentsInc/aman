//! Tool definitions for MapleBrain.
//!
//! This module provides OpenSecret-specific tool definitions that MapleBrain
//! can use. The core ToolExecutor trait is in brain-core.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Re-export from brain-core for convenience
pub use brain_core::{ToolExecutor, ToolRequest, ToolResult};

/// Built-in tool definitions that MapleBrain can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool type (always "function" for function tools).
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function specification.
    pub function: FunctionDefinition,
}

/// Function definition for a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// Name of the function.
    pub name: String,
    /// Description of what the function does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the function parameters.
    pub parameters: Value,
}

impl ToolDefinition {
    /// Create the realtime_search tool definition.
    ///
    /// This tool allows MapleBrain to request real-time information
    /// by crafting a privacy-safe search query.
    pub fn realtime_search() -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "realtime_search".to_string(),
                description: Some(
                    "Search for real-time information from the web and social media. \
                     Use this ONCE when the user asks about current events, news, trending topics, \
                     or anything that requires up-to-date information. \
                     IMPORTANT: Only call this tool once per user message, then synthesize a response \
                     from the results. Do not call it multiple times. \
                     Formulate a search query that captures what information is needed \
                     WITHOUT including any personal details, names, or private context from the conversation. \
                     The query should be generic enough to protect user privacy."
                        .to_string(),
                ),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "A privacy-safe search query. Do NOT include personal names, \
                                           locations, or any identifying information from the user's message. \
                                           Extract only the factual question or topic being asked about."
                        },
                        "search_type": {
                            "type": "string",
                            "enum": ["web", "social", "both"],
                            "description": "Type of search: 'web' for general web search, \
                                           'social' for social media/X, 'both' for comprehensive search."
                        }
                    },
                    "required": ["query"]
                }),
            },
        }
    }

    /// Convert to the OpenSecret SDK Tool type.
    pub fn to_opensecret_tool(&self) -> opensecret::types::Tool {
        opensecret::types::Tool {
            tool_type: self.tool_type.clone(),
            function: opensecret::types::Function {
                name: self.function.name.clone(),
                description: self.function.description.clone(),
                parameters: self.function.parameters.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realtime_search_definition() {
        let tool = ToolDefinition::realtime_search();
        assert_eq!(tool.tool_type, "function");
        assert_eq!(tool.function.name, "realtime_search");
        assert!(tool.function.description.is_some());

        // Verify it serializes correctly
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("realtime_search"));
        assert!(json.contains("privacy"));
    }
}
