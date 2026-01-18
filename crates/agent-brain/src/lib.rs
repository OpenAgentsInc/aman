//! Core brain for Aman Signal bot.
//!
//! This crate provides [`AgentBrain`], a Brain implementation that handles
//! basic user management and message processing.
//!
//! # Architecture
//!
//! ```text
//! User Message (via Signal)
//!        ↓
//! ┌─────────────────────────────────────┐
//! │           AGENT-BRAIN               │
//! │                                     │
//! │  1. Ensure user exists in DB        │
//! │  2. Parse command                   │
//! │  3. Return appropriate response     │
//! └─────────────────────────────────────┘
//!        ↓
//! Response back to user
//! ```
//!
//! # Commands
//!
//! - `help` / `?` - Show available commands
//! - `status` - Show brain status
//!
//! # Example
//!
//! ```rust,ignore
//! use agent_brain::{AgentBrain, AgentBrainConfig};
//! use brain_core::{Brain, InboundMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create from environment
//!     let brain = AgentBrain::from_env().await?;
//!
//!     // Process a message
//!     let message = InboundMessage::direct("+1234567890", "hello", 12345);
//!     let response = brain.process(message).await?;
//!     println!("{}", response.text);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Configuration
//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `SQLITE_PATH` | `./data/aman.db` | SQLite database path |
//! | `AMAN_DEFAULT_LANGUAGE` | `English` | Default language for new users |

mod brain;
mod config;

pub use brain::AgentBrain;
pub use config::AgentBrainConfig;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
