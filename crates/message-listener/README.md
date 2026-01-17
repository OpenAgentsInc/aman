# message_listener

## Responsibility

The message listener owns Signal inbound transport. It connects to the signal-cli daemon via the `signal-daemon` crate,
normalizes inbound messages, and writes `InboundMessage` records to the shared state store for `agent_brain` to consume.

## Public interfaces

Consumes:

- SSE stream from signal-cli daemon (via `signal-daemon`)

Produces:

- `InboundMessage` with fields: `message_id`, `sender_id`, `ts`, `body`, `source_device`
- persisted inbound records for dedupe and replay safety

Handoff to agent_brain:

- MVP intent: write to SQLite `messages` table and an `inbound_queue` table.

## Signal-cli mode

Preferred:

- signal-cli daemon `--http` with SSE events (`/api/v1/events`).

Fallback:

- `signal-cli receive` polling loop (manual mode).

## How to run it

This crate is a library. Use it from a service binary that configures `signal-daemon` with the daemon base URL
(`http://$HTTP_ADDR`). For daemon setup, see `docs/signal-cli-daemon.md`.

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
