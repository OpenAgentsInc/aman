# maple-brain

A Brain implementation for the Aman Signal bot that uses the [Maple/OpenSecret](https://trymaple.ai) API for end-to-end encrypted AI interactions.

## Features

- End-to-end encrypted communication via OpenSecret's TEE (Trusted Execution Environment)
- Automatic attestation handshake for secure session establishment
- Per-sender conversation history management
- Streaming response support
- **Vision support** - Automatically processes image attachments using vision-language models
- **Tool calling support** - Optional real-time search via ToolExecutor (e.g., Grok)
- Configurable via environment variables

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
maple-brain = { path = "../maple-brain" }
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `MAPLE_API_KEY` | Yes | - | API key for authentication |
| `MAPLE_API_URL` | No | `https://enclave.trymaple.ai` | Maple API endpoint |
| `MAPLE_MODEL` | No | `llama-3.3-70b` | Model for text-only messages |
| `MAPLE_VISION_MODEL` | No | `qwen3-vl-30b` | Model for messages with images |
| `MAPLE_SYSTEM_PROMPT` | No | - | System prompt (overrides prompt file) |
| `MAPLE_PROMPT_FILE` | No | `PROMPT.md` | Path to system prompt file |
| `MAPLE_MAX_TOKENS` | No | `1024` | Maximum tokens in response |
| `MAPLE_TEMPERATURE` | No | `0.7` | Sampling temperature (0.0-2.0) |
| `MAPLE_MAX_HISTORY_TURNS` | No | `10` | Conversation history per sender |

### System Prompt File

You can define a system prompt in a `PROMPT.md` file instead of using an environment variable. This is useful for longer, more complex prompts.

**Priority order:**
1. `MAPLE_SYSTEM_PROMPT` environment variable (if set)
2. Contents of prompt file (default: `PROMPT.md` in current directory)
3. No system prompt

**Example PROMPT.md:**
```markdown
You are a helpful AI assistant communicating via Signal messenger.

Guidelines:
- Be concise and direct in your responses
- Use plain text formatting (Signal doesn't support rich markdown)
- If you're unsure about something, say so
```

### Available Models

- `llama-3.3-70b` (default) - Llama 3.3 70B
- `gemma-3-27b` - Gemma 3 27B
- `deepseek-r1-0528` - DeepSeek R1
- `deepseek-v31-terminus` - DeepSeek V3.1
- `gpt-oss-120b` - GPT OSS 120B
- `qwen3-coder-480b` - Qwen3 Coder 480B
- `qwen3-vl-30b` - Qwen3 VL 30B (vision-language)
- `kimi-k2-thinking` - Kimi K2 Thinking
- `nomic-embed-text` - Nomic Embed (for embeddings only)

## Usage

### Basic Usage

```rust
use maple_brain::{Brain, InboundMessage, MapleBrain};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Create brain from environment
    let brain = MapleBrain::from_env().await?;

    // Process a message
    let message = InboundMessage::direct(
        "+1234567890",
        "Hello, how are you?",
        1234567890,
    );

    let response = brain.process(message).await?;
    println!("Response: {}", response.text);

    Ok(())
}
```

### Programmatic Configuration

```rust
use maple_brain::{MapleBrain, MapleBrainConfig};

let config = MapleBrainConfig::new("your-api-key", "llama-3.3-70b")
    .with_system_prompt("You are a helpful assistant.")
    .with_max_tokens(2048)
    .with_temperature(0.5)
    .with_max_history_turns(20);

let brain = MapleBrain::new(config).await?;
```

### With Message Listener

```rust
use message_listener::{MapleBrain, MessageProcessor, ProcessorConfig};
use signal_daemon::{DaemonConfig, SignalClient};

let client = SignalClient::connect(DaemonConfig::default()).await?;
let brain = MapleBrain::from_env().await?;
let config = ProcessorConfig::with_bot_number("+1234567890");

let processor = MessageProcessor::new(client, brain, config);
processor.run().await?;
```

## Vision Support

MapleBrain automatically detects image attachments in incoming messages and uses the vision model to process them. When a message contains images:

1. The vision model (`qwen3-vl-30b` by default) is used instead of the text model
2. Images are read from disk and base64-encoded
3. The message is formatted using OpenAI's multimodal format
4. Conversation history is not included for vision requests (to avoid context complexity)

Supported image formats: JPEG, PNG, GIF, WebP (any format your Signal client can send)

## Tool Calling (Real-Time Search)

MapleBrain can be configured with a `ToolExecutor` to fetch real-time data
while keeping user messages private. The model crafts a sanitized query,
then the executor performs the external call and returns results for synthesis.

### MapleBrain + GrokToolExecutor

```rust
use grok_brain::GrokToolExecutor;
use maple_brain::{MapleBrain, MapleBrainConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grok_executor = GrokToolExecutor::from_env()?;
    let maple_config = MapleBrainConfig::from_env()?;
    let brain = MapleBrain::with_tools(maple_config, grok_executor).await?;
    println!("Tools enabled: {}", brain.has_tools());
    Ok(())
}
```

### Tool Definition

MapleBrain exposes a single tool definition by default:

- `realtime_search` (privacy-safe query + optional `search_type` of `web`, `social`, or `both`)

### Programmatic Vision Example

```rust
use maple_brain::{Brain, InboundMessage, InboundAttachment, MapleBrain};

let brain = MapleBrain::from_env().await?;

// Create a message with an image attachment
let mut message = InboundMessage::direct("+1234567890", "What's in this photo?", 12345);
message.attachments.push(InboundAttachment {
    content_type: "image/jpeg".to_string(),
    filename: Some("photo.jpg".to_string()),
    file_path: Some("/path/to/photo.jpg".to_string()),
    size: None,
    width: None,
    height: None,
    caption: None,
});

let response = brain.process(message).await?;
println!("Vision response: {}", response.text);
```

## Examples

### List Available Models

```bash
cargo run -p maple-brain --example list_models
```

### Test Chat Completion

```bash
cargo run -p maple-brain --example test_chat
```

### Test Vision

```bash
cargo run -p maple-brain --example test_vision
```

### MapleBrain with Grok Tools

```bash
cargo run -p maple-brain --example test_with_grok
```

## How It Works

1. **Attestation Handshake**: On initialization, the client performs an attestation handshake with the Maple enclave to verify it's running in a secure TEE and establish encrypted communication.

2. **Session Encryption**: All requests and responses are encrypted using keys derived from the attestation handshake. This ensures end-to-end encryption between your application and the AI model.

3. **Model Selection**: MapleBrain automatically selects the appropriate model:
   - Text-only messages → `MAPLE_MODEL` (default: `llama-3.3-70b`)
   - Messages with images → `MAPLE_VISION_MODEL` (default: `qwen3-vl-30b`)

4. **Streaming Responses**: The Maple API only supports streaming completions. MapleBrain collects the streamed chunks into a complete response.

5. **Tool Calls (optional)**: The model can request tools (e.g., `realtime_search`). MapleBrain executes the tools and feeds results back before final response.

6. **Conversation History**: Each sender has their own conversation history, enabling multi-turn conversations. History is automatically trimmed to the configured maximum turns. (Note: Vision requests don't include history.)

## Security

- All communication with the Maple API is end-to-end encrypted
- The API runs in a Trusted Execution Environment (TEE)
- Attestation verification ensures you're connecting to a genuine enclave
- API keys are used for authentication but never sent in plaintext

## License

MIT
