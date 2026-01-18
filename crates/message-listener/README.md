# message-listener

## Responsibility

The message listener owns Signal inbound transport. It connects to the signal-cli daemon via the `signal-daemon` crate,
normalizes inbound messages into `InboundMessage` values (including attachment metadata), and can optionally run a
`MessageProcessor` to invoke a Brain implementation and send responses.

## Features

- **Brain Integration**: Process messages through any `Brain` implementation
- **Timeout Protection**: Configurable timeout prevents pipeline hangs from slow AI responses (default: 60s)
- **Graceful Shutdown**: Clean shutdown with `run_with_shutdown()` or `run_until_stopped()`
- **Attachment Support**: Processes messages with images; attachment-only messages supported
- **Typing Indicators**: Optional typing indicators during processing
- **Formatted Replies**: Forwards `OutboundMessage.styles` as Signal textStyle ranges when present
- **Auto-Reconnection**: Inherits SSE auto-reconnection from signal-daemon

## Public Interfaces

Consumes:

- SSE stream from signal-cli daemon (via `signal-daemon`)

Produces:

- `InboundMessage` (brain-core)
- `OutboundMessage` (brain-core)

Processing:

- `MessageProcessor` calls a `Brain` implementation and sends replies via `signal-daemon`
- `MessageProcessor` resolves attachment paths using `DaemonConfig`

## Usage

### Basic MessageProcessor

```rust
use message_listener::{MessageProcessor, ProcessorConfig, EchoBrain};
use signal_daemon::{DaemonConfig, SignalClient};

let client = SignalClient::connect(DaemonConfig::default()).await?;
let brain = EchoBrain::default();
let config = ProcessorConfig::with_bot_number("+15551234567");

let processor = MessageProcessor::new(client, brain, config);
processor.run().await?;
```

### With Timeout Configuration

```rust
use std::time::Duration;

let config = ProcessorConfig {
    bot_number: Some("+15551234567".to_string()),
    brain_timeout: Duration::from_secs(30),  // 30 second timeout
    ..Default::default()
};
```

### Graceful Shutdown

```rust
// Using a custom shutdown signal
let shutdown = async {
    tokio::signal::ctrl_c().await.unwrap();
};
processor.run_with_shutdown(shutdown).await?;

// Or use the convenience method (requires "signal" feature)
processor.run_until_stopped().await?;
```

### Attachment-Only Messages

```rust
let config = ProcessorConfig {
    process_attachment_only: true,  // Default: true
    ..Default::default()
};
```

## Configuration

### ProcessorConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bot_number` | `Option<String>` | `None` | Bot's phone number (to ignore messages from self) |
| `process_groups` | `bool` | `true` | Whether to process group messages |
| `process_direct` | `bool` | `true` | Whether to process direct messages |
| `send_typing_indicators` | `bool` | `false` | Send typing indicators while processing |
| `brain_timeout` | `Duration` | `60s` | Timeout for brain processing |
| `process_attachment_only` | `bool` | `true` | Process messages with attachments but no text |

## How to Run

This crate is a library. Use it from a service binary or run the examples.

Examples (requires a running signal-cli daemon):

```bash
# Echo processor using mock-brain
cargo run -p message-listener --example processor_bot

# MapleBrain (OpenSecret) processor
export MAPLE_API_KEY="..."
cargo run -p message-listener --example maple_bot --features maple

# With graceful shutdown support
cargo run -p message-listener --example processor_bot --features signal
```

For daemon setup, see `docs/signal-cli-daemon.md`.

## How to Test

```bash
# Unit tests
cargo test -p message-listener

# With signal feature
cargo test -p message-listener --features signal
```

## Error Handling

### ProcessorError Variants

| Variant | Description |
|---------|-------------|
| `Daemon(DaemonError)` | Error from the signal daemon |
| `Brain(BrainError)` | Error from the brain during processing |
| `Timeout(Duration)` | Brain processing timed out |
| `StreamEnded` | The message stream ended unexpectedly |

## Failure Modes

- signal-cli daemon not running or unreachable (auto-reconnects)
- Brain processing timeout (returns `ProcessorError::Timeout`)
- Duplicate deliveries without dedupe persistence
- Attachments present but files missing or inaccessible
- MapleBrain config/attestation failures when using OpenSecret

## Security Notes

- Do not log raw message bodies or attachment file paths by default
- Protect signal-cli storage paths and credentials
- Brain timeout prevents resource exhaustion from slow AI responses
