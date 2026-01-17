# broadcaster

## Responsibility

The broadcaster owns outbound delivery to Signal identities. It accepts `OutboundMessage` records and sends them via
the signal-cli daemon using the `signal-daemon` crate, applying chunking, retries, and rate limits.

## Public interfaces

Consumes:

- `OutboundMessage` with fields: `recipient_id`, `body`, `correlation_id`

Produces:

- send results (timestamp, status) persisted for auditing/dedupe

## Outbound semantics

- Chunk long responses to fit Signal message limits.
- Retry transient failures with exponential backoff.
- Throttle per-recipient to avoid spam loops.

## How to run it

This crate is a library. Use it from a service binary that configures `signal-daemon` with the daemon base URL
(`http://$HTTP_ADDR`). For daemon setup, see `docs/signal-cli-daemon.md`.

## How to test it

- `cargo test`
- Use a test Signal account and send a short message.

## Failure modes

- `signal-cli` send failures (registration, rate limits).
- Duplicate sends after restart if state is not persisted.
- Oversized messages without chunking.

## Roadmap

- See `ROADMAP.md` for planned RAG and Nostr phases. The broadcaster remains the outbound Signal delivery layer.

## Security notes

- Avoid logging full outbound content in production.
- Honor opt-out state before sending alerts.
