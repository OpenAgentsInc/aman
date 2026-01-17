# Aman Local Dev Runbook

This runbook is for a local MVP setup using `signal-cli` and the Aman services.

Scope: this runbook covers the Signal MVP only. Web UI, RAG, and Nostr components are planned and documented in
`ROADMAP.md`.

## 1) Prereqs

- `signal-cli` installed and working.
- A Signal phone number for Aman (SMS or voice verification).
- Java runtime required by `signal-cli`.
- An OpenAI-compatible API key.

## 2) Environment

Create a local `.env` or export variables:

```bash
export AMAN_NUMBER="+15551234567"
export SIGNAL_CLI_PATH="/usr/local/bin/signal-cli"
export SIGNAL_CLI_RPC_URL="http://127.0.0.1:8081/api/v1/rpc"
export SQLITE_PATH="./data/aman.sqlite"
export OPENAI_API_KEY="..."
export MODEL="gpt-5"
export STORE_OPENAI_RESPONSES="false"
export REGION_POLL_INTERVAL_SECONDS="60"
export LOG_LEVEL="info"
```

## 3) Register Aman's Signal number

```bash
$SIGNAL_CLI_PATH -a "$AMAN_NUMBER" register
$SIGNAL_CLI_PATH -a "$AMAN_NUMBER" verify <CODE>
```

If SMS fails, retry with voice verification:

```bash
$SIGNAL_CLI_PATH -a "$AMAN_NUMBER" register --voice
```

If you do not want to use a personal number, use a hosted/silent SIM provider.

## 4) Start signal-cli daemon

Use JSON-RPC over HTTP so services can subscribe to events:

```bash
$SIGNAL_CLI_PATH -a "$AMAN_NUMBER" daemon --http 127.0.0.1:8081
```

## 5) Start Aman services

Run each service in its own terminal. These commands assume service binaries exist; adjust to your runtime.

```bash
cd crates/message-listener
cargo run --bin message-listener -- \
  --rpc-url "$SIGNAL_CLI_RPC_URL" \
  --db "$SQLITE_PATH"
```

```bash
cd crates/agent-brain
cargo run --bin agent-brain -- \
  --db "$SQLITE_PATH" \
  --model "$MODEL"
```

```bash
cd crates/broadcaster
cargo run --bin broadcaster -- \
  --rpc-url "$SIGNAL_CLI_RPC_URL" \
  --db "$SQLITE_PATH"
```

## 6) Send a test message

From your Signal app, send a message to Aman's number. You should see:

- `message_listener` log an inbound message.
- `agent_brain` respond with onboarding.
- `broadcaster` send a reply.

## 7) Simulate a RegionEvent

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
