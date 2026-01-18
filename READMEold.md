# Aman (Signal AI Bot) - README

## What is Aman?

Aman is a Signal-native assistant and activist notification system.
It gives organizers, journalists, and human-rights defenders a safe, familiar way to ask questions, get guidance, and receive
opt-in regional alerts inside Signal.
You can use Aman for quick situational updates, communication support, and practical workflows without having to move to a
new platform.
The system is designed for minimal retention and respectful data handling so sensitive conversations stay focused and
operational.

### What this enables (MVP)

- Private, Signal-based chat for urgent questions and coordination.
- Opt-in regional alerts (e.g., outages, throttling, advisories) delivered directly to subscribers.
- A lightweight web UI for browser chat when a larger screen is useful.
- Operator tools for broadcast messages and monitoring.

### How it works (technical overview)

- A dedicated Signal account runs on a server using `signal-cli`.
- Inbound messages are decrypted locally and normalized by `message_listener`.
- An `agent_brain` service handles onboarding and routing decisions.
- Responses are sent back via `broadcaster`.
- Optional Brain backends include MapleBrain (OpenSecret) and GrokBrain (xAI).
- A separate web UI (`web/`) uses an OpenAI-compatible API for browser chat.
- Regional alerts are powered by a subscription state machine and event ingesters.

## Aman MVP

- Signal-native chat with a dedicated server-side account.
- Opt-in regional alerts with a simple state machine.
- Minimal storage for dedupe and short context.
- Text-first responses; MapleBrain can process image attachments and optionally call tools for real-time search.
- Web UI for browser chat (Next.js app in `web/`).
- Admin web panel for dashboards and broadcast (crate `admin-web`).

## Component overview

- `signal-daemon`: HTTP/SSE client for signal-cli daemon.
- `message_listener`: Signal inbound transport and message normalization.
- `agent_brain`: onboarding, subscriptions, and routing decisions.
- `broadcaster`: outbound Signal delivery, chunking, retries.
- `regional_event_listener`: regional event ingestion and normalization.
- `web`: Next.js UI for browser chat (separate from Signal flow).
- `api`: OpenAI-compatible inference gateway for local/dev web UI.
- `brain-core`: shared Brain trait and message types for AI backends.
- `maple-brain`: OpenSecret-backed Brain implementation (optional).
- `grok-brain`: xAI Grok Brain + tool executor for real-time search.
- `ingester`: file chunking + Nostr publishing for knowledge base content.
- `admin-web`: admin panel for dashboards and broadcast messaging.

## Message and event flow

Message flow:

1. Signal -> signal-cli daemon receives inbound message.
2. `message_listener` reads SSE events via `signal-daemon`.
3. `message_listener` emits normalized `InboundMessage`.
4. `agent_brain` decides: onboarding, chat response, or subscription update.
5. `broadcaster` sends reply via `signal-daemon` to signal-cli daemon.

Event flow:

1. `regional_event_listener` observes an event and normalizes to `RegionEvent`.
2. `agent_brain` queries subscriptions and creates alerts.
3. `broadcaster` delivers alerts to subscribed identities.

Web UI flow:

1. Browser -> Next.js app in `web/`.
2. `/api/chat` streams responses from the OpenAI-compatible API.

OpenAI-compatible API flow:

1. Web UI or client -> `api` service.
2. `api` returns OpenAI-style chat completions (stubbed echo for now).

Signal AI flow (optional):

1. Signal -> `message_listener` -> `MessageProcessor`.
2. `MessageProcessor` calls a `Brain` implementation (mock or MapleBrain).
3. Responses send back via `signal-daemon`.

Tool execution flow (optional):

1. MapleBrain receives a request that needs real-time info.
2. MapleBrain calls `realtime_search` with a sanitized query.
3. GrokToolExecutor fetches results from xAI and returns them.
4. MapleBrain synthesizes the final response.

Admin web flow:

1. Operator opens `admin-web` UI.
2. Dashboard reads stats from SQLite.
3. Broadcast sends a message to topic subscribers via `broadcaster`.

Nostr ingestion flow (local/dev):

