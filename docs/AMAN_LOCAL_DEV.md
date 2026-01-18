# Aman Local Dev Runbook

This runbook is for a local MVP setup using `signal-cli` and the Aman services.

Scope: this runbook covers the Signal MVP only. RAG and Nostr components are planned and documented in
`ROADMAP.md`.

## 1) Prereqs

- Java 21+ for `signal-cli`.
- A Signal phone number for Aman (SMS or voice verification).
- An OpenAI-compatible API key (if using `agent_brain` with a hosted model).
- A Maple/OpenSecret API key (if using MapleBrain).
- An xAI API key (if using GrokBrain or GrokToolExecutor).
- Rust toolchain (for crates and examples).
- qrencode (for linking a device via QR): `sudo apt install qrencode` or `brew install qrencode`.
- jq (for `scripts/send-message.sh`).
- Node.js 20+ (for the web UI in `web/`).

## 2) Environment

Create a local `.env` (scripts load it automatically). Use the example file:

```bash
cp .env.example .env
```

Example values:

```bash
export AMAN_NUMBER="+15551234567"
export HTTP_ADDR="127.0.0.1:8080"
# Optional override for daemon URL:
# export SIGNAL_DAEMON_URL="http://127.0.0.1:8080"
# Optional for multi-account daemon mode:
# export SIGNAL_DAEMON_ACCOUNT="+15551234567"
# Optional override:
# export SIGNAL_CLI_JAR="build/signal-cli.jar"
export SQLITE_PATH="./data/aman.db"
export AMAN_DEFAULT_LANGUAGE="English"
export OPENAI_API_KEY="..."   # optional (OpenAI-compatible provider)
export MODEL="gpt-5"           # optional
export STORE_OPENAI_RESPONSES="false"
export MAPLE_API_KEY="..."     # required for MapleBrain
export MAPLE_MODEL="hugging-quants/Meta-Llama-3.1-70B-Instruct-AWQ-INT4"
export MAPLE_VISION_MODEL="qwen3-vl-30b"
export MAPLE_API_URL="https://enclave.trymaple.ai"
export MAPLE_MAX_HISTORY_TURNS="10"
export MAPLE_PROMPT_FILE="crates/maple-brain/PROMPT.md"
export GROK_API_KEY="..."      # required for GrokBrain/GrokToolExecutor
export GROK_MODEL="grok-4-1-fast"
export GROK_ENABLE_X_SEARCH="false"
export GROK_ENABLE_WEB_SEARCH="false"
export REGION_POLL_INTERVAL_SECONDS="60"
export LOG_LEVEL="info"
export AMAN_API_ADDR="127.0.0.1:8787"
export AMAN_API_TOKEN="aman-local"
export AMAN_API_MODEL="aman-chat"
export ADMIN_ADDR="127.0.0.1:8788"
```

See `.env.example` for the full set of Maple/Grok configuration knobs.

## 3) Fetch and build signal-cli

The build script expects the `repos/signal-cli` submodule to be present:

```bash
git submodule update --init --recursive
./scripts/build-signal-cli.sh
```

## 4) Set up Aman's Signal account

Choose one path:

### Option A: Link as a secondary device (recommended for dev)

Link this machine to your phone's Signal account:

```bash
./scripts/link-device.sh "My Laptop"
./scripts/link-device.sh "Dev Server"
```

Scan the QR code in Signal: Settings > Linked Devices > Link New Device.

### Option B: Register a dedicated account (production)

```bash
./scripts/register-signal.sh "$AMAN_NUMBER"
```

If SMS fails, retry with voice verification:

```bash
./scripts/register-signal.sh "$AMAN_NUMBER" --voice
```

If you do not want to use a personal number, use a hosted/silent SIM provider.

After receiving the code:

```bash
./scripts/signal-cli.sh -a "$AMAN_NUMBER" verify <CODE>
```

To reset local Signal state, use `./scripts/unlink-device.sh` (removes local data).

## 5) Start signal-cli daemon

Use the wrapper script (defaults to `HTTP_ADDR`):

```bash
./scripts/run-signal-daemon.sh
```

For additional daemon modes and JSON-RPC/SSE details, see `docs/signal-cli-daemon.md`.

## 6) Start Aman services

`agent-brain` ships a simple bot binary that wires the message listener and brain together.

You can also validate the daemon connection using the `signal-daemon` examples:

```bash
cargo run -p signal-daemon --example health_check
cargo run -p signal-daemon --example echo_bot
```

Start the AgentBrain bot:

```bash
export SQLITE_PATH="./data/aman.db"
export AMAN_NUMBER="+15551234567"
export HTTP_ADDR="127.0.0.1:8080"
cargo run -p agent-brain --bin agent_brain_bot
```

Optional: run the orchestrated bot (recommended):

```bash
# Using the dev script (requires MAPLE_API_KEY and GROK_API_KEY in .env)
./scripts/dev.sh

# Or manually
export MAPLE_API_KEY="..."
export GROK_API_KEY="..."
export AMAN_NUMBER="+15551234567"
cargo run -p orchestrator --example orchestrated_bot
```

Optional: run the message listener with a Brain implementation:

