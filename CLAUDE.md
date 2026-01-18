# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aman is a Signal-native chatbot that runs on a server using `signal-cli`. It receives incoming Signal messages, forwards them to AI backends (OpenSecret TEE, xAI Grok), and replies back to the sender on Signal.

**Current phase**: MVP - text and image chat, tool-augmented responses via Grok search, no web UI integration yet.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         SIGNAL-DAEMON                                │
│  SignalClient (JSON-RPC) + DaemonProcess (JAR spawn) + SSE Stream   │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       MESSAGE-LISTENER                               │
│  MessageListener (event stream) + MessageProcessor (Brain adapter)  │
│  Features: timeout, graceful shutdown, attachment-only support      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         ORCHESTRATOR                                 │
│  Router (classify) → RoutingPlan [actions] → Orchestrator (execute) │
│  Actions: search, use_tool, clear_context, respond, show_help       │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        AGENT-TOOLS                                   │
│  ToolRegistry → Calculator, Weather, WebFetch                        │
└─────────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│      MAPLE-BRAIN        │     │      GROK-BRAIN         │
│  OpenSecret TEE         │     │  GrokToolExecutor       │
│  + tool support         │◀────│  (realtime_search)      │
│  + vision support       │     │  + GrokBrain (chat)     │
│  + ConversationHistory  │     │  + ConversationHistory  │
└─────────────────────────┘     └─────────────────────────┘
              │                               │
              └───────────────┬───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         BRAIN-CORE                                   │
│  Brain trait + ToolExecutor + ConversationHistory + Message types   │
└─────────────────────────────────────────────────────────────────────┘
```

**Key processes**:
1. `signal-cli` daemon (account session + message receive stream via SSE/JSON-RPC)
2. Bot worker (message processing + API calls + sending)

This decoupling prevents slow API calls from blocking inbound message handling.

## Crates

Located in `crates/`:

| Crate | Description | Status |
|-------|-------------|--------|
| **signal-daemon** | Core client for signal-cli daemon (HTTP/SSE), process spawning, auto-reconnection | Production-ready |
| **message-listener** | Signal inbound transport with Brain integration, timeout, graceful shutdown | Production-ready |
| **brain-core** | Core traits (Brain, ToolExecutor) and shared types (ConversationHistory) | Stable |
| **maple-brain** | OpenSecret TEE-based AI with vision and tool support | Production-ready |
| **grok-brain** | xAI Grok for real-time search tools | Production-ready |
| **orchestrator** | Message routing, action coordination, multi-step processing | Production-ready |
| **agent-tools** | Tool registry with 8 tools: Calculator, Weather, WebFetch, Dictionary, WorldTime, BitcoinPrice, CryptoPrice, CurrencyConverter | Production-ready |
| **mock-brain** | Mock brain implementations for testing | Stable |
| **broadcaster** | Signal outbound delivery | Working |
| **agent-brain** | Onboarding, routing, and subscriptions | Stub |

## Quick Start (Development)

```bash
# 1. Build signal-cli JAR (required first)
./scripts/build-signal-cli.sh

# 2. Set up environment
cp .env.example .env
# Edit .env with your keys:
#   AMAN_NUMBER=+1234567890
#   MAPLE_API_KEY=your_opensecret_key
#   GROK_API_KEY=your_xai_key

# 3. Link Signal account (if not already done)
./scripts/link-device.sh "Dev Machine"
# Scan QR code with your phone

# 4. Run the bot
./scripts/dev.sh --build
```

## Build Commands

```bash
# Build signal-cli JAR (required first)
./scripts/build-signal-cli.sh

# Build individual crates
cd crates/signal-daemon && cargo build
cd crates/brain-core && cargo build
cd crates/maple-brain && cargo build
cd crates/grok-brain && cargo build
cd crates/orchestrator && cargo build
cd crates/message-listener && cargo build

# Run tests
cd crates/brain-core && cargo test
cd crates/signal-daemon && cargo test
cd crates/message-listener && cargo test

# Run examples
cd crates/signal-daemon
AMAN_NUMBER=+1234567890 cargo run --example echo_bot

