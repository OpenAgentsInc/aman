//! Tool registry and implementations for the Aman Signal bot.
//!
//! This crate provides a `ToolRegistry` for registering and executing tools
//! that the chatbot can use. Tools are external capabilities (web fetch,
//! calculator, weather, etc.) that take input and return output.
//!
//! # Architecture
//!
//! Unlike brain-core's `ToolExecutor` trait which handles LLM tool calls
//! (when a model like Maple calls tools), the `Tool` trait here is for
//! orchestrator-level actions dispatched based on routing decisions.
//! The [`RegistryToolExecutor`] adapter lets you expose the same registry
//! as a `ToolExecutor` for LLM tool calls, with optional policy controls.
//!
//! # Built-in Tools
//!
//! ## Utility Tools
//! - [`Calculator`] - Safe mathematical expression evaluation using `meval`.
//! - [`Weather`] - Weather information via wttr.in (no API key needed).
//! - [`WebFetch`] - Fetch URL content, convert HTML to text, optionally summarize.
//! - [`Dictionary`] - Word definitions via Free Dictionary API.
//! - [`WorldTime`] - Current time in any timezone via WorldTimeAPI.
//! - [`UnitConverter`] - Convert between common units (length, weight, temperature, etc.).
//! - [`RandomNumber`] - Generate random numbers, dice rolls, or coin flips.
//!
//! ## Financial Tools
//! - [`BitcoinPrice`] - Bitcoin price via mempool.space (privacy-friendly).
//! - [`CryptoPrice`] - Any cryptocurrency price via CoinGecko.
//! - [`CurrencyConverter`] - Fiat currency conversion via exchangerate.host.
//!
//! # Example
//!
//! ```rust,ignore
//! use agent_tools::{default_registry, ToolRegistry};
//! use std::collections::HashMap;
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create registry with all default tools
//!     let registry = default_registry();
//!
//!     // Execute a calculation
//!     let mut params = HashMap::new();
//!     params.insert("expression".to_string(), Value::String("2 + 2 * 3".to_string()));
//!
//!     let result = registry.execute("calculator", params).await.unwrap();
//!     println!("{}", result.content); // "2 + 2 * 3 = 8"
//! }
//! ```

mod error;
mod executor;
mod registry;
mod tool;
pub mod tools;

pub use error::ToolError;
pub use executor::{RateLimit, RegistryToolExecutor, ToolPolicy};
pub use registry::ToolRegistry;
pub use tool::{Tool, ToolArgs, ToolOutput};
pub use tools::{
    sanitize_system_prompt, BitcoinPrice, Calculator, CryptoPrice, CurrencyConverter, Dictionary,
    RandomNumber, Sanitize, UnitConverter, Weather, WebFetch, WorldTime,
};

// Re-export async_trait for convenience
pub use async_trait::async_trait;

/// Create a new registry with all built-in tools registered.
///
/// Note: The `sanitize` tool requires a brain for PII detection.
/// Call `registry.set_brain(brain)` to enable it.
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Utility tools
    registry.register(Calculator::new());
    registry.register(Weather::new());
    registry.register(WebFetch::new());
    registry.register(Dictionary::new());
    registry.register(WorldTime::new());
    registry.register(UnitConverter::new());
    registry.register(RandomNumber::new());

    // Financial tools
    registry.register(BitcoinPrice::new());
    registry.register(CryptoPrice::new());
    registry.register(CurrencyConverter::new());

    // AI-powered tools (require brain to be set)
    registry.register(Sanitize::new());

    registry
}
