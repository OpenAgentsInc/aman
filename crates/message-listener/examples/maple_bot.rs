//! AI bot example using MapleBrain with OpenSecret API.
//!
//! This example demonstrates how to create a bot that uses the OpenSecret
//! encrypted AI API to generate responses. It maintains conversation history
//! per sender.
//!
//! Run with: cargo run -p message-listener --example maple_bot --features maple
//!
//! Configuration via .env file or environment variables:
//!   AMAN_NUMBER        - Bot's phone number (required for auto-spawn)
//!   SIGNAL_CLI_JAR     - JAR path (default: build/signal-cli.jar)
//!   MAPLE_API_URL      - OpenSecret API URL (default: https://api.opensecret.cloud)
//!   MAPLE_API_KEY      - API key for authentication (required)
//!   MAPLE_MODEL        - Model name (default: hugging-quants/Meta-Llama-3.1-70B-Instruct-AWQ-INT4)
//!   MAPLE_SYSTEM_PROMPT - System prompt (optional)
//!   MAPLE_MAX_TOKENS   - Max tokens (default: 1024)
//!   MAPLE_TEMPERATURE  - Temperature (default: 0.7)

use message_listener::{Brain, MapleBrain, MessageProcessor, ProcessorConfig};
use signal_daemon::{DaemonConfig, ProcessConfig, SignalClient};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (searches current dir and parents)
    let _ = dotenvy::dotenv();

    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Get bot number from environment
    let bot_number = env::var("AMAN_NUMBER").ok();

    // Create the AI brain from environment variables
    println!("Initializing MapleBrain...");
    let brain = MapleBrain::from_env().await?;
    println!("Using brain: {}", brain.name());
    println!("API URL: {}", brain.config().api_url);
    println!("Model: {}", brain.config().model);

    let (_process, client) = if let Some(ref account) = bot_number {
        // Spawn daemon from JAR
        let jar_path = env::var("SIGNAL_CLI_JAR")
            .unwrap_or_else(|_| "build/signal-cli.jar".to_string());

        println!("Spawning daemon for account {}...", account);
        println!("JAR: {}", jar_path);

        let config = ProcessConfig::new(&jar_path, account);
        let (process, client) =
            signal_daemon::spawn_and_connect(config, Duration::from_secs(30)).await?;

        (Some(process), client)
    } else {
        // Connect to existing daemon
        println!("AMAN_NUMBER not set, connecting to existing daemon...");
        let config = DaemonConfig::default();
        println!("Connecting to {}...", config.base_url);

        let client = SignalClient::connect(config).await?;
        (None, client)
    };

    println!("Connected!");

    // Create processor config with typing indicators enabled
    let mut config = if let Some(number) = bot_number {
        ProcessorConfig::with_bot_number(number)
    } else {
        ProcessorConfig::default()
    };
    config.send_typing_indicators = true;

    // Create and run the processor
    let processor = MessageProcessor::new(client, brain, config);

    println!("\nMaple AI bot is running!");
    println!("Send a message to the bot's number to chat.");
    println!("The bot maintains conversation history per sender.");
    println!("Press Ctrl+C to stop.\n");

    // Run until error or Ctrl+C
    processor.run().await?;

    Ok(())
}
