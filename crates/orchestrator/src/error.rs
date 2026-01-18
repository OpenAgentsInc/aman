//! Error types for orchestrator operations.

use brain_core::BrainError;
use thiserror::Error;

/// Errors that can occur during orchestration.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// Message was intentionally skipped.
    #[error("message skipped: {0}")]
    Skipped(String),

    /// Routing failed.
    #[error("routing failed: {0}")]
    RoutingFailed(String),

    /// Brain processing failed.
    #[error("brain error: {0}")]
    Brain(#[from] BrainError),

    /// Tool execution failed.
    #[error("tool execution failed: {0}")]
    ToolFailed(String),

    /// Message sending failed.
    #[error("send failed: {0}")]
    SendFailed(String),

    /// Invalid routing plan from router.
    #[error("invalid routing plan: {0}")]
    InvalidPlan(String),
}
