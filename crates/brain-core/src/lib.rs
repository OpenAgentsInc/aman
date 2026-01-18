//! Core trait and types for brain implementations.
//!
//! This crate provides the shared interface for all brain implementations
//! in the Aman Signal bot ecosystem. It defines:
//!
//! - [`Brain`] - The trait that all brain implementations must implement
//! - [`InboundMessage`] / [`OutboundMessage`] - Message types for input/output
//! - [`BrainError`] - Error types for brain operations
//! - [`ToolExecutor`] - Trait for external tool execution (e.g., real-time search)
//!
//! # Example
//!
//! ```rust
//! use brain_core::{Brain, BrainError, InboundMessage, OutboundMessage};
//! use async_trait::async_trait;
//!
//! struct MyBrain;
//!
//! #[async_trait]
//! impl Brain for MyBrain {
//!     async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
//!         Ok(OutboundMessage::reply_to(&message, "Hello!"))
//!     }
//!
//!     fn name(&self) -> &str {
//!         "MyBrain"
//!     }
//! }
//! ```

mod error;
mod message;
mod tools;
mod trait_def;

pub use error::BrainError;
pub use message::{InboundAttachment, InboundMessage, OutboundMessage};
pub use tools::{ToolExecutor, ToolRequest, ToolResult};
pub use trait_def::Brain;

// Re-export async_trait for convenience
pub use async_trait::async_trait;
