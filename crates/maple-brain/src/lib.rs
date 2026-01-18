//! OpenSecret-based brain implementation.
//!
//! This crate provides a brain implementation that uses the OpenSecret SDK
//! to process messages through an AI model with end-to-end encryption.
//!
//! # Tool Support
//!
//! MapleBrain supports tool calling, allowing it to request external data
//! (like real-time search results) while preserving user privacy. The model
//! crafts sanitized queries that don't contain personal information.
//!
//! ```ignore
//! use maple_brain::{MapleBrain, MapleBrainConfig, ToolExecutor};
//! use grok_brain::GrokToolExecutor;
//!
//! let config = MapleBrainConfig::from_env()?;
//! let grok_executor = GrokToolExecutor::from_env().await?;
//! let brain = MapleBrain::with_tools(config, grok_executor).await?;
//! ```

mod brain;
mod config;
mod tools;

pub use brain::{MapleBrain, StatusCallback, StatusUpdate};
pub use config::MapleBrainConfig;
pub use tools::{ToolDefinition, ToolExecutor, ToolRequest, ToolResult};

// Re-export brain-core types for convenience
pub use brain_core::{
    async_trait, Brain, BrainError, ConversationHistory, HistoryMessage, InboundMessage,
    OutboundMessage,
};