cd crates/orchestrator
cargo run --example orchestrated_bot
```

## Scripts

Located in `scripts/`:

| Script | Description |
|--------|-------------|
| `dev.sh` | **Main development script** - builds and runs orchestrated_bot |
| `build-signal-cli.sh` | Build signal-cli fat JAR to `build/signal-cli.jar` |
| `signal-cli.sh` | General wrapper - pass any args to signal-cli |
| `register-signal.sh` | Register/re-register a Signal account |
| `link-device.sh` | Link as secondary device to existing account (recommended for dev) |
| `unlink-device.sh` | Remove local Signal data and unlink this device |
| `run-signal-daemon.sh` | Run signal-cli daemon for development |
| `send-message.sh` | Send a test message via daemon JSON-RPC |
| `copy-project-context.sh` | Copy project files to clipboard (for AI context) |
| `_common.sh` | Shared utilities (sourced by other scripts) |

### dev.sh Usage

```bash
./scripts/dev.sh              # Run bot (uses cached binary)
./scripts/dev.sh --build      # Force rebuild before running
./scripts/dev.sh --daemon     # Also start signal daemon in background
```

## signal-cli Setup

### Build

```bash
./scripts/build-signal-cli.sh
# Output: build/signal-cli.jar
```

### Account Setup

| Path | Description | Best For |
|------|-------------|----------|
| **Linking** (recommended) | Link as secondary device to your phone | Development |
| **Registration** | Register a new standalone account | Production |

**Link to existing account (recommended for dev):**

```bash
# Install qrencode
sudo apt install qrencode    # Debian/Ubuntu
brew install qrencode        # macOS

# Link with a device name
./scripts/link-device.sh "My Laptop"
```

Scan QR code with phone: Settings > Linked Devices > Link New Device.

### Run Daemon

```bash
./scripts/run-signal-daemon.sh +1234567890
# Or: AMAN_NUMBER=+1234567890 ./scripts/run-signal-daemon.sh
```

### Test Endpoints

```bash
# Health check
curl http://127.0.0.1:8080/api/v1/check

# Version
curl -X POST http://127.0.0.1:8080/api/v1/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"version","id":1}'

# Subscribe to events
curl -N http://127.0.0.1:8080/api/v1/events
```

## Configuration

Environment variables (via `.env`):

### Signal Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_NUMBER` | - | Signal phone number for the bot |
| `SIGNAL_CLI_JAR` | `build/signal-cli.jar` | Path to signal-cli.jar |
| `HTTP_ADDR` | `127.0.0.1:8080` | Daemon HTTP bind address |

### MapleBrain (OpenSecret) Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `MAPLE_API_KEY` | - | **Required** - OpenSecret API key |
| `MAPLE_API_URL` | `https://enclave.trymaple.ai` | API URL |
| `MAPLE_MODEL` | `llama-3.3-70b` | Text model |
| `MAPLE_VISION_MODEL` | `qwen3-vl-30b` | Vision model |
| `MAPLE_SYSTEM_PROMPT` | - | System prompt override |
| `MAPLE_PROMPT_FILE` | `PROMPT.md` | Path to system prompt file |
| `MAPLE_MAX_TOKENS` | `1024` | Max response tokens |
| `MAPLE_TEMPERATURE` | `0.7` | Generation temperature |
| `MAPLE_MAX_HISTORY_TURNS` | `10` | Conversation history length |
| `MAPLE_MAX_TOOL_ROUNDS` | `2` | Max tool execution rounds |

### GrokBrain (xAI) Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `GROK_API_KEY` | - | **Required** - xAI API key |
| `GROK_API_URL` | `https://api.x.ai` | API URL |
| `GROK_MODEL` | `grok-4-1-fast` | Model name |
| `GROK_ENABLE_WEB_SEARCH` | `false` | Enable web search |
| `GROK_ENABLE_X_SEARCH` | `false` | Enable X/Twitter search |

### Processor Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level |

## Key Features

### SSE Auto-Reconnection
The signal-daemon crate automatically reconnects on connection loss with exponential backoff:
- Default: unlimited retries with 500ms initial delay, 30s max delay
- Configurable via `ReconnectConfig`

### Brain Processing Timeout
MessageProcessor has a configurable timeout (default 60s) to prevent pipeline hangs from slow AI responses.

