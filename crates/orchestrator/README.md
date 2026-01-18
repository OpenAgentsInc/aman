# orchestrator

Message orchestrator for coordinating brain routing and tool execution.

## Responsibility

The orchestrator coordinates message processing between signal-daemon, maple-brain, and grok-brain. It:

1. Routes messages through a privacy-preserving classifier (maple-brain TEE)
2. Classifies message sensitivity and task hints (sensitive→Maple, insensitive→Grok)
3. Manages user preferences for agent selection
4. Executes multi-step action plans (search, use_tool, clear context, respond, help, direct routing)
5. Prompts for PII handling choices when personal data is detected
6. Sends interim status messages to users
7. Maintains typing indicators throughout processing
8. Formats responses with markdown-to-Signal styles and metadata footers
9. Persists preferences, summaries, and tool history in SQLite (when configured)
10. Hydrates durable memory snapshots into prompts with policy controls

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
│     • use_tool → Run agent-tools, add context        │
│     • clear_context → Clear history                  │
│     • ask_privacy_choice → Prompt for PII handling   │
│     • respond → Final response via Maple/Grok + footer │
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
- `TaskHint` - Task type for model selection (General, Coding, Math, Creative, Multilingual, Quick, Vision, AboutBot)
- `UserPreference` - User's preferred agent (Default, PreferPrivacy, PreferSpeed)
- `PreferenceStore` - Thread-safe storage for user preferences
- `MemoryStore` - SQLite-backed rolling summaries and tool history
- `MemorySettings` - Summary + retention tuning for durable memory
- `RetentionPolicy` / `SummaryPolicy` - Memory retention and formatting controls
- `MemorySnapshot` - Durable memory payload used for prompt hydration
- `MemoryPromptPolicy` - Memory prompt formatting and PII policy controls
- `ModelSelector` - Selects optimal model based on task hint
- `MapleModels` / `GrokModels` - Model configurations per provider
- `AgentIndicator` - Response prefix indicator (Privacy, Speed)
- `Context` - Accumulated search results for augmenting responses
- `ToolRegistry` - Registry of orchestrator-level tools (agent-tools)
- `MessageSender` trait - Abstraction for sending messages (supports styled text)

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
| `SQLITE_PATH` | No | SQLite path or URL for durable preferences + memory |

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

### Memory and retention (optional)

Durable memory is enabled when `SQLITE_PATH` is set. Tune summary and retention via:

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_MEMORY_SUMMARY_MAX_ENTRIES` | `8` | Max exchanges to keep in rolling summary |
| `AMAN_MEMORY_SUMMARY_MAX_ENTRY_CHARS` | `160` | Max chars per summary line |
| `AMAN_MEMORY_SUMMARY_MAX_CHARS` | `1200` | Max total summary length |
| `AMAN_MEMORY_TOOL_OUTPUT_MAX_CHARS` | `2000` | Max chars stored per tool output |
| `AMAN_MEMORY_SUMMARY_TTL_DAYS` | `30` | Summary TTL in days (0 disables) |
| `AMAN_MEMORY_TOOL_TTL_DAYS` | `14` | Tool history TTL in days (0 disables) |
| `AMAN_MEMORY_CLEAR_TTL_DAYS` | `30` | Clear-context TTL in days (0 disables) |
| `AMAN_MEMORY_MAX_SUMMARIES` | `5000` | Max summary rows (0 disables) |
| `AMAN_MEMORY_MAX_TOOL_HISTORY` | `10000` | Max tool history rows (0 disables) |
| `AMAN_MEMORY_MAX_TOOL_HISTORY_PER_KEY` | `200` | Max tool rows per sender/group (0 disables) |
| `AMAN_MEMORY_MAX_CLEAR_EVENTS` | `5000` | Max clear-context rows (0 disables) |
| `AMAN_MEMORY_COMPACT_INTERVAL_SECS` | - | Background compaction interval in seconds (0 disables) |

Memory prompt policy (optional):

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_MEMORY_PROMPT_MAX_CHARS` | `1800` | Max characters for the injected memory prompt (0 disables) |
| `AMAN_MEMORY_PROMPT_MAX_TOKENS` | - | Approximate token cap (converted to chars, 4 chars/token) |
| `AMAN_MEMORY_PROMPT_MAX_SUMMARY_CHARS` | `1000` | Max summary characters included in the prompt |
| `AMAN_MEMORY_PROMPT_MAX_TOOL_ENTRIES` | `3` | Max tool history entries included |
| `AMAN_MEMORY_PROMPT_MAX_TOOL_ENTRY_CHARS` | `280` | Max characters per tool entry |
| `AMAN_MEMORY_PROMPT_MAX_CLEAR_EVENTS` | `2` | Max clear-context events included |
| `AMAN_MEMORY_PROMPT_INCLUDE_SUMMARY` | `true` | Include summary section in memory prompt |
| `AMAN_MEMORY_PROMPT_INCLUDE_TOOL_HISTORY` | `true` | Include tool history section |
| `AMAN_MEMORY_PROMPT_INCLUDE_CLEAR_CONTEXT` | `true` | Include clear-context section |
| `AMAN_MEMORY_PROMPT_PII_POLICY` | `allow` | PII handling policy (`allow`, `redact`, `skip`) |
| `AMAN_MEMORY_PROMPT_OVERRIDES` | - | JSON map of per-history overrides |

