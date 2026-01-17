# ingester

Document ingester that chunks files and publishes/indexes Nostr events.

## Responsibilities

- Read a local document.
- Chunk into fixed-size pieces with overlap.
- Write chunk files to disk and set `blob_ref` to file paths.
- Publish DocManifest + ChunkRef events (optional).
- Index directly into a local Nostr SQLite DB (optional).

## Run (local index only)

```bash
cargo run -p ingester -- \
  --file knowledge/using-ai-to-improve-movements-effectiveness.md \
  --out-dir ./data/ingest \
  --index-db ./data/nostr.db
```

## Run (publish to relays)

```bash
export NOSTR_SECRET_KEY=hex:...
cargo run -p ingester -- \
  --file knowledge/using-ai-to-improve-movements-effectiveness.md \
  --out-dir ./data/ingest \
  --relay wss://relay.damus.io
```

## Notes

- `--index-db` uses the local Nostr schema directly and does not require relays.
- `--relay` requires a Nostr secret key (via `--key` or `NOSTR_SECRET_KEY`).
