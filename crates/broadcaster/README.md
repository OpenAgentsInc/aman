# broadcaster

## Responsibility

The broadcaster owns outbound delivery to Signal identities. It accepts `OutboundMessage` records and sends them via
`signal-cli`, applying chunking, retries, and rate limits.

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

MVP target command (adjust to your runtime):

```bash
cargo run --bin broadcaster -- --rpc-url "$SIGNAL_CLI_RPC_URL" --db "$SQLITE_PATH"
```

## How to test it

- `cargo test`
- Use a test Signal account and send a short message.

## Failure modes

- `signal-cli` send failures (registration, rate limits).
- Duplicate sends after restart if state is not persisted.
- Oversized messages without chunking.

## Roadmap

- See `ROADMAP.md` for planned Web UI and RAG phases. The broadcaster remains the outbound Signal delivery layer.

## Security notes

- Avoid logging full outbound content in production.
- Honor opt-out state before sending alerts.
