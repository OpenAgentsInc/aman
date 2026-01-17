# message_listener

## Responsibility

The message listener owns Signal inbound transport. It receives encrypted messages via `signal-cli`, normalizes them, and
writes `InboundMessage` records to the shared state store for `agent_brain` to consume.

## Public interfaces

Consumes:

- `signal-cli` events (daemon mode JSON-RPC + SSE preferred for MVP)

Produces:

- `InboundMessage` with fields: `message_id`, `sender_id`, `ts`, `body`, `source_device`
- persisted inbound records for dedupe and replay safety

Handoff to agent_brain:

- MVP intent: write to SQLite `messages` table and an `inbound_queue` table.

## Signal-cli mode

Preferred:

- `signal-cli daemon --http` with JSON-RPC and SSE events (`/api/v1/events`).

Fallback:

- `signal-cli receive` polling loop (manual mode).

## How to run it

MVP target command (adjust to your runtime):

```bash
cargo run --bin message-listener -- --rpc-url "$SIGNAL_CLI_RPC_URL" --db "$SQLITE_PATH"
```

## How to test it

- `cargo test`
- Inject a sample JSON-RPC `receive` payload and confirm normalization.

## Failure modes

- `signal-cli` daemon not running or unreachable.
- Missed messages if receive loop is not active.
- Duplicate deliveries without dedupe persistence.

## Roadmap

- See `ROADMAP.md` for planned RAG and Nostr phases. The listener remains the inbound Signal transport layer.

## Security notes

- Do not log raw message bodies by default.
- Protect signal-cli storage paths and credentials.
