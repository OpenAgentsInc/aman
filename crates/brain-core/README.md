# brain-core

## Responsibility

Defines the shared `Brain` trait and message types used by Aman brain implementations.

## Public interface

- `Brain` trait: async `process` + metadata methods.
- `InboundMessage`: sender, text, timestamp, group_id, attachments.
- `InboundAttachment`: attachment metadata (content type, filename, file path, size, dimensions).
- `OutboundMessage`: reply container with recipient and text.
- `BrainError`: common error types for brain implementations.
- `ToolExecutor`: optional interface for executing external tools (e.g., real-time search).
- `ToolRequest` / `ToolResult`: tool call input/output types.

## Usage

```rust
use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};

struct MyBrain;

#[async_trait]
impl Brain for MyBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        Ok(OutboundMessage::reply_to(&message, "Hello!"))
    }

    fn name(&self) -> &str {
        "MyBrain"
    }
}
```

## Notes

- This crate has no I/O; it is purely types + trait definitions.
- Attachments are represented as metadata and file paths from signal-cli.
- Tool executors receive sanitized queries crafted by the brain (privacy boundary).

## Security notes

- Treat attachment file paths as sensitive.
- Avoid logging message contents by default.
