//! Agent brain utilities for Aman.

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
