# mock-brain

Mock brain implementations for testing Signal bot message processing.

## Overview

This crate provides:
- `Brain` trait - async interface for processing messages
- `InboundMessage` / `OutboundMessage` - message types
- Mock implementations: `EchoBrain`, `PrefixBrain`, `DelayedBrain`
- Optional signal-daemon integration

## Usage

### Basic

```rust
use mock_brain::{Brain, EchoBrain, InboundMessage};

#[tokio::main]
async fn main() -> Result<(), mock_brain::BrainError> {
    let brain = EchoBrain::with_prefix("Echo: ");

    let message = InboundMessage::direct("+15551234567", "Hello!", 1234567890);

    let response = brain.process(message).await?;
    println!("Response: {}", response.text);  // "Echo: Hello!"
    Ok(())
}
```

### With signal-daemon

Enable the `signal-daemon` feature:

```toml
[dependencies]
mock-brain = { path = "../mock-brain", features = ["signal-daemon"] }
```

```rust
use mock_brain::{Brain, EchoBrain, EnvelopeExt, send_response};
use signal_daemon::Envelope;

// Convert signal-daemon envelope to InboundMessage
if let Some(inbound) = envelope.to_inbound_message() {
    let response = brain.process(inbound).await?;
    send_response(&client, &response).await?;
}
```

## Brain Implementations

| Brain | Description |
|-------|-------------|
| `EchoBrain` | Echoes messages back, optionally with a prefix |
| `PrefixBrain` | Transforms messages with prefix/suffix |
| `DelayedBrain` | Wraps another brain with artificial delay |

## Implementing Custom Brain

```rust
use async_trait::async_trait;
use mock_brain::{Brain, BrainError, InboundMessage, OutboundMessage};

struct MyBrain;

#[async_trait]
impl Brain for MyBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        // Your logic here
        Ok(OutboundMessage::reply_to(&message, "Hello!"))
    }

    fn name(&self) -> &str {
        "MyBrain"
    }
}
```

## Build

```bash
# Build without signal-daemon integration
cargo build -p mock-brain

# Build with signal-daemon integration
cargo build -p mock-brain --features signal-daemon

# Run tests
cargo test -p mock-brain

# Run example (requires signal-daemon feature and running daemon)
cargo run -p mock-brain --example echo_with_signal --features signal-daemon
```
