//! Core decision layer for Aman regional alert subscriptions.
//!
//! This crate provides [`AgentBrain`], a Brain implementation that handles
//! onboarding, subscription state, and regional alert routing. Users can
//! subscribe to regions (e.g., "Iran", "Syria") and receive alerts when
//! events occur.
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
//! │  2. Parse command or region name    │
//! │  3. Update subscriptions            │
//! │  4. Return appropriate response     │
//! └─────────────────────────────────────┘
//!        ↓
//! Response back to user
//! ```
//!
//! # Commands
//!
//! - `help` / `?` - Show available commands
//! - `status` - Show current subscriptions
//! - `subscribe <region>` - Subscribe to a region
//! - `region <region>` - Alias for subscribe
//! - `stop` / `unsubscribe` - Unsubscribe from all alerts
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
//!     // Process a subscription request
//!     let message = InboundMessage::direct("+1234567890", "subscribe iran", 12345);
//!     let response = brain.process(message).await?;
//!     println!("{}", response.text); // "Subscribed to iran alerts."
//!
//!     Ok(())
//! }
//! ```
//!
//! # Regional Events
//!
//! Use [`RegionEvent`] to broadcast alerts to subscribers:
//!
//! ```rust,ignore
//! use agent_brain::RegionEvent;
//!
//! let event = RegionEvent {
//!     region: "Iran".to_string(),
//!     kind: "outage".to_string(),
//!     severity: "urgent".to_string(),
//!     confidence: "high".to_string(),
//!     summary: "Reported nationwide connectivity disruption.".to_string(),
//!     source_refs: vec!["https://example.org/report".to_string()],
//!     ts: Some("2025-01-01T12:00:00Z".to_string()),
//! };
//!
//! // Fan out to all subscribers
//! let messages = brain.fanout_event(&event).await?;
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
mod events;
mod regions;

pub use brain::AgentBrain;
pub use config::AgentBrainConfig;
pub use events::RegionEvent;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
