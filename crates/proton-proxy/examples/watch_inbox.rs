//! Example: Watch INBOX for new messages
//!
//! This example polls the INBOX folder and prints new messages as they arrive.
//!
//! Required environment variables:
//! - PROTON_USERNAME - Your Proton email
//! - PROTON_PASSWORD - Bridge password (from `protonmail-bridge -c` then `info`)
//!
//! Run with:
//! ```bash
//! cargo run -p proton-proxy --example watch_inbox
//! ```

use proton_proxy::{InboxMessage, InboxWatcher, ProtonConfig, ProtonError};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), ProtonError> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("proton_proxy=debug,watch_inbox=debug")
        .init();

    println!("Loading config from environment...");
    let config = ProtonConfig::from_env()?;
    println!("Username: {}", config.username);

    // Create watcher with 5 second poll interval
    let watcher = InboxWatcher::new(config).with_poll_interval(Duration::from_secs(5));

    println!("Starting INBOX watcher (poll every 5 seconds)...");
    println!("Press Ctrl+C to stop.\n");

    // Watch INBOX and print new messages
    watcher
        .watch("INBOX", |msg: InboxMessage| async move {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📬 NEW MESSAGE");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("UID:     {}", msg.uid);
            if let Some(from) = &msg.from {
                if let Some(name) = &msg.from_name {
                    println!("From:    {} <{}>", name, from);
                } else {
                    println!("From:    {}", from);
                }
            }
            println!("To:      {}", msg.to.join(", "));
            println!("Subject: {}", msg.subject);
            if let Some(date) = &msg.date {
                println!("Date:    {}", date);
            }
            println!();

            // Print body preview
            if let Some(body) = &msg.body {
                let preview = if body.len() > 200 {
                    format!("{}...", &body[..200])
                } else {
                    body.clone()
                };
                println!("Body:\n{}", preview);
            }

            // List attachments
            if !msg.attachments.is_empty() {
                println!("\nAttachments:");
                for att in &msg.attachments {
                    println!("  - {} ({}, {} bytes)", att.filename, att.content_type, att.data.len());
                }
            }

            println!();
            Ok(())
        })
        .await?;

    Ok(())
}
