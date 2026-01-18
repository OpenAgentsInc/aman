# nostr-persistence

## Responsibility

Publishes and indexes Nostr events for document metadata and memory durability (preferences, summaries, tool history,
clear-context events). This crate provides publishers (write path), an indexer (read path), and a rehydration helper
that materializes events into SQLite.

Built on rust-nostr (`nostr-sdk`).

## Public interfaces

Consumes:

- DocManifest, ChunkRef, AccessPolicy
- AmanPreferenceEvent, AmanSummaryEvent, AmanToolHistoryEvent, AmanClearContextEvent
- relay URLs and signing keys

Produces:

- Nostr events (parameterized replaceable kinds)
- local SQLite tables: nostr_events, docs, chunks (with optional inline `text`), policies, nostr_memory_*

Traits:

- `NostrPublisher` (publish_doc_manifest, publish_chunk_ref, publish_policy)
- `NostrMemoryPublisher` (publish_preference, publish_summary, publish_tool_history, publish_clear_context)
- `NostrIndexer` (start, backfill, handle_event)

## Event kinds and tags

- DocManifest: kind 30090, tag d=doc_id
- ChunkRef: kind 30091, tag d=chunk_id (optional inline `text` for worker-friendly retrieval)
- AccessPolicy: kind 30092, tag d=scope_id
- AmanPreference: kind 30093, tag d=<history_key>:preference
- AmanSummary: kind 30094, tag d=<history_key>:summary
- AmanToolHistoryEntry: kind 30095, tag d=<history_key>:<hash>
- AmanClearContextEvent: kind 30096, tag d=<history_key>:<hash>
- Required tags: d, k, enc (if encrypted)

## How to run it

Publish a fixture:

```bash
cargo run -p nostr-persistence --bin nostr-publish-fixture -- \
  --relay wss://relay.damus.io \
  --key <NOSTR_SECRET_KEY> \
  --doc crates/nostr-persistence/fixtures/doc.json
```

Start the indexer:

```bash
cargo run -p nostr-persistence --bin nostr-indexer -- \
  --relay wss://relay.damus.io \
  --db ./data/nostr.db
```

Rehydrate memory into the runtime DB:

```bash
cargo run -p nostr-persistence --bin nostr-rehydrate-memory -- \
  --relay wss://relay.damus.io \
  --nostr-db ./data/nostr.db \
  --aman-db ./data/aman.db
```

## How to test it

- `cargo test -p nostr-persistence`
- Integration test (ignored by default):
  - set `NOSTR_TEST_RELAY` and `NOSTR_TEST_KEY`
  - run `cargo test -p nostr-persistence -- --ignored`

## Failure modes

- Relay retention policies drop custom kinds (NIP-11).
- Missing or mismatched `enc` tag prevents decrypting content.
- Quorum publish fails if too few relays ack the event.

## Security notes

- `NOSTR_SECRETBOX_KEY` enables symmetric encryption of payloads.
- Avoid publishing sensitive plaintext unless encryption is enabled.
