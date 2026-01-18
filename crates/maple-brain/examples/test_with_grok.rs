//! Example: MapleBrain with Grok real-time search tools.
//!
//! This example demonstrates the privacy-preserving architecture where:
//! 1. User messages go to MapleBrain (in a TEE)
//! 2. MapleBrain can request real-time data by calling tools
//! 3. Only sanitized queries (crafted by MapleBrain) go to Grok
//! 4. Grok's response comes back to MapleBrain for final synthesis
//!
//! Required environment variables:
//! - MAPLE_API_KEY: OpenSecret API key
//! - GROK_API_KEY: xAI API key
//!
//! Optional:
//! - GROK_ENABLE_X_SEARCH=true (enabled by default for tool executor)
//! - GROK_ENABLE_WEB_SEARCH=true (enabled by default for tool executor)
//!
//! Run with:
//! ```bash
//! cargo run --example test_with_grok
//! ```

use brain_core::{Brain, InboundMessage, ToolExecutor};
use grok_brain::GrokToolExecutor;
use maple_brain::{MapleBrain, MapleBrainConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("maple_brain=debug".parse()?)
                .add_directive("grok_brain=debug".parse()?),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    println!("=== MapleBrain + Grok Tool Executor Demo ===\n");

    // Create Grok as a tool executor
    // This will be called when MapleBrain needs real-time data
    println!("Creating GrokToolExecutor...");
    let grok_executor = GrokToolExecutor::from_env()?;
    println!("  Supported tools: {:?}", grok_executor.supported_tools());

    // Create MapleBrain with Grok tools
    println!("\nCreating MapleBrain with tools...");
    let maple_config = MapleBrainConfig::from_env()?;
    let brain = MapleBrain::with_tools(maple_config, grok_executor).await?;
    println!("  Brain ready: {}", brain.name());
    println!("  Has tools: {}", brain.has_tools());

    // Test message that should trigger real-time search
    let test_messages = [
        "What's the latest news about AI?",
        "What's trending on X right now?",
        "Tell me about today's weather in major cities.",
    ];

    let sender = "+1234567890";

    for (i, text) in test_messages.iter().enumerate() {
        println!("\n--- Test {} ---", i + 1);
        println!("User: {}", text);

        let message = InboundMessage::direct(sender, *text, chrono::Utc::now().timestamp() as u64);

        match brain.process(message).await {
            Ok(response) => {
                println!("\nAssistant: {}", response.text);
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
            }
        }
    }

    // Clean up
    brain.shutdown().await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}
