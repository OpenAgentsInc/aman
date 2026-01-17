//! Simple health check example.
//!
//! Run with: cargo run --example health_check
//!
//! Set AMAN_NUMBER environment variable or it will try to connect to existing daemon.
//!
//! Examples:
//!   AMAN_NUMBER=+1234567890 cargo run --example health_check   # Spawns daemon
//!   cargo run --example health_check                            # Connects to existing

use signal_daemon::{DaemonConfig, ProcessConfig, SignalClient};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    println!("Connected!");

    let version = client.version().await?;
    println!("signal-cli version: {}", version);

    let healthy = client.health_check().await?;
    println!("Health check: {}", if healthy { "OK" } else { "FAILED" });

    // Process is automatically killed when dropped
    Ok(())
}
