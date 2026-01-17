//! Simple echo bot example.
//!
//! Run with: cargo run --example echo_bot
//!
//! Configuration via .env file or environment variables:
//!   AMAN_NUMBER     - If set, spawns daemon automatically
//!   SIGNAL_CLI_JAR  - JAR path (default: ../../build/signal-cli.jar)
//!
//! Or start daemon separately and run without AMAN_NUMBER:
//!   Terminal 1: ./scripts/run-signal-daemon.sh
//!   Terminal 2: cargo run --example echo_bot

use futures::StreamExt;
use signal_daemon::{subscribe, DaemonConfig, ProcessConfig, SignalClient};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (from project root)
    let _ = dotenvy::from_path("../../.env");

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
                let sender = &envelope.source;

                if let Some(msg) = &envelope.data_message {
                    if let Some(text) = &msg.message {
                        println!("[{}] Received: {}", sender, text);

                        // Echo back
                        let reply = format!("Echo: {}", text);
                        match client.send_text(sender, &reply).await {
                            Ok(result) => {
                                println!("[{}] Sent reply (ts={})", sender, result.timestamp);
                            }
                            Err(e) => {
                                eprintln!("[{}] Failed to send: {}", sender, e);
                            }
                        }
                    }
                }

                if let Some(_receipt) = &envelope.receipt_message {
                    println!("[{}] Receipt received", sender);
                }

                if let Some(typing) = &envelope.typing_message {
                    println!("[{}] Typing: {}", sender, typing.action);
                }
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
            }
        }
    }

    Ok(())
}