1. `ingester` chunks files and writes chunk blobs to disk.
2. `ingester` publishes DocManifest + ChunkRef events (or indexes directly into SQLite).
3. `nostr-indexer` stores Nostr events in `NOSTR_DB_PATH`.
4. `api` reads from `NOSTR_DB_PATH` for knowledge base answers.

## Quickstart (dev)

See the runbook: `docs/AMAN_LOCAL_DEV.md`.
For the web UI, see `web/README.md`.

## Setup

### Prerequisites

- Java 21+ (for signal-cli)
- Rust toolchain (for crates)
- qrencode (for device linking QR codes): `sudo apt install qrencode` or `brew install qrencode`
- jq (for `scripts/send-message.sh`)
- Node.js 20+ (for the web UI in `web/`)
- A phone number for the bot's Signal account

Copy the example env file and edit values as needed:

```bash
cp .env.example .env
```

### 1. Build signal-cli

```bash
./scripts/build-signal-cli.sh
```

This builds a fat JAR at `build/signal-cli.jar`.
If the build script reports a missing submodule, run `git submodule update --init --recursive`.

### 2. Set up Signal account

There are two ways to set up a Signal account:

| Path | Description | Best For |
|------|-------------|----------|
| **Linking** (recommended) | Link as secondary device to your phone | Development, multi-machine setups |
| **Registration** | Register a new standalone account | Production, dedicated bot numbers |

#### Option A: Link to existing account (Recommended for development)

This is the **preferred approach for development** because:
- Multiple machines (laptops, servers) can link to the same account
- Your phone remains the primary device for easy management
- Easy to unlink/relink without losing the account

```bash
# Install qrencode (required for QR display)
sudo apt install qrencode    # Debian/Ubuntu
brew install qrencode        # macOS

# Link with a device name (required)
./scripts/link-device.sh "My Laptop"
./scripts/link-device.sh "Dev Server"
./scripts/link-device.sh "aman-prod"
```

The script displays a QR code directly in the terminal. Scan with your phone: **Settings > Linked Devices > Link New Device**

**Multi-machine setup:** Run `link-device.sh` on each machine with a unique device name. Each device appears in your phone's Linked Devices list.

#### Option B: Register new account (Production)

Register a dedicated phone number as a standalone Signal account:

```bash
# Request SMS verification code
./scripts/register-signal.sh +1234567890

# If captcha is required
./scripts/register-signal.sh +1234567890 --captcha

# For voice call instead of SMS
./scripts/register-signal.sh +1234567890 --voice
```

After receiving the code:

```bash
./scripts/signal-cli.sh -a +1234567890 verify <CODE>
```

**Note:** A registered account is tied to one machine. To use on multiple machines, you'd need to copy `~/.local/share/signal-cli/data/` (not recommended due to sync issues).

### 3. Run the daemon

```bash
./scripts/run-signal-daemon.sh +1234567890

# Or with environment variable
AMAN_NUMBER=+1234567890 ./scripts/run-signal-daemon.sh
```

Test endpoints:

```bash
# Health check
curl http://127.0.0.1:8080/api/v1/check

# Get version
curl -X POST http://127.0.0.1:8080/api/v1/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"version","id":1}'

# Subscribe to incoming messages
curl -N http://127.0.0.1:8080/api/v1/events
```

### 4. Run the web UI (optional)

```bash
cd web
cat <<'EOF' > .env.local
OPENAI_API_KEY=sk-...
EOF
npm install
npm run dev
```

To point the web UI at the local Aman API instead of a hosted provider:

```bash
cd web
cat <<'EOF' > .env.local
AMAN_API_BASE_URL=http://127.0.0.1:8787
AMAN_API_KEY=aman-local
AMAN_API_MODEL=aman-chat
EOF
npm run dev
```

If you want the local API to use a simple knowledge base, set `AMAN_KB_PATH` (for example `./knowledge`) when starting the api service.
To use the full Nostr flow, set `NOSTR_DB_PATH` and ingest documents via the `ingester` crate.

## Scripts

