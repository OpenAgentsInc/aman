//! Echo bot example using MessageProcessor.
//!
//! This example demonstrates how to create a simple bot that echoes
//! all incoming messages using the MessageProcessor and EchoBrain.
//! It sends typing indicators while processing messages.
//!
//! Run with: cargo run -p message-listener --example processor_bot
//!
//! Configuration via .env file or environment variables:
//!   AMAN_NUMBER     - Bot's phone number (required for auto-spawn, used to filter self-messages)
//!   SIGNAL_CLI_JAR  - JAR path (default: build/signal-cli.jar)

use message_listener::{Brain, EchoBrain, MessageProcessor, ProcessorConfig};
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

    // Create the brain
    let brain = EchoBrain::with_prefix("Echo: ");
    println!("Using brain: {}", brain.name());

    // Create processor config (filter out messages from self, enable typing indicators)
    let mut config = if let Some(number) = bot_number {
        ProcessorConfig::with_bot_number(number)
    } else {
        ProcessorConfig::default()
    };
    config.send_typing_indicators = true;

    // Create and run the processor
    let processor = MessageProcessor::new(client, brain, config);

    println!("\nEcho bot is running!");
    println!("Send a message to the bot's number to test.");
    println!("Press Ctrl+C to stop.\n");

    // Run until error or Ctrl+C
    processor.run().await?;

    Ok(())
}
