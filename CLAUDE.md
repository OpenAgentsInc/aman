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

- **broadcaster** - Signal-native broadcast utilities for sending messages via signal-cli
- **message-listener** - Signal-native message listener utilities for receiving messages from signal-cli

Both crates are currently stubs (edition 2021, no dependencies yet). There is no workspace-level Cargo.toml; each crate is built independently.

## Build Commands

```bash
# Build a specific crate
cargo build -p broadcaster
cargo build -p message-listener

# Run tests for a specific crate
cargo test -p broadcaster
cargo test -p message-listener

# Run a single test
cargo test -p <crate> <test_name>
```

## signal-cli Submodule

The `repos/signal-cli/` directory is a Git submodule containing the Java-based signal-cli tool.

```bash
# Initialize submodule after clone
git submodule update --init

# Build signal-cli (from repos/signal-cli/)
./gradlew build
./gradlew installDist
```

signal-cli provides CLI and daemon mode (JSON-RPC with SSE stream) for Signal integration. See `docs/signal-cli-daemon.md` for detailed daemon setup, JSON-RPC/D-Bus interfaces, and event subscription.

## Configuration

Environment variables (via `.env`):
- `OPENAI_API_KEY` - API key for OpenAI-compatible provider
- `AMAN_NUMBER` - Signal phone number for the bot
- `SIGNAL_CLI_PATH` - Path to signal-cli binary
- `MODEL` - Model to use (optional)
- `STORE_OPENAI_RESPONSES` - Set to "false" to disable API-side storage
- `SQLITE_PATH` - Local database path

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