| Script | Description |
|--------|-------------|
| `scripts/build-signal-cli.sh` | Build signal-cli fat JAR to `build/signal-cli.jar` |
| `scripts/signal-cli.sh` | General wrapper - pass any args to signal-cli |
| `scripts/register-signal.sh` | Register/re-register a Signal account |
| `scripts/link-device.sh` | Link as secondary device to existing account (recommended for dev) |
| `scripts/unlink-device.sh` | Remove local Signal data and unlink this device |
| `scripts/run-signal-daemon.sh` | Run signal-cli daemon for development |
| `scripts/send-message.sh` | Send a test message via daemon JSON-RPC |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_NUMBER` | - | Bot's Signal phone number |
| `SIGNAL_CLI_JAR` | `build/signal-cli.jar` | Path to signal-cli JAR |
| `HTTP_ADDR` | `127.0.0.1:8080` | Daemon HTTP bind address |
| `SQLITE_PATH` | `./data/aman.db` | SQLite path for subscriptions/state |
| `AMAN_DEFAULT_LANGUAGE` | `English` | Default language label for new contacts |
| `SIGNAL_DAEMON_URL` | - | Override daemon base URL (optional) |
| `SIGNAL_DAEMON_ACCOUNT` | - | Account selector for multi-account daemon mode |
| `ADMIN_ADDR` | `127.0.0.1:8788` | Admin web bind address |
| `MAPLE_API_KEY` | - | OpenSecret API key (MapleBrain) |
| `MAPLE_API_URL` | `https://enclave.trymaple.ai` | Maple/OpenSecret API URL |
| `MAPLE_MODEL` | `llama-3.3-70b` | Text model for MapleBrain |
| `MAPLE_VISION_MODEL` | `qwen3-vl-30b` | Vision model for MapleBrain |
| `MAPLE_PROMPT_FILE` | `PROMPT.md` | System prompt file for MapleBrain |
| `GROK_API_KEY` | - | xAI API key (GrokBrain/GrokToolExecutor) |
| `GROK_API_URL` | `https://api.x.ai` | xAI API URL |
| `GROK_MODEL` | `grok-4-1-fast` | Grok model name |
| `GROK_ENABLE_X_SEARCH` | `false` | Enable X Search tool |
| `GROK_ENABLE_WEB_SEARCH` | `false` | Enable Web Search tool |

## Docs

- Architecture: `docs/ARCHITECTURE.md`
- Aman overview: `docs/AMAN.md`
- Data retention: `docs/DATA_RETENTION.md`
- signal-cli daemon guide: `docs/signal-cli-daemon.md`
- OpenSecret API: `docs/OPENSECRET_API.md`
- Grok brain: `crates/grok-brain/README.md`
- Roadmap: `ROADMAP.md`
- Admin web: `crates/admin-web/README.md`

## Crates

| Crate | Description |
|-------|-------------|
| `signal-daemon` | Core client for signal-cli daemon (HTTP/SSE) |
| `message-listener` | Signal inbound transport using signal-daemon |
| `broadcaster` | Signal outbound delivery using signal-daemon |
| `agent-brain` | Onboarding, routing, and API calls |
| `mock-brain` | Mock brain implementations for testing message flows |
| `brain-core` | Shared Brain trait + message types for AI backends |
| `maple-brain` | OpenSecret-backed Brain implementation |
| `grok-brain` | xAI Grok Brain + tool executor |
| `database` | SQLite persistence (users/topics/notifications) via SQLx |
| `api` | OpenAI-compatible chat API (local inference gateway) |
| `ingester` | Document chunking + Nostr publishing/indexing |
| `nostr-persistence` | Nostr publisher/indexer for durable doc/chunk metadata |
| `admin-web` | Admin dashboard + broadcast UI |

See individual READMEs in `crates/*/README.md` for API documentation.

## Safety and ops

- Opt-in alerts only; honor "stop" everywhere.
- Minimal retention and minimal logging.
- If your upstream provider supports retention controls (e.g., `store: false`), prefer disabling storage.
- Bind `admin-web` to localhost or put it behind authentication.

## Future work

- RAG pipeline with ingestion for documents and YouTube.
- Nostr relay persistence and local vector DB rehydration.