When enabled, the orchestrator formats a standardized memory block (summary first, tool history
after) and attaches it to routing metadata. Maple/Grok inject it as a system message and refresh
their cached memory prompt per request, with provider-specific size caps.

### Nostr memory publishing (optional)

Enable Nostr publishing by building with the `nostr` feature. When `NOSTR_RELAYS` and
`NOSTR_SECRET_KEY` are set, the orchestrator publishes preferences, summaries, tool history, and
clear-context events to Nostr. If `NOSTR_SECRETBOX_KEY` is set, payloads are encrypted.

```bash
cargo run -p orchestrator --example orchestrated_bot --features nostr
```

## Actions

The router classifies messages and generates action plans:

| Action | Description |
|--------|-------------|
| `Search { query, message }` | Execute real-time search via Grok, send status message |
| `ClearContext { message }` | Clear conversation history for sender |
| `Respond { sensitivity, has_pii, pii_types }` | Generate response, routed based on sensitivity and user preference (PII triggers privacy prompt) |
| `Help` | Display help text |
| `Grok { query }` | Route directly to Grok (user explicitly requested) |
| `Maple { query }` | Route directly to Maple (user explicitly requested) |
| `SetPreference { preference }` | Change user's default agent preference |
| `UseTool { name, args, message }` | Execute an `agent-tools` capability and add output to context |
| `AskPrivacyChoice { pii_types, original_message }` | Prompt user to choose sanitize/private/cancel when PII is detected |
| `PrivacyChoiceResponse { choice }` | Handle the user's response to a privacy choice prompt |
| `Skip { reason }` | Skip processing with reason |
| `Ignore` | Silently ignore message (typos, accidental sends) |

Note: Privacy choice responses are currently acknowledged but full sanitize/private routing is still in progress.

## Built-in Tools

The orchestrator uses the default `agent-tools` registry by default:

- `calculator` - Safe math expression evaluation
- `weather` - Current weather via wttr.in
- `web_fetch` - Fetch and optionally summarize URL content
- `dictionary` - Word definitions via Free Dictionary API
- `world_time` - Timezone lookup via WorldTimeAPI
- `bitcoin_price` - BTC price via mempool.space
- `crypto_price` - Crypto prices via CoinGecko
- `currency_converter` - Fiat conversion via exchangerate.host
- `unit_converter` - Unit conversions (length, temp, weight, data, etc.)
- `random_number` - Random numbers/dice/coin flips
- `sanitize` - PII redaction using a Maple-backed sanitizer

## Sensitivity-Based Routing

The router classifies each message's sensitivity:

