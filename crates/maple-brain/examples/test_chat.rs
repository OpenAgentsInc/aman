//! Simple test for MapleBrain chat completion.
//!
//! Run with: cargo run -p maple-brain --example test_chat
//! Or with a custom message: cargo run -p maple-brain --example test_chat -- "Your message here"
//!
//! Make sure to set environment variables in .env:
//!   MAPLE_API_URL - OpenSecret API URL
//!   MAPLE_API_KEY - API key for authentication

use maple_brain::{Brain, InboundMessage, MapleBrain};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get message from command line args or use default
    let args: Vec<String> = env::args().collect();
    let message_text = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "Hello! Please respond with a short greeting.".to_string()
    };

    println!("Initializing MapleBrain...");
    let brain = MapleBrain::from_env().await?;

    println!("Brain initialized: {}", brain.name());
    println!("API URL: {}", brain.config().api_url);
    println!("Model: {}", brain.config().model);
    if let Some(ref prompt) = brain.config().system_prompt {
        let preview: String = prompt.chars().take(50).collect();
        let suffix = if prompt.len() > 50 { "..." } else { "" };
        println!("System prompt: \"{}{}\"\n", preview, suffix);
    } else {
        println!("System prompt: (none)\n");
    }

    // Test message
    let test_message = InboundMessage::direct(
        "+1234567890",  // fake sender
        &message_text,
        1234567890,
    );

    println!("Sending: \"{}\"", test_message.text);
    println!("Waiting for response...\n");

    let response = brain.process(test_message).await?;

    println!("=== Response ===");
    println!("{}", response.text);
    println!("================");

    Ok(())
}
