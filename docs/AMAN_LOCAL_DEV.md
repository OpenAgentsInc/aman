# Aman Local Dev Runbook

This runbook is for a local MVP setup using `signal-cli` and the Aman services.

Scope: this runbook covers the Signal MVP only. RAG and Nostr components are planned and documented in
`ROADMAP.md`.

## 1) Prereqs

- Java 21+ for `signal-cli`.
- A Signal phone number for Aman (SMS or voice verification).
- An OpenAI-compatible API key.
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
# Optional override:
# export SIGNAL_CLI_JAR="build/signal-cli.jar"
export SQLITE_PATH="./data/aman.db"
export OPENAI_API_KEY="..."
export MODEL="gpt-5"
export STORE_OPENAI_RESPONSES="false"
export REGION_POLL_INTERVAL_SECONDS="60"
export LOG_LEVEL="info"
```

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

`message_listener`, `broadcaster`, and `agent_brain` are libraries. If you have service binaries in your environment,
start them now and configure the signal-daemon base URL (`http://$HTTP_ADDR`) plus `SQLITE_PATH`.

You can also validate the daemon connection using the `signal-daemon` examples:

```bash
cargo run -p signal-daemon --example health_check
cargo run -p signal-daemon --example echo_bot
```

## 7) Send a test message

From your Signal app, send a message to Aman's number. You should see:

- `message_listener` log an inbound message.
- `agent_brain` respond with onboarding.
- `broadcaster` send a reply.

Optional: send a test message via JSON-RPC:

```bash
./scripts/send-message.sh +15551234567 "Hello from JSON-RPC"
```

## 8) Run the web UI (optional)

```bash
cd web
cat <<'EOF' > .env.local
OPENAI_API_KEY=sk-...
EOF
npm install
npm run dev
```

Open http://localhost:3000 in your browser.

## 9) Simulate a RegionEvent

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

Post it to the event intake endpoint (MVP target):

```bash
curl -s -X POST http://127.0.0.1:9001/events \
  -H 'content-type: application/json' \
  --data @/tmp/region-event.json
```

You should see outbound alerts sent to all subscribers of the region.

## 10) Run Nostr indexer locally (Phase 2 foundation)

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
