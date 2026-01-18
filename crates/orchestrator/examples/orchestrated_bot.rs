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
//!
//! Logging configuration:
//!   AMAN_LOG_FILE      - Path to log file (default: logs/aman.log)
//!   AMAN_LOG_LEVEL     - Log level for file (default: debug)
//!   RUST_LOG           - Console log level (default: info)

use async_trait::async_trait;
use futures::StreamExt;
use orchestrator::{
    InboundMessage, MessageSender, Orchestrator, OrchestratorError,
};
use signal_daemon::{DaemonConfig, Envelope, ProcessConfig, SendParams, SignalClient};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

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

    async fn send_message_with_attachment(
        &self,
        recipient: &str,
        text: &str,
        attachment_path: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        let params = if is_group {
            SendParams::group(recipient, text)
        } else {
            SendParams::text(recipient, text)
        };
        let params = params.with_attachment(attachment_path);

        self.client
            .send(params)
            .await
            .map_err(|e| OrchestratorError::SendFailed(e.to_string()))?;
        Ok(())
    }
}

/// Convert a Signal envelope to an InboundMessage.
fn envelope_to_inbound(envelope: &Envelope) -> Option<InboundMessage> {
    let data_message = envelope.data_message.as_ref()?;

    // Convert attachments
    let attachments: Vec<brain_core::InboundAttachment> = data_message
        .attachments
        .iter()
        .map(|att| {
            // Resolve attachment path using signal-cli's default location
            let file_path = att.id.as_ref().map(|id| {
                // signal-cli stores attachments in ~/.local/share/signal-cli/attachments/
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                format!("{}/.local/share/signal-cli/attachments/{}", home, id)
            });

            brain_core::InboundAttachment {
                content_type: att.content_type.clone(),
                filename: att.filename.clone(),
                file_path,
                size: att.size,
                width: att.width,
                height: att.height,
                caption: att.caption.clone(),
            }
        })
        .collect();

    // Get text content - allow empty string if there are attachments
    let text = data_message
        .message
        .clone()
        .or_else(|| {
            if !attachments.is_empty() {
                Some(String::new())
            } else {
                None
            }
        })?;

    let group_id = data_message
        .group_info
        .as_ref()
        .map(|g| g.group_id.clone());

    Some(InboundMessage {
        sender: envelope.source.clone(),
        text,
        timestamp: envelope.timestamp,
        group_id,
        attachments,
        routing: None,
    })
}

/// Check if we should process this envelope.
fn should_process(envelope: &Envelope, bot_number: Option<&str>, startup_time_ms: u64) -> Result<(), String> {
    // Skip messages from ourselves
    if let Some(bot) = bot_number {
        if envelope.source == bot || envelope.source_number == bot {
            return Err("message from self".to_string());
        }
    }

    // Skip messages older than startup time (prevents replay of old messages)
    if envelope.timestamp < startup_time_ms {
        return Err(format!(
            "old message (ts={}, startup={})",
            envelope.timestamp, startup_time_ms
        ));
    }

    // Must have a data message
    let data_message = envelope
        .data_message
        .as_ref()
        .ok_or_else(|| "no data message".to_string())?;

    // Must have either text or attachments
    let has_text = data_message.message.is_some();
    let has_attachments = !data_message.attachments.is_empty();

    if !has_text && !has_attachments {
        return Err("no text content or attachments".to_string());
    }

    Ok(())
}

/// Set up logging with both console and file output.
///
/// Returns a guard that must be kept alive for the duration of the program
/// to ensure logs are flushed to the file.
fn setup_logging(log_file_path: &str) -> Result<tracing_appender::non_blocking::WorkerGuard, Box<dyn std::error::Error>> {
    use std::path::Path;

    // Create logs directory if needed
    if let Some(parent) = Path::new(log_file_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Set up file appender
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)?;
    let (file_writer, guard) = tracing_appender::non_blocking(file);

    // Console layer - human readable, respects RUST_LOG
    let console_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new("info")
                .add_directive("orchestrator=debug".parse().unwrap())
                .add_directive("maple_brain=info".parse().unwrap())
                .add_directive("grok_brain=info".parse().unwrap())
        });

    let console_layer = fmt::layer()
        .with_target(true)
        .with_filter(console_filter);

    // File layer - JSON format, debug level for full payloads
    let file_filter = env::var("AMAN_LOG_LEVEL")
        .ok()
        .and_then(|level| EnvFilter::try_new(&level).ok())
        .unwrap_or_else(|| {
            EnvFilter::new("debug")
                .add_directive("orchestrator=debug".parse().unwrap())
                .add_directive("maple_brain=debug".parse().unwrap())
                .add_directive("grok_brain=debug".parse().unwrap())
                .add_directive("agent_tools=debug".parse().unwrap())
                .add_directive("hyper=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap())
        });

    let file_layer = fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_filter(file_filter);

    // Initialize the subscriber with both layers
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    println!("Logging to file: {}", log_file_path);
    println!("  Tail logs: tail -f {}", log_file_path);
    println!("  View JSON: cat {} | jq", log_file_path);

    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize logging with file and console output
    let log_file_path = env::var("AMAN_LOG_FILE").unwrap_or_else(|_| "logs/aman.log".to_string());
    let _guard = setup_logging(&log_file_path)?;

    // Get bot number from environment
    let bot_number = env::var("AMAN_NUMBER").ok();

    // Capture startup time for filtering old messages
    let startup_time_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64;

    // Connect to Signal daemon
    let (mut process, client) = if let Some(ref account) = bot_number {
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

    // Main message loop with shutdown handling
    loop {
        tokio::select! {
            // Handle Ctrl+C gracefully
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received, stopping...");
                break;
            }
            // Process incoming messages
            result = stream.next() => {
                match result {
                    Some(Ok(envelope)) => {
                        // Check if we should process this message
                        if let Err(reason) = should_process(&envelope, bot_number.as_deref(), startup_time_ms) {
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
                    Some(Err(e)) => {
                        warn!("Stream error: {}", e);
                        // Continue on stream errors - they might be recoverable
                    }
                    None => {
                        warn!("Message stream ended");
                        break;
                    }
                }
            }
        }
    }

    // Clean up daemon process
    if let Some(ref mut proc) = process {
        info!("Stopping signal-cli daemon...");
        if let Err(e) = proc.kill() {
            warn!("Failed to kill daemon: {}", e);
        }
    }

    info!("Shutdown complete");
    Ok(())
}
