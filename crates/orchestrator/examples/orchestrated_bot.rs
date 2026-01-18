//! Orchestrated AI bot example.
//!
//! This example demonstrates how to create a bot that uses the Orchestrator
//! to route messages, execute tools, and generate responses.
//!
//! Run with: cargo run -p orchestrator --example orchestrated_bot
//!
//! Configuration via .env file or environment variables:
//!   AMAN_NUMBER        - Bot's phone number (required for auto-spawn)
//!   SIGNAL_CLI_JAR     - JAR path (default: build/signal-cli.jar)
//!   MAPLE_API_URL      - OpenSecret API URL
//!   MAPLE_API_KEY      - API key for MapleBrain (required)
//!   GROK_API_KEY       - API key for Grok search (required)

use async_trait::async_trait;
use futures::StreamExt;
use orchestrator::{
    InboundMessage, MessageSender, Orchestrator, OrchestratorError,
};
use signal_daemon::{DaemonConfig, Envelope, ProcessConfig, SignalClient};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Signal-based message sender for the orchestrator.
#[derive(Clone)]
pub struct SignalMessageSender {
    client: SignalClient,
}

impl SignalMessageSender {
    /// Create a new Signal message sender.
    pub fn new(client: SignalClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MessageSender for SignalMessageSender {
    async fn send_message(
        &self,
        recipient: &str,
        text: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        let result = if is_group {
            self.client.send_to_group(recipient, text).await
        } else {
            self.client.send_text(recipient, text).await
        };

        result.map_err(|e| OrchestratorError::SendFailed(e.to_string()))?;
        Ok(())
    }

    async fn set_typing(
        &self,
        recipient: &str,
        is_group: bool,
        started: bool,
    ) -> Result<(), OrchestratorError> {
        let result = if is_group {
            self.client.send_typing_to_group(recipient, started).await
        } else {
            self.client.send_typing(recipient, started).await
        };

        result.map_err(|e| OrchestratorError::SendFailed(e.to_string()))?;
        Ok(())
    }
}

/// Convert a Signal envelope to an InboundMessage.
fn envelope_to_inbound(envelope: &Envelope) -> Option<InboundMessage> {
    let data_message = envelope.data_message.as_ref()?;
    let text = data_message.message.as_ref()?;

    let group_id = data_message
        .group_info
        .as_ref()
        .map(|g| g.group_id.clone());

    Some(InboundMessage {
        sender: envelope.source.clone(),
        text: text.clone(),
        timestamp: envelope.timestamp,
        group_id,
        attachments: Vec::new(), // TODO: Handle attachments
        routing: None,
    })
}

/// Check if we should process this envelope.
fn should_process(envelope: &Envelope, bot_number: Option<&str>) -> Result<(), String> {
    // Skip messages from ourselves
    if let Some(bot) = bot_number {
        if envelope.source == bot || envelope.source_number == bot {
            return Err("message from self".to_string());
        }
    }

    // Must have a data message with text
    let data_message = envelope
        .data_message
        .as_ref()
        .ok_or_else(|| "no data message".to_string())?;

    data_message
        .message
        .as_ref()
        .ok_or_else(|| "no text content".to_string())?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("orchestrator=debug".parse().unwrap())
                .add_directive("maple_brain=info".parse().unwrap())
                .add_directive("grok_brain=info".parse().unwrap()),
        )
        .init();

    // Get bot number from environment
    let bot_number = env::var("AMAN_NUMBER").ok();

    // Connect to Signal daemon
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

    println!("Connected to Signal daemon!");

    // Create the message sender
    let sender = SignalMessageSender::new(client.clone());

    // Create the orchestrator
    println!("Initializing orchestrator...");
    let orchestrator = Arc::new(Orchestrator::from_env(sender).await?);
    println!("Orchestrator ready!");

    println!("\nOrchestrated AI bot is running!");
    println!("Features:");
    println!("  - Routes messages through privacy-preserving TEE");
    println!("  - Executes real-time searches when needed");
    println!("  - Maintains conversation history per sender");
    println!("  - Sends typing indicators during processing");
    println!("\nSend a message to test. Try:");
    println!("  - \"hello\" - Simple chat");
    println!("  - \"what's happening in tech today?\" - Will search first");
    println!("  - \"forget our conversation\" - Clears history");
    println!("  - \"what can you do?\" - Shows help");
    println!("\nPress Ctrl+C to stop.\n");

    // Subscribe to messages and process them
    let mut stream = signal_daemon::subscribe(&client)?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(envelope) => {
                // Check if we should process this message
                if let Err(reason) = should_process(&envelope, bot_number.as_deref()) {
                    debug!("Skipping message: {}", reason);
                    continue;
                }

                // Convert to inbound message
                let inbound = match envelope_to_inbound(&envelope) {
                    Some(msg) => msg,
                    None => {
                        debug!("Could not convert envelope to inbound message");
                        continue;
                    }
                };

                info!("Received message from {}: {}", inbound.sender, inbound.text);

                // Process through orchestrator
                let orchestrator = orchestrator.clone();
                let client = client.clone();
                let inbound_clone = inbound.clone();

                // Spawn a task to process the message
                tokio::spawn(async move {
                    match orchestrator.process(inbound_clone.clone()).await {
                        Ok(response) => {
                            // Send the final response
                            let send_result = if response.is_group {
                                client.send_to_group(&response.recipient, &response.text).await
                            } else {
                                client.send_text(&response.recipient, &response.text).await
                            };

                            match send_result {
                                Ok(_) => {
                                    info!(
                                        "Sent response to {}: {} chars",
                                        response.recipient,
                                        response.text.len()
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to send response: {}", e);
                                }
                            }
                        }
                        Err(OrchestratorError::Skipped(reason)) => {
                            debug!("Message skipped by orchestrator: {}", reason);
                        }
                        Err(e) => {
                            error!("Orchestrator error: {}", e);

                            // Try to send an error message to the user
                            let error_msg = "Sorry, I encountered an error processing your message. Please try again.";
                            let recipient = inbound_clone.group_id.as_ref().unwrap_or(&inbound_clone.sender);
                            let is_group = inbound_clone.group_id.is_some();

                            let _ = if is_group {
                                client.send_to_group(recipient, error_msg).await
                            } else {
                                client.send_text(recipient, error_msg).await
                            };
                        }
                    }
                });
            }
            Err(e) => {
                warn!("Stream error: {}", e);
                // Continue on stream errors - they might be recoverable
            }
        }
    }

    warn!("Message stream ended");
    Ok(())
}
