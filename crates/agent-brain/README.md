# agent-brain

## Responsibility

Core decision layer for Aman. AgentBrain handles user management and basic message processing.
It implements the shared `Brain` trait so it can be used directly with the message listener.

## Public interfaces

Consumes:

- `InboundMessage` (from `message-listener`)

Produces:

- `OutboundMessage` (to `message-listener` processor)
- User records in SQLite (via `database`)

## Commands (MVP)

- `help`
- `status`

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

## Failure modes

- Missing or invalid `SQLITE_PATH`.
- signal-cli daemon unavailable when sending responses.

## Future work

- Dedupe logic for inbound messages.
- RAG integration for richer responses.
- Nostr-backed document metadata ingestion.

## Security notes

- Do not log message bodies by default.
- Treat the SQLite database and signal-cli storage as sensitive.
