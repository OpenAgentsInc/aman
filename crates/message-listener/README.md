# message-listener

## Responsibility

The message listener owns Signal inbound transport. It connects to the signal-cli daemon via the `signal-daemon` crate,
normalizes inbound messages into `InboundMessage` values (including attachment metadata), and can optionally run a
`MessageProcessor` to invoke a Brain implementation and send responses.

## Public interfaces

Consumes:

- SSE stream from signal-cli daemon (via `signal-daemon`)

Produces:

- `InboundMessage` (brain-core)
- `OutboundMessage` (brain-core)

Processing:

- `MessageProcessor` calls a `Brain` implementation and sends replies via `signal-daemon`.
- `MessageProcessor` resolves attachment paths using `DaemonConfig` (defaults to the signal-cli data dir).
- For queue-based systems, persist `InboundMessage` to your store for `agent_brain` to consume.

## Signal-cli mode

Preferred:

- signal-cli daemon `--http` with SSE events (`/api/v1/events`).

Fallback:

- `signal-cli receive` polling loop (manual mode).

## How to run it

This crate is a library. Use it from a service binary or run the examples.

Examples (requires a running signal-cli daemon):

```bash
# Echo processor using mock-brain
cargo run -p message-listener --example processor_bot

# MapleBrain (OpenSecret) processor
export MAPLE_API_KEY="..."
cargo run -p message-listener --example maple_bot --features maple
```

For daemon setup, see `docs/signal-cli-daemon.md`.
If signal-cli uses a custom data directory (`--config`), ensure your client
config uses the matching directory (via `DaemonConfig::with_data_dir`) so
attachment paths resolve correctly.

## How to test it

- `cargo test -p message-listener`
- Use the examples above with a local signal-cli daemon.

## Failure modes

- signal-cli daemon not running or unreachable.
- Duplicate deliveries without dedupe persistence.
- Attachments present but files missing or inaccessible.
- Attachment-only messages are currently skipped (no text content).
- MapleBrain config/attestation failures when using OpenSecret.

## Roadmap

- See `ROADMAP.md` for planned RAG and Nostr phases. The listener remains the inbound Signal transport layer.

## Security notes

- Do not log raw message bodies or attachment file paths by default.
- Protect signal-cli storage paths and credentials.
