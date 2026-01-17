# agent-brain

## Responsibility

Core decision layer for Aman. AgentBrain handles onboarding, subscription state, and regional alert routing.
It implements the shared `Brain` trait so it can be used directly with the message listener.

## Public interfaces

Consumes:

- `InboundMessage` (from `message-listener`)
- `RegionEvent` (for alert fanout)

Produces:

- `OutboundMessage` (to `broadcaster` or `message-listener` processor)
- Subscription updates in SQLite (via `database`)

## Onboarding and state machine

- New contacts are prompted to opt in to regional alerts.
- Regions are parsed from user input ("Iran", "Syria", etc.).
- Users can opt out with "stop" or "unsubscribe".

## Subscription storage

Uses the `database` crate (SQLite) for users, topics, and notifications.

## Commands (MVP)

- `help`
- `status`
- `subscribe <region>`
- `region <region>`
- `stop` / `unsubscribe`

## Run (Signal bot)

```bash
export SQLITE_PATH="./data/aman.db"
export AMAN_DEFAULT_LANGUAGE="English"
export AMAN_NUMBER="+15551234567"
export HTTP_ADDR="127.0.0.1:8080"

cargo run -p agent-brain --bin agent_brain_bot
```

Optional multi-account mode:

```bash
export SIGNAL_DAEMON_ACCOUNT="+15551234567"
```

## Send a regional event

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

cargo run -p agent-brain --bin region_event_send -- /tmp/region-event.json
```

## Failure modes

- Missing or invalid `SQLITE_PATH`.
- Unknown region input (responds with help/choices).
- signal-cli daemon unavailable when sending alerts.

## Future work

- Dedupe logic for inbound messages.
- RAG integration for richer responses.
- Nostr-backed document metadata ingestion.

## Security notes

- Do not log message bodies by default.
- Treat the SQLite database and signal-cli storage as sensitive.
