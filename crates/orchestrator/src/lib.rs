//! Message orchestrator for coordinating brain routing and tool execution.
//!
//! This crate provides the [`Orchestrator`] type which coordinates message
//! processing between signal-daemon, maple-brain, and grok-brain.
//!
//! # Features
//!
//! - Routes messages through maple-brain for classification
//! - Executes multiple tool calls and actions (search, clear context, etc.)
//! - Sends interim status messages to the user
//! - Maintains typing indicators throughout processing
//! - Keeps all routing decisions private (via maple-brain TEE)
//!
//! # Architecture
//!
//! ```text
//! Signal Message (from message-listener)
//!          ↓
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      ORCHESTRATOR                           │
//! │                                                             │
//! │  1. Start typing indicator                                  │
//! │         ↓                                                   │
//! │  2. Route message (maple-brain, stateless)                  │
//! │         ↓                                                   │
//! │  3. Execute actions sequentially:                           │
//! │     • search → Call grok, send "Searching..." message       │
//! │     • clear_context → Clear brain history, send confirm     │
//! │     • respond → Pass to maple-brain for final response      │
//! │         ↓                                                   │
//! │  4. Stop typing indicator                                   │
//! │         ↓                                                   │
//! │  5. Send final response                                     │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use orchestrator::{Orchestrator, MessageSender, OrchestratorError};
//! use brain_core::InboundMessage;
//!
//! // Implement MessageSender for your transport
//! struct SignalSender { /* ... */ }
//!
//! #[async_trait]
//! impl MessageSender for SignalSender {
//!     async fn send_message(&self, recipient: &str, text: &str, is_group: bool)
//!         -> Result<(), OrchestratorError> {
//!         // Send via Signal
//!         Ok(())
//!     }
//!
//!     async fn set_typing(&self, recipient: &str, is_group: bool, started: bool)
//!         -> Result<(), OrchestratorError> {
//!         // Set typing indicator
//!         Ok(())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let sender = SignalSender { /* ... */ };
//!     let orchestrator = Orchestrator::from_env(sender).await?;
//!
//!     let message = InboundMessage::direct("+1234567890", "What's the weather?", 123);
//!     let response = orchestrator.process(message).await?;
//!
//!     println!("Response: {}", response.text);
//!     Ok(())
//! }
//! ```

mod actions;
mod context;
mod error;
mod model_selection;
mod orchestrator;
mod preferences;
mod router;
mod sender;

// Public exports
pub use actions::{OrchestratorAction, RoutingPlan, Sensitivity, TaskHint, UserPreference};
pub use context::Context;
pub use error::OrchestratorError;
pub use model_selection::{GrokModels, MapleModels, ModelSelector};
pub use orchestrator::{Orchestrator, HELP_TEXT};
pub use preferences::{AgentIndicator, PreferenceStore};
pub use router::{load_router_prompt, Router, DEFAULT_ROUTER_PROMPT_FILE, DEFAULT_ROUTER_SYSTEM_PROMPT};
pub use sender::{LoggingSender, MessageSender, NoOpSender};

// Re-export commonly used types from dependencies
pub use brain_core::{InboundMessage, OutboundMessage};
pub use grok_brain::GrokToolExecutor;
pub use maple_brain::{MapleBrain, MapleBrainConfig};