| Sensitivity | Behavior | Examples |
|-------------|----------|----------|
| `Sensitive` | Always uses Maple (TEE) | Health, finances, legal, personal info |
| `Insensitive` | Uses Grok (fast) by default | Weather, news, coding, general knowledge |
| `Uncertain` | Follows user preference (defaults to Maple) | Ambiguous context |

If PII is detected, the router can request an explicit privacy choice before responding.

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

Direct Grok responses are prefixed with `[*]` as a subtle indicator.

Preferences are stored in memory by default; set `SQLITE_PATH` to persist across restarts.

## Response Formatting

`Respond` actions are formatted before delivery:

- Markdown markers are converted into Signal text styles (bold/italic/monospace/strikethrough).
- A footer is appended with the mode label, selected model, and tools used.
- Styled ranges are attached via `OutboundMessage.styles`; transports can render or ignore them.

## Task-Based Model Selection

The router classifies each message's task type to select the optimal model:

| Task Hint | Description | Maple Model | Grok Model |
|-----------|-------------|-------------|------------|
| `General` | Standard conversations | llama-3.3-70b | grok-4-1-fast |
| `Coding` | Programming tasks | deepseek-r1-0528 | grok-3 |
| `Math` | Mathematical reasoning | deepseek-r1-0528 | grok-4 |
| `Creative` | Creative writing | gpt-oss-120b | grok-4 |
| `Multilingual` | Non-English/translation | qwen2-5-72b | grok-4-1-fast |
| `Quick` | Simple, fast queries | mistral-small-3-1-24b | grok-3-mini |
| `Vision` | Image/visual analysis | qwen3-vl-30b | N/A (Maple only) |
| `AboutBot` | Questions about Aman itself | llama-3.3-70b | grok-4-1-fast |

### Model Configuration

Override default models via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `MAPLE_MODEL` | llama-3.3-70b | Default Maple model |
| `MAPLE_MODEL_CODING` | deepseek-r1-0528 | Maple coding model |
| `MAPLE_MODEL_MATH` | deepseek-r1-0528 | Maple math model |
| `MAPLE_MODEL_CREATIVE` | gpt-oss-120b | Maple creative model |
| `MAPLE_MODEL_MULTILINGUAL` | qwen2-5-72b | Maple multilingual model |
| `MAPLE_MODEL_QUICK` | mistral-small-3-1-24b | Maple quick model |
| `MAPLE_VISION_MODEL` | qwen3-vl-30b | Maple vision model |
| `GROK_MODEL` | grok-4-1-fast | Default Grok model |
| `GROK_MODEL_CODING` | grok-3 | Grok coding model |
| `GROK_MODEL_MATH` | grok-4 | Grok math model |
| `GROK_MODEL_CREATIVE` | grok-4 | Grok creative model |
| `GROK_MODEL_MULTILINGUAL` | grok-4-1-fast | Grok multilingual model |
| `GROK_MODEL_QUICK` | grok-3-mini | Grok quick model |

The model selector applies per-request overrides via routing metadata; Maple/Grok respect those overrides.

## Attachment and Image Handling

The router is aware of message attachments and handles them appropriately:

### Image Detection

When a message includes image attachments:
1. The router is informed via the `[ATTACHMENTS: ...]` metadata line (e.g., `1 image (jpeg, 1024x768)`)
2. The router sets `task_hint` to `Vision` for image-related requests
3. Vision tasks are **always** routed to Maple (Grok has no vision support)

### Routing Behavior

| Attachment Type | Task Hint | Route | Notes |
|-----------------|-----------|-------|-------|
| Images | `Vision` | Maple only | Grok lacks vision support |
| Image + sensitive topic | `Vision` | Maple | Double protection |
| No attachments | Based on content | Normal routing | Standard sensitivity/task routing |
| Other files (PDF, etc.) | `General` | Normal routing | Not fully supported yet |

### Image-Only Messages

Messages with images but no text are treated as "What is this?" or "Describe this image" requests.

### Explicit Grok with Images

If a user explicitly requests `grok: <query>` but includes an image, the orchestrator automatically falls back to Maple with a warning logged.

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
- Image attachments are always processed in Maple (TEE) for privacy - Grok has no access to user images
