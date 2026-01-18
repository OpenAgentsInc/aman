//! Test GrokBrain with X Search and Web Search enabled.
//!
//! Run with: cargo run -p grok-brain --example test_search
//! Or with a custom query: cargo run -p grok-brain --example test_search -- "What's trending on X right now?"
//!
//! This example enables both X Search and Web Search tools to test real-time
//! information retrieval. Note: These tools incur additional costs ($5/invocation).
//!
//! Make sure to set environment variables in .env:
//!   GROK_API_KEY - xAI API key for authentication

use grok_brain::{Brain, GrokBrain, GrokBrainConfig, InboundMessage};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get API key from environment
    let api_key = env::var("GROK_API_KEY").expect("GROK_API_KEY must be set");

    // Get query from command line args or use default
    let args: Vec<String> = env::args().collect();
    let query = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "What are the top trending topics on Twitter right now? Give me a brief summary.".to_string()
    };

    // Build config with search tools enabled
    let config = GrokBrainConfig::builder()
        .api_key(api_key)
        .model("grok-4-1-fast")
        .system_prompt("You are a helpful assistant with access to real-time information. When asked about current events, trending topics, or recent news, use your search capabilities to provide accurate, up-to-date information.")
        .enable_x_search(true)
        .enable_web_search(true)
        .max_tokens(2048)
        .build();

    println!("Initializing GrokBrain with search tools...");
    let brain = GrokBrain::new(config)?;

    println!("Brain initialized: {}", brain.name());
    println!("Model: {}", brain.config().model);
    println!("X Search enabled: {}", brain.config().enable_x_search);
    println!("Web Search enabled: {}", brain.config().enable_web_search);
    println!();
    println!("WARNING: Search tools incur additional costs ($5/invocation)");
    println!();

    // Test message
    let test_message = InboundMessage::direct(
        "+1234567890", // fake sender
        &query,
        1234567890,
    );

    println!("Query: \"{}\"", test_message.text);
    println!("Waiting for response (this may take longer due to search)...\n");

    let response = brain.process(test_message).await?;

    println!("=== Response ===");
    println!("{}", response.text);
    println!("================");

    Ok(())
}