```bash
# Echo processor using mock-brain
cargo run -p message-listener --example processor_bot

# MapleBrain (OpenSecret) processor
export MAPLE_API_KEY="..."
cargo run -p message-listener --example maple_bot --features maple

# GrokBrain example (direct usage)
export GROK_API_KEY="..."
cargo run -p grok-brain --example test_chat

# MapleBrain + Grok tool executor (privacy-preserving realtime search)
export MAPLE_API_KEY="..."
export GROK_API_KEY="..."
cargo run -p maple-brain --example test_with_grok
```

For MapleBrain configuration details, see `docs/OPENSECRET_API.md`.
You can edit the default prompt at `crates/maple-brain/PROMPT.md` or point `MAPLE_PROMPT_FILE`
to a custom prompt.

Note: attachment file paths are resolved relative to the signal-cli data directory.
If you run signal-cli with a custom `--config` path, ensure your service uses the
matching data directory (or set `XDG_DATA_HOME`) so MapleBrain can load images.

## 7) Send a test message

From your Signal app, send a message to Aman's number. You should see:

- `message_listener` log an inbound message.
- `agent_brain` respond with onboarding.
- `broadcaster` send a reply.

Optional: send a test message via JSON-RPC:

```bash
./scripts/send-message.sh +15551234567 "Hello from JSON-RPC"
```

## 8) Run the Aman API (optional)

Start the OpenAI-compatible API gateway:

```bash
export AMAN_API_ADDR="127.0.0.1:8787"
export AMAN_API_TOKEN="aman-local"
export AMAN_API_MODEL="aman-chat"
export AMAN_KB_PATH="./knowledge"
cargo run -p api
```

## 9) Run the web UI (optional)

```bash
cd web
cat <<'EOF' > .env.local
AMAN_API_BASE_URL=http://127.0.0.1:8787
AMAN_API_KEY=aman-local
AMAN_API_MODEL=aman-chat
EOF
npm install
npm run dev
```

Open http://localhost:3000 in your browser.

## 10) Run admin web (optional)

The admin web UI provides dashboard stats and a broadcast tool:

```bash
export ADMIN_ADDR="127.0.0.1:8788"
export SQLITE_PATH="./data/aman.db"
export SIGNAL_DAEMON_URL="http://127.0.0.1:8080"
export AMAN_NUMBER="+15551234567"
cargo run -p admin-web
```

Open http://127.0.0.1:8788 in your browser.
Bind to localhost only or place behind auth for non-dev use.

## 11) Ingest local knowledge (Nostr flow)

Create or update chunked Nostr entries using the local knowledge file:

```bash
cargo run -p ingester -- \
  --file knowledge/using-ai-to-improve-movements-effectiveness.md \
  --out-dir ./data/ingest \
  --index-db ./data/nostr.db
```

Then start the API with Nostr DB enabled:

```bash
export NOSTR_DB_PATH="./data/nostr.db"
export AMAN_API_ADDR="127.0.0.1:8787"
export AMAN_API_TOKEN="aman-local"
cargo run -p api
```

## 12) Simulate a RegionEvent

Create a local event file:

```bash
cat <<'JSON' > /tmp/region-event.json
{
  "region": "Iran",
  "kind": "outage",
  "severity": "urgent",
  "confidence": "high",
  "summary": "Reported nationwide connectivity disruption.",
  "source_refs": ["https://example.org/report"],
  "ts": "2025-01-01T12:00:00Z"
}
JSON
```

Send it via the AgentBrain fanout helper:

```bash
cargo run -p agent-brain --bin region_event_send -- /tmp/region-event.json
```

Post it to the event intake endpoint (future target):

```bash
curl -s -X POST http://127.0.0.1:9001/events \
  -H 'content-type: application/json' \
  --data @/tmp/region-event.json
```

You should see outbound alerts sent to all subscribers of the region.

## 13) Run Nostr indexer locally (Phase 2 foundation)

Pick relays that retain custom kinds. Relay retention varies by operator (see NIP-11).

Start the indexer (SQLite will be created if missing):

```bash
cargo run -p nostr-persistence --bin nostr-indexer -- \
  --relay wss://relay.damus.io \
  --db ./data/nostr.db
```

Publish a fixture DocManifest:

```bash
cargo run -p nostr-persistence --bin nostr-publish-fixture -- \
  --relay wss://relay.damus.io \
  --key <NOSTR_SECRET_KEY> \
  --doc crates/nostr-persistence/fixtures/doc.json
```

If encryption is enabled, set a key:

```bash
export NOSTR_SECRETBOX_KEY="hex:32-byte-hex-key"
```

Verify SQLite tables:

```bash
sqlite3 ./data/nostr.db "select doc_id, title from docs;"
```

## Troubleshooting

- Not receiving messages:
  - Confirm `signal-cli` daemon is running and reachable.
  - Ensure incoming messages are received regularly (daemon or receive loop).
  - Check filesystem permissions for the `signal-cli` data path.
- Send failures:
  - Verify the account is registered and can send via `signal-cli`.
  - Check rate limits and retry backoff settings.
- Double replies after restart:
  - Ensure inbound dedupe is enabled and persisted.
- Storage path issues:
  - Confirm the configured `SQLITE_PATH` and signal-cli data directories exist.

## Security notes

- Treat the `signal-cli` data path as sensitive material.
- Do not log message bodies by default.
- Keep test accounts and production accounts separate.
