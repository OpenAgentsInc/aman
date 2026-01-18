# grok-brain

xAI Grok-based brain implementation for the Aman Signal bot.

## Overview

This crate provides a `Brain` implementation and a `ToolExecutor` that use the
[xAI Grok API](https://docs.x.ai) for AI-powered message processing. It features:

- **Grok 4.1 Fast** model for quick, intelligent responses
- Per-sender conversation history
- Optional **X Search** for real-time Twitter/X data
- Optional **Web Search** for current web information
- Fully configurable via environment variables
- `GrokToolExecutor` for privacy-preserving tool calls from MapleBrain
- Routing metadata support (per-request model overrides + prompt hashing)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
grok-brain = { path = "../grok-brain" }
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `GROK_API_KEY` | ✅ | - | xAI API key for authentication |
| `GROK_API_URL` | | `https://api.x.ai` | xAI API base URL |
| `GROK_MODEL` | | `grok-4-1-fast` | Model to use |
| `GROK_SYSTEM_PROMPT` | | - | System prompt for the AI |
| `GROK_PROMPT_FILE` | | `SYSTEM_PROMPT.md` | Path to system prompt file |
| `GROK_MAX_TOKENS` | | `1024` | Maximum response tokens |
| `GROK_TEMPERATURE` | | `0.7` | Generation temperature (0.0-2.0) |
| `GROK_MAX_HISTORY_TURNS` | | `10` | Max conversation turns to keep |
| `GROK_ENABLE_X_SEARCH` | | `false` | Enable X Search tool |
| `GROK_ENABLE_WEB_SEARCH` | | `false` | Enable Web Search tool |
| `GROK_MEMORY_PROMPT_MAX_CHARS` | | `1800` | Max memory prompt characters (0 disables) |
| `GROK_MEMORY_PROMPT_MAX_TOKENS` | | - | Approximate token cap (converted to chars) |

### Example `.env`

```bash
GROK_API_KEY=xai-your-api-key-here
GROK_SYSTEM_PROMPT="You are a helpful assistant that provides concise, accurate responses."
GROK_PROMPT_FILE=SYSTEM_PROMPT.md
GROK_ENABLE_X_SEARCH=true
GROK_ENABLE_WEB_SEARCH=true
```

System prompt priority:
1. `GROK_SYSTEM_PROMPT` (if set)
2. `GROK_PROMPT_FILE` contents (if file exists)
3. No system prompt

## Usage

### Basic Usage

```rust
use grok_brain::{GrokBrain, Brain, InboundMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create brain from environment variables
    let brain = GrokBrain::from_env().await?;
    
    // Process a message
    let message = InboundMessage::direct(
        "+1234567890",
        "What's happening on Twitter right now?",
        0,
    );
    
    let response = brain.process(message).await?;
    println!("Response: {}", response.text);
    
    Ok(())
}
```

### With Builder Pattern

```rust
use grok_brain::{GrokBrain, GrokBrainConfig};

let config = GrokBrainConfig::builder()
    .api_key("xai-your-api-key")
    .model("grok-4-1-fast")
    .system_prompt("You are a news analyst specializing in breaking events.")
    .enable_x_search(true)
    .enable_web_search(true)
    .max_history_turns(5)
    .build();

let brain = GrokBrain::new(config)?;
```

### Tool Executor (for MapleBrain)

```rust
use grok_brain::GrokToolExecutor;
use maple_brain::{MapleBrain, MapleBrainConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grok_executor = GrokToolExecutor::from_env()?;
    let maple_config = MapleBrainConfig::from_env()?;
    let brain = MapleBrain::with_tools(maple_config, grok_executor).await?;
    println!("MapleBrain tools enabled: {}", brain.has_tools());
    Ok(())
}
```

### Routing metadata

GrokBrain respects `InboundMessage.routing.model_override` when present, allowing
per-request model selection from the orchestrator or other callers.

If `InboundMessage.routing.memory_prompt` is present, GrokBrain injects it as a
system message (before history) and caps it via `GROK_MEMORY_PROMPT_MAX_CHARS`.

## Real-Time Search Tools

### X Search ($5/invocation)

When enabled, the model can search Twitter/X for real-time information:

- Breaking news and events
- Trending topics
- User posts and discussions
- Real-time sentiment

### Web Search ($5/invocation)

When enabled, the model can search the web for current information:

- Latest news articles
- Current events
- Up-to-date facts and figures

> **Note**: Search tools incur additional costs. Use them judiciously.

## Supported Models

| Model | Description | Best For |
|-------|-------------|----------|
| `grok-4-1-fast` | Fast, efficient responses | **Recommended for tools** |
| `grok-4-fast` | Balanced speed/quality | General use |
| `grok-4` | Highest quality | Complex tasks |

## API Reference

### GrokBrain

```rust
impl GrokBrain {
    /// Create from environment variables
    pub async fn from_env() -> Result<Self, BrainError>;
    
    /// Create with explicit config
    pub fn new(config: GrokBrainConfig) -> Result<Self, BrainError>;
    
    /// Clear history for a sender
    pub async fn clear_history(&self, sender: &str);
    
    /// Clear all conversation histories
    pub async fn clear_all_history(&self);
}
```

### Brain Trait

```rust
#[async_trait]
impl Brain for GrokBrain {
    /// Process an inbound message and return a response
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError>;
    
    /// Get the brain's name
    fn name(&self) -> &str;
}
```

### GrokToolExecutor

```rust
impl GrokToolExecutor {
    /// Build from env vars (GROK_*).
    pub fn from_env() -> Result<Self, BrainError>;
    /// Create with explicit config.
    pub fn new(config: GrokBrainConfig) -> Result<Self, BrainError>;
}
```

`GrokToolExecutor` implements `ToolExecutor` for the `realtime_search` tool.
The model crafts a sanitized query; the tool executor only sees that query.

## License

MIT
