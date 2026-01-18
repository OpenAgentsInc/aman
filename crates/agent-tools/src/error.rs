//! Error types for tool operations.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool not found in registry.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Missing required parameter.
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// Invalid parameter value.
    #[error("Invalid parameter '{name}': {reason}")]
    InvalidParameter { name: String, reason: String },

    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// JSON parsing failed.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Expression evaluation failed (calculator).
    #[error("Evaluation error: {0}")]
    EvalError(String),

    /// General execution error.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Brain processing error.
    #[error("Brain error: {0}")]
    BrainError(String),
}
