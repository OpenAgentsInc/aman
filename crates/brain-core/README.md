# brain-core

## Responsibility

Defines the shared `Brain` trait, `ToolExecutor` trait, and common types used by Aman brain implementations.

## Public Interface

### Traits

- `Brain` - Async `process()` method for message handling + metadata methods
- `ToolExecutor` - Interface for executing external tools (e.g., real-time search)

### Types

- `InboundMessage` - Incoming message with sender, text, timestamp, group_id, attachments, routing metadata
- `InboundAttachment` - Attachment metadata (content type, filename, file path, size, dimensions)
- `OutboundMessage` - Reply container with recipient, text, and optional styles
- `TextStyle` - Formatting ranges for Signal text (bold/italic/monospace/strikethrough)
- `BrainError` - Common error types for brain implementations
- `ToolRequest` / `ToolResult` - Tool call input/output types (optional metadata)
- `ToolRequestMeta` - Optional sender/group metadata for tools
- `RoutingInfo` - Sensitivity/task hint/model override metadata for routing
- `Sensitivity` / `TaskHint` - Router hints for privacy + model selection
- `ConversationHistory` - Per-sender conversation history with automatic trimming
- `HistoryMessage` - Individual message in conversation history
- `hash_prompt` - Prompt fingerprint helper for reproducibility

## Usage

### Basic Brain Implementation

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

### ConversationHistory

```rust
use brain_core::ConversationHistory;

// Create history that keeps 5 turns per sender
let history = ConversationHistory::new(5);

// Add exchanges
history.add_exchange("+1234", "Hello", "Hi there!").await;
history.add_exchange("+1234", "How are you?", "I'm doing well!").await;

// Retrieve history for a sender
let messages = history.get("+1234").await;
assert_eq!(messages.len(), 4); // 2 turns = 4 messages

// Clear history for a sender
history.clear("+1234").await;

// Clear all history
history.clear_all().await;
```

### ToolExecutor

```rust
use brain_core::{async_trait, ToolExecutor, ToolRequest, ToolResult, BrainError};

struct MyToolExecutor;

#[async_trait]
impl ToolExecutor for MyToolExecutor {
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        match request.name.as_str() {
            "search" => {
                let query = match request.require_string("query") {
                    Ok(query) => query,
                    Err(err) => return ToolResult::error(&request.id, err),
                };
                // Execute search...
                ToolResult::success(&request.id, format!("Search results for {}", query))
            }
            _ => ToolResult::error(&request.id, "Unknown tool"),
        }
    }

    fn supported_tools(&self) -> Vec<String> {
        vec!["search".to_string()]
    }
}
```

## Notes

- This crate has no I/O; it is purely types + trait definitions
- Attachments are represented as metadata and file paths from signal-cli
- Tool executors receive sanitized queries crafted by the brain (privacy boundary)
- ConversationHistory uses `tokio::sync::RwLock` for thread-safe access

## Security Notes

- Treat attachment file paths as sensitive
- Avoid logging message contents by default
- Tool queries should be sanitized to not leak raw user messages