### Graceful Shutdown
MessageProcessor supports graceful shutdown via:
- `run_with_shutdown(signal)` - custom shutdown signal
- `run_until_stopped()` - Ctrl+C handling (requires `signal` feature)

### Attachment Support
Messages with images are processed by MapleBrain's vision model. Attachment-only messages (no text) are now supported.

### Conversation History
Shared `ConversationHistory` in brain-core provides per-sender history with automatic turn-based trimming.

## Testing

### Unit Tests

```bash
# Run all unit tests for key crates
cd crates/brain-core && cargo test
cd crates/signal-daemon && cargo test
cd crates/message-listener && cargo test
cd crates/maple-brain && cargo test
cd crates/grok-brain && cargo test
```

### Integration Tests

```bash
# signal-daemon integration tests (requires running daemon)
./scripts/run-signal-daemon.sh +YOUR_NUMBER
cd crates/signal-daemon && cargo test -- --ignored
```

### Manual Testing

Send messages from your phone to test:
- Text messages: "hello"
- Search queries: "what's happening in tech today?"
- Images: send a photo
- Reconnection: kill daemon, restart, send message
- Shutdown: Ctrl+C (should see clean shutdown)

## Examples

### Orchestrator (Recommended)

| Example | Description | Run Command |
|---------|-------------|-------------|
| `orchestrated_bot` | Full bot with routing and search | `cargo run -p orchestrator --example orchestrated_bot` |

### Signal Daemon

| Example | Description | Run Command |
|---------|-------------|-------------|
| `echo_bot` | Echo incoming messages | `cargo run -p signal-daemon --example echo_bot` |
| `health_check` | Check daemon health | `cargo run -p signal-daemon --example health_check` |

### Message Listener

| Example | Description | Run Command |
|---------|-------------|-------------|
| `processor_bot` | Echo bot using MessageProcessor | `cargo run -p message-listener --example processor_bot` |
| `maple_bot` | MapleBrain processor | `cargo run -p message-listener --example maple_bot --features maple` |

### MapleBrain

| Example | Description | Run Command |
|---------|-------------|-------------|
| `test_chat` | Test chat completion | `cargo run -p maple-brain --example test_chat` |
| `test_vision` | Test vision model | `cargo run -p maple-brain --example test_vision` |
| `test_with_grok` | Test with Grok tool executor | `cargo run -p maple-brain --example test_with_grok` |
| `list_models` | List available models | `cargo run -p maple-brain --example list_models` |

### GrokBrain

| Example | Description | Run Command |
|---------|-------------|-------------|
| `test_chat` | Test chat completion | `cargo run -p grok-brain --example test_chat` |
| `test_search` | Test real-time search | `cargo run -p grok-brain --example test_search` |

### Mock Brain

| Example | Description | Run Command |
|---------|-------------|-------------|
| `echo_with_signal` | Echo bot for testing | `cargo run -p mock-brain --example echo_with_signal` |

## Security Notes

- signal-cli stores account keys at `$HOME/.local/share/signal-cli/data/` - treat as secret material
- Signal E2EE terminates at the server; if server is compromised, message content is exposed
- Tool executors receive only sanitized queries (privacy boundary)
- Avoid logging message bodies or attachment paths

## Prompt Files

The bot's behavior is controlled by two prompt files at the project root:

| File | Purpose | Env Override |
|------|---------|--------------|
| `SYSTEM_PROMPT.md` | Main bot persona and response style | `MAPLE_SYSTEM_PROMPT` or `MAPLE_PROMPT_FILE` |
| `ROUTER_PROMPT.md` | Message classification and action routing | `ROUTER_SYSTEM_PROMPT` or `ROUTER_PROMPT_FILE` |

**Priority for each prompt:**
1. Environment variable (inline string)
2. Environment variable (file path)
3. Default file at project root
4. Embedded fallback (router only)

## Key Files

- `build/signal-cli.jar` - Built signal-cli fat JAR
- `repos/signal-cli/` - signal-cli Git submodule
- `bin/orchestrated_bot` - Built bot binary (created by dev.sh)
- `SYSTEM_PROMPT.md` - Main bot persona prompt
- `ROUTER_PROMPT.md` - Message routing prompt
- `docs/ARCHITECTURE.md` - Detailed architecture documentation
- `docs/signal-cli-daemon.md` - Daemon API documentation
