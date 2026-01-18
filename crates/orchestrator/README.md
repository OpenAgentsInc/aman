# orchestrator

Message orchestrator for coordinating brain routing and tool execution.

## Responsibility

The orchestrator coordinates message processing between signal-daemon, maple-brain, and grok-brain. It:

1. Routes messages through a privacy-preserving classifier (maple-brain TEE)
2. Classifies message sensitivity (sensitive→Maple, insensitive→Grok)
3. Manages user preferences for agent selection
4. Executes multi-step action plans (search, clear context, respond)
5. Sends interim status messages to users
6. Maintains typing indicators throughout processing

## Architecture

```
Signal Message
       ↓
┌──────────────────────────────────────────────────────┐
│                    ORCHESTRATOR                       │
│                                                       │
│  1. Start typing indicator                           │
│         ↓                                            │
│  2. Route message (maple-brain, stateless)           │
│         ↓                                            │
│  3. Execute actions sequentially:                    │
│     • search → Call grok, send "Searching..." msg    │
│     • clear_context → Clear history, confirm         │
│     • respond → Final response via maple-brain       │
│         ↓                                            │
│  4. Stop typing indicator                            │
│         ↓                                            │
│  5. Return final response                            │
└──────────────────────────────────────────────────────┘
```

## Public Interface

### Core Types

- `Orchestrator<S: MessageSender>` - Main orchestrator struct
- `Router` - Message classifier using maple-brain
- `RoutingPlan` - List of actions to execute
- `OrchestratorAction` - Individual action (Search, ClearContext, Respond, Grok, Maple, etc.)
- `Sensitivity` - Message sensitivity level (Sensitive, Insensitive, Uncertain)
- `UserPreference` - User's preferred agent (Default, PreferPrivacy, PreferSpeed)
- `PreferenceStore` - Thread-safe storage for user preferences
- `AgentIndicator` - Response prefix indicator (Privacy, Speed)
- `Context` - Accumulated search results for augmenting responses
- `MessageSender` trait - Abstraction for sending messages

### Usage

```rust
use orchestrator::{Orchestrator, MessageSender, OrchestratorError, InboundMessage};
use async_trait::async_trait;

// Implement MessageSender for your transport
struct SignalSender { /* ... */ }

#[async_trait]
impl MessageSender for SignalSender {
    async fn send_message(&self, recipient: &str, text: &str, is_group: bool)
        -> Result<(), OrchestratorError> {
        // Send via Signal
        Ok(())
    }

    async fn set_typing(&self, recipient: &str, is_group: bool, started: bool)
        -> Result<(), OrchestratorError> {
        // Set typing indicator
        Ok(())
    }
}

// Create orchestrator from environment
let sender = SignalSender { /* ... */ };
let orchestrator = Orchestrator::from_env(sender).await?;

// Process a message
let message = InboundMessage::direct("+1234567890", "What's the weather?", 123);
let response = orchestrator.process(message).await?;
println!("Response: {}", response.text);
```

## Configuration

Environment variables (via `.env`):

| Variable | Required | Description |
|----------|----------|-------------|
| `MAPLE_API_KEY` | Yes | OpenSecret API key for routing and responses |
| `GROK_API_KEY` | Yes | xAI API key for real-time search |
| `MAPLE_API_URL` | No | OpenSecret API URL (default: `https://enclave.trymaple.ai`) |
| `GROK_API_URL` | No | xAI API URL (default: `https://api.x.ai`) |

### Router Prompt Configuration

The router uses a system prompt to classify messages. Configure it via:

| Variable | Default | Description |
|----------|---------|-------------|
| `ROUTER_SYSTEM_PROMPT` | - | Inline router prompt (overrides file) |
| `ROUTER_PROMPT_FILE` | `ROUTER_PROMPT.md` | Path to router prompt file |

**Priority:**
1. `ROUTER_SYSTEM_PROMPT` env var (if set)
2. Contents of prompt file
3. Embedded default prompt

Edit `ROUTER_PROMPT.md` at the project root to customize routing behavior without recompiling.

See the main `CLAUDE.md` for full configuration reference.

## Actions

The router classifies messages and generates action plans:

| Action | Description |
|--------|-------------|
| `Search { query, message }` | Execute real-time search via Grok, send status message |
| `ClearContext { message }` | Clear conversation history for sender |
| `Respond { sensitivity }` | Generate response, routed based on sensitivity and user preference |
| `Help` | Display help text |
| `Grok { query }` | Route directly to Grok (user explicitly requested) |
| `Maple { query }` | Route directly to Maple (user explicitly requested) |
| `SetPreference { preference }` | Change user's default agent preference |
| `Skip { reason }` | Skip processing with reason |
| `Ignore` | Silently ignore message (typos, accidental sends) |

## Sensitivity-Based Routing

The router classifies each message's sensitivity:

| Sensitivity | Behavior | Examples |
|-------------|----------|----------|
| `Sensitive` | Always uses Maple (TEE) | Health, finances, legal, personal info |
| `Insensitive` | Uses Grok (fast) by default | Weather, news, coding, general knowledge |
| `Uncertain` | Follows user preference (defaults to Maple) | Ambiguous context |

## User Preferences

Users can set their preferred agent:

| Command | Preference | Behavior |
|---------|------------|----------|
| "use grok", "prefer speed" | `PreferSpeed` | Uses Grok for insensitive and uncertain |
| "use maple", "prefer privacy" | `PreferPrivacy` | Always uses Maple |
| "reset preferences", "default" | `Default` | Sensitive→Maple, insensitive→Grok, uncertain→Maple |

Direct commands bypass normal routing:
- `grok: <query>` - Send directly to Grok
- `maple: <query>` - Send directly to Maple

Speed mode responses are prefixed with `[*]` as a subtle indicator.

## Example

Run the orchestrated bot example:

```bash
# Set required environment variables
export MAPLE_API_KEY="your_opensecret_key"
export GROK_API_KEY="your_xai_key"
export AMAN_NUMBER="+1234567890"

# Run the example
cargo run -p orchestrator --example orchestrated_bot

# Or use the dev script
./scripts/dev.sh --build
```

## Testing

```bash
# Unit tests
cargo test -p orchestrator

# Integration test (requires running daemon)
./scripts/run-signal-daemon.sh +YOUR_NUMBER
cargo run -p orchestrator --example orchestrated_bot
```

## Dependencies

- `brain-core` - Brain trait and message types
- `maple-brain` - OpenSecret TEE-based AI
- `grok-brain` - xAI Grok for search tools

## Security Notes

- All routing decisions happen inside the OpenSecret TEE (privacy-preserving)
- Sensitive messages are always processed in the TEE, never sent to external APIs
- Search queries are sanitized before being sent to Grok
- Raw user messages never leave the TEE for classification
- Users can opt into speed mode for non-sensitive queries
- Direct `grok:` commands bypass privacy protections (user's explicit choice)
