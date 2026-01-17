# signal-daemon

Rust client library for communicating with the signal-cli daemon over HTTP.

## Quick Reference

### Spawn Daemon from JAR

```rust
use signal_daemon::{ProcessConfig, spawn_and_connect};
use std::time::Duration;

// Configure and spawn daemon
let config = ProcessConfig::new("build/signal-cli.jar", "+1234567890")
    .with_http_addr("127.0.0.1:8080");

// Spawn daemon and get connected client
let (mut process, client) = spawn_and_connect(config, Duration::from_secs(30)).await?;

// Use client...
client.send_text("+0987654321", "Hello!").await?;

// Process is killed automatically when dropped
// Or manually: process.kill()?;
```

### Connect to Daemon

```rust
use signal_daemon::{DaemonConfig, SignalClient};

// Default: http://localhost:8080
let client = SignalClient::connect(DaemonConfig::default()).await?;

// Custom URL
let client = SignalClient::connect(DaemonConfig::new("http://127.0.0.1:9000")).await?;

// Multi-account mode
let client = SignalClient::connect(
    DaemonConfig::with_account("http://localhost:8080", "+1234567890")
).await?;
```

### Send Messages

```rust
// Simple text message
client.send_text("+1234567890", "Hello!").await?;

// Group message
client.send_to_group("GROUP_ID_BASE64", "Hello group!").await?;

// With attachments
use signal_daemon::SendParams;
let params = SendParams::text("+1234567890", "Check this out")
    .with_attachment("/path/to/file.jpg");
client.send(params).await?;

// Reply to a message
let params = SendParams::text("+1234567890", "This is a reply")
    .with_quote(1234567890123, "+0987654321");
client.send(params).await?;

// Typing indicator
client.send_typing("+1234567890", true).await?;
client.send_typing("+1234567890", false).await?;
```

### Receive Messages (SSE Stream)

```rust
use signal_daemon::subscribe;
use futures::StreamExt;

let mut stream = subscribe(&client);

while let Some(result) = stream.next().await {
    match result {
        Ok(envelope) => {
            // Source info
            let sender = &envelope.source;           // Phone number
            let sender_name = &envelope.source_name; // Contact name (Option)
            let timestamp = envelope.timestamp;      // Unix ms

            // Check message type
            if let Some(msg) = &envelope.data_message {
                // Regular incoming message
                if let Some(text) = &msg.message {
                    println!("Message from {}: {}", sender, text);
                }

                // Check if group message
                if let Some(group) = &msg.group_info {
                    println!("Group ID: {}", group.group_id);
                }

                // Handle attachments
                for attachment in &msg.attachments {
                    println!("Attachment: {} ({})",
                        attachment.filename.as_deref().unwrap_or("unknown"),
                        attachment.content_type);
                }
            }

            if let Some(sync) = &envelope.sync_message {
                // Message sent from another linked device
            }

            if let Some(receipt) = &envelope.receipt_message {
                // Delivery/read receipt
            }

            if let Some(typing) = &envelope.typing_message {
                // Typing indicator: "STARTED" or "STOPPED"
            }
        }
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
```

### Health Check

```rust
// Check if daemon is running
let healthy = client.health_check().await?;

// Get daemon version
let version = client.version().await?;

// Check connection state (non-blocking)
let connected = client.is_connected();

// Background health monitoring
use std::time::Duration;
let handle = client.start_health_monitor(Duration::from_secs(30));
```

## Types Reference

### Envelope (incoming message wrapper)

| Field | Type | Description |
|-------|------|-------------|
| `source` | `String` | Sender phone number |
| `source_number` | `String` | Same as source |
| `source_uuid` | `Option<String>` | Sender UUID |
| `source_name` | `Option<String>` | Contact name if known |
| `source_device` | `Option<u32>` | Device ID |
| `timestamp` | `u64` | Unix timestamp (ms) |
| `data_message` | `Option<DataMessage>` | Regular message content |
| `sync_message` | `Option<SyncMessage>` | Sync from linked device |
| `receipt_message` | `Option<ReceiptMessage>` | Delivery/read receipt |
| `typing_message` | `Option<TypingMessage>` | Typing indicator |

### DataMessage (message content)

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Option<String>` | Text content |
| `timestamp` | `u64` | Message timestamp |
| `expires_in_seconds` | `u32` | Disappearing message timer |
| `view_once` | `bool` | View-once message flag |
| `group_info` | `Option<GroupInfo>` | Group details if group message |
| `attachments` | `Vec<Attachment>` | File attachments |
| `quote` | `Option<Quote>` | Quoted/replied message |
| `reaction` | `Option<Reaction>` | Emoji reaction |
| `mentions` | `Vec<Mention>` | @mentions in message |

### SendParams (outgoing message)

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | `Vec<String>` | Recipient phone numbers |
| `group_id` | `Vec<String>` | Group IDs (base64) |
| `message` | `Option<String>` | Text content |
| `attachments` | `Vec<String>` | File paths to attach |
| `account` | `Option<String>` | Account (multi-account mode) |
| `quote_timestamp` | `Option<u64>` | Reply to message timestamp |
| `quote_author` | `Option<String>` | Reply to message author |

### SendResult

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | `u64` | Sent message timestamp |
| `results` | `Vec<RecipientResult>` | Per-recipient delivery status |

### ProcessConfig (daemon spawning)

| Field | Type | Description |
|-------|------|-------------|
| `jar_path` | `PathBuf` | Path to signal-cli.jar |
| `account` | `String` | Signal phone number |
| `http_addr` | `String` | HTTP bind address (default: "127.0.0.1:8080") |
| `config_dir` | `Option<PathBuf>` | signal-cli data directory |
| `send_read_receipts` | `bool` | Auto-send read receipts (default: true) |
| `trust_new_identities` | `bool` | Trust on first use (default: true) |

### DaemonError variants

| Variant | Description |
|---------|-------------|
| `Http(reqwest::Error)` | HTTP request failed |
| `Json(serde_json::Error)` | JSON parsing failed |
| `Rpc { code, message }` | JSON-RPC error from daemon |
| `Connection(String)` | Connection failed |
| `HealthCheckFailed` | Daemon not responding |
| `Sse(String)` | SSE stream error |
| `Config(String)` | Invalid configuration |
| `SendFailed(String)` | Message send failed |

## Daemon Endpoints

The client communicates with these signal-cli daemon HTTP endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/rpc` | POST | JSON-RPC 2.0 commands |
| `/api/v1/events` | GET | SSE message stream |
| `/api/v1/check` | GET | Health check |

