# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aman is a Signal-native chatbot that runs on a server using `signal-cli`. It receives incoming Signal messages, forwards them to an OpenAI-compatible API endpoint (Responses API), and replies back to the sender on Signal.

**Current phase**: MVP - text-only chat, no web UI, no document upload, no RAG.

## Architecture

```
Signal User <—E2EE—> signal-cli (server) -> Bot Worker -> OpenAI-compatible API -> signal-cli send reply
```

**Two long-lived processes**:
1. `signal-cli` runtime (account session + message receive stream via SSE/JSON-RPC)
2. Bot worker (queueing + API calls + sending)

This decoupling prevents slow API calls from blocking inbound message handling.

## Crates

Located in `crates/`:

| Crate | Description | Status |
|-------|-------------|--------|
| **signal-daemon** | Core client for signal-cli daemon (HTTP/SSE), process spawning | Working |
| **broadcaster** | Signal outbound delivery using signal-daemon | Working |
| **message-listener** | Signal inbound transport using signal-daemon | Working |
| **agent-brain** | Onboarding, routing, and API calls | Stub |

There is no workspace-level Cargo.toml; each crate is built independently.

## Build Commands

```bash
# Build signal-cli JAR (required first)
./scripts/build-signal-cli.sh

# Build a specific crate
cd crates/signal-daemon && cargo build
cd crates/broadcaster && cargo build
cd crates/message-listener && cargo build

# Run tests
cd crates/signal-daemon && cargo test

# Run examples
cd crates/signal-daemon
AMAN_NUMBER=+1234567890 cargo run --example health_check
AMAN_NUMBER=+1234567890 cargo run --example echo_bot
```

## Scripts

Located in `scripts/`:

| Script | Description |
|--------|-------------|
| `build-signal-cli.sh` | Build signal-cli fat JAR to `build/signal-cli.jar` |
| `signal-cli.sh` | General wrapper - pass any args to signal-cli |
| `register-signal.sh` | Register/re-register a Signal account |
| `link-device.sh` | Link as secondary device to existing account (recommended for dev) |
| `unlink-device.sh` | Remove local Signal data and unlink this device |
| `run-signal-daemon.sh` | Run signal-cli daemon for development |
| `send-message.sh` | Send a test message via daemon JSON-RPC |

## signal-cli Setup

### Build

```bash
./scripts/build-signal-cli.sh
# Output: build/signal-cli.jar
```

### Account Setup: Two Paths

There are two ways to set up a Signal account for signal-cli:

| Path | Description | Best For |
|------|-------------|----------|
| **Linking** (recommended) | Link as secondary device to your phone | Development, multi-machine setups |
| **Registration** | Register a new standalone account | Production, dedicated bot numbers |

**For development, use Linking.** This allows multiple machines (laptops, servers) to share your Signal account, and your phone remains the primary device for easy management.

### Path A: Link to Existing Account (Recommended for Dev)

Link this machine as a secondary device to your phone's Signal account:

```bash
# Install qrencode (required for QR display)
sudo apt install qrencode    # Debian/Ubuntu
brew install qrencode        # macOS

# Link with a device name (required)
./scripts/link-device.sh "My Laptop"
./scripts/link-device.sh "Dev Server"
./scripts/link-device.sh "aman-prod"
```

The script displays a QR code directly in the terminal. Scan with your phone: Settings > Linked Devices > Link New Device.

**Multi-machine setup:** Run `link-device.sh` on each machine with a unique device name. Each will appear in your phone's Linked Devices list.

### Path B: Register New Account (Production)

Register a dedicated phone number as a standalone Signal account:

```bash
# Request SMS verification
./scripts/register-signal.sh +1234567890

# If captcha required
./scripts/register-signal.sh +1234567890 --captcha

# Verify with code received via SMS
./scripts/signal-cli.sh -a +1234567890 verify <CODE>
```

**Note:** A registered account is tied to one device. To use on multiple machines, you'd need to copy `~/.local/share/signal-cli/data/` (not recommended due to sync issues).

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

| Variable | Description |
|----------|-------------|
| `AMAN_NUMBER` | Signal phone number for the bot |
| `SIGNAL_CLI_JAR` | Path to signal-cli.jar (default: `build/signal-cli.jar`) |
| `HTTP_ADDR` | Daemon HTTP bind address (default: `127.0.0.1:8080`) |
| `OPENAI_API_KEY` | API key for OpenAI-compatible provider |
| `MODEL` | Model to use (optional) |
| `STORE_OPENAI_RESPONSES` | Set to "false" to disable API-side storage |
| `SQLITE_PATH` | Local database path |

## Data Model (planned)

SQLite tables for:
- `contacts(sender_id, last_seen_at)` - sender tracking
- `messages(id, sender_id, ts, direction, body, status)` - message log
- `conversations(sender_id, summary, last_n_turns_json)` - context storage

## Security Notes

- signal-cli stores account keys at `$HOME/.local/share/signal-cli/data/` - treat as secret material
- Signal E2EE terminates at the server; if server is compromised, message content is exposed
- Use `store: false` with the API when you don't want application state retained
- Minimize logs, use short retention, implement per-sender throttles

## Key Files

- `build/signal-cli.jar` - Built signal-cli fat JAR
- `repos/signal-cli/` - signal-cli Git submodule
- `docs/signal-cli-daemon.md` - Daemon API documentation
- `crates/signal-daemon/README.md` - Rust client API reference
