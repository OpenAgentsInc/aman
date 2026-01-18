//! Tool trait definition and types.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use brain_core::Brain;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ToolError;

/// Arguments passed to a tool for execution.
#[derive(Clone)]
pub struct ToolArgs {
    /// Parameters as key-value pairs.
    pub params: HashMap<String, Value>,
    /// Optional brain for AI-powered processing (e.g., summarization).
    pub brain: Option<Arc<dyn Brain>>,
}

impl ToolArgs {
    /// Create new tool arguments with the given parameters.
    pub fn new(params: HashMap<String, Value>) -> Self {
        Self { params, brain: None }
    }

    /// Create tool arguments with a brain for AI processing.
    pub fn with_brain(params: HashMap<String, Value>, brain: Arc<dyn Brain>) -> Self {
        Self {
            params,
            brain: Some(brain),
        }
    }

    /// Get a string parameter, returning an error if missing or not a string.
    pub fn get_string(&self, key: &str) -> Result<String, ToolError> {
        self.params
            .get(key)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))?
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::InvalidParameter {
                name: key.to_string(),
                reason: "expected string".to_string(),
            })
    }

    /// Get an optional string parameter.
    pub fn get_string_opt(&self, key: &str) -> Option<String> {
        self.params.get(key)?.as_str().map(|s| s.to_string())
    }

    /// Get a boolean parameter, returning an error if missing or not a boolean.
    pub fn get_bool(&self, key: &str) -> Result<bool, ToolError> {
        self.params
            .get(key)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))?
            .as_bool()
            .ok_or_else(|| ToolError::InvalidParameter {
                name: key.to_string(),
                reason: "expected boolean".to_string(),
            })
    }

    /// Get an optional boolean parameter with a default value.
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.params
            .get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    /// Get an f64 parameter, returning an error if missing or not a number.
    pub fn get_f64(&self, key: &str) -> Result<f64, ToolError> {
        self.params
            .get(key)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))?
            .as_f64()
            .ok_or_else(|| ToolError::InvalidParameter {
                name: key.to_string(),
                reason: "expected number".to_string(),
            })
    }

    /// Alias for get_f64 for convenience.
    pub fn get_number(&self, key: &str) -> Result<f64, ToolError> {
        self.get_f64(key)
    }

    /// Get an optional f64 parameter.
    pub fn get_number_opt(&self, key: &str) -> Result<Option<f64>, ToolError> {
        match self.params.get(key) {
            Some(v) => {
                let num = v.as_f64().ok_or_else(|| ToolError::InvalidParameter {
                    name: key.to_string(),
                    reason: "expected number".to_string(),
                })?;
                Ok(Some(num))
            }
            None => Ok(None),
        }
    }

    /// Get an optional boolean parameter.
    pub fn get_bool_opt(&self, key: &str) -> Result<Option<bool>, ToolError> {
        match self.params.get(key) {
            Some(v) => {
                let b = v.as_bool().ok_or_else(|| ToolError::InvalidParameter {
                    name: key.to_string(),
                    reason: "expected boolean".to_string(),
                })?;
                Ok(Some(b))
            }
            None => Ok(None),
        }
    }
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The result content (text or JSON).
    pub content: String,
    /// Whether the execution was successful.
    pub success: bool,
}

impl ToolOutput {
    /// Create a successful output.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            success: true,
        }
    }

    /// Create a failed output.
    pub fn failure(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            success: false,
        }
    }
}

/// Trait for tools that can be executed by the orchestrator.
///
/// Tools are external capabilities (web fetch, calculator, weather) that
/// take input parameters and return output. Unlike brain-core's ToolExecutor
/// which handles LLM tool calls, these tools are dispatched by the orchestrator.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The tool's unique name (used for dispatch).
    fn name(&self) -> &str;

    /// Human-readable description of what the tool does.
    fn description(&self) -> &str;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError>;
}