## Dependencies

```toml
[dependencies]
signal-daemon = { path = "../signal-daemon" }
tokio = { version = "1", features = ["rt", "macros"] }
futures = "0.3"
```

## Prerequisites

### Option 1: Spawn from JAR (recommended)

Build the JAR first:
```bash
./scripts/build-signal-cli.sh
```

Then use `spawn_and_connect()` to start the daemon programmatically.

### Option 2: External daemon

Start signal-cli daemon manually:
```bash
java -jar build/signal-cli.jar -a +1234567890 daemon --http=127.0.0.1:8080
```

Then use `SignalClient::connect()` to connect.

## Error Handling Pattern

```rust
use signal_daemon::DaemonError;

match client.send_text(recipient, message).await {
    Ok(result) => println!("Sent at {}", result.timestamp),
    Err(DaemonError::Rpc { code, message }) => {
        // Signal-cli returned an error (e.g., invalid recipient)
        eprintln!("RPC error {}: {}", code, message);
    }
    Err(DaemonError::Http(e)) => {
        // Network/connection issue
        eprintln!("HTTP error: {}", e);
    }
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Common Patterns

### Echo Bot

```rust
use signal_daemon::{DaemonConfig, SignalClient, subscribe};
use futures::StreamExt;

let client = SignalClient::connect(DaemonConfig::default()).await?;
let mut stream = subscribe(&client);

while let Some(Ok(envelope)) = stream.next().await {
    if let Some(msg) = &envelope.data_message {
        if let Some(text) = &msg.message {
            // Echo back to sender
            client.send_text(&envelope.source, text).await?;
        }
    }
}
```

### Filtered Message Handler

```rust
while let Some(Ok(envelope)) = stream.next().await {
    // Skip non-text messages
    let Some(msg) = &envelope.data_message else { continue };
    let Some(text) = &msg.message else { continue };

    // Skip group messages
    if msg.group_info.is_some() { continue }

    // Process direct messages only
    handle_message(&envelope.source, text).await;
}
```

### Concurrent Send and Receive

```rust
use tokio::select;

let client = SignalClient::connect(DaemonConfig::default()).await?;
let mut stream = subscribe(&client);
let mut rx = message_queue_receiver();

loop {
    select! {
        Some(Ok(envelope)) = stream.next() => {
            // Handle incoming
        }
        Some(outgoing) = rx.recv() => {
            client.send_text(&outgoing.to, &outgoing.text).await?;
        }
    }
}
```

### Managed Daemon Lifecycle

```rust
use signal_daemon::{ProcessConfig, spawn_and_connect, subscribe};
use std::time::Duration;
use futures::StreamExt;

// Spawn daemon
let config = ProcessConfig::new("build/signal-cli.jar", "+1234567890");
let (mut process, client) = spawn_and_connect(config, Duration::from_secs(30)).await?;

// Echo bot
let mut stream = subscribe(&client);
while let Some(Ok(envelope)) = stream.next().await {
    if let Some(msg) = &envelope.data_message {
        if let Some(text) = &msg.message {
            client.send_text(&envelope.source, text).await?;
        }
    }
}

// Daemon killed automatically when process drops
```

## Examples

Run the included examples to test your setup:

```bash
cd crates/signal-daemon

# Health check (spawns daemon automatically)
AMAN_NUMBER=+1234567890 cargo run --example health_check

# Echo bot (replies to incoming messages)
AMAN_NUMBER=+1234567890 cargo run --example echo_bot

# Connect to existing daemon (without spawning)
cargo run --example health_check
```

Environment variables for examples:
- `AMAN_NUMBER` - If set, spawns daemon automatically
- `SIGNAL_CLI_JAR` - JAR path (default: `../../build/signal-cli.jar`)

## Testing

### Unit Tests (no daemon required)

```bash
cargo test --test integration_tests -- --skip daemon --skip spawn
```

Tests configuration, send params, reconnect logic without external dependencies.

### Integration Tests (require daemon)

Start daemon first, then run tests:

```bash
# Terminal 1: Start daemon
./scripts/run-signal-daemon.sh +1234567890

# Terminal 2: Run integration tests
cd crates/signal-daemon
cargo test --test integration_tests -- --ignored
```

### Full Test Suite

```bash
# Run all non-ignored tests
cargo test

# Run all tests including integration tests (requires daemon)
cargo test -- --include-ignored
```

### Test Environment Variables

| Variable | Description |
|----------|-------------|
| `AMAN_NUMBER` | Account for spawn tests |
| `SIGNAL_CLI_JAR` | JAR path for spawn tests |
| `TEST_RECIPIENT` | Phone number for send tests |
