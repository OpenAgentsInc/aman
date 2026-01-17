//! Example: Echo bot using mock-brain with signal-daemon.
//!
//! This example demonstrates how to use the mock-brain crate with
//! signal-daemon to create a simple echo bot.
//!
//! Run with: cargo run --example echo_with_signal --features signal-daemon
//!
//! Configuration via .env file or environment variables:
//!   AMAN_NUMBER     - If set, spawns daemon automatically
//!   SIGNAL_CLI_JAR  - JAR path (default: ../../build/signal-cli.jar)

use futures::StreamExt;
use mock_brain::{Brain, EchoBrain, EnvelopeExt};
use signal_daemon::{subscribe, DaemonConfig, ProcessConfig, SignalClient};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (from project root)
    let _ = dotenvy::from_path("../../.env");

    // Create our brain
    let brain = EchoBrain::with_prefix("Echo: ");
    println!("Using brain: {}", brain.name());

    // Check if we should spawn daemon or connect to existing
    let account = env::var("AMAN_NUMBER").ok();

    let (_process, client) = if let Some(account) = account {
        // Spawn daemon from JAR
        let jar_path = env::var("SIGNAL_CLI_JAR")
            .unwrap_or_else(|_| "../../build/signal-cli.jar".to_string());

        println!("Spawning daemon for account {}...", account);
        println!("JAR: {}", jar_path);

        let config = ProcessConfig::new(&jar_path, &account);
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

    println!("Connected! Waiting for messages...");
    println!("Send a message to the bot's number to test.");
    println!("Press Ctrl+C to stop.\n");

    let mut stream = subscribe(&client);

    while let Some(result) = stream.next().await {
        match result {
            Ok(envelope) => {
                // Try to convert envelope to inbound message
                if let Some(inbound) = envelope.to_inbound_message() {
                    println!("[{}] Received: {}", inbound.sender, inbound.text);

                    // Process with brain
                    match brain.process(inbound.clone()).await {
                        Ok(response) => {
                            // Send response
                            match mock_brain::send_response(&client, &response).await {
                                Ok(result) => {
                                    println!(
                                        "[{}] Sent reply (ts={}): {}",
                                        response.recipient, result.timestamp, response.text
                                    );
                                }
                                Err(e) => {
                                    eprintln!("[{}] Failed to send: {}", response.recipient, e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[{}] Brain error: {}", inbound.sender, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
            }
        }
    }

    Ok(())
}
