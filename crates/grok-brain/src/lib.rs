//! xAI Grok-based brain implementation.
//!
//! This crate provides a brain implementation that uses the xAI Grok API
//! to process messages with optional real-time search capabilities.
//!
//! # Features
//!
//! - Uses xAI's Grok 4.1 Fast model for quick responses
//! - Per-sender conversation history
//! - Optional X Search for real-time Twitter/X data
//! - Optional Web Search for current web information
//! - Configurable via environment variables
//! - **ToolExecutor implementation** for use with MapleBrain
//!
//! # Standalone Usage
//!
//! ```rust,no_run
//! use grok_brain::GrokBrain;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let brain = GrokBrain::from_env().await?;
//!     // Use the brain...
//!     Ok(())
//! }
//! ```
//!
//! # As a Tool Executor for MapleBrain
//!
//! ```rust,ignore
//! use grok_brain::GrokToolExecutor;
//! use maple_brain::{MapleBrain, MapleBrainConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create Grok as a tool executor
//!     let grok_executor = GrokToolExecutor::from_env()?;
//!
//!     // Create MapleBrain with Grok tools
//!     let maple_config = MapleBrainConfig::from_env()?;
//!     let brain = MapleBrain::with_tools(maple_config, grok_executor).await?;
//!
//!     // Now maple-brain can request real-time data via Grok
//!     // while preserving user privacy (only sanitized queries go to Grok)
//!     Ok(())
//! }
//! ```

mod api_types;
mod brain;
mod config;
mod tool_executor;

pub use brain::GrokBrain;
pub use config::GrokBrainConfig;
pub use tool_executor::GrokToolExecutor;

// Re-export brain-core types for convenience
pub use brain_core::{
    async_trait, Brain, BrainError, ConversationHistory, HistoryMessage, InboundMessage,
    OutboundMessage, ToolExecutor,
};
